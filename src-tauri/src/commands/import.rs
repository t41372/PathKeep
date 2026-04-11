use crate::{session::SessionState, worker_bridge};
use tauri::State;

#[cfg(not(test))]
#[tauri::command]
pub(crate) fn inspect_takeout(
    request: vault_core::TakeoutRequest,
) -> Result<vault_core::TakeoutInspection, String> {
    worker_bridge::inspect_takeout_impl(request)
}

#[cfg(not(test))]
#[tauri::command]
pub(crate) fn import_takeout(
    request: vault_core::TakeoutRequest,
    state: State<'_, SessionState>,
) -> Result<vault_core::TakeoutInspection, String> {
    worker_bridge::import_takeout_impl(request, state.get_key().as_deref())
}

#[cfg(not(test))]
#[tauri::command]
pub(crate) fn preview_import_batch(
    batch_id: i64,
    state: State<'_, SessionState>,
) -> Result<vault_core::ImportBatchDetail, String> {
    worker_bridge::preview_import_batch_impl(batch_id, state.get_key().as_deref())
}

#[cfg(not(test))]
#[tauri::command]
pub(crate) fn revert_import_batch(
    batch_id: i64,
    state: State<'_, SessionState>,
) -> Result<vault_core::ImportBatchDetail, String> {
    worker_bridge::revert_import_batch_impl(batch_id, state.get_key().as_deref())
}

#[cfg(not(test))]
#[tauri::command]
pub(crate) fn restore_import_batch(
    batch_id: i64,
    state: State<'_, SessionState>,
) -> Result<vault_core::ImportBatchDetail, String> {
    worker_bridge::restore_import_batch_impl(batch_id, state.get_key().as_deref())
}
