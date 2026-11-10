//! What the menu bar knows about backups, and the one path every in-app
//! backup takes.
//!
//! The status line has three sources: backups run by this process (through
//! [`run_backup`], whether started from the window or the menu), the archive's
//! runs (read at launch until the archive opens), and the scheduled-backup
//! ledger (re-read every few minutes: scheduled backups never reach
//! [`run_backup`]). From the last two it takes the last success and the newest
//! backup that ended without saving everything, so after a restart, or after a
//! scheduled backup, the menu says a backup was incomplete or failed as it does
//! right after one in the window.
//!
//! ## Responsibilities
//! - Keep the backup status the menu shows.
//! - Run a backup with progress events, a finished event and status updates.
//!
//! ## Not responsible for
//! - The backup itself (`vault-worker`), or drawing the menu (`tray.rs`).

use chrono::{DateTime, Utc};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, Runtime};

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
    last_failure_partial: bool,
    archive: ArchiveReadiness,
}

impl BackupStatus {
    pub(crate) fn line(&self) -> StatusLine {
        if self.running > 0 {
            return StatusLine::BackingUp;
        }
        match (self.last_success_at, self.last_failure_at) {
            (success, Some(failure)) if success.is_none_or(|success| failure > success) => {
                if self.last_failure_partial { StatusLine::Partial } else { StatusLine::Failed }
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
        self.last_failure_partial = false;
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

    /// Keeps the newest backup that ended without saving everything, from any source.
    fn record_incomplete(&mut self, at: DateTime<Utc>, partial: bool) {
        if self.last_failure_at.is_none_or(|current| at > current) {
            self.last_failure_at = Some(at);
            self.last_failure_partial = partial;
        }
    }

    /// Takes in what [`read_backup_status_from_disk`] found.
    pub(crate) fn apply(&mut self, disk: DiskStatus) {
        if let Some(archive) = disk.archive {
            self.set_archive(archive);
        }
        for at in disk.successes {
            self.record_success(at);
        }
        if let Some((at, partial)) = disk.incomplete {
            self.record_incomplete(at, partial);
        }
    }

    pub(crate) fn archive(&self) -> ArchiveReadiness {
        self.archive
    }

    pub(crate) fn set_archive(&mut self, archive: ArchiveReadiness) {
        self.archive = archive;
    }
}

/// What the archive and the scheduled-backup ledger say about backups.
#[derive(Debug, Default)]
pub(crate) struct DiskStatus {
    archive: Option<ArchiveReadiness>,
    successes: Vec<DateTime<Utc>>,
    /// The newest backup that ended without saving everything: when, and whether it saved part
    /// (`partial`) rather than nothing (`failed`).
    incomplete: Option<(DateTime<Utc>, bool)>,
}

impl DiskStatus {
    fn incomplete(&mut self, at: Option<&str>, partial: bool) {
        if let Some(at) = at.and_then(parse_time)
            && self.incomplete.is_none_or(|(current, _)| at > current)
        {
            self.incomplete = Some((at, partial));
        }
    }
}

/// Reads the archive's backups and the scheduled-backup ledger. Blocking I/O; call it off the
/// main thread.
pub(crate) fn read_backup_status_from_disk(
    vault: Option<&vault_core::Vault>,
    include_archive: bool,
) -> DiskStatus {
    let mut disk = DiskStatus::default();
    let Ok(paths) = vault_core::project_paths() else {
        return disk;
    };
    let Ok(config) = vault_core::load_config(&paths) else {
        return disk;
    };
    if include_archive {
        match vault.map(vault_core::archive_status).transpose().map(|status| {
            status.unwrap_or_else(|| vault_core::archive_availability(&paths, &config))
        }) {
            Ok(status) if !status.initialized => disk.archive = Some(ArchiveReadiness::NotSetUp),
            Ok(status) if status.unlocked => {
                disk.archive = Some(ArchiveReadiness::Ready);
                disk.successes
                    .extend(status.last_successful_backup_at.as_deref().and_then(parse_time));
                if let Some(Ok(Some((at, outcome)))) = vault.map(vault_core::last_finished_backup)
                    && outcome != "success"
                {
                    disk.incomplete(Some(&at), outcome == "partial");
                }
            }
            _ => disk.archive = Some(ArchiveReadiness::Unknown),
        }
    }
    // No wake time changes what is read here.
    if let Ok(ledger) = vault_core::load_schedule_attempt_ledger(
        &paths,
        config.schedule_check_interval_hours as f64,
        None,
    ) {
        disk.successes.extend(ledger.last_success_at.as_deref().and_then(parse_time));
        use vault_core::ScheduledBackupAttemptOutcome::{Failed, Partial, Success};
        // Newest first; a check that found nothing due is not a backup.
        if let Some(latest) = ledger
            .attempts
            .iter()
            .find(|attempt| matches!(attempt.outcome, Success | Partial | Failed))
            && latest.outcome != Success
        {
            disk.incomplete(latest.finished_at.as_deref(), latest.outcome == Partial);
        }
    }
    disk
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
) -> Result<vault_core::BackupReport, CommandError> {
    super::update_status(app, BackupStatus::started);
    let session = app.state::<crate::session::SessionState>();
    let result = worker_bridge::run_backup_now_impl(due_only, &session, |event| {
        let _ = app.emit(BACKUP_PROGRESS_EVENT, &event);
    });
    super::update_status(app, |status| match &result {
        Ok(report) if report.due_skipped => status.skipped(),
        Ok(report) if report.run.as_ref().is_some_and(|run| run.status == "partial") => {
            status.finished(false, Utc::now());
            status.last_failure_partial = true;
        }
        Ok(report) => status
            .finished(report.run.as_ref().is_some_and(|run| run.status == "success"), Utc::now()),
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
    //! What the menu says after a restart, read from a real archive and scheduled-backup ledger.
    //! Ways it can fail: a backup that saved only part, after the last one that saved everything,
    //! reads as "backed up" at the older time (a profile Safari never lets PathKeep read makes
    //! every backup partial, so the menu says "Never backed up" for good); a scheduled backup's
    //! outcome is never seen, as those backups do not go through `run_backup`; an older
    //! incomplete backup hides a newer success.
    use super::*;
    use crate::test_support::{PROJECT_ROOT_OVERRIDE_ENV, lock_env};
    use vault_core::ScheduledBackupAttemptOutcome::{Partial, Success};

    fn after_restart(vault: &vault_core::Vault) -> StatusLine {
        let mut status = BackupStatus::default();
        status.apply(read_backup_status_from_disk(Some(vault), true));
        status.line()
    }

    #[test]
    fn after_a_restart_the_menu_says_how_the_newest_backup_ended() {
        let _env = lock_env();
        let root = tempfile::tempdir().unwrap();
        unsafe { std::env::set_var(PROJECT_ROOT_OVERRIDE_ENV, root.path()) };
        let paths = vault_core::project_paths().unwrap();
        let config = vault_core::AppConfig { initialized: true, ..Default::default() };
        vault_core::save_config(&paths, &config).unwrap();
        let vault = vault_core::Vault::open(paths.clone(), config, None).unwrap();
        vault
            .store()
            .write(vault_store::Db::Archive, vault_store::Priority::Interactive, |s| {
                for (finished, status) in
                    [("2026-10-31T10:00:00Z", "success"), ("2026-10-31T11:00:00Z", "partial")]
                {
                    s.execute(
                        "INSERT INTO runs(run_type,trigger,started_at,finished_at,status)
                         VALUES('backup','manual',?1,?1,?2)",
                        vault_store::params![finished, status],
                    )?;
                }
                Ok(())
            })
            .unwrap();
        assert_eq!(after_restart(&vault), StatusLine::Partial, "the archive's newest run");

        let scheduled = |outcome| {
            vault_core::ScheduleAttemptRecorder::begin(&paths)
                .unwrap()
                .finish(outcome, None, None, None)
                .unwrap()
        };
        scheduled(Success);
        assert!(matches!(after_restart(&vault), StatusLine::BackedUp(_)), "a newer success");
        scheduled(Partial);
        assert_eq!(after_restart(&vault), StatusLine::Partial, "a scheduled backup's");
    }
}
