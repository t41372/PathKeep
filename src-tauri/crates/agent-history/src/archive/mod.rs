//! Responsibilities: open standalone archives and attach conversation services to their storage owner.
//! Not responsible for: source discovery, native parsing, UI policy or harness mutation.
//!
//! Raw evidence and normalized queries use separate stores, under the two roles PathKeep's own
//! archive gives them (`Db::AgentHistoryEvidence`, `Db::AgentHistoryReadModel`) and at the same
//! paths. A standalone archive has its own data key, which keeps it independent from PathKeep's
//! browser vault and assistant runtime.
mod attachment_counts;
mod attachment_outcomes;
mod capture_facts;
mod capture_notes;
mod chunks;
mod comparison;
mod conversation;
#[cfg(test)]
mod damage_tests;
mod durability;
mod generations;
pub(crate) mod lifetime;
mod lineage;
mod maintenance;
#[cfg(test)]
mod membership_tests;
mod observations;
mod parse_gaps;
pub(crate) mod projection;
#[cfg(test)]
mod read_concurrency_tests;
mod reader;
mod references;
mod reports;
mod retained_input;
mod retention;
mod retired_memberships;
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
    pub(crate) initial_search: crate::search::InitialSearch,
    pub(crate) reader_finalization_pending: AtomicBool,
    state_mirror_pending: AtomicBool,
    pub(crate) state_gate: Mutex<()>,
    pub(crate) publication_gate: Mutex<()>,
}

impl Archive {
    /// Recovers an owner's existing conversation roles without opening another store.
    /// The owner serializes attachment and drains file users before closing the store.
    pub fn from_store(store: Arc<Store>, resources: AgentHistoryResources) -> Result<Self> {
        Self::from_store_controlled(store, resources, &vault_store::CancelToken::new())
    }
    /// Owner attachment keeps this token reachable while recovery is outside the slot lock.
    pub fn from_store_controlled(
        store: Arc<Store>,
        resources: AgentHistoryResources,
        cancel: &vault_store::CancelToken,
    ) -> Result<Self> {
        cancel.check().map_err(|_| Error::Cancelled)?;
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
        let jobs = Arc::new(lifetime::Jobs::new(store.clone()));
        let archive = Self {
            store,
            search_path: resources.search_path,
            search_key: Mutex::new(resources.search_key.map(Arc::new)),
            jobs,
            root,
            search_gate: Mutex::new(()),
            initial_search: Default::default(),
            reader_finalization_pending: AtomicBool::new(false),
            state_mirror_pending: AtomicBool::new(false),
            state_gate: Mutex::new(()),
            publication_gate: Mutex::new(()),
        };
        let evidence = |e| recovery_error(Db::AgentHistoryEvidence, e);
        let model = |e| recovery_error(Db::AgentHistoryReadModel, e);
        archive.recover_unfinished_revisions().map_err(evidence)?;
        archive.reclaim_slices_with_cancel(cancel).map_err(evidence)?;
        archive.recover_projection_work().map_err(model)?;
        archive.recover_attachment_counts().map_err(model)?;
        archive.recover_retired_memberships()?;
        archive
            .store
            .write(Db::AgentHistoryReadModel, vault_store::Priority::Batch, |tx| {
                tx.execute(
                    "INSERT OR IGNORE INTO ah_reader_retired(publication)
                     SELECT publication FROM ah_reader_build",
                    (),
                )?;
                tx.execute("DELETE FROM ah_reader_build", ())?;
                tx.execute(
                    "UPDATE ah_reader_version SET state='retired' WHERE state='repairing'",
                    (),
                )?;
                Ok(())
            })
            .map_err(|e| e.at(Db::AgentHistoryReadModel, vault_store::OpenStage::Recover))?;
        // A parked conversation has its own repair path; the other heads can serve immediately.
        archive.recover_reader_heads(cancel).map_err(model)?;
        if let Err(error) = archive.collect_reader_garbage(cancel) {
            cancel.check().map_err(|_| Error::Cancelled)?;
            eprintln!("Deferred conversation reader collection during attachment: {error}");
        }
        archive.recover_backup_runs().map_err(evidence)?;
        // Replay touches both databases and marks failures at the individual request.
        archive.synchronize_user_state()?;
        Ok(archive)
    }

    /// Identifies the explicitly selected archive without exposing keys or source data.
    pub fn root(&self) -> &Path {
        &self.root
    }
}

pub(crate) fn recovery_error(role: Db, error: Error) -> Error {
    let cause = match error {
        Error::Cancelled => return Error::Cancelled,
        Error::Store(cause) => cause,
        other => vault_store::Error::Operation(Box::new(other)),
    };
    cause.at(role, vault_store::OpenStage::Recover).into()
}
