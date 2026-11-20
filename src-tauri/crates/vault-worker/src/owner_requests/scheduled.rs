//! Responsibilities: preserve scheduled backup attempt evidence inside the owner.
//! Not responsible for: CLI transport or database implementation.
//! Failure modes: keyring/config/archive failure leaves no evidence; due skip or write
//! contention is misclassified. Tests: below and owner_requests/tests.rs.
use crate::{
    archive_flows::run_scheduled_backup_with_progress, context::load_background_backup_config,
};
use anyhow::{Error, Result};
use vault_core::{
    ScheduleAttemptRecorder as AttemptRecorder, ScheduledBackupAttemptOutcome,
    ScheduledBackupAttemptPhase,
};
pub(super) fn run_scheduled_backup_cli(
    progress: &vault_owner_ipc::Progress,
    session: Option<&crate::VaultSession>,
    vault: &impl Fn() -> Result<vault_core::Vault>,
    session_key: Option<&str>,
) -> Result<String> {
    // The attempt ledger is written before the config is read, so that a failure to read it is
    // on record. An archive this build may not use gets no record: nothing is written into it.
    let paths = crate::context::usable_root()?;
    let mut attempt = AttemptRecorder::begin(&paths)?;

    attempt.advance(ScheduledBackupAttemptPhase::Keyring)?;
    let key = session_key;

    attempt.advance(ScheduledBackupAttemptPhase::Config)?;
    let config = match load_background_backup_config(&paths) {
        Ok(config) => config,
        Err(error) => {
            return Err(finalize_attempt_failure(&mut attempt, "config", error));
        }
    };

    let capture = session.map(|s| crate::data_kinds::backups::start_shared(s, true));
    let finish = move |attempt: &mut AttemptRecorder,
                       report: vault_core::BackupReport,
                       outcome,
                       reason: &str|
          -> Result<String> {
        use ScheduledBackupAttemptOutcome::*;
        let agent=capture.map(|r|r.and_then(|job|job.wait()).unwrap_or_else(|e|{
            log::warn!("Scheduled conversation capture failed: {e:#}");
            serde_json::json!({"kind":"agent-history","result":"failed","reason":"kind-unavailable"})
        }));
        let (outcome, reason) = match agent.as_ref().and_then(|r| r["result"].as_str()) {
            Some("success") if matches!(outcome, Skipped | Deferred) => {
                (Success, "backup-succeeded")
            }
            Some("partial" | "failed" | "interrupted") if matches!(outcome, Skipped | Deferred) => {
                (Failed, "backup-incomplete")
            }
            Some("failed" | "partial" | "interrupted") => (Partial, "backup-incomplete"),
            _ => (outcome, reason),
        };
        attempt.finish(
            outcome,
            Some(reason),
            report.reason.as_deref(),
            report.run.as_ref().map(|r| r.id),
        )?;
        let mut value = serde_json::to_value(report)?;
        if let Some(agent) = agent {
            value["agentHistory"] = agent;
        }
        Ok(serde_json::to_string(&value)?)
    };
    if let Some(reason) =
        config.capture_policy.backup_skip_reason(vault_core::DataKind::Browsing, false)
    {
        return finish(
            &mut attempt,
            vault_core::BackupReport {
                due_skipped: true,
                reason_code: Some(reason.into()),
                ..Default::default()
            },
            ScheduledBackupAttemptOutcome::Skipped,
            reason,
        );
    }
    attempt.advance(ScheduledBackupAttemptPhase::Backup)?;
    let vault = match vault() {
        Ok(vault) => vault,
        Err(error) => {
            return Err(finalize_attempt_failure(
                &mut attempt,
                if error.to_string().starts_with("keychain-unavailable:") {
                    "keyring"
                } else {
                    "archive-open"
                },
                error,
            ));
        }
    };
    let report = match run_scheduled_backup_with_progress(&vault, &paths, key, |event| {
        progress.send(serde_json::to_value(event).expect("backup progress is serializable"));
    }) {
        Ok(report) => report,
        Err(error) if error.downcast_ref::<vault_core::BackupAlreadyRunning>().is_some() => {
            return finish(
                &mut attempt,
                vault_core::BackupReport {
                    due_skipped: true,
                    reason_code: Some("write-lock".into()),
                    reason: Some("A backup is already running.".into()),
                    ..Default::default()
                },
                ScheduledBackupAttemptOutcome::Deferred,
                "write-lock",
            );
        }
        Err(error) => {
            let reason_code = classify_backup_failure(&error);
            return Err(finalize_attempt_failure(&mut attempt, reason_code, error));
        }
    };

    let (outcome, reason_code) = if report.run.as_ref().is_some_and(|run| run.status == "partial") {
        (ScheduledBackupAttemptOutcome::Partial, "backup-incomplete")
    } else {
        classify_scheduled_report(report.due_skipped, report.reason_code.as_deref())
    };
    finish(&mut attempt, report, outcome, reason_code)
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
        Some("kind-off") => (ScheduledBackupAttemptOutcome::Skipped, "kind-off"),
        Some("kind-paused") => (ScheduledBackupAttemptOutcome::Skipped, "kind-paused"),
        Some("not-due") => (ScheduledBackupAttemptOutcome::Skipped, "not-due"),
        _ => (ScheduledBackupAttemptOutcome::Skipped, "worker-error"),
    }
}

fn classify_backup_failure(error: &anyhow::Error) -> &'static str {
    if error.chain().any(|cause| {
        matches!(
            cause.downcast_ref::<vault_store::Error>(),
            Some(vault_store::Error::OwnedElsewhere { .. } | vault_store::Error::Busy { .. })
        )
    }) {
        return "write-lock";
    }
    if error.chain().any(|cause| {
        cause
            .downcast_ref::<std::io::Error>()
            .is_some_and(|error| error.kind() == std::io::ErrorKind::StorageFull)
    }) {
        return "disk-full";
    }
    let detail = format!("{error:#}").to_ascii_lowercase();
    if detail.contains("disk full")
        || detail.contains("no space left")
        || detail.contains("database or disk is full")
    {
        return "disk-full";
    }
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
    error: Error,
) -> Error {
    let detail = format!("{error:#}");
    match attempt.finish(
        ScheduledBackupAttemptOutcome::Failed,
        Some(reason_code),
        Some(&detail),
        None,
    ) {
        Ok(()) => error,
        Err(ledger_error) => error.context(format!(
            "additionally failed to persist scheduled-backup failure evidence: {ledger_error:#}"
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::finalize_attempt_failure;
    use crate::tests::{PROJECT_ROOT_OVERRIDE_ENV, lock_env, restore_env_var};
    use std::fs;
    use tempfile::tempdir;
    use vault_core::{ScheduleAttemptRecorder, project_paths};

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
            anyhow::anyhow!("original scheduled worker failure"),
        );
        let rendered = format!("{error:#}");
        assert!(rendered.contains("original scheduled worker failure"));
        assert!(rendered.contains("additionally failed to persist"));

        restore_env_var(PROJECT_ROOT_OVERRIDE_ENV, original_project_root.as_deref());
    }
}

#[cfg(test)]
mod failure_chain_tests {
    // The attempt ledger must not erase a typed machinery error after classification.
    #[derive(Debug)]
    struct ArchiveFailure;
    impl std::fmt::Display for ArchiveFailure {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str("synthetic archive disk failure")
        }
    }
    impl std::error::Error for ArchiveFailure {}

    #[test]
    fn machinery_causes_are_classified_through_run_context() {
        // What the system reports when the disk is full: ENOSPC, or ERROR_DISK_FULL on Windows,
        // where 28 means the printer is out of paper.
        let full = if cfg!(windows) { 112 } else { libc::ENOSPC };
        let error = anyhow::Error::new(std::io::Error::from_raw_os_error(full))
            .context("backup run 1 failed");
        assert_eq!(super::classify_backup_failure(&error), "disk-full");
        let error = anyhow::Error::new(vault_store::Error::OwnedElsewhere {
            path: "synthetic archive".into(),
            source: std::fs::TryLockError::WouldBlock,
        })
        .context("backup run 1 failed");
        assert_eq!(super::classify_backup_failure(&error), "write-lock");
    }

    #[test]
    fn attempt_keeps_the_original_error_type() {
        let root = tempfile::tempdir().unwrap();
        let paths = vault_core::project_paths_with_root(root.path());
        let mut attempt = vault_core::ScheduleAttemptRecorder::begin(&paths).unwrap();
        let error = super::finalize_attempt_failure(
            &mut attempt,
            "archive-open",
            anyhow::Error::new(ArchiveFailure),
        );
        assert!(error.downcast_ref::<ArchiveFailure>().is_some());
    }
}
