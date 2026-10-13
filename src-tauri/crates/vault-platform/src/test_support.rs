//! Shared test environment helpers for platform adapters.
//!
//! These helpers keep platform tests deterministic by centralizing environment
//! overrides for the keyring, scheduler labels and the LaunchAgents folder.

use std::path::PathBuf;

/// Default keyring service name used by PathKeep tests and local backends.
pub const DEFAULT_KEYRING_SERVICE: &str = "com.yi-ting.pathkeep";
/// Default scheduler label used by PathKeep tests and preview plans.
pub const DEFAULT_SCHEDULE_LABEL: &str = "com.yi-ting.pathkeep.backup";

/// Env var override for the file-backed test keyring directory.
pub const TEST_KEYRING_DIR_ENV: &str = "PATHKEEP_PLATFORM_TEST_KEYRING_DIR";
/// Env var override for the test keyring service name.
pub const TEST_KEYRING_SERVICE_ENV: &str = "PATHKEEP_PLATFORM_TEST_KEYRING_SERVICE";
/// Env var override for the test schedule label.
pub const TEST_SCHEDULE_LABEL_ENV: &str = "PATHKEEP_PLATFORM_TEST_SCHEDULE_LABEL";
/// Env var override for the Windows Task Scheduler user id used in tests.
pub const TEST_WINDOWS_USER_ID_ENV: &str = "PATHKEEP_PLATFORM_TEST_WINDOWS_USER_ID";
/// Env var override for the test LaunchAgents directory.
pub const TEST_LAUNCH_AGENTS_DIR_ENV: &str = "PATHKEEP_PLATFORM_TEST_LAUNCH_AGENTS_DIR";
/// A file of this name in the test keyring's directory makes the keyring unavailable, as on a
/// Linux without a secret service: nothing is read or saved, and its status says so.
pub const TEST_KEYRING_UNAVAILABLE_FILE: &str = "unavailable";

const LEGACY_TEST_KEYRING_DIR_ENV: &str = "CHB_TEST_KEYRING_DIR";
const LEGACY_TEST_LAUNCH_AGENTS_DIR_ENV: &str = "CHB_TEST_LAUNCH_AGENTS_DIR";

/// Returns the service name tests should use for native-keyring isolation.
pub fn keyring_service() -> String {
    std::env::var(TEST_KEYRING_SERVICE_ENV)
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| DEFAULT_KEYRING_SERVICE.to_string())
}

/// Returns the scheduler label tests should use for deterministic assertions.
pub fn schedule_label() -> String {
    std::env::var(TEST_SCHEDULE_LABEL_ENV)
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| DEFAULT_SCHEDULE_LABEL.to_string())
}

/// Returns an override for the macOS LaunchAgents directory when tests provide one.
///
/// Debug builds only, like the keyring redirect: a release build must always
/// install into the real `~/Library/LaunchAgents`.
pub(crate) fn launch_agents_dir_override() -> Option<PathBuf> {
    if !cfg!(debug_assertions) {
        return None;
    }
    std::env::var_os(TEST_LAUNCH_AGENTS_DIR_ENV)
        .or_else(|| std::env::var_os(LEGACY_TEST_LAUNCH_AGENTS_DIR_ENV))
        .map(PathBuf::from)
}

/// Returns the file-backed keyring directory when tests opt into one.
///
/// Release builds must NEVER honor the redirect env vars: doing so would let an
/// attacker-controlled launch environment force PathKeep to persist the
/// SQLCipher master key and provider API keys as plaintext files instead of the
/// OS keychain. `debug_assertions` is on for `cargo test` and `tauri dev` (the
/// e2e/desktop-bridge harness) and off for `--release`, so the test redirect
/// stays available where it is needed and is inert in shipped binaries.
pub(crate) fn test_keyring_dir() -> Option<PathBuf> {
    if !cfg!(debug_assertions) {
        return None;
    }
    std::env::var_os(TEST_KEYRING_DIR_ENV)
        .or_else(|| std::env::var_os(LEGACY_TEST_KEYRING_DIR_ENV))
        .map(PathBuf::from)
}

#[cfg(test)]
/// Returns the shared env-var mutex used by platform tests.
pub(crate) fn env_lock() -> &'static std::sync::Mutex<()> {
    static LOCK: std::sync::OnceLock<std::sync::Mutex<()>> = std::sync::OnceLock::new();
    LOCK.get_or_init(|| std::sync::Mutex::new(()))
}

#[cfg(test)]
/// Restores one environment variable to its prior value after a test.
pub(crate) fn restore_env_var(name: &str, value: Option<&std::ffi::OsStr>) {
    unsafe {
        if let Some(value) = value {
            std::env::set_var(name, value);
        } else {
            std::env::remove_var(name);
        }
    }
}

/// Number of secret reads from one file-backed keyring. Counts never include secrets.
#[cfg(debug_assertions)]
pub fn keyring_secret_read_count(root: &std::path::Path) -> usize {
    keyring_read_counts().lock().unwrap().get(root).copied().unwrap_or(0)
}
#[cfg(debug_assertions)]
pub(crate) fn record_keyring_secret_read(root: &std::path::Path) {
    *keyring_read_counts().lock().unwrap().entry(root.to_owned()).or_default() += 1;
}
#[cfg(debug_assertions)]
fn keyring_read_counts()
-> &'static std::sync::Mutex<std::collections::HashMap<std::path::PathBuf, usize>> {
    static COUNTS: std::sync::OnceLock<
        std::sync::Mutex<std::collections::HashMap<std::path::PathBuf, usize>>,
    > = std::sync::OnceLock::new();
    COUNTS.get_or_init(Default::default)
}
