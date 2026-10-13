//! macOS LaunchAgent owner for scheduled backup.
//!
//! ## Responsibilities
//! - Generate LaunchAgent plist previews.
//! - Apply/remove the canonical PathKeep LaunchAgent.
//! - Detect canonical, mismatched, unloaded, unreadable, and legacy LaunchAgent
//!   states without silently migrating older labels.
//! - Repair only known legacy labels when the user explicitly requests it.
//!
//! ## Not responsible for
//! - Running backups.
//! - Windows Task Scheduler or Linux systemd behavior.
//! - Localizing issue copy.
//!
//! ## Dependencies
//! - `launchctl` and `id -u` in production.
//! - A file-backed launchd emulator when the debug sandbox is active
//!   (`crate::sandbox`): plists go to `<sandbox>/LaunchAgents`, and a marker
//!   file per label under `<sandbox>/launchd-loaded` stands in for "loaded".
//! - `crate::test_support` environment overrides for deterministic tests.
//! - Under `cfg(test)`, stand-ins for `id -u` and `launchctl` that run nothing.
//! - `audit` for apply/remove/repair audit artifacts.
//!
//! ## Performance notes
//! - Status reads inspect only the canonical plist and the known legacy labels;
//!   they never enumerate all LaunchAgents.

use anyhow::{Context, Result};
#[cfg(not(test))]
use directories::UserDirs;
#[cfg(not(test))]
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

use super::{
    LEGACY_MACOS_SCHEDULE_LABELS, ScheduleParameters, SchedulerHost, audit, count_native_call,
    format_interval_label, interval_minutes_from_hours, interval_seconds_from_hours,
};
use crate::sandbox::{LAUNCH_AGENTS_SUBDIR, LAUNCHD_LOADED_SUBDIR};
use crate::test_support::launch_agents_dir_override;

pub(super) fn macos_schedule_plan(
    label: &str,
    executable_path: &Path,
    worker_args: &[String],
    log_dir: &Path,
    params: &ScheduleParameters,
) -> Result<SchedulePlan> {
    let check_interval_minutes = interval_minutes_from_hours(params.check_interval_hours);
    let check_interval_label = format_interval_label(check_interval_minutes);
    let stdout_log = log_dir.join("worker.stdout.log").display().to_string();
    let stderr_log = log_dir.join("worker.stderr.log").display().to_string();
    let plist = plist::Value::Dictionary(plist::Dictionary::from_iter([
        ("Label".to_string(), plist::Value::String(label.to_string())),
        (
            "ProgramArguments".to_string(),
            plist::Value::Array(worker_args.iter().cloned().map(plist::Value::String).collect()),
        ),
        ("RunAtLoad".to_string(), plist::Value::Boolean(true)),
        (
            "StartInterval".to_string(),
            plist::Value::Integer(interval_seconds_from_hours(params.check_interval_hours).into()),
        ),
        // launchd runs the worker directly and captures its output.
        ("StandardOutPath".to_string(), plist::Value::String(stdout_log.clone())),
        ("StandardErrorPath".to_string(), plist::Value::String(stderr_log.clone())),
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
                "Run worker mode every {check_interval_label} and immediately after login or boot."
            ),
            contents: contents.clone(),
        }],
        manual_steps: vec![
            format!("Save the plist to ~/Library/LaunchAgents/{label}.plist."),
            format!(
                "Run `launchctl bootstrap gui/$(id -u) ~/Library/LaunchAgents/{label}.plist` to load the new schedule."
            ),
        ],
        manual_step_details: vec![
            ScheduleManualStep {
                id: "macos-save-plist".to_string(),
                title_key: "schedule.manualMacosSavePlistTitle".to_string(),
                summary_key: "schedule.manualMacosSavePlistSummary".to_string(),
                why_key: "schedule.manualMacosSavePlistWhy".to_string(),
                command: None,
                file_path: Some(format!("~/Library/LaunchAgents/{label}.plist")),
                file_contents: Some(contents),
                directory_path: Some("~/Library/LaunchAgents".to_string()),
                can_auto_run: false,
                can_verify: true,
            },
            ScheduleManualStep {
                id: "macos-load-launch-agent".to_string(),
                title_key: "schedule.manualMacosLoadTitle".to_string(),
                summary_key: "schedule.manualMacosLoadSummary".to_string(),
                why_key: "schedule.manualMacosLoadWhy".to_string(),
                command: Some(vec![
                    "launchctl".to_string(),
                    "bootstrap".to_string(),
                    "gui/$(id -u)".to_string(),
                    format!("~/Library/LaunchAgents/{label}.plist"),
                ]),
                file_path: None,
                file_contents: None,
                directory_path: None,
                can_auto_run: true,
                can_verify: true,
            },
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

pub(super) fn apply_macos_schedule(
    host: &SchedulerHost,
    plan: &SchedulePlan,
    paths: &ProjectPaths,
) -> Result<ApplyResult> {
    let launch_agents_dir = host.launch_agents_dir()?;
    fs::create_dir_all(&launch_agents_dir)?;

    let mut written_files = Vec::new();
    for file in &plan.generated_files {
        let target_path = launch_agents_dir
            .join(PathBuf::from(&file.relative_path).file_name().unwrap_or_default());
        fs::write(&target_path, &file.contents)?;
        written_files.push(target_path.display().to_string());
    }

    let uid = host.launchd_uid()?;
    let plist_path =
        written_files.first().context("missing plist file for macOS schedule apply")?.clone();
    let bootstrap = host.bootstrap_launch_agent(&uid, &plan.label, &plist_path)?;
    let audit_path = audit::write_macos_apply_audit(
        paths,
        plan,
        &plist_path,
        &bootstrap.status_description,
        bootstrap.success,
    )?;

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
        step_results: vec![ScheduleVerificationCheck {
            key: "macos-bootstrap".to_string(),
            status: if bootstrap.success { "ok" } else { "error" }.to_string(),
            label_key: "schedule.verifyMacosBootstrap".to_string(),
            detail_key: if bootstrap.success {
                "schedule.verifyMacosBootstrapOk"
            } else {
                "schedule.verifyMacosBootstrapFailed"
            }
            .to_string(),
            evidence: vec![bootstrap.status_description],
        }],
    })
}

pub(super) fn remove_macos_schedule(
    host: &SchedulerHost,
    plan: &SchedulePlan,
    paths: &ProjectPaths,
) -> Result<ApplyResult> {
    let launch_agents_dir = host.launch_agents_dir()?;
    fs::create_dir_all(&launch_agents_dir)?;

    let uid = host.launchd_uid()?;
    let current_path = generated_plist_target_path(plan, &launch_agents_dir)?;
    let current_unload = host.bootout_launch_agent(&uid, &plan.label)?;

    let mut removed_files = Vec::new();
    if current_path.exists() {
        fs::remove_file(&current_path)?;
        removed_files.push(current_path.display().to_string());
    }

    let launchctl = vec![current_unload.status_description.clone()];
    let applied = !removed_files.is_empty();
    let audit_path = audit::write_macos_remove_audit(
        paths,
        plan,
        &removed_files,
        &launchctl,
        applied || current_unload.success,
    )?;

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
        step_results: vec![ScheduleVerificationCheck {
            key: "macos-remove".to_string(),
            status: if applied || current_unload.success { "ok" } else { "warning" }.to_string(),
            label_key: "schedule.verifyMacosRemove".to_string(),
            detail_key: if applied {
                "schedule.verifyMacosRemoveOk"
            } else {
                "schedule.verifyMacosRemoveNoop"
            }
            .to_string(),
            evidence: vec![current_unload.status_description],
        }],
    })
}

pub(super) fn repair_macos_schedule(
    host: &SchedulerHost,
    plan: &SchedulePlan,
    paths: &ProjectPaths,
) -> Result<ApplyResult> {
    let launch_agents_dir = host.launch_agents_dir()?;
    fs::create_dir_all(&launch_agents_dir)?;
    let uid = host.launchd_uid()?;
    let legacy_agents = detect_legacy_macos_launch_agents(host, &launch_agents_dir);
    let mut removed_files = Vec::new();
    let mut launchctl = Vec::new();

    for agent in legacy_agents {
        let unload = host.bootout_launch_agent(&uid, agent.label)?;
        launchctl.push(unload.status_description);
        if agent.file_present && agent.path.exists() {
            fs::remove_file(&agent.path)?;
            removed_files.push(agent.path.display().to_string());
        }
    }

    let audit_path = audit::write_macos_repair_audit(paths, plan, &removed_files, &launchctl)?;
    let applied = !removed_files.is_empty() || !launchctl.is_empty();
    Ok(ApplyResult {
        applied,
        platform: plan.platform.clone(),
        files: removed_files,
        audit_path: Some(audit_path.display().to_string()),
        message: if applied {
            "Legacy PathKeep LaunchAgent entries were removed.".to_string()
        } else {
            "No legacy PathKeep LaunchAgent entries were found.".to_string()
        },
        step_results: vec![ScheduleVerificationCheck {
            key: "macos-repair-legacy".to_string(),
            status: "ok".to_string(),
            label_key: "schedule.verifyMacosRepairLegacy".to_string(),
            detail_key: if applied {
                "schedule.verifyMacosRepairLegacyOk"
            } else {
                "schedule.verifyMacosRepairLegacyNoop"
            }
            .to_string(),
            evidence: launchctl,
        }],
    })
}

pub(super) fn macos_schedule_status(
    host: &SchedulerHost,
    plan: &SchedulePlan,
    mut status: ScheduleStatus,
) -> Result<ScheduleStatus> {
    let launch_agents_dir = host.launch_agents_dir()?;
    let target_path = generated_plist_target_path(plan, &launch_agents_dir)?;

    if target_path.exists() {
        status.detected_files.push(target_path.display().to_string());
        match fs::read_to_string(&target_path) {
            Ok(contents) => {
                if contents == plan.generated_files[0].contents {
                    if host.launch_agent_loaded(&plan.label) {
                        status.install_state = "installed".to_string();
                        status.verification_checks.push(verification_check(
                            "macos-launch-agent-loaded",
                            "ok",
                            "schedule.verifyMacosLoaded",
                            "schedule.verifyMacosLoadedOk",
                            vec![plan.label.clone()],
                        ));
                    } else {
                        status.install_state = "permission-warning".to_string();
                        status.issues.push(ScheduleIssue {
                            code: "macos-launch-agent-not-loaded".to_string(),
                            severity: "error".to_string(),
                            title_key: "schedule.issueLaunchAgentNotLoadedTitle".to_string(),
                            detail_key: "schedule.issueLaunchAgentNotLoadedDetail".to_string(),
                            consequence_key: "schedule.issueLaunchAgentNotLoadedConsequence"
                                .to_string(),
                            evidence: vec![plan.label.clone()],
                            repair_action: Some("reinstall".to_string()),
                            dismissible: false,
                        });
                        status.verification_checks.push(verification_check(
                            "macos-launch-agent-loaded",
                            "error",
                            "schedule.verifyMacosLoaded",
                            "schedule.verifyMacosLoadedFailed",
                            vec![plan.label.clone()],
                        ));
                    }
                } else {
                    status.install_state = "mismatch".to_string();
                    status
                        .warnings
                        .push("Installed LaunchAgent content no longer matches the current PathKeep schedule plan.".to_string());
                    status.issues.push(ScheduleIssue {
                        code: "macos-plist-mismatch".to_string(),
                        severity: "warning".to_string(),
                        title_key: "schedule.issuePlistMismatchTitle".to_string(),
                        detail_key: "schedule.issuePlistMismatchDetail".to_string(),
                        consequence_key: "schedule.issuePlistMismatchConsequence".to_string(),
                        evidence: vec![target_path.display().to_string()],
                        repair_action: Some("reinstall".to_string()),
                        dismissible: false,
                    });
                    status.verification_checks.push(verification_check(
                        "macos-plist-content",
                        "warning",
                        "schedule.verifyMacosPlist",
                        "schedule.verifyMacosPlistMismatch",
                        vec![target_path.display().to_string()],
                    ));
                };
            }
            Err(error) => {
                status.install_state = "permission-warning".to_string();
                status.warnings.push(format!(
                    "PathKeep could not read the installed LaunchAgent at {}: {error}",
                    target_path.display()
                ));
                status.issues.push(ScheduleIssue {
                    code: "macos-plist-unreadable".to_string(),
                    severity: "error".to_string(),
                    title_key: "schedule.issueInspectionFailedTitle".to_string(),
                    detail_key: "schedule.issueMacosInspectionFailedDetail".to_string(),
                    consequence_key: "schedule.issueInspectionFailedConsequence".to_string(),
                    evidence: vec![target_path.display().to_string()],
                    repair_action: Some("manual-remove".to_string()),
                    dismissible: false,
                });
            }
        }
    } else if host.launch_agent_loaded(&plan.label) {
        status.install_state = "permission-warning".to_string();
        status.issues.push(ScheduleIssue {
            code: "macos-plist-missing-loaded".to_string(),
            severity: "error".to_string(),
            title_key: "schedule.issuePlistMissingLoadedTitle".to_string(),
            detail_key: "schedule.issuePlistMissingLoadedDetail".to_string(),
            consequence_key: "schedule.issuePlistMissingLoadedConsequence".to_string(),
            evidence: vec![format!("LaunchAgent:{}", plan.label)],
            repair_action: Some("reinstall".to_string()),
            dismissible: false,
        });
        status.verification_checks.push(verification_check(
            "macos-plist-content",
            "error",
            "schedule.verifyMacosPlist",
            "schedule.verifyMacosPlistMissingLoaded",
            vec![format!("LaunchAgent:{}", plan.label)],
        ));
        status.verification_checks.push(verification_check(
            "macos-launch-agent-loaded",
            "warning",
            "schedule.verifyMacosLoaded",
            "schedule.verifyMacosLoadedWithoutPlist",
            vec![plan.label.clone()],
        ));
    } else {
        status.verification_checks.push(verification_check(
            "macos-plist-content",
            "pending",
            "schedule.verifyMacosPlist",
            "schedule.verifyMacosPlistMissing",
            Vec::new(),
        ));
    }

    let legacy_agents = detect_legacy_macos_launch_agents(host, &launch_agents_dir);
    if !legacy_agents.is_empty() {
        status.detected_files.extend(legacy_agents.iter().map(LegacyLaunchAgent::detected_value));
        status.install_state = "legacy-install-detected".to_string();
        status.warnings.push(format!(
            "A legacy PathKeep LaunchAgent is still present: {}. Current PathKeep uses `{}` and will not migrate or remove legacy schedules automatically.",
            legacy_agents
                .iter()
                .map(LegacyLaunchAgent::summary)
                .collect::<Vec<_>>()
                .join(", "),
            plan.label
        ));
        status.issues.push(ScheduleIssue {
            code: "legacy-launch-agent".to_string(),
            severity: "warning".to_string(),
            title_key: "schedule.issueLegacyAgentTitle".to_string(),
            detail_key: "schedule.issueLegacyAgentDetail".to_string(),
            consequence_key: "schedule.issueLegacyAgentConsequence".to_string(),
            evidence: legacy_agents.iter().map(LegacyLaunchAgent::detected_value).collect(),
            repair_action: Some("repair-legacy".to_string()),
            dismissible: false,
        });
        status.verification_checks.push(verification_check(
            "macos-legacy-agent",
            "warning",
            "schedule.verifyMacosLegacy",
            "schedule.verifyMacosLegacyFound",
            legacy_agents.iter().map(LegacyLaunchAgent::summary).collect(),
        ));
    }

    Ok(status)
}

pub(super) struct LegacyLaunchAgent {
    pub(super) label: &'static str,
    pub(super) path: PathBuf,
    pub(super) file_present: bool,
    pub(super) loaded: bool,
}

impl LegacyLaunchAgent {
    pub(super) fn detected_value(&self) -> String {
        if self.file_present {
            self.path.display().to_string()
        } else {
            format!("LaunchAgent:{}", self.label)
        }
    }

    pub(super) fn summary(&self) -> String {
        if self.loaded { format!("{} (loaded)", self.label) } else { self.label.to_string() }
    }
}

fn detect_legacy_macos_launch_agents(
    host: &SchedulerHost,
    launch_agents_dir: &Path,
) -> Vec<LegacyLaunchAgent> {
    LEGACY_MACOS_SCHEDULE_LABELS
        .iter()
        .copied()
        .filter_map(|label| {
            let path = launch_agents_dir.join(format!("{label}.plist"));
            let file_present = path.exists();
            // A bare LaunchAgents override (older tests) only redirects files,
            // so asking the real launchd about legacy labels would mix sources.
            let loaded = match host {
                SchedulerHost::Native => {
                    launch_agents_dir_override().is_none() && host.launch_agent_loaded(label)
                }
                SchedulerHost::Sandbox(_) => host.launch_agent_loaded(label),
            };
            (file_present || loaded).then_some(LegacyLaunchAgent {
                label,
                path,
                file_present,
                loaded,
            })
        })
        .collect()
}

fn generated_plist_target_path(plan: &SchedulePlan, launch_agents_dir: &Path) -> Result<PathBuf> {
    let generated =
        plan.generated_files.first().context("missing plist file for macOS schedule plan")?;
    Ok(launch_agents_dir
        .join(PathBuf::from(&generated.relative_path).file_name().unwrap_or_default()))
}

#[cfg(not(test))]
fn native_launch_agents_dir() -> Result<PathBuf> {
    if let Some(path) = launch_agents_dir_override() {
        return Ok(path);
    }
    Ok(UserDirs::new().context("resolving home dir")?.home_dir().join("Library/LaunchAgents"))
}

#[cfg(test)]
fn native_launch_agents_dir() -> Result<PathBuf> {
    Ok(launch_agents_dir_override()
        .unwrap_or_else(|| std::env::temp_dir().join("pathkeep-launch-agents")))
}

#[cfg(not(test))]
fn native_scheduler_uid() -> Result<String> {
    let uid = Command::new("id").arg("-u").output().context("running id -u")?;
    Ok(String::from_utf8_lossy(&uid.stdout).trim().to_string())
}

#[cfg(test)]
fn native_scheduler_uid() -> Result<String> {
    Ok("501".to_string())
}

#[cfg(not(test))]
fn native_launch_agent_loaded(label: &str) -> bool {
    let Ok(uid) = native_scheduler_uid() else {
        return false;
    };
    Command::new("launchctl")
        .args(["print", &format!("gui/{uid}/{label}")])
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

/// Test stand-in for `launchctl print`: runs nothing, and no label is loaded.
#[cfg(test)]
fn native_launch_agent_loaded(_label: &str) -> bool {
    false
}

#[cfg(not(test))]
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

#[cfg(not(test))]
fn native_bootstrap_launch_agent(
    uid: &str,
    label: &str,
    plist_path: &str,
) -> Result<LaunchctlOutcome> {
    let bootout = native_bootout_launch_agent(uid, label)?;
    // A failed bootout is normally harmless when no previous service exists.
    // It is NOT harmless when the old service is still loaded: writing a new
    // plist then reporting bootstrap success leaves the user in an endless
    // "reinstall" / mismatch loop. Preserve the exact launchctl evidence.
    if !bootout.success && native_launch_agent_loaded(label) {
        return Ok(LaunchctlOutcome {
            success: false,
            status_description: format!(
                "could not unload the existing LaunchAgent before reinstalling: {}",
                bootout.status_description
            ),
        });
    }
    let output = Command::new("launchctl")
        .args(["bootstrap", &format!("gui/{uid}"), plist_path])
        .output()
        .context("bootstrapping launch agent")?;
    Ok(LaunchctlOutcome {
        success: output.status.success(),
        status_description: describe_launchctl_output("bootstrap", plist_path, &output),
    })
}

/// Test stand-in for `launchctl bootstrap`: runs nothing and reports failure, so a
/// test that reaches the native layer by mistake cannot read as installed.
#[cfg(test)]
fn native_bootstrap_launch_agent(
    uid: &str,
    _label: &str,
    plist_path: &str,
) -> Result<LaunchctlOutcome> {
    Ok(LaunchctlOutcome {
        success: false,
        status_description: format!(
            "launchctl bootstrap gui/{uid} {plist_path} is not run in tests"
        ),
    })
}

#[cfg(not(test))]
fn native_bootout_launch_agent(uid: &str, label: &str) -> Result<LaunchctlOutcome> {
    let target = format!("gui/{uid}/{label}");
    let output = Command::new("launchctl")
        .args(["bootout", &target])
        .output()
        .context("unloading launch agent")?;
    Ok(LaunchctlOutcome {
        success: output.status.success(),
        status_description: describe_launchctl_output("bootout", &target, &output),
    })
}

/// Test stand-in for `launchctl bootout`: runs nothing and reports failure.
#[cfg(test)]
fn native_bootout_launch_agent(uid: &str, label: &str) -> Result<LaunchctlOutcome> {
    Ok(LaunchctlOutcome {
        success: false,
        status_description: format!("launchctl bootout gui/{uid}/{label} is not run in tests"),
    })
}

struct LaunchctlOutcome {
    success: bool,
    status_description: String,
}

/// launchd access for one scheduler operation: native, or the sandbox emulator.
impl SchedulerHost {
    fn launch_agents_dir(&self) -> Result<PathBuf> {
        match self {
            Self::Native => native_launch_agents_dir(),
            Self::Sandbox(root) => Ok(root.join(LAUNCH_AGENTS_SUBDIR)),
        }
    }

    fn launchd_uid(&self) -> Result<String> {
        match self {
            Self::Native => {
                count_native_call();
                native_scheduler_uid()
            }
            Self::Sandbox(_) => Ok("sandbox".to_string()),
        }
    }

    fn launch_agent_loaded(&self, label: &str) -> bool {
        match self {
            Self::Native => {
                count_native_call();
                native_launch_agent_loaded(label)
            }
            Self::Sandbox(root) => sandbox_loaded_marker(root, label).is_file(),
        }
    }

    fn bootstrap_launch_agent(
        &self,
        uid: &str,
        label: &str,
        plist_path: &str,
    ) -> Result<LaunchctlOutcome> {
        match self {
            Self::Native => {
                count_native_call();
                native_bootstrap_launch_agent(uid, label, plist_path)
            }
            Self::Sandbox(root) => sandbox_bootstrap_launch_agent(root, label, plist_path),
        }
    }

    fn bootout_launch_agent(&self, uid: &str, label: &str) -> Result<LaunchctlOutcome> {
        match self {
            Self::Native => {
                count_native_call();
                native_bootout_launch_agent(uid, label)
            }
            Self::Sandbox(root) => sandbox_bootout_launch_agent(root, label),
        }
    }
}

fn sandbox_loaded_marker(root: &Path, label: &str) -> PathBuf {
    root.join(LAUNCHD_LOADED_SUBDIR).join(label)
}

/// Emulates `launchctl bootstrap`: the plist must parse and carry the label,
/// as launchd requires, and the label then counts as loaded until booted out.
fn sandbox_bootstrap_launch_agent(
    root: &Path,
    label: &str,
    plist_path: &str,
) -> Result<LaunchctlOutcome> {
    let plist_label = plist::Value::from_file(plist_path).ok().and_then(|value| {
        value
            .as_dictionary()
            .and_then(|dict| dict.get("Label"))
            .and_then(plist::Value::as_string)
            .map(str::to_string)
    });
    if plist_label.as_deref() != Some(label) {
        return Ok(LaunchctlOutcome {
            success: false,
            status_description: format!(
                "sandbox bootstrap {plist_path}: not a launchd plist with Label {label}"
            ),
        });
    }
    let marker = sandbox_loaded_marker(root, label);
    super::ensure_parent_dir(&marker)?;
    fs::write(&marker, plist_path)
        .with_context(|| format!("writing sandbox launchd marker {}", marker.display()))?;
    Ok(LaunchctlOutcome {
        success: true,
        status_description: format!("sandbox bootstrap {label} {plist_path}"),
    })
}

/// Emulates `launchctl bootout`, which fails when the label is not loaded.
fn sandbox_bootout_launch_agent(root: &Path, label: &str) -> Result<LaunchctlOutcome> {
    let marker = sandbox_loaded_marker(root, label);
    if !marker.is_file() {
        return Ok(LaunchctlOutcome {
            success: false,
            status_description: format!("sandbox bootout {label}: no such process"),
        });
    }
    fs::remove_file(&marker)
        .with_context(|| format!("removing sandbox launchd marker {}", marker.display()))?;
    Ok(LaunchctlOutcome { success: true, status_description: format!("sandbox bootout {label}") })
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
