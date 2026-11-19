//! Owns lazy conversation attachment for one open generation, shared by vault clones.
//! The library recovers its roles. Session policy and filesystem replacement belong to the owner.

use super::Vault;
use agent_history::{AgentHistoryResources, Archive, archive::ArchivePause};
use std::sync::{Arc, Condvar, Mutex};
use vault_layout::ArtifactId;
use vault_store::CancelToken;

#[derive(Default)]
pub(super) struct Conversations(Mutex<Slot>, Condvar);

#[derive(Default)]
struct Slot {
    archive: Option<Arc<Archive>>,
    attaching: Option<CancelToken>,
    recovery_failure: Option<Arc<vault_store::StoreOpenFailure>>,
    closed: bool,
    pauses: usize,
    prepare_visible_search: bool,
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
    /// Reads use archive evidence for an undecided device. This lookup never
    /// discovers sources and never changes the durable capture policy.
    pub fn agent_visibility(
        &self,
        policy: &crate::CapturePolicy,
    ) -> anyhow::Result<crate::KindState> {
        if policy.agent_state_if_present(true) == policy.agent_history {
            return Ok(policy.agent_history);
        }
        Ok(policy.agent_state_if_present(self.has_saved_transcripts()?))
    }
    /// Raw transcripts remain valuable when a newer native format cannot yet be parsed.
    pub fn has_saved_transcripts(&self) -> vault_store::Result<bool> {
        self.store.read(vault_store::Db::AgentHistoryEvidence,|tx|tx.query_one(
            "SELECT EXISTS(SELECT 1 FROM ah_sources WHERE kind='transcript' AND latest_revision_id IS NOT NULL)",(),|r|Ok(r.get::<i64>(0)?!=0)))
    }
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
        self.store.preserve_observed_displacements()?;
        if self.conversation_availability().is_ok() {
            return if self.store.is_open() { Ok(()) } else { Err(vault_store::Error::Closed) };
        }
        let _pause = self.pause_conversations();
        let mut slot = self.lifetime.conversations.0.lock().unwrap();
        if slot.closed || !self.store.is_open() {
            return Err(vault_store::Error::Closed);
        }
        if slot.pauses != 1 {
            return Err(vault_store::Error::cancelled());
        }
        let layout = crate::admission::root_layout(&self.paths.app_root)
            .map_err(|e| vault_store::Error::Operation(Box::new(e)))?;
        if layout != self.layout {
            return Err(vault_store::Error::InvalidConfig("archive layout changed".into()));
        }
        super::password::check_partial_root(
            &self.paths,
            self.data_key().is_some(),
            ArtifactId::Envelope,
        )?;
        if self.store.retry_access()? {
            if slot.archive.is_some() && slot.recovery_failure.is_none() {
                return Ok(());
            }
        } else {
            if let Some(archive) = slot.archive.take() {
                archive.stop_jobs();
            }
            let mut config = super::storage_config(
                &self.paths,
                layout,
                self.data_key(),
                Some(self.store.owner()?),
            )?;
            config.required_existing =
                crate::maintenance::creation::check_required_stores(&self.paths, layout)
                    .map_err(|e| vault_store::Error::Operation(e.into_boxed_dyn_error()))?;
            let _update = super::upgrade::UpdateScope::track(&mut config.migrations);
            let prior = self.store.conversation_availability().err();
            self.store.retry_conversations_checked(config, |config| {
                if let Some(reason) = prior
                    && let vault_store::Error::RetainedBytes { revision, .. } = reason.cause
                {
                    agent_history::Archive::verify_recorded_revision(config, revision)?;
                }
                Ok(())
            })?;
        }
        // Reattach through the cancellable owner slot after the store replacement.
        drop(slot);
        drop(_pause);
        self.agent_history().map(|_| ()).map_err(|e| match e {
            agent_history::Error::Store(error) => error,
            other => vault_store::Error::Operation(Box::new(other)),
        })
    }

    /// Whole-archive operations must be able to account for every original database.
    pub fn require_complete(&self) -> vault_store::Result<()> {
        self.check_available_files()?;
        self.conversation_availability().map_err(vault_store::Error::unavailable)
    }

    /// Verify source pages and logical evidence before a conversion reads them.
    pub(crate) fn verify_conversion_source(
        &self,
        cancel: &vault_store::CancelToken,
    ) -> anyhow::Result<()> {
        if self.layout != vault_layout::ArchiveLayout::AgentHistoryV2 {
            return Ok(());
        }
        let result = (|| -> anyhow::Result<()> {
            for role in
                [vault_store::Db::AgentHistoryEvidence, vault_store::Db::AgentHistoryReadModel]
            {
                let messages = self.store.read(role, |s| s.integrity_check())?;
                if messages != ["ok"] {
                    self.store.remember_conversation_failure(Arc::new(
                        vault_store::StoreOpenFailure {
                            role,
                            path: self.store.database_path(role).to_owned(),
                            stage: vault_store::OpenStage::Read,
                            cause: vault_store::Error::IntegrityCheck {
                                detail: messages.join(", "),
                            },
                        },
                    ));
                    break;
                }
            }
            agent_history::Archive::verify_retained_evidence(&self.store, cancel)?;
            Ok(())
        })();
        self.store.conversation_availability().map_err(vault_store::Error::unavailable)?;
        result
    }

    /// Validate the browsing files first. If a conversion or at-rest check is the
    /// first to discover a displaced conversation role, publish its failure immediately.
    pub fn check_available_files(&self) -> vault_store::Result<()> {
        self.check_browsing_files()?;
        if self.conversation_availability().is_ok() {
            let roles = vault_layout::at_rest_databases(self.layout()).filter(|db| {
                vault_layout::spec(vault_layout::db_artifact(*db)).scope
                    == vault_layout::DataScope::AgentHistory
            });
            match self.store().check_database_files(roles) {
                Ok(()) => {}
                Err(vault_store::Error::Open(reason)) => {
                    let check_held = !reason.cause.access_refused();
                    self.store().remember_conversation_failure(reason);
                    if check_held {
                        self.store().preserve_displaced_conversations()?;
                    }
                }
                Err(other) => return Err(other),
            }
        }
        Ok(())
    }

    /// A browsing-only operation verifies the historical files even during conversation recovery.
    pub fn check_browsing_files(&self) -> vault_store::Result<()> {
        self.store.check_database_files(vault_layout::at_rest_databases(self.layout).filter(|db| {
            vault_layout::spec(vault_layout::db_artifact(*db)).scope
                != vault_layout::DataScope::AgentHistory
        }))
    }

    /// Returns this generation's conversation service. First use runs recovery on the
    /// calling thread. A failed recovery can be retried independently of browsing.
    pub fn agent_history(&self) -> agent_history::Result<Arc<Archive>> {
        let owner = &self.lifetime.conversations;
        let mut slot = owner.0.lock().unwrap();
        loop {
            if slot.closed || !self.store.is_open() {
                return Err(vault_store::Error::Closed.into());
            }
            if slot.pauses > 0 {
                return Err(agent_history::Error::Cancelled);
            }
            self.store.conversation_availability().map_err(vault_store::Error::Open)?;
            if let Some(archive) = slot.archive.clone() {
                if slot.prepare_visible_search {
                    archive.prepare_visible_search()?;
                    slot.prepare_visible_search = false;
                }
                return Ok(archive);
            }
            if slot.attaching.is_some() {
                slot = owner.1.wait(slot).unwrap();
                continue;
            }
            let cancel = CancelToken::new();
            slot.attaching = Some(cancel.clone());
            drop(slot);
            let result = self.attach_conversations(&cancel);
            slot = owner.0.lock().unwrap();
            slot.attaching = None;
            owner.1.notify_all();
            if slot.closed || slot.pauses > 0 || cancel.check().is_err() {
                return Err(agent_history::Error::Cancelled);
            }
            match result {
                Ok(archive) => {
                    slot.recovery_failure = None;
                    slot.archive = Some(archive);
                }
                Err(agent_history::Error::Store(vault_store::Error::Open(reason))) => {
                    self.store.remember_conversation_failure(reason.clone());
                    slot.recovery_failure = Some(reason.clone());
                    return Err(vault_store::Error::Open(reason).into());
                }
                Err(error) => return Err(error),
            }
        }
    }

    /// Register preparation on this generation without attaching a service just
    /// to paint the shell. If already attached, only the search reader waits.
    pub fn set_view_search_preparation(&self, visible: bool) -> agent_history::Result<()> {
        let mut slot = self.lifetime.conversations.0.lock().unwrap();
        if let Some(archive) = &slot.archive {
            if visible {
                archive.prepare_visible_search()?;
            } else {
                archive.cancel_visible_search();
            }
            slot.prepare_visible_search = false;
        } else {
            slot.prepare_visible_search = visible;
        }
        Ok(())
    }

    fn attach_conversations(&self, cancel: &CancelToken) -> agent_history::Result<Arc<Archive>> {
        let index = ArtifactId::AgentHistorySearch;
        let resources = AgentHistoryResources {
            search_path: self.paths.artifact_path(index),
            search_key: self
                .data_key()
                .map(|key| key.derive(vault_layout::spec(index).file_key.expect("search purpose"))),
        };
        let archive =
            Arc::new(Archive::from_store_controlled(self.store.clone(), resources, cancel)?);
        archive.enable_idle_reader_collection();
        Ok(archive)
    }

    /// Take this before a store maintenance guard. Active jobs can still submit SQL
    /// while they acknowledge cancellation. A pause never attaches an unused service.
    pub fn pause_conversations(&self) -> ConversationPause {
        let owner = self.lifetime.conversations.clone();
        let archive = {
            let mut slot = owner.0.lock().unwrap();
            slot.pauses += 1;
            if let Some(token) = &slot.attaching {
                token.cancel();
            }
            owner.1.notify_all();
            let slot = owner.1.wait_while(slot, |s| s.attaching.is_some()).unwrap();
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
            if let Some(token) = &slot.attaching {
                token.cancel();
            }
            self.1.notify_all();
            let slot = self.1.wait_while(slot, |s| s.attaching.is_some()).unwrap();
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

impl std::ops::DerefMut for PausedStore<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.storage
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
