//! Native scheduler adapters.
//!
//! PathKeep's schedule contract is explicit preview/manual/apply/remove/verify.
//! This facade keeps the public worker-facing API stable while platform-specific
//! behavior lives in focused owners under `scheduler/`.

mod audit;
mod linux;
mod macos;
#[cfg(test)]
mod sandbox_tests;
mod windows;

use crate::{host_capability::current_platform_name, test_support::schedule_label};
use anyhow::Result;
use chrono::Utc;
use serde::Serialize;
use std::{
    fs,
    path::{Path, PathBuf},
};
use vault_core::{
    ProjectPaths,
    models::{ApplyResult, ScheduleIssue, SchedulePlan, ScheduleStatus},
};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
/// Runtime parameters that shape the generated backup schedule plan.
pub struct ScheduleParameters {
    pub due_after_hours: f64,
    pub check_interval_hours: f64,
}

const LEGACY_MACOS_SCHEDULE_LABELS: &[&str] =
    &["dev.codex.pathkeep.backup", "dev.codex.browser-history-backup.backup"];

/// Where one scheduler operation sends its OS side effects.
///
/// Apply, remove, repair and status resolve this once and pass it down, so
/// every OS touchpoint in one operation goes to the same place. See
/// `crate::sandbox` for why the sandbox exists.
#[derive(Debug, Clone)]
pub(super) enum SchedulerHost {
    /// The real `launchctl` / `schtasks`. Under `cfg(test)`, stand-ins that run
    /// nothing and report failure, so in-crate tests never reach the real scheduler.
    Native,
    /// Files under the debug sandbox directory; no process is spawned.
    Sandbox(PathBuf),
}

impl SchedulerHost {
    /// Picks the sandbox when it is on, otherwise the real scheduler, which a
    /// debug run with a moved project root is not allowed to reach
    /// (see `crate::sandbox::ensure_native_os_allowed`).
    ///
    /// Only the platforms that call a native scheduler resolve a host; the
    /// manual-only Linux path never does, so it keeps working under the guard.
    fn resolve(platform: &str) -> Result<Self> {
        if let Some(dir) = crate::sandbox::sandbox_dir() {
            return Ok(Self::Sandbox(dir));
        }
        let service = if platform == "windows" { "Task Scheduler" } else { "launchd scheduler" };
        crate::sandbox::ensure_native_os_allowed(service)?;
        Ok(Self::Native)
    }
}

#[cfg(test)]
thread_local! {
    /// Calls that reached the native scheduler layer on this thread.
    static NATIVE_CALLS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// Records that a native scheduler call happened, so sandbox tests can assert none did.
pub(super) fn count_native_call() {
    #[cfg(test)]
    NATIVE_CALLS.with(|calls| calls.set(calls.get() + 1));
}

#[cfg(test)]
pub(super) fn native_call_count() -> usize {
    NATIVE_CALLS.with(std::cell::Cell::get)
}

/// Builds a preview-only native schedule plan for the requested platform.
pub fn preview_schedule(
    platform: Option<&str>,
    executable_path: &Path,
    paths: &ProjectPaths,
    params: &ScheduleParameters,
) -> Result<SchedulePlan> {
    let platform = platform.map(ToOwned::to_owned).unwrap_or_else(current_platform_name);
    let label = schedule_label();
    let worker_executable = scheduled_worker_executable(executable_path)?;
    let dedicated_worker = is_dedicated_worker(&worker_executable);
    let mut worker_args = vec![worker_executable.display().to_string()];
    if !dedicated_worker {
        worker_args.push("--worker".to_string());
    }
    worker_args.extend(["backup".to_string(), "--due-only".to_string()]);
    let log_dir = paths.schedule_dir.join("logs");
    let _ = fs::create_dir_all(&log_dir);

    match platform.as_str() {
        "macos" => {
            macos::macos_schedule_plan(&label, &worker_executable, &worker_args, &log_dir, params)
        }
        "windows" => {
            windows::windows_schedule_plan(&label, &worker_executable, &worker_args, params)
        }
        _ => linux::linux_schedule_plan(&label, &worker_executable, &worker_args, params),
    }
}

fn scheduled_worker_executable(desktop_executable: &Path) -> Result<PathBuf> {
    scheduled_worker_executable_with_policy(desktop_executable, !cfg!(debug_assertions))
}

fn scheduled_worker_executable_with_policy(
    desktop_executable: &Path,
    require_sidecar_for_installed_binary: bool,
) -> Result<PathBuf> {
    if is_dedicated_worker(desktop_executable) {
        return Ok(desktop_executable.to_path_buf());
    }
    let extension = desktop_executable.extension().and_then(|value| value.to_str());
    let candidate_name = if extension.is_some_and(|value| value.eq_ignore_ascii_case("exe")) {
        "pathkeep-worker.exe"
    } else {
        "pathkeep-worker"
    };
    let candidate = desktop_executable.with_file_name(candidate_name);
    if candidate.is_file() {
        return Ok(candidate);
    }
    if enclosing_app_bundle(desktop_executable).is_some() {
        if desktop_executable.is_file() {
            anyhow::bail!(
                "the installed PathKeep bundle is missing its scheduled-backup worker sidecar: {}",
                candidate.display()
            );
        }
        // Pure preview tests may model a bundle path that is not present on
        // the current host. Preserve the production path shape for review.
        return Ok(candidate);
    }
    if require_sidecar_for_installed_binary && desktop_executable.is_file() {
        anyhow::bail!(
            "the installed PathKeep application is missing its scheduled-backup worker sidecar: {}",
            candidate.display()
        );
    }
    // Developer and legacy CLI builds do not necessarily stage the sidecar
    // beside the desktop executable. Keep their explicit worker mode while
    // production packages fail closed above.
    Ok(desktop_executable.to_path_buf())
}

fn is_dedicated_worker(path: &Path) -> bool {
    path.file_stem().and_then(|value| value.to_str()) == Some("pathkeep-worker")
}

/// Returns the innermost `.app` directory containing `path`, if any.
pub(crate) fn enclosing_app_bundle(path: &Path) -> Option<&Path> {
    path.ancestors()
        .skip(1)
        .find(|ancestor| ancestor.extension().and_then(|value| value.to_str()) == Some("app"))
}

/// Applies a previously previewed native schedule plan when the platform supports it.
pub fn apply_schedule(plan: &SchedulePlan, paths: &ProjectPaths) -> Result<ApplyResult> {
    if plan.platform == "windows" {
        return windows::apply_windows_schedule(&SchedulerHost::resolve("windows")?, plan, paths);
    }
    if plan.platform != "macos" {
        return Ok(unsupported_action(plan, "Apply"));
    }
    macos::apply_macos_schedule(&SchedulerHost::resolve("macos")?, plan, paths)
}

/// Removes a previously applied native schedule plan when the platform supports it.
pub fn remove_schedule(plan: &SchedulePlan, paths: &ProjectPaths) -> Result<ApplyResult> {
    if plan.platform == "windows" {
        return windows::remove_windows_schedule(&SchedulerHost::resolve("windows")?, plan, paths);
    }
    if plan.platform != "macos" {
        return Ok(unsupported_action(plan, "Remove"));
    }
    macos::remove_macos_schedule(&SchedulerHost::resolve("macos")?, plan, paths)
}

/// Repairs user-confirmed scheduler problems that PathKeep knows how to fix.
pub fn repair_schedule(plan: &SchedulePlan, paths: &ProjectPaths) -> Result<ApplyResult> {
    if plan.platform != "macos" {
        return Ok(unsupported_action(plan, "Repair"));
    }
    macos::repair_macos_schedule(&SchedulerHost::resolve("macos")?, plan, paths)
}

/// Reports install/due-state information for the native scheduler plan.
pub fn schedule_status(
    platform: Option<&str>,
    executable_path: &Path,
    paths: &ProjectPaths,
    params: &ScheduleParameters,
) -> Result<ScheduleStatus> {
    let plan = preview_schedule(platform, executable_path, paths, params)?;
    let mut status = ScheduleStatus {
        platform: plan.platform.clone(),
        label: plan.label.clone(),
        due_after_hours: params.due_after_hours,
        check_interval_hours: params.check_interval_hours,
        apply_supported: plan.apply_supported,
        install_state: if plan.platform == "macos" || plan.platform == "windows" {
            "not-installed".to_string()
        } else {
            "manual-review".to_string()
        },
        manual_step_details: plan.manual_step_details.clone(),
        manual_steps: plan.manual_steps.clone(),
        audit_path: audit::latest_schedule_audit_path(paths),
        last_action: audit::latest_schedule_action(paths),
        checked_at: Some(Utc::now().to_rfc3339()),
        ..ScheduleStatus::default()
    };

    if plan.platform == "windows" {
        return windows::windows_schedule_status(
            &SchedulerHost::resolve("windows")?,
            &plan,
            status,
        );
    }

    if plan.platform != "macos" {
        // `warnings` stays as the diagnostic transcript in the payload; the
        // localized `issues` channel is what the UI actually renders.
        status.warnings.push(
            "Automatic install-status detection is only implemented on macOS in v1. Use the manual verification steps for this platform.".to_string(),
        );
        status.issues.push(ScheduleIssue {
            code: "status-detection-unavailable".to_string(),
            severity: "warning".to_string(),
            title_key: "schedule.issueStatusDetectionUnavailableTitle".to_string(),
            detail_key: "schedule.issueStatusDetectionUnavailableDetail".to_string(),
            consequence_key: "schedule.issueStatusDetectionUnavailableConsequence".to_string(),
            evidence: vec![plan.platform.clone()],
            repair_action: None,
            dismissible: true,
        });
        return Ok(status);
    }
    macos::macos_schedule_status(&SchedulerHost::resolve("macos")?, &plan, status)
}

fn unsupported_action(plan: &SchedulePlan, action: &str) -> ApplyResult {
    ApplyResult {
        applied: false,
        platform: plan.platform.clone(),
        files: Vec::new(),
        audit_path: None,
        message: format!(
            "{action} is only implemented on macOS and Windows in v1. Use the Manual steps on this platform."
        ),
        step_results: Vec::new(),
    }
}

pub(super) fn ensure_parent_dir(path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    Ok(())
}

pub(super) fn interval_minutes_from_hours(hours: f64) -> u64 {
    let minutes = (hours * 60.0).round();
    if minutes.is_finite() && minutes >= 1.0 && minutes <= u64::MAX as f64 {
        minutes as u64
    } else {
        1
    }
}

pub(super) fn interval_seconds_from_hours(hours: f64) -> u64 {
    interval_minutes_from_hours(hours) * 60
}

pub(super) fn format_interval_label(minutes: u64) -> String {
    if minutes % 60 == 0 {
        let hours = minutes / 60;
        if hours == 1 { "1 hour".to_string() } else { format!("{hours} hours") }
    } else if minutes == 1 {
        "1 minute".to_string()
    } else {
        format!("{minutes} minutes")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{
        TEST_SCHEDULE_LABEL_ENV, TEST_WINDOWS_USER_ID_ENV, env_lock, restore_env_var,
    };
    use tempfile::tempdir;

    fn sample_paths(root: &Path) -> ProjectPaths {
        vault_core::config::project_paths_with_root(root)
    }

    #[test]
    fn preview_schedule_supports_all_platform_variants() {
        let _guard = env_lock().lock().expect("env lock");
        let original_schedule_label = std::env::var_os(TEST_SCHEDULE_LABEL_ENV);
        unsafe {
            std::env::set_var(TEST_SCHEDULE_LABEL_ENV, "com.yi-ting.pathkeep.tests");
        }

        let dir = tempdir().expect("tempdir");
        let paths = sample_paths(dir.path());
        let params = ScheduleParameters { due_after_hours: 72.0, check_interval_hours: 6.0 };

        let mac =
            preview_schedule(Some("macos"), Path::new("/tmp/chb"), &paths, &params).expect("mac");
        let windows = preview_schedule(Some("windows"), Path::new("C:/chb.exe"), &paths, &params)
            .expect("windows");
        let linux = preview_schedule(Some("linux"), Path::new("/usr/bin/chb"), &paths, &params)
            .expect("linux");

        restore_env_var(TEST_SCHEDULE_LABEL_ENV, original_schedule_label.as_deref());

        assert_eq!(mac.platform, "macos");
        assert_eq!(windows.platform, "windows");
        assert_eq!(linux.platform, "linux");
        assert!(!mac.generated_files.is_empty());
        assert!(!windows.generated_files.is_empty());
        assert!(!linux.generated_files.is_empty());
        assert!(mac.apply_supported);
        assert!(windows.apply_supported);
        assert!(!linux.apply_supported);
        assert!(windows.generated_files[0].contents.starts_with("<Task"));
        assert!(!windows.generated_files[0].contents.contains("<?xml"));
        assert!(!windows.generated_files[0].contents.contains("encoding="));
        assert!(linux.generated_files[1].contents.contains("Persistent=true"));
        assert!(linux.generated_files[1].contents.contains("OnCalendar=*-*-* 00/6:00:00"));
        assert!(!linux.generated_files[1].contents.contains("OnUnitActiveSec"));
    }

    #[test]
    fn bundled_desktop_schedule_targets_the_dedicated_worker_sidecar() {
        let _guard = env_lock().lock().expect("env lock");
        let original_schedule_label = std::env::var_os(TEST_SCHEDULE_LABEL_ENV);
        unsafe {
            std::env::set_var(TEST_SCHEDULE_LABEL_ENV, "com.yi-ting.pathkeep.tests");
        }
        let dir = tempdir().expect("tempdir");
        let paths = sample_paths(dir.path());
        let params = ScheduleParameters { due_after_hours: 72.0, check_interval_hours: 6.0 };
        let desktop_executable =
            dir.path().join("PathKeep-Test.app/Contents/MacOS/pathkeep-desktop");
        let expected_worker = dir.path().join("PathKeep-Test.app/Contents/MacOS/pathkeep-worker");
        let plan = preview_schedule(Some("macos"), &desktop_executable, &paths, &params)
            .expect("bundled schedule");

        restore_env_var(TEST_SCHEDULE_LABEL_ENV, original_schedule_label.as_deref());

        assert_eq!(plan.executable_path, expected_worker.display().to_string());
        let plist = &plan.generated_files[0].contents;
        assert!(plist.contains(&expected_worker.display().to_string()));
        assert!(!plist.contains("/usr/bin/open"));
        assert!(!plist.contains("--worker"));
        assert!(plist.contains("<string>backup</string>"));
        assert!(plist.contains("<string>--due-only</string>"));
    }

    #[test]
    fn windows_schedule_xml_scopes_logon_trigger_and_principal_to_current_user() {
        let _guard = env_lock().lock().expect("env lock");
        let original_schedule_label = std::env::var_os(TEST_SCHEDULE_LABEL_ENV);
        let original_windows_user_id = std::env::var_os(TEST_WINDOWS_USER_ID_ENV);
        unsafe {
            std::env::set_var(TEST_SCHEDULE_LABEL_ENV, "com.yi-ting.pathkeep.tests");
            std::env::set_var(TEST_WINDOWS_USER_ID_ENV, "PATHKEEPTEST\\backup-user");
        }

        let dir = tempdir().expect("tempdir");
        let paths = sample_paths(dir.path());
        let params = ScheduleParameters { due_after_hours: 72.0, check_interval_hours: 6.0 };
        let windows = preview_schedule(
            Some("windows"),
            Path::new("C:/PathKeep/pathkeep.exe"),
            &paths,
            &params,
        )
        .expect("windows");

        restore_env_var(TEST_SCHEDULE_LABEL_ENV, original_schedule_label.as_deref());
        restore_env_var(TEST_WINDOWS_USER_ID_ENV, original_windows_user_id.as_deref());

        let xml = &windows.generated_files[0].contents;
        assert!(xml.contains("<LogonTrigger>\n      <UserId>PATHKEEPTEST\\backup-user</UserId>"));
        assert!(xml.contains(
            "<Principal id=\"Author\">\n      <UserId>PATHKEEPTEST\\backup-user</UserId>"
        ));
        assert!(xml.contains("<LogonType>InteractiveToken</LogonType>"));
        assert!(xml.contains("<RunLevel>LeastPrivilege</RunLevel>"));
        assert!(!xml.contains("<LogonTrigger />"));
        assert!(!xml.contains("encoding="));
    }
}
