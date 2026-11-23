//! A move installation uses the local conversation evidence as its base. Only the private
//! stage changes until the enclosing install publishes both browsing and conversations.
use crate::{AppConfig, ProjectPaths, Vault, maintenance};
use anyhow::{Context, Result};
use std::{fs, path::Path};
use vault_crypto::DataKey;
use vault_store::{CancelToken, Db, Priority};

const E: Db = Db::AgentHistoryEvidence;
const R: Db = Db::AgentHistoryReadModel;

/// Keep the original move file's evidence available for the existing capture-grant check.
pub(super) struct Combined {
    pub batch_id: String,
    pub incoming: Option<tempfile::TempDir>,
}

pub(super) fn combine(
    paths: &ProjectPaths,
    current: Option<&Vault>,
    stage: &Path,
    config: &AppConfig,
    key: Option<&DataKey>,
    cancel: &CancelToken,
) -> Result<Combined> {
    let staged_paths = crate::project_paths_with_root(stage);
    let open = || Vault::open_staged(staged_paths.clone(), config.clone(), key);
    let incoming = open()?;
    let id = incoming.store().read(E, |s| {
        s.query_one("SELECT lower(hex(randomblob(16)))", (), |r| Ok(r.get::<String>(0)?))
    })?;
    let date = crate::utils::now_rfc3339();
    let detached;
    let scratch;
    let local = if let Some(v) =
        current.filter(|v| v.layout() == vault_layout::ArchiveLayout::AgentHistoryV2)
    {
        Some(v)
    } else if [E, R].iter().any(|db| paths.app_root.join(vault_layout::database_path(*db)).exists())
    {
        anyhow::ensure!(
            !paths.artifact_path(vault_layout::ArtifactId::Envelope).exists(),
            "agent-history-key-required: enter the current archive password before importing"
        );
        scratch = tempfile::tempdir_in(stage)?;
        for role in [E, R] {
            let path = vault_layout::database_path(role);
            for suffix in
                vault_layout::CompanionFamily::Native.suffixes(vault_layout::CompanionUse::Install)
            {
                let relative = format!("{path}{suffix}");
                let source = paths.app_root.join(&relative);
                if suffix.is_empty() || source.try_exists()? {
                    vault_store::copy_archive_file(
                        vault_store::ArchiveFileRole::for_member(Path::new(&relative)),
                        &source,
                        &scratch.path().join(relative),
                    )?;
                }
            }
        }
        let source = crate::project_paths_with_root(scratch.path());
        let cfg = AppConfig {
            initialized: true,
            archive_mode: crate::ArchiveMode::Plaintext,
            ..Default::default()
        };
        crate::config::write_staged_control(
            &source,
            &cfg,
            Some(&crate::admission::written_compatibility()),
        )?;
        detached = Vault::open_staged(source, cfg, None)?;
        Some(&detached)
    } else {
        None
    };
    let Some(local) = local else {
        let batch = id.clone();
        incoming.store().write(E, Priority::Batch, move |s| {
            agent_history::archive::move_import::record_new(s, &batch, &date, true)
        })?;
        if incoming.has_saved_transcripts()? {
            incoming
                .agent_history()?
                .synchronize_search_with_cancel(&|| cancel.check().is_err())?;
        }
        incoming.close()?;
        return Ok(Combined { batch_id: id, incoming: None });
    };

    let has_evidence = incoming.store().read(E, |s| {
        s.query_one(
            "SELECT EXISTS(SELECT 1 FROM ah_sources) OR EXISTS(SELECT 1 FROM ah_backup_runs)",
            (),
            |r| Ok(r.get::<i64>(0)? != 0),
        )
    })?;
    let origin = incoming.store().read(E, |s| {
        s.query_one("SELECT origin FROM ah_move_origin", (), |r| Ok(r.get::<String>(0)?))
    })?;
    incoming.close()?;
    let donor = tempfile::tempdir_in(stage)?;
    let input = donor.path().join(vault_layout::database_path(E));
    fs::create_dir_all(input.parent().context("incoming evidence needs a parent")?)?;
    fs::rename(stage.join(vault_layout::database_path(E)), &input)?;
    fs::remove_file(stage.join(vault_layout::database_path(R)))?;
    for db in [E, R] {
        for suffix in
            vault_layout::CompanionFamily::Native.suffixes(vault_layout::CompanionUse::Install)
        {
            if suffix.is_empty() {
                continue;
            }
            let file = stage.join(format!("{}{suffix}", vault_layout::database_path(db)));
            if file.try_exists()? {
                fs::remove_file(file)?;
            }
        }
    }
    let search = staged_paths.artifact_path(vault_layout::ArtifactId::AgentHistorySearch);
    if search.try_exists()? {
        fs::remove_dir_all(search)?;
    }
    {
        let pause = local.pause_store()?;
        let roles: &[Db] = if has_evidence { &[E] } else { &[E, R] };
        maintenance::copy_databases(&pause, stage, key, cancel, |_| {}, roles)?;
    }
    let combined = open()?;
    if has_evidence {
        let subkey = key.map(|key| *key.derive(vault_layout::database_spec(E).key).expose_bytes());
        let token = cancel.clone();
        let batch = id.clone();
        combined.store().pause()?.with_staged_input(E, input, subkey, move |s| {
            agent_history::archive::move_import::merge(s, &origin, &batch, &date, &token)
        })?;
        let options = agent_history::capture::BackupOptions {
            read_git: false,
            cancel: cancel.clone(),
            ..Default::default()
        };
        combined.agent_history()?.rebuild_from_retained_evidence(&options)?;
    } else if combined.has_saved_transcripts()? {
        combined.agent_history()?.synchronize_search_with_cancel(&|| cancel.check().is_err())?;
    }
    combined.close()?;
    Ok(Combined { batch_id: id, incoming: Some(donor) })
}

// A conversation can have several source files. Count its receipt row once if any
// source identifies it as a root. Undo and its safety copy still need the child rows.
const IMPORTED_ROOTS: &str = "SELECT c.import_id, c.eligible FROM ah_import_conversations c
    WHERE EXISTS (SELECT 1 FROM ah_source_conversations s
                  WHERE s.conversation_id = c.conversation_id AND s.is_child = 0)";

/// Lists only imports that introduced a root, even after its browsing safety copy is freed.
pub fn imports(vault: &Vault) -> Result<Vec<serde_json::Value>> {
    vault.recheck_root()?;
    Ok(vault.store().read(E, |s| {
        s.query(
            &format!(
                "SELECT i.id, i.imported_at, i.previous_copy, count(*), sum(r.eligible)
                 FROM ah_move_imports i JOIN ({IMPORTED_ROOTS}) r ON r.import_id = i.id
                 GROUP BY i.id ORDER BY i.imported_at DESC"
            ),
            (),
            |r| {
                Ok(serde_json::json!({
                    "id": r.get::<String>(0)?,
                    "importedAt": r.get::<String>(1)?,
                    "previousCopy": r.get::<Option<String>>(2)?,
                    "added": r.get::<i64>(3)?,
                    "eligible": r.get::<i64>(4)?,
                }))
            },
        )
    })?)
}

fn eligible_roots(s: &vault_store::Session<'_>, id: &str) -> vault_store::Result<i64> {
    s.query_one(
        &format!("SELECT count(*) FROM ({IMPORTED_ROOTS}) WHERE import_id = ?1 AND eligible = 1"),
        [id],
        |r| Ok(r.get::<i64>(0)?),
    )
}

/// Keep a safety copy before moving the still-eligible conversations into Trash. A single
/// evidence transaction publishes all marks and their replay tickets.
pub fn undo(vault: &Vault, id: String) -> Result<serde_json::Value> {
    vault.recheck_root()?;
    let _operation = crate::archive::ArchiveOpGate::acquire(vault.paths());
    let archive = vault.agent_history()?;
    let _producers = vault.pause_conversations();
    let selected = id.clone();
    let eligible = vault.store().read(E, move |s| eligible_roots(s, &selected))?;
    if eligible == 0 {
        return Ok(serde_json::json!({"moved":0,"copyPath":null}));
    }
    let copy = super::undo_copy::publish(vault, &id)?;
    let count = vault.store().write(E, Priority::Interactive, move |s| {
        let n = eligible_roots(s, &id)?;
        if n == 0 { return Ok(0); }
        s.execute("UPDATE ah_state_generation SET generation=generation+1 WHERE id=1", ())?;
        s.execute("INSERT INTO ah_user_state(conversation_id,trashed_at) SELECT conversation_id,?2 FROM ah_import_conversations WHERE import_id=?1 AND eligible=1 ON CONFLICT(conversation_id) DO UPDATE SET trashed_at=coalesce(ah_user_state.trashed_at,excluded.trashed_at)",vault_store::params![id.clone(),chrono::Utc::now().timestamp_millis()])?;
        s.execute("INSERT OR REPLACE INTO ah_user_state_pending SELECT conversation_id,(SELECT generation FROM ah_state_generation WHERE id=1) FROM ah_import_conversations WHERE import_id=?1 AND eligible=1", [id.as_str()])?;
        s.execute("UPDATE ah_import_conversations SET eligible=0 WHERE import_id=?1", [id])?;
        Ok(n)
    })?;
    archive.refresh_import_marks()?;
    Ok(serde_json::json!({"moved":count,"copyPath":copy}))
}

pub(super) fn bind_previous(vault: &Vault, id: String, path: &Path) -> Result<()> {
    let path = path.file_name().and_then(|s| s.to_str()).context("previous copy name")?.to_owned();
    vault.store().write(E, Priority::Batch, move |s| {
        s.execute(
            "UPDATE ah_move_imports SET previous_copy=?2 WHERE id=?1",
            vault_store::params![id, path],
        )?;
        Ok(())
    })?;
    Ok(())
}
