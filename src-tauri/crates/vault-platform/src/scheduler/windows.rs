//! Windows Task Scheduler owner for scheduled backup.
//!
//! ## Responsibilities
//! - Generate Task Scheduler XML.
//! - Apply, remove, and inspect the task through `schtasks.exe`.
//! - Translate Windows task state into typed issues and verification checks.
//!
//! ## Not responsible for
//! - Backup execution.
//! - macOS LaunchAgent or Linux systemd behavior.
//!
//! ## Dependencies
//! - Parent scheduler facade for shared status initialization.
//! - `audit` for audit artifact writes.
//! - `std::process::Command` for production `schtasks.exe` calls.
//!
//! ## Performance notes
//! - Status checks compare only the behavior-bearing Task Scheduler fields
//!   because Windows canonicalizes imported XML and injects default nodes.
//! - Status checks do not enumerate all scheduler tasks.

use crate::test_support::TEST_WINDOWS_USER_ID_ENV;
use anyhow::{Context, Result};
#[cfg(not(any(test, coverage)))]
use std::process::{Command, Output};
use std::{
    env, fs,
    path::{Path, PathBuf},
};
use vault_core::{
    ProjectPaths,
    models::{
        ApplyResult, GeneratedFile, ScheduleIssue, ScheduleManualStep, SchedulePlan,
        ScheduleStatus, ScheduleVerificationCheck,
    },
};

use super::{ScheduleParameters, audit, ensure_parent_dir, interval_minutes_from_hours};
#[cfg(any(test, coverage))]
use super::{TEST_SCHTASKS_MODE_ENV, TEST_SCHTASKS_QUERY_XML_ENV};

pub(super) fn windows_schedule_plan(
    label: &str,
    executable_path: &Path,
    worker_args: &[String],
    params: &ScheduleParameters,
) -> Result<SchedulePlan> {
    let repetition_interval = windows_repetition_interval(params.check_interval_hours);
    let user_id = current_windows_task_user_id()?;
    let escaped_user_id = xml_escape(&user_id);
    // `schtasks /Create /XML` reports "unable to switch the encoding" when its
    // Task Scheduler import path receives an XML declaration whose encoding no
    // longer matches the already-decoded string. The task XML is ASCII-safe, so
    // omit the declaration and let the file bytes speak for themselves.
    let xml = format!(
        r#"<Task version="1.4" xmlns="http://schemas.microsoft.com/windows/2004/02/mit/task">
  <Triggers>
    <LogonTrigger>
      <UserId>{}</UserId>
    </LogonTrigger>
    <TimeTrigger>
      <Enabled>true</Enabled>
      <Repetition>
        <Interval>{}</Interval>
        <StopAtDurationEnd>false</StopAtDurationEnd>
      </Repetition>
      <StartBoundary>2026-01-01T09:00:00</StartBoundary>
    </TimeTrigger>
  </Triggers>
  <Principals>
    <Principal id="Author">
      <UserId>{}</UserId>
      <RunLevel>LeastPrivilege</RunLevel>
      <LogonType>InteractiveToken</LogonType>
    </Principal>
  </Principals>
  <Settings>
    <MultipleInstancesPolicy>IgnoreNew</MultipleInstancesPolicy>
    <StartWhenAvailable>true</StartWhenAvailable>
    <Enabled>true</Enabled>
  </Settings>
  <Actions Context="Author">
    <Exec>
      <Command>{}</Command>
      <Arguments>{}</Arguments>
    </Exec>
  </Actions>
</Task>"#,
        escaped_user_id,
        repetition_interval,
        escaped_user_id,
        xml_escape(&executable_path.display().to_string()),
        xml_escape(&worker_args[1..].join(" "))
    );
    Ok(SchedulePlan {
        platform: "windows".to_string(),
        label: label.to_string(),
        executable_path: executable_path.display().to_string(),
        generated_files: vec![GeneratedFile {
            relative_path: format!("windows/{label}.task.xml"),
            absolute_path: None,
            purpose: "Import into Task Scheduler or register with schtasks.exe".to_string(),
            contents: xml.clone(),
        }],
        manual_steps: vec![
            "Review the XML file before registering it with Task Scheduler.".to_string(),
            format!(
                "PathKeep can register it with `schtasks /Create /TN {label} /XML <generated XML> /F`."
            ),
        ],
        manual_step_details: vec![
            ScheduleManualStep {
                id: "windows-save-xml".to_string(),
                title_key: "schedule.manualWindowsSaveXmlTitle".to_string(),
                summary_key: "schedule.manualWindowsSaveXmlSummary".to_string(),
                why_key: "schedule.manualWindowsSaveXmlWhy".to_string(),
                command: None,
                file_path: Some(format!("windows/{label}.task.xml")),
                file_contents: Some(xml),
                directory_path: None,
                can_auto_run: false,
                can_verify: true,
            },
            ScheduleManualStep {
                id: "windows-register-task".to_string(),
                title_key: "schedule.manualWindowsRegisterTitle".to_string(),
                summary_key: "schedule.manualWindowsRegisterSummary".to_string(),
                why_key: "schedule.manualWindowsRegisterWhy".to_string(),
                command: Some(vec![
                    "schtasks".to_string(),
                    "/Create".to_string(),
                    "/TN".to_string(),
                    label.to_string(),
                    "/XML".to_string(),
                    format!("windows/{label}.task.xml"),
                    "/F".to_string(),
                ]),
                file_path: None,
                file_contents: None,
                directory_path: None,
                can_auto_run: true,
                can_verify: true,
            },
        ],
        apply_commands: vec![vec![
            "schtasks".to_string(),
            "/Create".to_string(),
            "/TN".to_string(),
            label.to_string(),
            "/XML".to_string(),
            format!("windows/{label}.task.xml"),
            "/F".to_string(),
        ]],
        rollback_commands: vec![vec![
            "schtasks".to_string(),
            "/Delete".to_string(),
            "/TN".to_string(),
            label.to_string(),
            "/F".to_string(),
        ]],
        apply_supported: true,
    })
}

fn windows_repetition_interval(hours: f64) -> String {
    let minutes = interval_minutes_from_hours(hours);
    let whole_hours = minutes / 60;
    let remaining_minutes = minutes % 60;
    match (whole_hours, remaining_minutes) {
        (0, minutes) => format!("PT{minutes}M"),
        (hours, 0) => format!("PT{hours}H"),
        (hours, minutes) => format!("PT{hours}H{minutes}M"),
    }
}

fn current_windows_task_user_id() -> Result<String> {
    if let Some(user_id) = configured_windows_task_user_id() {
        return Ok(user_id);
    }

    #[cfg(any(test, coverage))]
    {
        Ok("PATHKEEP_TEST\\CurrentUser".to_string())
    }

    #[cfg(all(not(any(test, coverage)), target_os = "windows"))]
    {
        if let Some(user_id) = whoami_windows_user_id()? {
            return Ok(user_id);
        }
        if let Some(user_id) = windows_user_id_from_env() {
            return Ok(user_id);
        }
        anyhow::bail!("resolving current Windows user for Task Scheduler");
    }

    #[cfg(all(not(any(test, coverage)), not(target_os = "windows")))]
    {
        Ok("PathKeep\\CurrentUser".to_string())
    }
}

fn configured_windows_task_user_id() -> Option<String> {
    env::var(TEST_WINDOWS_USER_ID_ENV).ok().and_then(non_empty_trimmed)
}

#[cfg(all(not(any(test, coverage)), target_os = "windows"))]
fn whoami_windows_user_id() -> Result<Option<String>> {
    let output =
        Command::new("whoami").output().context("resolving current Windows user with whoami")?;
    if !output.status.success() {
        return Ok(None);
    }
    Ok(non_empty_trimmed(String::from_utf8_lossy(&output.stdout).to_string()))
}

#[cfg(all(not(any(test, coverage)), target_os = "windows"))]
fn windows_user_id_from_env() -> Option<String> {
    let username = env::var("USERNAME").ok().and_then(non_empty_trimmed)?;
    if username.contains('\\') || username.contains('@') {
        return Some(username);
    }
    env::var("USERDOMAIN")
        .ok()
        .and_then(non_empty_trimmed)
        .map(|domain| format!("{domain}\\{username}"))
        .or(Some(username))
}

fn non_empty_trimmed(value: String) -> Option<String> {
    let trimmed = value.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}

pub(super) fn apply_windows_schedule(
    plan: &SchedulePlan,
    paths: &ProjectPaths,
) -> Result<ApplyResult> {
    let xml_path = write_windows_task_xml(plan, paths)?;
    let args = vec![
        "/Create".to_string(),
        "/TN".to_string(),
        plan.label.clone(),
        "/XML".to_string(),
        xml_path.display().to_string(),
        "/F".to_string(),
    ];
    let outcome = run_schtasks(&args).context("installing Windows Task Scheduler task")?;
    let audit_path = audit::write_windows_schedule_audit(
        paths,
        plan,
        "apply",
        &xml_path,
        outcome.success,
        &outcome.status_description,
    )?;

    Ok(ApplyResult {
        applied: outcome.success,
        platform: plan.platform.clone(),
        files: vec![xml_path.display().to_string()],
        audit_path: Some(audit_path.display().to_string()),
        message: if outcome.success {
            "Task Scheduler task registered.".to_string()
        } else if windows_schtasks_access_denied(&outcome) {
            "schedule.windowsAccessDeniedInstallMessage".to_string()
        } else {
            format!(
                "Task Scheduler XML was written, but schtasks /Create did not report success: {}",
                outcome.status_description
            )
        },
        step_results: vec![ScheduleVerificationCheck {
            key: "windows-register".to_string(),
            status: if outcome.success { "ok" } else { "error" }.to_string(),
            label_key: "schedule.verifyWindowsRegister".to_string(),
            detail_key: if outcome.success {
                "schedule.verifyWindowsRegisterOk"
            } else if windows_schtasks_access_denied(&outcome) {
                "schedule.verifyWindowsRegisterAccessDenied"
            } else {
                "schedule.verifyWindowsRegisterFailed"
            }
            .to_string(),
            evidence: vec![outcome.status_description],
        }],
    })
}

pub(super) fn remove_windows_schedule(
    plan: &SchedulePlan,
    paths: &ProjectPaths,
) -> Result<ApplyResult> {
    let args = vec!["/Delete".to_string(), "/TN".to_string(), plan.label.clone(), "/F".to_string()];
    let outcome = run_schtasks(&args).context("removing Windows Task Scheduler task")?;
    let audit_path = audit::write_windows_schedule_audit(
        paths,
        plan,
        "remove",
        Path::new("Task Scheduler"),
        outcome.success,
        &outcome.status_description,
    )?;

    Ok(ApplyResult {
        applied: outcome.success,
        platform: plan.platform.clone(),
        files: Vec::new(),
        audit_path: Some(audit_path.display().to_string()),
        message: if outcome.success {
            "Task Scheduler task removed.".to_string()
        } else if windows_schtasks_not_found(&outcome) {
            "No installed PathKeep Task Scheduler task was found to remove.".to_string()
        } else {
            format!("schtasks /Delete did not report success: {}", outcome.status_description)
        },
        step_results: vec![ScheduleVerificationCheck {
            key: "windows-remove".to_string(),
            status: if outcome.success || windows_schtasks_not_found(&outcome) {
                "ok"
            } else {
                "error"
            }
            .to_string(),
            label_key: "schedule.verifyWindowsRemove".to_string(),
            detail_key: if outcome.success || windows_schtasks_not_found(&outcome) {
                "schedule.verifyWindowsRemoveOk"
            } else {
                "schedule.verifyWindowsRemoveFailed"
            }
            .to_string(),
            evidence: vec![outcome.status_description],
        }],
    })
}

pub(super) fn windows_schedule_status(
    plan: &SchedulePlan,
    mut status: ScheduleStatus,
) -> Result<ScheduleStatus> {
    let expected_xml = &generated_windows_task_file(plan)?.contents;
    let args =
        vec!["/Query".to_string(), "/TN".to_string(), plan.label.clone(), "/XML".to_string()];
    let outcome = run_schtasks(&args).context("querying Windows Task Scheduler task")?;

    if outcome.success {
        status.detected_files.push(format!("Task Scheduler:{}", plan.label));
        if windows_task_matches_plan(&outcome.stdout, expected_xml) {
            status.install_state = "installed".to_string();
            status.verification_checks.push(verification_check(
                "windows-task-xml",
                "ok",
                "schedule.verifyWindowsTaskXml",
                "schedule.verifyWindowsTaskXmlOk",
                vec![format!("Task Scheduler:{}", plan.label)],
            ));
        } else {
            status.install_state = "mismatch".to_string();
            status
                .warnings
                .push("Installed Task Scheduler XML no longer matches the current PathKeep schedule plan.".to_string());
            status.issues.push(ScheduleIssue {
                code: "windows-task-mismatch".to_string(),
                severity: "warning".to_string(),
                title_key: "schedule.issueTaskMismatchTitle".to_string(),
                detail_key: "schedule.issueTaskMismatchDetail".to_string(),
                consequence_key: "schedule.issueTaskMismatchConsequence".to_string(),
                evidence: vec![outcome.status_description],
                repair_action: Some("reinstall".to_string()),
                dismissible: false,
            });
            status.verification_checks.push(verification_check(
                "windows-task-xml",
                "warning",
                "schedule.verifyWindowsTaskXml",
                "schedule.verifyWindowsTaskXmlMismatch",
                vec![format!("Task Scheduler:{}", plan.label)],
            ));
        }
        return Ok(status);
    }

    if windows_schtasks_not_found(&outcome) {
        status.install_state = "not-installed".to_string();
        status.verification_checks.push(verification_check(
            "windows-task",
            "pending",
            "schedule.verifyWindowsTask",
            "schedule.verifyWindowsTaskMissing",
            Vec::new(),
        ));
        return Ok(status);
    }

    status.install_state = "permission-warning".to_string();
    let access_denied = windows_schtasks_access_denied(&outcome);
    status.warnings.push(format!(
        "PathKeep could not inspect the Windows Task Scheduler task `{}`: {}",
        plan.label, outcome.status_description
    ));
    status.issues.push(ScheduleIssue {
        code: if access_denied {
            "windows-task-access-denied"
        } else {
            "windows-task-inspection-failed"
        }
        .to_string(),
        severity: "error".to_string(),
        title_key: "schedule.issueInspectionFailedTitle".to_string(),
        detail_key: if access_denied {
            "schedule.issueWindowsAccessDeniedDetail"
        } else {
            "schedule.issueWindowsInspectionFailedDetail"
        }
        .to_string(),
        consequence_key: "schedule.issueInspectionFailedConsequence".to_string(),
        evidence: vec![outcome.status_description],
        repair_action: Some("manual-remove".to_string()),
        dismissible: false,
    });
    status.verification_checks.push(verification_check(
        "windows-task",
        "error",
        "schedule.verifyWindowsTask",
        "schedule.verifyWindowsTaskFailed",
        vec![outcome.stderr],
    ));
    Ok(status)
}

fn generated_windows_task_file(plan: &SchedulePlan) -> Result<&GeneratedFile> {
    plan.generated_files
        .first()
        .context("missing Task Scheduler XML file for Windows schedule plan")
}

fn write_windows_task_xml(plan: &SchedulePlan, paths: &ProjectPaths) -> Result<PathBuf> {
    let generated = generated_windows_task_file(plan)?;
    let target_path = paths.schedule_dir.join(&generated.relative_path);
    ensure_parent_dir(&target_path)?;
    fs::write(&target_path, &generated.contents)
        .with_context(|| format!("writing Task Scheduler XML to {}", target_path.display()))?;
    Ok(target_path)
}

#[cfg(not(any(test, coverage)))]
fn describe_process_output(action: &str, target: &str, output: &Output) -> String {
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();

    match (stdout.is_empty(), stderr.is_empty()) {
        (true, true) => format!("{action} {target}: {:?}", output.status),
        (false, true) => format!("{action} {target}: {:?}; stdout: {stdout}", output.status),
        (true, false) => format!("{action} {target}: {:?}; stderr: {stderr}", output.status),
        (false, false) => {
            format!("{action} {target}: {:?}; stdout: {stdout}; stderr: {stderr}", output.status)
        }
    }
}

#[cfg(not(any(test, coverage)))]
fn run_schtasks(args: &[String]) -> Result<SchtasksOutcome> {
    let output = Command::new("schtasks").args(args).output().context("running schtasks.exe")?;
    let status_description = describe_process_output("schtasks", &args.join(" "), &output);
    Ok(SchtasksOutcome {
        success: output.status.success(),
        status_description,
        stdout: String::from_utf8_lossy(&output.stdout).trim().to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).trim().to_string(),
    })
}

#[cfg(any(test, coverage))]
fn run_schtasks(args: &[String]) -> Result<SchtasksOutcome> {
    let is_query = args.iter().any(|arg| arg.eq_ignore_ascii_case("/Query"));
    let is_delete = args.iter().any(|arg| arg.eq_ignore_ascii_case("/Delete"));
    let mode = std::env::var(TEST_SCHTASKS_MODE_ENV).unwrap_or_else(|_| "success".to_string());

    if is_query {
        return Ok(match mode.as_str() {
            "missing" => SchtasksOutcome {
                success: false,
                status_description: "stub schtasks query: task not found".to_string(),
                stdout: String::new(),
                stderr: "ERROR: The system cannot find the file specified.".to_string(),
            },
            "denied" => SchtasksOutcome {
                success: false,
                status_description: "stub schtasks query: access denied".to_string(),
                stdout: String::new(),
                stderr: "Access is denied.".to_string(),
            },
            "fail" => SchtasksOutcome {
                success: false,
                status_description: "stub schtasks query: failed".to_string(),
                stdout: String::new(),
                stderr: "schtasks failed".to_string(),
            },
            "mismatch" => SchtasksOutcome {
                success: true,
                status_description: "stub schtasks query: mismatch".to_string(),
                stdout: "<Task><Actions /></Task>".to_string(),
                stderr: String::new(),
            },
            _ => SchtasksOutcome {
                success: true,
                status_description: "stub schtasks query: installed".to_string(),
                stdout: std::env::var(TEST_SCHTASKS_QUERY_XML_ENV).unwrap_or_default(),
                stderr: String::new(),
            },
        });
    }

    if mode == "missing" {
        return Ok(SchtasksOutcome {
            success: false,
            status_description: format!("stub schtasks {}: task not found", args.join(" ")),
            stdout: String::new(),
            stderr: "ERROR: The system cannot find the file specified.".to_string(),
        });
    }

    if mode == "denied" {
        return Ok(SchtasksOutcome {
            success: false,
            status_description: format!("stub schtasks {}: access denied", args.join(" ")),
            stdout: String::new(),
            stderr: "ERROR: Access is denied.".to_string(),
        });
    }

    if mode == "fail" {
        return Ok(SchtasksOutcome {
            success: false,
            status_description: format!("stub schtasks {}: failed", args.join(" ")),
            stdout: String::new(),
            stderr: "schtasks failed".to_string(),
        });
    }

    Ok(SchtasksOutcome {
        success: true,
        status_description: format!(
            "stub schtasks {}: {}",
            if is_delete { "delete" } else { "create" },
            args.join(" ")
        ),
        stdout: String::new(),
        stderr: String::new(),
    })
}

struct SchtasksOutcome {
    success: bool,
    status_description: String,
    stdout: String,
    stderr: String,
}

fn windows_schtasks_not_found(outcome: &SchtasksOutcome) -> bool {
    let text = format!("{} {}", outcome.status_description, outcome.stderr).to_ascii_lowercase();
    text.contains("cannot find") || text.contains("not found") || text.contains("does not exist")
}

fn windows_schtasks_access_denied(outcome: &SchtasksOutcome) -> bool {
    let text = format!("{} {}", outcome.status_description, outcome.stderr).to_ascii_lowercase();
    text.contains("access is denied") || text.contains("access denied")
}

pub(super) fn xml_escape(value: &str) -> String {
    value.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

fn windows_task_matches_plan(installed_xml: &str, expected_xml: &str) -> bool {
    if normalize_scheduler_xml(installed_xml) == normalize_scheduler_xml(expected_xml) {
        return true;
    }

    match (
        WindowsTaskSemantics::from_xml(installed_xml),
        WindowsTaskSemantics::from_xml(expected_xml),
    ) {
        (Some(installed), Some(expected)) => installed.matches_expected(&expected),
        _ => false,
    }
}

#[derive(Debug, PartialEq, Eq)]
struct WindowsTaskSemantics {
    command: String,
    arguments: String,
    repetition_minutes: u64,
    has_logon_trigger: bool,
    logon_type: Option<String>,
    run_level: Option<String>,
    start_when_available: Option<bool>,
}

impl WindowsTaskSemantics {
    fn from_xml(xml: &str) -> Option<Self> {
        let task = strip_xml_declaration(xml);
        let exec = xml_element(task, "Exec")?;
        let time_trigger = xml_element(task, "TimeTrigger")?;
        let principal = xml_element(task, "Principal")?;
        let settings = xml_element(task, "Settings").unwrap_or_default();
        let interval = xml_text(&time_trigger, "Interval")?;

        Some(Self {
            command: normalize_windows_command(&xml_text(&exec, "Command")?),
            arguments: normalize_windows_arguments(&xml_text(&exec, "Arguments")?),
            repetition_minutes: parse_windows_iso_minutes(&interval)?,
            has_logon_trigger: xml_element(task, "LogonTrigger").is_some(),
            logon_type: xml_text(&principal, "LogonType").map(|value| value.trim().to_string()),
            run_level: xml_text(&principal, "RunLevel").map(|value| value.trim().to_string()),
            start_when_available: xml_text(&settings, "StartWhenAvailable")
                .and_then(|value| parse_windows_xml_bool(&value)),
        })
    }

    fn matches_expected(&self, expected: &Self) -> bool {
        self.command == expected.command
            && self.arguments == expected.arguments
            && self.repetition_minutes == expected.repetition_minutes
            && self.has_logon_trigger == expected.has_logon_trigger
            && optional_case_insensitive_match(&self.logon_type, &expected.logon_type)
            && optional_case_insensitive_match_or_default(
                &self.run_level,
                &expected.run_level,
                "LeastPrivilege",
            )
            && optional_bool_match(self.start_when_available, expected.start_when_available)
    }
}

fn normalize_scheduler_xml(value: &str) -> String {
    strip_xml_declaration(value).split_whitespace().collect::<String>()
}

fn strip_xml_declaration(value: &str) -> &str {
    let trimmed = value.trim_start_matches('\u{feff}').trim_start();
    if !trimmed.starts_with("<?xml") {
        return trimmed;
    }
    trimmed.find("?>").map_or(trimmed, |end| &trimmed[end + 2..])
}

fn xml_element(value: &str, tag: &str) -> Option<String> {
    let close_tag = format!("</{tag}>");
    let open_start = find_xml_open_tag(value, tag)?;
    let open_end = value[open_start..].find('>')? + open_start;
    let content_start = open_end + 1;
    let close_start = value[content_start..].find(&close_tag)? + content_start;
    Some(value[content_start..close_start].to_string())
}

fn find_xml_open_tag(value: &str, tag: &str) -> Option<usize> {
    let open_prefix = format!("<{tag}");
    let mut offset = 0;

    while let Some(relative_start) = value[offset..].find(&open_prefix) {
        let start = offset + relative_start;
        let next = value[start + open_prefix.len()..].chars().next()?;
        if matches!(next, '>' | '/' | ' ' | '\t' | '\r' | '\n') {
            return Some(start);
        }
        offset = start + open_prefix.len();
    }

    None
}

fn xml_text(value: &str, tag: &str) -> Option<String> {
    xml_element(value, tag).map(|text| xml_unescape(text.trim()))
}

fn xml_unescape(value: &str) -> String {
    value
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
}

fn normalize_windows_command(value: &str) -> String {
    value.trim().replace('/', "\\").to_ascii_lowercase()
}

fn normalize_windows_arguments(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn parse_windows_xml_bool(value: &str) -> Option<bool> {
    match value.trim().to_ascii_lowercase().as_str() {
        "true" => Some(true),
        "false" => Some(false),
        _ => None,
    }
}

fn optional_bool_match(actual: Option<bool>, expected: Option<bool>) -> bool {
    match expected {
        Some(expected) => actual == Some(expected),
        None => true,
    }
}

fn optional_case_insensitive_match(actual: &Option<String>, expected: &Option<String>) -> bool {
    match expected {
        Some(expected) => {
            actual.as_deref().is_some_and(|actual| actual.eq_ignore_ascii_case(expected.trim()))
        }
        None => true,
    }
}

fn optional_case_insensitive_match_or_default(
    actual: &Option<String>,
    expected: &Option<String>,
    default: &str,
) -> bool {
    match expected {
        Some(expected) if expected.trim().eq_ignore_ascii_case(default) => {
            actual.as_deref().is_none_or(|actual| actual.eq_ignore_ascii_case(expected.trim()))
        }
        _ => optional_case_insensitive_match(actual, expected),
    }
}

fn parse_windows_iso_minutes(value: &str) -> Option<u64> {
    let duration = value.trim().strip_prefix("PT")?;
    let mut total_minutes = 0_u64;
    let mut digits = String::new();

    for ch in duration.chars() {
        if ch.is_ascii_digit() {
            digits.push(ch);
            continue;
        }

        let amount = digits.parse::<u64>().ok()?;
        digits.clear();
        match ch {
            'H' => total_minutes = total_minutes.checked_add(amount.checked_mul(60)?)?,
            'M' => total_minutes = total_minutes.checked_add(amount)?,
            _ => return None,
        }
    }

    (digits.is_empty() && total_minutes > 0).then_some(total_minutes)
}

fn verification_check(
    key: &str,
    status: &str,
    label_key: &str,
    detail_key: &str,
    evidence: Vec<String>,
) -> ScheduleVerificationCheck {
    ScheduleVerificationCheck {
        key: key.to_string(),
        status: status.to_string(),
        label_key: label_key.to_string(),
        detail_key: detail_key.to_string(),
        evidence,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EXPECTED_TASK_XML: &str = r#"<Task version="1.4" xmlns="http://schemas.microsoft.com/windows/2004/02/mit/task">
  <Triggers>
    <LogonTrigger>
      <UserId>PATHKEEPTEST\backup-user</UserId>
    </LogonTrigger>
    <TimeTrigger>
      <Enabled>true</Enabled>
      <Repetition>
        <Interval>PT1H30M</Interval>
        <StopAtDurationEnd>false</StopAtDurationEnd>
      </Repetition>
      <StartBoundary>2026-01-01T09:00:00</StartBoundary>
    </TimeTrigger>
  </Triggers>
  <Principals>
    <Principal id="Author">
      <UserId>PATHKEEPTEST\backup-user</UserId>
      <RunLevel>LeastPrivilege</RunLevel>
      <LogonType>InteractiveToken</LogonType>
    </Principal>
  </Principals>
  <Settings>
    <MultipleInstancesPolicy>IgnoreNew</MultipleInstancesPolicy>
    <StartWhenAvailable>true</StartWhenAvailable>
    <Enabled>true</Enabled>
  </Settings>
  <Actions Context="Author">
    <Exec>
      <Command>C:/Program Files/PathKeep/pathkeep-desktop.exe</Command>
      <Arguments>--worker backup --due-only</Arguments>
    </Exec>
  </Actions>
</Task>"#;

    const CANONICAL_TASK_XML: &str = r#"<?xml version="1.0" encoding="UTF-16"?>
<Task version="1.4" xmlns="http://schemas.microsoft.com/windows/2004/02/mit/task">
  <RegistrationInfo>
    <URI>com.yi-ting.pathkeep.backup</URI>
  </RegistrationInfo>
  <Principals>
    <Principal id="Author">
      <UserId>S-1-5-21-4216521022-1034979134-2183607003-1001</UserId>
      <LogonType>InteractiveToken</LogonType>
    </Principal>
  </Principals>
  <Settings>
    <DisallowStartIfOnBatteries>true</DisallowStartIfOnBatteries>
    <StopIfGoingOnBatteries>true</StopIfGoingOnBatteries>
    <MultipleInstancesPolicy>IgnoreNew</MultipleInstancesPolicy>
    <StartWhenAvailable>true</StartWhenAvailable>
    <IdleSettings>
      <StopOnIdleEnd>true</StopOnIdleEnd>
      <RestartOnIdle>false</RestartOnIdle>
    </IdleSettings>
    <UseUnifiedSchedulingEngine>true</UseUnifiedSchedulingEngine>
    <Enabled>true</Enabled>
  </Settings>
  <Triggers>
    <LogonTrigger>
      <UserId>CORE-WINDOWS\backup-user</UserId>
    </LogonTrigger>
    <TimeTrigger>
      <StartBoundary>2026-01-01T09:00:00</StartBoundary>
      <Repetition>
        <Interval>PT90M</Interval>
      </Repetition>
    </TimeTrigger>
  </Triggers>
  <Actions Context="Author">
    <Exec>
      <Command>C:\Program Files\PathKeep\pathkeep-desktop.exe</Command>
      <Arguments> --worker   backup   --due-only </Arguments>
    </Exec>
  </Actions>
</Task>"#;

    #[test]
    fn windows_task_semantics_accept_task_scheduler_canonicalization() {
        assert!(windows_task_matches_plan(CANONICAL_TASK_XML, EXPECTED_TASK_XML));
        assert!(windows_task_matches_plan(
            &format!("{}\n{}", r#"<?xml version="1.0" encoding="UTF-16"?>"#, EXPECTED_TASK_XML),
            EXPECTED_TASK_XML
        ));
    }

    #[test]
    fn windows_task_semantics_reject_behavior_drift() {
        assert!(!windows_task_matches_plan(
            &CANONICAL_TASK_XML.replace("<Interval>PT90M</Interval>", "<Interval>PT2H</Interval>"),
            EXPECTED_TASK_XML
        ));
        assert!(!windows_task_matches_plan(
            &CANONICAL_TASK_XML.replace("--due-only", "--all"),
            EXPECTED_TASK_XML
        ));
        assert!(!windows_task_matches_plan(
            &CANONICAL_TASK_XML.replace("<LogonType>InteractiveToken</LogonType>", ""),
            EXPECTED_TASK_XML
        ));
    }

    #[test]
    fn windows_iso_minutes_parser_accepts_task_scheduler_duration_forms() {
        assert_eq!(parse_windows_iso_minutes("PT90M"), Some(90));
        assert_eq!(parse_windows_iso_minutes("PT1H30M"), Some(90));
        assert_eq!(parse_windows_iso_minutes("PT6H"), Some(360));
        assert_eq!(parse_windows_iso_minutes("PT0M"), None);
        assert_eq!(parse_windows_iso_minutes("P1D"), None);
        assert_eq!(parse_windows_iso_minutes("PT1S"), None);
    }

    #[test]
    fn windows_semantic_optional_helpers_cover_absent_and_invalid_values() {
        assert_eq!(parse_windows_xml_bool("false"), Some(false));
        assert_eq!(parse_windows_xml_bool("maybe"), None);
        assert!(optional_bool_match(Some(false), None));
        assert!(optional_case_insensitive_match(&Some("Anything".to_string()), &None));
        assert!(optional_case_insensitive_match_or_default(
            &Some("InteractiveToken".to_string()),
            &Some("interactivetoken".to_string()),
            "LeastPrivilege"
        ));
    }
}
