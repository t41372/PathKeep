//! Responsibilities: retain reviewable runs and manifests for file replacement.
//! Not responsible for: password rewrap (which must not write databases) or swaps.
//! Failure modes: restored data has no review run; locked status loses the review;
//! a failed artifact write rolls back an already-installed archive.
//! Tests: worker full-restore/security acceptance and maintenance recovery.
use crate::{Vault, archive::write_runs};
use anyhow::Result;
use vault_store::{Db, Priority, params};

/// Review remains separate from verification of the copied source rows.
pub fn record_restore(vault: &Vault, snapshot: &str) -> Result<i64> {
    record(vault, "archive_restore", snapshot, "restored-archive-safety-snapshot")
}

/// Mode changes retain a snapshot in the destination mode. Rewrap writes no run.
pub fn record_mode_change(vault: &Vault, snapshot: &str) -> Result<i64> {
    record(vault, "rekey", snapshot, "before-rekey")
}

fn record(vault: &Vault, kind: &'static str, snapshot: &str, reason: &'static str) -> Result<i64> {
    let snapshot = snapshot.to_owned();
    let bytes = walkdir::WalkDir::new(&snapshot)
        .into_iter()
        .map(|entry| {
            let entry = entry?;
            Ok(if entry.file_type().is_file() { entry.metadata()?.len() } else { 0 })
        })
        .collect::<Result<Vec<u64>>>()?
        .into_iter()
        .sum::<u64>();
    let mode = vault.config().archive_mode.clone();
    let stored_snapshot = snapshot.clone();
    let id = vault.write(Db::Archive, Priority::Batch, move |s| {
        let id = write_runs::start_in(s, kind, "manual", &[])?;
        s.execute(
            "INSERT INTO snapshots(run_id,file_path,file_size,reason,created_at) VALUES(?1,?2,?3,?4,?5)",
            params![id, stored_snapshot.as_str(), bytes as i64, reason, crate::utils::now_rfc3339()],
        )?;
        crate::archive::totals::add(s, "snapshots", 1)?;
        write_runs::complete_in(
            s, id, serde_json::json!({"snapshotPath":stored_snapshot,"targetMode":mode}), serde_json::json!([]),
        )?;
        Ok(id)
    })?;
    let paths = vault.paths().clone();
    // The run and its snapshot are recorded; the manifest follows, and its failure is the
    // run's warning (the caller had reported no run at all).
    let manifest = vault.write(Db::Archive, Priority::Batch, move |s| {
        let parent = s.query(
            "SELECT id,content_hash FROM manifests ORDER BY id DESC LIMIT 1", (),
            |r| Ok((r.get::<i64>(0)?, r.get::<String>(1)?)),
        )?.pop();
        let at = s.query_one("SELECT finished_at FROM runs WHERE id=?1", [id], |r| Ok(r.get::<String>(0)?))?;
        let counts = crate::archive::totals::load_cached_archive_totals(s)?;
        let payload = serde_json::json!({
            "runType":kind, "runId":id, "createdAt":at, "snapshotPath":snapshot,
            "parentManifestHash":parent.as_ref().map(|p|p.1.as_str()), "rowCounts":counts,
        });
        let text = serde_json::to_string_pretty(&payload)?;
        let hash = crate::utils::sha256_hex(text.as_bytes());
        let path = crate::archive::write_manifest_artifact(&paths, id, &at, &hash, &text)?;
        s.execute(
            "INSERT INTO manifests(run_id,parent_manifest_id,content_hash,row_counts_json,created_at,file_path) VALUES(?1,?2,?3,?4,?5,?6)",
            params![id, parent.map(|p|p.0), hash, serde_json::to_string(&counts)?, at, path.display().to_string()],
        )?;
        crate::archive::totals::add(s, "manifests", 1)?;
        Ok(())
    });
    write_runs::after_success(vault, id, "writing its manifest", manifest);
    Ok(id)
}
