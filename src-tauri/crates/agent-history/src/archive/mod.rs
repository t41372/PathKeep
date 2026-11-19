//! Responsibilities: open standalone archives and attach conversation services to their storage owner.
//! Not responsible for: source discovery, native parsing, UI policy or harness mutation.
//!
//! Raw evidence and normalized queries use separate stores, under the two roles PathKeep's own
//! archive gives them (`Db::AgentHistoryEvidence`, `Db::AgentHistoryReadModel`) and at the same
//! paths. A standalone archive has its own data key, which keeps it independent from PathKeep's
//! browser vault and assistant runtime.
mod attachment_counts;
mod attachment_outcomes;
mod chunks;
mod comparison;
mod conversation;
#[cfg(test)]
mod damage_tests;
mod durability;
mod generations;
mod lifetime;
mod lineage;
mod maintenance;
#[cfg(test)]
mod membership_tests;
mod observations;
mod parse_gaps;
mod projection;
#[cfg(test)]
mod read_concurrency_tests;
mod reader;
mod references;
mod reports;
mod retained_input;
mod retention;
mod revisions;
mod schema;
mod selection;
mod sources;
mod standalone;
mod user_state;

pub use lifetime::ArchivePause;

pub use observations::FolderObservation;
pub use projection::ProjectionAttempt;
pub use reader::RawReader;
pub(crate) use reader::expired_read;
pub use reports::BackupRun;
pub(crate) use retained_input::RetainedInput;
pub use retention::SLICE_BYTES;
pub use revisions::{RevisionInfo, RevisionOutcome, SourceInfo};
pub use schema::schema_steps;
pub use user_state::ConversationMark;

use crate::{Error, Result};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, atomic::AtomicBool},
};
use vault_store::{Db, Store};

/// Resources authorized by the owner of the store. Only an encrypted index needs a subkey.
pub struct AgentHistoryResources {
    /// Dedicated rebuildable directory. Synchronization may replace its contents.
    pub search_path: PathBuf,
    pub search_key: Option<vault_crypto::SubKey>,
}

/// A single owner keeps evidence, projections and the selected encryption mode consistent.
/// Open and query it from blocking workers; the storage facade owns its database threads.
pub struct Archive {
    pub(crate) store: Arc<Store>,
    pub(crate) search_path: PathBuf,
    pub(crate) search_key: Mutex<Option<Arc<vault_crypto::SubKey>>>,
    pub(crate) jobs: Arc<lifetime::Jobs>,
    pub(crate) root: PathBuf,
    pub(crate) search_gate: Mutex<()>,
    state_mirror_pending: AtomicBool,
    pub(crate) state_gate: Mutex<()>,
    pub(crate) publication_gate: Mutex<()>,
}

impl Archive {
    /// Recovers an owner's existing conversation roles without opening another store.
    /// The owner serializes attachment and drains file users before closing the store.
    pub fn from_store(store: Arc<Store>, resources: AgentHistoryResources) -> Result<Self> {
        let root = fs::canonicalize(store.owner()?.root())?;
        for role in [Db::AgentHistoryEvidence, Db::AgentHistoryReadModel] {
            let encrypted = store.database_encrypted(role).ok_or_else(|| {
                Error::InvalidArchive(format!("missing conversation role {role:?}"))
            })?;
            if encrypted != resources.search_key.is_some() {
                return Err(Error::InvalidInput(
                    "search and conversation databases require the same encryption mode".into(),
                ));
            }
        }
        if resources
            .search_key
            .as_ref()
            .is_some_and(|key| key.purpose() != vault_crypto::KeyPurpose::AgentHistorySearchIndex)
        {
            return Err(Error::InvalidInput("incorrect conversation search key purpose".into()));
        }
        let archive = Self {
            store,
            search_path: resources.search_path,
            search_key: Mutex::new(resources.search_key.map(Arc::new)),
            jobs: Arc::default(),
            root,
            search_gate: Mutex::new(()),
            state_mirror_pending: AtomicBool::new(false),
            state_gate: Mutex::new(()),
            publication_gate: Mutex::new(()),
        };
        archive.recover_unfinished_revisions()?;
        archive.reclaim_slices()?;
        archive.recover_projection_work()?;
        archive.recover_attachment_counts()?;
        archive.recover_backup_runs()?;
        archive.synchronize_user_state()?;
        Ok(archive)
    }

    /// Identifies the explicitly selected archive without exposing keys or source data.
    pub fn root(&self) -> &Path {
        &self.root
    }
}
