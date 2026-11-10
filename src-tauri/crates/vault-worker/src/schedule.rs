//! Schedule worker flows.
//!
//! These helpers bridge the schedule UI/CLI surface to `vault-platform`'s
//! native scheduler adapters while keeping the product contract centered on
//! preview/manual/execute/verify.

use crate::context::load_unlocked_config;
use anyhow::{Context, Result};
use std::path::PathBuf;
use vault_core::{
    ScheduleIssue, SchedulePlan, ScheduleStatus, load_config, load_schedule_attempt_ledger,
};
use vault_platform::{
    ScheduleParameters, apply_schedule, preview_schedule, remove_schedule, repair_schedule,
    schedule_status as detect_schedule_status,
};

/// Builds a platform-specific schedule plan without installing anything.
///
/// `due_after_hours` previews another interval than the saved one, so the UI
/// can show the exact file for an interval before the user commits to it.
/// Nothing is saved: the caller saves the interval only when the user
/// confirms, and then applies the plan it showed.
pub fn preview_schedule_plan(
    platform: Option<&str>,
    executable_path: Option<PathBuf>,
    due_after_hours: Option<f64>,
) -> Result<SchedulePlan> {
    let paths = vault_core::project_paths()?;
    let mut config = load_config(&paths)?;
    if let Some(hours) = due_after_hours {
        config.due_after_hours = validated_due_after_hours(hours)?;
    }
    let executable = executable_path
        .or_else(|| std::env::current_exe().ok())
        .context("resolving executable path for schedule preview")?;
    preview_schedule(platform, executable.as_path(), &paths, &ScheduleParameters::from(&config))
}

/// Longest interval the schedule preview accepts: 30 days.
pub const MAX_DUE_AFTER_HOURS: f64 = 30.0 * 24.0;

/// Rounds a requested interval to whole minutes and rejects values the
/// scheduler cannot express (under one minute, over 30 days, not a number).
fn validated_due_after_hours(hours: f64) -> Result<f64> {
    let minutes = (hours * 60.0).round();
    if !(1.0..=MAX_DUE_AFTER_HOURS * 60.0).contains(&minutes) {
        anyhow::bail!(
            "the backup interval must be between 1 minute and 30 days, got {hours} hours"
        );
    }
    Ok(minutes / 60.0)
}

/// Applies a native schedule plan.
pub fn apply_schedule_plan(plan: &SchedulePlan) -> Result<vault_core::ApplyResult> {
    let paths = vault_core::project_paths()?;
    apply_schedule(plan, &paths)
}

/// Removes a previously applied native schedule plan.
pub fn remove_schedule_plan(plan: &SchedulePlan) -> Result<vault_core::ApplyResult> {
    let paths = vault_core::project_paths()?;
    remove_schedule(plan, &paths)
}

/// Repairs known scheduler conflicts after explicit user confirmation.
pub fn repair_schedule_plan(plan: &SchedulePlan) -> Result<vault_core::ApplyResult> {
    let paths = vault_core::project_paths()?;
    repair_schedule(plan, &paths)
}

/// Adds archive backup history and a due-clock estimate to native schedule status.
pub fn schedule_status(
    vault: Option<&vault_core::Vault>,
    platform: Option<&str>,
    executable_path: Option<PathBuf>,
) -> Result<ScheduleStatus> {
    let paths = vault_core::project_paths()?;
    let config = load_unlocked_config(&paths)?;
    let executable = executable_path
        .or_else(|| std::env::current_exe().ok())
        .context("resolving executable path for schedule status")?;
    let params = ScheduleParameters::from(&config);
    let mut status = detect_schedule_status(platform, executable.as_path(), &paths, &params)?;
    status.last_successful_backup_at = vault
        .map(vault_core::archive_status)
        .transpose()?
        .and_then(|status| status.last_successful_backup_at);
    let awake_since = vault_platform::awake_since();
    let ledger =
        load_schedule_attempt_ledger(&paths, native_schedule_interval_hours(&config), awake_since)?;
    if status.install_state == "installed" {
        // When it was installed, if the audit trail recorded it.
        let installed_at = status
            .last_action
            .as_ref()
            .filter(|action| action.action == "apply" && action.status == "ok")
            .map(|action| action.at.as_str());
        let now = chrono::Utc::now();
        if let Some(vault) = vault {
            let wake_interval = params.wake_interval(platform);
            status.next_scheduled_backup_at = vault_core::next_scheduled_backup_at(
                vault_core::backup_due_anchor(vault)?,
                &ledger.attempts,
                installed_at,
                params.due_after_hours,
                wake_interval,
                now,
            )
            .map(|at| at.to_rfc3339());
            status.wake_interval_seconds = Some(wake_interval.as_secs());
        }
        status.issues.extend(
            vault_core::overdue_issue(
                &ledger.attempts,
                installed_at,
                native_schedule_interval_hours(&config),
                awake_since,
                now,
            )
            .map(schedule_attempt_issue),
        );
    }
    status.last_scheduled_success_at = ledger.last_success_at;
    status.recent_attempts = ledger.attempts;
    status.issues.extend(ledger.health_issues.into_iter().map(schedule_attempt_issue));
    Ok(status)
}

fn schedule_attempt_issue(issue: vault_core::ScheduledBackupHealthIssue) -> ScheduleIssue {
    let (title_key, detail_key, consequence_key) = match issue.code.as_str() {
        "latest-attempt-incomplete" => (
            "schedule.issueScheduleAttemptIncompleteTitle",
            "schedule.issueScheduleAttemptIncompleteDetail",
            "schedule.issueScheduleAttemptIncompleteConsequence",
        ),
        "latest-attempt-failed" => (
            "schedule.issueScheduleAttemptFailedTitle",
            "schedule.issueScheduleAttemptFailedDetail",
            "schedule.issueScheduleAttemptFailedConsequence",
        ),
        "worker-interrupted" => (
            "schedule.issueScheduleAttemptInterruptedTitle",
            "schedule.issueScheduleAttemptInterruptedDetail",
            "schedule.issueScheduleAttemptInterruptedConsequence",
        ),
        "scheduled-backup-overdue" => (
            "schedule.issueScheduleOverdueTitle",
            "schedule.issueScheduleOverdueDetail",
            "schedule.issueScheduleOverdueConsequence",
        ),
        _ => (
            "schedule.issueScheduleAttemptLedgerUnreadableTitle",
            "schedule.issueScheduleAttemptLedgerUnreadableDetail",
            "schedule.issueScheduleAttemptLedgerUnreadableConsequence",
        ),
    };
    ScheduleIssue {
        code: issue.code,
        severity: issue.severity,
        title_key: title_key.to_string(),
        detail_key: detail_key.to_string(),
        consequence_key: consequence_key.to_string(),
        evidence: issue.evidence,
        repair_action: None,
        dismissible: false,
    }
}

/// Requested check interval for schedule health; the native grid may wake more often on Linux.
pub(crate) fn native_schedule_interval_hours(config: &vault_core::AppConfig) -> f64 {
    ScheduleParameters::from(config).check_interval_hours
}

#[cfg(test)]
mod tests;
