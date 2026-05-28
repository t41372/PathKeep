//! Tauri commands for keyring and security-status surfaces.

#[cfg(not(test))]
use crate::{session::SessionState, worker_bridge};
#[cfg(not(test))]
use tauri::State;

#[cfg(not(test))]
#[tauri::command]
/// Returns whether local secrets are present in native keyrings.
pub(crate) fn keyring_status() -> vault_core::KeyringStatusReport {
    worker_bridge::keyring_status_impl()
}

#[cfg(not(test))]
#[tauri::command]
/// Loads the combined security read model for the current session.
pub(crate) fn security_status(
    state: State<'_, SessionState>,
) -> Result<vault_core::SecurityStatus, String> {
    worker_bridge::security_status_impl(state.get_key().as_deref())
}

#[cfg(not(test))]
#[tauri::command]
/// Reads the archive database key from the native keyring.
pub(crate) fn keyring_get_database_key() -> Result<Option<String>, String> {
    worker_bridge::keyring_get_database_key_impl()
}

#[cfg(not(test))]
#[tauri::command]
/// Stores the archive database key in the native keyring.
pub(crate) fn keyring_store_database_key(
    value: String,
) -> Result<vault_core::KeyringStatusReport, String> {
    worker_bridge::keyring_store_database_key_impl(value)
}

#[cfg(not(test))]
#[tauri::command]
/// Removes the archive database key from the native keyring.
pub(crate) fn keyring_clear_database_key() -> Result<vault_core::KeyringStatusReport, String> {
    worker_bridge::keyring_clear_database_key_impl()
}

#[cfg(not(test))]
#[tauri::command]
/// Clears PathKeep's local secret vault after explicit user confirmation.
pub(crate) fn reset_local_secret_vault() -> Result<(), String> {
    worker_bridge::reset_local_secret_vault_impl()
}
