//! Responsibilities: keyless inventory of the old SQLCipher format's safety snapshots.
//! Not responsible for: native snapshots (`vault_core::maintenance`), restoring a snapshot
//! (`crate::restore_snapshot`) or pruning them.
//! Failure modes: an I/O error read as "no snapshots", a dangling symlink or a sub-header file
//! offered as a backstop, a read-only plaintext snapshot mislabelled as unopenable.
//! Tests: legacy_recovery::snapshots::tests.
use anyhow::{Context, Result};
use rusqlite::{Connection, OpenFlags};
use std::{fs, path::Path, time::Duration as StdDuration};
use vault_core::{DiskEncryptionMode, ProjectPaths, RecoverySnapshot, detect_disk_encryption_mode};

const BUSY_TIMEOUT: StdDuration = StdDuration::from_secs(5);

/// Lists the verified rekey safety-snapshot files (`raw-snapshots/rekey/*.sqlite`) a Phase-D
/// restore could offer. Cheap: a directory listing, never a DB open. Sorted for a stable,
/// newest-last presentation. Missing is empty; an incomplete listing is an error.
pub(super) fn available_verified_snapshots(paths: &ProjectPaths) -> Result<Vec<String>> {
    let rekey_dir = paths.raw_snapshots_dir.join("rekey");
    let entries = fs::read_dir(&rekey_dir);
    let entries = match entries {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => {
            return Err(error).with_context(|| {
                format!("reading recovery snapshot bucket {}", rekey_dir.display())
            });
        }
    };
    let mut snapshots = Vec::new();
    for entry in entries {
        let entry =
            entry.with_context(|| format!("reading an entry in {}", rekey_dir.display()))?;
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) == Some("sqlite") {
            snapshots.push(path.display().to_string());
        }
    }
    snapshots.sort();
    Ok(snapshots)
}

/// Lists the old-format full-archive safety snapshots (`raw-snapshots/<op>/*.sqlite`) the
/// recovery screen can offer, with KEYLESS metadata (capture time, size, a cheap "does it open?"
/// signal, which old rewrite op produced it).
///
/// PERFORMANCE: a directory scan, then per file a 16-byte header read and — for plaintext files
/// ONLY — a page-1 `PRAGMA schema_version` (never a b-tree walk). Encrypted snapshots get a
/// structural size-only check because we hold no key here; the restore verifies them with the
/// key. Sorted NEWEST FIRST. A missing `raw-snapshots/` yields an empty list; every other
/// directory-entry or metadata I/O failure is returned, so an incomplete inventory is never
/// mistaken for "no snapshots".
pub fn list_legacy_snapshots(paths: &ProjectPaths) -> Result<Vec<RecoverySnapshot>> {
    /// Bucket subdirectory names mapped to a known `source_op`; anything else is `"unknown"`.
    const KNOWN_OPS: [&str; 4] = ["rekey", "reconcile", "import", "periodic"];

    let mut snapshots: Vec<RecoverySnapshot> = Vec::new();
    let buckets = fs::read_dir(&paths.raw_snapshots_dir);
    let mut buckets = match buckets {
        Ok(buckets) => buckets,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(snapshots),
        Err(error) => {
            return Err(error).with_context(|| {
                format!("reading recovery snapshot inventory {}", paths.raw_snapshots_dir.display())
            });
        }
    };
    loop {
        let bucket = buckets.next();
        let Some(bucket) = bucket else {
            break;
        };
        let bucket = bucket.with_context(|| {
            format!(
                "reading a recovery snapshot bucket entry in {}",
                paths.raw_snapshots_dir.display()
            )
        })?;
        let bucket_path = bucket.path();
        let bucket_metadata = fs::metadata(&bucket_path);
        let bucket_metadata = match bucket_metadata {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => {
                return Err(error).with_context(|| {
                    format!("reading recovery snapshot bucket metadata {}", bucket_path.display())
                });
            }
        };
        if !bucket_metadata.is_dir() {
            continue;
        }
        let source_op = bucket_path
            .file_name()
            .and_then(|name| name.to_str())
            .filter(|name| KNOWN_OPS.contains(name))
            .map(str::to_string)
            .unwrap_or_else(|| "unknown".to_string());
        let entries = fs::read_dir(&bucket_path);
        let mut entries = match entries {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => {
                return Err(error).with_context(|| {
                    format!("reading recovery snapshot bucket {}", bucket_path.display())
                });
            }
        };
        loop {
            let entry = entries.next();
            let Some(entry) = entry else {
                break;
            };
            let entry = entry.with_context(|| {
                format!("reading an entry in recovery snapshot bucket {}", bucket_path.display())
            })?;
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) != Some("sqlite") {
                continue;
            }
            // `fs::metadata` follows symlinks, so a dangling snapshot symlink errors here and is
            // skipped (rather than surfacing a bogus zero-size entry).
            let metadata = match fs::metadata(&path) {
                Ok(metadata) => metadata,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
                Err(error) => {
                    return Err(error).with_context(|| {
                        format!("reading recovery snapshot metadata {}", path.display())
                    });
                }
            };
            let size_bytes = metadata.len();
            let created_at = metadata
                .modified()
                .ok()
                .map(|mtime| chrono::DateTime::<chrono::Utc>::from(mtime).to_rfc3339());
            let path_string = path.display().to_string();
            let disk_mode = detect_disk_encryption_mode(&path);
            snapshots.push(RecoverySnapshot {
                id: path_string.clone(),
                verified_openable: snapshot_is_openable_keyless(disk_mode, &path, size_bytes),
                encrypted: matches!(disk_mode, DiskEncryptionMode::Encrypted),
                // Short, stable English fallback. The FE localizes from `source_op` + `created_at`.
                label: format!("Safety snapshot ({source_op})"),
                source_op: source_op.clone(),
                size_bytes,
                created_at,
                path: path_string,
            });
        }
    }
    // Newest first (created_at desc), path desc as a stable tiebreak so equal mtimes still order
    // deterministically. `None` mtimes sort last under the descending compare.
    snapshots.sort_by(|a, b| b.created_at.cmp(&a.created_at).then_with(|| b.path.cmp(&a.path)));
    Ok(snapshots)
}

/// KEYLESS "does this snapshot open?" probe. Plaintext: a page-1 `PRAGMA schema_version` (header
/// only — it does NOT walk the b-tree, so a 14.4M-row file is as cheap as a tiny one). Encrypted:
/// structural-only (`>= 512` bytes = at least one small page) because we hold no key to decrypt
/// it here; the authoritative keyed `quick_check` runs at restore time. `Absent`/unreadable header
/// is never openable.
fn snapshot_is_openable_keyless(mode: DiskEncryptionMode, path: &Path, size_bytes: u64) -> bool {
    match mode {
        DiskEncryptionMode::Absent => false,
        DiskEncryptionMode::Encrypted => size_bytes >= 512,
        DiskEncryptionMode::Plaintext => {
            // READ-ONLY open: the default RW+CREATE flags would mislabel a valid-but-read-only
            // plaintext snapshot as un-openable, which both hides a good backstop from the recovery
            // GUI AND lets `prune_snapshot_bucket` compute `protected = None` and delete the
            // last-good snapshot. Snapshots are checkpoint-TRUNCATE'd, so a read-only open needs no
            // WAL. A single fallible chain (open -> busy_timeout -> header-only `schema_version`)
            // folds every failure mode into one `is_ok()` — a malformed/truncated/unreadable file
            // surfaces as `false` here without an extra, list-unreachable early-return branch.
            Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)
                .and_then(|connection| {
                    connection.busy_timeout(BUSY_TIMEOUT)?;
                    connection.query_row("PRAGMA schema_version", [], |row| row.get::<_, i64>(0))
                })
                .is_ok()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use vault_core::project_paths_with_root;

    #[test]
    fn list_legacy_snapshots_surfaces_a_non_not_found_root_read_error() {
        let dir = tempdir().expect("tempdir");
        let paths = project_paths_with_root(dir.path());
        fs::create_dir_all(
            paths.raw_snapshots_dir.parent().expect("raw snapshots parent directory"),
        )
        .expect("create parent");
        fs::write(&paths.raw_snapshots_dir, b"not a directory").expect("replace root with file");

        list_legacy_snapshots(&paths).expect_err("a non-directory inventory root must fail");
    }
}
