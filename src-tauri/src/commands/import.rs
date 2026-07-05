//! Tauri commands for Takeout, Browser Direct, and import-batch review.

#[cfg(not(test))]
use super::blocking::run_blocking_command;
#[cfg(not(test))]
use crate::{session::SessionState, worker_bridge};
#[cfg(not(test))]
use tauri::{AppHandle, Emitter, State};

#[cfg(not(test))]
#[tauri::command]
/// Inspects a Takeout source without importing it.
pub(crate) async fn inspect_takeout(
    request: vault_core::TakeoutRequest,
) -> Result<vault_core::TakeoutInspection, String> {
    run_blocking_command("inspect_takeout", move || worker_bridge::inspect_takeout_impl(request))
        .await
}

#[cfg(not(test))]
#[tauri::command]
/// Imports a Takeout source into the current archive.
pub(crate) async fn import_takeout(
    app: AppHandle,
    request: vault_core::TakeoutRequest,
    state: State<'_, SessionState>,
) -> Result<vault_core::TakeoutInspection, String> {
    let session_database_key = state.get_key();
    run_blocking_command("import_takeout", move || {
        worker_bridge::import_takeout_impl(request, session_database_key.as_deref(), |event| {
            let _ = app.emit("pathkeep://import-progress", &event);
        })
    })
    .await
}

#[cfg(not(test))]
#[tauri::command]
/// Inspects a local browser history database without importing it.
pub(crate) async fn inspect_browser_history(
    request: vault_core::BrowserHistoryImportRequest,
) -> Result<vault_core::TakeoutInspection, String> {
    run_blocking_command("inspect_browser_history", move || {
        worker_bridge::inspect_browser_history_impl(request)
    })
    .await
}

#[cfg(not(test))]
#[tauri::command]
/// Imports a local browser history database into the current archive.
pub(crate) async fn import_browser_history(
    app: AppHandle,
    request: vault_core::BrowserHistoryImportRequest,
    state: State<'_, SessionState>,
) -> Result<vault_core::TakeoutInspection, String> {
    let session_database_key = state.get_key();
    run_blocking_command("import_browser_history", move || {
        worker_bridge::import_browser_history_impl(
            request,
            session_database_key.as_deref(),
            |event| {
                let _ = app.emit("pathkeep://import-progress", &event);
            },
        )
    })
    .await
}

#[cfg(not(test))]
#[tauri::command]
/// Loads the detailed preview for one previously recorded import batch, off the UI thread.
pub(crate) async fn preview_import_batch(
    batch_id: i64,
    state: State<'_, SessionState>,
) -> Result<vault_core::ImportBatchDetail, String> {
    let key = state.get_key();
    run_blocking_command("preview_import_batch", move || {
        worker_bridge::preview_import_batch_impl(batch_id, key.as_deref())
    })
    .await
}

#[cfg(not(test))]
#[tauri::command]
/// Hides one import batch from the visible archive surface, off the UI thread.
pub(crate) async fn revert_import_batch(
    batch_id: i64,
    state: State<'_, SessionState>,
) -> Result<vault_core::ImportBatchDetail, String> {
    let key = state.get_key();
    run_blocking_command("revert_import_batch", move || {
        worker_bridge::revert_import_batch_impl(batch_id, key.as_deref())
    })
    .await
}

#[cfg(not(test))]
#[tauri::command]
/// Restores a previously reverted import batch to the visible archive surface, off the UI thread.
pub(crate) async fn restore_import_batch(
    batch_id: i64,
    state: State<'_, SessionState>,
) -> Result<vault_core::ImportBatchDetail, String> {
    let key = state.get_key();
    run_blocking_command("restore_import_batch", move || {
        worker_bridge::restore_import_batch_impl(batch_id, key.as_deref())
    })
    .await
}
