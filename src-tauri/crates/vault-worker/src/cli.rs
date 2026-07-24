//! Worker CLI routing.
//!
//! The desktop binary can launch this crate in `--worker` mode. That path is a
//! thin transport boundary over the same worker functions the Tauri facade
//! uses, so this module only dispatches commands and serializes the result.

use crate::{
    archive_flows::{doctor_report, run_backup_now, run_scheduled_backup},
    context::load_background_backup_config,
    intelligence::{build_ai_index_now, run_ai_queue_jobs},
    mcp::run_mcp_stdio_server,
    security::read_database_key_from_keyring,
};
use anyhow::{Error, Result, anyhow};
use vault_core::{
    AiIndexRequest, ScheduleAttemptRecorder as AttemptRecorder, ScheduledBackupAttemptOutcome,
    ScheduledBackupAttemptPhase,
};

/// Executes one worker CLI command and returns a JSON payload when applicable.
pub fn run_worker_cli(arguments: &[String]) -> Result<String> {
    let command = arguments.first().map(String::as_str).unwrap_or("snapshot");
    match command {
        "backup" => {
            let due_only = arguments.iter().any(|arg| arg == "--due-only");
            if due_only {
                return run_scheduled_backup_cli();
            }
            let key = read_database_key_from_keyring()?;
            let report = run_backup_now(key.as_deref(), due_only)?;
            Ok(serde_json::to_string_pretty(&report)?)
        }
        "doctor" => {
            let key = read_database_key_from_keyring()?;
            let report = doctor_report(key.as_deref())?;
            Ok(serde_json::to_string_pretty(&report)?)
        }
        "ai-index" => {
            let key = read_database_key_from_keyring()?;
            let report = build_ai_index_now(key.as_deref(), &AiIndexRequest::default())?;
            Ok(serde_json::to_string_pretty(&report)?)
        }
        "ai-queue" => {
            let key = read_database_key_from_keyring()?;
            let status = run_ai_queue_jobs(key.as_deref(), None)?;
            Ok(serde_json::to_string_pretty(&status)?)
        }
        "mcp-server" => {
            run_mcp_stdio_server()?;
            Ok(String::new())
        }
        other => anyhow::bail!("unknown worker command: {other}"),
    }
}

fn run_scheduled_backup_cli() -> Result<String> {
    let paths = vault_core::project_paths()?;
    let mut attempt = AttemptRecorder::begin(&paths)?;

    attempt.advance(ScheduledBackupAttemptPhase::Keyring)?;
    let key = match read_database_key_from_keyring() {
        Ok(key) => key,
        Err(error) => {
            return Err(finalize_attempt_failure(&mut attempt, "keyring", &error));
        }
    };

    attempt.advance(ScheduledBackupAttemptPhase::Config)?;
    let config = match load_background_backup_config(&paths) {
        Ok(config) => config,
        Err(error) => {
            return Err(finalize_attempt_failure(&mut attempt, "config", &error));
        }
    };

    attempt.advance(ScheduledBackupAttemptPhase::Backup)?;
    let report = match run_scheduled_backup(&paths, &config, key.as_deref()) {
        Ok(report) => report,
        Err(error) => {
            let reason_code = classify_backup_failure(&error);
            return Err(finalize_attempt_failure(&mut attempt, reason_code, &error));
        }
    };

    let (outcome, reason_code) =
        classify_scheduled_report(report.due_skipped, report.reason_code.as_deref());
    attempt.finish(
        outcome,
        Some(reason_code),
        report.reason.as_deref(),
        report.run.as_ref().map(|run| run.id),
    )?;
    Ok(serde_json::to_string_pretty(&report)?)
}

fn classify_scheduled_report(
    due_skipped: bool,
    reason_code: Option<&str>,
) -> (ScheduledBackupAttemptOutcome, &'static str) {
    if !due_skipped {
        return (ScheduledBackupAttemptOutcome::Success, "backup-succeeded");
    }
    match reason_code {
        Some("write-lock") => (ScheduledBackupAttemptOutcome::Deferred, "write-lock"),
        Some("not-due") => (ScheduledBackupAttemptOutcome::Skipped, "not-due"),
        _ => (ScheduledBackupAttemptOutcome::Skipped, "worker-error"),
    }
}

fn classify_backup_failure(error: &anyhow::Error) -> &'static str {
    let detail = format!("{error:#}").to_ascii_lowercase();
    if detail.contains("app lock") || detail.contains("pathkeep is locked") {
        return "app-lock";
    }
    if detail.contains("opening canonical archive")
        || detail.contains("opening source-evidence")
        || detail.contains("archive database")
        || detail.contains("sqlcipher")
        || detail.contains("not a database")
    {
        return "archive-open";
    }
    "worker-error"
}

fn finalize_attempt_failure(
    attempt: &mut AttemptRecorder,
    reason_code: &str,
    error: &Error,
) -> Error {
    let detail = format!("{error:#}");
    match attempt.finish(
        ScheduledBackupAttemptOutcome::Failed,
        Some(reason_code),
        Some(&detail),
        None,
    ) {
        Ok(()) => anyhow!(detail),
        Err(ledger_error) => anyhow!(
            "{detail}; additionally failed to persist scheduled-backup failure evidence: {ledger_error:#}"
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::{classify_backup_failure, classify_scheduled_report, finalize_attempt_failure};
    use crate::tests::{PROJECT_ROOT_OVERRIDE_ENV, lock_env, restore_env_var};
    use std::fs;
    use tempfile::tempdir;
    use vault_core::{ScheduleAttemptRecorder, ScheduledBackupAttemptOutcome, project_paths};

    #[test]
    fn scheduled_report_classification_covers_success_skip_defer_and_unknown_reasons() {
        assert_eq!(
            classify_scheduled_report(false, None),
            (ScheduledBackupAttemptOutcome::Success, "backup-succeeded")
        );
        assert_eq!(
            classify_scheduled_report(true, Some("write-lock")),
            (ScheduledBackupAttemptOutcome::Deferred, "write-lock")
        );
        assert_eq!(
            classify_scheduled_report(true, Some("not-due")),
            (ScheduledBackupAttemptOutcome::Skipped, "not-due")
        );
        assert_eq!(
            classify_scheduled_report(true, Some("unexpected")),
            (ScheduledBackupAttemptOutcome::Skipped, "worker-error")
        );
    }

    #[test]
    fn backup_failure_classification_preserves_stable_reason_codes() {
        assert_eq!(
            classify_backup_failure(&anyhow::anyhow!("PathKeep is locked by App Lock")),
            "app-lock"
        );
        assert_eq!(
            classify_backup_failure(&anyhow::anyhow!("opening canonical archive: not a database")),
            "archive-open"
        );
        assert_eq!(
            classify_backup_failure(&anyhow::anyhow!("unexpected worker failure")),
            "worker-error"
        );
    }

    #[test]
    fn finalization_reports_when_failure_evidence_cannot_be_persisted() {
        let _guard = lock_env();
        let root = tempdir().expect("tempdir");
        let original_project_root = std::env::var_os(PROJECT_ROOT_OVERRIDE_ENV);
        unsafe {
            std::env::set_var(PROJECT_ROOT_OVERRIDE_ENV, root.path());
        }

        let paths = project_paths().expect("paths");
        let mut recorder = ScheduleAttemptRecorder::begin(&paths).expect("begin attempt");
        let attempts_path = paths.schedule_dir.join("attempts");
        fs::remove_dir_all(&attempts_path).expect("remove attempts directory");
        fs::write(&attempts_path, b"block attempt persistence").expect("block attempt directory");

        let error = finalize_attempt_failure(
            &mut recorder,
            "worker-error",
            &anyhow::anyhow!("original scheduled worker failure"),
        );
        let rendered = format!("{error:#}");
        assert!(rendered.contains("original scheduled worker failure"));
        assert!(rendered.contains("additionally failed to persist"));

        restore_env_var(PROJECT_ROOT_OVERRIDE_ENV, original_project_root.as_deref());
    }
}
