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

pub(crate) fn conversation_role(db: Db) -> bool {
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
        // The browsing roles already authenticated their derived keys. A page that
        // fails authentication here is unreadable under this role's key. Browsing
        // has already established that its own files are readable. Retain the engine's exact cause.
        match error {
            Error::Open(failure)
                if conversation_role(failure.role)
                    && !matches!(
                        failure.cause,
                        Error::InvalidConfig(_) | Error::Closed | Error::JobPanicked { .. }
                    ) =>
            {
                *self.initial_access_refusal.lock().unwrap() =
                    failure.cause.access_refused().then(|| self.conversation_file_ids());
                self.rejected_pair.store(
                    matches!(failure.cause, Error::PairUnverified),
                    std::sync::atomic::Ordering::Release,
                );
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
            conversation_writes: RwLock::default(),
            retired_files: Default::default(),
            conversation_generation: Default::default(),
            held_publisher: Default::default(),
            rejected_pair: Default::default(),
            initial_access_refusal: Default::default(),
            pending_anchors: Default::default(),
            table_writes: Default::default(),
            availability_events: Default::default(),
            conversation_failure: RwLock::default(),
            closed: false.into(),
            paths: config.paths.clone(),
            pair_seals: config.pair_seals,
            watch: crate::running::Watch::start(config.long_statement_after)?,
        })
    }

    fn start_with_scratch(&self, config: &Config, roles: &[Db]) -> Result<()> {
        engine::reset_scratch(config)
            .map_err(|e| e.at_file(Db::Scratch, OpenStage::Open, config.paths.get(Db::Scratch)))?;
        let ready = self.start_workers(config, roles, &[])?;
        self.workers_mut().extend(ready);
        Ok(())
    }

    /// Read-only verification for a candidate pair before either worker can migrate it.
    pub fn verify_conversation_pair(config: &Config) -> Result<()> {
        std::thread::scope(|scope| {
            scope
                .spawn(|| crate::engine::verify_pair(config, true, false).map(drop))
                .join()
                .map_err(|_| Error::JobPanicked { message: "verifying conversation pair".into() })?
        })
    }

    /// Legacy snapshots have file digests but no pairing ticket. Only their evidence
    /// may seed an offline rebuild. A modern mismatched pair is always refused.
    pub fn verify_copy_pair(config: &Config) -> Result<bool> {
        std::thread::scope(|scope| {
            scope
                .spawn(|| crate::engine::verify_pair(config, true, true))
                .join()
                .map_err(|_| Error::JobPanicked { message: "verifying safety copy".into() })?
        })
    }
    fn conversation_file_ids(&self) -> [Option<turso::core::io::FileId>; 2] {
        [Db::AgentHistoryEvidence, Db::AgentHistoryReadModel].map(|role| {
            turso::core::io::get_file_id(
                self.paths.get(role).to_str().expect("validated role path"),
            )
            .ok()
        })
    }
    /// Rechecks both conversation databases against the live paths, then migrates on
    /// private workers. Browsing reads continue until the short publication write lock.
    pub fn retry_conversations(&self, config: Config) -> Result<()> {
        self.retry_conversations_checked(config, |_| Ok(()))
    }
    pub fn retry_conversations_checked(
        &self,
        mut config: Config,
        verify: impl FnOnce(&Config) -> Result<()>,
    ) -> Result<()> {
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
        // A failed read or attachment may have left workers on displaced files.
        // The service has drained producers. Remove both before inspecting either pathname.
        {
            let mut old = self.workers_mut();
            self.protect_held(&old)?;
            for (db, w) in old.iter_mut().filter(|(db, _)| conversation_role(*db)) {
                w.quarantine.store(true, std::sync::atomic::Ordering::Release);
                // A failed retry must leave the unlinked generation available to a
                // subsequent safety copy even though its SQL workers must be closed.
                if crate::maintenance::displaced(*db, w, self.paths.get(*db))? {
                    self.retired_files.lock().unwrap().push((
                        self.generation(),
                        *db,
                        std::mem::take(&mut w.held_files),
                    ));
                }
            }
            old.retain(|(db, _)| !conversation_role(*db));
        }
        let result = preflight_conversations(config)
            .and_then(|()| {
                let ordinary = self.initial_access_refusal.lock().unwrap().is_some_and(|prior| {
                    prior
                        .into_iter()
                        .zip(self.conversation_file_ids())
                        .all(|(old, now)| old.is_none() || old == now)
                });
                if ordinary { Ok(()) } else { Self::verify_conversation_pair(config) }
            })
            .and_then(|()| verify(config))
            .and_then(|()| self.publish_conversations(config));
        if let Err(Error::Open(reason)) = &result {
            if !reason.cause.access_refused() {
                self.initial_access_refusal.lock().unwrap().take();
            }
            if matches!(reason.cause, Error::PairUnverified) {
                self.rejected_pair.store(true, std::sync::atomic::Ordering::Release);
            }
            let mut recorded = self.conversation_failure.write().unwrap();
            if !recorded.as_ref().is_some_and(|f| matches!(f.cause, Error::RetainedBytes { .. })) {
                *recorded = Some(reason.clone());
            }
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
        self.conversation_generation.fetch_add(1, std::sync::atomic::Ordering::AcqRel);
        self.rejected_pair.store(false, std::sync::atomic::Ordering::Release);
        self.initial_access_refusal.lock().unwrap().take();
        published.extend(ready);
        self.record_attachment_ids(&published);
        if self.conversation_failure.write().unwrap().take().is_some() {
            self.announce_availability();
        }
        Ok(())
    }

    /// Preserve the first failure across concurrent reads, including library attachment recovery.
    pub fn remember_conversation_failure(&self, reason: Arc<StoreOpenFailure>) {
        if !self.pair_seals {
            return;
        }
        debug_assert!(conversation_role(reason.role));
        self.quarantine(&self.workers(), reason);
    }

    pub(crate) fn quarantine(&self, workers: &[(Db, Workers)], reason: Arc<StoreOpenFailure>) {
        for (_, w) in workers.iter().filter(|(db, _)| conversation_role(*db)) {
            w.quarantine.store(true, std::sync::atomic::Ordering::Release);
        }
        let mut stored = self.conversation_failure.write().unwrap();
        if stored.is_none() {
            *stored = Some(reason);
            drop(stored);
            self.announce_availability();
        }
    }

    /// The owner uses this invalidation to tell its window to refetch kind status.
    pub fn on_availability_change(&self, observer: impl Fn() + Send + Sync + 'static) {
        if self.availability_events.observer.set(Box::new(observer)).is_ok()
            && self.conversation_availability().is_err()
        {
            self.announce_availability()
        }
    }
    pub(crate) fn announce_availability(&self) {
        self.availability_events.emit();
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
            .map_err(|e| e.at_file(db, OpenStage::Open, config.paths.get(db)))?;
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
