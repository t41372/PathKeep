//! Responsibilities: name the five stores and their caller-supplied paths/settings.
//! Not responsible for: finding application directories or selecting encryption policy.
//! Failure modes: duplicate paths create multiple writers; zero-sized pools cannot
//! serve reads; keys appear in Debug output. Tests: config tests and opening.
use crate::{Error, MigrationRegistry, Result};
use std::{
    num::NonZeroUsize,
    path::{Path, PathBuf},
    sync::Arc,
};

/// Stable database identities keep business callers independent of file layout.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(usize)]
pub enum Db {
    Archive,
    SourceEvidence,
    Intelligence,
    Agent,
    Scratch,
}
impl Db {
    pub const ALL: [Self; 5] =
        [Self::Archive, Self::SourceEvidence, Self::Intelligence, Self::Agent, Self::Scratch];
}

/// Caller-owned temp-file policy; vault-store does not implement a cipher.
pub type IoLayer = Arc<dyn turso::core::IO>;

/// Explicit paths allow callers to select a root without relying on platform globals.
#[derive(Clone, Debug)]
pub struct Paths {
    pub archive: PathBuf,
    pub source_evidence: PathBuf,
    pub intelligence: PathBuf,
    pub agent: PathBuf,
    pub scratch: PathBuf,
}
impl Paths {
    pub fn under(root: &Path) -> Self {
        Self {
            archive: root.join("archive/history-vault.db"),
            source_evidence: root.join("archive/source-evidence.db"),
            intelligence: root.join("derived/intelligence.db"),
            agent: root.join("derived/agent.db"),
            scratch: root.join("derived/scratch.db"),
        }
    }
    pub fn get(&self, db: Db) -> &Path {
        match db {
            Db::Archive => &self.archive,
            Db::SourceEvidence => &self.source_evidence,
            Db::Intelligence => &self.intelligence,
            Db::Agent => &self.agent,
            Db::Scratch => &self.scratch,
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
    pub paths: Paths,
    pub key: Option<[u8; 32]>,
    pub io: Option<IoLayer>,
    pub readers_per_database: NonZeroUsize,
    pub page_cache_kib: NonZeroUsize,
    pub interactive_queue_capacity: NonZeroUsize,
    pub migrations: MigrationRegistry,
}
impl Config {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        let root = root.into();
        let paths = Paths::under(&root);
        Self {
            root,
            paths,
            key: None,
            io: None,
            readers_per_database: NonZeroUsize::new(2).unwrap(),
            page_cache_kib: NonZeroUsize::new(8192).unwrap(),
            interactive_queue_capacity: NonZeroUsize::new(64).unwrap(),
            migrations: MigrationRegistry::default(),
        }
    }
    pub(crate) fn validate(&self) -> Result<()> {
        for (i, db) in Db::ALL.iter().enumerate() {
            let path = self.paths.get(*db);
            if path.to_str().is_none() {
                return Err(Error::InvalidConfig("Turso requires UTF-8 paths".into()));
            }
            if Db::ALL[..i].iter().any(|other| self.paths.get(*other) == path) {
                return Err(Error::InvalidConfig("database paths must be distinct".into()));
            }
        }
        self.migrations.validate()
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
