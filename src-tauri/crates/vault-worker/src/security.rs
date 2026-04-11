//! Security, App Lock, and keyring worker flows.
//!
//! This module owns the session-boundary and native-secret parts of the worker
//! contract:
//!
//! - App Lock setup/lock/unlock read models
//! - security status and last rekey review visibility
//! - rekey preview
//! - native keyring and stored-secret helpers
//!
//! The accepted product rule here is important: App Lock is a UI session
//! boundary, not archive encryption. The helpers below therefore preserve both
//! stories separately instead of blending them into one “locked” concept.

use crate::{
    app::{RekeyRequest, app_snapshot},
    context::{
        current_app_lock_biometric_state, load_hydrated_config, load_unlocked_config,
        resolved_app_lock_status,
    },
};
use anyhow::{Context, Result};
use rusqlite::OptionalExtension;
use std::{
    fs,
    path::{Path, PathBuf},
};
use vault_core::{
    AiProviderSecretInput, AppLockStatus, ArchiveMode, KeyringStatusReport, S3CredentialInput,
    archive, archive_status, clear_app_lock_passcode, lock_app_session, set_app_lock_passcode,
    unlock_app_session_with_biometric,
};
use vault_platform::{
    authenticate_app_lock_biometric, keyring_clear_database_key, keyring_clear_provider_api_key,
    keyring_clear_s3_credentials, keyring_get_database_key, keyring_set_database_key,
    keyring_set_provider_api_key, keyring_set_s3_credentials, keyring_status,
};

type RekeyReviewSummary = (Option<String>, Option<i64>, Option<String>);

/// Loads the security read model for the current archive/session state.
pub fn security_status(session_database_key: Option<&str>) -> Result<vault_core::SecurityStatus> {
    let paths = vault_core::project_paths()?;
    let config = load_unlocked_config(&paths)?;
    let archive = archive_status(&paths, &config, session_database_key)?;
    let keyring = keyring_status();
    let mut warnings = Vec::new();
    if let Some(warning) = archive.warning.clone() {
        warnings.push(warning);
    }
    if matches!(config.archive_mode, ArchiveMode::Encrypted)
        && config.remember_database_key_in_keyring
        && !keyring.available
    {
        warnings.push(
            "Archive is configured to remember the database key, but no native keyring backend is available on this machine.".to_string(),
        );
    }
    if matches!(config.archive_mode, ArchiveMode::Encrypted)
        && config.remember_database_key_in_keyring
        && !keyring.stored_secret
    {
        warnings.push(
            "Archive is encrypted, but the database key is not currently stored in the system keyring.".to_string(),
        );
    }

    let mode = if !archive.initialized {
        "uninitialized"
    } else if !archive.encrypted {
        "plaintext"
    } else if archive.unlocked {
        "encrypted"
    } else {
        "locked"
    };

    let (last_rekey_at, last_rekey_run_id, last_rekey_snapshot_path) = if archive.initialized
        && archive.unlocked
    {
        let connection = archive::open_archive_connection(&paths, &config, session_database_key)?;
        archive::create_schema(&connection)?;
        latest_rekey_review_from_archive(&connection)?
            .or_else(|| latest_rekey_review_from_manifests(&paths).ok().flatten())
            .unwrap_or((None, None, None))
    } else {
        latest_rekey_review_from_manifests(&paths)?.unwrap_or((None, None, None))
    };

    Ok(vault_core::SecurityStatus {
        initialized: archive.initialized,
        mode: mode.to_string(),
        encrypted: archive.encrypted,
        unlocked: archive.unlocked,
        database_path: archive.database_path,
        stronghold_path: paths.stronghold_path.display().to_string(),
        remember_database_key_in_keyring: config.remember_database_key_in_keyring,
        last_successful_backup_at: archive.last_successful_backup_at,
        last_rekey_at,
        last_rekey_run_id,
        last_rekey_snapshot_path,
        keyring_status: keyring,
        warnings,
    })
}

/// Reads the most recent rekey review directly from the archive when available.
fn latest_rekey_review_from_archive(
    connection: &rusqlite::Connection,
) -> Result<Option<RekeyReviewSummary>> {
    connection
        .query_row(
            "SELECT
                   runs.id,
                   runs.finished_at,
                   (
                     SELECT snapshots.file_path
                     FROM snapshots
                     WHERE snapshots.run_id = runs.id
                       AND snapshots.reason = 'before-rekey'
                     ORDER BY snapshots.id DESC
                     LIMIT 1
                   )
                 FROM runs
                 WHERE runs.run_type = 'rekey'
                 ORDER BY runs.id DESC
                 LIMIT 1",
            [],
            |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, Option<String>>(2)?,
                ))
            },
        )
        .optional()
        .map(|value| {
            value.map(|(run_id, finished_at, snapshot_path)| {
                (finished_at, Some(run_id), snapshot_path)
            })
        })
        .map_err(Into::into)
}

/// Falls back to manifest artifacts when the archive itself cannot be opened.
fn latest_rekey_review_from_manifests(
    paths: &vault_core::ProjectPaths,
) -> Result<Option<RekeyReviewSummary>> {
    let manifest_paths = collect_manifest_paths(&paths.manifests_dir)?;
    let mut latest: Option<(String, i64, Option<String>)> = None;

    for manifest_path in manifest_paths {
        let content = match fs::read_to_string(&manifest_path) {
            Ok(content) => content,
            Err(_) => continue,
        };
        let payload = match serde_json::from_str::<serde_json::Value>(&content) {
            Ok(payload) => payload,
            Err(_) => continue,
        };
        if payload.get("runType").and_then(serde_json::Value::as_str) != Some("rekey") {
            continue;
        }
        let Some(run_id) = payload.get("runId").and_then(serde_json::Value::as_i64) else {
            continue;
        };
        let created_at = payload
            .get("createdAt")
            .and_then(serde_json::Value::as_str)
            .map(ToString::to_string)
            .unwrap_or_default();
        let snapshot_path = payload
            .get("snapshotPath")
            .and_then(serde_json::Value::as_str)
            .map(ToString::to_string);

        let replace = latest.as_ref().is_none_or(|(current_created_at, current_run_id, _)| {
            created_at > *current_created_at
                || (created_at == *current_created_at && run_id > *current_run_id)
        });
        if replace {
            latest = Some((created_at, run_id, snapshot_path));
        }
    }

    Ok(latest.map(|(created_at, run_id, snapshot_path)| {
        ((!created_at.is_empty()).then_some(created_at), Some(run_id), snapshot_path)
    }))
}

/// Recursively collects manifest JSON files from the audit tree.
fn collect_manifest_paths(root: &Path) -> Result<Vec<PathBuf>> {
    if !root.exists() {
        return Ok(Vec::new());
    }

    let mut files = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(path) = stack.pop() {
        for entry in fs::read_dir(&path)
            .with_context(|| format!("reading manifest directory {}", path.display()))?
        {
            let entry = entry?;
            let entry_path = entry.path();
            if entry.file_type()?.is_dir() {
                stack.push(entry_path);
                continue;
            }
            if entry_path.extension().and_then(|value| value.to_str()) == Some("json") {
                files.push(entry_path);
            }
        }
    }
    Ok(files)
}

/// Loads the App Lock status without requiring the archive to be unlocked.
pub fn load_app_lock_status() -> Result<AppLockStatus> {
    let paths = vault_core::project_paths()?;
    let config = load_hydrated_config(&paths)?;
    resolved_app_lock_status(&paths, &config)
}

/// Configures an App Lock passcode and returns the updated lock status.
pub fn configure_app_lock_passcode(
    request: &vault_core::SetAppLockPasscodeRequest,
) -> Result<AppLockStatus> {
    let paths = vault_core::project_paths()?;
    let mut config = load_hydrated_config(&paths)?;
    set_app_lock_passcode(&paths, &mut config, request)?;
    resolved_app_lock_status(&paths, &config)
}

/// Clears the App Lock passcode and returns the updated lock status.
pub fn remove_app_lock_passcode() -> Result<AppLockStatus> {
    let paths = vault_core::project_paths()?;
    let mut config = load_hydrated_config(&paths)?;
    clear_app_lock_passcode(&paths, &mut config)?;
    resolved_app_lock_status(&paths, &config)
}

/// Locks the current UI session.
pub fn lock_app_ui_session(reason: Option<&str>) -> Result<AppLockStatus> {
    let paths = vault_core::project_paths()?;
    let config = load_hydrated_config(&paths)?;
    lock_app_session(&paths, &config, reason)?;
    resolved_app_lock_status(&paths, &config)
}

/// Unlocks the current UI session, optionally using biometrics.
pub fn unlock_app_ui_session(
    request: &vault_core::UnlockAppSessionRequest,
) -> Result<AppLockStatus> {
    let paths = vault_core::project_paths()?;
    let config = load_hydrated_config(&paths)?;
    unlock_app_session_with_biometric(
        &paths,
        &config,
        request,
        current_app_lock_biometric_state(),
        || authenticate_app_lock_biometric().map_err(anyhow::Error::msg),
    )
}

/// Previews the archive rewrite that a rekey/mode switch would perform.
pub fn preview_rekey_archive(
    session_database_key: Option<&str>,
    request: &RekeyRequest,
) -> Result<vault_core::RekeyPreview> {
    let paths = vault_core::project_paths()?;
    let config = load_unlocked_config(&paths)?;
    let archive = archive_status(&paths, &config, session_database_key)?;
    if !archive.initialized || !paths.archive_database_path.exists() {
        anyhow::bail!("initialize the archive before previewing a rekey operation");
    }

    let mut warnings = Vec::new();
    if archive.encrypted && !archive.unlocked {
        warnings.push(
            "The archive is currently locked. Unlock it before executing the rekey.".to_string(),
        );
    }
    if matches!(request.new_mode, ArchiveMode::Encrypted) && request.new_key.is_none() {
        warnings.push(
            "Encrypted rekey requires a new database key before execute can run.".to_string(),
        );
    }
    if config.archive_mode == request.new_mode {
        warnings.push(
            "The archive will still be rewritten because the target mode matches the current mode, which makes this a key rotation or validation pass rather than a mode switch.".to_string(),
        );
    }

    let snapshot_path =
        paths.raw_snapshots_dir.join("rekey").join("archive-before-rekey-<timestamp>.sqlite");
    let temp_path = paths.archive_database_path.with_extension("rekey.sqlite");

    Ok(vault_core::RekeyPreview {
        current_mode: config.archive_mode,
        next_mode: request.new_mode.clone(),
        requires_new_key: matches!(request.new_mode, ArchiveMode::Encrypted),
        snapshot_path: snapshot_path.display().to_string(),
        temp_database_path: temp_path.display().to_string(),
        steps: vec![
            format!(
                "Create a safety snapshot of the current archive at {}.",
                snapshot_path.display()
            ),
            format!(
                "Export the archive into a temporary database at {} using the requested target mode.",
                temp_path.display()
            ),
            "Swap the rewritten database into place only after the export succeeds, and keep the safety snapshot for manual recovery.".to_string(),
        ],
        warnings,
    })
}

/// Reads the stored archive key from the native keyring, if present.
pub fn read_database_key_from_keyring() -> Result<Option<String>> {
    keyring_get_database_key()
}

/// Stores the archive key in the native keyring and returns the updated keyring status.
pub fn write_database_key_to_keyring(key: &str) -> Result<KeyringStatusReport> {
    keyring_set_database_key(key)?;
    Ok(keyring_status())
}

/// Clears the stored archive key from the native keyring.
pub fn clear_database_key_from_keyring() -> Result<KeyringStatusReport> {
    keyring_clear_database_key()?;
    Ok(keyring_status())
}

/// Removes the local Stronghold vault file used for wrapped secrets.
pub fn reset_local_secret_vault() -> Result<()> {
    let paths = vault_core::project_paths()?;
    remove_file_if_exists(&paths.stronghold_path)?;
    Ok(())
}

/// Deletes a file only when it exists.
fn remove_file_if_exists(path: &Path) -> Result<()> {
    if path.exists() {
        fs::remove_file(path).with_context(|| format!("removing {}", path.display()))?;
    }
    Ok(())
}

/// Returns the current native keyring status snapshot.
pub fn keyring_report() -> KeyringStatusReport {
    keyring_status()
}

/// Stores S3 credentials in the native keyring.
pub fn store_s3_credentials(credentials: &S3CredentialInput) -> Result<()> {
    keyring_set_s3_credentials(credentials)
}

/// Removes stored S3 credentials from the native keyring.
pub fn clear_s3_credentials() -> Result<()> {
    keyring_clear_s3_credentials()
}

/// Stores an AI provider secret and returns a refreshed app snapshot.
pub fn store_ai_provider_api_key(
    input: &AiProviderSecretInput,
    session_database_key: Option<&str>,
) -> Result<vault_core::AppSnapshot> {
    keyring_set_provider_api_key(&input.provider_id, &input.api_key)?;
    app_snapshot(session_database_key)
}

/// Clears an AI provider secret and returns a refreshed app snapshot.
pub fn clear_ai_provider_api_key(
    provider_id: &str,
    session_database_key: Option<&str>,
) -> Result<vault_core::AppSnapshot> {
    keyring_clear_provider_api_key(provider_id)?;
    app_snapshot(session_database_key)
}
