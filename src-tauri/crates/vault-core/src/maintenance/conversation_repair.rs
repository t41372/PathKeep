//! Previews and installs a replacement conversation pair under the live archive key.
//! Source discovery and capture policy belong to the worker. Browsing databases and
//! the envelope are never installation members of a conversation repair.

pub(crate) mod cache;
mod schema;
mod source;
use super::{InstalledArchive, copy_databases, install, snapshot};
use crate::{Vault, content_identity, project_paths_with_root};
use agent_history::{AgentHistoryResources, Archive, capture::BackupOptions};
use anyhow::{Context, Result, ensure};
use schema::Schema;
use serde::Serialize;
use source::{Binding, CopySource};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::Arc,
};
use vault_crypto::DataKey;
use vault_layout::{ArchiveLayout, ArtifactId, CompanionUse};
use vault_store::{CancelToken, Db, Store};

const ROLES: [Db; 2] = [Db::AgentHistoryEvidence, Db::AgentHistoryReadModel];

/// The token binds the explicit choice to the copy seal and the exact live conversation
/// files inspected. A changed input requires another preview before any live rename.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversationRepairPreview {
    pub action: &'static str,
    pub copy_path: Option<String>,
    pub source_op: Option<String>,
    pub created_at: Option<String>,
    pub conversations: String,
    pub conversations_recovered_whole: Option<String>,
    pub conversations_recovered_partially: Option<String>,
    pub skipped_optional_records: String,
    pub unreadable_optional_sections: String,
    pub conversations_only_in_copy: Option<String>,
    pub kept_as: String,
    pub review_sources_before_backup: bool,
    pub next_backup_recaptures_tool_files: bool,
    pub archive_only_conversations_unreadable: bool,
    pub token: String,
}

fn files() -> impl Iterator<Item = PathBuf> {
    ROLES.into_iter().flat_map(|db| {
        vault_layout::database_spec(db).companions.suffixes(CompanionUse::Install).iter().map(
            move |suffix| PathBuf::from(format!("{}{suffix}", vault_layout::database_path(db))),
        )
    })
}

fn config(root: &Path, key: Option<&DataKey>, required: &[Db]) -> Result<vault_store::Config> {
    let mut config = crate::vault::storage_config(
        &project_paths_with_root(root),
        ArchiveLayout::AgentHistoryV2,
        key,
        None,
    )?;
    config.databases = [Db::Scratch].into_iter().chain(ROLES).collect();
    config.required_existing = required.to_vec();
    Ok(config)
}

fn service(store: Arc<Store>, root: &Path, key: Option<&DataKey>) -> Result<Archive> {
    let index = ArtifactId::AgentHistorySearch;
    Ok(Archive::from_store(
        store,
        AgentHistoryResources {
            search_path: vault_layout::path(root, index),
            search_key: key.map(|k| k.derive(vault_layout::spec(index).file_key.unwrap())),
        },
    )?)
}

fn count(store: &Store) -> Result<String> {
    let n = store.read(Db::AgentHistoryReadModel, |s| {
        s.query_one(
            "SELECT count(DISTINCT conversation_id) FROM ah_sessions WHERE conversation_current=1",
            (),
            |r| Ok(r.get::<i64>(0)?),
        )
    })?;
    Ok(n.to_string())
}

fn check_db(store: &Store, db: Db) -> Result<()> {
    ensure!(
        store.read(db, |s| s.integrity_check())? == ["ok"],
        "conversation-integrity-failed: {db:?}"
    );
    Ok(())
}

fn copy_members(from: &Path, to: &Path, db: Db) -> Result<()> {
    for suffix in vault_layout::database_spec(db).companions.suffixes(CompanionUse::Install) {
        let name = format!("{}{suffix}", vault_layout::database_path(db));
        let src = from.join(&name);
        match fs::symlink_metadata(&src) {
            Ok(meta) => ensure!(meta.is_file(), "conversation-file-unreadable: {}", src.display()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound && !suffix.is_empty() => continue,
            Err(e) => return Err(e.into()),
        }
        let dest = to.join(name);
        fs::create_dir_all(dest.parent().unwrap())?;
        fs::copy(src, dest)?;
    }
    Ok(())
}

/// A listing alone is not a proof. Only a sealed pair from one snapshot is a repair
/// source, and both copied files must match that seal in the private working folder.
fn owned(key: &DataKey) -> DataKey {
    DataKey::from_bytes(zeroize::Zeroizing::new(*key.expose_bytes()))
}

// Only role-local failures establish that the canonical evidence cannot be read.
// In particular a full staging volume or a newer schema never authorizes start-fresh.
fn evidence_damage(error: &anyhow::Error, live: &vault_store::StoreOpenFailure) -> bool {
    use std::io::ErrorKind;
    use vault_store::Error;
    error.chain().any(|cause| {
        if matches!(cause.downcast_ref::<agent_history::Error>(), Some(agent_history::Error::Integrity(_))) {
            return true;
        }
        let Some(error) = cause.downcast_ref::<Error>() else { return false };
        let error = match error {
            Error::Open(failure) if failure.role == Db::AgentHistoryEvidence => {
                if live.role == failure.role && live.rejected_migration() && failure.rejected_migration() {
                    return true;
                }
                &failure.cause
            }
            other => other,
        };
        matches!(error, Error::RetainedBytes{..} | Error::MissingStore(_) | Error::WrongKey { .. } | Error::Corrupt { .. })
            || matches!(error, Error::Io(e) if matches!(e.kind(), ErrorKind::InvalidData | ErrorKind::UnexpectedEof))
    })
}

fn evidence_store(
    root: &Path,
    key: Option<&DataKey>,
    cancel: &CancelToken,
    schema: &Schema,
) -> Result<Arc<Store>> {
    let main = root.join(vault_layout::database_path(Db::AgentHistoryEvidence));
    if let Ok(meta) = fs::symlink_metadata(&main)
        && (!meta.is_file() || meta.len() < 512)
    {
        return Err(vault_store::Error::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "invalid evidence file",
        ))
        .into());
    }
    let mut cfg = config(root, key, &[Db::AgentHistoryEvidence])?;
    cfg.databases.retain(|db| *db != Db::AgentHistoryReadModel);
    let store = Arc::new(Store::open_detailed(cfg)?);
    let checks = store.read(Db::AgentHistoryEvidence, |s| s.integrity_check())?;
    if checks != ["ok"] {
        return Err(
            agent_history::Error::Integrity("evidence integrity check failed".into()).into()
        );
    }
    schema.check(&store, Db::AgentHistoryEvidence)?;
    Archive::verify_retained_evidence(&store, cancel)?;
    Ok(store)
}

fn rebuild(
    evidence: Arc<Store>,
    dest: &Path,
    work: &Path,
    key: Option<&DataKey>,
    cancel: &CancelToken,
) -> Result<()> {
    // Copy canonical evidence before any recovery or reparse. Only the newly
    // built model travels back from the disposable parsing workspace.
    copy_databases(&evidence.pause()?, dest, key, cancel, |_| {}, &[Db::AgentHistoryEvidence])?;
    evidence.close()?;

    copy_members(dest, work, Db::AgentHistoryEvidence)?;
    let store = Arc::new(Store::open_detailed(config(work, key, &[Db::AgentHistoryEvidence])?)?);
    let archive = service(store.clone(), work, key)?;
    archive.rebuild_from_retained_evidence(&BackupOptions {
        cancel: cancel.clone(),
        read_git: false,
        ..Default::default()
    })?;
    archive.verify()?;
    archive.stop_jobs();
    copy_databases(&store.pause()?, dest, key, cancel, |_| {}, &[Db::AgentHistoryReadModel])?;
    store.close()?;
    Ok(())
}

struct Prepared {
    stage: tempfile::TempDir,
    preview: ConversationRepairPreview,
    relative_copy: PathBuf,
    originals: Vec<Option<String>>,
    source_identity: Option<Binding>,
    shared_identity: Vec<Option<String>>,
    staged_identity: Option<String>,
    held_identity: Option<String>,
    policy: String,
}

fn member_identity(path: &Path) -> Result<Option<String>> {
    Ok(content_identity::of(path)
        .map_err(|source| vault_store::Error::PreserveFile { path: path.to_owned(), source })?)
}
fn stage_binding(root: &Path) -> Result<String> {
    let members = files()
        .chain([PathBuf::from(vault_layout::spec(ArtifactId::AgentHistorySearch).path.relative())]);
    let ids = members.map(|p| member_identity(&root.join(p))).collect::<Result<Vec<_>>>()?;
    Ok(crate::utils::sha256_hex(&serde_json::to_vec(&ids)?))
}

fn identities(root: &Path) -> Result<Vec<Option<String>>> {
    files().map(|p| member_identity(&root.join(p))).collect()
}

fn shared_identity(vault: &Vault) -> Result<Vec<Option<String>>> {
    [ArtifactId::Config, ArtifactId::Envelope]
        .into_iter()
        .map(|id| member_identity(&vault.paths().artifact_path(id)))
        .collect()
}

fn prepare(
    vault: &Vault,
    source: Option<&Path>,
    password: Option<&str>,
    cancel: &CancelToken,
) -> Result<Prepared> {
    let control = crate::admission::require_control(&vault.paths().app_root)?
        .context("archive-control-missing: live archive has no control")?;
    ensure!(
        crate::admission::layout_of(Some(&control)) == vault.layout(),
        "archive-layout-changed"
    );
    let shared = shared_identity(vault)?;
    vault.store().preserve_observed_displacements()?;
    let failure = vault.conversation_availability().err().context("conversation-repair-healthy")?;
    if failure.cause.access_refused() {
        return Err(vault_store::Error::AccessUnavailable(failure).into());
    }
    ensure!(ROLES.contains(&failure.role), "conversation-repair-requires-isolated-damage");
    ensure!(
        !matches!(failure.cause, vault_store::Error::SchemaTooNew { .. }),
        "conversation-schema-too-new"
    );
    vault.check_browsing_files()?;
    let stage = cache::stage(vault.paths())?;
    let scratch = cache::stage(vault.paths())?;
    let original = scratch.path().join("original");
    fs::create_dir_all(&original)?;
    {
        let _producers = vault.pause_conversations();
        vault.store().copy_quarantined_conversations(&original)?;
    }
    let inputs = identities(&original)?;
    // A retired handle can still hold the only readable generation of canonical
    // evidence. Work on its private copy and keep both generations in the safety copy.
    let held = original.join(vault_layout::HELD_CONVERSATIONS);
    let held_identity = member_identity(&held)?;
    let evidence_input = if vault.store().current_held_role(Db::AgentHistoryEvidence)
        && held.join(vault_layout::database_path(Db::AgentHistoryEvidence)).is_file()
    {
        &held
    } else {
        &original
    };
    let key = vault.data_key();
    crate::fault_inject::checkpoint("conversation-repair-preparing")?;
    let reference = Store::open_detailed(config(&scratch.path().join("schema"), None, &[])?)?;
    let evidence_schema = Schema::read(&reference, Db::AgentHistoryEvidence)?;
    let model_schema = Schema::read(&reference, Db::AgentHistoryReadModel)?;
    reference.close()?;
    // A known digest failure can be confirmed by its own manifest. Do not
    // stream unrelated good revisions here and then stream them again in salvage.
    let known_bad = if let vault_store::Error::RetainedBytes { revision, .. } = failure.cause {
        Archive::verify_recorded_revision(&config(evidence_input, key, &ROLES)?, revision)
            .is_err_and(|e| agent_history::archive::salvage::unreadable_input(&e))
    } else {
        false
    };
    let evidence = if known_bad {
        None
    } else {
        match evidence_store(evidence_input, key, cancel, &evidence_schema) {
            Ok(store) => Some(store),
            Err(error) if evidence_damage(&error, &failure) => None,
            Err(error) => return Err(error),
        }
    };
    // A readable replacement evidence file is not necessarily the local one.
    // Never rebuild from it after a failed pair check or evidence-role refusal.
    let trusted_evidence = (failure.role == Db::AgentHistoryReadModel || evidence_input == &held)
        && !matches!(failure.cause, vault_store::Error::PairUnverified);
    cancel.check()?;
    let mut from_copy = None;
    let mut source_identity = None;
    let mut digest = None;
    let mut source_config = config(evidence_input, key, &ROLES)?;
    let model_root = if vault.store().current_held_role(Db::AgentHistoryReadModel)
        && held.join(vault_layout::database_path(Db::AgentHistoryReadModel)).is_file()
    {
        &held
    } else {
        &original
    };
    source_config.paths.agent_history_read_model =
        model_root.join(vault_layout::database_path(Db::AgentHistoryReadModel));
    let trust_model = !vault.store().pair_was_rejected();
    let mut diagnostics = agent_history::archive::salvage::Diagnostics::default();
    let action;
    if let Some(source) = source {
        ensure!(
            !(trusted_evidence && evidence.is_some()),
            "conversation-evidence-readable: rebuild the read model"
        );
        let work = scratch.path().join("incoming");
        let selected = CopySource::open(vault, source, password)?;
        let copy_key = selected.key.as_ref();
        if selected.info.conversations_unreadable {
            let mut verified = None;
            for (i, prefix) in selected.evidence_prefixes().into_iter().enumerate() {
                let input = work.join(i.to_string());
                let Some(binding) = selected.copy(&[Db::AgentHistoryEvidence], &prefix, &input)?
                else {
                    continue;
                };
                match evidence_store(&input, copy_key, cancel, &evidence_schema) {
                    Ok(store) => {
                        verified = Some(store);
                        source_identity = Some(binding);
                        break;
                    }
                    Err(e) if evidence_damage(&e, &failure) => continue,
                    Err(e) => return Err(e),
                }
            }
            let ev = verified.context("conversation-copy-no-verified-evidence")?;
            rebuild(ev, stage.path(), &scratch.path().join("verified-evidence"), key, cancel)?;
        } else {
            source_identity = Some(
                selected
                    .copy(&ROLES, Path::new(""), &work)?
                    .context("conversation-copy-incomplete")?,
            );
            let cfg = config(&work, copy_key, &ROLES)?;
            if Store::verify_copy_pair(&cfg)? {
                let store = Arc::new(Store::open_detailed(cfg)?);
                evidence_schema.check(&store, Db::AgentHistoryEvidence)?;
                model_schema.check(&store, Db::AgentHistoryReadModel)?;
                let archive = service(store.clone(), &work, copy_key)?;
                archive.verify()?;
                archive.stop_jobs();
                copy_databases(&store.pause()?, stage.path(), key, cancel, |_| {}, &ROLES)?;
                store.close()?;
            } else {
                let ev = evidence_store(&work, copy_key, cancel, &evidence_schema)?;
                rebuild(ev, stage.path(), &scratch.path().join("legacy-rebuild"), key, cancel)?;
            }
        }
        from_copy = Some(selected.info);
        digest = Some(selected.digest);
        action = "fromCopy";
    } else if let Some(evidence) = evidence {
        ensure!(
            trusted_evidence,
            "conversation-pair-unverified: retry the matching pair or select a sealed copy"
        );
        rebuild(evidence, stage.path(), &scratch.path().join("rebuild"), key, cancel)?;
        action = "rebuild";
    } else {
        crate::fault_inject::checkpoint("conversation-salvage-start")?;
        let target = Arc::new(Store::open_detailed(config(stage.path(), key, &[])?)?);
        let archive = service(target.clone(), stage.path(), key)?;

        let ev = evidence_input.join(vault_layout::database_path(Db::AgentHistoryEvidence));
        let readable_header = match fs::symlink_metadata(&ev) {
            Ok(meta) => meta.is_file() && meta.len() >= 512,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => false,
            Err(source) => return Err(vault_store::Error::PreserveFile { path: ev, source }.into()),
        };
        if readable_header {
            // An inaccessible catalog may leave zero identifiable revisions. Errors
            // writing or verifying the new generation always stop publication.
            // Probe the source by itself. A database with no readable catalog cannot
            // identify any revision. Once writing starts, target errors propagate.
            let catalog = Store::inspect_existing(&source_config, Db::AgentHistoryEvidence, |s| {
                s.query_one("SELECT count(*) FROM sqlite_schema", (), |r| Ok(r.get::<i64>(0)?))
            });
            match catalog {
                Ok(_) => archive.salvage_evidence(&source_config, cancel, &mut diagnostics)?,
                Err(e) if agent_history::archive::salvage::unreadable_input(&e) => {}
                Err(e) => return Err(e.into()),
            }
        }
        archive.rebuild_salvaged_model(&BackupOptions {
            cancel: cancel.clone(),
            read_git: false,
            ..Default::default()
        })?;
        archive.recover_salvaged_marks(&source_config, trust_model, cancel, &mut diagnostics)?;
        archive.stop_jobs();
        target.close()?;
        crate::fault_inject::checkpoint("conversation-salvage-finished")?;
        action = "startFresh";
    }
    cancel.check()?;
    let required = if action == "startFresh" { &[][..] } else { &ROLES[..] };
    let store = Arc::new(Store::open_detailed(config(stage.path(), key, required)?)?);
    for db in ROLES {
        check_db(&store, db)?;
    }
    evidence_schema.check(&store, Db::AgentHistoryEvidence)?;
    model_schema.check(&store, Db::AgentHistoryReadModel)?;
    let conversations = count(&store)?;
    let counts = if action == "startFresh" {
        agent_history::archive::salvage::loss::compare(
            &source_config,
            &store,
            trust_model,
            cancel,
            &mut diagnostics,
        )?
    } else {
        Default::default()
    };
    let conversations_only_in_copy = counts.only_in_copy.map(|n| n.to_string());
    if conversations != "0" {
        let archive = service(store.clone(), stage.path(), key)?;
        archive.rebuild_search()?;
        archive.stop_jobs();
    }
    store.pause()?.close()?;
    let config = crate::load_config(vault.paths())?;
    let policy = config.capture_policy.token();
    let decision = serde_json::to_vec(&(
        &vault.paths().app_root,
        &config.archive_mode,
        action,
        stage.path().file_name(),
        &inputs,
        source,
        &digest,
        &policy,
        &conversations,
    ))?;
    let copies = Path::new(vault_layout::spec(ArtifactId::NativeSnapshots).path.relative());
    let mut sequence = 0u64;
    let (token, relative_copy) = loop {
        let token = crate::utils::sha256_hex(&serde_json::to_vec(&(&decision, sequence))?);
        let relative = copies.join(format!("before-agent-repair-{token}"));
        if !install::exists(&vault.paths().app_root.join(&relative))? {
            break (token, relative);
        }
        sequence += 1;
    };
    let preview = ConversationRepairPreview {
        action,
        copy_path: from_copy.as_ref().map(|c| c.path.clone()),
        source_op: from_copy.as_ref().map(|c| c.source_op.clone()),
        created_at: from_copy.and_then(|c| c.created_at),
        conversations,
        conversations_recovered_whole: counts.whole.map(|n| n.to_string()),
        conversations_recovered_partially: counts.partial.map(|n| n.to_string()),
        skipped_optional_records: diagnostics.skipped_records.to_string(),
        unreadable_optional_sections: diagnostics.unreadable_sections.to_string(),
        conversations_only_in_copy: conversations_only_in_copy.clone(),
        kept_as: vault.paths().app_root.join(&relative_copy).display().to_string(),
        review_sources_before_backup: action != "rebuild",
        next_backup_recaptures_tool_files: action == "startFresh",
        archive_only_conversations_unreadable: action == "startFresh"
            && (counts.partial != Some(0) || counts.only_in_copy != Some(0)),
        token,
    };
    let staged_identity = Some(stage_binding(stage.path())?);
    Ok(Prepared {
        stage,
        preview,
        relative_copy,
        originals: inputs,
        policy,
        source_identity,
        staged_identity,
        shared_identity: shared,
        held_identity,
    })
}

/// Opens and verifies private copies. A preview never installs its trial generation.
pub fn preview_conversation_repair(
    vault: &Vault,
    source: Option<&Path>,
    password: Option<&str>,
) -> Result<ConversationRepairPreview> {
    preview_conversation_repair_with_cancel(vault, source, password, &CancelToken::new())
}

pub fn preview_conversation_repair_with_cancel(
    vault: &Vault,
    source: Option<&Path>,
    password: Option<&str>,
    cancel: &CancelToken,
) -> Result<ConversationRepairPreview> {
    let generation = vault.repair_preview.generation();
    let prepared = prepare(vault, source, password, cancel)?;
    cancel.check()?;
    let preview = prepared.preview.clone();
    vault.repair_preview.publish(generation, prepared)?;
    Ok(preview)
}

impl Vault {
    pub fn cancel_conversation_repair_preview(&self) -> bool {
        self.repair_preview.clear()
    }
}

/// Rechecks the reviewed inputs and atomically publishes the new pair and sealed displaced copy
/// under the existing install decision. Recovery keeps both or rolls both back.
pub fn repair_conversations(
    vault: &Vault,
    source: Option<&Path>,
    _password: Option<&str>,
    token: &str,
    cancel: CancelToken,
) -> Result<InstalledArchive> {
    let reviewed = verify_conversation_repair(vault, source, token)?;
    install_conversation_repair(vault, reviewed, cancel)
}

/// Consuming the cache gives verification exclusive ownership of the private trial.
/// No source database is consulted after this check.
pub struct VerifiedConversationRepair {
    plan: Prepared,
    owner: Arc<Store>,
}

pub fn verify_conversation_repair(
    vault: &Vault,
    source: Option<&Path>,
    token: &str,
) -> Result<VerifiedConversationRepair> {
    let plan = vault.repair_preview.take(token, source)?;
    vault.recheck_root()?;
    ensure!(
        match (&plan.source_identity, source) {
            (Some(binding), Some(path)) => binding.verify(vault, path)?,
            (None, None) => true,
            _ => false,
        } && Some(stage_binding(plan.stage.path())?) == plan.staged_identity,
        "conversation-repair-preview-stale"
    );
    crate::fault_inject::checkpoint("conversation-repair-verified")?;
    Ok(VerifiedConversationRepair { plan, owner: vault.store().clone() })
}

pub fn install_conversation_repair(
    vault: &Vault,
    verified: VerifiedConversationRepair,
    cancel: CancelToken,
) -> Result<InstalledArchive> {
    let _op = crate::archive::ArchiveOpGate::acquire(vault.paths());
    ensure!(Arc::ptr_eq(vault.store(), &verified.owner), "conversation-repair-preview-stale");
    let Prepared {
        stage,
        preview,
        relative_copy,
        originals,
        policy,
        shared_identity: shared,
        held_identity,
        ..
    } = verified.plan;
    vault.recheck_root()?;
    ensure!(shared_identity(vault)? == shared, "conversation-repair-preview-stale");
    cancel.check()?;
    let paths = vault.paths();
    let owner = vault.store().owner()?;
    let key = vault.data_key().map(owned);
    let control = crate::ControlGate::acquire(paths);
    let original_control = crate::content_identity::of(&paths.config_path)?;
    let mut settings = crate::load_config(paths)?;
    crate::ensure_app_lock_unlocked(paths, &settings)?;
    ensure!(settings.capture_policy.token() == policy, "conversation-repair-preview-stale");
    if preview.action != "rebuild" {
        // A different evidence ledger cannot authorize this device's old receipt
        // or selection. Require an explicit enable before reading any tool folder.
        settings.capture_policy.selection_ref = None;
        settings.capture_policy.pending_first_save = None;
        // Preserve the user’s chosen kinds, including a device whose only active kind
        // is Agent history. With no selection reference, capture requires a new preview.
    }
    crate::config::save_copy_config(paths, stage.path(), &settings)?;
    let mut pause = vault.pause_store()?;
    ensure!(vault.conversation_availability().is_err(), "conversation-repair-healthy");
    let mut members = files().collect::<Vec<_>>();
    members.extend([
        PathBuf::from(vault_layout::spec(ArtifactId::Config).path.relative()),
        PathBuf::from(vault_layout::spec(ArtifactId::AgentHistorySearch).path.relative()),
        relative_copy.clone(),
    ]);
    let mut expected = members
        .iter()
        .filter(|p| !files().any(|role| role == **p))
        .map(|p| Ok((p.clone(), member_identity(&paths.app_root.join(p))?)))
        .collect::<Result<install::Originals>>()?;
    expected.insert(PathBuf::from("config.json"), original_control);
    for (member, identity) in files().zip(originals.iter().cloned()) {
        expected.insert(member, identity);
    }
    expected.insert(relative_copy.clone(), None);
    let displaced = stage.path().join(&relative_copy);
    snapshot::snapshot_paused(vault, &mut pause, &displaced)?;
    ensure!(
        identities(&displaced)? == originals
            && member_identity(&displaced.join(vault_layout::HELD_CONVERSATIONS))? == held_identity,
        "conversation-repair-preview-stale"
    );
    cancel.check()?;
    if snapshot::keep_held_before_close(&displaced, &paths.app_root.join(&relative_copy))? {
        members.retain(|path| path != &relative_copy);
        expected.remove(&relative_copy);
    }
    crate::fault_inject::checkpoint("conversation-repair-safety-sealed")?;
    pause.close()?;
    ensure!(identities(&paths.app_root)? == originals, "conversation-repair-preview-stale");
    crate::fault_inject::checkpoint("conversation-repair-closed")?;
    // Once a journal can name the stage its recovery owns cleanup, including after
    // a failed publication fsync whose durable decision is still unknown.
    let stage = stage.keep();
    if let Err(e) =
        install::install_bound(&control, &stage, members, install::Keep::Nothing, Some(&expected))
    {
        if !install::pending(paths) && stage.exists() {
            install::remove(&stage)?;
        }
        return Err(e);
    }
    crate::fault_inject::checkpoint("conversation-repair-installed")?;
    drop(control);
    let vault = Vault::open_owned(paths.clone(), settings, key.as_ref(), Some(owner))?;
    vault.conversation_availability().map_err(vault_store::Error::Open)?;
    Ok(InstalledArchive {
        vault,
        data_key: key,
        previous_files: paths.app_root.join(relative_copy),
        previous_unsealed: None,
    })
}
