//! Responsibilities: adapt security status, key presence and backend-only unlocks.
//! Not responsible for: transporting data keys, storing session state or opening SQL.
//! Failure modes: secret material reaches a command response, passwords replace data
//! keys, or legacy readers receive a keychain key. Tests: dispatch session regressions.

use super::worker_result;
use crate::command_error::CommandError;

/// Returns whether PathKeep-managed secrets are present in native storage.
pub(crate) fn keyring_status_impl() -> vault_core::KeyringStatusReport {
    vault_worker::keyring_report()
}

/// Loads the combined security read model for the current archive/session.
pub(crate) fn security_status_impl(
    state: &crate::session::SessionState,
) -> Result<vault_core::SecurityStatus, CommandError> {
    let vault = state.current_vault();
    worker_result(vault_worker::security_status(vault.as_ref(), state.get_key()?.as_deref()))
}

/// Reports presence without returning key material.
pub(crate) fn keyring_get_database_key_impl() -> Result<bool, CommandError> {
    Ok(vault_worker::keyring_report().stored_secret)
}

/// Unwraps an existing envelope with the archive password and remembers its data key.
pub(crate) fn keyring_store_database_key_impl(
    value: String,
) -> Result<vault_core::KeyringStatusReport, CommandError> {
    worker_result(vault_worker::write_database_key_to_keyring(&value))
}

/// Removes the archive database key from native keyring storage.
pub(crate) fn keyring_clear_database_key_impl()
-> Result<vault_core::KeyringStatusReport, CommandError> {
    worker_result(vault_worker::clear_database_key_from_keyring())
}

/// Resets PathKeep's local secret vault for recovery/testing flows.
pub(crate) fn reset_local_secret_vault_impl() -> Result<(), CommandError> {
    worker_result(vault_worker::reset_local_secret_vault())
}

/// Opens the Vault from the native credential entry without exposing its data key.
pub(crate) fn unlock_archive_from_keychain_impl(
    state: &crate::session::SessionState,
) -> Result<(), CommandError> {
    Ok(state.unlock_from_keychain()?)
}
