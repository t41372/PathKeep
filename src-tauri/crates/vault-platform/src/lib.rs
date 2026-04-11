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
mod host_capability;
mod keyring;
mod launcher;
mod scheduler;
pub mod test_support;

/// Returns the current App Lock biometric capability state for the host.
pub use biometric::{app_lock_biometric_state, authenticate_app_lock_biometric};
/// Discovers browser profiles through the host-facing platform adapter.
pub use discovery::discover_browser_profiles;
/// Returns the normalized platform name used in schedule and diagnostics UIs.
pub use host_capability::current_platform_name;
/// Native keyring operations used for database keys, provider secrets, and S3 credentials.
pub use keyring::{
    keyring_clear_database_key, keyring_clear_provider_api_key, keyring_clear_s3_credentials,
    keyring_get_database_key, keyring_get_provider_api_key, keyring_get_s3_credentials,
    keyring_set_database_key, keyring_set_provider_api_key, keyring_set_s3_credentials,
    keyring_status, provider_api_key_saved, s3_credentials_saved,
};
/// Opens URLs and filesystem paths using the host shell.
pub use launcher::{open_external_url, open_path_in_file_manager};
/// Preview/apply/remove schedule adapters backed by the native scheduler.
pub use scheduler::{
    ScheduleParameters, apply_schedule, preview_schedule, remove_schedule, schedule_status,
};
