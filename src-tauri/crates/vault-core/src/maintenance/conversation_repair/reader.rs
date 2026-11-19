//! Prepare reader-ticket repairs on a frozen copy of the admitted pair. The live
//! store continues serving during reconstruction; the common installer owns displacement.
use super::*;

pub(super) fn prepare(
    vault: &Vault,
    source: Option<&Path>,
    shared: Vec<Option<String>>,
    cancel: &CancelToken,
) -> Result<Prepared> {
    ensure!(
        vault.layout() == ArchiveLayout::AgentHistoryV2
            && Archive::has_reader_repair_work(vault.store())?,
        "conversation-repair-healthy"
    );
    ensure!(source.is_none(), "conversation-evidence-readable: rebuild the read model");
    vault.check_available_files()?;
    let stage = cache::stage(vault.paths())?;
    vault.store().copy_admitted_conversations(stage.path())?;
    crate::fault_inject::checkpoint("conversation-repair-preparing")?;
    let inputs = identities(stage.path())?;
    let key = vault.data_key();
    let store = Arc::new(Store::open_detailed(config(stage.path(), key, &ROLES)?)?);
    ensure!(Archive::has_reader_repair_work(&store)?, "conversation-repair-healthy");
    let count = Archive::rebuild_pending_readers(
        store.clone(),
        resources(stage.path(), key),
        &agent_history::read_model::JobControl { cancel: cancel.clone(), ..Default::default() },
    )?
    .to_string();
    for role in ROLES {
        check_db(&store, role)?;
    }
    store.pause()?.close()?;
    let staged_identity = Some(stage_binding(stage.path())?);
    let policy = crate::load_config(vault.paths())?.capture_policy.token();
    let decision =
        serde_json::to_vec(&(stage.path(), &inputs, &staged_identity, &shared, &policy, &count))?;
    let (token, relative_copy) = copy_destination(vault, &decision)?;
    Ok(Prepared {
        reader_only: true,
        originals: inputs,
        shared_identity: shared,
        held_identity: None,
        source_identity: None,
        policy,
        staged_identity,
        preview: ConversationRepairPreview {
            action: "rebuild",
            copy_path: None,
            source_op: None,
            created_at: None,
            conversations: count.clone(),
            conversations_recovered_whole: Some(count),
            conversations_recovered_partially: Some("0".into()),
            conversations_only_in_copy: Some("0".into()),
            skipped_optional_records: "0".into(),
            unreadable_optional_sections: "0".into(),
            kept_as: vault.paths().app_root.join(&relative_copy).display().to_string(),
            review_sources_before_backup: false,
            next_backup_recaptures_tool_files: false,
            archive_only_conversations_unreadable: false,
            token,
        },
        stage,
        relative_copy,
    })
}
