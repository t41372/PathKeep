//! Native platform adapters for PathKeep.
//!
//! This crate wraps OS-specific capabilities that the core domain depends on
//! but should not implement itself: browser discovery helpers, keyrings,
//! schedulers, launcher/file-manager integration, and biometric prompts.
//!
//! The boundary here is intentionally narrow. These functions translate host
//! behavior into simple Rust results; they do not redefine product semantics
//! such as archive visibility, App Lock meaning, or schedule UX.

mod biometric;
mod discovery;
mod full_disk_access;
mod host_capability;
mod keyring;
mod launcher;
mod locale;
mod login_item;
mod sandbox;
mod scheduler;
pub mod test_support;

/// Returns the current App Lock biometric capability state for the host.
pub use biometric::{app_lock_biometric_state, authenticate_app_lock_biometric};
/// Discovers browser profiles through the host-facing platform adapter.
pub use discovery::discover_browser_profiles;
/// Probes whether macOS Full Disk Access is denied, independent of browser discovery.
pub use full_disk_access::{
    FullDiskAccessProbe, probe_full_disk_access, probe_full_disk_access_at,
};
/// Returns the normalized platform name used in schedule and diagnostics UIs.
pub use host_capability::{current_platform_name, menu_bar_icon_supported};
/// Native keyring operations used for database keys and AI provider secrets.
pub use keyring::{
    keyring_clear_database_key, keyring_clear_provider_api_key, keyring_get_database_key,
    keyring_get_provider_api_key, keyring_set_database_key, keyring_set_provider_api_key,
    keyring_status, provider_api_key_saved,
};
/// Opens URLs and filesystem paths using the host shell.
pub use launcher::{open_external_url, open_path_in_file_manager};
/// The OS's preferred UI languages, for native menus drawn outside the webview.
pub use locale::preferred_ui_languages;
/// "Open at login" as a LaunchAgent (macOS, and the debug sandbox everywhere).
pub use login_item::{
    LAUNCHED_AT_LOGIN_ARG, LOGIN_ITEM_LABEL, LoginItemStore, login_item_program_arguments,
    login_item_store,
};
/// Debug-only redirect of scheduler and login-item state into a directory.
pub use sandbox::{SANDBOX_DIR_ENV, sandbox_dir};
/// Preview/apply/remove schedule adapters backed by the native scheduler.
pub use scheduler::{
    ScheduleParameters, apply_schedule, preview_schedule, remove_schedule, repair_schedule,
    schedule_status,
};
