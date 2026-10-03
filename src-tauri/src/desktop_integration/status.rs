//! What the menu bar knows about backups, and the one path every in-app
//! backup takes.
//!
//! The status line has three sources: backups run by this process (through
//! [`run_backup`], whether started from the window or the menu), the archive's
//! last successful run (read at launch and after each backup), and the
//! scheduled-backup ledger the worker sidecar writes (re-read every few
//! minutes, because those backups run in another process).
//!
//! ## Responsibilities
//! - Keep the backup status the menu shows.
//! - Run a backup with progress events, a finished event and status updates.
//!
//! ## Not responsible for
//! - The backup itself (`vault-worker`), or drawing the menu (`tray.rs`).

use chrono::{DateTime, Utc};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Runtime};

use super::copy::StatusLine;
use crate::{command_error::CommandError, worker_bridge};

/// Event carrying backup progress (existing contract, see `backup-progress.ts`).
pub(crate) const BACKUP_PROGRESS_EVENT: &str = "pathkeep://backup-progress";
/// Event sent when any in-app backup ends, so the window can refresh after a
/// backup it did not start (e.g. "Back up now" in the menu bar).
pub(crate) const BACKUP_FINISHED_EVENT: &str = "pathkeep://backup-finished";

/// Who started a backup.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum BackupSource {
    /// The window (`run_backup_now`).
    App,
    /// "Back up now" in the menu bar menu.
    MenuBar,
}

/// Payload of [`BACKUP_FINISHED_EVENT`].
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BackupFinished {
    pub(crate) source: BackupSource,
    pub(crate) report: Option<vault_core::BackupReport>,
    pub(crate) error: Option<CommandError>,
}

/// Whether the archive exists and could be read the last time we looked.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum ArchiveReadiness {
    NotSetUp,
    Ready,
    #[default]
    Unknown,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct BackupStatus {
    running: usize,
    last_success_at: Option<DateTime<Utc>>,
    last_failure_at: Option<DateTime<Utc>>,
    archive: ArchiveReadiness,
}

impl BackupStatus {
    pub(crate) fn line(&self) -> StatusLine {
        if self.running > 0 {
            return StatusLine::BackingUp;
        }
        match (self.last_success_at, self.last_failure_at) {
            (success, Some(failure)) if success.is_none_or(|success| failure > success) => {
                StatusLine::Failed
            }
            (Some(success), _) => StatusLine::BackedUp(success),
            (None, _) => match self.archive {
                ArchiveReadiness::NotSetUp => StatusLine::NotSetUp,
                ArchiveReadiness::Ready => StatusLine::NeverBackedUp,
                ArchiveReadiness::Unknown => StatusLine::Unknown,
            },
        }
    }

    /// "Back up now" is offered unless a backup is running or there is no archive yet.
    pub(crate) fn can_back_up(&self) -> bool {
        self.running == 0 && self.archive != ArchiveReadiness::NotSetUp
    }

    pub(crate) fn started(&mut self) {
        self.running += 1;
    }

    pub(crate) fn finished(&mut self, succeeded: bool, at: DateTime<Utc>) {
        self.running = self.running.saturating_sub(1);
        if succeeded {
            self.record_success(at);
            self.archive = ArchiveReadiness::Ready;
        } else {
            self.last_failure_at = Some(at);
        }
    }

    /// A due-only run that found nothing due.
    pub(crate) fn skipped(&mut self) {
        self.running = self.running.saturating_sub(1);
    }

    /// Keeps the newest success from any source.
    pub(crate) fn record_success(&mut self, at: DateTime<Utc>) {
        if self.last_success_at.is_none_or(|current| at > current) {
            self.last_success_at = Some(at);
        }
    }

    pub(crate) fn archive(&self) -> ArchiveReadiness {
        self.archive
    }

    pub(crate) fn set_archive(&mut self, archive: ArchiveReadiness) {
        self.archive = archive;
    }
}

/// Reads the archive's last successful backup and the scheduled-backup
/// ledger. Blocking I/O; call it off the main thread.
pub(crate) fn read_backup_status_from_disk(
    database_key: Option<&str>,
    include_archive: bool,
) -> (Option<ArchiveReadiness>, Vec<DateTime<Utc>>) {
    let Ok(paths) = vault_core::project_paths() else {
        return (None, Vec::new());
    };
    let Ok(config) = vault_core::load_config(&paths) else {
        return (None, Vec::new());
    };
    let mut successes = Vec::new();
    let mut readiness = None;
    if include_archive {
        match vault_core::archive_status(&paths, &config, database_key) {
            Ok(status) if !status.initialized => readiness = Some(ArchiveReadiness::NotSetUp),
            Ok(status) if status.unlocked => {
                readiness = Some(ArchiveReadiness::Ready);
                successes.extend(status.last_successful_backup_at.as_deref().and_then(parse_time));
            }
            _ => readiness = Some(ArchiveReadiness::Unknown),
        }
    }
    if let Ok(ledger) = vault_core::load_schedule_attempt_ledger(
        &paths,
        config.schedule_check_interval_hours as f64,
    ) {
        successes.extend(ledger.last_success_at.as_deref().and_then(parse_time));
    }
    (readiness, successes)
}

fn parse_time(value: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(value).ok().map(|time| time.with_timezone(&Utc))
}

/// Runs one backup with progress events, updates the menu bar status, and
/// announces the result with [`BACKUP_FINISHED_EVENT`]. Blocking; call it off
/// the main thread.
pub(crate) fn run_backup<R: Runtime>(
    app: &AppHandle<R>,
    source: BackupSource,
    due_only: bool,
    database_key: Option<&str>,
) -> Result<vault_core::BackupReport, CommandError> {
    super::update_status(app, BackupStatus::started);
    let result = worker_bridge::run_backup_now_impl(due_only, database_key, |event| {
        let _ = app.emit(BACKUP_PROGRESS_EVENT, &event);
    });
    let ran = result.as_ref().map(|report| !report.due_skipped);
    super::update_status(app, |status| match ran {
        Ok(true) => status.finished(true, Utc::now()),
        // A due-only call that found nothing due neither succeeded nor failed.
        Ok(false) => status.skipped(),
        Err(_) => status.finished(false, Utc::now()),
    });
    let _ = app.emit(
        BACKUP_FINISHED_EVENT,
        BackupFinished {
            source,
            report: result.as_ref().ok().cloned(),
            error: result.as_ref().err().cloned(),
        },
    );
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    #[test]
    fn status_line_follows_running_failure_and_success() {
        let now = Utc::now();
        let mut status = BackupStatus::default();
        assert_eq!(status.line(), StatusLine::Unknown);
        status.set_archive(ArchiveReadiness::NotSetUp);
        assert_eq!(status.line(), StatusLine::NotSetUp);
        assert!(!status.can_back_up());
        status.set_archive(ArchiveReadiness::Ready);
        assert_eq!(status.line(), StatusLine::NeverBackedUp);

        status.started();
        status.started();
        assert_eq!(status.line(), StatusLine::BackingUp);
        assert!(!status.can_back_up());
        status.finished(false, now - Duration::minutes(5));
        assert_eq!(status.line(), StatusLine::BackingUp, "one run is still going");
        status.finished(true, now);
        assert_eq!(status.line(), StatusLine::BackedUp(now));
        assert!(status.can_back_up());

        status.started();
        status.finished(false, now + Duration::minutes(1));
        assert_eq!(status.line(), StatusLine::Failed);
        // A later scheduled success clears the failure; an older one does not.
        status.record_success(now - Duration::hours(1));
        assert_eq!(status.line(), StatusLine::Failed);
        status.record_success(now + Duration::minutes(2));
        assert_eq!(status.line(), StatusLine::BackedUp(now + Duration::minutes(2)));
    }
}
