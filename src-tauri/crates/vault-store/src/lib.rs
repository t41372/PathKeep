//! Responsibilities: expose the synchronous storage boundary.
//! Not responsible for: business SQL, IPC, encryption or key derivation.
//! Failure modes: jobs escape their owner thread; reentrant calls deadlock; shutdown
//! leaves workers or the ownership lock alive. Tests: ownership, concurrency, api.
mod cancel;
mod checkpoint;
mod config;
mod engine;
mod error;
mod migration;
mod owner_lock;
mod queue;
mod session;
mod sql;
mod worker;

pub use cancel::CancelToken;
pub use checkpoint::Metrics;
pub use config::{Config, Db, IoLayer, Paths};
pub use error::{Error, Result};
pub use migration::{Migration, MigrationRegistry};
pub use queue::Priority;
pub use session::Session;
pub use turso::{Value, params};

/// All handles are joined before the ownership file is released. Share Store via
/// Arc between blocking command threads; a closure cannot recursively call it.
pub struct Store {
    workers: Vec<(Db, worker::Workers)>,
    _lock: owner_lock::OwnerLock,
}
impl Store {
    /// Acquire ownership, verify every existing database, then migrate on writers.
    pub fn open(config: Config) -> Result<Self> {
        worker::check_caller()?;
        let lock = owner_lock::OwnerLock::acquire(&config.root)?;
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
        let mut store = Self { workers: Vec::new(), _lock: lock };
        // Keep scratch's database handle alive before any connection attaches it.
        for db in [Db::Scratch, Db::Archive, Db::SourceEvidence, Db::Intelligence, Db::Agent] {
            let scratch = store.workers.first().map(|(_, w)| w.gate.clone());
            let workers = worker::Workers::start(
                db,
                engine::Settings::new(&config, db),
                config.migrations.get(db),
                config.readers_per_database.get(),
                config.interactive_queue_capacity.get(),
                scratch,
            )?;
            store.workers.push((db, workers));
        }
        Ok(store)
    }
    /// Serve one short committed snapshot on an independent reader connection.
    pub fn read<T: Send + 'static>(
        &self,
        db: Db,
        work: impl FnOnce(&Session<'_>) -> Result<T> + Send + 'static,
    ) -> Result<T> {
        self.worker(db).request(true, Priority::Interactive, work)
    }
    /// Commit one complete job atomically. A batch producer submits its next batch
    /// only after this call returns, keeping the queued batch count bounded to one.
    pub fn write<T: Send + 'static>(
        &self,
        db: Db,
        priority: Priority,
        work: impl FnOnce(&Session<'_>) -> Result<T> + Send + 'static,
    ) -> Result<T> {
        self.worker(db).request(false, priority, work)
    }
    /// Inspect admission and maintenance without issuing database work.
    pub fn metrics(&self, db: Db) -> Metrics {
        self.worker(db).metrics()
    }
    fn worker(&self, db: Db) -> &worker::Workers {
        &self.workers.iter().find(|(id, _)| *id == db).unwrap().1
    }
}
impl Drop for Store {
    fn drop(&mut self) {
        while let Some((_, workers)) = self.workers.pop() {
            drop(workers)
        }
    }
}
