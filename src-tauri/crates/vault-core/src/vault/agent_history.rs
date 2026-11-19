//! Owns lazy conversation attachment for one open generation, shared by vault clones.
//! The library recovers its roles. Session policy and filesystem replacement belong to the owner.

use super::Vault;
use agent_history::{AgentHistoryResources, Archive, archive::ArchivePause};
use std::sync::{Arc, Mutex};
use vault_layout::ArtifactId;

#[derive(Default)]
pub(super) struct Conversations(Mutex<Slot>);

#[derive(Default)]
struct Slot {
    archive: Option<Arc<Archive>>,
    closed: bool,
    pauses: usize,
}

/// Holds off first attachment as well as work on an already attached service.
/// A cancelled maintenance operation releases this hold on its original generation.
pub struct ConversationPause {
    owner: Arc<Conversations>,
    jobs: Option<ArchivePause>,
}

impl Drop for ConversationPause {
    fn drop(&mut self) {
        let mut slot = self.owner.0.lock().unwrap();
        self.jobs.take();
        slot.pauses -= 1;
    }
}

impl Vault {
    /// Returns this generation's conversation service. First use runs recovery on the
    /// calling thread. A failed recovery can be retried independently of browsing.
    pub fn agent_history(&self) -> agent_history::Result<Arc<Archive>> {
        let mut slot = self.lifetime.conversations.0.lock().unwrap();
        if slot.closed || !self.store.is_open() {
            return Err(vault_store::Error::Closed.into());
        }
        if slot.pauses > 0 {
            return Err(agent_history::Error::Cancelled);
        }
        if let Some(archive) = &slot.archive {
            return Ok(archive.clone());
        }
        let index = ArtifactId::AgentHistorySearch;
        let resources = AgentHistoryResources {
            search_path: self.paths.artifact_path(index),
            search_key: self
                .data_key()
                .map(|key| key.derive(vault_layout::spec(index).file_key.expect("search purpose"))),
        };
        let archive = Arc::new(Archive::from_store(self.store.clone(), resources)?);
        slot.archive = Some(archive.clone());
        Ok(archive)
    }

    /// Take this before a store maintenance guard. Active jobs can still submit SQL
    /// while they acknowledge cancellation. A pause never attaches an unused service.
    pub fn pause_conversations(&self) -> ConversationPause {
        let owner = self.lifetime.conversations.clone();
        let archive = {
            let mut slot = owner.0.lock().unwrap();
            slot.pauses += 1;
            slot.archive.clone()
        };
        let jobs = archive.map(|archive| archive.pause_jobs());
        ConversationPause { owner, jobs }
    }
}

impl Conversations {
    pub(super) fn close(&self) {
        let archive = {
            let mut slot = self.0.lock().unwrap();
            slot.closed = true;
            slot.archive.clone()
        };
        if let Some(archive) = archive {
            archive.stop_jobs();
        }
    }
}

/// Couples the conversation producer barrier to the store freeze. Drop order matters:
/// a resumed producer must be able to submit its next database request.
pub(crate) struct PausedStore<'a> {
    storage: vault_store::Maintenance<'a>,
    vault: &'a Vault,
    _conversations: ConversationPause,
}

impl Vault {
    pub(crate) fn pause_store(&self) -> vault_store::Result<PausedStore<'_>> {
        let conversations = self.pause_conversations();
        Ok(PausedStore { storage: self.store.pause()?, vault: self, _conversations: conversations })
    }
}

impl<'a> std::ops::Deref for PausedStore<'a> {
    type Target = vault_store::Maintenance<'a>;
    fn deref(&self) -> &Self::Target {
        &self.storage
    }
}

impl PausedStore<'_> {
    /// A checkpoint failure leaves the old generation usable. After it succeeds no
    /// new work can reach this generation between key release and store shutdown.
    pub(crate) fn close(self) -> vault_store::Result<()> {
        self.storage.close_with(|| self.vault.lifetime.conversations.close())?;
        self.vault.close()
    }
}
