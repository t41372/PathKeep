//! Worker-bridge helpers for whole-app Export / Import.
//!
//! Adapts the `vault_worker::migration` surface to the string-error
//! transport contract used by Tauri commands. No business logic here —
//! every routine is a thin wrapper that surfaces the underlying
//! `anyhow::Error` chain so the Settings panel can show actionable copy
//! ("bundle was produced by a newer PathKeep build…" etc.).

use super::worker_result;
use std::path::PathBuf;

#[cfg_attr(test, allow(dead_code))]
pub(crate) fn export_app_data_impl(
    session_database_key: Option<&str>,
    target_path: PathBuf,
) -> Result<vault_core::ExportedBundle, String> {
    worker_result(vault_worker::export_app_data(session_database_key, target_path))
}

#[cfg_attr(test, allow(dead_code))]
pub(crate) fn preview_app_data_import_impl(
    bundle_path: PathBuf,
) -> Result<vault_core::ImportPreview, String> {
    worker_result(vault_worker::preview_import(bundle_path))
}

#[cfg_attr(test, allow(dead_code))]
pub(crate) fn apply_app_data_import_impl(
    session_database_key: Option<&str>,
    bundle_path: PathBuf,
    options: vault_core::ApplyImportOptions,
) -> Result<vault_core::ImportResult, String> {
    worker_result(vault_worker::apply_import(session_database_key, bundle_path, options))
}
