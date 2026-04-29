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
//! - `crate::test_support` environment overrides for deterministic tests.
//! - `audit` for apply/remove/repair audit artifacts.
//!
//! ## Performance notes
//! - Status reads inspect only the canonical plist and the known legacy labels;
//!   they never enumerate all LaunchAgents.

use anyhow::{Context, Result};
#[cfg(not(any(test, coverage)))]
use directories::UserDirs;
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

use super::{
    LEGACY_MACOS_SCHEDULE_LABELS, ScheduleParameters, audit, format_interval_label,
    interval_minutes_from_hours, interval_seconds_from_hours,
};
use crate::test_support::launch_agents_dir_override;
#[cfg(any(test, coverage))]
use crate::test_support::launchctl_stub_success;

#[cfg(any(test, coverage))]
const TEST_LAUNCHCTL_LOADED_LABELS_ENV: &str = "PATHKEEP_TEST_LAUNCHCTL_LOADED_LABELS";

pub(super) fn macos_schedule_plan(
    label: &str,
    executable_path: &Path,
    worker_args: &[String],
    log_dir: &Path,
    params: &ScheduleParameters,
) -> Result<SchedulePlan> {
    let check_interval_minutes = interval_minutes_from_hours(params.check_interval_hours);
    let check_interval_label = format_interval_label(check_interval_minutes);
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
    plan: &SchedulePlan,
    paths: &ProjectPaths,
) -> Result<ApplyResult> {
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
    let audit_path =
        audit::write_macos_apply_audit(paths, plan, &plist_path, &bootstrap.status_description)?;

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
    plan: &SchedulePlan,
    paths: &ProjectPaths,
) -> Result<ApplyResult> {
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

    let launchctl = vec![current_unload.status_description.clone()];
    let audit_path = audit::write_macos_remove_audit(paths, plan, &removed_files, &launchctl)?;
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
    plan: &SchedulePlan,
    paths: &ProjectPaths,
) -> Result<ApplyResult> {
    let launch_agents_dir = launch_agents_dir()?;
    fs::create_dir_all(&launch_agents_dir)?;
    let uid = scheduler_uid()?;
    let legacy_agents = detect_legacy_macos_launch_agents(&launch_agents_dir);
    let mut removed_files = Vec::new();
    let mut launchctl = Vec::new();

    for agent in legacy_agents {
        let unload = bootout_launch_agent(&uid, agent.label)?;
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
    plan: &SchedulePlan,
    mut status: ScheduleStatus,
) -> Result<ScheduleStatus> {
    let launch_agents_dir = launch_agents_dir()?;
    let target_path = generated_plist_target_path(plan, &launch_agents_dir)?;

    if target_path.exists() {
        status.detected_files.push(target_path.display().to_string());
        match fs::read_to_string(&target_path) {
            Ok(contents) => {
                if contents == plan.generated_files[0].contents {
                    if macos_launch_agent_loaded(&plan.label) {
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
    } else if macos_launch_agent_loaded(&plan.label) {
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

    let legacy_agents = detect_legacy_macos_launch_agents(&launch_agents_dir);
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

fn detect_legacy_macos_launch_agents(launch_agents_dir: &Path) -> Vec<LegacyLaunchAgent> {
    LEGACY_MACOS_SCHEDULE_LABELS
        .iter()
        .copied()
        .filter_map(|label| {
            let path = launch_agents_dir.join(format!("{label}.plist"));
            let file_present = path.exists();
            let loaded = launch_agents_dir_override().is_none() && macos_launch_agent_loaded(label);
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
fn macos_launch_agent_loaded(label: &str) -> bool {
    let Ok(uid) = scheduler_uid() else {
        return false;
    };
    Command::new("launchctl")
        .args(["print", &format!("gui/{uid}/{label}")])
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

#[cfg(any(test, coverage))]
fn macos_launch_agent_loaded(label: &str) -> bool {
    std::env::var(TEST_LAUNCHCTL_LOADED_LABELS_ENV)
        .ok()
        .map(|labels| labels.split(',').any(|candidate| candidate == label))
        .unwrap_or(false)
}

#[cfg(any(test, coverage))]
fn update_stub_loaded_label(label: &str, loaded: bool) {
    let mut labels = std::env::var(TEST_LAUNCHCTL_LOADED_LABELS_ENV)
        .ok()
        .map(|current| {
            current
                .split(',')
                .filter(|candidate| !candidate.is_empty() && *candidate != label)
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    if loaded {
        labels.push(label.to_string());
    }
    unsafe {
        if labels.is_empty() {
            std::env::remove_var(TEST_LAUNCHCTL_LOADED_LABELS_ENV);
        } else {
            std::env::set_var(TEST_LAUNCHCTL_LOADED_LABELS_ENV, labels.join(","));
        }
    }
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
    let success = launchctl_stub_success();
    if success {
        update_stub_loaded_label(label, true);
    }
    Ok(LaunchctlOutcome {
        success,
        status_description: format!("stub bootstrap gui/{uid} {plist_path}"),
    })
}

#[cfg(not(any(test, coverage)))]
fn bootout_launch_agent(uid: &str, label: &str) -> Result<LaunchctlOutcome> {
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

#[cfg(any(test, coverage))]
fn bootout_launch_agent(uid: &str, label: &str) -> Result<LaunchctlOutcome> {
    let success = launchctl_stub_success();
    if success {
        update_stub_loaded_label(label, false);
    }
    Ok(LaunchctlOutcome { success, status_description: format!("stub bootout gui/{uid} {label}") })
}

struct LaunchctlOutcome {
    success: bool,
    status_description: String,
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
pub(super) fn loaded_labels_env_name() -> &'static str {
    TEST_LAUNCHCTL_LOADED_LABELS_ENV
}
