mod biometric;
mod discovery;
mod host_capability;
mod keyring;
mod launcher;
mod scheduler;
pub mod test_support;

pub use biometric::{app_lock_biometric_state, authenticate_app_lock_biometric};
pub use discovery::discover_browser_profiles;
pub use host_capability::current_platform_name;
pub use keyring::{
    keyring_clear_database_key, keyring_clear_provider_api_key, keyring_clear_s3_credentials,
    keyring_get_database_key, keyring_get_provider_api_key, keyring_get_s3_credentials,
    keyring_set_database_key, keyring_set_provider_api_key, keyring_set_s3_credentials,
    keyring_status, provider_api_key_saved, s3_credentials_saved,
};
pub use launcher::{open_external_url, open_path_in_file_manager};
pub use scheduler::{
    ScheduleParameters, apply_schedule, preview_schedule, remove_schedule, schedule_status,
};
