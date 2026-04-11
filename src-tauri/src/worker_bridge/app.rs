//! Worker-bridge helpers for app bootstrap and session controls.

use crate::{
    PRODUCT_DISPLAY_NAME,
    session::{SessionState, update_session_key},
};
use vault_core::{AppConfig, SetAppLockPasscodeRequest, UnlockAppSessionRequest};

use super::worker_result;

/// Builds the immutable desktop build-info payload.
pub(crate) fn app_build_info_impl() -> vault_core::AppBuildInfo {
    vault_core::AppBuildInfo {
        product_name: PRODUCT_DISPLAY_NAME.to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        git_commit_short: option_env!("BHB_GIT_COMMIT_SHORT").unwrap_or("unknown").to_string(),
        git_commit_full: option_env!("BHB_GIT_COMMIT_FULL").unwrap_or("unknown").to_string(),
        git_dirty: option_env!("BHB_GIT_DIRTY").unwrap_or("false") == "true",
    }
}

/// Loads the current worker-composed desktop snapshot.
pub(crate) fn app_snapshot_impl(
    session_database_key: Option<&str>,
) -> Result<vault_core::AppSnapshot, String> {
    worker_result(vault_worker::app_snapshot(session_database_key))
}

/// Persists app config and returns the refreshed snapshot.
pub(crate) fn save_config_impl(
    config: AppConfig,
    session_database_key: Option<&str>,
) -> Result<vault_core::AppSnapshot, String> {
    worker_result(vault_worker::save_user_config(&config, session_database_key))
}

/// Caches a session-only database key in the in-process Tauri state.
pub(crate) fn set_session_database_key_impl(
    database_key: String,
    state: &SessionState,
) -> Result<(), String> {
    update_session_key(state, Some(database_key))
}

/// Clears the session-only database key from the in-process Tauri state.
pub(crate) fn clear_session_database_key_impl(state: &SessionState) -> Result<(), String> {
    update_session_key(state, None)
}
#[cfg_attr(test, allow(dead_code))]
/// Loads the current App Lock status from the worker layer.
pub(crate) fn app_lock_status_impl() -> Result<vault_core::AppLockStatus, String> {
    worker_result(vault_worker::load_app_lock_status())
}

#[cfg_attr(test, allow(dead_code))]
/// Configures the App Lock passcode and returns the updated lock state.
pub(crate) fn set_app_lock_passcode_impl(
    request: SetAppLockPasscodeRequest,
) -> Result<vault_core::AppLockStatus, String> {
    worker_result(vault_worker::configure_app_lock_passcode(&request))
}

#[cfg_attr(test, allow(dead_code))]
/// Removes the App Lock passcode and returns the updated lock state.
pub(crate) fn clear_app_lock_passcode_impl() -> Result<vault_core::AppLockStatus, String> {
    worker_result(vault_worker::remove_app_lock_passcode())
}

#[cfg_attr(test, allow(dead_code))]
/// Locks the current UI session for the supplied reason, if any.
pub(crate) fn lock_app_session_impl(
    reason: Option<String>,
) -> Result<vault_core::AppLockStatus, String> {
    worker_result(vault_worker::lock_app_ui_session(reason.as_deref()))
}

#[cfg_attr(test, allow(dead_code))]
/// Unlocks the current UI session through the worker contract.
pub(crate) fn unlock_app_session_impl(
    request: UnlockAppSessionRequest,
) -> Result<vault_core::AppLockStatus, String> {
    worker_result(vault_worker::unlock_app_ui_session(&request))
}
