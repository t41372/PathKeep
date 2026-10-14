//! Test fixtures: old-format archive files and the on-disk states a crash in the old app left,
//! built directly. Nothing here goes through an old flow; those are gone.
use rusqlite::Connection;
use std::{
    fs,
    path::{Path, PathBuf},
};
use vault_core::{
    AppConfig, ArchiveMode, DiskEncryptionMode, ProjectPaths, detect_disk_encryption_mode,
    load_config, save_config,
};

pub(super) const KEY: &str = "legacy-recovery-test-key";
pub(super) const CRASH_TIMESTAMP: &str = "2026-06-30T00-00-00Z";

pub(super) fn plaintext_config() -> AppConfig {
    AppConfig { archive_mode: ArchiveMode::Plaintext, initialized: true, ..AppConfig::default() }
}

pub(super) fn encrypted_config() -> AppConfig {
    AppConfig { archive_mode: ArchiveMode::Encrypted, initialized: true, ..AppConfig::default() }
}

/// A real SQLite file (SQLCipher-encrypted when `key` is set) with one sentinel `backup` run,
/// in rollback-journal mode so the file alone holds every row.
pub(super) fn legacy_db(path: &Path, key: Option<&str>) {
    fs::create_dir_all(path.parent().expect("parent")).expect("parent dir");
    let connection = Connection::open(path).expect("open legacy db");
    if let Some(key) = key {
        connection.pragma_update(None, "key", key).expect("key");
    }
    connection
        .execute_batch(
            "PRAGMA journal_mode=DELETE;
             CREATE TABLE runs(id INTEGER PRIMARY KEY, run_type TEXT NOT NULL);
             INSERT INTO runs(run_type) VALUES ('backup');",
        )
        .expect("seed legacy db");
}

/// Sentinel runs in `path`: proves a restored file carries the original rows.
pub(super) fn sentinel_runs(path: &Path, key: Option<&str>) -> i64 {
    let connection = Connection::open(path).expect("open legacy db");
    if let Some(key) = key {
        connection.pragma_update(None, "key", key).expect("key");
    }
    connection
        .query_row("SELECT COUNT(*) FROM runs WHERE run_type='backup'", [], |row| row.get(0))
        .expect("count sentinel runs")
}

pub(super) fn quick_check(path: &Path, key: Option<&str>) -> String {
    let connection = Connection::open(path).expect("open legacy db");
    if let Some(key) = key {
        connection.pragma_update(None, "key", key).expect("key");
    }
    connection.query_row("PRAGMA quick_check", [], |row| row.get(0)).expect("quick_check")
}

/// A file whose header reads as plaintext, without a database behind it.
pub(super) fn plaintext_header(path: &Path) {
    fs::create_dir_all(path.parent().expect("parent")).expect("parent dir");
    fs::write(path, b"SQLite format 3\0plaintext body").expect("write plaintext header");
}

/// A file whose header reads as encrypted.
pub(super) fn encrypted_header(path: &Path) {
    fs::create_dir_all(path.parent().expect("parent")).expect("parent dir");
    fs::write(path, [7u8; 32]).expect("write encrypted header");
}

pub(super) fn marker(paths: &ProjectPaths, name: &str) -> PathBuf {
    paths.legacy.archive.parent().expect("archive parent").join(name)
}

/// The state a rekey cut between swap and config left: the canonical file already at
/// `to`, config still at `from`, and the rekey journal.
pub(super) fn crash_mid_rekey(paths: &ProjectPaths, from: ArchiveMode, to: ArchiveMode) {
    let from_config = AppConfig { archive_mode: from.clone(), ..plaintext_config() };
    save_config(paths, &from_config).expect("save config");
    match to {
        ArchiveMode::Plaintext => legacy_db(&paths.legacy.archive, None),
        ArchiveMode::Encrypted => legacy_db(&paths.legacy.archive, Some(KEY)),
    }
    let journal = serde_json::json!({
        "version": 1,
        "timestamp": CRASH_TIMESTAMP,
        "from_mode": from,
        "to_mode": to,
        "snapshot_ref": paths.raw_snapshots_dir.join("rekey/before.sqlite").display().to_string(),
        "previous_config": serde_json::to_string(&from_config).unwrap(),
    });
    fs::write(marker(paths, ".pk-rekey-journal.json"), serde_json::to_vec(&journal).unwrap())
        .expect("write rekey journal");
}

/// The state a full restore cut inside its install window left: the canonical archive moved
/// into a dated quarantine folder, the journal written, the chosen snapshot still in place.
/// Returns the snapshot path.
pub(super) fn crash_mid_restore(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
) -> PathBuf {
    save_config(paths, config).expect("save config");
    legacy_db(&paths.legacy.archive, key);
    let snapshot = paths.raw_snapshots_dir.join("rekey").join("snap.sqlite");
    fs::create_dir_all(snapshot.parent().unwrap()).expect("snapshot dir");
    fs::copy(&paths.legacy.archive, &snapshot).expect("copy snapshot");
    let quarantine = paths.quarantine_dir.join(CRASH_TIMESTAMP);
    let journal = serde_json::json!({
        "version": 1,
        "timestamp": CRASH_TIMESTAMP,
        "snapshot_ref": snapshot.display().to_string(),
        "restored_mode": config.archive_mode,
        "quarantine_dir": quarantine.display().to_string(),
        "previous_config": fs::read_to_string(&paths.config_path).ok(),
    });
    fs::write(marker(paths, ".pk-restore-journal.json"), serde_json::to_vec(&journal).unwrap())
        .expect("write restore journal");
    fs::create_dir_all(&quarantine).expect("quarantine dir");
    fs::rename(&paths.legacy.archive, quarantine.join("history-vault.sqlite"))
        .expect("quarantine the canonical archive");
    snapshot
}

/// Holds the archive write lock through a second open file description, so to this process
/// it looks like another process holds it. Dropping the file releases it.
#[cfg(unix)]
pub(super) fn hold_write_lock_elsewhere(paths: &ProjectPaths) -> fs::File {
    use std::os::fd::AsRawFd;
    let path = marker(paths, ".pk-archive-write.lock");
    fs::create_dir_all(path.parent().unwrap()).expect("archive dir");
    let file = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&path)
        .expect("open lock file");
    // SAFETY: flock on a valid, owned descriptor.
    let held = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) };
    assert_eq!(held, 0, "the lock must be free when the test takes it");
    file
}

/// Quality-matrix rule 2 for old-format files: config.json's at-rest mode (read through the real
/// `load_config`) matches the header of each canonical file that exists.
pub(super) fn assert_config_matches_disk(paths: &ProjectPaths) {
    let expected = load_config(paths).expect("load config").archive_mode;
    for path in [&paths.legacy.archive, &paths.legacy.source_evidence] {
        let on_disk = match detect_disk_encryption_mode(path) {
            DiskEncryptionMode::Absent => continue,
            DiskEncryptionMode::Plaintext => ArchiveMode::Plaintext,
            DiskEncryptionMode::Encrypted => ArchiveMode::Encrypted,
        };
        assert_eq!(on_disk, expected, "config and {} disagree", path.display());
    }
}
