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
pub(crate) fn cancel_maintenance_impl() -> Result<bool, CommandError> {
    worker_result(vault_worker::cancel_maintenance())
}

pub(crate) fn export_app_data_impl(
    state: &crate::session::SessionState,
    target_path: PathBuf,
    export_password: String,
) -> Result<vault_core::ExportedBundle, CommandError> {
    worker_result(vault_worker::export_app_data(&state.vault()?, target_path, &export_password))
}

pub(crate) fn preview_app_data_import_impl(
    bundle_path: PathBuf,
) -> Result<vault_core::ImportPreview, CommandError> {
    let paths = worker_result(vault_core::project_paths())?;
    worker_result(vault_worker::preview_import(&paths, bundle_path))
}

pub(crate) fn apply_app_data_import_impl(
    state: &crate::session::SessionState,
    bundle_path: PathBuf,
    options: vault_core::ApplyImportOptions,
) -> Result<vault_core::ImportResult, CommandError> {
    let paths = worker_result(vault_core::project_paths())?;
    let password = options.source_archive_key.clone();
    let current = state.current_vault();
    let (report, archive) = worker_result(vault_worker::apply_import(
        &paths,
        current.as_ref(),
        state.owner(),
        bundle_path,
        options,
    ))?;
    worker_result(state.install_archive(archive, password))?;
    Ok(report)
}
