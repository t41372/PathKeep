//! Failure modes: snapshot provenance lost, missing source files, inconsistent artifact hashes.
//! Tests: artifact reconstruction tests and checkpoint_restore suites.
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

type ProfileFields = (String, Option<String>, String, String, Option<String>);

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
