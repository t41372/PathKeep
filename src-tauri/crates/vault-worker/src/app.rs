//! App/bootstrap worker flows.
//!
//! These entrypoints assemble the desktop snapshot that the Tauri layer and
//! CLI-facing worker commands consume. They coordinate config hydration,
//! browser discovery, archive status reads, and bootstrap-time mutations such
//! as archive initialization or archive-mode changes.
//!
//! This module deliberately keeps the public worker surface stable. It may
//! compose several `vault-core` and `vault-platform` calls, but it should not
//! invent new product semantics beyond what the accepted command/read-model
//! docs already describe.

use crate::context::{
    current_app_lock_biometric_state, derive_ai_status, derive_intelligence_status,
    hydrate_derived_config_state, load_hydrated_config, load_unlocked_config,
    resolved_app_lock_status,
};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use vault_core::{
    AppConfig, AppSnapshot, ArchiveMode, ReconcileReport, RuntimeDiagnostics, archive_status,
    ensure_app_lock_unlocked, ensure_archive_initialized, hydrate_app_lock_config,
    load_import_batches, load_recent_runs, rekey_archive, save_config,
    validate_app_lock_config_with_biometric,
};
use vault_platform::{discover_browser_profiles, keyring_status};

/// Rekey request payload used by desktop and worker command surfaces.
///
/// The worker keeps this as a simple transport struct: it describes the target
/// archive mode plus the optional new key material, while the actual
/// preview/execute semantics stay in `vault-core`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RekeyRequest {
    /// The archive mode to end up with after the rewrite finishes.
    pub new_mode: ArchiveMode,
    /// The new database key when the target mode is encrypted.
    pub new_key: Option<String>,
}

fn snapshot_runtime_diagnostics(paths: &vault_core::ProjectPaths) -> RuntimeDiagnostics {
    match vault_core::load_runtime_diagnostics(paths) {
        Ok(diagnostics) => diagnostics,
        Err(error) => runtime_diagnostics_fallback(paths, &error),
    }
}

fn runtime_diagnostics_fallback(
    paths: &vault_core::ProjectPaths,
    error: &anyhow::Error,
) -> RuntimeDiagnostics {
    log::warn!(target: "pathkeep::app_snapshot", "runtime diagnostics fallback during shell bootstrap: {error:#}");
    RuntimeDiagnostics {
        log_directory: paths.logs_dir.display().to_string(),
        rust_log_path: paths.rust_log_path.display().to_string(),
        frontend_log_path: paths.frontend_log_path.display().to_string(),
        crash_reports_directory: paths.crash_reports_dir.display().to_string(),
        latest_crash_report: None,
    }
}

fn snapshot_browser_profiles() -> Vec<vault_core::BrowserProfile> {
    match discover_browser_profiles() {
        Ok(profiles) => profiles,
        Err(error) => browser_profiles_fallback(&error),
    }
}

fn browser_profiles_fallback(error: &anyhow::Error) -> Vec<vault_core::BrowserProfile> {
    log::warn!(target: "pathkeep::app_snapshot", "browser discovery fallback during shell bootstrap: {error:#}");
    Vec::new()
}

/// Builds the canonical desktop snapshot for the current unlocked session.
pub fn app_snapshot(session_database_key: Option<&str>) -> Result<AppSnapshot> {
    let paths = vault_core::project_paths()?;
    let config = load_unlocked_config(&paths)?;
    let browser_profiles = snapshot_browser_profiles();
    let archive_status = archive_status(&paths, &config, session_database_key)?;
    let ai_status = derive_ai_status(&paths, &config, session_database_key);
    let intelligence_status = derive_intelligence_status(&paths, &config, session_database_key);
    let can_read_archive_ledger = archive_status.initialized && archive_status.unlocked;
    let recent_runs = if can_read_archive_ledger {
        load_recent_runs(&paths, &config, session_database_key).unwrap_or_default()
    } else {
        Vec::new()
    };
    let recent_import_batches = if can_read_archive_ledger {
        load_import_batches(&paths, &config, session_database_key).unwrap_or_default()
    } else {
        Vec::new()
    };
    let app_lock_status = resolved_app_lock_status(&paths, &config)?;
    let runtime_diagnostics = snapshot_runtime_diagnostics(&paths);

    Ok(AppSnapshot {
        directories: vault_core::AppDirectories {
            app_root: paths.app_root.display().to_string(),
            config_path: paths.config_path.display().to_string(),
            archive_database_path: paths.archive_database_path.display().to_string(),
            search_database_path: paths.search_database_path.display().to_string(),
            intelligence_database_path: paths.intelligence_database_path.display().to_string(),
            audit_repo_path: paths.audit_repo_path.display().to_string(),
            manifests_dir: paths.manifests_dir.display().to_string(),
            exports_dir: paths.exports_dir.display().to_string(),
            raw_snapshots_dir: paths.raw_snapshots_dir.display().to_string(),
            staging_dir: paths.staging_dir.display().to_string(),
            quarantine_dir: paths.quarantine_dir.display().to_string(),
            schedule_dir: paths.schedule_dir.display().to_string(),
            semantic_index_dir: paths.semantic_index_dir.display().to_string(),
            intelligence_blobs_dir: paths.intelligence_blobs_dir.display().to_string(),
            logs_dir: paths.logs_dir.display().to_string(),
            rust_log_path: paths.rust_log_path.display().to_string(),
            frontend_log_path: paths.frontend_log_path.display().to_string(),
            crash_reports_dir: paths.crash_reports_dir.display().to_string(),
            stronghold_path: paths.stronghold_path.display().to_string(),
            stronghold_salt_path: paths.stronghold_salt_path.display().to_string(),
        },
        runtime_diagnostics,
        config,
        archive_status,
        app_lock_status,
        keyring_status: keyring_status(),
        ai_status,
        intelligence_status,
        browser_profiles,
        recent_runs,
        recent_import_batches,
    })
}

/// Saves config, reconciles runtime-derived controls, and returns a fresh snapshot.
pub fn save_user_config(
    config: &AppConfig,
    session_database_key: Option<&str>,
) -> Result<AppSnapshot> {
    let paths = vault_core::project_paths()?;
    let previous_config = load_hydrated_config(&paths).unwrap_or_default();
    // Settings (including App Lock's own enabled/biometric fields) must not be
    // mutated while the session is locked: otherwise the lock could be disabled
    // out from under itself without the passcode. Enabling the lock from an
    // unlocked session, and all initial setup, still pass this no-op check.
    ensure_app_lock_unlocked(&paths, &previous_config)?;
    let mut next_config = config.clone();
    hydrate_derived_config_state(&mut next_config);
    hydrate_app_lock_config(&paths, &mut next_config)?;
    validate_app_lock_config_with_biometric(
        &paths,
        &next_config,
        current_app_lock_biometric_state(),
    )?;
    save_config(&paths, &next_config)?;
    reconcile_ai_queue_controls_or_restore_config(
        &paths,
        &previous_config,
        &next_config,
        session_database_key,
    )?;
    app_snapshot(session_database_key)
}

fn reconcile_ai_queue_controls_or_restore_config(
    paths: &vault_core::ProjectPaths,
    previous_config: &AppConfig,
    next_config: &AppConfig,
    session_database_key: Option<&str>,
) -> Result<()> {
    vault_core::reconcile_ai_queue_controls(
        paths,
        previous_config,
        next_config,
        session_database_key,
    )
    .or_else(|error| restore_config_after_ai_reconcile_failure(paths, previous_config, error))
}

fn restore_config_after_ai_reconcile_failure(
    paths: &vault_core::ProjectPaths,
    previous_config: &AppConfig,
    error: anyhow::Error,
) -> Result<()> {
    save_config(paths, previous_config).with_context(
        || "restoring the previous config after AI queue control reconciliation failed",
    )?;
    Err(error.context("syncing AI queue controls with the updated Settings"))
}

/// Initializes the archive with the provided config and returns the hydrated snapshot.
pub fn initialize_archive_database(
    config: &AppConfig,
    database_key: Option<&str>,
) -> Result<AppSnapshot> {
    let paths = vault_core::project_paths()?;
    let mut next_config = config.clone();
    hydrate_derived_config_state(&mut next_config);
    hydrate_app_lock_config(&paths, &mut next_config)?;
    validate_app_lock_config_with_biometric(
        &paths,
        &next_config,
        current_app_lock_biometric_state(),
    )?;
    save_config(&paths, &next_config)?;
    ensure_archive_initialized(&paths, &next_config, database_key)?;
    app_snapshot(database_key)
}

/// Executes a rekey/mode-switch request and returns the post-rewrite snapshot.
pub fn rekey_archive_database(
    old_key: Option<&str>,
    request: &RekeyRequest,
) -> Result<AppSnapshot> {
    let paths = vault_core::project_paths()?;
    let config = load_unlocked_config(&paths)?;
    rekey_archive(&paths, &config, old_key, request.new_mode.clone(), request.new_key.as_deref())?;
    let mut next_config = config;
    next_config.archive_mode = request.new_mode.clone();
    next_config.initialized = true;
    save_config(&paths, &next_config)?;
    app_snapshot(request.new_key.as_deref().or(old_key))
}

/// Self-heals a drifted encryption-at-rest state proactively, right after the
/// archive is unlocked, so the user does not have to wait for (and watch fail)
/// their next backup. Converges `source-evidence` to the configured archive mode
/// and reports whether anything was repaired. A cheap no-op when consistent.
pub fn reconcile_archive_encryption(session_database_key: Option<&str>) -> Result<ReconcileReport> {
    let paths = vault_core::project_paths()?;
    let config = load_unlocked_config(&paths)?;
    vault_core::reconcile_archive_encryption(&paths, &config, session_database_key)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{ffi::OsStr, fs};
    use tempfile::tempdir;
    use vault_core::config::project_paths_with_root;

    const CHROME_USER_DATA_OVERRIDE_ENV: &str = "CHB_CHROME_USER_DATA_DIR";

    fn restore_env_var(name: &str, value: Option<&OsStr>) {
        unsafe {
            if let Some(value) = value {
                std::env::set_var(name, value);
            } else {
                std::env::remove_var(name);
            }
        }
    }

    #[test]
    fn snapshot_helpers_fallback_to_truthful_empty_shell_values() {
        let _guard = crate::tests::lock_env();
        let original_chrome_root = std::env::var_os(CHROME_USER_DATA_OVERRIDE_ENV);
        let root = tempdir().expect("tempdir");
        let invalid_root = root.path().join("not-a-directory");
        fs::write(&invalid_root, "not a directory").expect("write invalid root");

        let diagnostics = snapshot_runtime_diagnostics(&project_paths_with_root(&invalid_root));
        assert_eq!(diagnostics.log_directory, invalid_root.join("logs").display().to_string());
        assert!(diagnostics.latest_crash_report.is_none());
        let diagnostics = runtime_diagnostics_fallback(
            &project_paths_with_root(root.path()),
            &anyhow::anyhow!("diagnostics failed"),
        );
        assert_eq!(
            diagnostics.rust_log_path,
            root.path().join("logs/rust.log").display().to_string()
        );

        unsafe {
            std::env::set_var(CHROME_USER_DATA_OVERRIDE_ENV, &invalid_root);
        }
        assert!(snapshot_browser_profiles().is_empty());
        assert!(browser_profiles_fallback(&anyhow::anyhow!("discovery failed")).is_empty());

        let restore_root = tempdir().expect("restore tempdir");
        let restore_paths = project_paths_with_root(restore_root.path());
        let previous_config = AppConfig { initialized: true, ..AppConfig::default() };
        let error = restore_config_after_ai_reconcile_failure(
            &restore_paths,
            &previous_config,
            anyhow::anyhow!("queue sync failed"),
        )
        .expect_err("restore helper returns sync failure");
        assert!(error.to_string().contains("syncing AI queue controls"));
        let restored_config = vault_core::load_config(&restore_paths).expect("restored config");
        assert!(restored_config.initialized);

        restore_env_var(CHROME_USER_DATA_OVERRIDE_ENV, original_chrome_root.as_deref());
    }
}
