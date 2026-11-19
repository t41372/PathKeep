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
    recovery_failure: Option<Arc<vault_store::StoreOpenFailure>>,
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
    /// Installed conversation storage for the backup header, including native
    /// WAL files and the owned search index. Never follow links into another tree.
    pub fn conversation_archive_bytes(&self) -> anyhow::Result<u64> {
        self.recheck_root()?;
        let mut bytes = 0;
        for role in [ArtifactId::AgentHistoryEvidence, ArtifactId::AgentHistoryReadModel] {
            let path = self.paths.artifact_path(role);
            for suffix in
                vault_layout::CompanionFamily::Native.suffixes(vault_layout::CompanionUse::Usage)
            {
                let mut member = path.as_os_str().to_owned();
                member.push(suffix);
                match std::fs::symlink_metadata(member) {
                    Ok(meta) if meta.is_file() => bytes += meta.len(),
                    Ok(_) => {}
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                    Err(e) => return Err(e.into()),
                }
            }
        }
        let path = self.paths.artifact_path(ArtifactId::AgentHistorySearch);
        for entry in walkdir::WalkDir::new(path).follow_links(false).follow_root_links(false) {
            match entry {
                Ok(entry) if entry.file_type().is_file() => match entry.metadata() {
                    Ok(meta) => bytes += meta.len(),
                    Err(e)
                        if e.io_error()
                            .is_some_and(|e| e.kind() == std::io::ErrorKind::NotFound) => {}
                    Err(e) => return Err(e.into()),
                },
                Ok(_) => {}
                Err(e)
                    if e.io_error().is_some_and(|e| e.kind() == std::io::ErrorKind::NotFound) => {}
                Err(e) => return Err(e.into()),
            }
        }
        Ok(bytes)
    }

    /// The saved failure is shared by every handle to this generation.
    pub fn conversation_availability(&self) -> Result<(), Arc<vault_store::StoreOpenFailure>> {
        self.store.conversation_availability()?;
        self.lifetime.conversations.0.lock().unwrap().recovery_failure.clone().map_or(Ok(()), Err)
    }

    /// Data kinds this generation currently admits. Shared maintenance needs all of them.
    pub fn available_kinds(&self) -> Vec<vault_layout::DataScope> {
        use vault_layout::DataScope;
        if !self.store.is_open() {
            return Vec::new();
        }
        let mut kinds = vec![DataScope::Browsing, DataScope::Assistant];
        if self.layout != vault_layout::ArchiveLayout::BrowsingV1
            && self.conversation_availability().is_ok()
        {
            kinds.push(DataScope::AgentHistory);
        }
        kinds
    }

    /// After an external fix, retry both conversation workers in this generation.
    pub fn retry_conversations(&self) -> vault_store::Result<()> {
        let mut slot = self.lifetime.conversations.0.lock().unwrap();
        if slot.closed || !self.store.is_open() {
            return Err(vault_store::Error::Closed);
        }
        if slot.pauses > 0 {
            return Err(vault_store::Error::cancelled());
        }
        if slot.recovery_failure.is_some() {
            return self.attach_conversations(&mut slot).map(|_| ()).map_err(|e| match e {
                agent_history::Error::Store(error) => error,
                other => vault_store::Error::Operation(Box::new(other)),
            });
        }
        if self.store.conversation_availability().is_ok() {
            return Ok(());
        }
        let layout = crate::admission::root_layout(&self.paths.app_root)
            .map_err(|e| vault_store::Error::Operation(Box::new(e)))?;
        if layout != self.layout {
            return Err(vault_store::Error::InvalidConfig("archive layout changed".into()));
        }
        super::password::check_partial_root(&self.paths, self.data_key().is_some())?;
        let mut config =
            super::storage_config(&self.paths, layout, self.data_key(), Some(self.store.owner()?))?;
        config.required_existing =
            crate::maintenance::creation::check_required_stores(&self.paths, layout)
                .map_err(|e| vault_store::Error::Operation(e.into_boxed_dyn_error()))?;
        let _update = super::upgrade::UpdateScope::track(&mut config.migrations);
        self.store.retry_conversations(config)
    }

    /// Whole-archive operations must be able to account for every original database.
    pub fn require_complete(&self) -> vault_store::Result<()> {
        self.conversation_availability().map_err(vault_store::Error::Open)?;
        self.store.check_database_files(vault_layout::at_rest_databases(self.layout))
    }

    /// Returns this generation's conversation service. First use runs recovery on the
    /// calling thread. A failed recovery can be retried independently of browsing.
    pub fn agent_history(&self) -> agent_history::Result<Arc<Archive>> {
        let mut slot = self.lifetime.conversations.0.lock().unwrap();
        if slot.closed || !self.store.is_open() {
            return Err(vault_store::Error::Closed.into());
        }
        self.store.conversation_availability().map_err(vault_store::Error::Open)?;
        if slot.pauses > 0 {
            return Err(agent_history::Error::Cancelled);
        }
        if let Some(archive) = &slot.archive {
            return Ok(archive.clone());
        }
        self.attach_conversations(&mut slot)
    }

    fn attach_conversations(&self, slot: &mut Slot) -> agent_history::Result<Arc<Archive>> {
        let index = ArtifactId::AgentHistorySearch;
        let resources = AgentHistoryResources {
            search_path: self.paths.artifact_path(index),
            search_key: self
                .data_key()
                .map(|key| key.derive(vault_layout::spec(index).file_key.expect("search purpose"))),
        };
        let archive = match Archive::from_store(self.store.clone(), resources) {
            Ok(archive) => Arc::new(archive),
            Err(agent_history::Error::Store(vault_store::Error::Open(reason))) => {
                slot.recovery_failure = Some(reason.clone());
                return Err(vault_store::Error::Open(reason).into());
            }
            Err(other) => return Err(other),
        };
        slot.recovery_failure = None;
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
        self.conversation_availability().map_err(vault_store::Error::Open)?;
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
