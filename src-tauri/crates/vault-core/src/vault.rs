//! Responsibilities: own Store/config, recover queue pause ownership and open the password envelope.
//! Not responsible for: native keychain I/O, browser staging, IPC or search semantics.
//! Failure modes: omitted migrations, two owners, incorrect database paths,
//! unfinished runs surviving reopen as running, lost committed progress, ignored keys, reused sub-keys, invalid envelopes, unchecked upgrade passwords and lost envelopes creating new keys.
//! Tests: vault::tests, using real plaintext and encrypted temporary stores.

pub(crate) mod live;
pub(crate) mod password;
pub use password::ArchivePasswordWrong;
#[cfg(test)]
mod tests;

use crate::{AppConfig, ProjectPaths};
use std::sync::Arc;
use vault_store::{Db, Priority, Result, Store};

// The owner can receive password-unlock and remember-key requests concurrently.
// Both must observe the same first envelope before either publishes a data key.
pub(crate) static ENVELOPE_ACCESS: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// The application's one storage owner. Share this handle across blocking jobs.
#[derive(Clone)]
pub struct Vault {
    // Shared owner lifetime closes retained service handles before artifact roots drop.
    lifetime: Arc<Lifetime>,
    search_sessions: Arc<crate::search::SearchSessions>,
    search: Arc<crate::search::Indexer>,
    store: Arc<Store>,
    paths: ProjectPaths,
    config: AppConfig,
    backup: Arc<std::sync::Mutex<()>>,
    intelligence: Arc<std::sync::Mutex<()>>,
    site_days: Arc<crate::intelligence::SiteDayCache>,
    // Must outlive Store's threads and files.
    _temporary_root: Option<Arc<tempfile::TempDir>>,
}

impl Vault {
    /// Opens databases from the raw data key returned by password unlock or the keychain.
    pub fn open(paths: ProjectPaths, config: AppConfig, mut key: Option<[u8; 32]>) -> Result<Self> {
        use zeroize::Zeroize;
        let data_key = key
            .as_ref()
            .map(|bytes| vault_crypto::DataKey::from_bytes(zeroize::Zeroizing::new(*bytes)));
        key.zeroize();
        Self::open_owned(paths, config, data_key.as_ref(), None)
    }

    /// Use the app's startup reservation for the same root, before IPC accepts requests.
    pub fn open_owned(
        paths: ProjectPaths,
        config: AppConfig,
        key: Option<&vault_crypto::DataKey>,
        owner: Option<Arc<vault_store::OwnerLock>>,
    ) -> Result<Self> {
        Self::open_database(paths, config, key, owner, true)
    }

    /// A staged maintenance copy preserves its run ledger until publication.
    pub fn open_staged(
        paths: ProjectPaths,
        config: AppConfig,
        key: Option<&vault_crypto::DataKey>,
    ) -> Result<Self> {
        Self::open_database(paths, config, key, None, false)
    }

    fn open_database(
        paths: ProjectPaths,
        config: AppConfig,
        key: Option<&vault_crypto::DataKey>,
        owner: Option<Arc<vault_store::OwnerLock>>,
        recover: bool,
    ) -> Result<Self> {
        let owner = match owner {
            Some(owner) => owner,
            None => Arc::new(vault_store::OwnerLock::acquire(&paths.app_root)?),
        };
        let recovered = crate::maintenance::recover_native_maintenance(&paths)
            .map_err(|e| vault_store::Error::Operation(e.into_boxed_dyn_error()))?;
        let config = if recovered {
            crate::load_config(&paths)
                .map_err(|e| vault_store::Error::Operation(e.into_boxed_dyn_error()))?
        } else {
            config
        };
        let storage = storage_config(&paths, key, Some(owner))?;
        let store = Arc::new(Store::open(storage)?);
        if recover {
            store.write(Db::Archive, Priority::Interactive, |s| {
            s.execute(
                // One process owns the archive, so a run still running when it opens was cut
                // short, whatever its kind (a retention prune, a repair, an index build).
                "UPDATE runs SET status='interrupted', finished_at=?1 WHERE status='running'",
                [crate::utils::now_rfc3339()],
            )?;
            s.execute("UPDATE import_batches SET status='interrupted',previous_status='interrupted' WHERE status='running'", ())?;
            Ok(())
        })?;
            crate::ai_queue::recover_owner_claims(&store, config.ai.job_queue_paused)?;
            crate::intelligence_runtime::recover_owner_claims(&store)?;
        }
        let search = Arc::new(
            crate::search::Indexer::open(store.clone(), &paths.search_index_dir, key, !recover)
                .map_err(|e| vault_store::Error::Operation(e.into_boxed_dyn_error()))?,
        );
        let search_sessions = Arc::default();
        live::register(&paths.app_root, &store);
        live::register_search(&paths.app_root, &search, &search_sessions);
        let lifetime = Arc::new(Lifetime {
            store: Arc::downgrade(&store),
            search: Arc::downgrade(&search),
            sessions: Arc::downgrade(&search_sessions),
        });
        Ok(Self {
            lifetime,
            search_sessions,
            search,
            store,
            paths,
            config,
            backup: Arc::default(),
            intelligence: Arc::default(),
            site_days: Arc::default(),
            _temporary_root: None,
        })
    }

    /// Password unlock is the only path that may create an envelope. It refuses
    /// while an archive of the old format waits for conversion.
    pub fn open_password(
        paths: ProjectPaths,
        config: AppConfig,
        password: &str,
        owner: Option<Arc<vault_store::OwnerLock>>,
    ) -> anyhow::Result<(Self, vault_crypto::DataKey)> {
        password::open(paths, config, password, owner)
    }

    /// Unwrap only an existing published envelope.
    pub fn unlock_data_key(
        paths: &ProjectPaths,
        password: &str,
    ) -> anyhow::Result<vault_crypto::DataKey> {
        password::unlock(paths, password)
    }

    pub(crate) fn begin_backup(&self) -> anyhow::Result<std::sync::MutexGuard<'_, ()>> {
        self.backup.try_lock().map_err(|_| BackupAlreadyRunning.into())
    }

    /// Site-day reads shared by the Insights cards, kept per vault (`intelligence::site_days`).
    pub(crate) fn site_days(&self) -> &Arc<crate::intelligence::SiteDayCache> {
        &self.site_days
    }

    pub(crate) fn intelligence_build(&self) -> std::sync::MutexGuard<'_, ()> {
        self.intelligence.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// Gives tests the same schema and ownership rules as a real archive.
    pub fn open_temp() -> Result<Self> {
        Self::open_temp_with_key(None)
    }

    /// Exercises native encryption without involving the application's key envelope.
    pub fn open_temp_with_key(mut key: Option<[u8; 32]>) -> Result<Self> {
        let root = tempfile::tempdir()?;
        let paths = crate::project_paths_with_root(root.path());
        let config = AppConfig {
            initialized: true,
            git_enabled: false,
            archive_mode: if key.is_some() {
                crate::ArchiveMode::Encrypted
            } else {
                crate::ArchiveMode::Plaintext
            },
            ..AppConfig::default()
        };
        use zeroize::Zeroize;
        let opened = Self::open(paths, config, key);
        key.zeroize();
        let mut vault = opened?;
        vault._temporary_root = Some(Arc::new(root));
        Ok(vault)
    }

    /// Routes requests to the single owner rather than opening per-operation connections.
    pub fn store(&self) -> &Arc<Store> {
        &self.store
    }

    /// The one indexer survives App Lock; closing a Vault joins it before Store shutdown.
    pub fn search(&self) -> &Arc<crate::search::Indexer> {
        &self.search
    }

    pub fn search_sessions(&self) -> &Arc<crate::search::SearchSessions> {
        &self.search_sessions
    }

    pub fn close(&self) -> Result<()> {
        self.lifetime.close()
    }

    /// Artifact paths accompany the database owner through backup and import jobs.
    pub fn paths(&self) -> &ProjectPaths {
        &self.paths
    }

    /// The configuration snapshot used to open this vault.
    pub fn config(&self) -> &AppConfig {
        &self.config
    }

    /// New requests can use saved settings without opening another database owner.
    pub fn with_config(&self, config: AppConfig) -> Self {
        Self { config, ..self.clone() }
    }

    /// Preserve domain errors while Store remains responsible for rollback.
    pub(crate) fn write<T: Send + 'static>(
        &self,
        db: Db,
        priority: Priority,
        work: impl FnOnce(&vault_store::Session<'_>) -> anyhow::Result<T> + Send + 'static,
    ) -> anyhow::Result<T> {
        Ok(self.store.write(db, priority, move |s| {
            work(s).map_err(|error| vault_store::Error::Operation(error.into_boxed_dyn_error()))
        })?)
    }

    pub(crate) fn read<T: Send + 'static>(
        &self,
        db: Db,
        work: impl FnOnce(&vault_store::Session<'_>) -> anyhow::Result<T> + Send + 'static,
    ) -> anyhow::Result<T> {
        Ok(self.store.read(db, move |s| {
            work(s).map_err(|error| vault_store::Error::Operation(error.into_boxed_dyn_error()))
        })?)
    }
}

impl std::fmt::Debug for Vault {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Vault").field("root", &self.paths.app_root).finish_non_exhaustive()
    }
}

/// A missing envelope cannot be replaced without losing access to existing files.
#[derive(Debug)]
pub struct EnvelopeMissing;
impl std::fmt::Display for EnvelopeMissing {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("envelope-missing: the encrypted archive's key envelope is missing")
    }
}
impl std::error::Error for EnvelopeMissing {}

#[derive(Debug)]
pub struct BackupAlreadyRunning;
impl std::fmt::Display for BackupAlreadyRunning {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("backup-already-running: a backup is already in progress")
    }
}
impl std::error::Error for BackupAlreadyRunning {}

fn storage_config(
    paths: &ProjectPaths,
    key: Option<&vault_crypto::DataKey>,
    owner: Option<Arc<vault_store::OwnerLock>>,
) -> Result<vault_store::Config> {
    let mut storage = vault_store::Config::new(&paths.app_root);
    storage.owner = owner;
    if let Some(data_key) = key {
        use vault_crypto::KeyPurpose;
        storage.keys = [
            KeyPurpose::ArchiveDatabase,
            KeyPurpose::SourceEvidenceDatabase,
            KeyPurpose::IntelligenceDatabase,
            KeyPurpose::AgentDatabase,
            KeyPurpose::ScratchDatabase,
        ]
        .map(|purpose| Some(*data_key.derive(purpose).expose_bytes()));
        storage.io = Some(
            vault_crypto::TempCryptIo::new(data_key)
                .map_err(|e| vault_store::Error::Operation(Box::new(e)))?,
        );
    }
    if storage.io.is_none() {
        // A plaintext owner can later ATTACH an encrypted export destination.
        // Its index-build spills must already pass through encrypted IO.
        let mut bytes = zeroize::Zeroizing::new([0u8; 32]);
        getrandom::fill(&mut *bytes).map_err(|e| vault_store::Error::Operation(Box::new(e)))?;
        let ephemeral = vault_crypto::DataKey::from_bytes(bytes);
        storage.io = Some(
            vault_crypto::TempCryptIo::new(&ephemeral)
                .map_err(|e| vault_store::Error::Operation(Box::new(e)))?,
        );
    }
    storage.migrations = crate::store_schema::migration_registry();
    Ok(storage)
}

struct Lifetime {
    store: std::sync::Weak<Store>,
    search: std::sync::Weak<crate::search::Indexer>,
    sessions: std::sync::Weak<crate::search::SearchSessions>,
}
impl Lifetime {
    fn close(&self) -> Result<()> {
        if let Some(sessions) = self.sessions.upgrade() {
            sessions.close();
        }
        if let Some(search) = self.search.upgrade() {
            search.close();
        }
        if let Some(store) = self.store.upgrade() {
            store.close()?;
        }
        Ok(())
    }
}
impl Drop for Lifetime {
    fn drop(&mut self) {
        let _ = self.close();
    }
}
