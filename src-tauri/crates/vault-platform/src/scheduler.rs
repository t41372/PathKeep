//! Native scheduler adapters.
//!
//! PathKeep's schedule contract is explicit preview/manual/apply/remove/verify.
//! This module turns that contract into host-specific plan files and install
//! commands without hiding what will be written or executed.

use crate::host_capability::current_platform_name;
#[cfg(any(test, coverage))]
use crate::test_support::launchctl_stub_success;
use crate::test_support::{launch_agents_dir_override, schedule_label};
use anyhow::{Context, Result};
use chrono::Utc;
#[cfg(not(any(test, coverage)))]
use directories::UserDirs;
use serde::Serialize;
#[cfg(not(any(test, coverage)))]
use std::process::{Command, Output};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};
use vault_core::{
    ProjectPaths,
    models::{ApplyResult, GeneratedFile, SchedulePlan, ScheduleStatus},
};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
/// Runtime parameters that shape the generated backup schedule plan.
pub struct ScheduleParameters {
    pub due_after_hours: u64,
    pub check_interval_hours: u64,
}

#[cfg(any(test, coverage))]
const TEST_SCHTASKS_MODE_ENV: &str = "PATHKEEP_TEST_SCHTASKS_MODE";
#[cfg(any(test, coverage))]
const TEST_SCHTASKS_QUERY_XML_ENV: &str = "PATHKEEP_TEST_SCHTASKS_QUERY_XML";

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
        "macos" => macos_schedule_plan(&label, executable_path, &worker_args, &log_dir, params),
        "windows" => windows_schedule_plan(&label, executable_path, &worker_args, params),
        _ => linux_schedule_plan(&label, executable_path, &worker_args, params),
    }
}

/// Applies a previously previewed native schedule plan when the platform supports it.
pub fn apply_schedule(plan: &SchedulePlan, paths: &ProjectPaths) -> Result<ApplyResult> {
    if plan.platform == "windows" {
        return apply_windows_schedule(plan, paths);
    }
    if plan.platform != "macos" {
        return Ok(ApplyResult {
            applied: false,
            platform: plan.platform.clone(),
            files: Vec::new(),
            audit_path: None,
            message:
                "Apply is only implemented on macOS in v1. Use the Manual steps on other platforms."
                    .to_string(),
        });
    }

    let launch_agents_dir = launch_agents_dir()?;
    fs::create_dir_all(&launch_agents_dir)?;

    let mut written_files = Vec::new();
    for file in &plan.generated_files {
        let target_path = launch_agents_dir
            .join(PathBuf::from(&file.relative_path).file_name().unwrap_or_default());
        fs::write(&target_path, &file.contents)?;
        written_files.push(target_path.display().to_string());
    }

    let uid = scheduler_uid()?;
    let plist_path =
        written_files.first().context("missing plist file for macOS schedule apply")?.clone();
    let bootstrap = bootstrap_launch_agent(&uid, &plan.label, &plist_path)?;
    let audit_path = write_schedule_apply_audit(paths, plan, &plist_path, &bootstrap)?;

    Ok(ApplyResult {
        applied: bootstrap.success,
        platform: plan.platform.clone(),
        files: written_files,
        audit_path: Some(audit_path.display().to_string()),
        message: if bootstrap.success {
            "LaunchAgent installed and bootstrapped.".to_string()
        } else {
            format!(
                "LaunchAgent files were written, but launchctl bootstrap did not report success: {}",
                bootstrap.status_description
            )
        },
    })
}

/// Removes a previously applied native schedule plan when the platform supports it.
pub fn remove_schedule(plan: &SchedulePlan, paths: &ProjectPaths) -> Result<ApplyResult> {
    if plan.platform == "windows" {
        return remove_windows_schedule(plan, paths);
    }
    if plan.platform != "macos" {
        return Ok(ApplyResult {
            applied: false,
            platform: plan.platform.clone(),
            files: Vec::new(),
            audit_path: None,
            message:
                "Remove is only implemented on macOS in v1. Use the Manual rollback steps on other platforms."
                    .to_string(),
        });
    }

    let launch_agents_dir = launch_agents_dir()?;
    fs::create_dir_all(&launch_agents_dir)?;

    let uid = scheduler_uid()?;
    let current_path = generated_plist_target_path(plan, &launch_agents_dir)?;
    let current_unload = bootout_launch_agent(&uid, &plan.label)?;

    let mut removed_files = Vec::new();
    if current_path.exists() {
        fs::remove_file(&current_path)?;
        removed_files.push(current_path.display().to_string());
    }

    let audit_path = write_schedule_remove_audit(paths, plan, &removed_files, &[current_unload])?;
    let applied = !removed_files.is_empty();

    Ok(ApplyResult {
        applied,
        platform: plan.platform.clone(),
        files: removed_files,
        audit_path: Some(audit_path.display().to_string()),
        message: if applied {
            "Installed LaunchAgent files were removed.".to_string()
        } else {
            "No installed PathKeep LaunchAgent files were found to remove.".to_string()
        },
    })
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
        manual_steps: plan.manual_steps.clone(),
        audit_path: latest_schedule_audit_path(paths),
        ..ScheduleStatus::default()
    };

    if plan.platform == "windows" {
        return windows_schedule_status(&plan, status);
    }

    if plan.platform != "macos" {
        status.warnings.push(
            "Automatic install-status detection is only implemented on macOS in v1. Use the manual verification steps for this platform.".to_string(),
        );
        return Ok(status);
    }

    let launch_agents_dir = launch_agents_dir()?;
    let target_path = generated_plist_target_path(&plan, &launch_agents_dir)?;

    if target_path.exists() {
        status.detected_files.push(target_path.display().to_string());
        match fs::read_to_string(&target_path) {
            Ok(contents) => {
                status.install_state = if contents == plan.generated_files[0].contents {
                    "installed".to_string()
                } else {
                    status
                        .warnings
                        .push("Installed LaunchAgent content no longer matches the current PathKeep schedule plan.".to_string());
                    "mismatch".to_string()
                };
            }
            Err(error) => {
                status.install_state = "permission-warning".to_string();
                status.warnings.push(format!(
                    "PathKeep could not read the installed LaunchAgent at {}: {error}",
                    target_path.display()
                ));
            }
        }
    }

    Ok(status)
}

fn generated_plist_target_path(plan: &SchedulePlan, launch_agents_dir: &Path) -> Result<PathBuf> {
    let generated =
        plan.generated_files.first().context("missing plist file for macOS schedule plan")?;
    Ok(launch_agents_dir
        .join(PathBuf::from(&generated.relative_path).file_name().unwrap_or_default()))
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

fn apply_windows_schedule(plan: &SchedulePlan, paths: &ProjectPaths) -> Result<ApplyResult> {
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
    let audit_path = write_windows_schedule_audit(paths, plan, "apply", &xml_path, &outcome)?;

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
    })
}

fn remove_windows_schedule(plan: &SchedulePlan, paths: &ProjectPaths) -> Result<ApplyResult> {
    let args = vec![
        "/Delete".to_string(),
        "/TN".to_string(),
        plan.label.clone(),
        "/F".to_string(),
    ];
    let outcome = run_schtasks(&args).context("removing Windows Task Scheduler task")?;
    let audit_path =
        write_windows_schedule_audit(paths, plan, "remove", Path::new("Task Scheduler"), &outcome)?;

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
    })
}

fn windows_schedule_status(plan: &SchedulePlan, mut status: ScheduleStatus) -> Result<ScheduleStatus> {
    let expected_xml = &generated_windows_task_file(plan)?.contents;
    let args = vec!["/Query".to_string(), "/TN".to_string(), plan.label.clone(), "/XML".to_string()];
    let outcome = run_schtasks(&args).context("querying Windows Task Scheduler task")?;

    if outcome.success {
        status.detected_files.push(format!("Task Scheduler:{}", plan.label));
        status.install_state = if normalize_scheduler_xml(&outcome.stdout)
            == normalize_scheduler_xml(expected_xml)
        {
            "installed".to_string()
        } else {
            status.warnings.push(
                "Installed Task Scheduler XML no longer matches the current PathKeep schedule plan."
                    .to_string(),
            );
            "mismatch".to_string()
        };
        return Ok(status);
    }

    if windows_schtasks_not_found(&outcome) {
        status.install_state = "not-installed".to_string();
        return Ok(status);
    }

    status.install_state = "permission-warning".to_string();
    status.warnings.push(format!(
        "PathKeep could not inspect the Windows Task Scheduler task `{}`: {}",
        plan.label, outcome.status_description
    ));
    Ok(status)
}

fn write_schedule_apply_audit(
    paths: &ProjectPaths,
    plan: &SchedulePlan,
    plist_path: &str,
    bootstrap: &LaunchctlOutcome,
) -> Result<PathBuf> {
    let audit_path = paths
        .audit_repo_path
        .join("scheduler")
        .join(format!("apply-{}.json", Utc::now().to_rfc3339().replace(':', "-")));
    ensure_parent_dir(&audit_path)?;

    let contents = serde_json::to_string_pretty(&BTreeMap::from([
        ("platform".to_string(), plan.platform.clone()),
        ("plistPath".to_string(), plist_path.to_string()),
        ("status".to_string(), bootstrap.status_description.clone()),
    ]))?;
    fs::write(&audit_path, contents)?;
    Ok(audit_path)
}

fn write_schedule_remove_audit(
    paths: &ProjectPaths,
    plan: &SchedulePlan,
    removed_files: &[String],
    unloads: &[LaunchctlOutcome],
) -> Result<PathBuf> {
    let audit_path = paths
        .audit_repo_path
        .join("scheduler")
        .join(format!("remove-{}.json", Utc::now().to_rfc3339().replace(':', "-")));
    ensure_parent_dir(&audit_path)?;

    let contents = serde_json::to_string_pretty(&BTreeMap::from([
        ("action".to_string(), "remove".to_string()),
        ("platform".to_string(), plan.platform.clone()),
        ("label".to_string(), plan.label.clone()),
        ("removedFiles".to_string(), serde_json::to_string(removed_files)?),
        (
            "launchctl".to_string(),
            serde_json::to_string(
                &unloads
                    .iter()
                    .map(|outcome| outcome.status_description.clone())
                    .collect::<Vec<_>>(),
            )?,
        ),
    ]))?;
    fs::write(&audit_path, contents)?;
    Ok(audit_path)
}

fn write_windows_schedule_audit(
    paths: &ProjectPaths,
    plan: &SchedulePlan,
    action: &str,
    xml_path: &Path,
    outcome: &SchtasksOutcome,
) -> Result<PathBuf> {
    let audit_path = paths.audit_repo_path.join("scheduler").join(format!(
        "{action}-windows-{}.json",
        Utc::now().to_rfc3339().replace(':', "-")
    ));
    ensure_parent_dir(&audit_path)?;

    let contents = serde_json::to_string_pretty(&BTreeMap::from([
        ("action".to_string(), action.to_string()),
        ("platform".to_string(), plan.platform.clone()),
        ("label".to_string(), plan.label.clone()),
        ("xmlPath".to_string(), xml_path.display().to_string()),
        ("success".to_string(), outcome.success.to_string()),
        ("status".to_string(), outcome.status_description.clone()),
    ]))?;
    fs::write(&audit_path, contents)?;
    Ok(audit_path)
}

fn latest_schedule_audit_path(paths: &ProjectPaths) -> Option<String> {
    let scheduler_dir = paths.audit_repo_path.join("scheduler");
    let mut newest = fs::read_dir(&scheduler_dir)
        .ok()?
        .filter_map(|entry| entry.ok())
        .filter_map(|entry| {
            let metadata = entry.metadata().ok()?;
            let modified = metadata.modified().ok()?;
            Some((modified, entry.path()))
        })
        .collect::<Vec<_>>();
    newest.sort_by_key(|(modified, _)| *modified);
    newest.last().map(|(_, path)| path.display().to_string())
}

#[rustfmt::skip]
fn ensure_parent_dir(path: &Path) -> Result<()> { if let Some(parent) = path.parent() { fs::create_dir_all(parent)?; } Ok(()) }

fn macos_schedule_plan(
    label: &str,
    executable_path: &Path,
    worker_args: &[String],
    log_dir: &Path,
    params: &ScheduleParameters,
) -> Result<SchedulePlan> {
    let plist = plist::Value::Dictionary(plist::Dictionary::from_iter([
        ("Label".to_string(), plist::Value::String(label.to_string())),
        (
            "ProgramArguments".to_string(),
            plist::Value::Array(worker_args.iter().cloned().map(plist::Value::String).collect()),
        ),
        ("RunAtLoad".to_string(), plist::Value::Boolean(true)),
        (
            "StartInterval".to_string(),
            plist::Value::Integer((params.check_interval_hours * 3600).into()),
        ),
        (
            "StandardOutPath".to_string(),
            plist::Value::String(log_dir.join("worker.stdout.log").display().to_string()),
        ),
        (
            "StandardErrorPath".to_string(),
            plist::Value::String(log_dir.join("worker.stderr.log").display().to_string()),
        ),
    ]));
    let mut buffer = Vec::new();
    plist::to_writer_xml(&mut buffer, &plist)?;
    let contents = String::from_utf8(buffer)?;
    Ok(SchedulePlan {
        platform: "macos".to_string(),
        label: label.to_string(),
        executable_path: executable_path.display().to_string(),
        generated_files: vec![GeneratedFile {
            relative_path: format!("launchd/{label}.plist"),
            absolute_path: None,
            purpose: format!(
                "Run worker mode every {} hours and immediately after login or boot.",
                params.check_interval_hours
            ),
            contents,
        }],
        manual_steps: vec![
            format!("Save the plist to ~/Library/LaunchAgents/{label}.plist."),
            format!(
                "Run `launchctl bootstrap gui/$(id -u) ~/Library/LaunchAgents/{label}.plist` to load the new schedule."
            ),
        ],
        apply_commands: vec![
            vec![
                "launchctl".to_string(),
                "bootout".to_string(),
                "gui/$(id -u)".to_string(),
                label.to_string(),
            ],
            vec![
                "launchctl".to_string(),
                "bootstrap".to_string(),
                "gui/$(id -u)".to_string(),
                format!("~/Library/LaunchAgents/{label}.plist"),
            ],
        ],
        rollback_commands: vec![vec![
            "launchctl".to_string(),
            "bootout".to_string(),
            "gui/$(id -u)".to_string(),
            label.to_string(),
        ]],
        apply_supported: true,
    })
}

fn windows_schedule_plan(
    label: &str,
    executable_path: &Path,
    worker_args: &[String],
    params: &ScheduleParameters,
) -> Result<SchedulePlan> {
    let xml = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<Task version="1.4" xmlns="http://schemas.microsoft.com/windows/2004/02/mit/task">
  <Triggers>
    <LogonTrigger />
    <TimeTrigger>
      <Enabled>true</Enabled>
      <Repetition>
        <Interval>PT{}H</Interval>
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
        params.check_interval_hours,
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
            contents: xml,
        }],
        manual_steps: vec![
            "Review the XML file before registering it with Task Scheduler.".to_string(),
            format!("PathKeep can register it with `schtasks /Create /TN {label} /XML <generated XML> /F`."),
        ],
        apply_commands: vec![
            vec![
                "schtasks".to_string(),
                "/Create".to_string(),
                "/TN".to_string(),
                label.to_string(),
                "/XML".to_string(),
                format!("windows/{label}.task.xml"),
                "/F".to_string(),
            ],
        ],
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

fn linux_schedule_plan(
    label: &str,
    executable_path: &Path,
    worker_args: &[String],
    params: &ScheduleParameters,
) -> Result<SchedulePlan> {
    let service = format!(
        "[Unit]\nDescription=PathKeep backup worker\n\n[Service]\nType=oneshot\nExecStart={} {}\n",
        executable_path.display(),
        worker_args[1..].join(" ")
    );
    let timer = format!(
        "[Unit]\nDescription=PathKeep periodic backup\n\n[Timer]\nOnCalendar={}\nPersistent=true\n\n[Install]\nWantedBy=timers.target\n",
        linux_on_calendar(params.check_interval_hours)
    );
    Ok(SchedulePlan {
        platform: "linux".to_string(),
        label: label.to_string(),
        executable_path: executable_path.display().to_string(),
        generated_files: vec![
            GeneratedFile {
                relative_path: format!("systemd/{label}.service"),
                absolute_path: None,
                purpose: "User service entry for the worker mode".to_string(),
                contents: service,
            },
            GeneratedFile {
                relative_path: format!("systemd/{label}.timer"),
                absolute_path: None,
                purpose: format!(
                    "Persistent user timer that wakes every {} hours.",
                    params.check_interval_hours
                ),
                contents: timer,
            },
        ],
        manual_steps: vec![
            "Copy the files to ~/.config/systemd/user/.".to_string(),
            "Run `systemctl --user daemon-reload`.".to_string(),
            format!("Run `systemctl --user enable --now {label}.timer`."),
            format!(
                "Run `systemctl --user list-timers {label}.timer` to verify the next scheduled run."
            ),
        ],
        apply_commands: Vec::new(),
        rollback_commands: vec![vec![
            "systemctl".to_string(),
            "--user".to_string(),
            "disable".to_string(),
            "--now".to_string(),
            format!("{label}.timer"),
        ]],
        apply_supported: false,
    })
}

fn linux_on_calendar(hours: u64) -> String {
    if hours <= 1 { "*-*-* *:00:00".to_string() } else { format!("*-*-* 00/{hours}:00:00") }
}

fn xml_escape(value: &str) -> String {
    value.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

fn normalize_scheduler_xml(value: &str) -> String {
    value.split_whitespace().collect::<String>()
}

struct LaunchctlOutcome {
    success: bool,
    status_description: String,
}

struct SchtasksOutcome {
    success: bool,
    status_description: String,
    stdout: String,
    stderr: String,
}

#[cfg(not(any(test, coverage)))]
fn describe_launchctl_output(action: &str, target: &str, output: &Output) -> String {
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
    let output = Command::new("schtasks")
        .args(args)
        .output()
        .context("running schtasks.exe")?;
    let status_description = describe_launchctl_output("schtasks", &args.join(" "), &output);
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

fn windows_schtasks_not_found(outcome: &SchtasksOutcome) -> bool {
    let text = format!("{} {}", outcome.status_description, outcome.stderr).to_ascii_lowercase();
    text.contains("cannot find") || text.contains("not found") || text.contains("does not exist")
}

#[cfg(not(any(test, coverage)))]
fn launch_agents_dir() -> Result<PathBuf> {
    if let Some(path) = launch_agents_dir_override() {
        return Ok(path);
    }
    Ok(UserDirs::new().context("resolving home dir")?.home_dir().join("Library/LaunchAgents"))
}

#[cfg(any(test, coverage))]
fn launch_agents_dir() -> Result<PathBuf> {
    Ok(launch_agents_dir_override()
        .unwrap_or_else(|| std::env::temp_dir().join("pathkeep-launch-agents")))
}

#[cfg(not(any(test, coverage)))]
fn scheduler_uid() -> Result<String> {
    let uid = Command::new("id").arg("-u").output().context("running id -u")?;
    Ok(String::from_utf8_lossy(&uid.stdout).trim().to_string())
}

#[cfg(any(test, coverage))]
fn scheduler_uid() -> Result<String> {
    Ok("501".to_string())
}

#[cfg(not(any(test, coverage)))]
fn bootstrap_launch_agent(uid: &str, label: &str, plist_path: &str) -> Result<LaunchctlOutcome> {
    let _ = bootout_launch_agent(uid, label);
    let output = Command::new("launchctl")
        .args(["bootstrap", &format!("gui/{uid}"), plist_path])
        .output()
        .context("bootstrapping launch agent")?;
    Ok(LaunchctlOutcome {
        success: output.status.success(),
        status_description: describe_launchctl_output("bootstrap", plist_path, &output),
    })
}

#[cfg(any(test, coverage))]
fn bootstrap_launch_agent(uid: &str, label: &str, plist_path: &str) -> Result<LaunchctlOutcome> {
    let _ = label;
    Ok(LaunchctlOutcome {
        success: launchctl_stub_success(),
        status_description: format!("stub bootstrap gui/{uid} {plist_path}"),
    })
}

#[cfg(not(any(test, coverage)))]
fn bootout_launch_agent(uid: &str, label: &str) -> Result<LaunchctlOutcome> {
    let output = Command::new("launchctl")
        .args(["bootout", &format!("gui/{uid}"), label])
        .output()
        .context("unloading launch agent")?;
    Ok(LaunchctlOutcome {
        success: output.status.success(),
        status_description: describe_launchctl_output("bootout", label, &output),
    })
}

#[cfg(any(test, coverage))]
fn bootout_launch_agent(uid: &str, label: &str) -> Result<LaunchctlOutcome> {
    Ok(LaunchctlOutcome {
        success: launchctl_stub_success(),
        status_description: format!("stub bootout gui/{uid} {label}"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{
        TEST_LAUNCH_AGENTS_DIR_ENV, TEST_LAUNCHCTL_SUCCESS_ENV, TEST_SCHEDULE_LABEL_ENV, env_lock,
        restore_env_var,
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
        let params = ScheduleParameters { due_after_hours: 72, check_interval_hours: 6 };

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
        assert!(windows.generated_files[0].contents.contains("<Task"));
        assert!(windows.generated_files[0].contents.contains("encoding=\"UTF-8\""));
        assert!(linux.generated_files[1].contents.contains("Persistent=true"));
        assert!(linux.generated_files[1].contents.contains("OnCalendar="));
        assert!(!linux.generated_files[1].contents.contains("OnUnitActiveSec"));
    }

    #[test]
    fn preview_schedule_uses_current_platform_when_unspecified() {
        let dir = tempdir().expect("tempdir");
        let paths = sample_paths(dir.path());
        let params = ScheduleParameters { due_after_hours: 72, check_interval_hours: 6 };
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
        unsafe {
            std::env::set_var(TEST_LAUNCH_AGENTS_DIR_ENV, &launch_agents_dir);
            std::env::set_var(TEST_SCHEDULE_LABEL_ENV, "com.yi-ting.pathkeep.tests");
        }

        let paths = sample_paths(dir.path());
        let params = ScheduleParameters { due_after_hours: 72, check_interval_hours: 6 };
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

        assert_eq!(status.install_state, "installed");
        assert_eq!(status.detected_files.len(), 1);
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
        let params = ScheduleParameters { due_after_hours: 72, check_interval_hours: 6 };
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
    fn linux_apply_is_manual_only() {
        let dir = tempdir().expect("tempdir");
        let result = apply_schedule(
            &SchedulePlan {
                platform: "linux".to_string(),
                label: "example".to_string(),
                executable_path: "/usr/bin/chb".to_string(),
                generated_files: Vec::new(),
                manual_steps: Vec::new(),
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
        let params = ScheduleParameters { due_after_hours: 72, check_interval_hours: 6 };
        let plan =
            preview_schedule(Some("windows"), Path::new("C:/PathKeep/pathkeep.exe"), &paths, &params)
                .expect("windows plan");
        unsafe {
            std::env::set_var(TEST_SCHTASKS_QUERY_XML_ENV, &plan.generated_files[0].contents);
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
    fn windows_schedule_status_reports_missing_mismatch_and_permission_warning() {
        let _guard = env_lock().lock().expect("env lock");
        let dir = tempdir().expect("tempdir");
        let original_schedule_label = std::env::var_os(TEST_SCHEDULE_LABEL_ENV);
        let original_schtasks_mode = std::env::var_os(TEST_SCHTASKS_MODE_ENV);
        unsafe {
            std::env::set_var(TEST_SCHEDULE_LABEL_ENV, "com.yi-ting.pathkeep.tests");
        }

        let paths = sample_paths(dir.path());
        let params = ScheduleParameters { due_after_hours: 72, check_interval_hours: 6 };
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

        restore_env_var(TEST_SCHEDULE_LABEL_ENV, original_schedule_label.as_deref());
        restore_env_var(TEST_SCHTASKS_MODE_ENV, original_schtasks_mode.as_deref());

        assert_eq!(missing.install_state, "not-installed");
        assert_eq!(mismatch.install_state, "mismatch");
        assert!(mismatch.warnings.iter().any(|warning| warning.contains("no longer matches")));
        assert_eq!(denied.install_state, "permission-warning");
        assert!(denied.warnings.iter().any(|warning| warning.contains("could not inspect")));
    }

    #[test]
    fn macos_apply_schedule_writes_files_and_audit_report() {
        let _guard = env_lock().lock().expect("env lock");
        let dir = tempdir().expect("tempdir");
        let launch_agents_dir = dir.path().join("LaunchAgents");
        let original_launch_agents = std::env::var_os(TEST_LAUNCH_AGENTS_DIR_ENV);
        let original_launchctl_success = std::env::var_os(TEST_LAUNCHCTL_SUCCESS_ENV);
        let original_schedule_label = std::env::var_os(TEST_SCHEDULE_LABEL_ENV);
        unsafe {
            std::env::set_var(TEST_LAUNCH_AGENTS_DIR_ENV, &launch_agents_dir);
            std::env::set_var(TEST_LAUNCHCTL_SUCCESS_ENV, "1");
            std::env::set_var(TEST_SCHEDULE_LABEL_ENV, "com.yi-ting.pathkeep.tests");
        }

        let paths = sample_paths(dir.path());
        let params = ScheduleParameters { due_after_hours: 72, check_interval_hours: 6 };
        let plan =
            preview_schedule(Some("macos"), Path::new("/tmp/chb"), &paths, &params).expect("plan");
        let result = apply_schedule(&plan, &paths).expect("apply macos plan");

        restore_env_var(TEST_LAUNCH_AGENTS_DIR_ENV, original_launch_agents.as_deref());
        restore_env_var(TEST_LAUNCHCTL_SUCCESS_ENV, original_launchctl_success.as_deref());
        restore_env_var(TEST_SCHEDULE_LABEL_ENV, original_schedule_label.as_deref());

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
        unsafe {
            std::env::set_var(TEST_LAUNCH_AGENTS_DIR_ENV, &launch_agents_dir);
            std::env::set_var(TEST_LAUNCHCTL_SUCCESS_ENV, "0");
            std::env::set_var(TEST_SCHEDULE_LABEL_ENV, "com.yi-ting.pathkeep.tests");
        }

        let paths = sample_paths(dir.path());
        let params = ScheduleParameters { due_after_hours: 72, check_interval_hours: 6 };
        let plan =
            preview_schedule(Some("macos"), Path::new("/tmp/chb"), &paths, &params).expect("plan");
        let result = apply_schedule(&plan, &paths).expect("apply failing macos plan");

        restore_env_var(TEST_LAUNCH_AGENTS_DIR_ENV, original_launch_agents.as_deref());
        restore_env_var(TEST_LAUNCHCTL_SUCCESS_ENV, original_launchctl_success.as_deref());
        restore_env_var(TEST_SCHEDULE_LABEL_ENV, original_schedule_label.as_deref());

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
        unsafe {
            std::env::set_var(TEST_LAUNCH_AGENTS_DIR_ENV, &launch_agents_dir);
            std::env::set_var(TEST_LAUNCHCTL_SUCCESS_ENV, "1");
            std::env::set_var(TEST_SCHEDULE_LABEL_ENV, "com.yi-ting.pathkeep.tests");
        }

        let paths = sample_paths(dir.path());
        let params = ScheduleParameters { due_after_hours: 72, check_interval_hours: 6 };
        let plan =
            preview_schedule(Some("macos"), Path::new("/tmp/chb"), &paths, &params).expect("plan");
        let applied = apply_schedule(&plan, &paths).expect("apply macos plan");
        assert!(applied.applied);

        let removed = remove_schedule(&plan, &paths).expect("remove macos plan");

        restore_env_var(TEST_LAUNCH_AGENTS_DIR_ENV, original_launch_agents.as_deref());
        restore_env_var(TEST_LAUNCHCTL_SUCCESS_ENV, original_launchctl_success.as_deref());
        restore_env_var(TEST_SCHEDULE_LABEL_ENV, original_schedule_label.as_deref());

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
        let params = ScheduleParameters { due_after_hours: 72, check_interval_hours: 6 };
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
        let params = ScheduleParameters { due_after_hours: 72, check_interval_hours: 6 };
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
            xml_escape("<tag attr=\"1\">&</tag>"),
            "&lt;tag attr=&quot;1&quot;&gt;&amp;&lt;/tag&gt;"
        );
    }
}
