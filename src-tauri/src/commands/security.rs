//! Tauri commands for keyring and security-status surfaces.
//!
//! Every command here touches the OS keyring or the security read model (disk + keyring), all of
//! which is blocking I/O that can stall — on some platforms a keyring access even prompts the user.
//! So each command hops onto the blocking thread pool and never runs on the WebView thread.

#[cfg(not(test))]
use super::blocking::run_blocking_command;
#[cfg(not(test))]
use crate::{session::SessionState, worker_bridge};
#[cfg(not(test))]
use tauri::State;

#[cfg(not(test))]
#[tauri::command]
/// Returns whether local secrets are present in native keyrings, off the UI thread.
pub(crate) async fn keyring_status() -> vault_core::KeyringStatusReport {
    run_blocking_command("keyring_status", move || Ok(worker_bridge::keyring_status_impl()))
        .await
        .unwrap_or_default()
}

#[cfg(not(test))]
#[tauri::command]
/// Loads the combined security read model for the current session, off the UI thread.
pub(crate) async fn security_status(
    state: State<'_, SessionState>,
) -> Result<vault_core::SecurityStatus, String> {
    let key = state.get_key();
    run_blocking_command("security_status", move || {
        worker_bridge::security_status_impl(key.as_deref())
    })
    .await
}

#[cfg(not(test))]
#[tauri::command]
/// Reads the archive database key from the native keyring, off the UI thread.
pub(crate) async fn keyring_get_database_key() -> Result<Option<String>, String> {
    run_blocking_command("keyring_get_database_key", worker_bridge::keyring_get_database_key_impl)
        .await
}

#[cfg(not(test))]
#[tauri::command]
/// Stores the archive database key in the native keyring, off the UI thread.
pub(crate) async fn keyring_store_database_key(
    value: String,
) -> Result<vault_core::KeyringStatusReport, String> {
    run_blocking_command("keyring_store_database_key", move || {
        worker_bridge::keyring_store_database_key_impl(value)
    })
    .await
}

#[cfg(not(test))]
#[tauri::command]
/// Removes the archive database key from the native keyring, off the UI thread.
pub(crate) async fn keyring_clear_database_key() -> Result<vault_core::KeyringStatusReport, String>
{
    run_blocking_command(
        "keyring_clear_database_key",
        worker_bridge::keyring_clear_database_key_impl,
    )
    .await
}

#[cfg(not(test))]
#[tauri::command]
/// Clears PathKeep's local secret vault after explicit user confirmation, off the UI thread.
pub(crate) async fn reset_local_secret_vault() -> Result<(), String> {
    run_blocking_command("reset_local_secret_vault", worker_bridge::reset_local_secret_vault_impl)
        .await
}
