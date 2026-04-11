use std::path::PathBuf;

pub const DEFAULT_KEYRING_SERVICE: &str = "com.yi-ting.pathkeep";
pub const DEFAULT_SCHEDULE_LABEL: &str = "com.yi-ting.pathkeep.backup";

pub const TEST_KEYRING_DIR_ENV: &str = "PATHKEEP_PLATFORM_TEST_KEYRING_DIR";
pub const TEST_KEYRING_SERVICE_ENV: &str = "PATHKEEP_PLATFORM_TEST_KEYRING_SERVICE";
pub const TEST_SCHEDULE_LABEL_ENV: &str = "PATHKEEP_PLATFORM_TEST_SCHEDULE_LABEL";
pub const TEST_LAUNCH_AGENTS_DIR_ENV: &str = "PATHKEEP_PLATFORM_TEST_LAUNCH_AGENTS_DIR";
pub const TEST_LAUNCHCTL_SUCCESS_ENV: &str = "PATHKEEP_PLATFORM_TEST_LAUNCHCTL_SUCCESS";

const LEGACY_TEST_KEYRING_DIR_ENV: &str = "CHB_TEST_KEYRING_DIR";
const LEGACY_TEST_LAUNCH_AGENTS_DIR_ENV: &str = "CHB_TEST_LAUNCH_AGENTS_DIR";
#[cfg(any(test, coverage))]
const LEGACY_TEST_LAUNCHCTL_SUCCESS_ENV: &str = "CHB_TEST_LAUNCHCTL_SUCCESS";

pub fn keyring_service() -> String {
    std::env::var(TEST_KEYRING_SERVICE_ENV)
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| DEFAULT_KEYRING_SERVICE.to_string())
}

pub fn schedule_label() -> String {
    std::env::var(TEST_SCHEDULE_LABEL_ENV)
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| DEFAULT_SCHEDULE_LABEL.to_string())
}

pub(crate) fn launch_agents_dir_override() -> Option<PathBuf> {
    std::env::var_os(TEST_LAUNCH_AGENTS_DIR_ENV)
        .or_else(|| std::env::var_os(LEGACY_TEST_LAUNCH_AGENTS_DIR_ENV))
        .map(PathBuf::from)
}

#[cfg(any(test, coverage))]
pub(crate) fn launchctl_stub_success() -> bool {
    std::env::var(TEST_LAUNCHCTL_SUCCESS_ENV)
        .or_else(|_| std::env::var(LEGACY_TEST_LAUNCHCTL_SUCCESS_ENV))
        .unwrap_or_else(|_| "1".to_string())
        != "0"
}

#[cfg(coverage)]
pub(crate) fn test_keyring_dir() -> Option<PathBuf> {
    static ROOT: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();
    Some(
        std::env::var_os(TEST_KEYRING_DIR_ENV)
            .or_else(|| std::env::var_os(LEGACY_TEST_KEYRING_DIR_ENV))
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                ROOT.get_or_init(|| {
                    std::env::temp_dir()
                        .join(format!("pathkeep-coverage-keyring-{}", std::process::id()))
                })
                .clone()
            }),
    )
}

#[cfg(not(coverage))]
pub(crate) fn test_keyring_dir() -> Option<PathBuf> {
    std::env::var_os(TEST_KEYRING_DIR_ENV)
        .or_else(|| std::env::var_os(LEGACY_TEST_KEYRING_DIR_ENV))
        .map(PathBuf::from)
}

#[cfg(test)]
pub(crate) fn env_lock() -> &'static std::sync::Mutex<()> {
    static LOCK: std::sync::OnceLock<std::sync::Mutex<()>> = std::sync::OnceLock::new();
    LOCK.get_or_init(|| std::sync::Mutex::new(()))
}

#[cfg(test)]
pub(crate) fn restore_env_var(name: &str, value: Option<&std::ffi::OsStr>) {
    unsafe {
        if let Some(value) = value {
            std::env::set_var(name, value);
        } else {
            std::env::remove_var(name);
        }
    }
}
