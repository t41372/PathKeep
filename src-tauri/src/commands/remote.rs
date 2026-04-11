#[cfg(not(test))]
use crate::{session::SessionState, worker_bridge};
#[cfg(not(test))]
use tauri::State;

#[cfg(not(test))]
#[tauri::command]
pub(crate) fn preview_remote_backup() -> Result<vault_core::RemoteBackupPreview, String> {
    worker_bridge::preview_remote_backup_impl()
}

#[cfg(not(test))]
#[tauri::command]
pub(crate) fn run_remote_backup(
    state: State<'_, SessionState>,
) -> Result<vault_core::RemoteBackupResult, String> {
    worker_bridge::run_remote_backup_impl(state.get_key().as_deref())
}

#[cfg(not(test))]
#[tauri::command]
pub(crate) fn verify_remote_backup(
    bundle_path: String,
    state: State<'_, SessionState>,
) -> Result<vault_core::RemoteBackupVerification, String> {
    worker_bridge::verify_remote_backup_impl(bundle_path, state.get_key().as_deref())
}
