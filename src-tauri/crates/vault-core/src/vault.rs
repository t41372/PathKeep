//! Responsibilities: own the shared Store, paths and config, and open the password envelope.
//! Not responsible for: native keychain I/O, browser staging, IPC or indexing.
//! Failure modes: omitted migrations, two owners, incorrect database paths,
//! unfinished runs surviving reopen as running, lost committed progress, ignored keys, reused sub-keys, invalid envelopes, unchecked upgrade passwords and lost envelopes creating new keys.
//! Tests: vault::tests, using real plaintext and encrypted temporary stores.

pub(crate) mod live;
mod password;
pub use password::ArchivePasswordWrong;
#[cfg(test)]
mod tests;

use crate::{AppConfig, ProjectPaths};
use std::sync::Arc;
use vault_store::{Db, Priority, Result, Store};

// The owner can receive password-unlock and remember-key requests concurrently.
// Both must observe the same first envelope before either publishes a data key.
static ENVELOPE_ACCESS: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// The application's one storage owner. Share this handle across blocking jobs.
#[derive(Clone)]
pub struct Vault {
    store: Arc<Store>,
    paths: ProjectPaths,
    config: AppConfig,
    backup: Arc<std::sync::Mutex<()>>,
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
        mut paths: ProjectPaths,
        config: AppConfig,
        key: Option<&vault_crypto::DataKey>,
        owner: Option<Arc<vault_store::OwnerLock>>,
    ) -> Result<Self> {
        let storage = storage_config(&paths, key, owner)?;
        paths.archive_database_path = paths.app_root.join("archive/history-vault.db");
        paths.source_evidence_database_path = paths.app_root.join("archive/source-evidence.db");
        paths.intelligence_database_path = paths.derived_dir.join("intelligence.db");
        paths.agent_database_path = paths.derived_dir.join("agent.db");
        let store = Arc::new(Store::open(storage)?);
        store.write(Db::Archive, Priority::Interactive, |s| {
            s.execute(
                "UPDATE runs SET status='interrupted', finished_at=?1
                 WHERE status='running' AND run_type IN ('backup','import','snapshot_restore','rollback','restore')",
                [crate::utils::now_rfc3339()],
            )?;
            s.execute("UPDATE import_batches SET status='interrupted',previous_status='interrupted' WHERE status='running'", ())?;
            Ok(())
        })?;
        live::register(&paths.app_root, &store);
        Ok(Self { store, paths, config, backup: Arc::default(), _temporary_root: None })
    }

    /// Password unlock is the only path that may create an envelope. The legacy
    /// archive authenticates upgrades before any Turso file is created.
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
    storage.migrations = crate::store_schema::migration_registry();
    Ok(storage)
}
