//! Responsibilities: route committed startup migration batches through archive-upgrade progress,
//! and know whether an open in this process is updating the archive's databases right now.
//! Not responsible for: migration SQL, credentials, frontend navigation, or what quitting does
//! during an update (the desktop app asks first, through `vault_worker::foreground_work`).
//! Failure modes: an update that never stops counting (every later quit asks about it); an
//! update counted before it changes anything, or not counted until after its first change.
use crate::{ArchiveUpgradePhase, ArchiveUpgradeProgress};
use std::{
    cell::RefCell,
    sync::{
        Arc, OnceLock,
        atomic::{AtomicBool, AtomicUsize, Ordering},
        mpsc,
    },
};

pub(crate) type Reporter = Arc<dyn Fn(ArchiveUpgradeProgress) + Send + Sync>;
static LISTENER: OnceLock<Reporter> = OnceLock::new();
thread_local! {static SCOPED:RefCell<Option<Reporter>>=RefCell::new(None);}

/// Desktop startup can report native upgrades even when opening from the keychain.
pub fn register_archive_upgrade_listener(
    listener: impl Fn(ArchiveUpgradeProgress) + Send + Sync + 'static,
) {
    let _ = LISTENER.set(Arc::new(listener));
}
pub(crate) fn observer() -> Option<Reporter> {
    SCOPED.with(|slot| slot.borrow().clone()).or_else(|| LISTENER.get().cloned())
}
pub(crate) fn event(progress: vault_store::BackfillProgress) -> ArchiveUpgradeProgress {
    let mut event = ArchiveUpgradeProgress::phase(
        ArchiveUpgradePhase::SchemaMigration,
        progress.processed,
        progress.total,
    );
    event.database = Some("Intelligence".into());
    event.table = progress.table;
    event
}

/// [`event`] for a backfill of the Archive database.
pub(crate) fn archive_event(progress: vault_store::BackfillProgress) -> ArchiveUpgradeProgress {
    let mut event = event(progress);
    event.database = Some("Archive".into());
    event
}

/// Existing upgrade commands can forward a non-Send callback while storage opens on its worker.
/// The callback runs on the calling thread; captured reporters never extend the wait past `work`.
pub fn with_archive_upgrade_progress<T: Send + 'static>(
    mut report: impl FnMut(ArchiveUpgradeProgress),
    work: impl FnOnce() -> anyhow::Result<T> + Send,
) -> anyhow::Result<T> {
    enum Message<T> {
        Progress(ArchiveUpgradeProgress),
        Finished(anyhow::Result<T>),
    }
    std::thread::scope(|scope| {
        let (sender, receiver) = mpsc::channel();
        let progress = sender.clone();
        let reporter: Reporter = Arc::new(move |event| {
            let _ = progress.send(Message::Progress(event));
        });
        let worker = scope.spawn(move || {
            struct Reset(Option<Reporter>);
            impl Drop for Reset {
                fn drop(&mut self) {
                    SCOPED.with(|slot| *slot.borrow_mut() = self.0.take());
                }
            }
            let _reset = Reset(SCOPED.with(|slot| slot.replace(Some(reporter))));
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(work))
                .unwrap_or_else(|_| Err(anyhow::anyhow!("archive upgrade worker panicked")));
            let _ = sender.send(Message::Finished(result));
        });
        let result = loop {
            match receiver.recv() {
                Ok(Message::Progress(event)) => report(event),
                Ok(Message::Finished(result)) => break result,
                Err(_) => {
                    break Err(anyhow::anyhow!("archive upgrade worker ended without a result"));
                }
            }
        };
        worker.join().map_err(|_| anyhow::anyhow!("archive upgrade worker panicked"))?;
        // Done or failed, the progress screen ends; a failure is reported by the caller.
        report(ArchiveUpgradeProgress::finished());
        result
    })
}

/// Opens in this process that are updating a database right now.
static UPDATING: AtomicUsize = AtomicUsize::new(0);

/// Whether an open in this process is updating the archive's databases to this version's schema:
/// from just before it changes the first one until it returns. Quitting then is safe, since the
/// next launch carries on where the update stopped, but that launch has to finish it before the
/// archive opens, so the desktop app asks first. Reads memory only.
pub fn schema_update_running() -> bool {
    UPDATING.load(Ordering::SeqCst) > 0
}

/// One open's share of [`UPDATING`]: counted from the moment its migrations start to update a
/// database (`MigrationRegistry::on_update`) until this is dropped, when the open returns,
/// whether it succeeded, failed or panicked.
pub(crate) struct UpdateScope(Arc<AtomicBool>);

impl UpdateScope {
    pub(crate) fn new() -> Self {
        Self(Arc::default())
    }

    /// The registry's update observer for this open. Runs on a writer thread, before the change.
    pub(crate) fn observer(&self) -> impl Fn(vault_store::Db) + Send + Sync + 'static {
        let counted = self.0.clone();
        move |_| {
            if !counted.swap(true, Ordering::SeqCst) {
                UPDATING.fetch_add(1, Ordering::SeqCst);
            }
            crate::test_support::hold_schema_update();
        }
    }
}

impl Drop for UpdateScope {
    fn drop(&mut self) {
        if self.0.swap(false, Ordering::SeqCst) {
            UPDATING.fetch_sub(1, Ordering::SeqCst);
        }
    }
}
