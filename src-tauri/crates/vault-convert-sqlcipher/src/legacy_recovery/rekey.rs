//! Responsibilities: finish or roll back a password change / mode switch of the old SQLCipher
//! format that a crash cut between the file swap and the config write.
//! Not responsible for: native rekeys (`vault_core::maintenance`), writing journals (the old
//! rekey flow is gone) or decrypting source evidence without the old key.
//! Failure modes: a config that no longer matches the canonical file's header (NOTADB on the
//! next open), silently committing Plaintext over a still-encrypted source evidence.
//! Tests: legacy_recovery::rekey::tests.
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf};
use vault_core::{
    AppConfig, ArchiveMode, DiskEncryptionMode, ProjectPaths,
    archive::ArchiveWriteLock,
    detect_disk_encryption_mode,
    durable_io::{atomic_durable_write, remove_file_durably},
    save_config,
};

/// Filename of the durable interrupted-rekey marker, placed beside the canonical archive
/// database. The `.pk-` prefix keeps retention / cleanup from sweeping it — the same
/// dotfile-skip contract the interrupted-import journal and the write-lock sentinel rely
/// on — so a crash-mid-rekey signal can never be deleted before recovery acts on it.
const REKEY_JOURNAL_FILE: &str = ".pk-rekey-journal.json";

/// The durable record of an in-flight rekey's swap+commit phase.
///
/// Written (durably) just BEFORE the canonical swap and removed (durably) AFTER the config
/// write, mirroring the interrupted-import journal. Its mere presence at archive-open time
/// SIGNALS that a crash MAY have cut the swap+commit window; its contents are exactly what
/// [`recover_interrupted_rekey`] needs to converge config to the file's real at-rest mode
/// (the swap landed) or roll config back to the captured pre-rekey state (the swap did not).
/// The file swap and the config write are NOT atomic with each other, so this marker — not
/// the filesystem alone — is the source of truth for "did the rekey commit?".
#[derive(Debug, Clone, Serialize, Deserialize)]
struct RekeyJournal {
    /// Schema version of this journal record (currently `1`).
    version: u32,
    /// The rekey's RFC 3339 start instant, for diagnostics / log correlation.
    timestamp: String,
    /// The at-rest mode the archive was in BEFORE the rekey (the rollback target).
    from_mode: ArchiveMode,
    /// The at-rest mode the rekey is converting the archive TO (the complete target).
    to_mode: ArchiveMode,
    /// Display path of the verified pre-rekey safety snapshot, named in fail-closed errors.
    snapshot_ref: String,
    /// The dest's `config.json` bytes pre-rekey (`None` when none existed), so a rollback
    /// restores the exact prior config rather than guessing.
    previous_config: Option<String>,
}

/// Path of the interrupted-rekey marker (beside the canonical archive database).
pub(super) fn rekey_journal_path(paths: &ProjectPaths) -> PathBuf {
    paths
        .archive_database_path
        .parent()
        .expect("archive database path has a parent directory")
        .join(REKEY_JOURNAL_FILE)
}

/// `true` when the durable interrupted-rekey marker is present beside the canonical archive —
/// a single `stat`, no lock. The launch-time fast path
/// ([`super::recover_on_launch`]) uses it to decide whether to take the gate +
/// flock at all, so the overwhelmingly common no-marker launch never touches a lock.
pub(super) fn interrupted_rekey_marker_present(paths: &ProjectPaths) -> bool {
    rekey_journal_path(paths).exists()
}

#[cfg(test)]
/// Durably writes `journal` to the marker path (atomic temp + F_FULLFSYNC + rename + dir
/// fsync), so its presence is itself durable before the swap runs.
fn write_rekey_journal(paths: &ProjectPaths, journal: &RekeyJournal) -> Result<()> {
    let bytes = serde_json::to_vec(journal).context("serializing the interrupted-rekey journal")?;
    atomic_durable_write(&rekey_journal_path(paths), &bytes)
        .context("writing the interrupted-rekey journal")
}

/// Reads the interrupted-rekey marker. `Ok(None)` when it is absent OR unparseable; the caller
/// decides what an unparseable one means ([`super::drop_unreadable_marker`]).
fn read_rekey_journal(paths: &ProjectPaths) -> Result<Option<RekeyJournal>> {
    let path = rekey_journal_path(paths);
    if !path.exists() {
        return Ok(None);
    }
    let bytes = fs::read(&path).with_context(|| format!("reading {}", path.display()))?;
    Ok(serde_json::from_slice::<RekeyJournal>(&bytes).ok())
}

/// Maps a configured [`ArchiveMode`] to the on-disk header a file in that mode presents.
///
fn disk_mode_for(mode: &ArchiveMode) -> DiskEncryptionMode {
    match mode {
        ArchiveMode::Plaintext => DiskEncryptionMode::Plaintext,
        ArchiveMode::Encrypted => DiskEncryptionMode::Encrypted,
    }
}

/// Crash-recovery twin of the old `rekey_swap_and_commit`'s in-band commit.
///
/// A NESTED recover-first helper, called from the launch-time `recover_archive_on_launch`
/// (which already holds the top-level gate + lock when it calls in). When an interrupted-rekey
/// marker is present a crash MAY have cut the swap+commit window, so it reconciles the
/// canonical archive's REAL on-disk at-rest mode against the journal — KEY-FREE — then clears
/// the marker. Returns `true` only when it actually recovered something.
///
/// Cheap in the common case — a single `stat` of the marker path, no lock taken — so it is
/// safe to call on every open. Only when the marker exists does it take the cross-process
/// [`ArchiveWriteLock`] (recovery rewrites config / removes orphaned canonical temps, which
/// must serialize against a concurrent scheduled backup) and re-read the journal (it may have
/// been cleared while we waited for the lock). It takes ONLY the reentrant flock, NEVER the
/// non-reentrant top-level [`vault_core::archive::ArchiveOpGate`]: re-acquiring the flock its caller already holds
/// yields a nested guard (no self-deadlock), while taking the gate its caller already holds
/// WOULD self-deadlock — so this nested helper must never touch it.
pub(super) fn recover_interrupted_rekey(paths: &ProjectPaths) -> Result<bool> {
    if !rekey_journal_path(paths).exists() {
        return Ok(false);
    }
    let _lock = ArchiveWriteLock::acquire(paths)
        .context("acquiring the archive write lock for interrupted-rekey recovery")?;
    let Some(journal) = read_rekey_journal(paths)? else {
        // Present (checked above) but unreadable.
        super::drop_unreadable_marker(paths, &rekey_journal_path(paths))?;
        return Ok(false);
    };
    resolve_interrupted_rekey(paths, &journal)?;
    remove_file_durably(&rekey_journal_path(paths))
        .context("clearing the rekey marker after recovery")?;
    Ok(true)
}

/// Reconciles an interrupted rekey KEY-FREE, treating the canonical archive's on-disk header
/// as the authority (`install_file_durably` is ATOMIC, so the file is EITHER the whole old
/// `from_mode` file OR the whole new `to_mode` file, never a partial mix).
fn resolve_interrupted_rekey(paths: &ProjectPaths, journal: &RekeyJournal) -> Result<()> {
    let history = detect_disk_encryption_mode(&paths.archive_database_path);
    if history == DiskEncryptionMode::Absent {
        // FAIL-CLOSED: the canonical archive is missing/unreadable, so its at-rest mode cannot
        // be confirmed and we must not guess a config that could brick the open. Leave the
        // marker (the caller does not clear on Err) and name the snapshot to restore from.
        anyhow::bail!(
            "interrupted-rekey recovery cannot proceed: the canonical history-vault {} is \
             missing or unreadable, so its at-rest mode cannot be confirmed. Restore it from the \
             rekey safety snapshot at {} before retrying.",
            paths.archive_database_path.display(),
            journal.snapshot_ref,
        );
    }
    let to_disk = disk_mode_for(&journal.to_mode);
    let from_disk = disk_mode_for(&journal.from_mode);
    let mode_changed = journal.from_mode != journal.to_mode;
    if mode_changed && history == to_disk {
        // (a) The swap COMPLETED: the new `to_mode` file is canonical, only config lags.
        // Converging config to `to_mode` un-bricks the CANONICAL archive (history-vault) — config
        // now matches its header, so no NOTADB.
        //
        // FAIL-CLOSED in the DECRYPT direction (mirrors migration.rs's
        // `ensure_recovered_modes_are_consistent`): source-evidence is converted in lockstep AFTER
        // the history-vault swap (step 7 of `rekey_swap_and_commit`), so a crash in this window can
        // leave source-evidence STILL Encrypted while the history-vault is already Plaintext.
        // Completing `to_mode == Plaintext` here would commit a Plaintext config and CLEAR the
        // marker, but that still-Encrypted source-evidence is decryptable only with the now-
        // unprompted OLD key we do not hold at this keyless launch — silently committing Plaintext
        // would bury the drift as recurring backup failures. So if we are about to commit Plaintext
        // while source-evidence is still Encrypted, REFUSE: leave the marker (the caller keeps it on
        // Err) so the launch surfaces it as an unrecoverable state for the recovery GUI. We do NOT
        // attempt the keyed decrypt (the deferred decrypt-direction follow-up). The ENCRYPT
        // direction (`to_mode == Encrypted`) is unaffected: a plaintext source-evidence self-heals
        // on the next KEYED open via the on-unlock `reconcile_archive_encryption`.
        if journal.to_mode == ArchiveMode::Plaintext
            && detect_disk_encryption_mode(&paths.source_evidence_database_path)
                == DiskEncryptionMode::Encrypted
        {
            anyhow::bail!(
                "interrupted-rekey recovery cannot complete the decrypt direction: the \
                 history-vault swapped to Plaintext but source-evidence {} is still Encrypted, \
                 which can only be decrypted with the now-unprompted old key. Committing a \
                 Plaintext config would leave a config↔source-evidence drift that surfaces only as \
                 repeated backup failures, so the rekey marker is left in place to keep the state \
                 flagged and retry-able. Restore from the rekey safety snapshot at {} (or re-run \
                 the rekey with the old key) before retrying.",
                paths.source_evidence_database_path.display(),
                journal.snapshot_ref,
            );
        }
        complete_rekey_config(paths, journal)
    } else if mode_changed && history == from_disk {
        // (b) The swap did NOT land: the original `from_mode` file is still canonical, so roll
        // config back to the captured pre-rekey state and drop the orphaned export temp.
        rollback_rekey(paths, journal)
    } else {
        // (c) A same-mode key rotation (`from == to`): the at-rest MODE is already consistent
        // with either config, mode-drift cannot brick the open, and key drift is unobservable
        // key-free + out of scope — so converge config to `to_mode` and clear the marker.
        complete_rekey_config(paths, journal)
    }
}

/// Commits a landed rekey's config: bases on the journal's captured pre-rekey config, forces
/// `archive_mode = to_mode` + `initialized = true`, and persists it durably.
fn complete_rekey_config(paths: &ProjectPaths, journal: &RekeyJournal) -> Result<()> {
    let mut config = base_config_for_recovery(paths, journal);
    config.archive_mode = journal.to_mode.clone();
    config.initialized = true;
    save_config(paths, &config)
}

/// Reconstructs the config a rekey recovery should base on: the journal's captured pre-rekey
/// config when it parses, else the dest's current config, else defaults. Mirrors the
/// `apply_import` recovery's "honour the captured config, fall back to the live one" rule so
/// non-archive user preferences survive a rekey recovery.
fn base_config_for_recovery(paths: &ProjectPaths, journal: &RekeyJournal) -> AppConfig {
    match journal.previous_config.as_deref().map(serde_json::from_str::<AppConfig>) {
        Some(Ok(config)) => config,
        _ => vault_core::load_config(paths).unwrap_or_default(),
    }
}

/// Rolls back an un-landed rekey: the `from_mode` original is intact (the atomic swap never
/// replaced it), so drop the orphaned export temp + its sidecars and restore config to the
/// captured pre-rekey bytes (`None` = none existed pre-rekey).
fn rollback_rekey(paths: &ProjectPaths, journal: &RekeyJournal) -> Result<()> {
    let temp_path = paths.archive_database_path.with_extension("rekey.sqlite");
    let _ = fs::remove_file(&temp_path);
    super::remove_stale_sidecars(&temp_path);
    match &journal.previous_config {
        Some(bytes) => atomic_durable_write(&paths.config_path, bytes.as_bytes())
            .context("restoring the pre-rekey config")?,
        None => {
            let _ = remove_file_durably(&paths.config_path);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;
    use tempfile::tempdir;
    use vault_core::project_paths_with_root;

    fn plaintext_archive_config() -> AppConfig {
        AppConfig {
            archive_mode: ArchiveMode::Plaintext,
            initialized: true,
            ..AppConfig::default()
        }
    }

    fn encrypted_archive_config() -> AppConfig {
        AppConfig {
            archive_mode: ArchiveMode::Encrypted,
            initialized: true,
            ..AppConfig::default()
        }
    }

    /// Writes a file whose 16-byte header makes `detect_disk_encryption_mode` classify it as
    /// plaintext, without needing a real SQLite database (the recovery is header-only).
    fn write_plaintext_db(path: &Path) {
        fs::create_dir_all(path.parent().expect("archive parent")).expect("archive dir");
        fs::write(path, b"SQLite format 3\0plaintext body").expect("write plaintext db");
    }

    /// Writes a file whose header makes `detect_disk_encryption_mode` classify it as encrypted.
    fn write_encrypted_db(path: &Path) {
        fs::create_dir_all(path.parent().expect("archive parent")).expect("archive dir");
        fs::write(path, [7u8; 32]).expect("write encrypted db");
    }

    fn sample_journal(
        from: ArchiveMode,
        to: ArchiveMode,
        previous_config: Option<String>,
    ) -> RekeyJournal {
        RekeyJournal {
            version: 1,
            timestamp: "2026-06-30T00-00-00Z".to_string(),
            from_mode: from,
            to_mode: to,
            snapshot_ref: "/snapshots/rekey/before.sqlite".to_string(),
            previous_config,
        }
    }

    #[test]
    fn a_corrupt_rekey_marker_goes_only_when_the_archive_is_consistent() {
        let dir = tempdir().expect("tempdir");
        let paths = project_paths_with_root(dir.path());
        fs::create_dir_all(paths.archive_database_path.parent().expect("archive parent"))
            .expect("archive dir");
        fs::write(rekey_journal_path(&paths), b"not valid json").expect("seed corrupt marker");

        let recovered =
            recover_interrupted_rekey(&paths).expect("a corrupt marker must not error recovery");
        assert!(!recovered, "a corrupt marker cannot drive a recovery");
        assert!(
            !rekey_journal_path(&paths).exists(),
            "the unactionable corrupt marker must be removed",
        );

        // The decrypt direction cut after the history-vault swap: nothing can say which way to go.
        super::super::fixtures::plaintext_header(&paths.archive_database_path);
        super::super::fixtures::encrypted_header(&paths.source_evidence_database_path);
        fs::write(rekey_journal_path(&paths), b"not valid json").expect("seed corrupt marker");
        assert!(recover_interrupted_rekey(&paths).is_err(), "a mixed archive fails closed");
        assert!(rekey_journal_path(&paths).exists(), "the marker stays to keep the state flagged");
    }

    #[test]
    fn recover_interrupted_rekey_same_mode_rotation_converges_config() {
        let dir = tempdir().expect("tempdir");
        let paths = project_paths_with_root(dir.path());
        write_encrypted_db(&paths.archive_database_path);
        let prev = serde_json::to_string(&encrypted_archive_config()).expect("config json");
        // from == to == Encrypted: a key rotation, not a mode change -> the complete arm via the
        // `Some(Ok)` base-config branch.
        write_rekey_journal(
            &paths,
            &sample_journal(ArchiveMode::Encrypted, ArchiveMode::Encrypted, Some(prev)),
        )
        .expect("write marker");

        assert!(recover_interrupted_rekey(&paths).expect("recover"));
        let config = vault_core::load_config(&paths).expect("load");
        assert!(matches!(config.archive_mode, ArchiveMode::Encrypted));
        assert!(!rekey_journal_path(&paths).exists());
    }

    #[test]
    fn recover_interrupted_rekey_rolls_back_an_unlanded_swap() {
        let dir = tempdir().expect("tempdir");
        let paths = project_paths_with_root(dir.path());
        // history-vault is still at the OLD (plaintext) at-rest mode: the swap never landed.
        write_plaintext_db(&paths.archive_database_path);
        // An orphaned export temp + sidecar the interrupted rekey left behind.
        let temp = paths.archive_database_path.with_extension("rekey.sqlite");
        fs::write(&temp, b"stale export").expect("seed temp");
        let temp_wal = PathBuf::from(format!("{}-wal", temp.display()));
        fs::write(&temp_wal, b"stale wal").expect("seed temp wal");
        // The captured pre-rekey config rollback must restore, with config drifted to the NEW
        // mode on disk so the restore is observable.
        let prev = serde_json::to_string(&plaintext_archive_config()).expect("config json");
        save_config(&paths, &encrypted_archive_config()).expect("seed drifted config");
        write_rekey_journal(
            &paths,
            &sample_journal(ArchiveMode::Plaintext, ArchiveMode::Encrypted, Some(prev.clone())),
        )
        .expect("write marker");

        assert!(recover_interrupted_rekey(&paths).expect("recover"));
        assert!(!rekey_journal_path(&paths).exists());
        assert!(!temp.exists(), "the orphaned export temp must be removed");
        assert!(!temp_wal.exists(), "its sidecar must be removed too");
        assert_eq!(
            fs::read_to_string(&paths.config_path).expect("read restored config"),
            prev,
            "rollback must restore the captured pre-rekey config verbatim",
        );
    }
}
