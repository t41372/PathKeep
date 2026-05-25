//! Tauri commands for whole-app Export / Import.
//!
//! Each command runs the actual zip pack / unpack off the UI thread via
//! `spawn_blocking` so the Settings panel stays responsive on big
//! archives (the user reported 14M-row archives in mind). The work
//! delegates to `vault_worker::migration`; the bridge handles paths +
//! config hydration + error stringification.

#[cfg(not(test))]
use super::blocking::run_blocking_command;
#[cfg(not(test))]
use crate::{session::SessionState, worker_bridge};
#[cfg(not(test))]
use std::path::PathBuf;
#[cfg(not(test))]
use tauri::State;

#[cfg(not(test))]
#[tauri::command]
/// Packs the entire local project into a `.pathkeep-bundle` zip.
pub(crate) async fn export_app_data(
    state: State<'_, SessionState>,
    target_path: String,
) -> Result<vault_core::ExportedBundle, String> {
    let key = state.get_key();
    let target = PathBuf::from(target_path);
    run_blocking_command("export_app_data", move || {
        worker_bridge::export_app_data_impl(key.as_deref(), target)
    })
    .await
}

#[cfg(not(test))]
#[tauri::command]
/// Reads a bundle's manifest, validates the anti-tamper sha256, and
/// returns the preview Settings shows before the user confirms the
/// destructive overwrite.
pub(crate) async fn preview_app_data_import(
    bundle_path: String,
) -> Result<vault_core::ImportPreview, String> {
    let bundle = PathBuf::from(bundle_path);
    run_blocking_command("preview_app_data_import", move || {
        worker_bridge::preview_app_data_import_impl(bundle)
    })
    .await
}

#[cfg(not(test))]
#[tauri::command]
/// Applies a previously-previewed bundle onto the live project tree.
/// Surfaces the manifest, applied migration list, and a flag indicating
/// whether the prior tree was preserved as `*.bak-<timestamp>` sibling
/// paths next to each restored subtree.
pub(crate) async fn apply_app_data_import(
    state: State<'_, SessionState>,
    bundle_path: String,
    options: vault_core::ApplyImportOptions,
) -> Result<vault_core::ImportResult, String> {
    let key = state.get_key();
    let bundle = PathBuf::from(bundle_path);
    run_blocking_command("apply_app_data_import", move || {
        worker_bridge::apply_app_data_import_impl(key.as_deref(), bundle, options)
    })
    .await
}
