//! Responsibilities: expose security status, key presence and backend-only unlocking.
//! Not responsible for: key storage, envelope cryptography or archive queries.
//! Failure modes: key bytes cross IPC or blocking keychain/Store work stalls the UI.
//! Tests: dev_ipc_bridge::dispatch::tests.
//!
//! Every command here touches the OS keyring or the security read model (disk + keyring), all of
//! which is blocking I/O that can stall — on some platforms a keyring access even prompts the user.
//! So each command hops onto the blocking thread pool and never runs on the WebView thread.

#[cfg(not(test))]
use super::blocking::run_blocking_command;
#[cfg(not(test))]
use crate::command_error::CommandError;
#[cfg(not(test))]
use crate::{session::SessionState, worker_bridge};
#[cfg(not(test))]
use tauri::State;

#[cfg(not(test))]
#[tauri::command]
/// Returns whether local secrets are present in native keyrings, off the UI thread.
///
/// Propagates join failures instead of defaulting to an empty report — an
/// empty default would render the Security page as "no secrets stored" when
/// the truth is "the status query failed", which are very different claims.
pub(crate) async fn keyring_status() -> Result<vault_core::KeyringStatusReport, CommandError> {
    run_blocking_command("keyring_status", move || Ok(worker_bridge::keyring_status_impl())).await
}

#[cfg(not(test))]
#[tauri::command]
/// Reports whether a data key is stored; the value never enters the WebView.
pub(crate) async fn keyring_get_database_key() -> Result<bool, CommandError> {
    run_blocking_command("keyring_get_database_key", worker_bridge::keyring_get_database_key_impl)
        .await
}

#[cfg(not(test))]
#[tauri::command]
/// Uses the supplied archive password to remember the envelope data key.
pub(crate) async fn keyring_store_database_key(
    value: String,
) -> Result<vault_core::KeyringStatusReport, CommandError> {
    run_blocking_command("keyring_store_database_key", move || {
        worker_bridge::keyring_store_database_key_impl(value)
    })
    .await
}

#[cfg(not(test))]
#[tauri::command]
/// Removes the archive database key from the native keyring, off the UI thread.
pub(crate) async fn keyring_clear_database_key()
-> Result<vault_core::KeyringStatusReport, CommandError> {
    run_blocking_command(
        "keyring_clear_database_key",
        worker_bridge::keyring_clear_database_key_impl,
    )
    .await
}

#[cfg(not(test))]
#[tauri::command]
/// Clears PathKeep's local secret vault after explicit user confirmation, off the UI thread.
pub(crate) async fn reset_local_secret_vault() -> Result<(), CommandError> {
    run_blocking_command("reset_local_secret_vault", worker_bridge::reset_local_secret_vault_impl)
        .await
}

#[cfg(not(test))]
#[tauri::command]
pub(crate) async fn unlock_archive_from_keychain(
    state: State<'_, SessionState>,
) -> Result<(), CommandError> {
    let state = state.inner().clone();
    run_blocking_command("unlock_archive_from_keychain", move || {
        worker_bridge::unlock_archive_from_keychain_impl(&state)
    })
    .await
}
