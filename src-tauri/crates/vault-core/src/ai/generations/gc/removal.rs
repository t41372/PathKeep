//! Review and retire all browsing generations at one Intelligence commit. Collection owns the
//! file inventory, while the ledger owns publication. Conversation vectors have a separate owner.
use super::{Reservation, clean_trash, trash_path};
use crate::{Vault, ai::source_watermark, ai_sidecar};
use anyhow::{Context, Result, ensure};
use std::{
    fs,
    hash::{DefaultHasher, Hash, Hasher},
    path::Path,
    sync::Arc,
};
use vault_store::{Db, OptionalRow, Priority, Session};

/// The identity of the files and database state seen by a space preview, local to its owner.
pub struct KeptIndex {
    pub bytes: u64,
    files: u64,
    archive: Arc<()>,
    database: DatabaseStamp,
}

#[derive(PartialEq, Eq, Clone, Copy)]
struct DatabaseStamp {
    visits: i64,
    writes: i64,
}

/// Refusals the command layer translates into the same codes as Agent index removal.
#[derive(Debug, thiserror::Error)]
pub enum RemovalRefusal {
    #[error("archive-busy")]
    Busy,
    #[error("preview-expired")]
    Expired,
}

fn database_stamp(s: &Session<'_>) -> Result<Option<DatabaseStamp>> {
    let (active, writes) = s.query_one(
        "SELECT EXISTS(SELECT 1 FROM ai_jobs WHERE state IN ('queued','running')
            AND job_type IN ('index-build','index-clear')), total_changes()",
        (),
        |r| Ok((r.get::<i64>(0)?, r.get::<i64>(1)?)),
    )?;
    Ok((active == 0)
        .then_some(DatabaseStamp { visits: source_watermark::read(s, Db::Intelligence)?, writes }))
}

// File lengths and names distinguish unpublished and retired generations too. Stat files only:
// hashing gigabytes of immutable vectors to open a confirmation dialog would stall the disk.
fn inventory(v: &Vault) -> Result<(u64, u64)> {
    let mut bytes = 0;
    let mut signature = DefaultHasher::new();
    for path in ai_sidecar::storage_roots(v.paths()) {
        let parent = path.parent().context("semantic index path has no parent")?;
        if parent.try_exists()? {
            ensure!(
                parent.canonicalize()?
                    == v.paths()
                        .app_root
                        .canonicalize()?
                        .join(parent.strip_prefix(&v.paths().app_root)?),
                "semantic index parent is linked outside its archive"
            );
        }
        match fs::symlink_metadata(&path) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            meta => ensure!(!meta?.is_symlink(), "linked semantic index directory"),
        }
        for entry in walkdir::WalkDir::new(&path).follow_root_links(false).sort_by_file_name() {
            let entry = entry?;
            entry.path().hash(&mut signature);
            let meta = entry.metadata()?;
            meta.modified()?.hash(&mut signature);
            meta.len().hash(&mut signature);
            if meta.is_file() {
                bytes += meta.len();
            }
        }
    }
    Ok((bytes, signature.finish()))
}

/// Capture a receipt without scanning embeddings or visits. Both database stamps are read on
/// the Intelligence writer, so its connection-local change counter is comparable at confirmation.
pub fn kept_browsing_index(v: &Vault) -> Result<KeptIndex> {
    let _backup = v.begin_backup().map_err(|_| RemovalRefusal::Busy)?;
    let _collection = v.vector_generations.admit().ok_or(RemovalRefusal::Busy)?;
    let archive = v.store().read_token(Db::Archive)?;
    let database = v
        .write(Db::Intelligence, Priority::Interactive, database_stamp)?
        .ok_or(RemovalRefusal::Busy)?;
    let (bytes, files) = inventory(v)?;
    Ok(KeptIndex { bytes, files, archive, database })
}

/// Unpublish every reviewed pointer together, then use the collector's reservation and trash.
/// A reader can finish with its old generation before any of that generation's files move.
pub fn free_browsing_index(
    v: &Vault,
    kept: &KeptIndex,
    authorize: impl FnOnce() -> Result<crate::ControlGate>,
) -> Result<()> {
    let backup = v.begin_backup().map_err(|_| RemovalRefusal::Busy)?;
    let state = &v.vector_generations;
    let _collection = state.admit().ok_or(RemovalRefusal::Busy)?;
    let current = inventory(v)?;
    let archive = v.store().read_token(Db::Archive)?;
    let files_match = current == (kept.bytes, kept.files) && Arc::ptr_eq(&archive, &kept.archive);
    let reviewed = kept.database;
    let control = authorize()?;
    let result = v.write(Db::Intelligence, Priority::Interactive, move |s| {
        let Some(current) = database_stamp(s)? else { return Ok(Err(RemovalRefusal::Busy)) };
        if !files_match || current != reviewed {
            return Ok(Err(RemovalRefusal::Expired));
        }
        // Paused and interrupted jobs must never resume a cursor into a discarded lineage.
        s.execute(
            "UPDATE ai_jobs SET state='cancelled',stop_requested=1,finished_at=?1,updated_at=?1,
                heartbeat_at=NULL,lease_owner=NULL,lease_expires_at=NULL
                WHERE state IN ('paused','stale') AND job_type IN ('index-build','index-clear')",
            [crate::utils::now_rfc3339()],
        )?;
        s.execute("DELETE FROM ai_index_ledger", ())?;
        Ok(Ok(()))
    })?;
    result?;
    drop(control);
    #[cfg(test)]
    state.test_hook("removal_committed");
    drop(backup);
    let root = &v.paths().vectors_dir;
    let _reservation = {
        let registry = state.registry.lock().unwrap_or_else(|e| e.into_inner());
        let mut registry = state
            .released
            .wait_while(registry, |r| !state.is_closed() && !r.pins.is_empty())
            .unwrap_or_else(|e| e.into_inner());
        ensure!(!state.is_closed(), "archive closed during semantic index removal");
        registry.collecting.insert(root.clone());
        Reservation { state, paths: vec![root.clone()] }
    };
    // Clear rows in the existing bounded GC transactions. Totals survive until their final
    // batch, so a later preview/collection can finish work interrupted by an I/O failure.
    while let Some((provider, model, lineage)) = v.read(Db::Intelligence, |s| {
        // Look at the first indexed key even if an older archive has lost its totals row.
        Ok(s.query_one(
            "SELECT provider_id,model,lineage FROM ai_embedding_totals
            UNION ALL SELECT provider_id,model,lineage FROM ai_embeddings LIMIT 1",
            (),
            |r| Ok((r.get::<String>(0)?, r.get::<String>(1)?, r.get::<String>(2)?)),
        )
        .optional()?)
    })? {
        crate::ai::indexing::store_rows::remove_lineage_rows(v, &provider, &model, &lineage)?;
    }
    if root.is_file() {
        crate::durable_io::remove_file_durably(root)?;
    } else if root.try_exists()? {
        let trash = root.join("trash");
        if trash.is_symlink() || trash.is_file() {
            crate::durable_io::remove_file_durably(&trash)?;
        }
        fs::create_dir_all(&trash)?;
        for entry in fs::read_dir(root)? {
            let path = entry?.path();
            if path != trash {
                trash_path(v, &path, &trash)?;
            }
        }
        clean_trash(v, &trash)?;
    }
    clear_legacy(&v.paths().semantic_index_dir)?;
    ensure!(inventory(v)?.0 == 0, "semantic index bytes remain after collection");
    Ok(())
}

fn clear_legacy(path: &Path) -> Result<()> {
    if !path.try_exists()? {
        return Ok(());
    }
    if path.is_file() {
        return crate::durable_io::remove_file_durably(path);
    }
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        if entry.file_type()?.is_dir() {
            vault_fs::remove_dir_all(entry.path())?;
        } else {
            crate::durable_io::remove_file_durably(&entry.path())?;
        }
    }
    crate::durable_io::fsync_dir(path)
}
