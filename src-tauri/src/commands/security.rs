use crate::{session::SessionState, worker_bridge};
use tauri::State;

#[cfg(not(test))]
#[tauri::command]
pub(crate) fn keyring_status() -> vault_core::KeyringStatusReport {
    worker_bridge::keyring_status_impl()
}

#[cfg(not(test))]
#[tauri::command]
pub(crate) fn security_status(
    state: State<'_, SessionState>,
) -> Result<vault_core::SecurityStatus, String> {
    worker_bridge::security_status_impl(state.get_key().as_deref())
}

#[cfg(not(test))]
#[tauri::command]
pub(crate) fn keyring_get_database_key() -> Result<Option<String>, String> {
    worker_bridge::keyring_get_database_key_impl()
}

#[cfg(not(test))]
#[tauri::command]
pub(crate) fn keyring_store_database_key(
    value: String,
) -> Result<vault_core::KeyringStatusReport, String> {
    worker_bridge::keyring_store_database_key_impl(value)
}

#[cfg(not(test))]
#[tauri::command]
pub(crate) fn keyring_clear_database_key() -> Result<vault_core::KeyringStatusReport, String> {
    worker_bridge::keyring_clear_database_key_impl()
}

#[cfg(not(test))]
#[tauri::command]
pub(crate) fn store_s3_credentials(
    credentials: vault_core::S3CredentialInput,
) -> Result<(), String> {
    worker_bridge::store_s3_credentials_impl(credentials)
}

#[cfg(not(test))]
#[tauri::command]
pub(crate) fn clear_s3_credentials() -> Result<(), String> {
    worker_bridge::clear_s3_credentials_impl()
}

#[cfg(not(test))]
#[tauri::command]
pub(crate) fn reset_local_secret_vault() -> Result<(), String> {
    worker_bridge::reset_local_secret_vault_impl()
}
