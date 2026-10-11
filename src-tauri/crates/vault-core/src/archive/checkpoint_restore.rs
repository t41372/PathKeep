//! Responsibilities: replay saved browser checkpoints through the batched writer.
//! Not responsible for: replacing whole archives, SQLCipher rekey or retention.
//! Failure modes: wrong profile replayed, duplicate visits, interrupted replay
//! drops committed rows. Tests: archive checkpoint replay and ingest dedup suites.

use super::*;
use crate::Vault;
use vault_store::{CancelToken, Db};

/// Checkpoint replay shares the archive owner and dedup rules with normal backup.
pub fn run_snapshot_restore(
    vault: &Vault,
    request: &SnapshotRestoreRequest,
) -> Result<BackupReport> {
    let _backup = vault.begin_backup()?;
    anyhow::ensure!(vault.config().initialized, "archive has not been initialized");
    let path = PathBuf::from(&request.snapshot_path);
    anyhow::ensure!(path.is_dir(), "automatic restore requires a saved browser source checkpoint");
    let record = record(vault, &request.snapshot_path)?;
    let created_at = record.created_at.clone();
    let checkpoint = checkpoint(vault, &record)?;
    let run_id = write_runs::start(
        vault,
        "snapshot_restore",
        "manual",
        std::slice::from_ref(&checkpoint.profile.profile_id),
    )?;
    let mut profiles = Vec::new();
    let result = (|| {
        let summary = ingest::process_profile_snapshot(
            vault,
            run_id,
            &checkpoint,
            false,
            false,
            &CancelToken::new(),
            None,
        )?;
        profiles.push(summary);
        let (size, checksum) = super::artifacts::snapshot_artifact_bytes_and_checksum(&path)?;
        let stored = path.display().to_string();
        vault.store().write(Db::Archive,vault_store::Priority::Batch,move |s| {
            s.execute("INSERT INTO snapshots(run_id,file_path,file_size,checksum,reason,created_at) VALUES(?1,?2,?3,?4,'restored-source-checkpoint',?5)",vault_store::params![run_id,stored,size as i64,checksum,created_at])?;super::totals::add(s, "snapshots", 1)?;Ok(())
        })?;
        super::backup_manifest::finish(
            vault,
            run_id,
            profiles.clone(),
            Vec::new(),
            BTreeMap::from([(
                checkpoint.profile.profile_id.clone(),
                ingest::snapshot_source_hashes(&checkpoint),
            )]),
        )
    })();
    if let Err(error) = &result {
        if profiles.is_empty() {
            let stats: Value = vault.read(Db::Archive, move |s| {
                Ok(serde_json::from_str(&s.query_one(
                    "SELECT stats_json FROM runs WHERE id=?1",
                    [run_id],
                    |r| Ok(r.get::<String>(0)?),
                )?)?)
            })?;
            profiles.push(BackupProfileSummary {
                profile_id: checkpoint.profile.profile_id.clone(),
                new_urls: stats["newUrls"].as_u64().unwrap_or(0) as usize,
                new_visits: stats["newVisits"].as_u64().unwrap_or(0) as usize,
                new_downloads: stats["newDownloads"].as_u64().unwrap_or(0) as usize,
                same_visits: 0,
                replaced_profiles: Vec::new(),
                checkpoint_created: false,
                notes: vec![],
                note_details: vec![],
            });
        }
        write_runs::stopped(
            vault,
            run_id,
            error,
            json!({"profilesProcessed":profiles.len(),"profiles":profiles}),
            Some(serde_json::to_value(
                profiles.iter().flat_map(|p| &p.note_details).collect::<Vec<_>>(),
            )?),
        )?;
    }
    result
}

fn record(vault: &Vault, path: &str) -> Result<super::artifacts::SnapshotRecord> {
    let path = path.to_owned();
    vault.read(Db::Archive,move |s| {
        s.query("SELECT s.run_id,r.profile_scope_json,s.file_path,s.created_at,s.reason FROM snapshots s JOIN runs r ON r.id=s.run_id WHERE s.file_path=?1 ORDER BY s.id LIMIT 1",[path],|r|Ok(super::artifacts::SnapshotRecord {
            run_id:r.get(0)?,profile_scope:decode_profile_scope(r.get::<Option<String>>(1)?.as_deref()),file_path:r.get(2)?,created_at:r.get(3)?,reason:r.get(4)?,
        }))?.pop().context("snapshot was not found in the recorded ledger")
    })
}

pub(super) fn checkpoint(
    vault: &Vault,
    record: &super::artifacts::SnapshotRecord,
) -> Result<ProfileSnapshot> {
    let path = Path::new(&record.file_path);
    let profile_id = profile_id(path, record)?;
    let history = path.join("History");
    anyhow::ensure!(history.is_file(), "snapshot is not a saved browser source checkpoint");
    let favicons = path.join("Favicons");
    let id = profile_id.clone();
    let row=vault.read(Db::Archive,move |s|Ok(s.query("SELECT browser_kind,browser_version,profile_name,profile_path,user_name FROM source_profiles WHERE profile_key=?1",[id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?)))?.pop()))?;
    let profile =
        super::artifacts::snapshot_browser_profile(&profile_id, &history, favicons.exists(), row);
    let source_hashes = super::artifacts::snapshot_file_fingerprints(
        &history,
        favicons.exists().then_some(favicons.as_path()),
    )?;
    Ok(ProfileSnapshot {
        profile,
        temp_dir: tempdir()?,
        history_path: history,
        favicons_path: favicons.exists().then_some(favicons),
        source_hashes,
    })
}

/// Preview uses the same recorded scope and source files as checkpoint execution.
pub fn preview_snapshot_restore(
    vault: &Vault,
    request: &SnapshotRestoreRequest,
) -> Result<SnapshotRestorePreview> {
    let record = record(vault, &request.snapshot_path)?;
    if Path::new(&record.file_path).is_file() {
        return Ok(SnapshotRestorePreview {
            snapshot_path:record.file_path,snapshot_kind:"archive-safety-snapshot".into(),source_run_id:Some(record.run_id),
            source_profile_id:record.profile_scope.first().cloned(),source_browser_name:None,created_at:Some(record.created_at),reason:record.reason,
            execute_supported:true,estimated_visits:0,estimated_urls:0,estimated_downloads:0,
            warnings:vec!["One-click restore quarantines the current archive and installs this recorded safety snapshot.".into()],
        });
    }
    let checkpoint = checkpoint(vault, &record)?;
    let (estimated_visits, estimated_urls, estimated_downloads) =
        ingest::preview_snapshot_counts(&checkpoint)?;
    Ok(SnapshotRestorePreview {
        snapshot_path:record.file_path,snapshot_kind:"raw-source-checkpoint".into(),source_run_id:Some(record.run_id),
        source_profile_id:Some(checkpoint.profile.profile_id),source_browser_name:Some(checkpoint.profile.browser_name),created_at:Some(record.created_at),reason:record.reason,
        execute_supported:true,estimated_visits,estimated_urls,estimated_downloads,
        warnings:vec!["Snapshot restore replays the saved browser checkpoint into the current archive. Existing visible archive facts stay in place and duplicate rows are skipped.".into()],
    })
}

pub(super) fn profile_id(path: &Path, record: &super::artifacts::SnapshotRecord) -> Result<String> {
    let encoded = path
        .parent()
        .and_then(|p| p.file_name())
        .and_then(|s| s.to_str())
        .map(identifier_from_filesystem_segment);
    let profile = if record.profile_scope.len() <= 1 {
        record.profile_scope.first().cloned().or(encoded)
    } else {
        encoded.or_else(|| record.profile_scope.first().cloned())
    };
    profile.context(
        "snapshot restore requires a recorded profile scope or checkpoint directory identity",
    )
}
