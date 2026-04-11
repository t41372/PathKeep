//! Tauri commands for remote backup preview/upload/verification.

#[cfg(not(test))]
use crate::{session::SessionState, worker_bridge};
#[cfg(not(test))]
use tauri::State;

#[cfg(not(test))]
#[tauri::command]
/// Previews the remote-backup bundle and upload plan.
pub(crate) fn preview_remote_backup() -> Result<vault_core::RemoteBackupPreview, String> {
    worker_bridge::preview_remote_backup_impl()
}

#[cfg(not(test))]
#[tauri::command]
/// Uploads the latest local backup bundle to the configured remote store.
pub(crate) fn run_remote_backup(
    state: State<'_, SessionState>,
) -> Result<vault_core::RemoteBackupResult, String> {
    worker_bridge::run_remote_backup_impl(state.get_key().as_deref())
}

#[cfg(not(test))]
#[tauri::command]
/// Verifies a previously created remote-backup bundle from disk.
pub(crate) fn verify_remote_backup(
    bundle_path: String,
    state: State<'_, SessionState>,
) -> Result<vault_core::RemoteBackupVerification, String> {
    worker_bridge::verify_remote_backup_impl(bundle_path, state.get_key().as_deref())
}
