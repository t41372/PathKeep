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

/// Identifies one fallible filesystem operation in the recovery-snapshot inventory.
///
/// The value is only observed by the test-only fault seam below. Keeping the seam at the I/O
/// boundary lets tests reproduce iterator errors and delete races deterministically while release
/// builds call `std::fs` directly.
#[cfg(test)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SnapshotInventoryFaultSite {
    LegacyBucketReadDir,
    RootEntry,
    BucketMetadata,
    BucketReadDir,
    SnapshotEntry,
}

#[cfg(test)]
thread_local! {
    static SNAPSHOT_INVENTORY_FAULT:
        std::cell::Cell<Option<(SnapshotInventoryFaultSite, std::io::ErrorKind)>> =
        const { std::cell::Cell::new(None) };
}

#[cfg(test)]
fn inject_snapshot_inventory_fault(site: SnapshotInventoryFaultSite, kind: std::io::ErrorKind) {
    SNAPSHOT_INVENTORY_FAULT.with(|fault| fault.set(Some((site, kind))));
}

#[cfg(test)]
fn take_snapshot_inventory_fault(site: SnapshotInventoryFaultSite) -> Option<std::io::Error> {
    SNAPSHOT_INVENTORY_FAULT.with(|fault| {
        let armed = fault.get();
        if armed.is_some_and(|(armed_site, _)| armed_site == site) {
            fault.set(None);
            armed.map(|(_, kind)| std::io::Error::new(kind, "injected snapshot inventory fault"))
        } else {
            None
        }
    })
}

#[cfg(test)]
fn snapshot_read_dir(
    path: &Path,
    site: SnapshotInventoryFaultSite,
) -> std::io::Result<fs::ReadDir> {
    if let Some(error) = take_snapshot_inventory_fault(site) {
        return Err(error);
    }
    fs::read_dir(path)
}

#[cfg(test)]
fn snapshot_metadata(
    path: &Path,
    site: SnapshotInventoryFaultSite,
) -> std::io::Result<fs::Metadata> {
    if let Some(error) = take_snapshot_inventory_fault(site) {
        return Err(error);
    }
    fs::metadata(path)
}

#[cfg(test)]
fn next_snapshot_entry(
    entries: &mut fs::ReadDir,
    site: SnapshotInventoryFaultSite,
) -> Option<std::io::Result<fs::DirEntry>> {
    if let Some(error) = take_snapshot_inventory_fault(site) {
        return Some(Err(error));
    }
    entries.next()
}

/// Lists the verified rekey safety-snapshot files (`raw-snapshots/rekey/*.sqlite`) a Phase-D
/// restore could offer. Cheap: a directory listing, never a DB open. Sorted for a stable,
/// newest-last presentation. Missing is empty; an incomplete listing is an error.
pub(super) fn available_verified_snapshots(paths: &ProjectPaths) -> Result<Vec<String>> {
    let rekey_dir = paths.raw_snapshots_dir.join("rekey");
    #[cfg(test)]
    let entries = snapshot_read_dir(&rekey_dir, SnapshotInventoryFaultSite::LegacyBucketReadDir);
    #[cfg(not(test))]
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
        #[cfg(test)]
        let bucket = next_snapshot_entry(&mut buckets, SnapshotInventoryFaultSite::RootEntry);
        #[cfg(not(test))]
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
        #[cfg(test)]
        let bucket_metadata =
            snapshot_metadata(&bucket_path, SnapshotInventoryFaultSite::BucketMetadata);
        #[cfg(not(test))]
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
        #[cfg(test)]
        let entries = snapshot_read_dir(&bucket_path, SnapshotInventoryFaultSite::BucketReadDir);
        #[cfg(not(test))]
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
            #[cfg(test)]
            let entry =
                next_snapshot_entry(&mut entries, SnapshotInventoryFaultSite::SnapshotEntry);
            #[cfg(not(test))]
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
    use std::time::Duration;
    use tempfile::tempdir;
    use vault_core::project_paths_with_root;

    /// Seeds a tiny but REAL plaintext SQLite database at `path` (so the keyless openability probe
    /// passes), creating parent dirs as needed.
    fn seed_plaintext_snapshot(path: &Path) {
        fs::create_dir_all(path.parent().expect("snapshot parent")).expect("snapshot dir");
        let connection = Connection::open(path).expect("open snapshot db");
        connection
            .execute_batch("CREATE TABLE t(a); INSERT INTO t VALUES (1);")
            .expect("seed snapshot db");
    }

    /// Pins `path`'s mtime to a fixed instant so newest-first ordering is deterministic.
    fn set_snapshot_mtime(path: &Path, secs: u64) {
        let file = fs::File::options().write(true).open(path).expect("open for mtime");
        file.set_modified(std::time::SystemTime::UNIX_EPOCH + Duration::from_secs(secs))
            .expect("set mtime");
    }

    #[test]
    fn list_legacy_snapshots_reports_validity_and_newest_first() {
        let dir = tempdir().expect("tempdir");
        let paths = project_paths_with_root(dir.path());
        let rekey_dir = paths.raw_snapshots_dir.join("rekey");

        // A valid plaintext snapshot (real db) and a corrupt one (SQLite header + garbage body so it
        // is detected Plaintext but `PRAGMA schema_version` errors).
        let valid = rekey_dir.join("archive-before-rekey-valid.sqlite");
        seed_plaintext_snapshot(&valid);
        let corrupt = rekey_dir.join("archive-before-rekey-corrupt.sqlite");
        fs::write(&corrupt, b"SQLite format 3\0\x01\x02\x03corrupt-body").expect("write corrupt");

        // Pin mtimes so newest-first ordering is deterministic: the valid one is newer.
        set_snapshot_mtime(&corrupt, 1_000_000_000);
        set_snapshot_mtime(&valid, 2_000_000_000);

        let snapshots = list_legacy_snapshots(&paths).expect("list snapshots");
        assert_eq!(snapshots.len(), 2, "both snapshots are listed");
        assert_eq!(snapshots[0].path, valid.display().to_string(), "newest (valid) leads");
        assert_eq!(snapshots[1].path, corrupt.display().to_string());
        assert!(snapshots[0].verified_openable, "a real plaintext db opens");
        assert!(!snapshots[1].verified_openable, "a corrupt plaintext file does not open");
        for snapshot in &snapshots {
            assert_eq!(snapshot.source_op, "rekey");
            assert_eq!(snapshot.id, snapshot.path, "id == path");
            assert!(snapshot.size_bytes > 0, "size is read from metadata");
            assert!(snapshot.created_at.is_some(), "created_at comes from mtime");
            assert!(snapshot.label.contains("rekey"), "label encodes the op");
        }
    }

    #[test]
    fn list_legacy_snapshots_classifies_encrypted_unknown_and_skips_noise() {
        let dir = tempdir().expect("tempdir");
        let paths = project_paths_with_root(dir.path());

        // Encrypted-looking (non-SQLite header) >= 512 bytes under import/ -> structurally openable.
        let import_dir = paths.raw_snapshots_dir.join("import");
        fs::create_dir_all(&import_dir).expect("import dir");
        fs::write(import_dir.join("archive-before-import-big.sqlite"), vec![0xAB; 1024])
            .expect("write encrypted snapshot");

        // A tiny (< 512B) encrypted-looking file under rekey/ -> structurally NOT openable.
        let rekey_dir = paths.raw_snapshots_dir.join("rekey");
        fs::create_dir_all(&rekey_dir).expect("rekey dir");
        fs::write(rekey_dir.join("archive-before-rekey-tiny.sqlite"), vec![0xCD; 100])
            .expect("write tiny snapshot");

        // A snapshot under an UNKNOWN bucket -> source_op "unknown".
        seed_plaintext_snapshot(&paths.raw_snapshots_dir.join("mystery").join("whatever.sqlite"));

        // Noise that MUST be ignored: a non-.sqlite file in a bucket, and a plain file directly
        // under raw-snapshots/ (not a bucket directory).
        fs::write(import_dir.join("notes.txt"), b"ignore me").expect("write non-sqlite");
        fs::write(paths.raw_snapshots_dir.join("loose.sqlite"), b"loose")
            .expect("write loose file");

        let snapshots = list_legacy_snapshots(&paths).expect("list snapshots");
        assert_eq!(snapshots.len(), 3, "only the three bucketed .sqlite files are listed");
        let find = |needle: &str| {
            snapshots.iter().find(|s| s.path.contains(needle)).expect("snapshot present").clone()
        };
        let big = find("big.sqlite");
        assert_eq!(big.source_op, "import");
        assert!(big.verified_openable, ">= 512B encrypted is structurally openable");
        assert!(big.encrypted, "a 0xAB header is a SQLCipher salt, not the SQLite magic");
        assert!(!find("tiny.sqlite").verified_openable, "< 512B encrypted is not openable");
        assert_eq!(find("whatever.sqlite").source_op, "unknown");
        assert!(
            !find("whatever.sqlite").encrypted,
            "a seeded plaintext snapshot is not flagged encrypted",
        );
    }

    #[test]
    fn list_legacy_snapshots_empty_for_missing_and_empty_dirs() {
        let dir = tempdir().expect("tempdir");
        let paths = project_paths_with_root(dir.path());
        assert!(
            list_legacy_snapshots(&paths).expect("missing inventory is valid").is_empty(),
            "missing dir -> empty"
        );
        fs::create_dir_all(&paths.raw_snapshots_dir).expect("raw snapshots dir");
        assert!(
            list_legacy_snapshots(&paths).expect("empty inventory is valid").is_empty(),
            "empty dir -> empty"
        );
    }

    #[test]
    fn list_legacy_snapshots_surfaces_a_non_not_found_root_read_error() {
        let dir = tempdir().expect("tempdir");
        let paths = project_paths_with_root(dir.path());
        fs::create_dir_all(
            paths.raw_snapshots_dir.parent().expect("raw snapshots parent directory"),
        )
        .expect("create parent");
        fs::write(&paths.raw_snapshots_dir, b"not a directory").expect("replace root with file");

        let error =
            list_legacy_snapshots(&paths).expect_err("a non-directory inventory root must fail");
        assert!(
            format!("{error:#}").contains("reading recovery snapshot inventory"),
            "the error identifies the incomplete inventory: {error:#}"
        );
    }

    #[test]
    fn available_verified_snapshots_surfaces_a_non_not_found_read_error() {
        let dir = tempdir().expect("tempdir");
        let paths = project_paths_with_root(dir.path());
        inject_snapshot_inventory_fault(
            SnapshotInventoryFaultSite::LegacyBucketReadDir,
            std::io::ErrorKind::PermissionDenied,
        );

        let error = available_verified_snapshots(&paths)
            .expect_err("an unreadable legacy rekey bucket must fail");
        assert!(
            format!("{error:#}").contains("reading recovery snapshot bucket"),
            "the error identifies the unreadable bucket: {error:#}"
        );
    }

    #[test]
    fn list_legacy_snapshots_surfaces_a_root_iterator_error() {
        let dir = tempdir().expect("tempdir");
        let paths = project_paths_with_root(dir.path());
        fs::create_dir_all(&paths.raw_snapshots_dir).expect("raw snapshots dir");
        inject_snapshot_inventory_fault(
            SnapshotInventoryFaultSite::RootEntry,
            std::io::ErrorKind::PermissionDenied,
        );

        let error =
            list_legacy_snapshots(&paths).expect_err("a root iterator error must fail inventory");
        assert!(
            format!("{error:#}").contains("reading a recovery snapshot bucket entry"),
            "the error identifies the incomplete root iteration: {error:#}"
        );
    }

    #[test]
    fn list_legacy_snapshots_skips_a_bucket_deleted_before_metadata() {
        let dir = tempdir().expect("tempdir");
        let paths = project_paths_with_root(dir.path());
        fs::create_dir_all(paths.raw_snapshots_dir.join("rekey")).expect("rekey bucket");
        inject_snapshot_inventory_fault(
            SnapshotInventoryFaultSite::BucketMetadata,
            std::io::ErrorKind::NotFound,
        );

        assert!(
            list_legacy_snapshots(&paths)
                .expect("a concurrently deleted bucket is skipped")
                .is_empty()
        );
    }

    #[test]
    fn list_legacy_snapshots_surfaces_a_bucket_metadata_error() {
        let dir = tempdir().expect("tempdir");
        let paths = project_paths_with_root(dir.path());
        fs::create_dir_all(paths.raw_snapshots_dir.join("rekey")).expect("rekey bucket");
        inject_snapshot_inventory_fault(
            SnapshotInventoryFaultSite::BucketMetadata,
            std::io::ErrorKind::PermissionDenied,
        );

        let error =
            list_legacy_snapshots(&paths).expect_err("a bucket metadata error must fail inventory");
        assert!(
            format!("{error:#}").contains("reading recovery snapshot bucket metadata"),
            "the error identifies the unreadable bucket metadata: {error:#}"
        );
    }

    #[test]
    fn list_legacy_snapshots_skips_a_bucket_deleted_before_read_dir() {
        let dir = tempdir().expect("tempdir");
        let paths = project_paths_with_root(dir.path());
        fs::create_dir_all(paths.raw_snapshots_dir.join("rekey")).expect("rekey bucket");
        inject_snapshot_inventory_fault(
            SnapshotInventoryFaultSite::BucketReadDir,
            std::io::ErrorKind::NotFound,
        );

        assert!(
            list_legacy_snapshots(&paths)
                .expect("a bucket deleted before scanning is skipped")
                .is_empty()
        );
    }

    #[test]
    fn list_legacy_snapshots_surfaces_a_bucket_read_error() {
        let dir = tempdir().expect("tempdir");
        let paths = project_paths_with_root(dir.path());
        fs::create_dir_all(paths.raw_snapshots_dir.join("rekey")).expect("rekey bucket");
        inject_snapshot_inventory_fault(
            SnapshotInventoryFaultSite::BucketReadDir,
            std::io::ErrorKind::PermissionDenied,
        );

        let error =
            list_legacy_snapshots(&paths).expect_err("an unreadable bucket must fail inventory");
        assert!(
            format!("{error:#}").contains("reading recovery snapshot bucket"),
            "the error identifies the unreadable bucket: {error:#}"
        );
    }

    #[test]
    fn list_legacy_snapshots_surfaces_a_snapshot_iterator_error() {
        let dir = tempdir().expect("tempdir");
        let paths = project_paths_with_root(dir.path());
        fs::create_dir_all(paths.raw_snapshots_dir.join("rekey")).expect("rekey bucket");
        inject_snapshot_inventory_fault(
            SnapshotInventoryFaultSite::SnapshotEntry,
            std::io::ErrorKind::PermissionDenied,
        );

        let error = list_legacy_snapshots(&paths)
            .expect_err("a snapshot iterator error must fail inventory");
        assert!(
            format!("{error:#}").contains("reading an entry in recovery snapshot bucket"),
            "the error identifies the incomplete bucket iteration: {error:#}"
        );
    }

    #[test]
    fn list_legacy_snapshots_marks_a_sub_header_sized_file_unopenable() {
        // A `.sqlite` smaller than the 16-byte header makes `detect_disk_encryption_mode` read fewer
        // than 16 bytes and classify it `Absent`, driving the `Absent` arm of
        // `snapshot_is_openable_keyless` (verified_openable == false) — a truncated/empty snapshot
        // file is never a usable backstop.
        let dir = tempdir().expect("tempdir");
        let paths = project_paths_with_root(dir.path());
        let rekey_dir = paths.raw_snapshots_dir.join("rekey");
        fs::create_dir_all(&rekey_dir).expect("rekey dir");
        fs::write(rekey_dir.join("tiny.sqlite"), b"abcde").expect("write 5-byte snapshot");

        let snapshots = list_legacy_snapshots(&paths).expect("list snapshots");
        assert_eq!(snapshots.len(), 1, "the sub-header file is still listed");
        assert!(
            !snapshots[0].verified_openable,
            "a <16-byte (Absent) file is never verified_openable",
        );
    }

    #[cfg(unix)]
    #[test]
    fn list_legacy_snapshots_skips_a_dangling_symlink() {
        let dir = tempdir().expect("tempdir");
        let paths = project_paths_with_root(dir.path());
        let rekey_dir = paths.raw_snapshots_dir.join("rekey");
        fs::create_dir_all(&rekey_dir).expect("rekey dir");
        // A .sqlite symlink whose target does not exist: it passes the extension filter, then
        // `fs::metadata` (which follows links) errors, so the entry is skipped.
        std::os::unix::fs::symlink(
            rekey_dir.join("missing-target.sqlite"),
            rekey_dir.join("dangling.sqlite"),
        )
        .expect("create dangling symlink");
        assert!(
            list_legacy_snapshots(&paths).expect("list snapshots").is_empty(),
            "a dangling snapshot symlink contributes nothing",
        );
    }

    #[cfg(unix)]
    #[test]
    fn list_legacy_snapshots_surfaces_a_non_not_found_metadata_error() {
        let dir = tempdir().expect("tempdir");
        let paths = project_paths_with_root(dir.path());
        let rekey_dir = paths.raw_snapshots_dir.join("rekey");
        fs::create_dir_all(&rekey_dir).expect("rekey dir");
        let loop_path = rekey_dir.join("metadata-loop.sqlite");
        std::os::unix::fs::symlink("metadata-loop.sqlite", &loop_path)
            .expect("create self-referential symlink");

        let error =
            list_legacy_snapshots(&paths).expect_err("a metadata I/O error must fail inventory");
        assert!(
            format!("{error:#}").contains("reading recovery snapshot metadata"),
            "the error identifies the unreadable snapshot: {error:#}"
        );
    }
}
