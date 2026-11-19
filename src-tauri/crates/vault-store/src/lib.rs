//! Responsibilities: expose the synchronous storage boundary.
//! Not responsible for: business SQL, IPC, encryption or key derivation.
//! Failure modes: jobs escape their owner thread; reentrant calls deadlock; shutdown
//! leaves workers or the ownership lock alive. Tests: ownership, concurrency, api.
mod cancel;
mod checkpoint;
mod config;
mod copy;
mod engine;
mod engine_log;
mod error;
mod existing_files;
mod maintenance;
#[cfg(windows)]
mod maintenance_windows;
mod migration;
mod opening;
mod pair;
#[cfg(any(windows, test))]
mod windows_disposition;
pub use pair::SCHEMA as PAIR_SCHEMA;
mod owner_lock;
mod query_plans;
mod queue;
mod running;
mod session;
mod statement_cache;
pub use query_plans::QueryPlan;
mod sql;
mod statements;
mod worker;

pub use cancel::CancelToken;
pub use checkpoint::Metrics;
pub use config::{Config, Db, IoLayer, Paths};
pub use copy::{CopyPhase, CopyProgress, TableDigest};
pub use engine_log::route_engine_diagnostics_to_log;
pub use error::{ArchiveFileRole, Error, OpenStage, Result, StoreOpenFailure};
pub use maintenance::{HeldGeneration, Maintenance, copy_archive_file};
pub use migration::{BackfillProgress, Migration, MigrationRegistry};
pub use owner_lock::OwnerLock;
pub use queue::Priority;
pub use session::Session;
pub use statements::{MappedRows, OptionalRow, Rows, Statement};
use std::sync::{
    Arc, Mutex, PoisonError, RwLock, RwLockReadGuard, RwLockWriteGuard, TryLockError,
    atomic::{AtomicBool, Ordering},
};
pub use turso::{IntoParams, Row, Value, params, params_from_iter};

type HeldPublisher = dyn Fn(HeldGeneration<'_>) -> Result<()> + Send + Sync;

/// All handles are joined before the ownership file is released. Share Store via
/// Arc between blocking command threads; a closure cannot recursively call it.
pub struct Store {
    workers: RwLock<Vec<(Db, worker::Workers)>>,
    /// Serializes attempts to add workers. Ordinary reads never take this lock.
    adding: Mutex<()>,
    retired_files: Mutex<Vec<(u64, Db, worker::HeldFiles)>>,
    conversation_generation: std::sync::atomic::AtomicU64,
    held_publisher: std::sync::OnceLock<Box<HeldPublisher>>,
    rejected_pair: AtomicBool,
    initial_access_refusal: Mutex<Option<[Option<turso::core::io::FileId>; 2]>>,
    pending_anchors: Mutex<Vec<(Db, turso::core::io::FileId)>>,
    availability_observer: std::sync::OnceLock<Box<dyn Fn() + Send + Sync>>,
    conversation_failure: RwLock<Option<Arc<StoreOpenFailure>>>,
    ownership: Mutex<Option<Arc<owner_lock::OwnerLock>>>,
    closed: AtomicBool,
    paths: Paths,
    pair_seals: bool,
    /// After `workers`, whose connections it watches: it stops when the store is dropped.
    watch: running::Watch,
}
impl Store {
    /// The file admitted for this role, also for owners opened on a private copy.
    pub fn database_path(&self, db: Db) -> &std::path::Path {
        self.paths.get(db)
    }
    /// Keep the originating role and archive member when recovery adds its stage.
    pub fn role_error(&self, error: Error, db: Db, stage: OpenStage) -> Error {
        error.at_file(db, stage, self.database_path(db))
    }
    /// Keep the ownership inode reserved across a verified maintenance replacement.
    pub fn owner(&self) -> Result<Arc<OwnerLock>> {
        self.ownership.lock().unwrap().clone().ok_or(Error::Closed)
    }
    /// An attached index must match its database's protection. An absent role has no mode.
    pub fn database_encrypted(&self, db: Db) -> Option<bool> {
        self.workers().iter().find(|(role, _)| *role == db).map(|(_, workers)| workers.encrypted)
    }
    /// Reports availability without waiting on a database connection.
    pub fn is_open(&self) -> bool {
        !self.closed.load(std::sync::atomic::Ordering::Acquire)
    }
    /// Serve one short committed snapshot on an independent reader connection.
    pub fn read<T: Send + 'static>(
        &self,
        db: Db,
        work: impl FnOnce(&Session<'_>) -> Result<T> + Send + 'static,
    ) -> Result<T> {
        self.request(db, true, Priority::Interactive, work)
    }
    /// Commit one complete job atomically. A batch producer submits its next batch
    /// only after this call returns, keeping the queued batch count bounded to one.
    pub fn write<T: Send + 'static>(
        &self,
        db: Db,
        priority: Priority,
        work: impl FnOnce(&Session<'_>) -> Result<T> + Send + 'static,
    ) -> Result<T> {
        self.request(db, false, priority, work)
    }
    /// Inspect admission and maintenance without issuing database work.
    pub fn metrics(&self, db: Db) -> Metrics {
        let workers = match self.workers.try_read() {
            Ok(workers) => workers,
            Err(TryLockError::Poisoned(poisoned)) => poisoned.into_inner(),
            Err(TryLockError::WouldBlock) => return Metrics::default(),
        };
        workers.iter().find(|(id, _)| *id == db).map(|(_, w)| w.metrics()).unwrap_or_default()
    }
    /// Enables opt-in plan evidence without changing the application query path.
    pub fn capture_query_plans(&self, db: Db, enabled: bool) -> Result<()> {
        let workers = self.workers();
        workers.iter().find(|(id, _)| *id == db).ok_or(Error::Closed)?.1.plans.set_enabled(enabled);
        Ok(())
    }
    /// Drains the statements captured since the last call whose program opens an ephemeral table,
    /// once per run: each such run creates a temporary file (`temp_store=FILE`).
    pub fn take_ephemeral_tables(&self, db: Db) -> Result<Vec<String>> {
        let workers = self.workers();
        Ok(workers.iter().find(|(id, _)| *id == db).ok_or(Error::Closed)?.1.plans.take_ephemeral())
    }
    /// Drains captured plans; callers decide which scans and sorts are acceptable.
    pub fn take_query_plans(&self, db: Db) -> Result<Vec<QueryPlan>> {
        let workers = self.workers();
        Ok(workers.iter().find(|(id, _)| *id == db).ok_or(Error::Closed)?.1.plans.take())
    }
    fn request<T: Send + 'static>(
        &self,
        db: Db,
        read: bool,
        priority: Priority,
        work: impl FnOnce(&Session<'_>) -> Result<T> + Send + 'static,
    ) -> Result<T> {
        worker::check_caller()?;
        if self.closed.load(Ordering::Acquire) {
            return Err(Error::Closed);
        }
        if !read && opening::conversation_role(db) && self.held_publisher.get().is_some() {
            // Preservation may drain the group, so check before taking its workers guard.
            self.preserve_observed_displacements()?;
        }
        let workers = self.workers();
        let worker = &workers.iter().find(|(id, _)| *id == db).ok_or(Error::Closed)?.1;
        if !self.pair_seals || !opening::conversation_role(db) {
            return worker.request(read, priority, work);
        }
        self.conversation_availability().map_err(Error::Open)?;
        match worker.request(read, priority, work) {
            Err(cause)
                if matches!(cause, Error::Corrupt { .. } | Error::WrongKey { .. })
                    || (matches!(cause, Error::Engine(_) | Error::Busy { .. })
                        && cause.access_refused())
                    || (matches!(cause, Error::JobPanicked { .. })
                        && worker.integrity_check_failed()) =>
            {
                let stage = if read { OpenStage::Read } else { OpenStage::Write };
                let failure = Arc::new(StoreOpenFailure {
                    role: db,
                    stage,
                    cause,
                    path: self.paths.get(db).to_owned(),
                });
                self.quarantine(&workers, failure.clone());
                Err(Error::Open(failure))
            }
            result => result,
        }
    }
    /// Revoke retained handles on explicit session clear or shutdown, finish admitted
    /// jobs, then release connections and ownership. App Lock leaves this Store open.
    pub fn close(&self) -> Result<()> {
        worker::check_caller()?;
        let mut workers = self.workers_mut();
        self.protect_held(&workers)?;
        let sealed = if workers.iter().any(|(_, w)| *w.held_saved.lock().unwrap()) {
            Ok(())
        } else {
            self.seal_if_available(&workers)
        };
        self.closed.store(true, Ordering::Release);
        while let Some((_, worker)) = workers.pop() {
            drop(worker);
        }
        self.retired_files.lock().unwrap().clear();
        self.ownership.lock().unwrap().take();
        sealed
    }
    fn seal_if_available(&self, workers: &[(Db, worker::Workers)]) -> Result<()> {
        if self.pair_seals && self.conversation_availability().is_ok() {
            self.protect_held(workers)?;
            for (db, w) in workers.iter().filter(|(db, _)| opening::conversation_role(*db)) {
                maintenance::check_identity(*db, w, self.paths.get(*db))?;
            }
            pair::seal(workers, &self.paths)?;
        }
        Ok(())
    }
    /// The workers, also after a panic poisoned their lock. The maintenance pause holds them
    /// locked across its caller's code, so a caller that panics there poisons the lock while the
    /// list stays as it was. Group publication and close take their own exclusive guard. The store goes on
    /// serving, and closing it, from `Drop` too, never panics a second time, which during the
    /// caller's unwinding would abort the process and hide the first panic.
    fn workers(&self) -> RwLockReadGuard<'_, Vec<(Db, worker::Workers)>> {
        self.workers.read().unwrap_or_else(PoisonError::into_inner)
    }
    /// [`Store::workers`], to pause or to close.
    fn workers_mut(&self) -> RwLockWriteGuard<'_, Vec<(Db, worker::Workers)>> {
        self.workers.write().unwrap_or_else(PoisonError::into_inner)
    }
}
impl Drop for Store {
    fn drop(&mut self) {
        let _ = self.close();
    }
}
