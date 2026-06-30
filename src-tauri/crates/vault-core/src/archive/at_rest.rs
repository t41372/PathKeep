//! At-rest encryption reconciliation for the canonical encrypted-tier databases.
//!
//! ## Responsibilities
//! - Detect when `source-evidence.sqlite`'s on-disk encryption state has drifted
//!   from the configured `archive_mode`, and converge it in place (encrypt a
//!   plaintext file, or decrypt an encrypted one) atomically and verifiably.
//! - Migrate `source-evidence.sqlite` in lockstep with an archive rekey.
//!
//! ## Not responsible for
//! - The canonical archive (`history-vault.sqlite`) itself — owned by the rekey
//!   flow in `maintenance.rs`, which calls this module to migrate
//!   source-evidence alongside it.
//! - The derived sidecars (search / intelligence / agent) — plaintext by design.
//!
//! ## Why this exists
//! The historical rekey migrated only the archive, leaving `source-evidence` in
//! its prior at-rest mode. When the prior mode was plaintext and the new mode
//! encrypted, `open_source_evidence_connection` then applied `PRAGMA key` to a
//! plaintext file; SQLCipher decoded the plaintext header as ciphertext, derived
//! a bogus page size, and aborted with `SQLITE_NOMEM` (out of memory) on *every*
//! backup — foreground and scheduled. Reconciling the at-rest *mode* fixes that
//! (the confirmed bug) and self-heals installs already in the drifted state.
//!
//! Scope: this handles MODE drift (plaintext-vs-encrypted), detected from the
//! file header without a key. It cannot detect or repair KEY drift — a file
//! encrypted with a stale passphrase (possible only on installs that rotated the
//! passphrase under the old archive-only rekey) looks identical to a
//! correctly-keyed one, and recovering it would need the old passphrase we never
//! store. The fixed rekey prevents new key drift by migrating both DBs in
//! lockstep; pre-existing key drift is tracked as a separate follow-up.

use super::{apply_cipher_key, current_timezone_name, export_archive_database};
use crate::{
    config::{ProjectPaths, ensure_paths},
    models::{AppConfig, ArchiveMode},
    utils::now_rfc3339,
};
use anyhow::{Context, Result};
use rusqlite::{Connection, params};
use serde::Serialize;
use serde_json::json;
use std::{fs, io::Read, path::Path, time::Duration as StdDuration};

/// The 16-byte magic every plaintext SQLite database file begins with. An
/// encrypted SQLCipher file starts with a random salt instead, so the presence
/// of this header is a cheap, key-free signal that a file is plaintext.
const SQLITE_FILE_HEADER: &[u8; 16] = b"SQLite format 3\0";

const BUSY_TIMEOUT: StdDuration = StdDuration::from_secs(5);

/// On-disk encryption state of a database file, detected without a key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DiskEncryptionMode {
    /// File begins with the SQLite plaintext header.
    Plaintext,
    /// File exists but does not begin with the plaintext header (SQLCipher salt).
    Encrypted,
    /// File is missing or shorter than the 16-byte header (nothing to reconcile).
    Absent,
}

/// Reads the first 16 bytes to classify a file's at-rest mode without a key.
pub(crate) fn detect_disk_encryption_mode(path: &Path) -> DiskEncryptionMode {
    let mut header = [0u8; 16];
    match fs::File::open(path).and_then(|mut file| file.read_exact(&mut header)) {
        Ok(()) if &header == SQLITE_FILE_HEADER => DiskEncryptionMode::Plaintext,
        Ok(()) => DiskEncryptionMode::Encrypted,
        Err(_) => DiskEncryptionMode::Absent,
    }
}

/// Outcome of a source-evidence at-rest reconciliation, surfaced to the UI so a
/// silent self-heal can still be reported honestly.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReconcileReport {
    /// True when source-evidence had to be rewritten to match the configured mode.
    pub repaired: bool,
    /// The drifted on-disk mode that was found (`None` when nothing was repaired).
    pub from_mode: Option<ArchiveMode>,
    /// The mode source-evidence now matches (the configured archive mode).
    pub to_mode: ArchiveMode,
}

/// Converges `source-evidence.sqlite` to `config.archive_mode`, using the
/// already-open `archive` connection for the repair run-ledger entry.
///
/// Cheap no-op (a single 16-byte header read) when source-evidence is absent or
/// already consistent, so it is safe to call on every backup and every unlock.
/// Records a `rekey`/`repair` run for PME transparency only when it actually
/// rewrites the file. `key` is the archive's current key; it both decrypts an
/// encrypted source and encrypts a plaintext one, so the realistic drift cases
/// use it directly.
pub(crate) fn reconcile_source_evidence_with_archive(
    archive: &Connection,
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
) -> Result<ReconcileReport> {
    ensure_paths(paths)?;
    let to_mode = config.archive_mode.clone();
    let path = &paths.source_evidence_database_path;

    // First, heal any rewrite interrupted by a crash/power loss: restore the live
    // file if a swap was cut between renames, and scrub a stale (possibly
    // plaintext) backstop left after a completed swap. MUST run before the
    // detect below, or a mid-swap-missing file would read as `Absent` and the
    // next open would silently create an empty database, orphaning the data.
    recover_interrupted_rewrite(path);

    let plan: Option<(Option<&str>, Option<&str>, ArchiveMode)> =
        match (&to_mode, detect_disk_encryption_mode(path)) {
            (ArchiveMode::Encrypted, DiskEncryptionMode::Plaintext) => {
                key.map(|_| (None, key, ArchiveMode::Plaintext))
            }
            (ArchiveMode::Plaintext, DiskEncryptionMode::Encrypted) => {
                key.map(|_| (key, None, ArchiveMode::Encrypted))
            }
            _ => None,
        };

    let Some((open_key, write_key, from_mode)) = plan else {
        return Ok(ReconcileReport { repaired: false, from_mode: None, to_mode });
    };

    let started_at = now_rfc3339();
    archive.execute(
        "INSERT INTO runs (run_type, trigger, started_at, timezone, status, profile_scope_json, warnings_json, stats_json, due_only)
         VALUES ('rekey', 'repair', ?1, ?2, 'running', '[]', '[]', '{}', 0)",
        params![started_at, current_timezone_name()],
    )?;
    let run_id = archive.last_insert_rowid();

    let rewrite = rewrite_database_file(path, open_key, write_key);
    finalize_repair_run(archive, run_id, &from_mode, &to_mode, &rewrite)?;
    rewrite?;

    Ok(ReconcileReport { repaired: true, from_mode: Some(from_mode), to_mode })
}

/// Opens the archive itself (for the run ledger) and reconciles source-evidence.
///
/// Used by the on-unlock repair command so the self-heal happens proactively,
/// before the user's next backup. The backup path instead reuses its
/// already-open archive connection via [`reconcile_source_evidence_with_archive`]
/// to avoid re-opening the (potentially multi-GB) archive twice.
pub fn reconcile_archive_encryption(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
) -> Result<ReconcileReport> {
    ensure_paths(paths)?;
    let archive = super::open_archive_connection(paths, config, key)?;
    reconcile_source_evidence_with_archive(&archive, paths, config, key)
}

/// Migrates source-evidence to the post-rekey at-rest mode, in lockstep with the
/// archive.
///
/// Detects the file's ACTUAL on-disk mode (it may already have drifted from
/// `current_config` on installs hit by the archive-only rekey) to pick the open
/// key, and writes with `target_key` (`None` when the new mode is plaintext).
pub(crate) fn migrate_source_evidence_for_rekey(
    paths: &ProjectPaths,
    old_key: Option<&str>,
    target_key: Option<&str>,
) -> Result<()> {
    let path = &paths.source_evidence_database_path;
    recover_interrupted_rewrite(path);
    let open_key = match detect_disk_encryption_mode(path) {
        DiskEncryptionMode::Encrypted => old_key,
        DiskEncryptionMode::Plaintext => None,
        DiskEncryptionMode::Absent => return Ok(()),
    };
    rewrite_database_file(path, open_key, target_key)
}

/// Rewrites one SQLite/SQLCipher database file from `open_key` (`None` =
/// plaintext source) to `write_key` (`None` = plaintext target), atomically.
///
/// The new copy is produced via `sqlcipher_export` into a sibling temp file
/// (`export_archive_database` propagates any write failure), then swapped in
/// atomically. The previous file is kept as a transient `.preencrypt` backstop
/// until the swap succeeds, then removed — we deliberately do NOT persist a
/// lingering plaintext snapshot of evidence the user just asked to encrypt.
pub(crate) fn rewrite_database_file(
    path: &Path,
    open_key: Option<&str>,
    write_key: Option<&str>,
) -> Result<()> {
    // Heal any prior interrupted rewrite (restore a cut swap, scrub stale
    // siblings) before starting a new one — never blindly delete a `.preencrypt`
    // that may be the only surviving copy.
    recover_interrupted_rewrite(path);
    let temp_path = path.with_extension("reencrypt.sqlite");
    let backup_path = path.with_extension("preencrypt.sqlite");

    export_source_to_temp(path, &temp_path, open_key, write_key)?;
    swap_in_place(path, &temp_path, &backup_path)?;
    remove_stale_sidecars(path);
    let _ = fs::remove_file(&backup_path);
    Ok(())
}

/// Recovers from a rewrite interrupted by a crash or power loss, and scrubs any
/// stale swap siblings left behind. Idempotent and cheap (a few `stat`s).
///
/// Two windows are handled:
/// - Crash *between* the two swap renames: the live file is missing but the
///   pre-rewrite copy survives at `.preencrypt`. We rename it back so the data is
///   never orphaned (a later reconcile redoes the rewrite).
/// - Crash *after* a completed swap but before the backstop was deleted: the live
///   file is correct and `.preencrypt` is a now-redundant copy — for a
///   plaintext→encrypted repair that is lingering plaintext, so it MUST be
///   removed. The exported `.reencrypt` is always stale and self-contained.
fn recover_interrupted_rewrite(path: &Path) {
    let temp_path = path.with_extension("reencrypt.sqlite");
    let backup_path = path.with_extension("preencrypt.sqlite");
    if !path.exists() && backup_path.exists() {
        let _ = fs::rename(&backup_path, path);
    }
    let _ = fs::remove_file(&temp_path);
    let _ = fs::remove_file(&backup_path);
}

/// Opens `path` in its current at-rest mode, confirms it is readable, and
/// exports a self-contained copy to `temp_path` in the target mode.
fn export_source_to_temp(
    path: &Path,
    temp_path: &Path,
    open_key: Option<&str>,
    write_key: Option<&str>,
) -> Result<()> {
    let source = open_in_mode(path, open_key)
        .with_context(|| format!("opening {} for at-rest rewrite", path.display()))?;
    // Confirm the source is actually readable in the assumed mode before export —
    // a wrong-mode open would otherwise surface as a confusing export failure.
    source
        .query_row("SELECT count(*) FROM sqlite_master", [], |row| row.get::<_, i64>(0))
        .with_context(|| format!("reading {} before at-rest rewrite", path.display()))?;
    export_archive_database(&source, temp_path, write_key)
}

/// Atomically swaps `temp_path` into `path`, keeping the previous file at
/// `backup_path` so a failed replace can be rolled back.
fn swap_in_place(path: &Path, temp_path: &Path, backup_path: &Path) -> Result<()> {
    fs::rename(path, backup_path)
        .with_context(|| format!("staging at-rest swap for {}", path.display()))?;
    if let Err(error) = fs::rename(temp_path, path) {
        let _ = fs::rename(backup_path, path);
        let _ = fs::remove_file(temp_path);
        return Err(error)
            .with_context(|| format!("replacing {} after at-rest rewrite", path.display()));
    }
    Ok(())
}

fn open_in_mode(path: &Path, key: Option<&str>) -> Result<Connection> {
    let connection =
        Connection::open(path).with_context(|| format!("opening {}", path.display()))?;
    connection.busy_timeout(BUSY_TIMEOUT)?;
    if let Some(key) = key {
        apply_cipher_key(&connection, key)?;
    }
    Ok(connection)
}

/// Removes stale rollback/WAL sidecars left next to the freshly swapped-in file.
/// The exported copy is self-contained, so any sidecar present belongs to the
/// previous file and would be wrongly replayed against the new one.
///
/// Shared with the archive rekey swap (`maintenance.rs`), which has the identical
/// "scrub the swapped-in file's foreign WAL so it cannot replay" need.
pub(crate) fn remove_stale_sidecars(path: &Path) {
    for suffix in ["-wal", "-shm", "-journal"] {
        let _ = fs::remove_file(std::path::PathBuf::from(format!("{}{}", path.display(), suffix)));
    }
}

fn finalize_repair_run(
    archive: &Connection,
    run_id: i64,
    from_mode: &ArchiveMode,
    to_mode: &ArchiveMode,
    rewrite: &Result<()>,
) -> Result<()> {
    let finished_at = now_rfc3339();
    let (status, error) = match rewrite {
        Ok(()) => ("success", None),
        Err(error) => ("failed", Some(format!("{error:#}"))),
    };
    let stats = json!({
        "operation": "source-evidence-at-rest-repair",
        "fromMode": from_mode,
        "toMode": to_mode,
    });
    archive.execute(
        "UPDATE runs
         SET finished_at = ?1, status = ?2, stats_json = ?3, error_message = ?4
         WHERE id = ?5",
        params![finished_at, status, serde_json::to_string(&stats)?, error, run_id],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::archive::{open_archive_connection, open_source_evidence_connection};
    use crate::config::project_paths_with_root;
    use tempfile::tempdir;

    const KEY: &str = "at-rest-reconcile-test-key";

    fn encrypted_config() -> AppConfig {
        AppConfig {
            archive_mode: ArchiveMode::Encrypted,
            initialized: true,
            ..AppConfig::default()
        }
    }

    fn plaintext_config() -> AppConfig {
        AppConfig {
            archive_mode: ArchiveMode::Plaintext,
            initialized: true,
            ..AppConfig::default()
        }
    }

    /// Seeds a *plaintext* source-evidence database with one row, exactly the
    /// drifted state an archive-only rekey leaves behind.
    fn seed_plaintext_source_evidence(paths: &ProjectPaths) {
        let connection =
            open_source_evidence_connection(paths, &plaintext_config(), None).expect("seed open");
        connection
            .execute(
                "INSERT INTO source_batches (
                   source_profile_id, run_id, source_kind, browser_version,
                   schema_version_text, schema_version_int, schema_fingerprint,
                   parser_version, capability_snapshot_json, coverage_stats_json,
                   artifact_refs_json, notes_json, created_at)
                 VALUES (1, NULL, 'chromium', NULL, NULL, NULL, 'fp', 'pv', '{}', '{}', NULL, NULL, '2026-06-28T00:00:00Z')",
                [],
            )
            .expect("seed row");
    }

    #[test]
    fn detect_disk_encryption_mode_classifies_each_state() {
        let dir = tempdir().expect("tempdir");
        let plaintext = dir.path().join("plain.sqlite");
        fs::write(&plaintext, b"SQLite format 3\0and then some").expect("write plain");
        assert_eq!(detect_disk_encryption_mode(&plaintext), DiskEncryptionMode::Plaintext);

        let encrypted = dir.path().join("enc.sqlite");
        fs::write(&encrypted, [0u8; 32]).expect("write enc");
        assert_eq!(detect_disk_encryption_mode(&encrypted), DiskEncryptionMode::Encrypted);

        let absent = dir.path().join("missing.sqlite");
        assert_eq!(detect_disk_encryption_mode(&absent), DiskEncryptionMode::Absent);

        let too_short = dir.path().join("short.sqlite");
        fs::write(&too_short, b"SQLite").expect("write short");
        assert_eq!(detect_disk_encryption_mode(&too_short), DiskEncryptionMode::Absent);
    }

    #[test]
    fn reproduces_oom_then_self_heals_plaintext_source_evidence() {
        // The regression: an encrypted config over a plaintext source-evidence
        // makes `open_source_evidence_connection` apply PRAGMA key to a plaintext
        // file, which SQLCipher rejects (NOMEM/NOTADB) — the live "out of memory"
        // backup failure.
        let dir = tempdir().expect("tempdir");
        let paths = project_paths_with_root(dir.path());
        seed_plaintext_source_evidence(&paths);
        assert_eq!(
            detect_disk_encryption_mode(&paths.source_evidence_database_path),
            DiskEncryptionMode::Plaintext
        );

        let config = encrypted_config();
        // The archive must exist + open with the key for the repair run ledger.
        let archive = open_archive_connection(&paths, &config, Some(KEY)).expect("open archive");

        // Bug present: opening source-evidence as if encrypted errors out.
        assert!(
            open_source_evidence_connection(&paths, &config, Some(KEY)).is_err(),
            "plaintext source-evidence opened as encrypted should fail (the OOM)"
        );

        // Auto-repair converges it.
        let report = reconcile_source_evidence_with_archive(&archive, &paths, &config, Some(KEY))
            .expect("reconcile");
        assert!(report.repaired);
        assert_eq!(report.from_mode, Some(ArchiveMode::Plaintext));
        assert_eq!(report.to_mode, ArchiveMode::Encrypted);
        assert_eq!(
            detect_disk_encryption_mode(&paths.source_evidence_database_path),
            DiskEncryptionMode::Encrypted
        );

        // Fix proven: the encrypted open now succeeds AND preserves the row.
        let healed =
            open_source_evidence_connection(&paths, &config, Some(KEY)).expect("encrypted re-open");
        let rows: i64 = healed
            .query_row("SELECT COUNT(*) FROM source_batches", [], |row| row.get(0))
            .expect("count");
        assert_eq!(rows, 1, "repair must preserve the source-evidence rows");

        // No lingering plaintext backstop or temp files.
        assert!(!paths.source_evidence_database_path.with_extension("preencrypt.sqlite").exists());
        assert!(!paths.source_evidence_database_path.with_extension("reencrypt.sqlite").exists());
    }

    #[test]
    fn reconcile_is_a_noop_when_already_consistent() {
        let dir = tempdir().expect("tempdir");
        let paths = project_paths_with_root(dir.path());
        let config = plaintext_config();
        seed_plaintext_source_evidence(&paths);
        let archive = open_archive_connection(&paths, &config, None).expect("open archive");

        let report = reconcile_source_evidence_with_archive(&archive, &paths, &config, None)
            .expect("reconcile");
        assert!(!report.repaired);
        assert_eq!(report.from_mode, None);

        // Absent source-evidence is also a no-op.
        let empty = tempdir().expect("tempdir");
        let empty_paths = project_paths_with_root(empty.path());
        let empty_archive =
            open_archive_connection(&empty_paths, &config, None).expect("open empty archive");
        let absent_report =
            reconcile_source_evidence_with_archive(&empty_archive, &empty_paths, &config, None)
                .expect("reconcile absent");
        assert!(!absent_report.repaired);
    }

    #[test]
    fn reconcile_without_key_leaves_the_file_untouched() {
        // When encrypted config drifts but no key is present (locked), we cannot
        // encrypt — reconcile must not touch the file (the upstream open will
        // surface the honest "key required" error instead).
        let dir = tempdir().expect("tempdir");
        let paths = project_paths_with_root(dir.path());
        seed_plaintext_source_evidence(&paths);
        let config = encrypted_config();
        // Build a plaintext archive connection purely to host the runs table.
        let archive =
            open_archive_connection(&paths, &plaintext_config(), None).expect("open archive");

        let report = reconcile_source_evidence_with_archive(&archive, &paths, &config, None)
            .expect("reconcile");
        assert!(!report.repaired);
        assert_eq!(
            detect_disk_encryption_mode(&paths.source_evidence_database_path),
            DiskEncryptionMode::Plaintext
        );
    }

    #[test]
    fn reconcile_archive_encryption_opens_and_repairs() {
        let dir = tempdir().expect("tempdir");
        let paths = project_paths_with_root(dir.path());
        seed_plaintext_source_evidence(&paths);
        // Materialise an encrypted archive so the public entry point can open it.
        let config = encrypted_config();
        drop(
            open_archive_connection(&paths, &config, Some(KEY)).expect("create encrypted archive"),
        );

        let report =
            reconcile_archive_encryption(&paths, &config, Some(KEY)).expect("reconcile entry");
        assert!(report.repaired);
        assert_eq!(
            detect_disk_encryption_mode(&paths.source_evidence_database_path),
            DiskEncryptionMode::Encrypted
        );
    }

    #[test]
    fn migrate_source_evidence_for_rekey_encrypts_and_decrypts() {
        let dir = tempdir().expect("tempdir");
        let paths = project_paths_with_root(dir.path());
        seed_plaintext_source_evidence(&paths);

        // Plaintext -> encrypted (enabling encryption).
        migrate_source_evidence_for_rekey(&paths, None, Some(KEY)).expect("encrypt");
        assert_eq!(
            detect_disk_encryption_mode(&paths.source_evidence_database_path),
            DiskEncryptionMode::Encrypted
        );
        let encrypted =
            open_source_evidence_connection(&paths, &encrypted_config(), Some(KEY)).expect("open");
        assert_eq!(
            encrypted
                .query_row("SELECT COUNT(*) FROM source_batches", [], |row| row.get::<_, i64>(0))
                .expect("count"),
            1
        );
        drop(encrypted);

        // Encrypted -> plaintext (disabling encryption); open key detected from disk.
        migrate_source_evidence_for_rekey(&paths, Some(KEY), None).expect("decrypt");
        assert_eq!(
            detect_disk_encryption_mode(&paths.source_evidence_database_path),
            DiskEncryptionMode::Plaintext
        );

        // Absent file is a no-op.
        let empty = tempdir().expect("tempdir");
        let empty_paths = project_paths_with_root(empty.path());
        migrate_source_evidence_for_rekey(&empty_paths, None, Some(KEY)).expect("absent no-op");
    }

    #[test]
    fn rewrite_database_file_rejects_unreadable_source() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("source-evidence.sqlite");
        // A non-SQLite blob cannot be read as plaintext SQLite.
        fs::write(&path, b"not a database at all, just bytes").expect("write garbage");
        let error = rewrite_database_file(&path, None, Some(KEY)).expect_err("garbage source");
        assert!(format!("{error:#}").contains("at-rest rewrite"));
        // The original file is left in place (no half-swap).
        assert!(path.exists());
    }

    #[test]
    fn reconcile_decrypts_drifted_encrypted_source_evidence() {
        // The inverse drift: config is plaintext but source-evidence is still
        // encrypted. Reconcile must decrypt it using the supplied key.
        let dir = tempdir().expect("tempdir");
        let paths = project_paths_with_root(dir.path());
        seed_plaintext_source_evidence(&paths);
        migrate_source_evidence_for_rekey(&paths, None, Some(KEY)).expect("encrypt");
        assert_eq!(
            detect_disk_encryption_mode(&paths.source_evidence_database_path),
            DiskEncryptionMode::Encrypted
        );

        let config = plaintext_config();
        let archive = open_archive_connection(&paths, &config, None).expect("open archive");
        let report = reconcile_source_evidence_with_archive(&archive, &paths, &config, Some(KEY))
            .expect("reconcile");
        assert!(report.repaired);
        assert_eq!(report.from_mode, Some(ArchiveMode::Encrypted));
        assert_eq!(report.to_mode, ArchiveMode::Plaintext);
        assert_eq!(
            detect_disk_encryption_mode(&paths.source_evidence_database_path),
            DiskEncryptionMode::Plaintext
        );
    }

    #[test]
    fn reconcile_records_failed_run_when_rewrite_fails() {
        // A file that *looks* plaintext (right header) but is unreadable forces
        // the rewrite to fail; reconcile must finalize the repair run as failed
        // and surface the error rather than swallowing it.
        let dir = tempdir().expect("tempdir");
        let paths = project_paths_with_root(dir.path());
        let config = encrypted_config();
        let archive =
            open_archive_connection(&paths, &config, Some(KEY)).expect("open encrypted archive");
        let mut header = b"SQLite format 3\0".to_vec();
        header.extend_from_slice(b"corrupt body that is not a real page");
        fs::write(&paths.source_evidence_database_path, header).expect("write corrupt");

        let error = reconcile_source_evidence_with_archive(&archive, &paths, &config, Some(KEY))
            .expect_err("rewrite of a corrupt source must fail");
        assert!(format!("{error:#}").contains("at-rest rewrite"));

        let failed: i64 = archive
            .query_row(
                "SELECT COUNT(*) FROM runs WHERE run_type = 'rekey' AND trigger = 'repair' AND status = 'failed'",
                [],
                |row| row.get(0),
            )
            .expect("count failed repair runs");
        assert_eq!(failed, 1, "a failed repair run must be recorded for transparency");
    }

    #[test]
    fn swap_in_place_rolls_back_when_replace_fails() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("db.sqlite");
        let temp = dir.path().join("db.reencrypt.sqlite"); // intentionally missing
        let backup = dir.path().join("db.preencrypt.sqlite");
        fs::write(&path, b"original").expect("write path");

        let error = swap_in_place(&path, &temp, &backup).expect_err("missing temp fails replace");
        assert!(format!("{error:#}").contains("replacing"));
        // Rollback must restore the original, untouched.
        assert!(path.exists());
        assert_eq!(fs::read(&path).expect("read restored"), b"original");
        assert!(!backup.exists());
    }

    #[test]
    fn recover_interrupted_rewrite_restores_then_scrubs() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("source-evidence.sqlite");
        let temp = path.with_extension("reencrypt.sqlite");
        let backup = path.with_extension("preencrypt.sqlite");

        // Crash BETWEEN the two swap renames: live file missing, backstop survives.
        fs::write(&backup, b"original data").expect("write backup");
        fs::write(&temp, b"half-written export").expect("write temp");
        recover_interrupted_rewrite(&path);
        assert!(path.exists(), "the pre-rewrite copy must be restored");
        assert_eq!(fs::read(&path).expect("read restored"), b"original data");
        assert!(!backup.exists());
        assert!(!temp.exists(), "stale export must be scrubbed");

        // Crash AFTER a completed swap: live file present, plaintext backstop lingers.
        fs::write(&backup, b"lingering plaintext").expect("rewrite backstop");
        recover_interrupted_rewrite(&path);
        assert!(path.exists());
        assert_eq!(fs::read(&path).expect("read"), b"original data");
        assert!(!backup.exists(), "lingering plaintext backstop must be scrubbed");

        // Nothing to do when the file is already clean.
        recover_interrupted_rewrite(&path);
        assert!(path.exists());
    }
}
