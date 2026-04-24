//! Archive artifact and source-payload helpers.
//!
//! ## Responsibilities
//! - Persist and load saved snapshot artifacts referenced by archive runs.
//! - Derive stable payload hashes and schema captures for canonical ingest.
//! - Reconstruct enough browser-profile metadata to replay saved checkpoints.
//!
//! ## Not responsible for
//! - Deciding when backups or restores should run.
//! - Writing canonical archive rows or mutating watermarks.
//! - Rendering Audit, Dashboard, or Explorer read models.
//!
//! ## Dependencies
//! - `crate::chrome` profile snapshot metadata.
//! - `archive::read_models` helpers for file sizes and profile-scope decoding.
//! - Local filesystem access for snapshot directories and staged source files.
//!
//! ## Performance notes
//! - Snapshot checksums recurse over directory contents, so callers should use
//!   these helpers on bounded artifact trees rather than broad project roots.
//! - Payload serialization hashes one row at a time; it is intended for
//!   streaming ingest loops, not for cloning whole history batches.

use super::*;
use serde::Serialize;

/// Audit-facing description of one saved snapshot artifact.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SnapshotArtifact {
    kind: String,
    path: String,
    checksum: String,
    reason: String,
}

/// Snapshot metadata loaded from the archive ledger before replay or inspection.
#[derive(Debug, Clone)]
pub(crate) struct SnapshotRecord {
    pub(crate) run_id: i64,
    pub(crate) profile_scope: Vec<String>,
    pub(crate) file_path: String,
    pub(crate) created_at: String,
    pub(crate) reason: Option<String>,
}

/// Stable hash wrapper returned when one source payload has been serialized.
#[derive(Debug)]
pub(crate) struct SerializedPayload {
    pub(crate) hash: String,
}

/// Copies one staged browser snapshot into the raw-checkpoint archive.
///
/// Backup ingest uses this only after the caller has already decided a
/// checkpoint is warranted. The returned artifact is append-only audit
/// metadata; callers still own the higher-level checkpoint policy.
pub(crate) fn create_snapshot_artifact(
    archive: &Transaction<'_>,
    run_id: i64,
    paths: &ProjectPaths,
    snapshot: &ProfileSnapshot,
    reason: &str,
) -> Result<SnapshotArtifact> {
    let checkpoint_dir = paths
        .raw_snapshots_dir
        .join(&snapshot.profile.profile_id)
        .join(now_rfc3339().replace(':', "-"));
    fs::create_dir_all(&checkpoint_dir)?;

    let mut copied = Vec::<(String, String)>::new();
    let history_target = checkpoint_dir.join("History");
    fs::copy(&snapshot.history_path, &history_target)?;
    copied.push((
        history_target.display().to_string(),
        crate::utils::file_sha256_hex(&history_target)?,
    ));
    if let Some(favicons_path) = &snapshot.favicons_path {
        let target = checkpoint_dir.join("Favicons");
        fs::copy(favicons_path, &target)?;
        copied.push((target.display().to_string(), crate::utils::file_sha256_hex(&target)?));
    }

    let metadata_json = serde_json::to_string(&copied)?;
    let checksum = sha256_hex(metadata_json.as_bytes());
    let file_path = checkpoint_dir.display().to_string();
    let file_size = copied
        .iter()
        .map(|(path, _)| fs::metadata(path).map(|meta| meta.len()).unwrap_or_default())
        .sum::<u64>() as i64;

    archive.execute(
        "INSERT INTO snapshots (run_id, file_path, file_size, checksum, reason, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![run_id, file_path, file_size, checksum, reason, now_rfc3339()],
    )?;

    Ok(SnapshotArtifact {
        kind: "raw-source-checkpoint".to_string(),
        path: checkpoint_dir.display().to_string(),
        checksum,
        reason: reason.to_string(),
    })
}

/// Loads one snapshot ledger row by its recorded filesystem path.
///
/// Maintenance flows use this before previewing or replaying a checkpoint, so
/// the path lookup remains strict and returns `None` only when the ledger has
/// no matching record.
pub(crate) fn load_snapshot_record(
    connection: &Connection,
    snapshot_path: &str,
) -> Result<Option<SnapshotRecord>> {
    connection
        .query_row(
            "SELECT snapshots.run_id, runs.profile_scope_json, snapshots.file_path, snapshots.created_at, snapshots.reason
             FROM snapshots
             JOIN runs
               ON runs.id = snapshots.run_id
             WHERE snapshots.file_path = ?1
             LIMIT 1",
            [snapshot_path],
            |row| {
                Ok(SnapshotRecord {
                    run_id: row.get(0)?,
                    profile_scope: decode_profile_scope(row.get::<_, Option<String>>(1)?.as_deref()),
                    file_path: row.get(2)?,
                    created_at: row.get(3)?,
                    reason: row.get(4)?,
                })
            },
        )
        .optional()
        .map_err(Into::into)
}

/// Rebuilds a `ProfileSnapshot` view over one saved raw checkpoint directory.
///
/// Restore preview and execute flows call this after validating that the saved
/// snapshot is directory-backed rather than a full safety-copy file.
pub(crate) fn load_checkpoint_profile_snapshot(
    connection: &Connection,
    snapshot_path: &Path,
    snapshot: &SnapshotRecord,
) -> Result<ProfileSnapshot> {
    let profile_id = checkpoint_profile_id_for_snapshot(snapshot_path, snapshot)
        .context("snapshot restore requires a recorded profile scope")?;
    let history_path = snapshot_path.join("History");
    if !history_path.exists() {
        anyhow::bail!(
            "snapshot {} is not a saved browser source checkpoint",
            snapshot_path.display()
        );
    }
    let favicons_path = snapshot_path.join("Favicons");
    let profile = load_snapshot_browser_profile(
        connection,
        &profile_id,
        &history_path,
        favicons_path.exists(),
    )?;
    let source_hashes = snapshot_file_fingerprints(
        &history_path,
        favicons_path.exists().then_some(favicons_path.as_path()),
    )?;
    Ok(ProfileSnapshot {
        profile,
        temp_dir: tempdir().context("allocating restore snapshot tempdir")?,
        history_path,
        favicons_path: favicons_path.exists().then_some(favicons_path),
        source_hashes,
    })
}

/// Chooses the profile id that actually owns one saved checkpoint directory.
///
/// Multi-profile backup runs persist one snapshot row per profile but only one
/// run-level `profile_scope_json`. When that scope contains multiple profile
/// ids, the checkpoint directory name is the only stable way to decide whether
/// this artifact belongs to Firefox, Safari, or Chromium.
fn checkpoint_profile_id_for_snapshot(
    snapshot_path: &Path,
    snapshot: &SnapshotRecord,
) -> Option<String> {
    if snapshot.profile_scope.len() <= 1 {
        return snapshot
            .profile_scope
            .first()
            .cloned()
            .or_else(|| checkpoint_profile_id_from_path(snapshot_path));
    }
    checkpoint_profile_id_from_path(snapshot_path)
        .or_else(|| snapshot.profile_scope.first().cloned())
}

/// Records that a later run reused or consumed an existing snapshot artifact.
///
/// Restore and rekey flows use this instead of duplicating files when they only
/// need an audit reference to an existing artifact.
pub(crate) fn record_snapshot_reference(
    connection: &Connection,
    run_id: i64,
    path: &Path,
    reason: &str,
    created_at: &str,
) -> Result<()> {
    let (file_size, checksum) = snapshot_artifact_bytes_and_checksum(path)?;
    connection.execute(
        "INSERT INTO snapshots (run_id, file_path, file_size, checksum, reason, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            run_id,
            path.display().to_string(),
            file_size as i64,
            checksum,
            reason,
            created_at,
        ],
    )?;
    Ok(())
}

/// Captures the schema payload of one staged source database for evidence/audit.
///
/// Ingest uses this before canonical rows are written so schema observations can
/// be stored even when later row-level processing fails.
pub(crate) fn collect_schema_payload(path: &Path) -> Result<Value> {
    let connection = open_readonly_source(path)?;
    let mut statement = connection.prepare(
        "SELECT type, name, tbl_name, sql
         FROM sqlite_master
         WHERE sql IS NOT NULL
         ORDER BY type, name",
    )?;
    let rows = statement.query_map([], |row| {
        Ok(json!({
            "type": row.get::<_, String>(0)?,
            "name": row.get::<_, String>(1)?,
            "tableName": row.get::<_, String>(2)?,
            "sql": row.get::<_, String>(3)?,
        }))
    })?;
    Ok(Value::Array(rows.collect::<rusqlite::Result<Vec<_>>>()?))
}

/// Serializes one parsed source row and returns the stable hash of that payload.
///
/// Canonical ingest uses the hash for deduplication/audit fields. Callers
/// should pass one already-bounded payload at a time rather than whole
/// collections to keep memory use predictable.
pub(crate) fn serialize_payload<T: Serialize>(value: &T) -> Result<SerializedPayload> {
    let json = serde_json::to_string(value)?;
    let hash = sha256_hex(json.as_bytes());
    Ok(SerializedPayload { hash })
}

/// Derives the profile id encoded in a checkpoint directory layout.
///
/// This is a fallback for older snapshot rows whose profile scope was not
/// recorded explicitly.
fn checkpoint_profile_id_from_path(snapshot_path: &Path) -> Option<String> {
    snapshot_path.parent()?.file_name()?.to_str().map(str::to_string)
}

/// Rehydrates minimal browser-profile metadata for a saved checkpoint.
///
/// When the original `source_profiles` row is missing, this falls back to a
/// best-effort profile derived from the checkpoint path so restore preview can
/// still explain what it is replaying.
fn load_snapshot_browser_profile(
    connection: &Connection,
    profile_id: &str,
    history_path: &Path,
    has_favicons: bool,
) -> Result<crate::models::BrowserProfile> {
    let row = connection
        .query_row(
            "SELECT browser_kind, browser_version, profile_name, profile_path, user_name
             FROM source_profiles
             WHERE profile_key = ?1
             LIMIT 1",
            [profile_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, Option<String>>(4)?,
                ))
            },
        )
        .optional()?;
    let (browser_kind, browser_version, profile_name, profile_path, user_name) = row
        .unwrap_or_else(|| {
            let browser_kind = profile_id.split(':').next().unwrap_or("archive").to_string();
            (
                browser_kind.clone(),
                None,
                profile_id.to_string(),
                snapshot_path_parent_display(history_path),
                None,
            )
        });
    let history_bytes = file_size(history_path);
    let favicons_path = history_path.parent().map(|parent| parent.join("Favicons"));
    let favicons_bytes = favicons_path
        .as_ref()
        .filter(|path| path.exists())
        .map(|path| file_size(path))
        .unwrap_or_default();
    Ok(crate::models::BrowserProfile {
        profile_id: profile_id.to_string(),
        profile_name,
        browser_family: browser_family_for_profile(&browser_kind),
        browser_name: browser_name_for_profile(&browser_kind),
        user_name,
        profile_path,
        history_path: Some(history_path.display().to_string()),
        favicons_path: has_favicons.then(|| {
            history_path.parent().unwrap_or(history_path).join("Favicons").display().to_string()
        }),
        history_exists: true,
        browser_version,
        history_file_name: history_file_name_for_profile(&browser_kind),
        history_bytes,
        favicons_bytes,
        supporting_bytes: 0,
        retention_boundary: crate::browser_retention::retention_boundary_for_browser(&browser_kind),
    })
}

/// Builds a display path for the directory that owns the history database.
///
/// This keeps fallback profile reconstruction readable in Audit and restore
/// previews even when the original source-profile row is gone.
fn snapshot_path_parent_display(path: &Path) -> String {
    path.parent().unwrap_or(path).display().to_string()
}

/// Maps a concrete browser kind to the broader family PathKeep reports.
///
/// Restore preview uses the family to stay consistent with live browser
/// discovery even when the checkpoint is older than the current host state.
fn browser_family_for_profile(browser_kind: &str) -> String {
    match browser_kind {
        "firefox" | "librewolf" | "floorp" | "waterfox" => "firefox".to_string(),
        "safari" => "safari".to_string(),
        _ => "chromium".to_string(),
    }
}

/// Maps a stored browser kind to the user-visible browser name.
///
/// This is only for checkpoint explanations, so it intentionally covers the
/// browser kinds PathKeep already persists today.
fn browser_name_for_profile(browser_kind: &str) -> String {
    match browser_kind {
        "atlas" => "ChatGPT Atlas".to_string(),
        "comet" => "Perplexity Comet".to_string(),
        "edge" => "Microsoft Edge".to_string(),
        "brave" => "Brave".to_string(),
        "vivaldi" => "Vivaldi".to_string(),
        "arc" => "Arc".to_string(),
        "firefox" => "Firefox".to_string(),
        "librewolf" => "LibreWolf".to_string(),
        "floorp" => "Floorp".to_string(),
        "waterfox" => "Waterfox".to_string(),
        "safari" => "Safari".to_string(),
        _ => "Google Chrome".to_string(),
    }
}

/// Resolves the canonical history filename for one persisted browser kind.
///
/// Restore preview uses this to keep reconstructed profile metadata aligned
/// with the same browser-specific filenames shown during live discovery.
fn history_file_name_for_profile(browser_kind: &str) -> String {
    if matches!(browser_kind, "firefox" | "librewolf" | "floorp" | "waterfox") {
        "places.sqlite".to_string()
    } else if browser_kind == "safari" {
        "History.db".to_string()
    } else {
        "History".to_string()
    }
}

/// Computes content fingerprints for the files inside one checkpoint.
///
/// Restore preview uses these hashes to keep the replayed checkpoint tied to
/// the exact saved source bytes rather than only the directory name.
fn snapshot_file_fingerprints(
    history_path: &Path,
    favicons_path: Option<&Path>,
) -> Result<Vec<FileFingerprint>> {
    let mut fingerprints = vec![FileFingerprint {
        path: history_path.display().to_string(),
        sha256: file_sha256_hex(history_path)?,
    }];
    if let Some(favicons_path) = favicons_path
        && favicons_path.exists()
    {
        fingerprints.push(FileFingerprint {
            path: favicons_path.display().to_string(),
            sha256: file_sha256_hex(favicons_path)?,
        });
    }
    Ok(fingerprints)
}

/// Computes the byte size and stable checksum for one snapshot artifact path.
///
/// File-backed safety snapshots hash the file directly. Directory-backed raw
/// checkpoints hash the sorted file list plus each child checksum.
fn snapshot_artifact_bytes_and_checksum(path: &Path) -> Result<(u64, Option<String>)> {
    if !path.exists() {
        return Ok((0, None));
    }
    if path.is_file() {
        return Ok((file_size(path), Some(file_sha256_hex(path)?)));
    }
    let file_hashes = collect_path_file_hashes(path, path)?;
    let checksum = sha256_hex(serde_json::to_string(&file_hashes)?.as_bytes());
    let size = file_hashes.iter().map(|(_, _, bytes)| *bytes).sum();
    Ok((size, Some(checksum)))
}

/// Recursively collects file hashes underneath one artifact directory.
///
/// Entries are sorted by relative path before hashing so directory checksums
/// remain stable across filesystem iteration order.
fn collect_path_file_hashes(root: &Path, path: &Path) -> Result<Vec<(String, String, u64)>> {
    let mut entries = Vec::new();
    let Ok(children) = fs::read_dir(path) else {
        return Ok(entries);
    };
    for child in children.flatten() {
        let child_path = child.path();
        if child_path.is_dir() {
            entries.extend(collect_path_file_hashes(root, &child_path)?);
            continue;
        }
        let relative_path =
            child_path.strip_prefix(root).unwrap_or(&child_path).display().to_string();
        let bytes = file_size(&child_path);
        entries.push((relative_path, file_sha256_hex(&child_path)?, bytes));
    }
    entries.sort_by(|left, right| left.0.cmp(&right.0));
    Ok(entries)
}

/// Opens one staged source database in strict read-only mode.
///
/// Schema inspection and payload extraction use this helper so source files are
/// never mutated during backup/import analysis.
fn open_readonly_source(path: &Path) -> Result<Connection> {
    Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .with_context(|| format!("opening source {}", path.display()))
}
