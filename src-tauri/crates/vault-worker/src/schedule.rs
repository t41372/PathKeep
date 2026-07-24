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
pub fn preview_schedule_plan(
    platform: Option<&str>,
    executable_path: Option<PathBuf>,
) -> Result<SchedulePlan> {
    let paths = vault_core::project_paths()?;
    let config = load_config(&paths)?;
    let executable = executable_path
        .or_else(|| std::env::current_exe().ok())
        .context("resolving executable path for schedule preview")?;
    preview_schedule(
        platform,
        executable.as_path(),
        &paths,
        &ScheduleParameters {
            due_after_hours: config.due_after_hours,
            check_interval_hours: native_schedule_interval_hours(&config),
        },
    )
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

/// Loads the current schedule status and annotates it with the last successful backup.
pub fn schedule_status(
    session_database_key: Option<&str>,
    platform: Option<&str>,
    executable_path: Option<PathBuf>,
) -> Result<ScheduleStatus> {
    let paths = vault_core::project_paths()?;
    let config = load_unlocked_config(&paths)?;
    let executable = executable_path
        .or_else(|| std::env::current_exe().ok())
        .context("resolving executable path for schedule status")?;
    let mut status = detect_schedule_status(
        platform,
        executable.as_path(),
        &paths,
        &ScheduleParameters {
            due_after_hours: config.due_after_hours,
            check_interval_hours: native_schedule_interval_hours(&config),
        },
    )?;
    status.last_successful_backup_at =
        vault_core::archive_status(&paths, &config, session_database_key)?
            .last_successful_backup_at;
    let ledger = load_schedule_attempt_ledger(&paths, native_schedule_interval_hours(&config))?;
    status.last_scheduled_success_at = ledger.last_success_at;
    status.recent_attempts = ledger.attempts;
    status.issues.extend(ledger.health_issues.into_iter().map(schedule_attempt_issue));
    Ok(status)
}

fn schedule_attempt_issue(issue: vault_core::ScheduledBackupHealthIssue) -> ScheduleIssue {
    let (title_key, detail_key, consequence_key) = match issue.code.as_str() {
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

fn native_schedule_interval_hours(config: &vault_core::AppConfig) -> f64 {
    let configured_check_interval = config.schedule_check_interval_hours as f64;
    if config.due_after_hours.is_finite() && config.due_after_hours > 0.0 {
        config.due_after_hours.min(configured_check_interval)
    } else {
        configured_check_interval
    }
}

#[cfg(test)]
mod tests {
    use super::{native_schedule_interval_hours, schedule_attempt_issue};
    use vault_core::AppConfig;

    #[test]
    fn native_schedule_interval_tracks_custom_due_interval_without_exceeding_health_check() {
        let custom_minutes = AppConfig { due_after_hours: 1.5, ..AppConfig::default() };
        let longer_due = AppConfig { due_after_hours: 72.0, ..AppConfig::default() };
        let invalid_due = AppConfig { due_after_hours: 0.0, ..AppConfig::default() };

        assert_eq!(native_schedule_interval_hours(&custom_minutes), 1.5);
        assert_eq!(native_schedule_interval_hours(&longer_due), 6.0);
        assert_eq!(native_schedule_interval_hours(&invalid_due), 6.0);
    }

    #[test]
    fn schedule_attempt_health_uses_stable_existing_issue_contract() {
        let issue = schedule_attempt_issue(vault_core::ScheduledBackupHealthIssue {
            code: "latest-attempt-failed".to_string(),
            severity: "error".to_string(),
            detected_at: "2026-07-23T00:00:00Z".to_string(),
            related_attempt_id: Some("attempt-1".to_string()),
            evidence: vec!["keyring unavailable".to_string()],
        });
        assert_eq!(issue.code, "latest-attempt-failed");
        assert_eq!(issue.title_key, "schedule.issueScheduleAttemptFailedTitle");
        assert_eq!(issue.evidence, vec!["keyring unavailable"]);
        assert!(!issue.dismissible);
    }

    #[test]
    fn schedule_attempt_health_maps_interruption_and_ledger_failures() {
        let interrupted = schedule_attempt_issue(vault_core::ScheduledBackupHealthIssue {
            code: "worker-interrupted".to_string(),
            severity: "error".to_string(),
            detected_at: "2026-07-23T00:00:00Z".to_string(),
            related_attempt_id: Some("attempt-2".to_string()),
            evidence: vec!["worker stopped during backup".to_string()],
        });
        assert_eq!(interrupted.title_key, "schedule.issueScheduleAttemptInterruptedTitle");
        assert_eq!(interrupted.detail_key, "schedule.issueScheduleAttemptInterruptedDetail");
        assert_eq!(
            interrupted.consequence_key,
            "schedule.issueScheduleAttemptInterruptedConsequence"
        );

        let unreadable = schedule_attempt_issue(vault_core::ScheduledBackupHealthIssue {
            code: "attempt-ledger-unreadable".to_string(),
            severity: "error".to_string(),
            detected_at: "2026-07-23T00:00:00Z".to_string(),
            related_attempt_id: None,
            evidence: vec!["invalid attempt json".to_string()],
        });
        assert_eq!(unreadable.title_key, "schedule.issueScheduleAttemptLedgerUnreadableTitle");
        assert_eq!(unreadable.detail_key, "schedule.issueScheduleAttemptLedgerUnreadableDetail");
        assert_eq!(
            unreadable.consequence_key,
            "schedule.issueScheduleAttemptLedgerUnreadableConsequence"
        );
    }
}
