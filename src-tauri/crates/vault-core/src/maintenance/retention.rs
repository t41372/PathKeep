//! Responsibilities: prune selected local artifacts and their native snapshot ledger.
//! Not responsible for: canonical fact deletion, password verification or legacy SQLCipher.
//! Failure modes: keychain-only owners need a password; the last restore point is
//! deleted; a running backup loses staging files; native rows point at pruned files.
//! Tests: maintenance::tests and retained filesystem retention regressions.
use crate::{
    ProjectPaths, RetentionPreview, RetentionPruneRequest, RetentionPruneResult, Vault,
    archive::{
        remove_directory_contents, remove_directory_contents_except, retention_directory_bucket,
        write_runs,
    },
};
use anyhow::Result;
use vault_store::{CancelToken, Db, Priority, params};

/// Filesystem sizing remains available before opening an archive.
pub fn preview_retention(paths: &ProjectPaths, vault: Option<&Vault>) -> Result<RetentionPreview> {
    let mut buckets = Vec::new();
    for (id, kind) in vault_layout::retention_members() {
        let mut bucket = retention_directory_bucket(kind.name(), &paths.artifact_path(id));
        if kind == vault_layout::RetentionBucket::Snapshots {
            if let Some(vault) = vault {
                bucket.item_count = vault.store().read(Db::Archive, |s| {
                    s.query_one("SELECT count(*) FROM snapshots", (), |r| Ok(r.get::<i64>(0)?))
                })? as usize;
            }
        }
        buckets.push(bucket);
    }
    Ok(RetentionPreview {
        buckets,
        warnings: vec![
            "Pruning snapshots removes saved restore checkpoints from future Audit review. Manifest and run summaries stay in place.".into(),
            "Export pruning removes only local files under the PathKeep data directory.".into(),
        ],
        warning_codes: vec![
            crate::RETENTION_WARNING_SNAPSHOT_PRUNE_REMOVES_CHECKPOINTS.into(),
            crate::RETENTION_WARNING_EXPORT_PRUNE_LOCAL_ONLY.into(),
        ],
    })
}

/// Retention shares the producer guard so it cannot remove an active backup's staging.
///
/// `keep` is the snapshot that must survive a snapshot prune: the caller's newest openable
/// one, native or old-format (the worker lists both).
pub fn prune(
    vault: &Vault,
    request: &RetentionPruneRequest,
    keep: Option<&std::path::Path>,
    cancel: &CancelToken,
) -> Result<RetentionPruneResult> {
    let _operation = crate::archive::ArchiveOpGate::acquire(vault.paths());
    let _producer = vault.begin_backup()?;
    let paths = vault.paths();
    let preview = preview_retention(paths, Some(vault))?;
    let selected = preview
        .buckets
        .into_iter()
        .filter(|b| request.bucket_ids.contains(&b.id))
        .collect::<Vec<_>>();
    if selected.is_empty() {
        return Ok(RetentionPruneResult::default());
    }
    let protected = keep.filter(|_| {
        selected.iter().any(|b| b.id == vault_layout::RetentionBucket::Snapshots.name())
    });
    cancel.check()?;
    let run = write_runs::start(vault, "retention_prune", "manual", &[])?;
    let result = (|| {
        let mut bytes = 0;
        let mut files = 0;
        for bucket in &selected {
            cancel.check()?;
            let (id, kind) = vault_layout::retention_members()
                .into_iter()
                .find(|(_, kind)| kind.name() == bucket.id)
                .expect("selected preview bucket");
            let directory = paths.artifact_path(id);
            let (removed_bytes, removed_files) = match kind {
                vault_layout::RetentionBucket::Snapshots => {
                    let removed = remove_directory_contents_except(&directory, protected)?;
                    let kept = protected.map(|p| p.display().to_string());
                    vault.store().write(Db::Archive, Priority::Batch, move |s| {
                        let deleted = match kept {
                            Some(path) => {
                                s.execute("DELETE FROM snapshots WHERE file_path<>?1", [path])?
                            }
                            None => s.execute("DELETE FROM snapshots", ())?,
                        };
                        crate::archive::totals::add(s, "snapshots", -(deleted as i64))?;
                        Ok(())
                    })?;
                    removed
                }
                vault_layout::RetentionBucket::Exports
                | vault_layout::RetentionBucket::Staging
                | vault_layout::RetentionBucket::Quarantine => {
                    remove_directory_contents(&directory)?
                }
            };
            bytes += removed_bytes;
            files += removed_files;
        }
        let details = serde_json::json!({"deletedBytes":bytes,"deletedFiles":files,"buckets":request.bucket_ids});
        write_runs::complete(
            vault,
            run,
            details.clone(),
            serde_json::to_value(&preview.warnings)?,
            crate::utils::now_rfc3339(),
        )?;
        // The files are gone; the manifest records it.
        write_runs::after_success(
            vault,
            run,
            "writing its manifest",
            write_manifest(vault, run, details),
        );
        Ok(RetentionPruneResult {
            run_id: Some(run),
            deleted_bytes: bytes,
            deleted_files: files,
            buckets: selected,
            warnings: preview.warnings,
        })
    })();
    if let Err(error) = &result {
        write_runs::end_failed(vault, run, error, serde_json::json!({}), None);
    }
    result
}

/// The prune's manifest, chained to the archive's latest one.
fn write_manifest(vault: &Vault, run: i64, details: serde_json::Value) -> Result<()> {
    let paths = vault.paths().clone();
    vault.write(Db::Archive, Priority::Batch, move |s| {
        let parent = s.query(
            "SELECT id,content_hash FROM manifests ORDER BY id DESC LIMIT 1", (),
            |r| Ok((r.get::<i64>(0)?, r.get::<String>(1)?)),
        )?.pop();
        let at = crate::utils::now_rfc3339();
        let text = serde_json::to_string(&serde_json::json!({
            "runType":"retention_prune", "runId":run, "createdAt":at,
            "parentManifestHash":parent.as_ref().map(|p|p.1.as_str()), "summary":details,
        }))?;
        let hash = crate::utils::sha256_hex(text.as_bytes());
        let path = crate::archive::write_manifest_artifact(&paths, run, &at, &hash, &text)?;
        s.execute(
            "INSERT INTO manifests(run_id,parent_manifest_id,content_hash,row_counts_json,created_at,file_path) VALUES(?1,?2,?3,'{}',?4,?5)",
            params![run, parent.map(|p|p.0), hash, at, path.display().to_string()],
        )?;
        crate::archive::totals::add(s, "manifests", 1)?;
        Ok(())
    })
}
