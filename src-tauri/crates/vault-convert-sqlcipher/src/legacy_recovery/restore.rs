//! Responsibilities: complete or roll back a full-archive restore of the old SQLCipher format
//! that a crash cut while the canonical archive was quarantined and the snapshot not yet in place.
//! Not responsible for: native restores (`vault_core::maintenance`), writing journals (the old
//! restore flow is gone) or rebuilding source evidence (the converter recreates it).
//! Failure modes: booting an empty archive over quarantined originals, trusting an encrypted
//! canonical we cannot verify without a key, dropping an unreadable marker while the canonical is
//! absent. Tests: legacy_recovery::restore::tests.
use anyhow::{Context, Result};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
    time::Duration as StdDuration,
};
use vault_core::{
    AppConfig, ArchiveMode, DiskEncryptionMode, ProjectPaths,
    archive::ArchiveWriteLock,
    detect_disk_encryption_mode,
    durable_io::{atomic_durable_write, install_file_durably, remove_file_durably},
    save_config,
};

/// Filename of the durable interrupted-restore marker, placed beside the canonical archive
/// database. The `.pk-` prefix keeps retention / cleanup from sweeping it — the same dotfile-skip
/// contract the rekey/import journals and the write-lock sentinel rely on — so a crash-mid-restore
/// signal can never be deleted before recovery acts on it.
const RESTORE_JOURNAL_FILE: &str = ".pk-restore-journal.json";

/// The durable record of an in-flight full-archive restore's quarantine→install→commit phase.
/// Written (durably) just BEFORE the quarantine move and removed (durably) AFTER the final
/// post-restore verify. Its mere presence at next launch SIGNALS that a crash MAY have cut the
/// window in which the canonical history-vault is ABSENT (quarantined but the snapshot not yet
/// installed); its contents are exactly what [`recover_interrupted_restore`] needs to COMPLETE the
/// restore from the still-available snapshot, or ROLL BACK to the quarantined originals — never
/// boot an empty archive. The quarantine move and the install are NOT atomic with each other, so
/// this marker — not the filesystem alone — is the source of truth for "did the restore commit?".
#[derive(Debug, Clone, Serialize, Deserialize)]
struct RestoreJournal {
    version: u32,
    timestamp: String,
    /// The chosen snapshot path UNDER raw-snapshots/ (NOT moved by quarantine; stays available to
    /// re-install from). Already de-canonicalized when the journal was written.
    snapshot_ref: String,
    /// The snapshot's real at-rest mode (config is reconciled to this on completion).
    restored_mode: ArchiveMode,
    /// The dated `quarantine/<ts>/` dir the pre-restore originals were moved into (the rollback source).
    quarantine_dir: String,
    /// The dest's pre-restore `config.json` bytes (`None` when none existed), so a rollback restores
    /// the exact prior config rather than guessing.
    previous_config: Option<String>,
}

/// Path of the interrupted-restore marker (beside the canonical archive database).
fn restore_journal_path(paths: &ProjectPaths) -> PathBuf {
    paths
        .archive_database_path
        .parent()
        .expect("archive database path has a parent directory")
        .join(RESTORE_JOURNAL_FILE)
}

/// `true` when the durable interrupted-restore marker is present beside the canonical archive —
/// a single `stat`, no lock. The launch-time fast path
/// ([`super::recover_on_launch`]) uses it to decide whether to take the gate +
/// flock at all, so the overwhelmingly common no-marker launch never touches a lock.
pub(super) fn interrupted_restore_marker_present(paths: &ProjectPaths) -> bool {
    restore_journal_path(paths).exists()
}

#[cfg(test)]
/// Durably writes `journal` to the marker path (atomic temp + F_FULLFSYNC + rename + dir fsync), so
/// its presence is itself durable before the quarantine move runs.
fn write_restore_journal(paths: &ProjectPaths, journal: &RestoreJournal) -> Result<()> {
    let bytes =
        serde_json::to_vec(journal).context("serializing the interrupted-restore journal")?;
    atomic_durable_write(&restore_journal_path(paths), &bytes)
        .context("writing the interrupted-restore journal")
}

/// Reads the interrupted-restore marker. `Ok(None)` when it is absent OR unparseable.
///
/// Like the rekey and import markers, an unparseable one is never dropped here: a restore can be
/// mid-window with the canonical ABSENT, and dropping the marker then would let launch step (4)
/// see an Absent canonical, return `Healthy`, and boot a brand-new EMPTY archive over the
/// quarantined originals. The canonical-aware drop/fail-closed decision is the CALLER's
/// ([`recover_interrupted_restore`]): drop it only when the canonical is present, fail closed
/// (leave it) when the canonical is absent.
fn read_restore_journal(paths: &ProjectPaths) -> Result<Option<RestoreJournal>> {
    let path = restore_journal_path(paths);
    if !path.exists() {
        return Ok(None);
    }
    let bytes = fs::read(&path).with_context(|| format!("reading {}", path.display()))?;
    match serde_json::from_slice::<RestoreJournal>(&bytes) {
        Ok(journal) => Ok(Some(journal)),
        // LEAVE the marker (do NOT delete here) — the caller decides based on the canonical.
        Err(_) => Ok(None),
    }
}

/// Crash-recovery twin of the old full-archive restore's quarantine→install→commit phase.
///
/// A NESTED recover-first helper, called from the launch-time `recover_archive_on_launch_locked`
/// (which already holds the top-level gate + lock when it calls in). When an interrupted-restore
/// marker is present a crash MAY have cut the window in which the canonical history-vault is ABSENT
/// (quarantined but the snapshot not yet installed), so it COMPLETES the restore from the
/// still-available snapshot — KEY-FREE — or ROLLS BACK to the quarantined originals, then clears
/// the marker. It never boots an empty archive. Returns `true` only when it actually recovered
/// something.
///
/// Cheap in the common case — a single `stat` of the marker path, no lock taken — so it is safe to
/// call on every open. Only when the marker exists does it take the cross-process
/// [`ArchiveWriteLock`] (recovery re-installs / moves canonical DB files + rewrites config, which
/// must serialize against a concurrent scheduled backup) and re-read the journal (it may have been
/// cleared while we waited for the lock). It takes ONLY the reentrant flock, NEVER the non-reentrant
/// top-level [`vault_core::archive::ArchiveOpGate`]: re-acquiring the flock its caller already holds yields a nested
/// guard (no self-deadlock), while taking the gate its caller already holds WOULD self-deadlock — so
/// this nested helper must never touch it.
pub(super) fn recover_interrupted_restore(paths: &ProjectPaths) -> Result<bool> {
    if !restore_journal_path(paths).exists() {
        return Ok(false);
    }
    let _lock = ArchiveWriteLock::acquire(paths)
        .context("acquiring the archive write lock for interrupted-restore recovery")?;
    // Re-check under the lock: the marker may have been cleared by a concurrent recovery while we
    // waited. We now hold the lock, so this read is stable.
    if !restore_journal_path(paths).exists() {
        return Ok(false);
    }
    let Some(journal) = read_restore_journal(paths)? else {
        // Marker present (just confirmed) but UNPARSEABLE. Unlike rekey/import (whose canonical is
        // never absent), a restore can be mid-window with the canonical ABSENT; silently dropping the
        // marker then would let launch step (4) see Absent -> Healthy -> boot an EMPTY archive over
        // the quarantined originals. FAIL CLOSED when the canonical is absent (leave the marker,
        // surface Unrecoverable); only drop the unusable marker when the canonical is present (step 4
        // then reconciles config to it).
        if detect_disk_encryption_mode(&paths.archive_database_path) == DiskEncryptionMode::Absent {
            anyhow::bail!(
                "interrupted-restore recovery cannot proceed: the restore journal {} is present but \
                 unreadable AND the canonical history-vault is absent, so the restore can neither be \
                 completed nor rolled back automatically. Recover from a safety snapshot manually \
                 before retrying.",
                restore_journal_path(paths).display(),
            );
        }
        remove_file_durably(&restore_journal_path(paths))
            .context("removing the unparseable restore journal")?;
        return Ok(true);
    };
    resolve_interrupted_restore(paths, &journal)?;
    remove_file_durably(&restore_journal_path(paths))
        .context("clearing the restore marker after recovery")?;
    Ok(true)
}

/// Reconciles an interrupted restore KEY-FREE, treating the canonical archive's on-disk header and
/// the still-available snapshot as the authorities (`install_file_durably` is ATOMIC, so the
/// canonical file is EITHER the whole restored snapshot OR the whole pre-restore original, never a
/// partial mix).
fn resolve_interrupted_restore(paths: &ProjectPaths, journal: &RestoreJournal) -> Result<()> {
    if canonical_restore_already_landed(paths, &journal.restored_mode) {
        // The restore already landed (or the original is fine to keep): only the marker-clear commit
        // didn't finish. Converge config to the restored mode and let the caller clear the marker.
        reconcile_restore_config(paths, journal)?;
        return Ok(());
    }
    let snapshot_ref = PathBuf::from(&journal.snapshot_ref);
    if snapshot_still_restorable(&snapshot_ref, &journal.restored_mode) {
        // COMPLETE: the canonical history-vault is absent/unverifiable but the chosen snapshot is
        // still restorable, so finish the half-done install from it. Scrub any half-staged
        // `restore.sqlite` temp + its sidecars first so no phantom file blocks the durable install.
        let restore_temp = paths.archive_database_path.with_extension("restore.sqlite");
        let _ = fs::remove_file(&restore_temp);
        super::remove_stale_sidecars(&restore_temp);
        install_restored_archive(&snapshot_ref, &restore_temp, paths)?;
        reconcile_restore_config(paths, journal)?;
        // The canonical is now definitively the restored snapshot, so any rekey/import marker the
        // quarantine move could not relocate is stale — purge it so a later launch never acts on it.
        clear_superseded_crash_markers(paths);
        // Source-evidence is NOT rebuilt here — it self-heals on the next KEYED open (the keyed
        // on-unlock reconcile / next backup), mirroring the rekey source-evidence deferral. Whether
        // it is absent (full quarantine) or a stale leftover (partial quarantine window), the keyed
        // path converges it; a keyless launch holds no key to recreate it in the restored mode.
        return Ok(());
    }
    // Neither the canonical nor the snapshot is usable: roll back to the quarantined originals (or
    // fail closed if those are gone too). NEVER boot an empty archive.
    rollback_restore(paths, journal)
}

/// `true` when the canonical history-vault on disk is ALREADY the restored archive (so only the
/// marker-clear commit is outstanding), judged KEY-FREE from its header.
fn canonical_restore_already_landed(paths: &ProjectPaths, restored_mode: &ArchiveMode) -> bool {
    match detect_disk_encryption_mode(&paths.archive_database_path) {
        // Absent: the install never landed (the crash cut the absent-canonical window) — re-install.
        DiskEncryptionMode::Absent => false,
        // Plaintext: we can key-free `quick_check` it, so confirm it is BOTH the restored mode AND
        // structurally sound before treating the restore as landed.
        DiskEncryptionMode::Plaintext => {
            restored_mode == &ArchiveMode::Plaintext
                && plaintext_quick_check(&paths.archive_database_path).is_ok()
        }
        // Encrypted: we hold NO key here, so we cannot key-free-verify it — we cannot tell a
        // correctly-restored encrypted snapshot from a still-broken encrypted pre-restore original.
        // Force a re-install from the snapshot that WAS keyed-verified at restore start rather than
        // trusting a header we cannot validate.
        DiskEncryptionMode::Encrypted => false,
    }
}

/// `true` when the chosen snapshot is still usable to (re-)install as the canonical archive, judged
/// KEY-FREE.
fn snapshot_still_restorable(snapshot_ref: &Path, restored_mode: &ArchiveMode) -> bool {
    match detect_disk_encryption_mode(snapshot_ref) {
        // Absent/too-small: the snapshot is gone or truncated — not restorable.
        DiskEncryptionMode::Absent => false,
        // Plaintext: a key-free `quick_check` confirms it is BOTH the restored mode AND sound.
        DiskEncryptionMode::Plaintext => {
            restored_mode == &ArchiveMode::Plaintext && plaintext_quick_check(snapshot_ref).is_ok()
        }
        // Encrypted: we hold no key here, so trust the HEADER match — the snapshot was keyed-verified
        // (`quick_check`) at restore start, just before the journal was written, so a present
        // encrypted header is the same verified file we are re-installing.
        DiskEncryptionMode::Encrypted => restored_mode == &ArchiveMode::Encrypted,
    }
}

/// Commits a restored archive's config: bases on the journal's captured pre-restore config when it
/// parses (so non-archive user preferences survive), else the dest's current config, else defaults;
/// forces `archive_mode = restored_mode` + `initialized = true`; and persists it durably. Mirrors
/// `complete_rekey_config` / `base_config_for_recovery`.
fn reconcile_restore_config(paths: &ProjectPaths, journal: &RestoreJournal) -> Result<()> {
    let mut config = match journal.previous_config.as_deref().map(serde_json::from_str::<AppConfig>)
    {
        Some(Ok(config)) => config,
        _ => vault_core::load_config(paths).unwrap_or_default(),
    };
    config.archive_mode = journal.restored_mode.clone();
    config.initialized = true;
    save_config(paths, &config)
}

/// Rolls back an interrupted restore whose snapshot is no longer usable: move the quarantined
/// pre-restore originals back into place and restore the pre-restore config. FAIL-CLOSED when the
/// snapshot is gone AND the originals cannot be recovered from quarantine — never boot empty.
fn rollback_restore(paths: &ProjectPaths, journal: &RestoreJournal) -> Result<()> {
    let quarantine_dir = PathBuf::from(&journal.quarantine_dir);
    for canonical in [&paths.archive_database_path, &paths.source_evidence_database_path] {
        restore_quarantined_original(&quarantine_dir, canonical);
    }
    if detect_disk_encryption_mode(&paths.archive_database_path) == DiskEncryptionMode::Absent {
        // FAIL-CLOSED Unrecoverable: the chosen snapshot is missing/unverifiable AND the pre-restore
        // originals are not recoverable from quarantine, so no canonical archive can be reached.
        // Leave the marker (the caller keeps it on Err) and name both backstops so the recovery GUI
        // can guide a manual fix — we must NEVER fall through and boot an empty archive.
        anyhow::bail!(
            "interrupted-restore recovery cannot proceed: the chosen snapshot {} is missing or \
             unverifiable AND the pre-restore originals are not recoverable from quarantine {}; \
             restore manually before retrying.",
            journal.snapshot_ref,
            quarantine_dir.display(),
        );
    }
    match &journal.previous_config {
        Some(bytes) => atomic_durable_write(&paths.config_path, bytes.as_bytes())
            .context("restoring the pre-restore config")?,
        None => {
            let _ = remove_file_durably(&paths.config_path);
        }
    }
    Ok(())
}

/// Moves one quarantined pre-restore original back onto its canonical path (and its
/// `-wal`/`-shm`/`-journal` sidecars), if present in `quarantine_dir`. Best-effort — a genuinely
/// absent quarantined file is fine (nothing to restore), so this returns no `Result`.
fn restore_quarantined_original(quarantine_dir: &Path, canonical: &Path) {
    let Some(name) = canonical.file_name() else {
        return;
    };
    let quarantined = quarantine_dir.join(name);
    if quarantined.exists() {
        let _ = fs::rename(&quarantined, canonical);
        for suffix in ["-wal", "-shm", "-journal"] {
            let from = PathBuf::from(format!("{}{}", quarantined.display(), suffix));
            if from.exists() {
                let to = PathBuf::from(format!("{}{}", canonical.display(), suffix));
                let _ = fs::rename(&from, &to);
            }
        }
    }
}

/// Best-effort durable removal of superseded interrupted-rekey/import markers, safe to call ONLY
/// once the canonical archive IS the restored snapshot (a full-archive restore replaces the whole
/// canonical, so any leftover rekey/import marker is definitively stale and acting on it would
/// re-introduce drift — e.g. a later `rollback_rekey` overwriting the reconciled config). Best-effort:
/// a marker that cannot be removed is no worse than today. Never call it while a PRE-restore original
/// is in place (a rollback path), whose rekey/import marker must be preserved.
fn clear_superseded_crash_markers(paths: &ProjectPaths) {
    let _ = remove_file_durably(&super::rekey::rekey_journal_path(paths));
    let _ = remove_file_durably(&super::import::import_journal_path(paths));
}

/// Copies the verified snapshot into a temp beside the canonical archive, then durably installs it
/// and scrubs any foreign WAL.
fn install_restored_archive(
    snapshot_path: &Path,
    restore_temp: &Path,
    paths: &ProjectPaths,
) -> Result<()> {
    fs::copy(snapshot_path, restore_temp)
        .with_context(|| format!("staging the restore snapshot at {}", restore_temp.display()))?;
    install_file_durably(restore_temp, &paths.archive_database_path)
        .context("durably installing the restored archive")?;
    super::remove_stale_sidecars(&paths.archive_database_path);
    Ok(())
}

/// Confirms a plaintext file opens and passes `PRAGMA quick_check`.
fn plaintext_quick_check(path: &Path) -> Result<()> {
    let connection = Connection::open(path)
        .with_context(|| format!("opening {} for verification", path.display()))?;
    connection.busy_timeout(StdDuration::from_secs(5))?;
    let status: String = connection
        .query_row("PRAGMA quick_check", [], |row| row.get(0))
        .with_context(|| format!("running quick_check on {}", path.display()))?;
    if status != "ok" {
        anyhow::bail!("integrity check of {} failed: {status}", path.display());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::super::fixtures::*;
    use super::*;
    use tempfile::tempdir;
    use vault_core::{config::ensure_paths, project_paths_with_root};

    fn journal(paths: &ProjectPaths, snapshot: &str, quarantine: &Path) -> RestoreJournal {
        RestoreJournal {
            version: 1,
            timestamp: CRASH_TIMESTAMP.to_string(),
            snapshot_ref: paths
                .raw_snapshots_dir
                .join("rekey")
                .join(snapshot)
                .display()
                .to_string(),
            restored_mode: ArchiveMode::Plaintext,
            quarantine_dir: quarantine.display().to_string(),
            previous_config: None,
        }
    }

    #[test]
    fn completes_an_absent_canonical_from_the_snapshot() {
        let dir = tempdir().unwrap();
        let paths = project_paths_with_root(dir.path());
        ensure_paths(&paths).unwrap();
        let snapshot = crash_mid_restore(&paths, &plaintext_config(), None);

        assert!(recover_interrupted_restore(&paths).expect("recovery"));
        assert_eq!(quick_check(&paths.archive_database_path, None), "ok");
        assert_eq!(sentinel_runs(&paths.archive_database_path, None), 1, "data, not an empty file");
        assert!(!interrupted_restore_marker_present(&paths));
        assert!(snapshot.exists(), "the snapshot is copied, never moved");
    }

    #[test]
    fn completes_an_encrypted_restore_without_the_key() {
        let dir = tempdir().unwrap();
        let paths = project_paths_with_root(dir.path());
        ensure_paths(&paths).unwrap();
        crash_mid_restore(&paths, &encrypted_config(), Some(KEY));

        assert!(recover_interrupted_restore(&paths).expect("key-free recovery"));
        assert_eq!(
            detect_disk_encryption_mode(&paths.archive_database_path),
            DiskEncryptionMode::Encrypted
        );
        assert!(matches!(
            vault_core::load_config(&paths).unwrap().archive_mode,
            ArchiveMode::Encrypted
        ));
        assert_eq!(sentinel_runs(&paths.archive_database_path, Some(KEY)), 1);
        assert_eq!(quick_check(&paths.archive_database_path, Some(KEY)), "ok");
    }

    #[test]
    fn rolls_back_to_the_quarantined_original_when_the_snapshot_is_gone() {
        let dir = tempdir().unwrap();
        let paths = project_paths_with_root(dir.path());
        ensure_paths(&paths).unwrap();
        let snapshot = crash_mid_restore(&paths, &plaintext_config(), None);
        fs::remove_file(snapshot).unwrap();

        assert!(recover_interrupted_restore(&paths).expect("rollback"));
        assert_eq!(sentinel_runs(&paths.archive_database_path, None), 1, "the original came back");
        assert!(!interrupted_restore_marker_present(&paths));
    }

    #[test]
    fn fails_closed_when_snapshot_and_originals_are_both_gone() {
        let dir = tempdir().unwrap();
        let paths = project_paths_with_root(dir.path());
        ensure_paths(&paths).unwrap();
        let snapshot = crash_mid_restore(&paths, &plaintext_config(), None);
        fs::remove_file(snapshot).unwrap();
        fs::remove_file(paths.quarantine_dir.join(CRASH_TIMESTAMP).join("history-vault.sqlite"))
            .unwrap();

        let error = recover_interrupted_restore(&paths).expect_err("nothing to restore from");
        assert!(format!("{error:#}").contains("not recoverable from quarantine"), "{error:#}");
        assert!(interrupted_restore_marker_present(&paths), "the marker stays for a retry");
        assert_eq!(
            detect_disk_encryption_mode(&paths.archive_database_path),
            DiskEncryptionMode::Absent,
            "no empty archive appears",
        );
    }

    #[test]
    fn only_reconciles_config_when_the_restore_already_landed() {
        let dir = tempdir().unwrap();
        let paths = project_paths_with_root(dir.path());
        ensure_paths(&paths).unwrap();
        legacy_db(&paths.archive_database_path, None);
        let landed = fs::read(&paths.archive_database_path).unwrap();
        let quarantine = paths.quarantine_dir.join(CRASH_TIMESTAMP);
        write_restore_journal(&paths, &journal(&paths, "gone.sqlite", &quarantine)).unwrap();

        assert!(recover_interrupted_restore(&paths).expect("recovery"));
        assert_eq!(fs::read(&paths.archive_database_path).unwrap(), landed, "not re-copied");
        let config = vault_core::load_config(&paths).unwrap();
        assert!(matches!(config.archive_mode, ArchiveMode::Plaintext));
        assert!(config.initialized);
        assert!(!interrupted_restore_marker_present(&paths));
    }

    #[test]
    fn without_a_marker_does_nothing() {
        let dir = tempdir().unwrap();
        let paths = project_paths_with_root(dir.path());
        assert!(!recover_interrupted_restore(&paths).unwrap());
        assert!(!interrupted_restore_marker_present(&paths));
    }

    #[test]
    fn keeps_an_unreadable_marker_while_the_canonical_is_absent() {
        let dir = tempdir().unwrap();
        let paths = project_paths_with_root(dir.path());
        ensure_paths(&paths).unwrap();
        fs::write(restore_journal_path(&paths), b"not valid json").unwrap();

        let error = recover_interrupted_restore(&paths).expect_err("must fail closed");
        let rendered = format!("{error:#}");
        assert!(rendered.contains("present but unreadable") && rendered.contains("absent"));
        assert!(interrupted_restore_marker_present(&paths));
    }

    #[test]
    fn drops_an_unreadable_marker_when_the_canonical_is_present() {
        let dir = tempdir().unwrap();
        let paths = project_paths_with_root(dir.path());
        ensure_paths(&paths).unwrap();
        legacy_db(&paths.archive_database_path, None);
        let before = fs::read(&paths.archive_database_path).unwrap();
        fs::write(restore_journal_path(&paths), b"not valid json").unwrap();

        assert!(recover_interrupted_restore(&paths).unwrap());
        assert!(!interrupted_restore_marker_present(&paths));
        assert_eq!(fs::read(&paths.archive_database_path).unwrap(), before, "untouched");
    }

    #[test]
    fn journal_round_trips_through_disk() {
        let dir = tempdir().unwrap();
        let paths = project_paths_with_root(dir.path());
        ensure_paths(&paths).unwrap();
        assert!(read_restore_journal(&paths).unwrap().is_none());
        let mut written = journal(&paths, "snap.sqlite", Path::new("/quarantine/x"));
        written.restored_mode = ArchiveMode::Encrypted;
        written.previous_config = Some("cfg".into());
        write_restore_journal(&paths, &written).unwrap();
        let read = read_restore_journal(&paths).unwrap().unwrap();
        assert!(matches!(read.restored_mode, ArchiveMode::Encrypted));
        assert_eq!(read.snapshot_ref, written.snapshot_ref);
        assert_eq!(read.quarantine_dir, "/quarantine/x");
        assert_eq!(read.previous_config.as_deref(), Some("cfg"));
    }

    #[test]
    fn landed_judges_each_on_disk_state() {
        let dir = tempdir().unwrap();
        let paths = project_paths_with_root(dir.path());
        ensure_paths(&paths).unwrap();
        let archive = &paths.archive_database_path;
        assert!(!canonical_restore_already_landed(&paths, &ArchiveMode::Plaintext), "absent");
        legacy_db(archive, None);
        assert!(canonical_restore_already_landed(&paths, &ArchiveMode::Plaintext));
        assert!(
            !canonical_restore_already_landed(&paths, &ArchiveMode::Encrypted),
            "mode mismatch"
        );
        plaintext_header(archive);
        assert!(
            !canonical_restore_already_landed(&paths, &ArchiveMode::Plaintext),
            "not a database"
        );
        encrypted_header(archive);
        assert!(!canonical_restore_already_landed(&paths, &ArchiveMode::Encrypted), "unverifiable");
    }

    #[test]
    fn restorable_judges_each_on_disk_state() {
        let dir = tempdir().unwrap();
        let snap = dir.path().join("snap.sqlite");
        assert!(!snapshot_still_restorable(&snap, &ArchiveMode::Plaintext), "absent");
        legacy_db(&snap, None);
        assert!(snapshot_still_restorable(&snap, &ArchiveMode::Plaintext));
        assert!(!snapshot_still_restorable(&snap, &ArchiveMode::Encrypted), "mode mismatch");
        plaintext_header(&snap);
        assert!(!snapshot_still_restorable(&snap, &ArchiveMode::Plaintext), "not a database");
        encrypted_header(&snap);
        assert!(snapshot_still_restorable(&snap, &ArchiveMode::Encrypted), "verified at start");
        assert!(!snapshot_still_restorable(&snap, &ArchiveMode::Plaintext), "mode mismatch");
    }

    #[test]
    fn rollback_moves_originals_and_their_wal_back() {
        let dir = tempdir().unwrap();
        let paths = project_paths_with_root(dir.path());
        ensure_paths(&paths).unwrap();
        let quarantine = paths.quarantine_dir.join(CRASH_TIMESTAMP);
        legacy_db(&quarantine.join("history-vault.sqlite"), None);
        fs::write(quarantine.join("history-vault.sqlite-wal"), b"hot wal").unwrap();
        legacy_db(&quarantine.join("source-evidence.sqlite"), None);
        save_config(&paths, &plaintext_config()).unwrap();

        rollback_restore(&paths, &journal(&paths, "gone.sqlite", &quarantine)).unwrap();
        assert!(paths.archive_database_path.exists());
        assert!(PathBuf::from(format!("{}-wal", paths.archive_database_path.display())).exists());
        assert!(paths.source_evidence_database_path.exists());
        assert!(!paths.config_path.exists(), "no config existed before the restore");
    }
}
