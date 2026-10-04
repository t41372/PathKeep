//! Debug-only sandbox for OS integrations that outlive the process.
//!
//! `tauri dev`, `bun run dev:demo` and the desktop-bridge E2E run debug builds
//! under the developer's own account. Without a sandbox, installing the
//! backup schedule there bootstraps a real launchd agent or Task Scheduler
//! task under the real label, schedule status reads the real
//! `~/Library/LaunchAgents/com.yi-ting.pathkeep.backup.plist`, and "Open at
//! login" registers a real login item that points at `target/debug`.
//!
//! When [`SANDBOX_DIR_ENV`] names a directory in a debug build, those
//! integrations keep their state in files under it and never call
//! `launchctl`, `schtasks` or the login-item API. Everything around the OS
//! call still runs: plan generation, plist / XML contents, audit files and
//! status parsing. Release builds ignore the variable, the same way they
//! ignore the keyring redirect in `test_support`.
//!
//! ## Responsibilities
//! - Decide whether the sandbox is active.
//! - Name the sub-directory each emulated service keeps its state in.
//!
//! ## Not responsible for
//! - Emulating a service. `scheduler/macos.rs` (launchd),
//!   `scheduler/windows.rs` (Task Scheduler) and `login_item.rs` (open at
//!   login) own their emulators. The Linux scheduler is manual-only and
//!   calls nothing.
//! - The keyring redirect (`PATHKEEP_PLATFORM_TEST_KEYRING_DIR`), which is set
//!   separately.
//!
//! ## How this could fail, and what guards it
//! 1. Honored in a release build: an inherited environment variable would
//!    silently turn off real scheduling and login items. Guard: the debug
//!    check lives in [`sandbox_dir_with_policy`], tested with `debug_build:
//!    false`, and `sandbox_dir_matches_the_build_profile` checks the real
//!    entry point under `cargo test --release` too.
//! 2. Partially honored, so one call in an operation still reaches the OS.
//!    Guard: each scheduler operation resolves its host once and passes it
//!    down; tests count native calls and require zero under the sandbox.
//! 3. Status reads the real location while apply writes the sandbox. Guard:
//!    the LaunchAgents directory comes from the same resolved host; tests run
//!    apply → status → remove and require every reported path to sit inside
//!    the sandbox and status to see what apply installed.
//! 4. Emulated state kept in memory or the environment, so a second process
//!    (the worker sidecar, a relaunch) sees something different. Guard: state
//!    is plain files; tests read it back through separate calls.
//! 5. An empty value treated as "sandbox in the current directory". Guard:
//!    blank values count as unset.
//! 6. "Open at login" on Windows / Linux still reaching the autostart plugin.
//!    Guard: `login_item_store()` returns the sandbox store on every OS
//!    before it considers the platform, and the desktop crate only falls back
//!    to the plugin when that returns `None`.

use std::{ffi::OsString, path::PathBuf};

/// Directory that receives scheduler and login-item state in debug builds.
pub const SANDBOX_DIR_ENV: &str = "PATHKEEP_PLATFORM_TEST_SANDBOX_DIR";

/// Sub-directory standing in for `~/Library/LaunchAgents`.
pub(crate) const LAUNCH_AGENTS_SUBDIR: &str = "LaunchAgents";
/// Sub-directory holding one marker file per "loaded" launchd label.
pub(crate) const LAUNCHD_LOADED_SUBDIR: &str = "launchd-loaded";
/// Sub-directory holding one XML file per registered Task Scheduler task.
pub(crate) const TASK_SCHEDULER_SUBDIR: &str = "TaskScheduler";
/// Sub-directory holding the login item plist (`crate::login_item`).
pub(crate) const LOGIN_ITEMS_SUBDIR: &str = "login-items";

/// Returns the sandbox directory when this is a debug build and the variable is set.
///
/// Everything that honours the sandbox goes through this, so there is one
/// debug gate to audit.
pub fn sandbox_dir() -> Option<PathBuf> {
    sandbox_dir_with_policy(cfg!(debug_assertions), std::env::var_os(SANDBOX_DIR_ENV))
}

/// Applies the sandbox rules to an explicit build profile and variable value.
///
/// Split out so tests can check the release behaviour from a debug test binary.
pub(crate) fn sandbox_dir_with_policy(
    debug_build: bool,
    value: Option<OsString>,
) -> Option<PathBuf> {
    if !debug_build {
        return None;
    }
    let value = value?;
    if value.to_string_lossy().trim().is_empty() {
        return None;
    }
    Some(PathBuf::from(value))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{env_lock, restore_env_var};

    #[test]
    fn sandbox_dir_is_ignored_when_not_a_debug_build() {
        assert_eq!(sandbox_dir_with_policy(false, Some(OsString::from("/tmp/pk-sandbox"))), None);
    }

    #[test]
    fn sandbox_dir_needs_a_non_blank_value() {
        assert_eq!(sandbox_dir_with_policy(true, None), None);
        assert_eq!(sandbox_dir_with_policy(true, Some(OsString::new())), None);
        assert_eq!(sandbox_dir_with_policy(true, Some(OsString::from("  "))), None);
        assert_eq!(
            sandbox_dir_with_policy(true, Some(OsString::from("/tmp/pk-sandbox"))),
            Some(PathBuf::from("/tmp/pk-sandbox"))
        );
    }

    #[test]
    fn sandbox_dir_matches_the_build_profile() {
        let _guard = env_lock().lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let original = std::env::var_os(SANDBOX_DIR_ENV);
        unsafe {
            std::env::set_var(SANDBOX_DIR_ENV, "/tmp/pk-sandbox");
        }
        let resolved = sandbox_dir();
        restore_env_var(SANDBOX_DIR_ENV, original.as_deref());

        // `cargo test --release` turns debug assertions off; this then checks
        // the shipped behaviour through the real entry point.
        if cfg!(debug_assertions) {
            assert_eq!(resolved, Some(PathBuf::from("/tmp/pk-sandbox")));
        } else {
            assert_eq!(resolved, None);
        }
    }
}
