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
//! - XML comparison normalizes whitespace only; status checks do not parse large
//!   files or enumerate all scheduler tasks.

use anyhow::{Context, Result};
#[cfg(not(any(test, coverage)))]
use std::process::{Command, Output};
use std::{
    fs,
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
    let xml = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<Task version="1.4" xmlns="http://schemas.microsoft.com/windows/2004/02/mit/task">
  <Triggers>
    <LogonTrigger />
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
        repetition_interval,
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
        if normalize_scheduler_xml(&outcome.stdout) == normalize_scheduler_xml(expected_xml) {
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
    status.warnings.push(format!(
        "PathKeep could not inspect the Windows Task Scheduler task `{}`: {}",
        plan.label, outcome.status_description
    ));
    status.issues.push(ScheduleIssue {
        code: "windows-task-inspection-failed".to_string(),
        severity: "error".to_string(),
        title_key: "schedule.issueInspectionFailedTitle".to_string(),
        detail_key: "schedule.issueWindowsInspectionFailedDetail".to_string(),
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

pub(super) fn xml_escape(value: &str) -> String {
    value.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

fn normalize_scheduler_xml(value: &str) -> String {
    value.split_whitespace().collect::<String>()
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
