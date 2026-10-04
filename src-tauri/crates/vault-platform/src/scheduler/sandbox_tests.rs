//! Tests for the debug scheduler sandbox (`crate::sandbox`).
//!
//! Each test walks a whole apply → status → remove cycle through the public
//! scheduler API with the sandbox on, and checks the failure modes listed in
//! the sandbox module header: no native call, no file outside the sandbox,
//! status reading what apply wrote, and state that survives between calls.
//! The round trips need a debug build; under `--release` (or
//! `-C debug-assertions=off`) the last test checks the sandbox is ignored.

// Most helpers serve the debug-only round trips.
#![cfg_attr(not(debug_assertions), allow(unused_imports, dead_code))]

use super::*;
use crate::sandbox::{LAUNCHD_LOADED_SUBDIR, SANDBOX_DIR_ENV, TASK_SCHEDULER_SUBDIR};
use crate::test_support::{
    TEST_LAUNCH_AGENTS_DIR_ENV, TEST_LAUNCHCTL_SUCCESS_ENV, env_lock, restore_env_var,
};
use std::ffi::OsString;
use tempfile::tempdir;

/// Sets env vars for one test and restores them on drop, even after a panic.
struct EnvVars(Vec<(&'static str, Option<OsString>)>);

impl EnvVars {
    fn set(vars: &[(&'static str, &std::ffi::OsStr)]) -> Self {
        let saved = vars.iter().map(|(name, _)| (*name, std::env::var_os(name))).collect();
        for (name, value) in vars {
            unsafe { std::env::set_var(name, value) };
        }
        Self(saved)
    }
}

impl Drop for EnvVars {
    fn drop(&mut self) {
        for (name, value) in &self.0 {
            restore_env_var(name, value.as_deref());
        }
    }
}

fn params() -> ScheduleParameters {
    ScheduleParameters { due_after_hours: 72.0, check_interval_hours: 6.0 }
}

fn assert_inside(path: &str, root: &Path) {
    assert!(Path::new(path).starts_with(root), "{path} escaped the sandbox {}", root.display());
}

#[test]
#[cfg(debug_assertions)]
fn macos_sandbox_round_trip_never_reaches_launchd_or_the_real_agents_dir() {
    let _guard = env_lock().lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let dir = tempdir().expect("tempdir");
    let sandbox = dir.path().join("sandbox");
    // Decoys: if any step fell back to the older overrides or the launchctl
    // stub, files would land in `decoy-agents` or the stub would fail apply.
    let decoy_agents = dir.path().join("decoy-agents");
    let _env = EnvVars::set(&[
        (SANDBOX_DIR_ENV, sandbox.as_os_str()),
        (TEST_LAUNCH_AGENTS_DIR_ENV, decoy_agents.as_os_str()),
        (TEST_LAUNCHCTL_SUCCESS_ENV, "0".as_ref()),
    ]);
    let paths = vault_core::config::project_paths_with_root(&dir.path().join("project"));
    let executable = Path::new("/tmp/pathkeep-desktop");
    let native_calls_before = native_call_count();

    let plan = preview_schedule(Some("macos"), executable, &paths, &params()).expect("plan");
    let applied = apply_schedule(&plan, &paths).expect("apply");
    assert!(applied.applied, "{}", applied.message);
    assert_eq!(applied.files.len(), 1);
    assert_inside(&applied.files[0], &sandbox);
    assert!(sandbox.join(LAUNCHD_LOADED_SUBDIR).join(&plan.label).is_file());

    // A separate call sees what apply left behind: state is on disk.
    let installed = schedule_status(Some("macos"), executable, &paths, &params()).expect("status");
    assert_eq!(installed.install_state, "installed");
    assert_eq!(installed.detected_files.len(), 1);
    assert_inside(&installed.detected_files[0], &sandbox);

    // Real status parsing still runs: a drifted plist reads as a mismatch.
    std::fs::write(&applied.files[0], "<plist/>").expect("drift plist");
    let drifted = schedule_status(Some("macos"), executable, &paths, &params()).expect("status");
    assert_eq!(drifted.install_state, "mismatch");

    let removed = remove_schedule(&plan, &paths).expect("remove");
    assert!(removed.applied);
    assert!(!Path::new(&applied.files[0]).exists());
    assert!(!sandbox.join(LAUNCHD_LOADED_SUBDIR).join(&plan.label).exists());
    let after = schedule_status(Some("macos"), executable, &paths, &params()).expect("status");
    assert_eq!(after.install_state, "not-installed");

    let repaired = repair_schedule(&plan, &paths).expect("repair");
    assert!(!repaired.applied);

    assert!(!decoy_agents.exists(), "the older LaunchAgents override was used");
    assert_eq!(native_call_count(), native_calls_before, "a native launchd call happened");
}

#[test]
#[cfg(debug_assertions)]
fn macos_sandbox_bootstrap_rejects_a_plist_launchd_could_not_load() {
    let _guard = env_lock().lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let dir = tempdir().expect("tempdir");
    let sandbox = dir.path().join("sandbox");
    let _env = EnvVars::set(&[(SANDBOX_DIR_ENV, sandbox.as_os_str())]);
    let paths = vault_core::config::project_paths_with_root(&dir.path().join("project"));

    let mut plan =
        preview_schedule(Some("macos"), Path::new("/tmp/pathkeep-desktop"), &paths, &params())
            .expect("plan");
    plan.generated_files[0].contents = "not a plist".to_string();
    let result = apply_schedule(&plan, &paths).expect("apply");

    assert!(!result.applied);
    assert!(!sandbox.join(LAUNCHD_LOADED_SUBDIR).join(&plan.label).exists());
}

#[test]
#[cfg(debug_assertions)]
fn windows_sandbox_round_trip_never_reaches_schtasks() {
    let _guard = env_lock().lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let dir = tempdir().expect("tempdir");
    let sandbox = dir.path().join("sandbox");
    // Decoy: the schtasks stub fails every call in this mode.
    let _env = EnvVars::set(&[
        (SANDBOX_DIR_ENV, sandbox.as_os_str()),
        (TEST_SCHTASKS_MODE_ENV, "fail".as_ref()),
    ]);
    let paths = vault_core::config::project_paths_with_root(&dir.path().join("project"));
    let executable = Path::new("C:/Program Files/PathKeep/pathkeep-desktop.exe");
    let native_calls_before = native_call_count();

    let missing = schedule_status(Some("windows"), executable, &paths, &params()).expect("status");
    assert_eq!(missing.install_state, "not-installed");

    let plan = preview_schedule(Some("windows"), executable, &paths, &params()).expect("plan");
    let applied = apply_schedule(&plan, &paths).expect("apply");
    assert!(applied.applied, "{}", applied.message);
    let task_file = sandbox.join(TASK_SCHEDULER_SUBDIR).join(format!("{}.xml", plan.label));
    assert!(task_file.is_file());

    let installed =
        schedule_status(Some("windows"), executable, &paths, &params()).expect("status");
    assert_eq!(installed.install_state, "installed");

    let removed = remove_schedule(&plan, &paths).expect("remove");
    assert!(removed.applied);
    assert!(!task_file.exists());
    let removed_again = remove_schedule(&plan, &paths).expect("remove again");
    assert!(!removed_again.applied);
    assert_eq!(removed_again.step_results[0].status, "ok", "not-found reads as already removed");

    assert_eq!(native_call_count(), native_calls_before, "a native schtasks call happened");
}

#[test]
fn without_the_sandbox_scheduler_calls_go_to_the_native_layer() {
    // The counter itself must work, or the zero-call assertions above prove nothing.
    let _guard = env_lock().lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let dir = tempdir().expect("tempdir");
    let original = std::env::var_os(SANDBOX_DIR_ENV);
    unsafe { std::env::remove_var(SANDBOX_DIR_ENV) };
    let paths = vault_core::config::project_paths_with_root(dir.path());
    let before = native_call_count();

    let status = schedule_status(Some("windows"), Path::new("C:/pk.exe"), &paths, &params());

    restore_env_var(SANDBOX_DIR_ENV, original.as_deref());
    status.expect("status");
    assert_eq!(native_call_count(), before + 1);
}

#[test]
#[cfg(not(debug_assertions))]
fn release_builds_ignore_the_sandbox_for_scheduler_calls() {
    let _guard = env_lock().lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let dir = tempdir().expect("tempdir");
    let sandbox = dir.path().join("sandbox");
    let _env = EnvVars::set(&[(SANDBOX_DIR_ENV, sandbox.as_os_str())]);
    let paths = vault_core::config::project_paths_with_root(&dir.path().join("project"));
    let before = native_call_count();

    let plan =
        preview_schedule(Some("windows"), Path::new("C:/pk.exe"), &paths, &params()).expect("plan");
    apply_schedule(&plan, &paths).expect("apply");
    schedule_status(Some("windows"), Path::new("C:/pk.exe"), &paths, &params()).expect("status");

    assert_eq!(native_call_count(), before + 2, "release must use the native scheduler");
    assert!(!sandbox.exists(), "release wrote into the sandbox");
}

/// A run that moved its project root but forgot the sandbox must not read or
/// change the real scheduler or login item (`crate::sandbox` failure mode 7).
#[test]
#[cfg(debug_assertions)]
fn moved_project_root_without_a_sandbox_never_reaches_the_native_scheduler() {
    use crate::sandbox::PROJECT_ROOT_OVERRIDE_ENV;
    let _guard = env_lock().lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let dir = tempdir().expect("tempdir");
    let project = dir.path().join("project");
    // Decoy: if status fell through to the older override, it would read here.
    let decoy_agents = dir.path().join("decoy-agents");
    let original_sandbox = std::env::var_os(SANDBOX_DIR_ENV);
    unsafe { std::env::remove_var(SANDBOX_DIR_ENV) };
    let _env = EnvVars::set(&[
        (PROJECT_ROOT_OVERRIDE_ENV, project.as_os_str()),
        (TEST_LAUNCH_AGENTS_DIR_ENV, decoy_agents.as_os_str()),
    ]);
    let paths = vault_core::config::project_paths_with_root(&project);
    let executable = Path::new("/tmp/pathkeep-desktop");
    let native_calls_before = native_call_count();

    let plan = preview_schedule(Some("macos"), executable, &paths, &params()).expect("preview");
    let refused = [
        schedule_status(Some("macos"), executable, &paths, &params()).map(|_| ()),
        schedule_status(Some("windows"), executable, &paths, &params()).map(|_| ()),
        apply_schedule(&plan, &paths).map(|_| ()),
        remove_schedule(&plan, &paths).map(|_| ()),
        repair_schedule(&plan, &paths).map(|_| ()),
        crate::login_item::login_item_store().map(|_| ()),
    ];
    // Linux setup is manual: nothing native is touched, so status still works.
    let linux = schedule_status(Some("linux"), executable, &paths, &params());
    restore_env_var(SANDBOX_DIR_ENV, original_sandbox.as_deref());

    for result in refused {
        let message = result.expect_err("must refuse").to_string();
        assert!(message.contains(SANDBOX_DIR_ENV), "{message}");
    }
    assert_eq!(linux.expect("linux status").install_state, "manual-review");
    assert!(!decoy_agents.exists(), "status read a LaunchAgents folder");
    assert!(!paths.audit_repo_path.join("scheduler").exists(), "an audit file was written");
    assert_eq!(native_call_count(), native_calls_before, "a native scheduler call happened");
}
