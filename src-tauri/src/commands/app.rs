#[cfg(not(test))]
use crate::{session::SessionState, worker_bridge};
#[cfg(not(test))]
use tauri::State;

#[cfg(not(test))]
#[tauri::command]
pub(crate) fn app_build_info() -> vault_core::AppBuildInfo {
    worker_bridge::app_build_info_impl()
}

#[cfg(not(test))]
#[tauri::command]
pub(crate) fn app_snapshot(
    state: State<'_, SessionState>,
) -> Result<vault_core::AppSnapshot, String> {
    worker_bridge::app_snapshot_impl(state.get_key().as_deref())
}

#[cfg(not(test))]
#[tauri::command]
pub(crate) fn save_config(
    config: vault_core::AppConfig,
    state: State<'_, SessionState>,
) -> Result<vault_core::AppSnapshot, String> {
    worker_bridge::save_config_impl(config, state.get_key().as_deref())
}

#[cfg(not(test))]
#[tauri::command]
pub(crate) fn set_session_database_key(
    database_key: String,
    state: State<'_, SessionState>,
) -> Result<(), String> {
    worker_bridge::set_session_database_key_impl(database_key, &state)
}

#[cfg(not(test))]
#[tauri::command]
pub(crate) fn clear_session_database_key(state: State<'_, SessionState>) -> Result<(), String> {
    worker_bridge::clear_session_database_key_impl(&state)
}

#[cfg(not(test))]
#[tauri::command]
pub(crate) fn app_lock_status() -> Result<vault_core::AppLockStatus, String> {
    worker_bridge::app_lock_status_impl()
}

#[cfg(not(test))]
#[tauri::command]
pub(crate) fn set_app_lock_passcode(
    request: vault_core::SetAppLockPasscodeRequest,
) -> Result<vault_core::AppLockStatus, String> {
    worker_bridge::set_app_lock_passcode_impl(request)
}

#[cfg(not(test))]
#[tauri::command]
pub(crate) fn clear_app_lock_passcode() -> Result<vault_core::AppLockStatus, String> {
    worker_bridge::clear_app_lock_passcode_impl()
}

#[cfg(not(test))]
#[tauri::command]
pub(crate) fn lock_app_session(
    reason: Option<String>,
) -> Result<vault_core::AppLockStatus, String> {
    worker_bridge::lock_app_session_impl(reason)
}

#[cfg(not(test))]
#[tauri::command]
pub(crate) fn unlock_app_session(
    request: vault_core::UnlockAppSessionRequest,
) -> Result<vault_core::AppLockStatus, String> {
    worker_bridge::unlock_app_session_impl(request)
}
