//! Opens role groups and publishes complete worker pools under one owner.
//! The application decides layout membership, missing-file policy and root admission.

use crate::{
    Config, Db, Error, OpenStage, OwnerLock, Result, Store, StoreOpenFailure,
    checkpoint::Gate,
    engine,
    worker::{self, Workers},
};
use std::{
    sync::{Arc, Mutex, RwLock},
    thread,
};

fn conversation_role(db: Db) -> bool {
    vault_layout::spec(vault_layout::db_artifact(db)).scope == vault_layout::DataScope::AgentHistory
}

fn conversations(config: &Config) -> Vec<Db> {
    config.databases.iter().copied().filter(|db| conversation_role(*db)).collect()
}

impl Store {
    /// Strict open for standalone archives, staged copies and caller-selected roles.
    /// Retains the historical top-level error variants for existing callers.
    pub fn open(config: Config) -> Result<Self> {
        Self::open_detailed(config).map_err(|error| match error {
            Error::Open(failure) => match Arc::try_unwrap(failure) {
                Ok(failure) => failure.cause,
                Err(shared) => Error::Open(shared),
            },
            other => other,
        })
    }

    /// Strict open retaining the role and stage of a database failure.
    pub fn open_detailed(mut config: Config) -> Result<Self> {
        config.protect_existing_files()?;
        let config = &config;
        let store = Self::reserve(config)?;
        preflight(config, &config.databases)?;
        engine::check_paths(config, &config.databases)?;
        store.start_with_scratch(config, &config.databases)?;
        Ok(store)
    }

    /// Preflight both groups before the first migration or Scratch reset. The owner
    /// runs its boundary checkpoint after browsing starts and before the second group starts.
    pub fn open_grouped(
        mut config: Config,
        between_groups: impl FnOnce() -> Result<()>,
    ) -> Result<Self> {
        config.validate_grouped()?;
        config.protect_existing_files()?;
        let config = &config;
        let store = Self::reserve(config)?;
        let browsing: Vec<_> =
            config.databases.iter().copied().filter(|db| !conversation_role(*db)).collect();
        preflight(config, &browsing)?;
        engine::check_paths(config, &browsing)?;
        let conversations = preflight_conversations(config);
        if let Err(error) = conversations {
            store.record_isolated_failure(error)?;
        }
        store.start_with_scratch(config, &browsing)?;
        between_groups()?;
        if store.conversation_availability().is_ok()
            && let Err(error) = store.publish_conversations(config)
        {
            store.record_isolated_failure(error)?;
        }
        Ok(store)
    }

    fn record_isolated_failure(&self, error: Error) -> Result<()> {
        match error {
            Error::Open(failure)
                if conversation_role(failure.role)
                    && !matches!(
                        failure.cause,
                        Error::WrongKey { .. }
                            | Error::InvalidConfig(_)
                            | Error::Closed
                            | Error::JobPanicked { .. }
                    ) =>
            {
                *self.conversation_failure.write().unwrap() = Some(failure);
                Ok(())
            }
            other => Err(other),
        }
    }

    fn reserve(config: &Config) -> Result<Self> {
        worker::check_caller()?;
        config.validate()?;
        let lock = match &config.owner {
            Some(owner) if owner.root == config.root => owner.clone(),
            Some(_) => return Err(Error::InvalidConfig("owner belongs to another root".into())),
            None => Arc::new(OwnerLock::acquire(&config.root)?),
        };
        Ok(Self {
            workers: RwLock::default(),
            ownership: Mutex::new(Some(lock)),
            adding: Mutex::default(),
            conversation_failure: RwLock::default(),
            closed: false.into(),
            paths: config.paths.clone(),
            watch: crate::running::Watch::start(config.long_statement_after)?,
        })
    }

    fn start_with_scratch(&self, config: &Config, roles: &[Db]) -> Result<()> {
        engine::reset_scratch(config).map_err(|e| e.at(Db::Scratch, OpenStage::Open))?;
        let ready = self.start_workers(config, roles, &[])?;
        self.workers_mut().extend(ready);
        Ok(())
    }

    /// Rechecks both conversation databases against the live paths, then migrates on
    /// private workers. Browsing reads continue until the short publication write lock.
    pub fn retry_conversations(&self, mut config: Config) -> Result<()> {
        worker::check_caller()?;
        let _adding = self.adding.lock().unwrap();
        let owner = self.owner()?;
        if !config.owner.as_ref().is_some_and(|o| Arc::ptr_eq(o, &owner))
            || config.paths != self.paths
        {
            return Err(Error::InvalidConfig(
                "group must use this owner's existing databases".into(),
            ));
        }
        if self.conversation_availability().is_ok() {
            return Ok(());
        }
        config.validate_grouped()?;
        config.protect_existing_files()?;
        let config = &config;
        let result =
            preflight_conversations(config).and_then(|()| self.publish_conversations(config));
        if let Err(Error::Open(reason)) = &result {
            *self.conversation_failure.write().unwrap() = Some(reason.clone());
        }
        result
    }

    fn publish_conversations(&self, config: &Config) -> Result<()> {
        let live = self.workers();
        if !self.is_open() {
            return Err(Error::Closed);
        }
        let ready = self.start_workers(config, &conversations(config), &live)?;
        drop(live);
        let mut published = self.workers_mut();
        if !self.is_open() {
            return Err(Error::Closed);
        }
        published.extend(ready);
        *self.conversation_failure.write().unwrap() = None;
        Ok(())
    }

    /// The role failure that keeps this owner from supplying its complete layout.
    pub fn conversation_availability(&self) -> std::result::Result<(), Arc<StoreOpenFailure>> {
        self.conversation_failure.read().unwrap().clone().map_or(Ok(()), Err)
    }

    fn start_workers(
        &self,
        config: &Config,
        roles: &[Db],
        live: &[(Db, Workers)],
    ) -> Result<Vec<(Db, Workers)>> {
        let mut ready = Vec::new();
        for db in vault_layout::startup_databases().into_iter().filter(|db| roles.contains(db)) {
            let gate = |id| -> Option<Arc<Gate>> {
                live.iter().chain(&ready).find(|(db, _)| *db == id).map(|(_, w)| w.gate.clone())
            };
            let mut settings = engine::Settings::new(config, db);
            let scratch = settings.scratch.as_ref().and_then(|_| gate(Db::Scratch));
            let archive = settings.archive.as_ref().and_then(|_| gate(Db::Archive));
            let readers = config.readers_per_database.get();
            settings.watched = Some((
                self.watch.slot(format!("{db:?} writer")),
                (0..readers).map(|i| self.watch.slot(format!("{db:?} reader {i}"))).collect(),
            ));
            let worker = Workers::start(
                db,
                settings,
                config.migrations.plan(db),
                readers,
                config.interactive_queue_capacity.get(),
                scratch,
                archive,
            )
            .map_err(|e| e.at(db, OpenStage::Open))?;
            ready.push((db, worker));
        }
        Ok(ready)
    }
}

fn preflight_conversations(config: &Config) -> Result<()> {
    // An unreadable or missing member cannot hide an alias between another member
    // and a browsing writer. Validate identities before asking the engine for either schema.
    engine::check_paths(config, &config.databases)?;
    preflight(config, &conversations(config))
}

fn preflight(config: &Config, roles: &[Db]) -> Result<()> {
    // Turso drives a current-thread runtime; callers may already be on a runtime.
    thread::scope(|scope| {
        thread::Builder::new()
            .name("store-preflight".into())
            .spawn_scoped(scope, || engine::preflight(config, roles))?
            .join()
            .map_err(|_| Error::JobPanicked { message: "opening preflight".into() })?
    })
}
