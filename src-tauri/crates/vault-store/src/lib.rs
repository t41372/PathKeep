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
mod maintenance;
mod migration;
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
pub use error::{Error, Result};
pub use maintenance::Maintenance;
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

/// All handles are joined before the ownership file is released. Share Store via
/// Arc between blocking command threads; a closure cannot recursively call it.
pub struct Store {
    workers: RwLock<Vec<(Db, worker::Workers)>>,
    ownership: Mutex<Option<Arc<owner_lock::OwnerLock>>>,
    closed: AtomicBool,
    paths: Paths,
    /// After `workers`, whose connections it watches: it stops when the store is dropped.
    watch: running::Watch,
}
impl Store {
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
    /// Acquire ownership, verify every selected database, then migrate on writers.
    pub fn open(config: Config) -> Result<Self> {
        worker::check_caller()?;
        config.validate()?;
        let lock = match &config.owner {
            Some(owner) if owner.root == config.root => owner.clone(),
            Some(_) => return Err(Error::InvalidConfig("owner belongs to another root".into())),
            None => Arc::new(owner_lock::OwnerLock::acquire(&config.root)?),
        };
        let config = std::thread::Builder::new()
            .name("store-preflight".into())
            .spawn(move || {
                engine::preflight(&config)?;
                engine::prepare_paths(&config)?;
                Ok::<_, Error>(config)
            })?
            .join()
            .map_err(|_| Error::JobPanicked { message: "opening preflight".into() })??;
        let mut store = Self {
            workers: RwLock::new(Vec::new()),
            ownership: Mutex::new(Some(lock)),
            closed: AtomicBool::new(false),
            paths: config.paths.clone(),
            watch: running::Watch::start(config.long_statement_after)?,
        };
        // Keep scratch's database handle alive before any connection attaches it.
        for db in vault_layout::startup_databases() {
            if !config.databases.contains(&db) {
                continue;
            }
            let scratch = store.workers.get_mut().unwrap().first().map(|(_, w)| w.gate.clone());
            let archive = if vault_layout::database_spec(db).dependencies.contains(&Db::Archive) {
                store
                    .workers
                    .get_mut()
                    .unwrap()
                    .iter()
                    .find(|(id, _)| *id == Db::Archive)
                    .map(|(_, w)| w.gate.clone())
            } else {
                None
            };
            let mut settings = engine::Settings::new(&config, db);
            settings.watched = Some((store.watch.slot(format!("{db:?} writer")), {
                (0..config.readers_per_database.get())
                    .map(|index| store.watch.slot(format!("{db:?} reader {index}")))
                    .collect()
            }));
            let workers = worker::Workers::start(
                db,
                settings,
                config.migrations.plan(db),
                config.readers_per_database.get(),
                config.interactive_queue_capacity.get(),
                scratch,
                archive,
            )?;
            store.workers.get_mut().unwrap().push((db, workers));
        }
        Ok(store)
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
        let workers = self.workers();
        let worker = &workers.iter().find(|(id, _)| *id == db).ok_or(Error::Closed)?.1;
        worker.request(read, priority, work)
    }
    /// Revoke retained handles on explicit session clear or shutdown, finish admitted
    /// jobs, then release connections and ownership. App Lock leaves this Store open.
    pub fn close(&self) -> Result<()> {
        worker::check_caller()?;
        self.closed.store(true, Ordering::Release);
        let mut workers = self.workers_mut();
        while let Some((_, worker)) = workers.pop() {
            drop(worker);
        }
        self.ownership.lock().unwrap().take();
        Ok(())
    }
    /// The workers, also after a panic poisoned their lock. The maintenance pause holds them
    /// locked across its caller's code, so a caller that panics there poisons the lock while the
    /// list stays as it was: once the store is open only closing it changes the list
    /// (`Store::close`, `Maintenance::close`), and neither panics halfway. The store goes on
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
