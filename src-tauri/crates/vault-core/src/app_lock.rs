use crate::{
    config::{ProjectPaths, ensure_paths, save_config},
    models::{AppConfig, AppLockStatus, SetAppLockPasscodeRequest, UnlockAppSessionRequest},
    utils::now_rfc3339,
};
use anyhow::{Context, Result, bail};
use getrandom::fill as fill_random;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{fs, path::PathBuf};

const PASSCODE_MIN_LENGTH: usize = 4;
const PASSCODE_ROUNDS: usize = 120_000;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct AppLockStateRecord {
    locked: bool,
    lock_reason: Option<String>,
    locked_at: Option<String>,
    last_unlocked_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AppLockSecretRecord {
    salt_hex: String,
    hash_hex: String,
    recovery_hint: Option<String>,
}

fn app_lock_state_path(paths: &ProjectPaths) -> PathBuf {
    paths.app_root.join("app-lock-state.json")
}

fn app_lock_secret_path(paths: &ProjectPaths) -> PathBuf {
    paths.app_root.join("app-lock-passcode.json")
}

fn load_app_lock_state(paths: &ProjectPaths, config: &AppConfig) -> Result<AppLockStateRecord> {
    if !config.app_lock.enabled {
        return Ok(AppLockStateRecord::default());
    }

    let path = app_lock_state_path(paths);
    if !path.exists() {
        return Ok(AppLockStateRecord::default());
    }

    let content =
        fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
    serde_json::from_str(&content).context("parsing app lock session state")
}

fn save_app_lock_state(paths: &ProjectPaths, state: &AppLockStateRecord) -> Result<()> {
    ensure_paths(paths)?;
    let path = app_lock_state_path(paths);
    let content = serde_json::to_string_pretty(state)?;
    fs::write(&path, content).with_context(|| format!("writing {}", path.display()))?;
    Ok(())
}

fn clear_app_lock_state(paths: &ProjectPaths) -> Result<()> {
    let path = app_lock_state_path(paths);
    if path.exists() {
        fs::remove_file(&path).with_context(|| format!("removing {}", path.display()))?;
    }
    Ok(())
}

fn load_app_lock_secret(paths: &ProjectPaths) -> Result<Option<AppLockSecretRecord>> {
    let path = app_lock_secret_path(paths);
    if !path.exists() {
        return Ok(None);
    }

    let content =
        fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
    serde_json::from_str(&content).context("parsing app lock secret").map(Some)
}

fn save_app_lock_secret(paths: &ProjectPaths, secret: &AppLockSecretRecord) -> Result<()> {
    ensure_paths(paths)?;
    let path = app_lock_secret_path(paths);
    let content = serde_json::to_string_pretty(secret)?;
    fs::write(&path, content).with_context(|| format!("writing {}", path.display()))?;
    Ok(())
}

fn clear_app_lock_secret(paths: &ProjectPaths) -> Result<()> {
    let path = app_lock_secret_path(paths);
    if path.exists() {
        fs::remove_file(&path).with_context(|| format!("removing {}", path.display()))?;
    }
    Ok(())
}

fn biometric_available() -> bool {
    false
}

fn biometric_note() -> Option<&'static str> {
    if cfg!(target_os = "linux") {
        Some(
            "Linux currently uses passcode-only app lock because biometric integration is not wired into this build.",
        )
    } else {
        Some(
            "Biometric unlock is reserved for future platform integration; this build currently falls back to the app-lock passcode.",
        )
    }
}

fn normalize_hint(value: Option<String>) -> Option<String> {
    value.and_then(|hint| {
        let trimmed = hint.trim();
        if trimmed.is_empty() { None } else { Some(trimmed.to_string()) }
    })
}

fn random_salt_hex() -> Result<String> {
    let mut bytes = [0u8; 16];
    fill_random(&mut bytes).context("generating app lock salt")?;
    Ok(hex::encode(bytes))
}

fn derive_passcode_hash(passcode: &str, salt_hex: &str) -> Result<String> {
    let salt = hex::decode(salt_hex).context("decoding app lock salt")?;
    let mut digest = {
        let mut hasher = Sha256::new();
        hasher.update(&salt);
        hasher.update(passcode.as_bytes());
        hasher.finalize().to_vec()
    };

    for _ in 1..PASSCODE_ROUNDS {
        let mut hasher = Sha256::new();
        hasher.update(&salt);
        hasher.update(&digest);
        hasher.update(passcode.as_bytes());
        digest = hasher.finalize().to_vec();
    }

    Ok(hex::encode(digest))
}

pub fn hydrate_app_lock_config(paths: &ProjectPaths, config: &mut AppConfig) -> Result<()> {
    config.app_lock.idle_timeout_minutes = config.app_lock.idle_timeout_minutes.clamp(1, 60);
    let secret = load_app_lock_secret(paths)?;
    config.app_lock.passcode_configured = secret.is_some();
    config.app_lock.recovery_hint = secret.and_then(|value| normalize_hint(value.recovery_hint));
    Ok(())
}

pub fn validate_app_lock_config(paths: &ProjectPaths, config: &AppConfig) -> Result<()> {
    if !config.app_lock.enabled {
        return Ok(());
    }

    if !config.app_lock.passcode_enabled && !config.app_lock.biometric_enabled {
        bail!("Enable a passcode before turning on App Lock in this build.");
    }

    if config.app_lock.biometric_enabled && !biometric_available() {
        bail!("Biometric unlock is not available in the current desktop build.");
    }

    if config.app_lock.passcode_enabled && load_app_lock_secret(paths)?.is_none() {
        bail!("Set an app lock passcode before turning on App Lock.");
    }

    Ok(())
}

pub fn app_lock_status(paths: &ProjectPaths, config: &AppConfig) -> Result<AppLockStatus> {
    let state = load_app_lock_state(paths, config)?;
    let mut warnings = Vec::new();
    let mut degradation_notes = vec![
        "App Lock only protects the PathKeep UI session. Archive encryption still protects data at rest."
            .to_string(),
    ];

    if config.app_lock.enabled
        && config.app_lock.passcode_enabled
        && !config.app_lock.passcode_configured
    {
        warnings.push(
            "Set an app lock passcode before relying on session lock on this device.".to_string(),
        );
    }

    if let Some(note) = biometric_note() {
        degradation_notes.push(note.to_string());
        if config.app_lock.biometric_enabled {
            warnings.push(note.to_string());
        }
    }

    Ok(AppLockStatus {
        enabled: config.app_lock.enabled,
        locked: config.app_lock.enabled && state.locked,
        idle_timeout_minutes: config.app_lock.idle_timeout_minutes,
        biometric_available: biometric_available(),
        biometric_enabled: config.app_lock.biometric_enabled,
        passcode_enabled: config.app_lock.passcode_enabled,
        passcode_configured: config.app_lock.passcode_configured,
        config_path: paths.config_path.display().to_string(),
        lock_reason: state.lock_reason,
        locked_at: state.locked_at,
        last_unlocked_at: state.last_unlocked_at,
        recovery_hint: config.app_lock.recovery_hint.clone(),
        warnings,
        degradation_notes,
    })
}

pub fn initialize_app_lock_session(paths: &ProjectPaths, config: &AppConfig) -> Result<()> {
    if !config.app_lock.enabled {
        clear_app_lock_state(paths)?;
        return Ok(());
    }

    save_app_lock_state(
        paths,
        &AppLockStateRecord {
            locked: true,
            lock_reason: Some("startup".to_string()),
            locked_at: Some(now_rfc3339()),
            last_unlocked_at: None,
        },
    )
}

pub fn ensure_app_lock_unlocked(paths: &ProjectPaths, config: &AppConfig) -> Result<()> {
    let status = app_lock_status(paths, config)?;
    if status.locked {
        bail!("PathKeep is currently locked. Unlock the app before requesting archive data.");
    }
    Ok(())
}

pub fn lock_app_session(
    paths: &ProjectPaths,
    config: &AppConfig,
    reason: Option<&str>,
) -> Result<AppLockStatus> {
    if !config.app_lock.enabled {
        return app_lock_status(paths, config);
    }

    save_app_lock_state(
        paths,
        &AppLockStateRecord {
            locked: true,
            lock_reason: Some(reason.unwrap_or("manual").to_string()),
            locked_at: Some(now_rfc3339()),
            last_unlocked_at: load_app_lock_state(paths, config)?.last_unlocked_at,
        },
    )?;
    app_lock_status(paths, config)
}

pub fn unlock_app_session(
    paths: &ProjectPaths,
    config: &AppConfig,
    request: &UnlockAppSessionRequest,
) -> Result<AppLockStatus> {
    if !config.app_lock.enabled {
        return app_lock_status(paths, config);
    }

    if request.use_biometric {
        bail!("Biometric unlock is not available in the current desktop build.");
    }

    if config.app_lock.passcode_enabled {
        let passcode = request
            .passcode
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .context("Enter the app lock passcode before unlocking PathKeep.")?;
        let secret = load_app_lock_secret(paths)?
            .context("Set an app lock passcode in Settings before unlocking with a passcode.")?;
        let derived = derive_passcode_hash(passcode, &secret.salt_hex)?;
        if derived != secret.hash_hex {
            bail!("The app lock passcode did not match.");
        }
    }

    let previous = load_app_lock_state(paths, config)?;
    save_app_lock_state(
        paths,
        &AppLockStateRecord {
            locked: false,
            lock_reason: None,
            locked_at: previous.locked_at,
            last_unlocked_at: Some(now_rfc3339()),
        },
    )?;
    app_lock_status(paths, config)
}

pub fn set_app_lock_passcode(
    paths: &ProjectPaths,
    config: &mut AppConfig,
    request: &SetAppLockPasscodeRequest,
) -> Result<AppLockStatus> {
    let passcode = request.passcode.trim();
    if passcode.len() < PASSCODE_MIN_LENGTH {
        bail!("App lock passcodes must be at least 4 characters long.");
    }

    let salt_hex = random_salt_hex()?;
    let secret = AppLockSecretRecord {
        hash_hex: derive_passcode_hash(passcode, &salt_hex)?,
        salt_hex,
        recovery_hint: normalize_hint(request.recovery_hint.clone()),
    };
    save_app_lock_secret(paths, &secret)?;

    config.app_lock.passcode_configured = true;
    config.app_lock.recovery_hint = secret.recovery_hint.clone();
    save_config(paths, config)?;
    app_lock_status(paths, config)
}

pub fn clear_app_lock_passcode(
    paths: &ProjectPaths,
    config: &mut AppConfig,
) -> Result<AppLockStatus> {
    clear_app_lock_secret(paths)?;
    clear_app_lock_state(paths)?;
    config.app_lock.enabled = false;
    config.app_lock.passcode_configured = false;
    config.app_lock.recovery_hint = None;
    save_config(paths, config)?;
    app_lock_status(paths, config)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{config::ProjectPaths, models::AppLockConfig};
    use tempfile::tempdir;

    fn temp_paths() -> ProjectPaths {
        let dir = tempdir().expect("tempdir");
        let root = dir.keep();
        ProjectPaths {
            app_root: root.clone(),
            config_path: root.join("config.json"),
            archive_database_path: root.join("archive/history-vault.sqlite"),
            audit_repo_path: root.join("audit"),
            manifests_dir: root.join("audit/manifests"),
            exports_dir: root.join("exports"),
            raw_snapshots_dir: root.join("raw-snapshots"),
            staging_dir: root.join("staging"),
            quarantine_dir: root.join("quarantine"),
            schedule_dir: root.join("schedule"),
            stronghold_path: root.join("vault.hold"),
            stronghold_salt_path: root.join("stronghold-salt.txt"),
        }
    }

    #[test]
    fn app_lock_session_round_trips_passcode_and_state() {
        let paths = temp_paths();
        let mut config = AppConfig {
            initialized: true,
            app_lock: AppLockConfig {
                enabled: false,
                idle_timeout_minutes: 10,
                biometric_enabled: false,
                passcode_enabled: true,
                passcode_configured: false,
                recovery_hint: None,
            },
            ..AppConfig::default()
        };

        let status = set_app_lock_passcode(
            &paths,
            &mut config,
            &SetAppLockPasscodeRequest {
                passcode: "2468".to_string(),
                recovery_hint: Some("only digits".to_string()),
            },
        )
        .expect("set passcode");
        assert!(status.passcode_configured);

        hydrate_app_lock_config(&paths, &mut config).expect("hydrate");
        config.app_lock.enabled = true;
        validate_app_lock_config(&paths, &config).expect("valid config");

        let locked = lock_app_session(&paths, &config, Some("manual")).expect("lock session");
        assert!(locked.locked);

        let error = unlock_app_session(
            &paths,
            &config,
            &UnlockAppSessionRequest { passcode: Some("9999".to_string()), use_biometric: false },
        )
        .expect_err("bad passcode");
        assert!(error.to_string().contains("did not match"));

        let unlocked = unlock_app_session(
            &paths,
            &config,
            &UnlockAppSessionRequest { passcode: Some("2468".to_string()), use_biometric: false },
        )
        .expect("unlock");
        assert!(!unlocked.locked);
        ensure_app_lock_unlocked(&paths, &config).expect("unlock guard");

        let cleared = clear_app_lock_passcode(&paths, &mut config).expect("clear passcode");
        assert!(!cleared.enabled);
        assert!(!cleared.passcode_configured);
    }
}
