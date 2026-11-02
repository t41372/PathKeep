//! Responsibilities: know which operations a person started in this process and is waiting on
//! (an export, an import, a backup, a change to the archive), so the desktop app can ask before
//! quitting throws one away.
//! Not responsible for: what quitting does (`desktop_integration::quit` in the app), stopping or
//! resuming the work, or queue workers that carry on by themselves after a restart (index builds,
//! Insights, link previews: `background_work_running`).
//! Failure modes: an operation that is not registered (quitting drops it without asking); a guard
//! that is never dropped (every later quit asks about work long gone); two runs of the same kind
//! where the first to end hides the second (each run has its own id). The registry only reads
//! memory, because the app asks it on the main thread while a window closes.
//! Tests: below, and the E2E spec `quit-during-work.spec.ts`.
use serde::Serialize;
use std::sync::Mutex;

/// One kind of work someone is waiting on; the quit prompt says what each one is and what
/// quitting now does to it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ForegroundWork {
    /// "Back up now", in the window or the menu bar. A scheduled backup is not on this list: it
    /// runs again at the next check.
    Backup,
    /// Settings → Storage → Export history.
    HistoryExport,
    /// A Takeout or browser history file being imported.
    HistoryImport,
    /// Move to another computer → Export.
    MoveExport,
    /// Opening a file another computer exported, which replaces this archive.
    MoveImport,
    /// Restoring the archive from a safety snapshot.
    Restore,
    /// Free up space (retention).
    FreeUpSpace,
    /// Converting an archive an older PathKeep made.
    ArchiveUpgrade,
    /// A new archive password.
    PasswordChange,
    /// Turning archive encryption on or off, which copies every database.
    EncryptionChange,
    /// Deleting all data.
    DataWipe,
    /// Undoing or restoring an import (Backup → Recent imports).
    ImportUndo,
}

/// The work running in a process, oldest first, one entry per run.
struct Registry(Mutex<Runs>);

struct Runs {
    next_id: u64,
    runs: Vec<(u64, ForegroundWork)>,
}

impl Registry {
    const fn new() -> Self {
        Self(Mutex::new(Runs { next_id: 0, runs: Vec::new() }))
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Runs> {
        // A panic elsewhere while the lock was held left the list as it was; keep using it.
        self.0.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn begin(&'static self, work: ForegroundWork) -> Running {
        let mut running = self.lock();
        running.next_id += 1;
        let id = running.next_id;
        running.runs.push((id, work));
        Running { registry: self, id }
    }

    fn kinds(&self) -> Vec<ForegroundWork> {
        let mut kinds = Vec::new();
        for (_, work) in &self.lock().runs {
            if !kinds.contains(work) {
                kinds.push(*work);
            }
        }
        kinds
    }
}

/// This process's registry.
static RUNNING: Registry = Registry::new();

/// Held for as long as the work runs; dropping it, on success, error or panic, takes this run
/// off the list.
#[must_use = "the work counts as running only while the guard lives"]
pub struct Running {
    registry: &'static Registry,
    id: u64,
}

impl Drop for Running {
    fn drop(&mut self) {
        self.registry.lock().runs.retain(|(id, _)| *id != self.id);
    }
}

/// Puts `work` on the list until the returned guard is dropped.
pub fn begin(work: ForegroundWork) -> Running {
    RUNNING.begin(work)
}

/// What is running now, each kind once, in the order its oldest run started. Memory only, so the
/// desktop app may ask from the main thread.
pub fn foreground_work() -> Vec<ForegroundWork> {
    RUNNING.kinds()
}

#[cfg(test)]
mod tests {
    use super::*;

    // Each test has a registry of its own: other tests in this binary run real exports, wipes and
    // restores, which register on the process's list at the same time.
    fn registry() -> &'static Registry {
        Box::leak(Box::new(Registry::new()))
    }

    #[test]
    fn a_second_run_of_the_same_kind_keeps_it_listed_until_both_end() {
        let registry = registry();
        let first = registry.begin(ForegroundWork::DataWipe);
        let second = registry.begin(ForegroundWork::DataWipe);
        drop(first);
        assert_eq!(registry.kinds(), [ForegroundWork::DataWipe]);
        drop(second);
        assert!(registry.kinds().is_empty());
    }

    #[test]
    fn work_that_panics_is_taken_off_the_list() {
        let registry = registry();
        let result = std::panic::catch_unwind(|| {
            let _running = registry.begin(ForegroundWork::FreeUpSpace);
            panic!("the work failed");
        });
        assert!(result.is_err());
        assert!(registry.kinds().is_empty());
    }

    #[test]
    fn each_kind_is_listed_once_in_the_order_its_oldest_run_started() {
        let registry = registry();
        let export = registry.begin(ForegroundWork::MoveExport);
        let _restore = registry.begin(ForegroundWork::Restore);
        let _another_export = registry.begin(ForegroundWork::MoveExport);
        assert_eq!(registry.kinds(), [ForegroundWork::MoveExport, ForegroundWork::Restore]);
        // The later export is still running, but it started after the restore.
        drop(export);
        assert_eq!(registry.kinds(), [ForegroundWork::Restore, ForegroundWork::MoveExport]);
    }
}
