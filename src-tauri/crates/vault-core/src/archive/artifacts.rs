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
    pub(crate) kind: String,
    pub(crate) path: String,
    pub(crate) checksum: String,
    pub(crate) reason: String,
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

type ProfileFields = (String, Option<String>, String, String, Option<String>);

/// Reconstructs a recorded source, even when the live discovery row no longer exists.
pub(super) fn snapshot_browser_profile(
    profile_id: &str,
    history_path: &Path,
    has_favicons: bool,
    row: Option<ProfileFields>,
) -> crate::models::BrowserProfile {
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
    crate::models::BrowserProfile {
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
        history_readable: true,
        access_issue: None,
        browser_version,
        history_file_name: history_file_name_for_profile(&browser_kind),
        history_bytes,
        favicons_bytes,
        supporting_bytes: 0,
        retention_boundary: crate::browser_retention::retention_boundary_for_browser(&browser_kind),
    }
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
pub(super) fn snapshot_file_fingerprints(
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
pub(super) fn snapshot_artifact_bytes_and_checksum(path: &Path) -> Result<(u64, Option<String>)> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn checkpoint_profile_reconstruction_covers_missing_source_rows_and_artifact_edges() {
        let vault = crate::Vault::open_temp().unwrap();
        let root = tempdir().expect("tempdir");
        let checkpoint = root.path().join("checkpoint");
        fs::create_dir_all(checkpoint.join("nested")).expect("checkpoint dirs");
        fs::write(checkpoint.join("History"), "history").expect("history");
        fs::write(checkpoint.join("Favicons"), "favicons").expect("favicons");
        fs::write(checkpoint.join("nested").join("child.txt"), "child").expect("child");

        let profile =
            snapshot_browser_profile("firefox:default", &checkpoint.join("History"), true, None);
        assert_eq!(profile.browser_family, "firefox");
        assert_eq!(profile.browser_name, "Firefox");
        assert_eq!(profile.history_file_name, "places.sqlite");
        assert!(profile.favicons_path.expect("favicons path").ends_with("Favicons"));

        let fingerprints = snapshot_file_fingerprints(
            &checkpoint.join("History"),
            Some(&checkpoint.join("Favicons")),
        )
        .expect("fingerprints");
        assert_eq!(fingerprints.len(), 2);

        let (missing_size, missing_checksum) =
            snapshot_artifact_bytes_and_checksum(&checkpoint.join("missing")).expect("missing");
        assert_eq!((missing_size, missing_checksum), (0, None));
        let (file_size_bytes, file_checksum) =
            snapshot_artifact_bytes_and_checksum(&checkpoint.join("History")).expect("file hash");
        assert_eq!(file_size_bytes, 7);
        assert!(file_checksum.is_some());
        let (dir_size_bytes, dir_checksum) =
            snapshot_artifact_bytes_and_checksum(&checkpoint).expect("dir hash");
        assert!(dir_size_bytes >= file_size_bytes);
        assert!(dir_checksum.is_some());
        assert!(
            collect_path_file_hashes(&checkpoint, &checkpoint.join("History"))
                .expect("file as dir")
                .is_empty()
        );
        assert_eq!(snapshot_path_parent_display(Path::new("History")), "");

        let broken_snapshot = SnapshotRecord {
            run_id: 1,
            profile_scope: vec!["chrome:Default".to_string()],
            file_path: root.path().join("missing-history-checkpoint").display().to_string(),
            created_at: "2026-01-01T00:00:00Z".to_string(),
            reason: Some("test".to_string()),
        };
        let missing_history = root.path().join("missing-history-checkpoint");
        fs::create_dir_all(&missing_history).expect("missing history dir");
        let error = super::super::checkpoint_restore::checkpoint(&vault, &broken_snapshot)
            .expect_err("missing history should fail");
        assert!(format!("{error:#}").contains("saved browser source checkpoint"));

        let encoded_checkpoint = root.path().join("firefox%3Adefault-release").join("2026");
        let encoded_snapshot = SnapshotRecord {
            run_id: 2,
            profile_scope: vec![
                "firefox:default-release".to_string(),
                "chrome:Default".to_string(),
            ],
            file_path: encoded_checkpoint.display().to_string(),
            created_at: "2026-01-01T00:00:00Z".to_string(),
            reason: Some("test".to_string()),
        };
        assert_eq!(
            super::super::checkpoint_restore::profile_id(&encoded_checkpoint, &encoded_snapshot)
                .unwrap(),
            "firefox:default-release"
        );
    }

    #[test]
    fn checkpoint_profile_scope_keeps_the_legacy_directory_fallback() {
        let path = Path::new("/snapshots/firefox%3Adefault/2026");
        let mut record = SnapshotRecord {
            run_id: 1,
            profile_scope: Vec::new(),
            file_path: path.display().to_string(),
            created_at: "2026".into(),
            reason: None,
        };
        assert_eq!(
            super::super::checkpoint_restore::profile_id(path, &record).unwrap(),
            "firefox:default"
        );
        record.profile_scope = vec!["chrome:recorded".into()];
        assert_eq!(
            super::super::checkpoint_restore::profile_id(path, &record).unwrap(),
            "chrome:recorded"
        );
    }

    #[test]
    fn checkpoint_browser_name_helpers_cover_supported_kinds() {
        for (kind, family, name, filename) in [
            ("atlas", "chromium", "ChatGPT Atlas", "History"),
            ("comet", "chromium", "Perplexity Comet", "History"),
            ("edge", "chromium", "Microsoft Edge", "History"),
            ("brave", "chromium", "Brave", "History"),
            ("vivaldi", "chromium", "Vivaldi", "History"),
            ("arc", "chromium", "Arc", "History"),
            ("firefox", "firefox", "Firefox", "places.sqlite"),
            ("librewolf", "firefox", "LibreWolf", "places.sqlite"),
            ("floorp", "firefox", "Floorp", "places.sqlite"),
            ("waterfox", "firefox", "Waterfox", "places.sqlite"),
            ("safari", "safari", "Safari", "History.db"),
            ("chrome", "chromium", "Google Chrome", "History"),
        ] {
            assert_eq!(browser_family_for_profile(kind), family);
            assert_eq!(browser_name_for_profile(kind), name);
            assert_eq!(history_file_name_for_profile(kind), filename);
        }
    }
}
