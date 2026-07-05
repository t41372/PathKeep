//! Tauri commands for app bootstrap and session controls.

#[cfg(not(test))]
use super::blocking::run_blocking_command;
#[cfg(not(test))]
use crate::{session::SessionState, worker_bridge};
#[cfg(not(test))]
use tauri::State;

#[cfg(not(test))]
#[tauri::command]
/// Returns immutable build metadata shown in About/diagnostics surfaces.
///
/// Pure in-memory metadata (no I/O), so it stays a synchronous command.
pub(crate) fn app_build_info() -> vault_core::AppBuildInfo {
    worker_bridge::app_build_info_impl()
}

#[cfg(not(test))]
#[tauri::command]
/// Loads the desktop snapshot used to hydrate the shell after startup, off the UI thread.
///
/// Reads config + archive state from SQLite, so it runs on the blocking pool to avoid freezing the
/// WebView thread during startup hydration.
pub(crate) async fn app_snapshot(
    state: State<'_, SessionState>,
) -> Result<vault_core::AppSnapshot, String> {
    let key = state.get_key();
    run_blocking_command("app_snapshot", move || worker_bridge::app_snapshot_impl(key.as_deref()))
        .await
}

#[cfg(not(test))]
#[tauri::command]
/// Persists user config changes and returns the refreshed desktop snapshot, off the UI thread.
pub(crate) async fn save_config(
    config: vault_core::AppConfig,
    state: State<'_, SessionState>,
) -> Result<vault_core::AppSnapshot, String> {
    let key = state.get_key();
    run_blocking_command("save_config", move || {
        worker_bridge::save_config_impl(config, key.as_deref())
    })
    .await
}

#[cfg(not(test))]
#[tauri::command]
/// Stores a session-only archive key without touching native keyrings.
pub(crate) fn set_session_database_key(
    database_key: String,
    state: State<'_, SessionState>,
) -> Result<(), String> {
    worker_bridge::set_session_database_key_impl(database_key, &state)
}

#[cfg(not(test))]
#[tauri::command]
/// Clears the transient session database key cached by the desktop process.
pub(crate) fn clear_session_database_key(state: State<'_, SessionState>) -> Result<(), String> {
    worker_bridge::clear_session_database_key_impl(&state)
}

#[cfg(not(test))]
#[tauri::command]
/// Returns the current App Lock session status, off the UI thread.
///
/// Reads the lock-state file from disk, so it runs on the blocking pool.
pub(crate) async fn app_lock_status() -> Result<vault_core::AppLockStatus, String> {
    run_blocking_command("app_lock_status", worker_bridge::app_lock_status_impl).await
}

#[cfg(not(test))]
#[tauri::command]
/// Configures or rotates the App Lock passcode for future UI sessions, off the UI thread.
///
/// Runs the passcode KDF and writes the lock secret — both blocking work that must stay off the
/// WebView thread.
pub(crate) async fn set_app_lock_passcode(
    request: vault_core::SetAppLockPasscodeRequest,
) -> Result<vault_core::AppLockStatus, String> {
    run_blocking_command("set_app_lock_passcode", move || {
        worker_bridge::set_app_lock_passcode_impl(request)
    })
    .await
}

#[cfg(not(test))]
#[tauri::command]
/// Removes the App Lock passcode and returns the updated lock status, off the UI thread.
pub(crate) async fn clear_app_lock_passcode() -> Result<vault_core::AppLockStatus, String> {
    run_blocking_command("clear_app_lock_passcode", worker_bridge::clear_app_lock_passcode_impl)
        .await
}

#[cfg(not(test))]
#[tauri::command]
/// Locks the current UI session without changing archive encryption state, off the UI thread.
pub(crate) async fn lock_app_session(
    reason: Option<String>,
) -> Result<vault_core::AppLockStatus, String> {
    run_blocking_command("lock_app_session", move || worker_bridge::lock_app_session_impl(reason))
        .await
}

#[cfg(not(test))]
#[tauri::command]
/// Attempts to unlock the current UI session with the provided credentials, off the UI thread.
///
/// Verifies the passcode via the KDF (deliberately expensive), so it must run on the blocking pool.
pub(crate) async fn unlock_app_session(
    request: vault_core::UnlockAppSessionRequest,
) -> Result<vault_core::AppLockStatus, String> {
    run_blocking_command("unlock_app_session", move || {
        worker_bridge::unlock_app_session_impl(request)
    })
    .await
}
