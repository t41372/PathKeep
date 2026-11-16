//! Responsibilities: prune selected local artifacts, the safety copies the user picked one by one,
//! and their snapshot ledger rows.
//! Not responsible for: canonical fact deletion, password verification, listing the copies (the
//! worker lists native and old-format ones) or legacy SQLCipher.
//! Failure modes: a copy the user did not pick is deleted (before 2026-11-15 the newest openable
//! copy was always kept and every other one went, so right after a restore the copy it made could
//! not be freed and the one just restored from was deleted); a copy made while the prune waited is
//! deleted with the other snapshots; a path that is not a listed copy is deleted on request; an
//! old-format copy leaves the files SQLite keeps beside it behind, or loses them while it stays;
//! the size shown is not what is deleted; a symlink is followed out of the data folder; a running
//! backup loses staging files; ledger rows outlive their files, or go with files that stay.
//! Tests: retention_tests.
use crate::{
    ProjectPaths, RecoverySnapshot, RetentionBucket, RetentionCopy, RetentionPreview,
    RetentionPruneRequest, RetentionPruneResult, Vault, archive::write_runs,
};
use anyhow::Result;
use std::{
    fs, io,
    path::{Path, PathBuf},
};
use vault_store::{CancelToken, Db, Priority, params};

/// Filesystem sizing remains available before opening an archive. `copies` are the safety copies
/// the restore list offers: each is a row of its own, and the snapshots bucket is what the
/// snapshots folder holds besides them.
pub fn preview_retention(
    paths: &ProjectPaths,
    copies: &[RecoverySnapshot],
) -> Result<RetentionPreview> {
    let kept = every_member(copies);
    let mut buckets = Vec::new();
    for (id, kind) in vault_layout::retention_members() {
        let directory = paths.artifact_path(id);
        let (bytes, item_count) = walk_except(&directory, bucket_keeps(kind, &kept), measure)?;
        buckets.push(RetentionBucket {
            id: kind.name().into(),
            bytes,
            item_count,
            paths: vec![directory.display().to_string()],
        });
    }
    let copies = copies
        .iter()
        .map(|copy| {
            let bytes =
                members(copy).iter().map(|member| Ok(measure(member)?.0)).sum::<Result<u64>>()?;
            Ok(RetentionCopy {
                path: copy.path.clone(),
                source_op: copy.source_op.clone(),
                created_at: copy.created_at.clone(),
                bytes,
                restorable: copy.verified_openable,
                unchecked: copy.unchecked,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(RetentionPreview {
        buckets,
        copies,
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
/// `list_copies` lists the safety copies once no other archive operation runs, so a copy a restore
/// made while this one waited is listed. A copy is deleted only when the request names it and it
/// is listed; the snapshots bucket never takes a listed copy with it.
pub fn prune(
    vault: &Vault,
    request: &RetentionPruneRequest,
    list_copies: impl FnOnce() -> Result<Vec<RecoverySnapshot>>,
    cancel: &CancelToken,
) -> Result<RetentionPruneResult> {
    vault.recheck_root()?;
    let _operation = crate::archive::ArchiveOpGate::acquire(vault.paths());
    let _producer = vault.begin_backup()?;
    let paths = vault.paths();
    let copies = list_copies()?;
    let picked = request
        .copies
        .iter()
        .map(|requested| {
            copies.iter().find(|copy| copy.path == *requested).ok_or_else(|| {
                anyhow::anyhow!(
                    "copy-not-listed: {requested} is not a safety copy PathKeep lists; nothing was deleted"
                )
            })
        })
        .collect::<Result<Vec<_>>>()?;
    let preview = preview_retention(paths, &copies)?;
    let selected = preview
        .buckets
        .into_iter()
        .filter(|b| request.bucket_ids.contains(&b.id))
        .collect::<Vec<_>>();
    if selected.is_empty() && picked.is_empty() {
        return Ok(RetentionPruneResult::default());
    }
    let kept = every_member(&copies);
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
            let (removed_bytes, removed_files) =
                walk_except(&paths.artifact_path(id), bucket_keeps(kind, &kept), remove_path)?;
            if kind == vault_layout::RetentionBucket::Snapshots {
                let listed = copies.iter().map(|copy| copy.path.clone()).collect::<Vec<_>>();
                forget(vault, move |path| !listed.iter().any(|copy| copy == path))?;
            }
            bytes += removed_bytes;
            files += removed_files;
        }
        for copy in &picked {
            cancel.check()?;
            for member in members(copy) {
                let (removed_bytes, removed_files) = remove_path(&member)?;
                bytes += removed_bytes;
                files += removed_files;
            }
            let path = copy.path.clone();
            forget(vault, move |row| row == path)?;
        }
        let details = serde_json::json!({"deletedBytes":bytes,"deletedFiles":files,"buckets":request.bucket_ids,"copies":request.copies});
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

/// The files a listed copy is made of: a native copy is one folder, an old-format one a SQLite file
/// and the files SQLite may keep beside it.
fn members(copy: &RecoverySnapshot) -> Vec<PathBuf> {
    let path = Path::new(&copy.path);
    if fs::symlink_metadata(path).is_ok_and(|metadata| metadata.is_dir()) {
        return vec![path.to_owned()];
    }
    vault_layout::CompanionFamily::LegacySqlite
        .suffixes(vault_layout::CompanionUse::Recovery)
        .iter()
        .map(|suffix| {
            let mut name = path.as_os_str().to_owned();
            name.push(suffix);
            PathBuf::from(name)
        })
        .collect()
}

/// Every file of every listed copy.
fn every_member(copies: &[RecoverySnapshot]) -> Vec<PathBuf> {
    copies.iter().flat_map(members).collect()
}

/// What a bucket leaves in place: the snapshots bucket the listed copies, the others nothing.
fn bucket_keeps(kind: vault_layout::RetentionBucket, kept: &[PathBuf]) -> &[PathBuf] {
    if kind == vault_layout::RetentionBucket::Snapshots { kept } else { &[] }
}

/// Runs `each` on every entry of the folder `root` that holds none of `keep`, and goes into those
/// that do, so the kept paths and the folders on their way stay; `root` stays too. `root` is
/// followed when it is a link (a user who keeps the folder elsewhere); the entries are not.
fn walk_except(
    root: &Path,
    keep: &[PathBuf],
    each: fn(&Path) -> Result<(u64, usize)>,
) -> Result<(u64, usize)> {
    let entries = match fs::read_dir(root) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok((0, 0)),
        Err(error) => return Err(error.into()),
    };
    let mut totals = (0, 0);
    for entry in entries {
        let path = entry?.path();
        if keep.contains(&path) {
            continue;
        }
        let (bytes, files) = if keep.iter().any(|kept| kept.starts_with(&path)) {
            walk_except(&path, keep, each)?
        } else {
            each(&path)?
        };
        totals = (totals.0 + bytes, totals.1 + files);
    }
    Ok(totals)
}

/// Removes one file, folder or link and returns the bytes and files it deleted. A link goes as a
/// link: what it points to, possibly outside the data folder, is never touched.
fn remove_path(path: &Path) -> Result<(u64, usize)> {
    let Some(metadata) = metadata(path)? else {
        return Ok((0, 0));
    };
    if metadata.is_dir() {
        let mut totals = (0, 0);
        for entry in fs::read_dir(path)? {
            let (bytes, files) = remove_path(&entry?.path())?;
            totals = (totals.0 + bytes, totals.1 + files);
        }
        crate::durable_io::retry_while_held(|| fs::remove_dir(path))?;
        return Ok(totals);
    }
    #[cfg(windows)]
    if std::os::windows::fs::FileTypeExt::is_symlink_dir(&metadata.file_type()) {
        // Windows keeps a link to a folder as a folder entry, which `remove_file` refuses.
        crate::durable_io::retry_while_held(|| fs::remove_dir(path))?;
        return Ok((0, 1));
    }
    crate::durable_io::retry_while_held(|| fs::remove_file(path))?;
    Ok((if metadata.is_file() { metadata.len() } else { 0 }, 1))
}

/// The bytes and files [`remove_path`] would delete, deleting nothing.
pub(super) fn measure(path: &Path) -> Result<(u64, usize)> {
    let Some(metadata) = metadata(path)? else {
        return Ok((0, 0));
    };
    if !metadata.is_dir() {
        return Ok((if metadata.is_file() { metadata.len() } else { 0 }, 1));
    }
    let mut totals = (0, 0);
    for entry in fs::read_dir(path)? {
        let (bytes, files) = measure(&entry?.path())?;
        totals = (totals.0 + bytes, totals.1 + files);
    }
    Ok(totals)
}

/// The path's own metadata, a link's rather than its target's; `None` when it is not there.
fn metadata(path: &Path) -> Result<Option<fs::Metadata>> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => Ok(Some(metadata)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error.into()),
    }
}

/// Drops the ledger rows of the files `gone` says were deleted, right after they were, so a prune
/// cut short leaves no row without its file.
fn forget(vault: &Vault, gone: impl Fn(&str) -> bool + Send + 'static) -> Result<()> {
    vault.write(Db::Archive, Priority::Batch, move |s| {
        let rows = s.query("SELECT id,file_path FROM snapshots", (), |r| {
            Ok((r.get::<i64>(0)?, r.get::<String>(1)?))
        })?;
        let mut deleted = 0i64;
        for (id, _) in rows.iter().filter(|(_, path)| gone(path)) {
            deleted += s.execute("DELETE FROM snapshots WHERE id=?1", [*id])? as i64;
        }
        crate::archive::totals::add(s, "snapshots", -deleted)?;
        Ok(())
    })
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
