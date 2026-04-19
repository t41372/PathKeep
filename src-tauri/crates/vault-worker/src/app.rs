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
    AppConfig, AppSnapshot, ArchiveMode, RuntimeDiagnostics, archive_status,
    ensure_archive_initialized, hydrate_app_lock_config, load_import_batches, load_recent_runs,
    rekey_archive, save_config, validate_app_lock_config_with_biometric,
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
        Err(error) => {
            log::warn!(
                target: "pathkeep::app_snapshot",
                "runtime diagnostics fallback during shell bootstrap: {error:#}"
            );
            RuntimeDiagnostics {
                log_directory: paths.logs_dir.display().to_string(),
                rust_log_path: paths.rust_log_path.display().to_string(),
                frontend_log_path: paths.frontend_log_path.display().to_string(),
                crash_reports_directory: paths.crash_reports_dir.display().to_string(),
                latest_crash_report: None,
            }
        }
    }
}

fn snapshot_browser_profiles() -> Vec<vault_core::BrowserProfile> {
    match discover_browser_profiles() {
        Ok(profiles) => profiles,
        Err(error) => {
            log::warn!(
                target: "pathkeep::app_snapshot",
                "browser discovery fallback during shell bootstrap: {error:#}"
            );
            Vec::new()
        }
    }
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
    let mut next_config = config.clone();
    hydrate_derived_config_state(&mut next_config);
    hydrate_app_lock_config(&paths, &mut next_config)?;
    validate_app_lock_config_with_biometric(
        &paths,
        &next_config,
        current_app_lock_biometric_state(),
    )?;
    save_config(&paths, &next_config)?;
    if let Err(error) = vault_core::reconcile_ai_queue_controls(
        &paths,
        &previous_config,
        &next_config,
        session_database_key,
    ) {
        save_config(&paths, &previous_config).with_context(
            || "restoring the previous config after AI queue control reconciliation failed",
        )?;
        return Err(error.context("syncing AI queue controls with the updated Settings"));
    }
    app_snapshot(session_database_key)
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
