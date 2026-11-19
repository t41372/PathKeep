//! Responsibilities: select stores and their caller-supplied paths/settings.
//! Not responsible for: finding application directories or selecting encryption policy.
//! Failure modes: duplicate paths create multiple writers; zero-sized pools cannot
//! serve reads; disabled stores are opened; attachments lack their dependency store;
//! keys appear in Debug output. Tests: config tests, opening and selected_databases.
use crate::{Error, MigrationRegistry, Result};
use std::{
    num::NonZeroUsize,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

pub use vault_layout::Db;

/// Caller-owned temp-file policy; vault-store does not implement a cipher.
pub type IoLayer = Arc<dyn turso::core::IO>;

/// Explicit paths allow callers to select a root without relying on platform globals.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Paths {
    pub archive: PathBuf,
    pub source_evidence: PathBuf,
    pub intelligence: PathBuf,
    pub agent: PathBuf,
    pub scratch: PathBuf,
    pub agent_history_evidence: PathBuf,
    pub agent_history_read_model: PathBuf,
}
impl Paths {
    pub fn under(root: &Path) -> Self {
        Self {
            archive: root.join(vault_layout::database_path(Db::Archive)),
            source_evidence: root.join(vault_layout::database_path(Db::SourceEvidence)),
            intelligence: root.join(vault_layout::database_path(Db::Intelligence)),
            agent: root.join(vault_layout::database_path(Db::Agent)),
            scratch: root.join(vault_layout::database_path(Db::Scratch)),
            agent_history_evidence: root
                .join(vault_layout::database_path(Db::AgentHistoryEvidence)),
            agent_history_read_model: root
                .join(vault_layout::database_path(Db::AgentHistoryReadModel)),
        }
    }
    pub fn get(&self, db: Db) -> &Path {
        match db {
            Db::Archive => &self.archive,
            Db::SourceEvidence => &self.source_evidence,
            Db::Intelligence => &self.intelligence,
            Db::Agent => &self.agent,
            Db::Scratch => &self.scratch,
            Db::AgentHistoryEvidence => &self.agent_history_evidence,
            Db::AgentHistoryReadModel => &self.agent_history_read_model,
        }
    }
}

/// Opening policy is supplied by the app; key derivation and IO encryption stay there.
/// This type deliberately has no Debug implementation, to avoid printing the key.
///
/// ```compile_fail
/// println!("{:?}", vault_store::Config::new("archive-root"));
/// ```
pub struct Config {
    pub root: PathBuf,
    /// Only these roles are opened, migrated and included in snapshots. Their order
    /// does not affect startup: Scratch precedes all stores, Archive precedes Intelligence.
    /// Every selection requires Scratch; Intelligence also requires Archive.
    /// Defaults to all roles for the desktop application. Unselected paths are ignored.
    pub databases: Vec<Db>,
    /// Existing durable files that the owner forbids the engine to initialize.
    pub required_existing: Vec<Db>,
    /// Track writes to the paired desktop conversation stores.
    pub pair_seals: bool,
    pub paths: Paths,
    pub keys: [Option<[u8; 32]>; Db::COUNT],
    pub owner: Option<Arc<crate::OwnerLock>>,
    pub io: Option<IoLayer>,
    pub readers_per_database: NonZeroUsize,
    pub page_cache_kib: NonZeroUsize,
    pub interactive_queue_capacity: NonZeroUsize,
    /// How long one statement may run before the log names it (`running.rs`).
    pub long_statement_after: Duration,
    pub migrations: MigrationRegistry,
}
impl Config {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        let root = root.into();
        let paths = Paths::under(&root);
        Self {
            root,
            databases: Db::ALL.to_vec(),
            paths,
            required_existing: Vec::new(),
            pair_seals: false,
            keys: [None; Db::COUNT],
            owner: None,
            io: None,
            readers_per_database: NonZeroUsize::new(2).unwrap(),
            page_cache_kib: NonZeroUsize::new(8192).unwrap(),
            interactive_queue_capacity: NonZeroUsize::new(64).unwrap(),
            long_statement_after: Duration::from_secs(30),
            migrations: MigrationRegistry::default(),
        }
    }
    /// Keeps the public key-slot representation tied to the declared database identities.
    pub(crate) fn key(&self, db: Db) -> Option<[u8; 32]> {
        self.keys[db as usize]
    }

    pub(crate) fn validate_grouped(&self) -> Result<()> {
        if Db::ALL.into_iter().any(|db| {
            !self.databases.contains(&db)
                || (db != Db::Scratch && !self.required_existing.contains(&db))
        }) {
            return Err(Error::InvalidConfig(
                "grouped open requires every existing desktop database".into(),
            ));
        }
        self.validate()
    }
    pub(crate) fn validate(&self) -> Result<()> {
        if self
            .required_existing
            .iter()
            .any(|db| *db == Db::Scratch || !self.databases.contains(db))
        {
            return Err(Error::InvalidConfig(
                "existing files must be selected durable databases".into(),
            ));
        }
        if self.databases.is_empty() {
            return Err(Error::InvalidConfig("at least one database must be selected".into()));
        }
        if !self.databases.contains(&vault_layout::scratch_database()) {
            return Err(Error::InvalidConfig("selected databases require Scratch".into()));
        }
        for db in &self.databases {
            for dependency in vault_layout::database_spec(*db).dependencies {
                if !self.databases.contains(dependency) {
                    return Err(Error::InvalidConfig(format!("{db:?} requires {dependency:?}")));
                }
            }
        }
        for (i, db) in self.databases.iter().enumerate() {
            if self.databases[..i].contains(db) {
                return Err(Error::InvalidConfig("database roles must be distinct".into()));
            }
            let path = self.paths.get(*db);
            if path.to_str().is_none() {
                return Err(Error::InvalidConfig("Turso requires UTF-8 paths".into()));
            }
            if self.databases[..i].iter().any(|other| self.paths.get(*other) == path) {
                return Err(Error::InvalidConfig("database paths must be distinct".into()));
            }
        }
        self.migrations.validate()
    }
}
impl Drop for Config {
    fn drop(&mut self) {
        use zeroize::Zeroize;
        self.keys.zeroize();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn duplicate_paths_are_rejected_and_defaults_are_bounded() {
        let mut config = Config::new("fixture");
        assert_eq!(config.page_cache_kib.get(), 8192);
        assert_eq!(config.readers_per_database.get(), 2);
        config.paths.agent = config.paths.archive.clone();
        assert!(config.validate().is_err());
    }
}
