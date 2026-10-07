//! Responsibilities: retain backup manifests and finalize their run provenance.
//! Not responsible for: staging, row ingestion or full-text indexing.
//! Failure modes: manifest points at another database, committed run lacks audit
//! facts, partial results are marked complete. Tests: backup behavioral suites.

use super::{
    SnapshotArtifact,
    run_support::{BackupManifest, backup_run_summary, write_manifest_artifact},
    write_runs,
};
use crate::{
    Vault,
    models::{BackupProfileSummary, BackupReport, BackupWarning, backup_warning_messages},
    utils::{now_rfc3339, sha256_hex},
};
use anyhow::Result;
use std::collections::BTreeMap;
use vault_store::{Db, Priority, params};

pub(super) fn finish(
    vault: &Vault,
    run_id: i64,
    profiles: Vec<BackupProfileSummary>,
    mut warnings: Vec<BackupWarning>,
    source_hashes: BTreeMap<String, BTreeMap<String, String>>,
) -> Result<BackupReport> {
    let finished = now_rfc3339();
    let (kind,started,trigger,scope_json,due_only,timezone)=vault.store().read(Db::Archive,move |s|s.query_one(
        "SELECT run_type,started_at,trigger,profile_scope_json,due_only,timezone FROM runs WHERE id=?1",[run_id],
        |r|Ok((r.get::<String>(0)?,r.get::<String>(1)?,r.get::<String>(2)?,r.get::<String>(3)?,r.get::<i64>(4)?!=0,r.get::<String>(5)?))))?;
    let scope: Vec<String> = serde_json::from_str(&scope_json)?;
    let mut summary =
        backup_run_summary(&kind, run_id, &started, &finished, &trigger, &scope, &profiles);
    let (parent, counts, snapshots) = vault.read(Db::Archive, move |s| {
        let parent = s
            .query("SELECT id,content_hash FROM manifests ORDER BY id DESC LIMIT 1", (), |r| {
                Ok((r.get::<i64>(0)?, r.get::<String>(1)?))
            })?
            .pop();
        let mut totals = super::totals::load_cached_archive_totals(s)?;
        totals.remove("sourceProfiles");
        let counts = serde_json::to_value(totals)?;
        let snapshots = s.query(
            "SELECT file_path,checksum,reason FROM snapshots WHERE run_id=?1",
            [run_id],
            |r| {
                Ok(SnapshotArtifact {
                    kind: "raw-source-checkpoint".into(),
                    path: r.get(0)?,
                    checksum: r.get(1)?,
                    reason: r.get(2)?,
                })
            },
        )?;
        Ok((parent, counts, snapshots))
    })?;
    let manifest = BackupManifest {
        created_at: finished.clone(),
        run_id,
        timezone,
        due_only,
        database_path: vault.paths().archive_database_path.display().to_string(),
        summary: summary.clone(),
        profiles: profiles.clone(),
        warnings: backup_warning_messages(&warnings),
        source_hashes,
        snapshots,
        row_counts: counts.clone(),
        parent_manifest_hash: parent.as_ref().map(|p| p.1.clone()),
    };
    let text = serde_json::to_string_pretty(&manifest)?;
    let hash = sha256_hex(text.as_bytes());
    let path = write_manifest_artifact(vault.paths(), run_id, &finished, &hash, &text)?;
    let stored_finished = finished.clone();
    let (stored_hash, stored_path) = (hash.clone(), path.display().to_string());
    vault.store().write(Db::Archive,Priority::Batch,move |s| {
        s.execute("INSERT INTO manifests(run_id,parent_manifest_id,content_hash,row_counts_json,created_at,file_path) VALUES(?1,?2,?3,?4,?5,?6)",
            params![run_id,parent.map(|p|p.0),stored_hash,counts.to_string(),stored_finished,stored_path])?; super::totals::add(s, "manifests", 1)?; Ok(())
    })?;
    let git_commit = if vault.config().git_enabled {
        let (commit, warning) = crate::git_audit::commit_all_optional(
            &vault.paths().audit_repo_path,
            &audit_message(&kind, run_id),
        );
        warnings.extend(warning);
        commit
    } else {
        None
    };
    write_runs::complete(
        vault,
        run_id,
        serde_json::json!({"manifestHash":hash,"profilesProcessed":profiles.len()}),
        serde_json::to_value(&warnings)?,
        finished,
    )?;
    summary.manifest_hash = Some(hash);
    Ok(BackupReport {
        run: Some(summary),
        profiles,
        git_commit,
        manifest_path: Some(path.display().to_string()),
        warnings: backup_warning_messages(&warnings),
        warning_details: warnings,
        ..BackupReport::default()
    })
}

fn audit_message(kind: &str, run: i64) -> String {
    let action = if kind == "snapshot_restore" {
        "restore: replay source checkpoint"
    } else {
        "backup: archive browser history"
    };
    format!(
        "{action} (run {run})\n\nKeep the run manifest and source provenance available for archive review."
    )
}
#[cfg(test)]
mod tests {
    #[test]
    fn checkpoint_audit_message_names_restore_and_its_reason() {
        let message = super::audit_message("snapshot_restore", 42);
        assert!(message.starts_with("restore: replay source checkpoint (run 42)\n\n"));
        assert!(message.contains("source provenance"));
    }
}
