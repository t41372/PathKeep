//! Worker-bridge helpers for keyring and security surfaces.

use super::worker_result;

/// Returns whether PathKeep-managed secrets are present in native storage.
pub(crate) fn keyring_status_impl() -> vault_core::KeyringStatusReport {
    vault_worker::keyring_report()
}

/// Loads the combined security read model for the current archive/session.
pub(crate) fn security_status_impl(
    session_database_key: Option<&str>,
) -> Result<vault_core::SecurityStatus, String> {
    worker_result(vault_worker::security_status(session_database_key))
}

/// Reads the archive database key from native keyring storage.
pub(crate) fn keyring_get_database_key_impl() -> Result<Option<String>, String> {
    worker_result(vault_worker::read_database_key_from_keyring())
}

/// Stores the archive database key in native keyring storage.
pub(crate) fn keyring_store_database_key_impl(
    value: String,
) -> Result<vault_core::KeyringStatusReport, String> {
    worker_result(vault_worker::write_database_key_to_keyring(&value))
}

/// Removes the archive database key from native keyring storage.
pub(crate) fn keyring_clear_database_key_impl() -> Result<vault_core::KeyringStatusReport, String> {
    worker_result(vault_worker::clear_database_key_from_keyring())
}

/// Resets PathKeep's local secret vault for recovery/testing flows.
pub(crate) fn reset_local_secret_vault_impl() -> Result<(), String> {
    worker_result(vault_worker::reset_local_secret_vault())
}
