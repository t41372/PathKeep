//! Non-distribution sandbox for OS integrations that outlive the process.
//!
//! `tauri dev`, `bun run dev:demo` and the desktop-bridge E2E run non-distribution builds
//! under the developer's own account. Without a sandbox, installing the
//! backup schedule there bootstraps a real launchd agent or Task Scheduler
//! task under the development label, schedule status reads the real
//! `~/Library/LaunchAgents/com.yi-ting.pathkeep.dev.backup.plist`, and "Open at
//! login" registers a real login item that points at `target/debug`.
//!
//! When [`SANDBOX_DIR_ENV`] names a directory in a non-distribution build, those
//! integrations keep their state in files under it and never call
//! `launchctl`, `schtasks` or the login-item API. Everything around the OS
//! call still runs: plan generation, plist / XML contents, audit files and
//! status parsing. Distribution builds ignore the variable, the same way they
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
//! 1. Honored in a distribution build: an inherited environment variable would
//!    silently turn off real scheduling and login items. Guard: the classification
//!    check lives in [`sandbox_dir_with_policy`], tested with `allow_test_overrides:
//!    false` by `distribution_build_ignores_the_os_sandbox`. The scheduler
//!    sandbox round-trip tests exercise the real entry point.
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
//! 7. A test or dev run points the project root somewhere else
//!    (`CHB_PROJECT_ROOT`) but forgets the sandbox variable. It then reads,
//!    and could install over or remove, the developer's real LaunchAgent or
//!    Task Scheduler task and login item. Guard: [`ensure_native_os_allowed`]
//!    refuses in a non-distribution build when the project root is overridden and no
//!    sandbox is set; the scheduler and `login_item_store()` call it before
//!    any native call. Distribution builds are not affected: an installed app
//!    with an overridden root still manages the real scheduler.

use std::{ffi::OsString, path::PathBuf};

/// Directory that receives scheduler and login-item state in development builds.
pub const SANDBOX_DIR_ENV: &str = "PATHKEEP_PLATFORM_TEST_SANDBOX_DIR";

/// Moves PathKeep's whole project root; set by tests, `dev:demo` and the E2E run.
///
/// Shares the root override name used by `vault_build` and `vault_core::config`.
pub const PROJECT_ROOT_OVERRIDE_ENV: &str = "CHB_PROJECT_ROOT";

/// Sub-directory standing in for `~/Library/LaunchAgents`.
pub(crate) const LAUNCH_AGENTS_SUBDIR: &str = "LaunchAgents";
/// Sub-directory holding one marker file per "loaded" launchd label.
pub(crate) const LAUNCHD_LOADED_SUBDIR: &str = "launchd-loaded";
/// Sub-directory holding one XML file per registered Task Scheduler task.
pub(crate) const TASK_SCHEDULER_SUBDIR: &str = "TaskScheduler";
/// Sub-directory holding the login item plist (`crate::login_item`).
pub(crate) const LOGIN_ITEMS_SUBDIR: &str = "login-items";

/// Returns the sandbox directory when this is a non-distribution build and the variable is set.
///
/// Everything that honours the sandbox goes through this, so there is one
/// classification gate to audit.
pub fn sandbox_dir() -> Option<PathBuf> {
    sandbox_dir_with_policy(
        !vault_build::CURRENT.is_distribution(),
        std::env::var_os(SANDBOX_DIR_ENV),
    )
}

/// Applies the sandbox rules to an explicit build profile and variable value.
///
/// Split out so tests can check the release behaviour from a development test binary.
pub(crate) fn sandbox_dir_with_policy(
    allow_test_overrides: bool,
    value: Option<OsString>,
) -> Option<PathBuf> {
    if !allow_test_overrides {
        return None;
    }
    let value = value?;
    if value.to_string_lossy().trim().is_empty() {
        return None;
    }
    Some(PathBuf::from(value))
}

/// Fails when a development build is about to reach the real OS scheduler or login
/// items from a run that moved its project root but has no sandbox.
///
/// Such a run is a test or a dev session working on a throwaway archive; its
/// schedule status would describe, and its install / remove would change, the
/// developer's own automatic backup. `service` names what was about to be
/// touched, for the error message.
pub fn ensure_native_os_allowed(service: &str) -> anyhow::Result<()> {
    native_os_guard_with_policy(
        !vault_build::CURRENT.is_distribution(),
        std::env::var_os(PROJECT_ROOT_OVERRIDE_ENV),
        std::env::var_os(SANDBOX_DIR_ENV),
        service,
    )
}

/// Applies the guard rules to an explicit build profile and variable values.
///
/// Split out so tests can check the release behaviour from a development test binary.
pub(crate) fn native_os_guard_with_policy(
    allow_test_overrides: bool,
    project_root: Option<OsString>,
    sandbox: Option<OsString>,
    service: &str,
) -> anyhow::Result<()> {
    if !allow_test_overrides || sandbox_dir_with_policy(allow_test_overrides, sandbox).is_some() {
        return Ok(());
    }
    let Some(root) = project_root.filter(|value| !value.to_string_lossy().trim().is_empty()) else {
        return Ok(());
    };
    anyhow::bail!(
        "Refusing to touch the real {service}: this development build uses the project root {} \
         ({PROJECT_ROOT_OVERRIDE_ENV}) but {SANDBOX_DIR_ENV} is not set, so it would read or \
         change your own installed PathKeep. Set {SANDBOX_DIR_ENV} to a scratch folder.",
        PathBuf::from(root).display()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn development_override_requires_an_os_sandbox() {
        let error = native_os_guard_with_policy(
            true,
            Some(OsString::from("/tmp/pk-root")),
            None,
            "launchd scheduler",
        )
        .expect_err("development build with a moved root and no sandbox must refuse");
        let message = error.to_string();
        assert!(message.contains("launchd scheduler"), "{message}");
        assert!(message.contains(PROJECT_ROOT_OVERRIDE_ENV), "{message}");
        assert!(message.contains(SANDBOX_DIR_ENV), "{message}");
        assert!(message.contains("/tmp/pk-root"), "{message}");

        // A blank sandbox value is unset, so it does not lift the guard.
        assert!(
            native_os_guard_with_policy(
                true,
                Some(OsString::from("/tmp/pk-root")),
                Some(OsString::from(" ")),
                "login items",
            )
            .is_err()
        );
    }

    #[test]
    fn distribution_build_ignores_the_os_sandbox() {
        assert_eq!(sandbox_dir_with_policy(false, Some(OsString::from("/tmp/pk-sandbox"))), None);
    }
}
