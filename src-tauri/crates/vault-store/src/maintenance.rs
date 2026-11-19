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

/// An open handle still supplies these bytes even if no pathname does.
pub struct HeldGeneration<'a> {
    pub role: Db,
    pub generation: u64,
    files: &'a crate::worker::HeldFiles,
}
impl HeldGeneration<'_> {
    pub fn copy_to(&self, root: &Path) -> Result<()> {
        copy_held(self.role, self.files, root)
    }
}
impl Store {
    /// Watch identities even while the window is idle. Retry and shutdown also
    /// check synchronously, so a normal close never depends on this interval.
    pub fn on_displacement(
        self: &std::sync::Arc<Self>,
        publisher: impl Fn(HeldGeneration<'_>) -> Result<()> + Send + Sync + 'static,
    ) -> Result<()> {
        let workers = self.workers();
        self.record_attachment_ids(&workers);
        if self.held_publisher.set(Box::new(publisher)).is_err() {
            return Ok(());
        }
        let weak = std::sync::Arc::downgrade(self);
        let (stop, wait) = std::sync::mpsc::channel();
        std::thread::Builder::new().name("held-conversation-files".into()).spawn(move || {
            use std::time::Duration;
            let mut interval = Duration::from_secs(1);
            while wait.recv_timeout(interval) == Err(std::sync::mpsc::RecvTimeoutError::Timeout) {
                let Some(store) = weak.upgrade() else { break };
                if !store.is_open() {
                    break;
                }
                interval = match store.preserve_observed_displacements() {
                    Ok(()) => Duration::from_secs(1),
                    Err(error) => {
                        log::warn!("Could not protect a held conversation file: {error}");
                        Duration::from_secs(30)
                    }
                };
            }
        })?;
        *self.anchor_monitor.lock().unwrap() = Some(stop);
        Ok(())
    }
    pub(crate) fn record_attachment_ids(&self, workers: &[(Db, Workers)]) {
        *self.pending_anchors.lock().unwrap() = workers
            .iter()
            .filter(|(db, _)| crate::opening::conversation_role(*db))
            .filter_map(|(db, w)| w.file_id.map(|id| (*db, id)))
            .collect();
    }

    /// Quit-now can interrupt foreground work. It waits for a safety copy only
    /// when an unprotected member no longer has its admitted filesystem identity.
    pub fn preserve_observed_displacements(&self) -> Result<()> {
        let at_risk = self.pending_anchors.lock().unwrap().iter().any(|(db, id)| {
            turso::core::io::get_file_id(self.paths.get(*db).to_str().expect("validated role path"))
                .ok()
                != Some(*id)
        });
        if at_risk {
            self.preserve_displaced_conversations()?;
        }
        Ok(())
    }
    pub fn pair_was_rejected(&self) -> bool {
        self.rejected_pair.load(Ordering::Acquire)
    }
    pub(crate) fn generation(&self) -> u64 {
        self.conversation_generation.load(Ordering::Acquire)
    }
    pub fn current_held_role(&self, role: Db) -> bool {
        self.retired_files
            .lock()
            .unwrap()
            .iter()
            .any(|(g, db, _)| *g == self.generation() && *db == role)
    }
    pub fn preserve_displaced_conversations(&self) -> Result<()> {
        crate::worker::check_caller()?;
        let workers = self.workers();
        if self.held_publisher.get().is_none() {
            return Ok(());
        }
        let mut displaced_role = None;
        for (db, w) in workers.iter().filter(|(db, _)| crate::opening::conversation_role(*db)) {
            if !*w.held_saved.lock().unwrap() && displaced(*db, w, self.paths.get(*db))? {
                displaced_role = Some(*db);
                break;
            }
        }
        let Some(role) = displaced_role else { return Ok(()) };
        self.quarantine(
            &workers,
            std::sync::Arc::new(crate::StoreOpenFailure {
                role,
                stage: crate::OpenStage::Preflight,
                cause: Error::ReplacedFile(self.paths.get(role).to_owned()),
            }),
        );
        drop(workers);
        // Drain already admitted SQL. Publication can then stream pinned bytes under a
        // shared guard while the quarantined group refuses new writes.
        drop(self.workers_mut());
        self.protect_held(&self.workers())
    }
    pub(crate) fn protect_held(&self, workers: &[(Db, Workers)]) -> Result<()> {
        let Some(publish) = self.held_publisher.get() else { return Ok(()) };
        for (db, w) in workers.iter().filter(|(db, _)| crate::opening::conversation_role(*db)) {
            let path = self.paths.get(*db);
            if !displaced(*db, w, path)? {
                continue;
            }
            if self.conversation_availability().is_ok() {
                self.quarantine(
                    workers,
                    std::sync::Arc::new(crate::StoreOpenFailure {
                        role: *db,
                        stage: crate::OpenStage::Preflight,
                        cause: Error::ReplacedFile(path.to_owned()),
                    }),
                );
            }
            let mut saved = w.held_saved.lock().unwrap();
            if !*saved {
                w.quarantine.store(true, Ordering::Release);
                w.maintenance(|_| Ok(()))?;
                publish(HeldGeneration {
                    role: *db,
                    generation: self.generation(),
                    files: &w.held_files,
                })?;
                *saved = true;
                self.pending_anchors.lock().unwrap().retain(|(role, _)| role != db);
            }
        }
        Ok(())
    }
    /// Retry can release an access pause in place when both original workers and files still match.
    pub fn retry_access(&self) -> Result<bool> {
        let _adding = self.adding.lock().unwrap();
        if !self.conversation_availability().is_err_and(|f| f.cause.access_refused()) {
            return Ok(false);
        }
        if [Db::AgentHistoryEvidence, Db::AgentHistoryReadModel]
            .iter()
            .any(|role| self.database_encrypted(*role).is_none())
        {
            return Ok(false);
        }
        match self
            .check_database_files([Db::AgentHistoryEvidence, Db::AgentHistoryReadModel].into_iter())
        {
            Ok(()) => {}
            Err(Error::Open(reason)) if !reason.cause.access_refused() => return Ok(false),
            Err(error) => return Err(error),
        }
        let workers = self.workers_mut();
        for (db, w) in workers.iter().filter(|(db, _)| crate::opening::conversation_role(*db)) {
            check_identity(*db, w, self.paths.get(*db))?;
            w.maintenance(|engine| {
                engine
                    .session(true)
                    .query_one("SELECT count(*) FROM sqlite_schema", (), |r| Ok(r.get::<i64>(0)?))
                    .map(drop)
            })?;
        }
        self.conversation_generation.fetch_add(1, Ordering::AcqRel);
        for (_, w) in workers.iter().filter(|(db, _)| crate::opening::conversation_role(*db)) {
            *w.held_saved.lock().unwrap() = false;
            w.quarantine.store(false, Ordering::Release);
        }
        self.record_attachment_ids(&workers);
        self.conversation_failure.write().unwrap().take();
        self.announce_availability();
        Ok(true)
    }
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
            if let Err(error) = check_identity(db, worker, path) {
                if crate::opening::conversation_role(db)
                    && matches!(&error,Error::Open(f) if !f.cause.access_refused())
                {
                    if let Error::Open(reason) = &error {
                        self.quarantine(&workers, reason.clone());
                    }
                    drop(workers);
                    self.preserve_displaced_conversations()?;
                }
                return Err(error);
            }
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
        let _adding = self.adding.lock().unwrap();
        self.preserve_displaced_conversations()?;
        drop(self.pause()?);
        #[cfg(windows)]
        {
            let close = self.anchor_pending(&self.workers())?;
            if !close.is_empty() {
                self.release_pending(&mut self.workers_mut(), close);
            }
        }
        let workers = self.workers();
        if !self.is_open() {
            return Err(Error::Closed);
        }
        self.protect_held(&workers)?;
        let stale = self.capture_opaque(&workers, root)?;
        drop(workers);
        if !stale.is_empty() {
            self.retire_displaced(&mut self.workers_mut(), stale);
        }
        Ok(())
    }

    fn copy_opaque_members(&self, root: &Path) -> Result<()> {
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
                    Err(e) => {
                        return Err(Error::PreserveFile { path: source.to_owned(), source: e });
                    }
                }
            }
        }
        Ok(())
    }
}

impl Store {
    fn capture_opaque(&self, workers: &[(Db, Workers)], root: &Path) -> Result<Vec<Db>> {
        if self.conversation_availability().is_ok() {
            return Err(Error::InvalidConfig("opaque copy requires unavailable roles".into()));
        }
        let mut stale = Vec::new();
        for (db, worker) in workers.iter().filter(|(db, _)| crate::opening::conversation_role(*db))
        {
            worker.quarantine.store(true, Ordering::Release);
            worker.maintenance(|_| Ok(()))?;
            if displaced(*db, worker, self.paths.get(*db))? {
                stale.push(*db);
            }
        }
        self.copy_opaque_members(root)?;
        // Normally there is one displaced generation. A second external replacement
        // after a retry must not overwrite the earlier handle's bytes.
        let retired = self.retired_files.lock().unwrap();
        for db in [Db::AgentHistoryEvidence, Db::AgentHistoryReadModel] {
            let current = workers
                .iter()
                .find(|(role, _)| *role == db && stale.contains(role))
                .map(|(_, w)| (self.generation(), &w.held_files));
            let mut older = 0;
            for (generation, files) in current.into_iter().chain(
                retired
                    .iter()
                    .rev()
                    .filter(|(_, role, _)| *role == db)
                    .map(|(g, _, files)| (*g, files)),
            ) {
                let mut base = root.join(vault_layout::HELD_CONVERSATIONS);
                if generation != self.generation() {
                    older += 1;
                    base = base.join(format!("older-{older}"));
                }
                copy_held(db, files, &base)?;
            }
        }
        for (db, worker) in workers
            .iter()
            .filter(|(db, _)| crate::opening::conversation_role(*db) && !stale.contains(db))
        {
            check_identity(*db, worker, self.paths.get(*db))?;
        }
        Ok(stale)
    }
    fn retire_displaced(&self, workers: &mut Vec<(Db, Workers)>, stale: Vec<Db>) {
        let mut retired = self.retired_files.lock().unwrap();
        for db in stale {
            let index = workers.iter().position(|(id, _)| *id == db).unwrap();
            let (_, mut worker) = workers.remove(index);
            retired.push((self.generation(), db, std::mem::take(&mut worker.held_files)));
        }
    }
}

impl Maintenance<'_> {
    fn worker(&self, db: Db) -> Result<&Workers> {
        self.workers.iter().find(|(id, _)| *id == db).map(|(_, worker)| worker).ok_or(Error::Closed)
    }

    pub fn seal_and_checkpoint(&self) -> Result<()> {
        self.store.seal_if_available(&self.workers)?;
        self.checkpoint()
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
    pub fn snapshot(&mut self, root: &Path) -> Result<()> {
        self.snapshot_with(root, || Ok(()))
    }

    /// Bind an install to its originals at the checkpoint used to preserve them.
    pub fn snapshot_with<T>(&mut self, root: &Path, bind: impl FnOnce() -> Result<T>) -> Result<T> {
        let damaged = self.store.conversation_availability().is_err();
        if damaged {
            self.store.protect_held(&self.workers)?;
            #[cfg(windows)]
            {
                let close = self.store.anchor_pending(&self.workers)?;
                self.store.release_pending(&mut self.workers, close);
            }
            let stale = self.store.capture_opaque(&self.workers, root)?;
            self.store.retire_displaced(&mut self.workers, stale);
        }
        self.seal_and_checkpoint()?;
        let originals = bind()?;
        let targets = Paths::under(root);
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
            let mut marker =
                fs::File::create_new(root.join(vault_layout::UNREADABLE_CONVERSATIONS))?;
            std::io::Write::write_all(&mut marker, b"1\n")?;
            marker.sync_all()?;
        }
        Ok(originals)
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
        let worker = self.worker(db)?;
        let result = worker.maintenance(move |engine| {
            crate::copy::database(engine, &destination, key.as_ref(), &cancel, progress)
        });
        match result {
            Err(
                cause
                @ (Error::WrongKey { .. } | Error::Corrupt { .. } | Error::JobPanicked { .. }),
            ) if crate::opening::conversation_role(db)
                && self.store.pair_seals
                && worker.integrity_check_failed() =>
            {
                let reason = std::sync::Arc::new(crate::StoreOpenFailure {
                    role: db,
                    stage: crate::OpenStage::Read,
                    cause,
                });
                self.store.quarantine(&self.workers, reason.clone());
                Err(Error::RepairRequired(reason))
            }
            other => other,
        }
    }

    /// Revoke queued handles before a verified file swap; no writer can slip
    /// between verification and shutdown. The caller may retain the owner lock.
    pub fn close(self) -> Result<()> {
        self.close_with(|| {})
    }

    /// Retires an already paused service after the final checkpoint succeeds and before
    /// closing connections. The callback must neither submit store work nor wait for it.
    pub fn close_with(mut self, retire: impl FnOnce()) -> Result<()> {
        self.store.protect_held(&self.workers)?;
        self.store.seal_if_available(&self.workers)?;
        self.checkpoint()?;
        retire();
        self.store.closed.store(true, Ordering::Release);
        self.store.anchor_monitor.lock().unwrap().take();
        while let Some((_, worker)) = self.workers.pop() {
            drop(worker);
        }
        self.store.retired_files.lock().unwrap().clear();
        self.store.ownership.lock().unwrap().take();
        Ok(())
    }
}

pub(crate) fn displaced(db: Db, worker: &Workers, path: &Path) -> Result<bool> {
    match check_identity(db, worker, path) {
        Ok(()) => Ok(false),
        Err(Error::Open(f))
            if matches!(f.cause, Error::ReplacedFile(_))
                || matches!(&f.cause,Error::Io(e) if e.kind()==std::io::ErrorKind::NotFound) =>
        {
            Ok(true)
        }
        #[cfg(windows)]
        Err(Error::Open(f)) if matches!(&f.cause,Error::Io(e) if crate::maintenance_windows::pending_name(e)) => {
            Ok(true)
        }
        Err(Error::Open(f)) if matches!(&f.cause, Error::Io(_)) => {
            Err(preserve_error(path, std::io::Error::other(f)))
        }
        Err(e) => Err(e),
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
        let meta = fs::symlink_metadata(&source).map_err(|e| preserve_error(&source, e))?;
        if meta.is_file() {
            fs::create_dir_all(target.parent().expect("snapshot member has a parent"))?;
            let mut input = fs::File::open(&source).map_err(|e| preserve_error(&source, e))?;
            let mut output = fs::File::create_new(target)?;
            copy_bytes(&source, &mut input, &mut output)?;
            output.sync_all()?;
        } else if meta.is_dir() {
            fs::create_dir_all(&target)?;
            for entry in fs::read_dir(&source).map_err(|e| preserve_error(&source, e))? {
                let entry = entry.map_err(|e| preserve_error(&source, e))?;
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

fn preserve_error(path: &Path, source: std::io::Error) -> Error {
    Error::PreserveFile { path: path.to_owned(), source }
}
pub(super) fn copy_bytes(path: &Path, input: &mut fs::File, output: &mut fs::File) -> Result<()> {
    use std::io::{Read, Write};
    let mut buf = [0; 65536];
    loop {
        let n = input.read(&mut buf).map_err(|e| preserve_error(path, e))?;
        if n == 0 {
            return Ok(());
        }
        output.write_all(&buf[..n])?;
    }
}

fn copy_held(db: Db, files: &[(String, fs::File)], root: &Path) -> Result<()> {
    use std::io::Write;
    for (suffix, handle) in files {
        let name = format!("{}{suffix}", vault_layout::database_path(db));
        let path = root.join(&name);
        fs::create_dir_all(path.parent().unwrap())?;
        let mut output = fs::File::create_new(path)?;
        let mut offset = 0;
        let mut buf = [0; 65536];
        loop {
            #[cfg(unix)]
            let read = std::os::unix::fs::FileExt::read_at(handle, &mut buf, offset);
            #[cfg(windows)]
            let read = std::os::windows::fs::FileExt::seek_read(handle, &mut buf, offset);
            let n = read.map_err(|e| preserve_error(Path::new(&name), e))?;
            if n == 0 {
                break;
            }
            output.write_all(&buf[..n])?;
            offset += n as u64;
        }
        output.sync_all()?;
    }
    Ok(())
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    #[test]
    fn kernel_eio_names_the_unreadable_source() {
        use super::*;
        // Linux returns EIO for a read at an unmapped address in the process's
        // own memory file. No disk corruption, privileges or injected errno needed.
        let mut source = fs::File::open("/proc/self/mem").unwrap();
        let mut stage = tempfile::tempfile().unwrap();
        let member = Path::new("archive/agent-history.db");
        let failure = copy_bytes(member, &mut source, &mut stage).unwrap_err();
        assert!(
            matches!(&failure, Error::PreserveFile {path,source} if path==member && source.raw_os_error()==Some(5))
        );
        assert!(failure.to_string().contains("nothing was changed"));
        assert_eq!(stage.metadata().unwrap().len(), 0);
    }
}
