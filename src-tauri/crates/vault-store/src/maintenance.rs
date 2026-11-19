//! Responsibilities: drain admitted jobs, checkpoint and hold a stable file-copy boundary.
//! Not responsible for: application file swaps, envelopes or snapshot retention.
//! Failure modes: snapshot overlaps a batch; busy checkpoint is ignored; an error
//! strands admission; closing releases the owner before all file users finish;
//! snapshots copy disabled files. Tests: maintenance.rs and selected_databases.rs,
//! with real encrypted files and competing batches.
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
    /// Before choosing maintenance members, verify that every required live file still
    /// exists under its own path and has the protection its worker opened.
    pub fn check_database_files(&self, required: impl Iterator<Item = Db>) -> Result<()> {
        let workers = self.workers();
        for db in required {
            let worker = &workers.iter().find(|(role, _)| *role == db).ok_or(Error::Closed)?.1;
            let encrypted = worker.encrypted;
            let path = self.paths.get(db);
            check_identity(db, worker, path)?;
            let on_disk = crate::engine::header_encrypted(path).map_err(|e| {
                let cause = if e.kind() == std::io::ErrorKind::NotFound {
                    Error::MissingStore(path.into())
                } else {
                    Error::Io(e)
                };
                cause.at(db, crate::OpenStage::Preflight)
            })?;
            if on_disk != encrypted {
                return Err(Error::InvalidConfig("database protection changed since open".into())
                    .at(db, crate::OpenStage::Preflight));
            }
        }
        Ok(())
    }

    /// Wait for admitted requests to finish, then pause between producer batches.
    /// Callers must not call Store again while holding the returned guard.
    pub fn pause(&self) -> Result<Maintenance<'_>> {
        crate::worker::check_caller()?;
        let workers = self.workers_mut();
        if self.closed.load(Ordering::Acquire) {
            return Err(Error::Closed);
        }
        Ok(Maintenance { store: self, workers })
    }
    /// Drain requests already admitted at the time of damage. The long file copy
    /// holds a shared worker guard so browsing reads and writes can continue.
    pub fn copy_quarantined_conversations(&self, root: &Path) -> Result<()> {
        drop(self.pause()?);
        let workers = self.workers();
        if !self.is_open() {
            return Err(Error::Closed);
        }
        self.copy_opaque_conversations(&workers, root)
    }

    /// Capture the held generation as filesystem bytes, including its WAL, without
    /// querying or checkpointing the suspect databases. The group is already quarantined.
    fn copy_opaque_conversations(&self, workers: &[(Db, Workers)], root: &Path) -> Result<()> {
        if self.conversation_availability().is_ok() {
            return Err(Error::InvalidConfig(
                "opaque copy requires an unavailable conversation group".into(),
            ));
        }
        for (db, worker) in workers.iter().filter(|(db, _)| crate::opening::conversation_role(*db))
        {
            check_identity(*db, worker, self.paths.get(*db))?;
            worker.quarantine.store(true, Ordering::Release);
            worker.maintenance(|_| Ok(()))?;
        }
        for db in Db::ALL.into_iter().filter(|db| crate::opening::conversation_role(*db)) {
            for suffix in vault_layout::database_spec(db)
                .companions
                .suffixes(vault_layout::CompanionUse::Install)
            {
                let relative = format!("{}{suffix}", vault_layout::database_path(db));
                let mut source = self.paths.get(db).as_os_str().to_owned();
                source.push(suffix);
                let source = Path::new(&source);
                match fs::symlink_metadata(source) {
                    Ok(_) => copy_member(source, &root.join(relative))?,
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                    Err(e) => return Err(e.into()),
                }
            }
        }
        for (db, worker) in workers.iter().filter(|(db, _)| crate::opening::conversation_role(*db))
        {
            check_identity(*db, worker, self.paths.get(*db))?;
        }
        Ok(())
    }
}

impl Maintenance<'_> {
    fn worker(&self, db: Db) -> Result<&Workers> {
        self.workers.iter().find(|(id, _)| *id == db).map(|(_, worker)| worker).ok_or(Error::Closed)
    }

    /// A snapshot must include the WAL's committed rows before copying main files.
    pub fn checkpoint(&self) -> Result<()> {
        for (db, worker) in self.workers.iter() {
            if crate::opening::conversation_role(*db)
                && self.store.conversation_availability().is_err()
            {
                continue;
            }
            if vault_layout::spec(vault_layout::db_artifact(*db)).lifecycle.snapshot
                == vault_layout::SnapshotPolicy::CopyDatabase
            {
                check_identity(*db, worker, self.store.paths.get(*db))?;
                worker.maintenance(|engine| crate::copy::checkpoint(engine, "main"))?;
            }
        }
        Ok(())
    }

    /// Copies every selected durable database with its on-disk encryption intact.
    /// The caller copies its envelope/config while retaining this same guard.
    pub fn snapshot(&self, root: &Path) -> Result<()> {
        self.store.seal_if_available(&self.workers)?;
        self.checkpoint()?;
        let targets = Paths::under(root);
        let damaged = self.store.conversation_availability().is_err();
        for (db, worker) in self.workers.iter() {
            if vault_layout::spec(vault_layout::db_artifact(*db)).lifecycle.snapshot
                != vault_layout::SnapshotPolicy::CopyDatabase
                || (damaged && crate::opening::conversation_role(*db))
            {
                continue;
            }
            let source = self.store.paths.get(*db);
            check_identity(*db, worker, source)?;
            copy_member(source, targets.get(*db))?;
            check_identity(*db, worker, source)?;
        }
        if damaged {
            self.store.copy_opaque_conversations(&self.workers, root)?;
            let mut marker =
                fs::File::create_new(root.join(vault_layout::UNREADABLE_CONVERSATIONS))?;
            std::io::Write::write_all(&mut marker, b"1\n")?;
            marker.sync_all()?;
        }
        Ok(())
    }

    /// Clones one complete database through an explicitly keyed ATTACH. The
    /// returned proofs compare source and destination values, not physical bytes.
    /// An unselected database returns `Error::Closed`, as regular reads/writes do.
    pub fn copy_database(
        &self,
        db: Db,
        destination: PathBuf,
        key: Option<[u8; 32]>,
        cancel: CancelToken,
        progress: impl FnMut(CopyProgress) + Send + 'static,
    ) -> Result<Vec<TableDigest>> {
        let key = zeroize::Zeroizing::new(key);
        self.worker(db)?.maintenance(move |engine| {
            crate::copy::database(engine, &destination, key.as_ref(), &cancel, progress)
        })
    }

    /// Revoke queued handles before a verified file swap; no writer can slip
    /// between verification and shutdown. The caller may retain the owner lock.
    pub fn close(self) -> Result<()> {
        self.close_with(|| {})
    }

    /// Retires an already paused service after the final checkpoint succeeds and before
    /// closing connections. The callback must neither submit store work nor wait for it.
    pub fn close_with(mut self, retire: impl FnOnce()) -> Result<()> {
        self.store.seal_if_available(&self.workers)?;
        self.checkpoint()?;
        retire();
        self.store.closed.store(true, Ordering::Release);
        while let Some((_, worker)) = self.workers.pop() {
            drop(worker);
        }
        self.store.ownership.lock().unwrap().take();
        Ok(())
    }
}

pub(crate) fn check_identity(db: Db, worker: &Workers, path: &Path) -> Result<()> {
    let id = turso::core::io::get_file_id(path.to_str().expect("validated role path"))
        .map_err(|e| Error::Io(e).at(db, crate::OpenStage::Preflight))?;
    if worker.file_id != Some(id) {
        return Err(Error::ReplacedFile(path.into()).at(db, crate::OpenStage::Preflight));
    }
    Ok(())
}

/// Opaque members may be a directory where the main database used to be. Preserve
/// regular children and empty directories. Links and special files require attention.
fn copy_member(source: &Path, target: &Path) -> Result<()> {
    let mut pending = vec![(source.to_owned(), target.to_owned())];
    while let Some((source, target)) = pending.pop() {
        let meta = fs::symlink_metadata(&source)?;
        if meta.is_file() {
            fs::create_dir_all(target.parent().expect("snapshot member has a parent"))?;
            let mut input = fs::File::open(source)?;
            let mut output = fs::File::create_new(target)?;
            std::io::copy(&mut input, &mut output)?;
            output.sync_all()?;
        } else if meta.is_dir() {
            fs::create_dir_all(&target)?;
            for entry in fs::read_dir(source)? {
                let entry = entry?;
                pending.push((entry.path(), target.join(entry.file_name())));
            }
        } else {
            return Err(Error::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "unexpected snapshot member type",
            )));
        }
    }
    Ok(())
}
