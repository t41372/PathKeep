//! Native scheduler adapters.
//!
//! PathKeep's schedule contract is explicit preview/manual/apply/remove/verify.
//! This facade keeps the public worker-facing API stable while platform-specific
//! behavior lives in focused owners under `scheduler/`.

mod audit;
mod linux;
mod macos;
mod windows;

use crate::{host_capability::current_platform_name, test_support::schedule_label};
use anyhow::Result;
use chrono::Utc;
use serde::Serialize;
use std::{fs, path::Path};
use vault_core::{
    ProjectPaths,
    models::{ApplyResult, SchedulePlan, ScheduleStatus},
};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
/// Runtime parameters that shape the generated backup schedule plan.
pub struct ScheduleParameters {
    pub due_after_hours: f64,
    pub check_interval_hours: f64,
}

#[cfg(any(test, coverage))]
const TEST_SCHTASKS_MODE_ENV: &str = "PATHKEEP_TEST_SCHTASKS_MODE";
#[cfg(any(test, coverage))]
const TEST_SCHTASKS_QUERY_XML_ENV: &str = "PATHKEEP_TEST_SCHTASKS_QUERY_XML";
const LEGACY_MACOS_SCHEDULE_LABELS: &[&str] =
    &["dev.codex.pathkeep.backup", "dev.codex.browser-history-backup.backup"];

/// Builds a preview-only native schedule plan for the requested platform.
pub fn preview_schedule(
    platform: Option<&str>,
    executable_path: &Path,
    paths: &ProjectPaths,
    params: &ScheduleParameters,
) -> Result<SchedulePlan> {
    let platform = platform.map(ToOwned::to_owned).unwrap_or_else(current_platform_name);
    let label = schedule_label();
    let worker_args = vec![
        executable_path.display().to_string(),
        "--worker".to_string(),
        "backup".to_string(),
        "--due-only".to_string(),
    ];
    let log_dir = paths.schedule_dir.join("logs");
    let _ = fs::create_dir_all(&log_dir);

    match platform.as_str() {
        "macos" => {
            macos::macos_schedule_plan(&label, executable_path, &worker_args, &log_dir, params)
        }
        "windows" => windows::windows_schedule_plan(&label, executable_path, &worker_args, params),
        _ => linux::linux_schedule_plan(&label, executable_path, &worker_args, params),
    }
}

/// Applies a previously previewed native schedule plan when the platform supports it.
pub fn apply_schedule(plan: &SchedulePlan, paths: &ProjectPaths) -> Result<ApplyResult> {
    if plan.platform == "windows" {
        return windows::apply_windows_schedule(plan, paths);
    }
    if plan.platform != "macos" {
        return Ok(unsupported_action(plan, "Apply"));
    }
    macos::apply_macos_schedule(plan, paths)
}

/// Removes a previously applied native schedule plan when the platform supports it.
pub fn remove_schedule(plan: &SchedulePlan, paths: &ProjectPaths) -> Result<ApplyResult> {
    if plan.platform == "windows" {
        return windows::remove_windows_schedule(plan, paths);
    }
    if plan.platform != "macos" {
        return Ok(unsupported_action(plan, "Remove"));
    }
    macos::remove_macos_schedule(plan, paths)
}

/// Repairs user-confirmed scheduler problems that PathKeep knows how to fix.
pub fn repair_schedule(plan: &SchedulePlan, paths: &ProjectPaths) -> Result<ApplyResult> {
    if plan.platform != "macos" {
        return Ok(unsupported_action(plan, "Repair"));
    }
    macos::repair_macos_schedule(plan, paths)
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
        checked_at: Some(Utc::now().to_rfc3339()),
        ..ScheduleStatus::default()
    };

    if plan.platform == "windows" {
        return windows::windows_schedule_status(&plan, status);
    }

    if plan.platform != "macos" {
        status.warnings.push(
            "Automatic install-status detection is only implemented on macOS in v1. Use the manual verification steps for this platform.".to_string(),
        );
        return Ok(status);
    }
    macos::macos_schedule_status(&plan, status)
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
        TEST_LAUNCH_AGENTS_DIR_ENV, TEST_LAUNCHCTL_SUCCESS_ENV, TEST_SCHEDULE_LABEL_ENV,
        TEST_WINDOWS_USER_ID_ENV, env_lock, restore_env_var,
    };
    use std::path::PathBuf;
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
    fn preview_schedule_supports_minute_level_intervals() {
        let _guard = env_lock().lock().expect("env lock");
        let original_schedule_label = std::env::var_os(TEST_SCHEDULE_LABEL_ENV);
        unsafe {
            std::env::set_var(TEST_SCHEDULE_LABEL_ENV, "com.yi-ting.pathkeep.tests");
        }

        let dir = tempdir().expect("tempdir");
        let paths = sample_paths(dir.path());
        let params = ScheduleParameters { due_after_hours: 1.5, check_interval_hours: 1.5 };

        let mac =
            preview_schedule(Some("macos"), Path::new("/tmp/chb"), &paths, &params).expect("mac");
        let windows = preview_schedule(Some("windows"), Path::new("C:/chb.exe"), &paths, &params)
            .expect("windows");
        let linux = preview_schedule(Some("linux"), Path::new("/usr/bin/chb"), &paths, &params)
            .expect("linux");

        restore_env_var(TEST_SCHEDULE_LABEL_ENV, original_schedule_label.as_deref());

        assert!(mac.generated_files[0].contents.contains("<integer>5400</integer>"));
        assert!(mac.generated_files[0].purpose.contains("90 minutes"));
        assert!(windows.generated_files[0].contents.contains("<Interval>PT1H30M</Interval>"));
        assert!(linux.generated_files[1].contents.contains("OnCalendar=*-*-* *:00/30:00"));
        assert!(linux.generated_files[1].purpose.contains("wakes every 30 minutes"));
    }

    #[test]
    fn preview_schedule_covers_sub_hour_and_calendar_fallback_intervals() {
        let _guard = env_lock().lock().expect("env lock");
        let original_schedule_label = std::env::var_os(TEST_SCHEDULE_LABEL_ENV);
        unsafe {
            std::env::set_var(TEST_SCHEDULE_LABEL_ENV, "com.yi-ting.pathkeep.tests");
        }

        let dir = tempdir().expect("tempdir");
        let paths = sample_paths(dir.path());
        let half_hour = ScheduleParameters { due_after_hours: 0.5, check_interval_hours: 0.5 };
        let one_hour = ScheduleParameters { due_after_hours: 1.0, check_interval_hours: 1.0 };
        let seven_minutes =
            ScheduleParameters { due_after_hours: 7.0 / 60.0, check_interval_hours: 7.0 / 60.0 };

        let mac = preview_schedule(Some("macos"), Path::new("/tmp/chb"), &paths, &half_hour)
            .expect("mac");
        let windows =
            preview_schedule(Some("windows"), Path::new("C:/chb.exe"), &paths, &half_hour)
                .expect("windows");
        let linux_half_hour =
            preview_schedule(Some("linux"), Path::new("/usr/bin/chb"), &paths, &half_hour)
                .expect("linux half hour");
        let linux_one_hour =
            preview_schedule(Some("linux"), Path::new("/usr/bin/chb"), &paths, &one_hour)
                .expect("linux one hour");
        let linux_seven_minutes =
            preview_schedule(Some("linux"), Path::new("/usr/bin/chb"), &paths, &seven_minutes)
                .expect("linux seven minutes");

        restore_env_var(TEST_SCHEDULE_LABEL_ENV, original_schedule_label.as_deref());

        assert!(mac.generated_files[0].contents.contains("<integer>1800</integer>"));
        assert!(windows.generated_files[0].contents.contains("<Interval>PT30M</Interval>"));
        assert!(
            linux_half_hour.generated_files[1].contents.contains("OnCalendar=*-*-* *:00/30:00")
        );
        assert!(linux_one_hour.generated_files[1].contents.contains("OnCalendar=*-*-* *:00:00"));
        assert!(
            linux_seven_minutes.generated_files[1].contents.contains("OnCalendar=*-*-* *:*:00")
        );
        assert!(linux_seven_minutes.generated_files[1].purpose.contains("wakes every 1 minute"));
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

    #[test]
    fn interval_helpers_clamp_invalid_values_and_format_units() {
        assert_eq!(interval_minutes_from_hours(f64::NAN), 1);
        assert_eq!(interval_seconds_from_hours(0.5), 1800);
        assert_eq!(format_interval_label(1), "1 minute");
        assert_eq!(format_interval_label(60), "1 hour");
        assert_eq!(format_interval_label(90), "90 minutes");
    }

    #[test]
    fn preview_schedule_uses_current_platform_when_unspecified() {
        let dir = tempdir().expect("tempdir");
        let paths = sample_paths(dir.path());
        let params = ScheduleParameters { due_after_hours: 72.0, check_interval_hours: 6.0 };
        let preview = preview_schedule(None, Path::new("/tmp/bhb"), &paths, &params)
            .expect("default preview");
        assert_eq!(preview.platform, current_platform_name());
        assert!(!preview.manual_steps.is_empty());
    }

    #[test]
    fn macos_schedule_status_detects_installed_content() {
        let _guard = env_lock().lock().expect("env lock");
        let dir = tempdir().expect("tempdir");
        let launch_agents_dir = dir.path().join("LaunchAgents");
        let original_launch_agents = std::env::var_os(TEST_LAUNCH_AGENTS_DIR_ENV);
        let original_schedule_label = std::env::var_os(TEST_SCHEDULE_LABEL_ENV);
        let original_loaded_labels = std::env::var_os(macos::loaded_labels_env_name());
        unsafe {
            std::env::set_var(TEST_LAUNCH_AGENTS_DIR_ENV, &launch_agents_dir);
            std::env::set_var(TEST_SCHEDULE_LABEL_ENV, "com.yi-ting.pathkeep.tests");
            std::env::set_var(macos::loaded_labels_env_name(), "com.yi-ting.pathkeep.tests");
        }

        let paths = sample_paths(dir.path());
        let params = ScheduleParameters { due_after_hours: 72.0, check_interval_hours: 6.0 };
        let plan =
            preview_schedule(Some("macos"), Path::new("/tmp/chb"), &paths, &params).expect("plan");
        fs::create_dir_all(&launch_agents_dir).expect("create launch agents dir");
        fs::write(
            launch_agents_dir.join("com.yi-ting.pathkeep.tests.plist"),
            &plan.generated_files[0].contents,
        )
        .expect("write current plist");

        let status =
            schedule_status(Some("macos"), Path::new("/tmp/chb"), &paths, &params).expect("status");

        restore_env_var(TEST_LAUNCH_AGENTS_DIR_ENV, original_launch_agents.as_deref());
        restore_env_var(TEST_SCHEDULE_LABEL_ENV, original_schedule_label.as_deref());
        restore_env_var(macos::loaded_labels_env_name(), original_loaded_labels.as_deref());

        assert_eq!(status.install_state, "installed");
        assert_eq!(status.detected_files.len(), 1);
    }

    #[test]
    fn macos_schedule_status_reports_installed_file_that_is_not_loaded() {
        let _guard = env_lock().lock().expect("env lock");
        let dir = tempdir().expect("tempdir");
        let launch_agents_dir = dir.path().join("LaunchAgents");
        let original_launch_agents = std::env::var_os(TEST_LAUNCH_AGENTS_DIR_ENV);
        let original_schedule_label = std::env::var_os(TEST_SCHEDULE_LABEL_ENV);
        let original_loaded_labels = std::env::var_os(macos::loaded_labels_env_name());
        unsafe {
            std::env::set_var(TEST_LAUNCH_AGENTS_DIR_ENV, &launch_agents_dir);
            std::env::set_var(TEST_SCHEDULE_LABEL_ENV, "com.yi-ting.pathkeep.tests");
            std::env::remove_var(macos::loaded_labels_env_name());
        }

        let paths = sample_paths(dir.path());
        let params = ScheduleParameters { due_after_hours: 72.0, check_interval_hours: 6.0 };
        let plan =
            preview_schedule(Some("macos"), Path::new("/tmp/chb"), &paths, &params).expect("plan");
        fs::create_dir_all(&launch_agents_dir).expect("create launch agents dir");
        fs::write(
            launch_agents_dir.join("com.yi-ting.pathkeep.tests.plist"),
            &plan.generated_files[0].contents,
        )
        .expect("write current plist");

        let status =
            schedule_status(Some("macos"), Path::new("/tmp/chb"), &paths, &params).expect("status");

        restore_env_var(TEST_LAUNCH_AGENTS_DIR_ENV, original_launch_agents.as_deref());
        restore_env_var(TEST_SCHEDULE_LABEL_ENV, original_schedule_label.as_deref());
        restore_env_var(macos::loaded_labels_env_name(), original_loaded_labels.as_deref());

        assert_eq!(status.install_state, "permission-warning");
        assert!(status.issues.iter().any(|issue| issue.code == "macos-launch-agent-not-loaded"));
    }

    #[test]
    fn macos_schedule_status_reports_loaded_agent_with_missing_plist() {
        let _guard = env_lock().lock().expect("env lock");
        let dir = tempdir().expect("tempdir");
        let fallback_launch_agents = std::env::temp_dir().join("pathkeep-launch-agents");
        let original_launch_agents = std::env::var_os(TEST_LAUNCH_AGENTS_DIR_ENV);
        let original_schedule_label = std::env::var_os(TEST_SCHEDULE_LABEL_ENV);
        let original_loaded_labels = std::env::var_os(macos::loaded_labels_env_name());
        unsafe {
            std::env::remove_var(TEST_LAUNCH_AGENTS_DIR_ENV);
            std::env::set_var(TEST_SCHEDULE_LABEL_ENV, "com.yi-ting.pathkeep.tests");
            std::env::set_var(macos::loaded_labels_env_name(), "com.yi-ting.pathkeep.tests");
        }
        let _ = fs::remove_dir_all(&fallback_launch_agents);

        let paths = sample_paths(dir.path());
        let params = ScheduleParameters { due_after_hours: 72.0, check_interval_hours: 6.0 };
        let status =
            schedule_status(Some("macos"), Path::new("/tmp/chb"), &paths, &params).expect("status");

        restore_env_var(TEST_LAUNCH_AGENTS_DIR_ENV, original_launch_agents.as_deref());
        restore_env_var(TEST_SCHEDULE_LABEL_ENV, original_schedule_label.as_deref());
        restore_env_var(macos::loaded_labels_env_name(), original_loaded_labels.as_deref());
        let _ = fs::remove_dir_all(&fallback_launch_agents);

        assert_eq!(status.install_state, "permission-warning");
        assert!(status.issues.iter().any(|issue| {
            issue.code == "macos-plist-missing-loaded" && issue.severity == "error"
        }));
        assert!(status.verification_checks.iter().any(|check| {
            check.key == "macos-plist-content"
                && check.status == "error"
                && check.detail_key == "schedule.verifyMacosPlistMissingLoaded"
        }));
    }

    #[test]
    fn macos_schedule_status_detects_mismatch() {
        let _guard = env_lock().lock().expect("env lock");
        let dir = tempdir().expect("tempdir");
        let launch_agents_dir = dir.path().join("LaunchAgents");
        let original_launch_agents = std::env::var_os(TEST_LAUNCH_AGENTS_DIR_ENV);
        let original_schedule_label = std::env::var_os(TEST_SCHEDULE_LABEL_ENV);
        unsafe {
            std::env::set_var(TEST_LAUNCH_AGENTS_DIR_ENV, &launch_agents_dir);
            std::env::set_var(TEST_SCHEDULE_LABEL_ENV, "com.yi-ting.pathkeep.tests");
        }

        let paths = sample_paths(dir.path());
        let params = ScheduleParameters { due_after_hours: 72.0, check_interval_hours: 6.0 };
        fs::create_dir_all(&launch_agents_dir).expect("create launch agents dir");
        fs::write(
            launch_agents_dir.join("com.yi-ting.pathkeep.tests.plist"),
            "<plist>outdated</plist>",
        )
        .expect("write mismatched plist");

        let status =
            schedule_status(Some("macos"), Path::new("/tmp/chb"), &paths, &params).expect("status");

        restore_env_var(TEST_LAUNCH_AGENTS_DIR_ENV, original_launch_agents.as_deref());
        restore_env_var(TEST_SCHEDULE_LABEL_ENV, original_schedule_label.as_deref());

        assert_eq!(status.install_state, "mismatch");
        assert!(status.warnings.iter().any(|warning| warning.contains("no longer matches")));
    }

    #[test]
    fn macos_schedule_status_detects_legacy_launch_agent() {
        let _guard = env_lock().lock().expect("env lock");
        let dir = tempdir().expect("tempdir");
        let launch_agents_dir = dir.path().join("LaunchAgents");
        let original_launch_agents = std::env::var_os(TEST_LAUNCH_AGENTS_DIR_ENV);
        let original_schedule_label = std::env::var_os(TEST_SCHEDULE_LABEL_ENV);
        unsafe {
            std::env::set_var(TEST_LAUNCH_AGENTS_DIR_ENV, &launch_agents_dir);
            std::env::set_var(TEST_SCHEDULE_LABEL_ENV, "com.yi-ting.pathkeep.tests");
        }

        let paths = sample_paths(dir.path());
        let params = ScheduleParameters { due_after_hours: 72.0, check_interval_hours: 6.0 };
        fs::create_dir_all(&launch_agents_dir).expect("create launch agents dir");
        let legacy_path = launch_agents_dir.join("dev.codex.pathkeep.backup.plist");
        fs::write(&legacy_path, "<plist>legacy</plist>").expect("write legacy plist");

        let status =
            schedule_status(Some("macos"), Path::new("/tmp/chb"), &paths, &params).expect("status");

        restore_env_var(TEST_LAUNCH_AGENTS_DIR_ENV, original_launch_agents.as_deref());
        restore_env_var(TEST_SCHEDULE_LABEL_ENV, original_schedule_label.as_deref());

        assert_eq!(status.install_state, "legacy-install-detected");
        assert!(status.detected_files.contains(&legacy_path.display().to_string()));
        assert!(status.warnings.iter().any(|warning| warning.contains("legacy PathKeep")));
    }

    #[test]
    fn macos_repair_schedule_removes_legacy_launch_agent_and_writes_audit_report() {
        let _guard = env_lock().lock().expect("env lock");
        let dir = tempdir().expect("tempdir");
        let launch_agents_dir = dir.path().join("LaunchAgents");
        let original_launch_agents = std::env::var_os(TEST_LAUNCH_AGENTS_DIR_ENV);
        let original_launchctl_success = std::env::var_os(TEST_LAUNCHCTL_SUCCESS_ENV);
        let original_schedule_label = std::env::var_os(TEST_SCHEDULE_LABEL_ENV);
        let original_loaded_labels = std::env::var_os(macos::loaded_labels_env_name());
        unsafe {
            std::env::set_var(TEST_LAUNCH_AGENTS_DIR_ENV, &launch_agents_dir);
            std::env::set_var(TEST_LAUNCHCTL_SUCCESS_ENV, "1");
            std::env::set_var(TEST_SCHEDULE_LABEL_ENV, "com.yi-ting.pathkeep.tests");
        }

        let paths = sample_paths(dir.path());
        let params = ScheduleParameters { due_after_hours: 72.0, check_interval_hours: 6.0 };
        let plan =
            preview_schedule(Some("macos"), Path::new("/tmp/chb"), &paths, &params).expect("plan");
        fs::create_dir_all(&launch_agents_dir).expect("create launch agents dir");
        let legacy_path = launch_agents_dir.join("dev.codex.pathkeep.backup.plist");
        fs::write(&legacy_path, "<plist>legacy</plist>").expect("write legacy plist");

        let result = repair_schedule(&plan, &paths).expect("repair macos plan");

        restore_env_var(TEST_LAUNCH_AGENTS_DIR_ENV, original_launch_agents.as_deref());
        restore_env_var(TEST_LAUNCHCTL_SUCCESS_ENV, original_launchctl_success.as_deref());
        restore_env_var(TEST_SCHEDULE_LABEL_ENV, original_schedule_label.as_deref());
        restore_env_var(macos::loaded_labels_env_name(), original_loaded_labels.as_deref());

        assert!(result.applied);
        assert_eq!(result.files, vec![legacy_path.display().to_string()]);
        assert!(!legacy_path.exists());
        assert!(result.message.contains("Legacy PathKeep LaunchAgent entries were removed"));
        let audit_path = result.audit_path.as_deref().expect("audit path");
        assert!(Path::new(audit_path).exists());
        let audit_json: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(audit_path).expect("read audit"))
                .expect("parse audit json");
        assert_eq!(audit_json["action"], "repair");
        assert!(result.step_results.iter().any(|check| {
            check.key == "macos-repair-legacy"
                && check.detail_key == "schedule.verifyMacosRepairLegacyOk"
                && check.evidence.iter().any(|line| line.contains("dev.codex.pathkeep.backup"))
        }));
    }

    #[test]
    fn macos_repair_schedule_reports_noop_when_no_legacy_launch_agent_exists() {
        let _guard = env_lock().lock().expect("env lock");
        let dir = tempdir().expect("tempdir");
        let launch_agents_dir = dir.path().join("LaunchAgents");
        let original_launch_agents = std::env::var_os(TEST_LAUNCH_AGENTS_DIR_ENV);
        let original_schedule_label = std::env::var_os(TEST_SCHEDULE_LABEL_ENV);
        unsafe {
            std::env::set_var(TEST_LAUNCH_AGENTS_DIR_ENV, &launch_agents_dir);
            std::env::set_var(TEST_SCHEDULE_LABEL_ENV, "com.yi-ting.pathkeep.tests");
        }

        let paths = sample_paths(dir.path());
        let params = ScheduleParameters { due_after_hours: 72.0, check_interval_hours: 6.0 };
        let plan =
            preview_schedule(Some("macos"), Path::new("/tmp/chb"), &paths, &params).expect("plan");
        let result = repair_schedule(&plan, &paths).expect("repair without legacy entries");

        restore_env_var(TEST_LAUNCH_AGENTS_DIR_ENV, original_launch_agents.as_deref());
        restore_env_var(TEST_SCHEDULE_LABEL_ENV, original_schedule_label.as_deref());

        assert!(!result.applied);
        assert!(result.files.is_empty());
        assert!(result.message.contains("No legacy PathKeep LaunchAgent entries"));
        assert!(Path::new(result.audit_path.as_deref().expect("audit path")).exists());
        assert!(result.step_results.iter().any(|check| {
            check.key == "macos-repair-legacy"
                && check.detail_key == "schedule.verifyMacosRepairLegacyNoop"
                && check.evidence.is_empty()
        }));
    }

    #[test]
    fn macos_repair_schedule_unloads_legacy_agent_without_plist_file() {
        let _guard = env_lock().lock().expect("env lock");
        let dir = tempdir().expect("tempdir");
        let fallback_launch_agents = std::env::temp_dir().join("pathkeep-launch-agents");
        let original_launch_agents = std::env::var_os(TEST_LAUNCH_AGENTS_DIR_ENV);
        let original_launchctl_success = std::env::var_os(TEST_LAUNCHCTL_SUCCESS_ENV);
        let original_schedule_label = std::env::var_os(TEST_SCHEDULE_LABEL_ENV);
        let original_loaded_labels = std::env::var_os(macos::loaded_labels_env_name());
        unsafe {
            std::env::remove_var(TEST_LAUNCH_AGENTS_DIR_ENV);
            std::env::set_var(TEST_LAUNCHCTL_SUCCESS_ENV, "1");
            std::env::set_var(TEST_SCHEDULE_LABEL_ENV, "com.yi-ting.pathkeep.tests");
            std::env::set_var(
                macos::loaded_labels_env_name(),
                "dev.codex.browser-history-backup.backup",
            );
        }
        let _ = fs::remove_dir_all(&fallback_launch_agents);

        let paths = sample_paths(dir.path());
        let params = ScheduleParameters { due_after_hours: 72.0, check_interval_hours: 6.0 };
        let plan =
            preview_schedule(Some("macos"), Path::new("/tmp/chb"), &paths, &params).expect("plan");

        let result = repair_schedule(&plan, &paths).expect("repair loaded legacy");

        restore_env_var(TEST_LAUNCH_AGENTS_DIR_ENV, original_launch_agents.as_deref());
        restore_env_var(TEST_LAUNCHCTL_SUCCESS_ENV, original_launchctl_success.as_deref());
        restore_env_var(TEST_SCHEDULE_LABEL_ENV, original_schedule_label.as_deref());
        restore_env_var(macos::loaded_labels_env_name(), original_loaded_labels.as_deref());
        let _ = fs::remove_dir_all(&fallback_launch_agents);

        assert!(result.applied);
        assert!(result.files.is_empty());
        assert!(result.step_results.iter().any(|check| {
            check.key == "macos-repair-legacy"
                && check.detail_key == "schedule.verifyMacosRepairLegacyOk"
                && check
                    .evidence
                    .iter()
                    .any(|line| line.contains("dev.codex.browser-history-backup"))
        }));
    }

    #[test]
    fn legacy_launch_agent_detected_value_covers_loaded_without_file() {
        let agent = macos::LegacyLaunchAgent {
            label: "dev.codex.browser-history-backup.backup",
            path: PathBuf::from("/tmp/missing-legacy.plist"),
            file_present: false,
            loaded: true,
        };

        assert_eq!(agent.detected_value(), "LaunchAgent:dev.codex.browser-history-backup.backup");
        assert_eq!(agent.summary(), "dev.codex.browser-history-backup.backup (loaded)");
    }

    #[test]
    fn linux_apply_is_manual_only() {
        let dir = tempdir().expect("tempdir");
        let result = apply_schedule(
            &SchedulePlan {
                platform: "linux".to_string(),
                label: "example".to_string(),
                executable_path: "/usr/bin/chb".to_string(),
                generated_files: Vec::new(),
                manual_steps: Vec::new(),
                manual_step_details: Vec::new(),
                apply_commands: Vec::new(),
                rollback_commands: Vec::new(),
                apply_supported: false,
            },
            &sample_paths(dir.path()),
        )
        .expect("apply");

        assert!(!result.applied);
        assert!(result.message.contains("Manual"));
    }

    #[test]
    fn linux_remove_is_manual_only() {
        let dir = tempdir().expect("tempdir");
        let result = remove_schedule(
            &SchedulePlan {
                platform: "linux".to_string(),
                label: "example".to_string(),
                executable_path: "/usr/bin/chb".to_string(),
                generated_files: Vec::new(),
                manual_steps: Vec::new(),
                manual_step_details: Vec::new(),
                apply_commands: Vec::new(),
                rollback_commands: Vec::new(),
                apply_supported: false,
            },
            &sample_paths(dir.path()),
        )
        .expect("remove");

        assert!(!result.applied);
        assert!(result.message.contains("Manual"));
    }

    #[test]
    fn windows_apply_status_and_remove_use_schtasks() {
        let _guard = env_lock().lock().expect("env lock");
        let dir = tempdir().expect("tempdir");
        let original_schedule_label = std::env::var_os(TEST_SCHEDULE_LABEL_ENV);
        let original_schtasks_mode = std::env::var_os(TEST_SCHTASKS_MODE_ENV);
        let original_schtasks_xml = std::env::var_os(TEST_SCHTASKS_QUERY_XML_ENV);
        unsafe {
            std::env::set_var(TEST_SCHEDULE_LABEL_ENV, "com.yi-ting.pathkeep.tests");
            std::env::set_var(TEST_SCHTASKS_MODE_ENV, "success");
        }

        let paths = sample_paths(dir.path());
        let params = ScheduleParameters { due_after_hours: 72.0, check_interval_hours: 6.0 };
        let plan = preview_schedule(
            Some("windows"),
            Path::new("C:/PathKeep/pathkeep.exe"),
            &paths,
            &params,
        )
        .expect("windows plan");
        unsafe {
            std::env::set_var(
                TEST_SCHTASKS_QUERY_XML_ENV,
                format!(
                    "{}\n{}",
                    r#"<?xml version="1.0" encoding="UTF-16"?>"#, plan.generated_files[0].contents
                ),
            );
        }

        let applied = apply_schedule(&plan, &paths).expect("apply windows schedule");
        let status = schedule_status(
            Some("windows"),
            Path::new("C:/PathKeep/pathkeep.exe"),
            &paths,
            &params,
        )
        .expect("windows status");
        let removed = remove_schedule(&plan, &paths).expect("remove windows schedule");

        restore_env_var(TEST_SCHEDULE_LABEL_ENV, original_schedule_label.as_deref());
        restore_env_var(TEST_SCHTASKS_MODE_ENV, original_schtasks_mode.as_deref());
        restore_env_var(TEST_SCHTASKS_QUERY_XML_ENV, original_schtasks_xml.as_deref());

        assert!(applied.applied);
        assert_eq!(applied.files.len(), 1);
        assert!(Path::new(&applied.files[0]).exists());
        assert!(Path::new(applied.audit_path.as_deref().expect("apply audit")).exists());
        assert_eq!(status.install_state, "installed");
        assert_eq!(
            status.detected_files,
            vec!["Task Scheduler:com.yi-ting.pathkeep.tests".to_string()]
        );
        assert!(removed.applied);
        assert!(Path::new(removed.audit_path.as_deref().expect("remove audit")).exists());
    }

    #[test]
    fn windows_apply_and_remove_report_schtasks_failures() {
        let _guard = env_lock().lock().expect("env lock");
        let dir = tempdir().expect("tempdir");
        let original_schedule_label = std::env::var_os(TEST_SCHEDULE_LABEL_ENV);
        let original_schtasks_mode = std::env::var_os(TEST_SCHTASKS_MODE_ENV);
        unsafe {
            std::env::set_var(TEST_SCHEDULE_LABEL_ENV, "com.yi-ting.pathkeep.tests");
            std::env::set_var(TEST_SCHTASKS_MODE_ENV, "fail");
        }

        let paths = sample_paths(dir.path());
        let params = ScheduleParameters { due_after_hours: 72.0, check_interval_hours: 6.0 };
        let plan = preview_schedule(
            Some("windows"),
            Path::new("C:/PathKeep/pathkeep.exe"),
            &paths,
            &params,
        )
        .expect("windows plan");

        let failed_apply = apply_schedule(&plan, &paths).expect("failed apply result");
        let failed_remove = remove_schedule(&plan, &paths).expect("failed remove result");
        unsafe {
            std::env::set_var(TEST_SCHTASKS_MODE_ENV, "denied");
        }
        let denied_apply = apply_schedule(&plan, &paths).expect("denied apply result");
        unsafe {
            std::env::set_var(TEST_SCHTASKS_MODE_ENV, "missing");
        }
        let missing_remove = remove_schedule(&plan, &paths).expect("missing remove result");

        restore_env_var(TEST_SCHEDULE_LABEL_ENV, original_schedule_label.as_deref());
        restore_env_var(TEST_SCHTASKS_MODE_ENV, original_schtasks_mode.as_deref());

        assert!(!failed_apply.applied);
        assert!(failed_apply.message.contains("schtasks /Create did not report success"));
        assert!(!failed_remove.applied);
        assert!(failed_remove.message.contains("schtasks /Delete did not report success"));
        assert!(!denied_apply.applied);
        assert_eq!(denied_apply.message, "schedule.windowsAccessDeniedInstallMessage");
        assert_eq!(
            denied_apply.step_results[0].detail_key,
            "schedule.verifyWindowsRegisterAccessDenied"
        );
        assert!(!missing_remove.applied);
        assert!(missing_remove.message.contains("No installed PathKeep Task Scheduler task"));
    }

    #[test]
    fn windows_schedule_status_reports_missing_mismatch_and_permission_warning() {
        let _guard = env_lock().lock().expect("env lock");
        let dir = tempdir().expect("tempdir");
        let original_schedule_label = std::env::var_os(TEST_SCHEDULE_LABEL_ENV);
        let original_schtasks_mode = std::env::var_os(TEST_SCHTASKS_MODE_ENV);
        unsafe {
            std::env::set_var(TEST_SCHEDULE_LABEL_ENV, "com.yi-ting.pathkeep.tests");
        }

        let paths = sample_paths(dir.path());
        let params = ScheduleParameters { due_after_hours: 72.0, check_interval_hours: 6.0 };
        unsafe {
            std::env::set_var(TEST_SCHTASKS_MODE_ENV, "missing");
        }
        let missing = schedule_status(
            Some("windows"),
            Path::new("C:/PathKeep/pathkeep.exe"),
            &paths,
            &params,
        )
        .expect("missing status");
        unsafe {
            std::env::set_var(TEST_SCHTASKS_MODE_ENV, "mismatch");
        }
        let mismatch = schedule_status(
            Some("windows"),
            Path::new("C:/PathKeep/pathkeep.exe"),
            &paths,
            &params,
        )
        .expect("mismatch status");
        unsafe {
            std::env::set_var(TEST_SCHTASKS_MODE_ENV, "denied");
        }
        let denied = schedule_status(
            Some("windows"),
            Path::new("C:/PathKeep/pathkeep.exe"),
            &paths,
            &params,
        )
        .expect("denied status");
        unsafe {
            std::env::set_var(TEST_SCHTASKS_MODE_ENV, "fail");
        }
        let failed = schedule_status(
            Some("windows"),
            Path::new("C:/PathKeep/pathkeep.exe"),
            &paths,
            &params,
        )
        .expect("failed status");

        restore_env_var(TEST_SCHEDULE_LABEL_ENV, original_schedule_label.as_deref());
        restore_env_var(TEST_SCHTASKS_MODE_ENV, original_schtasks_mode.as_deref());

        assert_eq!(missing.install_state, "not-installed");
        assert_eq!(mismatch.install_state, "mismatch");
        assert!(mismatch.warnings.iter().any(|warning| warning.contains("no longer matches")));
        assert_eq!(denied.install_state, "permission-warning");
        assert!(denied.warnings.iter().any(|warning| warning.contains("could not inspect")));
        assert_eq!(denied.issues[0].code, "windows-task-access-denied");
        assert_eq!(denied.issues[0].detail_key, "schedule.issueWindowsAccessDeniedDetail");
        assert_eq!(failed.install_state, "permission-warning");
        assert_eq!(failed.issues[0].code, "windows-task-inspection-failed");
        assert_eq!(failed.issues[0].detail_key, "schedule.issueWindowsInspectionFailedDetail");
    }

    #[test]
    fn macos_apply_schedule_writes_files_and_audit_report() {
        let _guard = env_lock().lock().expect("env lock");
        let dir = tempdir().expect("tempdir");
        let launch_agents_dir = dir.path().join("LaunchAgents");
        let original_launch_agents = std::env::var_os(TEST_LAUNCH_AGENTS_DIR_ENV);
        let original_launchctl_success = std::env::var_os(TEST_LAUNCHCTL_SUCCESS_ENV);
        let original_schedule_label = std::env::var_os(TEST_SCHEDULE_LABEL_ENV);
        let original_loaded_labels = std::env::var_os(macos::loaded_labels_env_name());
        unsafe {
            std::env::set_var(TEST_LAUNCH_AGENTS_DIR_ENV, &launch_agents_dir);
            std::env::set_var(TEST_LAUNCHCTL_SUCCESS_ENV, "1");
            std::env::set_var(TEST_SCHEDULE_LABEL_ENV, "com.yi-ting.pathkeep.tests");
        }

        let paths = sample_paths(dir.path());
        let params = ScheduleParameters { due_after_hours: 72.0, check_interval_hours: 6.0 };
        let plan =
            preview_schedule(Some("macos"), Path::new("/tmp/chb"), &paths, &params).expect("plan");
        let result = apply_schedule(&plan, &paths).expect("apply macos plan");

        restore_env_var(TEST_LAUNCH_AGENTS_DIR_ENV, original_launch_agents.as_deref());
        restore_env_var(TEST_LAUNCHCTL_SUCCESS_ENV, original_launchctl_success.as_deref());
        restore_env_var(TEST_SCHEDULE_LABEL_ENV, original_schedule_label.as_deref());
        restore_env_var(macos::loaded_labels_env_name(), original_loaded_labels.as_deref());

        assert!(result.applied);
        assert_eq!(result.files.len(), 1);
        assert!(Path::new(&result.files[0]).exists());
        assert!(Path::new(result.audit_path.as_deref().expect("audit path")).exists());
    }

    #[test]
    fn macos_apply_schedule_reports_bootstrap_failures_without_erroring() {
        let _guard = env_lock().lock().expect("env lock");
        let dir = tempdir().expect("tempdir");
        let launch_agents_dir = dir.path().join("LaunchAgents");
        let original_launch_agents = std::env::var_os(TEST_LAUNCH_AGENTS_DIR_ENV);
        let original_launchctl_success = std::env::var_os(TEST_LAUNCHCTL_SUCCESS_ENV);
        let original_schedule_label = std::env::var_os(TEST_SCHEDULE_LABEL_ENV);
        let original_loaded_labels = std::env::var_os(macos::loaded_labels_env_name());
        unsafe {
            std::env::set_var(TEST_LAUNCH_AGENTS_DIR_ENV, &launch_agents_dir);
            std::env::set_var(TEST_LAUNCHCTL_SUCCESS_ENV, "0");
            std::env::set_var(TEST_SCHEDULE_LABEL_ENV, "com.yi-ting.pathkeep.tests");
        }

        let paths = sample_paths(dir.path());
        let params = ScheduleParameters { due_after_hours: 72.0, check_interval_hours: 6.0 };
        let plan =
            preview_schedule(Some("macos"), Path::new("/tmp/chb"), &paths, &params).expect("plan");
        let result = apply_schedule(&plan, &paths).expect("apply failing macos plan");

        restore_env_var(TEST_LAUNCH_AGENTS_DIR_ENV, original_launch_agents.as_deref());
        restore_env_var(TEST_LAUNCHCTL_SUCCESS_ENV, original_launchctl_success.as_deref());
        restore_env_var(TEST_SCHEDULE_LABEL_ENV, original_schedule_label.as_deref());
        restore_env_var(macos::loaded_labels_env_name(), original_loaded_labels.as_deref());

        assert!(!result.applied);
        assert!(result.message.contains("did not report success"));
    }

    #[test]
    fn macos_remove_schedule_removes_files_and_writes_audit_report() {
        let _guard = env_lock().lock().expect("env lock");
        let dir = tempdir().expect("tempdir");
        let launch_agents_dir = dir.path().join("LaunchAgents");
        let original_launch_agents = std::env::var_os(TEST_LAUNCH_AGENTS_DIR_ENV);
        let original_launchctl_success = std::env::var_os(TEST_LAUNCHCTL_SUCCESS_ENV);
        let original_schedule_label = std::env::var_os(TEST_SCHEDULE_LABEL_ENV);
        let original_loaded_labels = std::env::var_os(macos::loaded_labels_env_name());
        unsafe {
            std::env::set_var(TEST_LAUNCH_AGENTS_DIR_ENV, &launch_agents_dir);
            std::env::set_var(TEST_LAUNCHCTL_SUCCESS_ENV, "1");
            std::env::set_var(TEST_SCHEDULE_LABEL_ENV, "com.yi-ting.pathkeep.tests");
        }

        let paths = sample_paths(dir.path());
        let params = ScheduleParameters { due_after_hours: 72.0, check_interval_hours: 6.0 };
        let plan =
            preview_schedule(Some("macos"), Path::new("/tmp/chb"), &paths, &params).expect("plan");
        let applied = apply_schedule(&plan, &paths).expect("apply macos plan");
        assert!(applied.applied);

        let removed = remove_schedule(&plan, &paths).expect("remove macos plan");

        restore_env_var(TEST_LAUNCH_AGENTS_DIR_ENV, original_launch_agents.as_deref());
        restore_env_var(TEST_LAUNCHCTL_SUCCESS_ENV, original_launchctl_success.as_deref());
        restore_env_var(TEST_SCHEDULE_LABEL_ENV, original_schedule_label.as_deref());
        restore_env_var(macos::loaded_labels_env_name(), original_loaded_labels.as_deref());

        assert!(removed.applied);
        assert_eq!(removed.files.len(), 1);
        assert!(!Path::new(&removed.files[0]).exists());
        assert!(Path::new(removed.audit_path.as_deref().expect("audit path")).exists());
    }

    #[test]
    fn macos_remove_schedule_reports_noop_when_no_installed_file_exists() {
        let _guard = env_lock().lock().expect("env lock");
        let dir = tempdir().expect("tempdir");
        let launch_agents_dir = dir.path().join("LaunchAgents");
        let original_launch_agents = std::env::var_os(TEST_LAUNCH_AGENTS_DIR_ENV);
        let original_schedule_label = std::env::var_os(TEST_SCHEDULE_LABEL_ENV);
        unsafe {
            std::env::set_var(TEST_LAUNCH_AGENTS_DIR_ENV, &launch_agents_dir);
            std::env::set_var(TEST_SCHEDULE_LABEL_ENV, "com.yi-ting.pathkeep.tests");
        }

        let paths = sample_paths(dir.path());
        let params = ScheduleParameters { due_after_hours: 72.0, check_interval_hours: 6.0 };
        let plan =
            preview_schedule(Some("macos"), Path::new("/tmp/chb"), &paths, &params).expect("plan");
        let result = remove_schedule(&plan, &paths).expect("remove without installed file");

        restore_env_var(TEST_LAUNCH_AGENTS_DIR_ENV, original_launch_agents.as_deref());
        restore_env_var(TEST_SCHEDULE_LABEL_ENV, original_schedule_label.as_deref());

        assert!(!result.applied);
        assert!(result.message.contains("No installed PathKeep LaunchAgent files"));
    }

    #[test]
    fn macos_schedule_status_reports_permission_warning_when_installed_file_is_unreadable() {
        let _guard = env_lock().lock().expect("env lock");
        let dir = tempdir().expect("tempdir");
        let launch_agents_dir = dir.path().join("LaunchAgents");
        fs::create_dir_all(launch_agents_dir.join("com.yi-ting.pathkeep.tests.plist"))
            .expect("directory at plist path");
        let original_launch_agents = std::env::var_os(TEST_LAUNCH_AGENTS_DIR_ENV);
        let original_schedule_label = std::env::var_os(TEST_SCHEDULE_LABEL_ENV);
        unsafe {
            std::env::set_var(TEST_LAUNCH_AGENTS_DIR_ENV, &launch_agents_dir);
            std::env::set_var(TEST_SCHEDULE_LABEL_ENV, "com.yi-ting.pathkeep.tests");
        }

        let paths = sample_paths(dir.path());
        let params = ScheduleParameters { due_after_hours: 72.0, check_interval_hours: 6.0 };
        let status =
            schedule_status(Some("macos"), Path::new("/tmp/chb"), &paths, &params).expect("status");

        restore_env_var(TEST_LAUNCH_AGENTS_DIR_ENV, original_launch_agents.as_deref());
        restore_env_var(TEST_SCHEDULE_LABEL_ENV, original_schedule_label.as_deref());

        assert_eq!(status.install_state, "permission-warning");
        assert!(status.warnings.iter().any(|warning| warning.contains("could not read")));
    }

    #[test]
    fn macos_remove_schedule_rejects_missing_generated_files() {
        let dir = tempdir().expect("tempdir");
        let result = remove_schedule(
            &SchedulePlan {
                platform: "macos".to_string(),
                label: "com.yi-ting.pathkeep.tests".to_string(),
                executable_path: "/tmp/pathkeep".to_string(),
                generated_files: Vec::new(),
                manual_steps: Vec::new(),
                manual_step_details: Vec::new(),
                apply_commands: Vec::new(),
                rollback_commands: Vec::new(),
                apply_supported: true,
            },
            &sample_paths(dir.path()),
        );

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("missing plist file"));
    }

    #[test]
    fn xml_escape_rewrites_reserved_characters() {
        assert_eq!(
            windows::xml_escape("<tag attr=\"1\">&</tag>"),
            "&lt;tag attr=&quot;1&quot;&gt;&amp;&lt;/tag&gt;"
        );
    }
}
