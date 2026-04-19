//! Tauri commands for Takeout inspection/import and import-batch review.

#[cfg(not(test))]
use crate::{session::SessionState, worker_bridge};
#[cfg(not(test))]
use tauri::{AppHandle, Emitter, State};

#[cfg(not(test))]
#[tauri::command]
/// Inspects a Takeout source without importing it.
pub(crate) fn inspect_takeout(
    request: vault_core::TakeoutRequest,
) -> Result<vault_core::TakeoutInspection, String> {
    worker_bridge::inspect_takeout_impl(request)
}

#[cfg(not(test))]
#[tauri::command]
/// Imports a Takeout source into the current archive.
pub(crate) fn import_takeout(
    app: AppHandle,
    request: vault_core::TakeoutRequest,
    state: State<'_, SessionState>,
) -> Result<vault_core::TakeoutInspection, String> {
    worker_bridge::import_takeout_impl(request, state.get_key().as_deref(), |event| {
        let _ = app.emit("pathkeep://import-progress", &event);
    })
}

#[cfg(not(test))]
#[tauri::command]
/// Loads the detailed preview for one previously recorded import batch.
pub(crate) fn preview_import_batch(
    batch_id: i64,
    state: State<'_, SessionState>,
) -> Result<vault_core::ImportBatchDetail, String> {
    worker_bridge::preview_import_batch_impl(batch_id, state.get_key().as_deref())
}

#[cfg(not(test))]
#[tauri::command]
/// Hides one import batch from the visible archive surface.
pub(crate) fn revert_import_batch(
    batch_id: i64,
    state: State<'_, SessionState>,
) -> Result<vault_core::ImportBatchDetail, String> {
    worker_bridge::revert_import_batch_impl(batch_id, state.get_key().as_deref())
}

#[cfg(not(test))]
#[tauri::command]
/// Restores a previously reverted import batch to the visible archive surface.
pub(crate) fn restore_import_batch(
    batch_id: i64,
    state: State<'_, SessionState>,
) -> Result<vault_core::ImportBatchDetail, String> {
    worker_bridge::restore_import_batch_impl(batch_id, state.get_key().as_deref())
}
