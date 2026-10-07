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
    let mut snapshot = retention_directory_bucket("snapshots", &paths.raw_snapshots_dir);
    if let Some(vault) = vault {
        snapshot.item_count = vault.store().read(Db::Archive, |s| {
            s.query_one("SELECT count(*) FROM snapshots", (), |r| Ok(r.get::<i64>(0)?))
        })? as usize;
    }
    Ok(RetentionPreview {
        buckets: vec![
            snapshot,
            retention_directory_bucket("exports", &paths.exports_dir),
            retention_directory_bucket("staging", &paths.staging_dir),
            retention_directory_bucket("quarantine", &paths.quarantine_dir),
        ],
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
pub fn prune(
    vault: &Vault,
    request: &RetentionPruneRequest,
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
    let protected = if selected.iter().any(|b| b.id == "snapshots") {
        crate::list_recovery_snapshots(paths)?
            .into_iter()
            .find(|s| s.verified_openable)
            .map(|s| std::path::PathBuf::from(s.path))
    } else {
        None
    };
    cancel.check()?;
    let run = write_runs::start(vault, "retention_prune", "manual", &[])?;
    let result = (|| {
        let mut bytes = 0;
        let mut files = 0;
        for bucket in &selected {
            cancel.check()?;
            let (removed_bytes, removed_files) = match bucket.id.as_str() {
                "snapshots" => {
                    let removed = remove_directory_contents_except(
                        &paths.raw_snapshots_dir,
                        protected.as_deref(),
                    )?;
                    let keep = protected.as_ref().map(|p| p.display().to_string());
                    vault.store().write(Db::Archive, Priority::Batch, move |s| {
                        let deleted = match keep {
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
                "exports" => remove_directory_contents(&paths.exports_dir)?,
                "staging" => remove_directory_contents(&paths.staging_dir)?,
                "quarantine" => remove_directory_contents(&paths.quarantine_dir)?,
                _ => unreachable!("selected buckets come from the fixed preview"),
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
        let paths = paths.clone();
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
        })?;
        Ok(RetentionPruneResult {
            run_id: Some(run),
            deleted_bytes: bytes,
            deleted_files: files,
            buckets: selected,
            warnings: preview.warnings,
        })
    })();
    if let Err(error) = &result {
        write_runs::stopped(vault, run, error, serde_json::json!({}), None)?;
    }
    result
}
