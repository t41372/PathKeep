//! Responsibilities: adapt portable migration and publish the installed owner session.
//! Not responsible for: cryptography, ZIP validation or database copying.
//! Failure modes: stale session after import, password serialized under the wrong name,
//! progress/cancel blocked on SQL. Tests: worker_bridge migration round trip.
//!
//! Worker-bridge helpers for whole-app Export / Import.
//!
//! Adapts the `vault_worker::migration` surface to the string-error
//! transport contract used by Tauri commands. No business logic here —
//! every routine is a thin wrapper that surfaces the underlying
//! `anyhow::Error` chain so the Settings panel can show actionable copy
//! ("bundle was produced by a newer PathKeep build…" etc.).

use super::worker_result;
use crate::command_error::CommandError;
use std::path::PathBuf;

pub(crate) fn maintenance_progress_impl()
-> Result<Option<vault_core::maintenance::Progress>, CommandError> {
    worker_result(vault_worker::maintenance_progress())
}
pub(crate) fn cancel_maintenance_impl(
    session: &crate::session::SessionState,
) -> Result<bool, CommandError> {
    let active = worker_result(vault_worker::cancel_maintenance())?;
    Ok(session.current_vault().is_some_and(|v| v.cancel_conversation_repair_preview()) || active)
}

pub(crate) fn export_app_data_impl(
    state: &crate::session::SessionState,
    target_path: PathBuf,
    export_password: String,
) -> Result<vault_core::ExportedBundle, CommandError> {
    super::worker_result(vault_worker::export_app_data(
        &state.vault()?,
        target_path,
        &export_password,
    ))
}

pub(crate) fn preview_app_data_import_impl(
    bundle_path: PathBuf,
) -> Result<vault_worker::DeviceImportPreview, CommandError> {
    let paths = worker_result(vault_core::project_paths())?;
    worker_result(vault_worker::preview_import(&paths, bundle_path))
}

pub(crate) fn apply_app_data_import_impl(
    state: &crate::session::SessionState,
    bundle_path: PathBuf,
    options: vault_core::ApplyImportOptions,
    current_password: Option<String>,
) -> Result<vault_core::ImportResult, CommandError> {
    let paths = worker_result(vault_core::project_paths())?;
    let password = options.source_archive_key.clone();
    let current = if let Some(password) = current_password {
        let config = worker_result(vault_core::load_config(&paths))?;
        worker_result(vault_core::ensure_app_lock_unlocked(&paths, &config))?;
        Some(state.open_password(paths.clone(), config, password)?)
    } else {
        state.current_vault()
    };
    // Through installing the imported archive and its key, not only the import, so a quit
    // that waits for it doesn't leave the keychain behind.
    let _running = vault_worker::begin_foreground_work(vault_worker::ForegroundWork::MoveImport);
    // Nothing opens the archive while it is replaced, until the imported one is published.
    let lease = state.begin_maintenance()?;
    let (report, archive) = super::worker_result(vault_worker::apply_import(
        &paths,
        current.as_ref(),
        state.owner(),
        bundle_path,
        options,
    ))?;
    worker_result(state.install_archive(
        archive,
        password,
        vault_worker::KeychainAfterInstall::AsSaved,
    ))?;
    drop(lease);
    Ok(report)
}

pub(crate) fn conversation_imports_impl(
    state: &crate::session::SessionState,
) -> Result<Vec<serde_json::Value>, CommandError> {
    worker_result(vault_core::migration::conversations::imports(&state.vault()?))
}

pub(crate) fn undo_conversation_import_impl(
    state: &crate::session::SessionState,
    id: String,
) -> Result<serde_json::Value, CommandError> {
    worker_result(vault_core::migration::conversations::undo(&state.vault()?, id))
}

pub(crate) fn rollback_preview() -> Result<vault_core::maintenance::rollback::Preview, CommandError>
{
    let paths = worker_result(vault_core::project_paths())?;
    worker_result(vault_core::maintenance::rollback::preview(&paths))
}

pub(crate) fn rollback_prepare(
    session: &crate::session::SessionState,
    password: Option<String>,
) -> Result<PathBuf, CommandError> {
    let paths = worker_result(vault_core::project_paths())?;
    let _lease = session.begin_maintenance()?;
    let dir = worker_result(vault_core::maintenance::rollback::detach(
        &paths,
        session.current_vault().as_ref(),
        session.owner(),
        password.as_deref(),
    ))?;
    crate::desktop_integration::restrict_to_recovery();
    Ok(dir)
}

pub(crate) fn rollback_resolve(
    session: &crate::session::SessionState,
    rejoin: bool,
    password: Option<String>,
    package: Option<String>,
) -> Result<(), CommandError> {
    let paths = worker_result(vault_core::project_paths())?;
    let _lease = session.begin_maintenance()?;
    worker_result(vault_core::maintenance::rollback::resolve(
        &paths,
        session.owner(),
        rejoin,
        password.as_deref(),
        package.as_deref(),
    ))
}
