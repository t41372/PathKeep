//! Responsibilities: drain admitted jobs, checkpoint and hold a stable file-copy boundary.
//! Not responsible for: application file swaps, envelopes or snapshot retention.
//! Failure modes: snapshot overlaps a batch; busy checkpoint is ignored; an error
//! strands admission; closing releases the owner before all file users finish.
//! Tests: tests/maintenance.rs, with real encrypted files and competing batches.
use crate::{
    CancelToken, CopyProgress, Db, Error, Paths, Result, Store, TableDigest, worker::Workers,
};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::{RwLockWriteGuard, atomic::Ordering},
};

/// Holding this guard freezes committed state across all databases. Dropping it
/// resumes admission, including on cancellation or an I/O error.
pub struct Maintenance<'a> {
    store: &'a Store,
    workers: RwLockWriteGuard<'a, Vec<(Db, Workers)>>,
}

impl Store {
    /// Wait for admitted requests to finish, then pause between producer batches.
    /// Callers must not call Store again while holding the returned guard.
    pub fn pause(&self) -> Result<Maintenance<'_>> {
        crate::worker::check_caller()?;
        let workers = self.workers.write().unwrap();
        if self.closed.load(Ordering::Acquire) {
            return Err(Error::Closed);
        }
        Ok(Maintenance { store: self, workers })
    }
}

impl Maintenance<'_> {
    fn worker(&self, db: Db) -> &Workers {
        &self.workers.iter().find(|(id, _)| *id == db).unwrap().1
    }

    /// A snapshot must include the WAL's committed rows before copying main files.
    pub fn checkpoint(&self) -> Result<()> {
        for (db, worker) in self.workers.iter() {
            if *db != Db::Scratch {
                worker.maintenance(|engine| crate::copy::checkpoint(engine, "main"))?;
            }
        }
        Ok(())
    }

    /// Copies every durable database with its current on-disk encryption intact.
    /// The caller copies its envelope/config while retaining this same guard.
    pub fn snapshot(&self, root: &Path) -> Result<()> {
        self.checkpoint()?;
        let targets = Paths::under(root);
        for db in [Db::Archive, Db::SourceEvidence, Db::Intelligence, Db::Agent] {
            let target = targets.get(db);
            fs::create_dir_all(target.parent().unwrap())?;
            fs::copy(self.store.paths.get(db), target)?;
            fs::File::open(target)?.sync_all()?;
        }
        Ok(())
    }

    /// Clones one complete database through an explicitly keyed ATTACH. The
    /// returned proofs compare source and destination values, not physical bytes.
    pub fn copy_database(
        &self,
        db: Db,
        destination: PathBuf,
        key: Option<[u8; 32]>,
        cancel: CancelToken,
        progress: impl FnMut(CopyProgress) + Send + 'static,
    ) -> Result<Vec<TableDigest>> {
        let key = zeroize::Zeroizing::new(key);
        self.worker(db).maintenance(move |engine| {
            crate::copy::database(engine, &destination, key.as_ref(), &cancel, progress)
        })
    }

    /// Revoke queued handles before a verified file swap; no writer can slip
    /// between verification and shutdown. The caller may retain the owner lock.
    pub fn close(mut self) -> Result<()> {
        self.checkpoint()?;
        self.store.closed.store(true, Ordering::Release);
        while let Some((_, worker)) = self.workers.pop() {
            drop(worker);
        }
        self.store.ownership.lock().unwrap().take();
        Ok(())
    }
}
