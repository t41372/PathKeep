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
pub use migration::{Migration, MigrationRegistry};
pub use owner_lock::OwnerLock;
pub use queue::Priority;
pub use session::Session;
pub use statements::{MappedRows, OptionalRow, Rows, Statement};
use std::sync::{
    Arc, Mutex, RwLock,
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
}
impl Store {
    /// Keep the ownership inode reserved across a verified maintenance replacement.
    pub fn owner(&self) -> Result<Arc<OwnerLock>> {
        self.ownership.lock().unwrap().clone().ok_or(Error::Closed)
    }
    /// Reports availability without waiting on a database connection.
    pub fn is_open(&self) -> bool {
        !self.closed.load(std::sync::atomic::Ordering::Acquire)
    }
    /// Acquire ownership, verify every existing database, then migrate on writers.
    pub fn open(config: Config) -> Result<Self> {
        worker::check_caller()?;
        let lock = match &config.owner {
            Some(owner) if owner.root == config.root => owner.clone(),
            Some(_) => return Err(Error::InvalidConfig("owner belongs to another root".into())),
            None => Arc::new(owner_lock::OwnerLock::acquire(&config.root)?),
        };
        config.validate()?;
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
        };
        // Keep scratch's database handle alive before any connection attaches it.
        for db in [Db::Scratch, Db::Archive, Db::SourceEvidence, Db::Intelligence, Db::Agent] {
            let scratch = store.workers.get_mut().unwrap().first().map(|(_, w)| w.gate.clone());
            let archive = if db == Db::Intelligence {
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
            let workers = worker::Workers::start(
                db,
                engine::Settings::new(&config, db),
                config.migrations.get(db),
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
        self.workers
            .try_read()
            .ok()
            .and_then(|workers| workers.iter().find(|(id, _)| *id == db).map(|(_, w)| w.metrics()))
            .unwrap_or_default()
    }
    /// Enables opt-in plan evidence without changing the application query path.
    pub fn capture_query_plans(&self, db: Db, enabled: bool) -> Result<()> {
        let workers = self.workers.read().unwrap();
        workers.iter().find(|(id, _)| *id == db).ok_or(Error::Closed)?.1.plans.set_enabled(enabled);
        Ok(())
    }
    /// Drains captured plans; callers decide which scans and sorts are acceptable.
    pub fn take_query_plans(&self, db: Db) -> Result<Vec<QueryPlan>> {
        let workers = self.workers.read().unwrap();
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
        let workers = self.workers.read().unwrap();
        let worker = &workers.iter().find(|(id, _)| *id == db).ok_or(Error::Closed)?.1;
        worker.request(read, priority, work)
    }
    /// Revoke retained handles on explicit session clear or shutdown, finish admitted
    /// jobs, then release connections and ownership. App Lock leaves this Store open.
    pub fn close(&self) -> Result<()> {
        worker::check_caller()?;
        self.closed.store(true, Ordering::Release);
        let mut workers = self.workers.write().unwrap();
        while let Some((_, worker)) = workers.pop() {
            drop(worker);
        }
        self.ownership.lock().unwrap().take();
        Ok(())
    }
}
impl Drop for Store {
    fn drop(&mut self) {
        let _ = self.close();
    }
}
