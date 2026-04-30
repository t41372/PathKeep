//! App Lock session-boundary domain.
//!
//! App Lock is a UI/session privacy feature, not the same thing as archive
//! encryption. This module owns that distinction by managing lock state,
//! passcode hashes, biometric integration hooks, and the rules for when the UI
//! must refuse archive access until the session is unlocked again.

use crate::{
    config::{ProjectPaths, ensure_paths, save_config},
    models::{
        AppConfig, AppLockBiometricState, AppLockStatus, SetAppLockPasscodeRequest,
        UnlockAppSessionRequest,
    },
    utils::now_rfc3339,
};
use anyhow::{Context, Result, anyhow, bail};
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

fn default_biometric_state() -> AppLockBiometricState {
    AppLockBiometricState::Unsupported
}

fn biometric_available(state: AppLockBiometricState) -> bool {
    matches!(state, AppLockBiometricState::TouchIdAvailable)
}

fn biometric_note(state: AppLockBiometricState) -> Option<String> {
    biometric_note_for_platform(state, cfg!(target_os = "linux"))
}

fn biometric_note_for_platform(state: AppLockBiometricState, linux: bool) -> Option<String> {
    match state {
        AppLockBiometricState::TouchIdAvailable => Some(
            "Touch ID is available on this Mac and can unlock the current PathKeep session."
                .to_string(),
        ),
        AppLockBiometricState::TouchIdUnavailable => Some(
            "Touch ID is unavailable on this Mac right now, so PathKeep falls back to the app-lock passcode."
                .to_string(),
        ),
        AppLockBiometricState::Unsupported => {
            if linux {
                Some(
                    "Linux currently uses passcode-only app lock because biometric integration is not wired into this build."
                        .to_string(),
                )
            } else {
                Some(
                    "Biometric unlock is reserved for future platform integration; this build currently falls back to the app-lock passcode."
                        .to_string(),
                )
            }
        }
    }
}

fn biometric_unavailable_error(state: AppLockBiometricState) -> &'static str {
    match state {
        AppLockBiometricState::TouchIdUnavailable => {
            "Touch ID is unavailable on this Mac right now. Use the app lock passcode instead."
        }
        _ => "Biometric unlock is not available in the current desktop build.",
    }
}

/// Validates an App Lock config using an explicit biometric capability snapshot.
pub fn validate_app_lock_config_with_biometric(
    paths: &ProjectPaths,
    config: &AppConfig,
    biometric_state: AppLockBiometricState,
) -> Result<()> {
    if !config.app_lock.enabled {
        return Ok(());
    }

    if !config.app_lock.passcode_enabled && !config.app_lock.biometric_enabled {
        bail!("Enable a passcode before turning on App Lock in this build.");
    }

    if config.app_lock.biometric_enabled && !biometric_available(biometric_state) {
        bail!(biometric_unavailable_error(biometric_state));
    }

    if config.app_lock.passcode_enabled && load_app_lock_secret(paths)?.is_none() {
        bail!("Set an app lock passcode before turning on App Lock.");
    }

    Ok(())
}

/// Returns the shell-facing App Lock status using an explicit biometric snapshot.
pub fn app_lock_status_with_biometric(
    paths: &ProjectPaths,
    config: &AppConfig,
    biometric_state: AppLockBiometricState,
) -> Result<AppLockStatus> {
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

    if let Some(note) = biometric_note(biometric_state) {
        if config.app_lock.biometric_enabled && !biometric_available(biometric_state) {
            warnings.push(note.clone());
        }
        degradation_notes.push(note);
    }

    Ok(AppLockStatus {
        enabled: config.app_lock.enabled,
        locked: config.app_lock.enabled && state.locked,
        idle_timeout_minutes: config.app_lock.idle_timeout_minutes,
        biometric_available: biometric_available(biometric_state),
        biometric_enabled: config.app_lock.biometric_enabled,
        biometric_state,
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

fn normalize_hint(value: Option<String>) -> Option<String> {
    value.and_then(|hint| {
        let trimmed = hint.trim();
        if trimmed.is_empty() { None } else { Some(trimmed.to_string()) }
    })
}

fn random_salt_hex() -> Result<String> {
    let mut bytes = [0u8; 16];
    fill_random(&mut bytes).map_err(|error| anyhow!("generating app lock salt: {error}"))?;
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

/// Hydrates transient App Lock config fields from persisted secret/state files.
pub fn hydrate_app_lock_config(paths: &ProjectPaths, config: &mut AppConfig) -> Result<()> {
    config.app_lock.idle_timeout_minutes = config.app_lock.idle_timeout_minutes.clamp(1, 60);
    let secret = load_app_lock_secret(paths)?;
    config.app_lock.passcode_configured = secret.is_some();
    config.app_lock.recovery_hint = secret.and_then(|value| normalize_hint(value.recovery_hint));
    Ok(())
}

/// Validates an App Lock config against the current platform's biometric capability.
pub fn validate_app_lock_config(paths: &ProjectPaths, config: &AppConfig) -> Result<()> {
    validate_app_lock_config_with_biometric(paths, config, default_biometric_state())
}

/// Returns the current App Lock status using the default platform biometric state.
pub fn app_lock_status(paths: &ProjectPaths, config: &AppConfig) -> Result<AppLockStatus> {
    app_lock_status_with_biometric(paths, config, default_biometric_state())
}

/// Initializes the session state used by App Lock when the app starts.
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

/// Returns an error when the current App Lock session is still locked.
pub fn ensure_app_lock_unlocked(paths: &ProjectPaths, config: &AppConfig) -> Result<()> {
    let status = app_lock_status(paths, config)?;
    if status.locked {
        bail!("PathKeep is currently locked. Unlock the app before requesting archive data.");
    }
    Ok(())
}

/// Locks the current App Lock session and returns the updated status.
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
    app_lock_status_with_biometric(paths, config, default_biometric_state())
}

/// Unlocks the session using either passcode or an injected biometric callback.
pub fn unlock_app_session_with_biometric<F>(
    paths: &ProjectPaths,
    config: &AppConfig,
    request: &UnlockAppSessionRequest,
    biometric_state: AppLockBiometricState,
    authenticate_biometric: F,
) -> Result<AppLockStatus>
where
    F: FnOnce() -> Result<()>,
{
    if !config.app_lock.enabled {
        return app_lock_status_with_biometric(paths, config, biometric_state);
    }

    if request.use_biometric {
        if !config.app_lock.biometric_enabled {
            bail!("Biometric unlock is currently turned off in Settings.");
        }
        if !biometric_available(biometric_state) {
            bail!(biometric_unavailable_error(biometric_state));
        }
        authenticate_biometric()?;
    } else if config.app_lock.passcode_enabled {
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
    } else {
        bail!("PathKeep cannot unlock without an enabled app lock credential.");
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
    app_lock_status_with_biometric(paths, config, biometric_state)
}

/// Unlocks the session using the default non-biometric fallback behavior.
pub fn unlock_app_session(
    paths: &ProjectPaths,
    config: &AppConfig,
    request: &UnlockAppSessionRequest,
) -> Result<AppLockStatus> {
    unlock_app_session_with_biometric(
        paths,
        config,
        request,
        default_biometric_state(),
        default_biometric_unlock_unavailable,
    )
}

fn default_biometric_unlock_unavailable() -> Result<()> {
    bail!("Biometric unlock is not available in the current desktop build.")
}

/// Stores a new App Lock passcode and returns the updated status.
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
    app_lock_status_with_biometric(paths, config, default_biometric_state())
}

/// Clears the App Lock passcode, disables App Lock, and returns the updated status.
pub fn clear_app_lock_passcode(
    paths: &ProjectPaths,
    config: &mut AppConfig,
) -> Result<AppLockStatus> {
    config.app_lock.enabled = false;
    config.app_lock.passcode_configured = false;
    config.app_lock.recovery_hint = None;
    save_config(paths, config)?;
    clear_app_lock_state(paths)?;
    clear_app_lock_secret(paths)?;
    app_lock_status_with_biometric(paths, config, default_biometric_state())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        config::{ProjectPaths, project_paths_with_root},
        models::AppLockConfig,
    };
    use tempfile::tempdir;

    fn temp_paths() -> ProjectPaths {
        let dir = tempdir().expect("tempdir");
        let root = dir.keep();
        project_paths_with_root(&root)
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

        initialize_app_lock_session(&paths, &config).expect("initialize startup lock");
        let startup = app_lock_status(&paths, &config).expect("startup status");
        assert!(startup.locked);
        assert_eq!(startup.lock_reason.as_deref(), Some("startup"));
        let guard = ensure_app_lock_unlocked(&paths, &config).expect_err("startup lock guard");
        assert!(guard.to_string().contains("currently locked"));

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
        initialize_app_lock_session(&paths, &config).expect("disabled clears startup state");
        assert!(!app_lock_state_path(&paths).exists());
    }

    #[test]
    fn biometric_status_and_unlock_follow_the_supplied_capability() {
        let paths = temp_paths();
        let mut config = AppConfig {
            initialized: true,
            app_lock: AppLockConfig {
                enabled: true,
                idle_timeout_minutes: 5,
                biometric_enabled: true,
                passcode_enabled: true,
                passcode_configured: false,
                recovery_hint: None,
            },
            ..AppConfig::default()
        };

        set_app_lock_passcode(
            &paths,
            &mut config,
            &SetAppLockPasscodeRequest { passcode: "2468".to_string(), recovery_hint: None },
        )
        .expect("set passcode");
        hydrate_app_lock_config(&paths, &mut config).expect("hydrate");

        validate_app_lock_config_with_biometric(
            &paths,
            &config,
            AppLockBiometricState::TouchIdAvailable,
        )
        .expect("biometric-capable config");

        let status = app_lock_status_with_biometric(
            &paths,
            &config,
            AppLockBiometricState::TouchIdAvailable,
        )
        .expect("status");
        assert!(status.biometric_available);
        assert_eq!(status.biometric_state, AppLockBiometricState::TouchIdAvailable);

        let unavailable = validate_app_lock_config_with_biometric(
            &paths,
            &config,
            AppLockBiometricState::TouchIdUnavailable,
        )
        .expect_err("touch id unavailable");
        assert!(unavailable.to_string().contains("Touch ID is unavailable"));

        lock_app_session(&paths, &config, Some("manual")).expect("lock");
        let unlocked = unlock_app_session_with_biometric(
            &paths,
            &config,
            &UnlockAppSessionRequest { passcode: None, use_biometric: true },
            AppLockBiometricState::TouchIdAvailable,
            || Ok(()),
        )
        .expect("touch id unlock");
        assert!(!unlocked.locked);

        let canceled = unlock_app_session_with_biometric(
            &paths,
            &config,
            &UnlockAppSessionRequest { passcode: None, use_biometric: true },
            AppLockBiometricState::TouchIdAvailable,
            || bail!("Touch ID unlock was canceled."),
        )
        .expect_err("canceled");
        assert!(canceled.to_string().contains("Touch ID unlock was canceled"));

        config.app_lock.biometric_enabled = false;
        let disabled = unlock_app_session_with_biometric(
            &paths,
            &config,
            &UnlockAppSessionRequest { passcode: None, use_biometric: true },
            AppLockBiometricState::TouchIdAvailable,
            || Ok(()),
        )
        .expect_err("biometric disabled");
        assert!(disabled.to_string().contains("turned off in Settings"));
    }

    #[test]
    fn unlock_rejects_no_auth_branch_when_passcode_and_biometric_are_disabled() {
        let paths = temp_paths();
        let config = AppConfig {
            initialized: true,
            app_lock: AppLockConfig {
                enabled: true,
                idle_timeout_minutes: 5,
                biometric_enabled: false,
                passcode_enabled: false,
                passcode_configured: false,
                recovery_hint: None,
            },
            ..AppConfig::default()
        };

        let error = unlock_app_session_with_biometric(
            &paths,
            &config,
            &UnlockAppSessionRequest { passcode: None, use_biometric: false },
            AppLockBiometricState::Unsupported,
            || Ok(()),
        )
        .expect_err("missing auth should fail");
        assert!(error.to_string().contains("cannot unlock without an enabled app lock credential"));
    }

    #[test]
    fn validation_status_and_unlock_cover_unavailable_edges() {
        let paths = temp_paths();
        let no_credentials = AppConfig {
            initialized: true,
            app_lock: AppLockConfig {
                enabled: true,
                idle_timeout_minutes: 5,
                biometric_enabled: false,
                passcode_enabled: false,
                passcode_configured: false,
                recovery_hint: None,
            },
            ..AppConfig::default()
        };
        let no_credential_error = validate_app_lock_config_with_biometric(
            &paths,
            &no_credentials,
            AppLockBiometricState::Unsupported,
        )
        .expect_err("app lock requires a credential");
        assert!(no_credential_error.to_string().contains("Enable a passcode"));

        let mut missing_secret = no_credentials.clone();
        missing_secret.app_lock.passcode_enabled = true;
        let missing_secret_error = validate_app_lock_config_with_biometric(
            &paths,
            &missing_secret,
            AppLockBiometricState::Unsupported,
        )
        .expect_err("passcode config requires stored secret");
        assert!(missing_secret_error.to_string().contains("Set an app lock passcode"));
        let missing_secret_status = app_lock_status_with_biometric(
            &paths,
            &missing_secret,
            AppLockBiometricState::Unsupported,
        )
        .expect("missing secret status");
        assert!(
            missing_secret_status
                .warnings
                .iter()
                .any(|warning| warning.contains("Set an app lock passcode"))
        );
        let default_biometric_error =
            default_biometric_unlock_unavailable().expect_err("default biometric unavailable");
        assert!(default_biometric_error.to_string().contains("not available"));

        let short_passcode = set_app_lock_passcode(
            &paths,
            &mut missing_secret,
            &SetAppLockPasscodeRequest { passcode: "123".to_string(), recovery_hint: None },
        )
        .expect_err("short passcode");
        assert!(short_passcode.to_string().contains("at least 4"));

        let mut biometric_config = missing_secret;
        set_app_lock_passcode(
            &paths,
            &mut biometric_config,
            &SetAppLockPasscodeRequest { passcode: "1234".to_string(), recovery_hint: None },
        )
        .expect("set passcode");
        biometric_config.app_lock.enabled = true;
        biometric_config.app_lock.biometric_enabled = true;
        hydrate_app_lock_config(&paths, &mut biometric_config).expect("hydrate");

        let status = app_lock_status_with_biometric(
            &paths,
            &biometric_config,
            AppLockBiometricState::TouchIdUnavailable,
        )
        .expect("status");
        assert!(status.warnings.iter().any(|warning| warning.contains("Touch ID is unavailable")));
        assert!(
            status.degradation_notes.iter().any(|note| note.contains("Touch ID is unavailable"))
        );
        assert!(
            biometric_note_for_platform(AppLockBiometricState::Unsupported, true)
                .expect("linux note")
                .contains("Linux currently uses passcode-only")
        );

        let unavailable_unlock = unlock_app_session_with_biometric(
            &paths,
            &biometric_config,
            &UnlockAppSessionRequest { passcode: None, use_biometric: true },
            AppLockBiometricState::TouchIdUnavailable,
            || unreachable!("unavailable biometric must not authenticate"),
        )
        .expect_err("unavailable biometric");
        assert!(unavailable_unlock.to_string().contains("Touch ID is unavailable"));

        let default_unlock = unlock_app_session(
            &paths,
            &biometric_config,
            &UnlockAppSessionRequest { passcode: None, use_biometric: true },
        )
        .expect_err("default biometric unavailable");
        assert!(default_unlock.to_string().contains("not available"));
    }
}
