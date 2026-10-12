//! Responsibilities: roll back a whole-app import of the old SQLCipher format that a crash cut
//! mid-commit, so the archive converts from its consistent pre-import state.
//! Not responsible for: native imports (their journal carries a `stage`; see
//! `vault_core::maintenance::install`) or writing journals (the old import flow is gone).
//! Failure modes: a NEW `-wal` replayed into a restored OLD main, dropping an OLD hot `-wal`,
//! committing one config over canonical databases left at different at-rest modes.
//! Tests: legacy_recovery::import::tests.
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
};
use vault_core::{
    AppConfig, ArchiveMode, DiskEncryptionMode, ProjectPaths,
    archive::ArchiveWriteLock,
    detect_disk_encryption_mode,
    durable_io::{atomic_durable_write, remove_file_durably},
};

/// Filename of the durable interrupted-import marker, placed beside the canonical
/// archive database. The `.pk-` prefix keeps retention / cleanup from deleting it
/// (the same dotfile-skip contract the archive write-lock sentinel relies on), so a
/// crash-mid-commit signal can never be swept away before recovery acts on it.
const IMPORT_JOURNAL_FILE: &str = ".pk-import-journal.json";

/// One canonical-DB or subtree path the import is about to replace, plus whether a
/// previous version existed there before the swap — so rollback knows whether to
/// restore a `.bak` or simply delete a fresh install.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ImportJournalEntry {
    target: PathBuf,
    had_previous: bool,
}

/// The durable record of an in-flight import's commit phase.
///
/// Written (durably) BEFORE the first swap and removed (durably) AFTER the config
/// write. Its mere presence at archive-open time SIGNALS that a crash cut the commit
/// phase; its contents are exactly what [`rollback_import`] needs to restore the
/// consistent pre-import state. The two canonical renames are NOT atomic, so this
/// marker — not the filesystem — is the source of truth for "did the import commit?".
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ImportJournal {
    /// Schema version of this journal record (currently `1`).
    version: u32,
    /// The `.bak-<ts>` suffix the commit phase used, so rollback finds the backups.
    timestamp: String,
    /// Canonical databases (history-vault, source-evidence) in install order.
    canonical: Vec<ImportJournalEntry>,
    /// Rebuildable subtrees (derived/audit/raw-snapshots/sidecars).
    subtrees: Vec<ImportJournalEntry>,
    /// The dest's `config.json` bytes pre-import (`None` when none existed), so a
    /// rollback restores the exact prior config rather than guessing.
    previous_config: Option<String>,
}

/// Path of the interrupted-import marker (beside the canonical archive database).
///
/// `pub(crate)` so the Phase-D full-archive restore can quarantine a superseded import marker
/// alongside the broken canonical files (a stale marker must not drive a confusing recovery on
/// the next launch after a one-click restore has already healed the archive).
pub(super) fn import_journal_path(paths: &ProjectPaths) -> PathBuf {
    paths
        .archive_database_path
        .parent()
        .expect("archive database path has a parent directory")
        .join(IMPORT_JOURNAL_FILE)
}

/// `true` when the durable interrupted-import marker is present beside the canonical archive —
/// a single `stat`, no lock. The launch-time fast path
/// ([`super::recover_on_launch`]) uses it to decide whether to take the gate +
/// flock at all, so the overwhelmingly common no-marker launch never touches a lock.
pub(super) fn interrupted_import_marker_present(paths: &ProjectPaths) -> bool {
    import_journal_path(paths).exists()
}

#[cfg(test)]
/// Durably writes `journal` to the marker path (atomic temp + F_FULLFSYNC + rename
/// + dir fsync), so its presence is itself durable before the first swap runs.
fn write_import_journal(paths: &ProjectPaths, journal: &ImportJournal) -> Result<()> {
    let bytes =
        serde_json::to_vec(journal).context("serializing the interrupted-import journal")?;
    atomic_durable_write(&import_journal_path(paths), &bytes)
        .context("writing the interrupted-import journal")
}

/// Reads the interrupted-import marker. `Ok(None)` when it is absent OR unparseable; the caller
/// decides what an unparseable one means ([`super::drop_unreadable_marker`]).
fn read_import_journal(paths: &ProjectPaths) -> Result<Option<ImportJournal>> {
    let path = import_journal_path(paths);
    if !path.exists() {
        return Ok(None);
    }
    let bytes = fs::read(&path).with_context(|| format!("reading {}", path.display()))?;
    Ok(serde_json::from_slice::<ImportJournal>(&bytes).ok())
}

/// Removes `path` whether it is a file or a directory; a no-op when absent. Shared by
/// [`rollback_import`] to clear a canonical DB file or a rebuildable subtree.
fn remove_path_if_exists(path: &Path) -> Result<()> {
    if !path.exists() {
        return Ok(());
    }
    if path.is_dir() {
        fs::remove_dir_all(path).with_context(|| format!("removing directory {}", path.display()))
    } else {
        fs::remove_file(path).with_context(|| format!("removing {}", path.display()))
    }
}

/// Moves a preserved `.bak` unit's `-wal`/`-shm`/`-journal` sidecars back onto `target`
/// (each only if present at `<bak><suffix>`). Idempotent: a re-run after some sidecars
/// already moved finds those `<bak>` sidecars gone and leaves the already-restored
/// `<target>` ones in place. Used by [`rollback_import`] to restore the OLD database's
/// hot WAL and to FINISH an interrupted restore.
fn restore_bak_sidecars(bak: &Path, target: &Path) -> Result<()> {
    for suffix in ["-wal", "-shm", "-journal"] {
        let bak_sidecar = PathBuf::from(format!("{}{}", bak.display(), suffix));
        if bak_sidecar.exists() {
            let target_sidecar = PathBuf::from(format!("{}{}", target.display(), suffix));
            fs::rename(&bak_sidecar, &target_sidecar).with_context(|| {
                format!(
                    "restoring sidecar {} from {}",
                    target_sidecar.display(),
                    bak_sidecar.display()
                )
            })?;
        }
    }
    Ok(())
}

/// Undoes a partially-applied import, restoring the consistent pre-import state.
///
/// The SINGLE undo implementation shared by `apply_import`'s in-band rollback (a
/// returned error mid-commit) and [`recover_interrupted_import`] (a crash mid-commit
/// detected at next open), walking the SAME journal both produce so the two paths can
/// never diverge. Idempotent even across a crash WITHIN recovery: the OLD main is
/// renamed back FIRST and its `.bak` sidecars are moved AFTER, with no barrier between,
/// so a re-run that finds the main `.bak` already consumed but its sidecars still
/// orphaned takes the "orphaned sidecar" arm and just finishes the sidecar move — never
/// dropping a just-restored OLD `-wal`.
fn rollback_import(paths: &ProjectPaths, journal: &ImportJournal) -> Result<()> {
    for entry in journal.canonical.iter().chain(journal.subtrees.iter()) {
        let target = entry.target.as_path();
        let bak = backup_sidecar_path(target, &journal.timestamp);
        let bak_sidecar_present = ["-wal", "-shm", "-journal"]
            .iter()
            .any(|suffix| PathBuf::from(format!("{}{}", bak.display(), suffix)).exists());
        if bak.exists() {
            // The swap had started: scrub any half-installed NEW sidecars first (so a
            // NEW `-wal` cannot replay into the restored OLD main), drop
            // whatever sits at the target, restore the OLD main, THEN move its `.bak`
            // sidecars across. The main rename is the commit point of the restore.
            super::remove_stale_sidecars(target);
            remove_path_if_exists(target)?;
            fs::rename(&bak, target).with_context(|| {
                format!("restoring {} from {}", target.display(), bak.display())
            })?;
            restore_bak_sidecars(&bak, target)?;
        } else if bak_sidecar_present {
            // An INTERRUPTED restore: a previous rollback pass already renamed the OLD
            // main back (the main `.bak` is gone) but crashed before moving its
            // sidecars, which are still orphaned at `<bak>-*`. Finish the move — and do
            // NOT scrub here, so the just-restored OLD `<target>-wal` (if some sidecars
            // already moved) is never deleted. This keeps recovery idempotent across a
            // crash WITHIN the restore.
            restore_bak_sidecars(&bak, target)?;
        } else if !entry.had_previous {
            // A fresh install with no prior version: scrub the NEW sidecars (so a NEW
            // `-wal` can't later replay into a fresh-created DB) and remove the NEW main
            // — pre-import there was nothing here.
            super::remove_stale_sidecars(target);
            remove_path_if_exists(target)?;
        }
        // else (had_previous but no `.bak` unit at all): the swap never started for this
        // entry, so the still-original target — INCLUDING its possibly-hot OLD `-wal`/
        // `-shm`/`-journal` (committed-but-uncheckpointed frames) — is left
        // ENTIRELY untouched. Scrubbing here would silently drop committed rows from a DB
        // we are NOT replacing.
    }
    // FAIL-CLOSED (defense in depth): the "leave untouched" branch above assumes a
    // not-yet-swapped canonical DB is unchanged since the crash. That holds ONLY while no
    // destructive op rewrites the archive in the unrecovered window. If one did (the
    // rekey-on-a-half-import hazard: a plaintext archive skips the encryption-gated launch
    // reconcile, so a rekey can rewrite ONE canonical DB's at-rest mode while the other is
    // restored from a stale-mode `.bak`), the two canonical DBs can end at DIFFERENT modes
    // that no single config can serve. Committing `previous_config` over that mix is a
    // silent config↔source-evidence brick. Detect it here from the REAL on-disk headers and
    // refuse — leaving the marker (the caller keeps it on Err) and surfacing a recoverable
    // error so the state stays flagged + retry-able rather than silently bricked.
    ensure_recovered_modes_are_consistent(paths, journal)?;
    // Restore config to its pre-import bytes (None = none existed pre-import).
    match &journal.previous_config {
        Some(bytes) => atomic_durable_write(&paths.config_path, bytes.as_bytes())
            .context("restoring the pre-import config")?,
        None => {
            let _ = remove_file_durably(&paths.config_path);
        }
    }
    Ok(())
}

/// Maps a configured [`ArchiveMode`] to the on-disk header a file in that mode must
/// present, so a restored canonical DB's real header can be compared to the config the
/// recovery is about to write.
fn disk_mode_for(mode: &ArchiveMode) -> DiskEncryptionMode {
    match mode {
        ArchiveMode::Plaintext => DiskEncryptionMode::Plaintext,
        ArchiveMode::Encrypted => DiskEncryptionMode::Encrypted,
    }
}

/// Fail-closed guard run by [`rollback_import`] AFTER it has restored the canonical
/// databases but BEFORE it commits the recovered config.
///
/// A single `config.json` declares ONE `archive_mode`, so it can correctly serve the
/// archive only if BOTH canonical databases are at that one at-rest mode. This re-reads
/// each canonical DB's ACTUAL on-disk header (`detect_disk_encryption_mode`) and refuses
/// when a PRESENT database disagrees with the mode the recovery is about to commit — the
/// rekey-on-a-half-import signature (history-vault restored to Plaintext while the
/// untouched source-evidence is now Encrypted, or vice versa). The required mode is the
/// pre-import config's when the journal carries one (that IS the config about to be
/// written), else inferred from whichever canonical DB is present. An `Absent` database
/// is fine: the next open recreates it in the final-config mode. Returning `Err` here
/// leaves the marker in place (the caller does not clear it on Err), turning a silent
/// permanent brick into a loud, retry-able failure.
fn ensure_recovered_modes_are_consistent(
    paths: &ProjectPaths,
    journal: &ImportJournal,
) -> Result<()> {
    let history = detect_disk_encryption_mode(&paths.archive_database_path);
    let evidence = detect_disk_encryption_mode(&paths.source_evidence_database_path);
    // The single at-rest mode the recovered archive must converge to.
    let required = journal
        .previous_config
        .as_deref()
        .and_then(|bytes| serde_json::from_str::<AppConfig>(bytes).ok())
        .map(|config| disk_mode_for(&config.archive_mode))
        .or_else(|| {
            [history, evidence].into_iter().find(|mode| *mode != DiskEncryptionMode::Absent)
        });
    let Some(required) = required else {
        // Nothing installed and no config to honour — a clean empty state, nothing to drift.
        return Ok(());
    };
    for (label, mode) in [("history-vault.sqlite", history), ("source-evidence.sqlite", evidence)] {
        if mode != DiskEncryptionMode::Absent && mode != required {
            anyhow::bail!(
                concat!(
                    "interrupted-import recovery cannot reach a single consistent at-rest mode: {} ",
                    "is {:?} but the recovered archive must be {:?}. The canonical databases drifted ",
                    "apart (most likely a rekey ran on a half-applied import before recovery) and no ",
                    "single config can serve both, so the interrupted-import marker is left in place ",
                    "to keep the state flagged and retry-able rather than committing a silently ",
                    "mode-drifted config that would brick the next open.",
                ),
                label,
                mode,
                required,
            );
        }
    }
    Ok(())
}

/// Crash-recovery twin of the old `apply_import`'s in-band rollback.
///
/// Called from the launch recovery ([`super::recover_on_launch`]), which already holds the
/// top-level gate + lock. When an interrupted-import marker is present a crash cut a commit
/// phase, so it restores the archive to the consistent PRE-IMPORT state over the journal, then
/// clears the marker. Returns `true` only when it actually recovered something.
///
/// Only when the marker exists does it take the cross-process [`ArchiveWriteLock`] and re-read
/// the journal (it may have been cleared while we waited for the lock). It takes ONLY the
/// reentrant flock, never the non-reentrant top-level gate its caller already holds.
pub(super) fn recover_interrupted_import(paths: &ProjectPaths) -> Result<bool> {
    if !import_journal_path(paths).exists() {
        return Ok(false);
    }
    let _write_lock = ArchiveWriteLock::acquire(paths)
        .context("acquiring the archive write lock for interrupted-import recovery")?;
    let Some(journal) = read_import_journal(paths)? else {
        // Present (checked above) but unreadable.
        super::drop_unreadable_marker(paths, &import_journal_path(paths))?;
        return Ok(false);
    };
    rollback_import(paths, &journal)?;
    remove_file_durably(&import_journal_path(paths))
        .context("clearing the interrupted-import marker after recovery")?;
    Ok(true)
}

fn backup_sidecar_path(target: &Path, timestamp: &str) -> PathBuf {
    let file_name = target
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "pathkeep".to_string());
    let mut sidecar_name = file_name;
    sidecar_name.push_str(&format!(".bak-{timestamp}"));
    target.with_file_name(sidecar_name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use vault_core::project_paths_with_root;

    fn fresh_paths() -> (TempDir, ProjectPaths) {
        let dir = TempDir::new().expect("tempdir");
        let paths = project_paths_with_root(dir.path());
        (dir, paths)
    }

    /// Creates `app_root/archive` so a hand-built journal can be written beside the
    /// (possibly not-yet-existent) archive database.
    fn ensure_archive_dir(paths: &ProjectPaths) {
        fs::create_dir_all(paths.archive_database_path.parent().expect("archive parent"))
            .expect("archive dir");
    }

    #[test]
    fn recover_interrupted_import_without_a_marker_is_a_noop() {
        let (_dir, paths) = fresh_paths();
        assert!(
            !recover_interrupted_import(&paths).expect("no-marker recovery must not error"),
            "a missing marker must be a cheap no-op",
        );
    }

    #[test]
    fn a_corrupt_import_marker_goes_only_when_the_archive_is_consistent() {
        let (_dir, paths) = fresh_paths();
        ensure_archive_dir(&paths);
        fs::write(import_journal_path(&paths), b"not valid json at all")
            .expect("seed corrupt marker");
        assert!(import_journal_path(&paths).exists());

        let recovered = recover_interrupted_import(&paths)
            .expect("a corrupt marker must not error the recovery");
        assert!(!recovered, "a corrupt marker cannot drive a rollback");
        assert!(
            !import_journal_path(&paths).exists(),
            "the unactionable corrupt marker must be removed"
        );

        // A cut commit that left the two files in different modes: a mixed archive.
        super::super::fixtures::encrypted_header(&paths.archive_database_path);
        super::super::fixtures::plaintext_header(&paths.source_evidence_database_path);
        fs::write(import_journal_path(&paths), b"not valid json at all").expect("seed marker");
        assert!(recover_interrupted_import(&paths).is_err(), "a mixed archive fails closed");
        assert!(import_journal_path(&paths).exists(), "the marker stays to keep the state flagged");
    }

    #[test]
    fn recover_interrupted_import_restores_a_preserved_bak_unit_over_the_target() {
        let (_dir, paths) = fresh_paths();
        ensure_archive_dir(&paths);
        let target = paths.archive_database_path.clone();
        let ts = "2026-06-30T00-00-00Z";
        // The cut commit left a NEW file at the target and the preserved OLD unit at `.bak`
        // (with a `-wal` sidecar, so the `.bak` is a complete unit).
        fs::write(&target, b"NEW half-installed db").expect("seed new target");
        let bak = backup_sidecar_path(&target, ts);
        fs::write(&bak, b"OLD db bytes").expect("seed bak");
        fs::write(PathBuf::from(format!("{}-wal", bak.display())), b"OLD hot wal")
            .expect("seed bak wal");
        let journal = ImportJournal {
            version: 1,
            timestamp: ts.to_string(),
            canonical: vec![ImportJournalEntry { target: target.clone(), had_previous: true }],
            subtrees: Vec::new(),
            previous_config: None, // none existed pre-import => config must be absent after.
        };
        write_import_journal(&paths, &journal).expect("write journal");

        assert!(
            recover_interrupted_import(&paths).expect("recover"),
            "a present marker must recover"
        );
        assert!(!import_journal_path(&paths).exists(), "the marker must be cleared after recovery");
        assert_eq!(
            fs::read(&target).expect("read restored target"),
            b"OLD db bytes",
            "the OLD .bak unit must be restored over the target",
        );
        assert!(
            PathBuf::from(format!("{}-wal", target.display())).exists(),
            "the OLD unit's -wal must be restored beside the target",
        );
        assert!(!bak.exists(), "the .bak must be consumed by the restore");
        assert!(!paths.config_path.exists(), "a None previous_config must leave config absent");
    }

    #[test]
    fn recover_interrupted_import_removes_a_fresh_install_with_no_prior_and_restores_config() {
        let (_dir, paths) = fresh_paths();
        ensure_archive_dir(&paths);
        let target = paths.archive_database_path.clone();
        fs::write(&target, b"freshly installed, no prior").expect("seed fresh target");
        let ts = "2026-06-30T01-00-00Z";
        let prior_config_bytes =
            serde_json::to_string_pretty(&AppConfig::default()).expect("config");
        let journal = ImportJournal {
            version: 1,
            timestamp: ts.to_string(),
            canonical: vec![
                // had_previous=false, target exists => removed (a fresh install).
                ImportJournalEntry { target: target.clone(), had_previous: false },
                // had_previous=false, target ABSENT => remove_path_if_exists is a clean no-op.
                ImportJournalEntry {
                    target: target.with_file_name("absent.sqlite"),
                    had_previous: false,
                },
            ],
            subtrees: Vec::new(),
            previous_config: Some(prior_config_bytes.clone()),
        };
        write_import_journal(&paths, &journal).expect("write journal");

        assert!(recover_interrupted_import(&paths).expect("recover"));
        assert!(!target.exists(), "a fresh install with no prior must be removed on rollback");
        assert!(!import_journal_path(&paths).exists(), "the marker must be cleared");
        assert_eq!(
            fs::read_to_string(&paths.config_path).expect("read restored config"),
            prior_config_bytes,
            "a Some previous_config must be restored verbatim",
        );
    }

    #[test]
    fn recover_interrupted_import_removes_a_fresh_subtree_install() {
        let (_dir, paths) = fresh_paths();
        ensure_archive_dir(&paths);
        let target = paths.app_root.join("derived");
        fs::create_dir_all(&target).expect("seed fresh subtree");
        fs::write(target.join("new.txt"), b"new").expect("seed subtree content");
        let ts = "2026-06-30T02-00-00Z";
        let journal = ImportJournal {
            version: 1,
            timestamp: ts.to_string(),
            canonical: Vec::new(),
            // had_previous=false, dir target exists => remove_dir_all branch of remove_path_if_exists.
            subtrees: vec![ImportJournalEntry { target: target.clone(), had_previous: false }],
            previous_config: None,
        };
        write_import_journal(&paths, &journal).expect("write journal");

        assert!(recover_interrupted_import(&paths).expect("recover"));
        assert!(
            !target.exists(),
            "a fresh subtree install with no prior must be removed (dir branch)"
        );
    }

    #[test]
    fn write_import_journal_errors_when_the_archive_dir_is_missing() {
        // atomic_durable_write cannot create a sibling temp without the parent dir.
        let (_dir, paths) = fresh_paths();
        let journal = ImportJournal {
            version: 1,
            timestamp: "ts".to_string(),
            canonical: Vec::new(),
            subtrees: Vec::new(),
            previous_config: None,
        };
        let err = write_import_journal(&paths, &journal)
            .expect_err("writing the journal without an archive dir must error");
        assert!(format!("{err:#}").contains("interrupted-import journal"), "got {err:#}");
    }

    #[test]
    fn recover_interrupted_import_finishes_an_interrupted_sidecar_restore() {
        // A rollback that crashed AFTER renaming the OLD main back but BEFORE moving
        // its `.bak` sidecars leaves the main `.bak` consumed and `<bak>-<ts>-wal` orphaned.
        // A re-run must FINISH the sidecar move (not strip the just-restored OLD wal), so the
        // restored DB keeps its committed-but-uncheckpointed frames.
        let (_dir, paths) = fresh_paths();
        ensure_archive_dir(&paths);
        let target = paths.archive_database_path.clone();
        let ts = "2026-06-30T03-00-00Z";
        // Mid-recovery state: the OLD main is ALREADY back at the target, the main `.bak` is
        // gone, and only its `-wal` sidecar is still orphaned at `<bak>-<ts>-wal`.
        fs::write(&target, b"OLD main already restored").expect("seed restored main");
        let bak = backup_sidecar_path(&target, ts);
        let bak_wal = PathBuf::from(format!("{}-wal", bak.display()));
        fs::write(&bak_wal, b"OLD orphaned wal").expect("seed orphaned bak wal");
        let journal = ImportJournal {
            version: 1,
            timestamp: ts.to_string(),
            canonical: vec![ImportJournalEntry { target: target.clone(), had_previous: true }],
            subtrees: Vec::new(),
            previous_config: None,
        };
        write_import_journal(&paths, &journal).expect("write journal");

        assert!(recover_interrupted_import(&paths).expect("recover"));
        assert!(!import_journal_path(&paths).exists(), "the marker must be cleared after recovery");
        let target_wal = PathBuf::from(format!("{}-wal", target.display()));
        assert!(
            target_wal.exists(),
            "the orphaned OLD `-wal` must be moved onto the restored target, not dropped",
        );
        assert_eq!(fs::read(&target_wal).expect("read restored wal"), b"OLD orphaned wal");
        assert!(
            !bak_wal.exists(),
            "the orphaned bak `-wal` must be consumed by the finish-restore"
        );
        assert_eq!(
            fs::read(&target).expect("read main"),
            b"OLD main already restored",
            "the already-restored OLD main must be left untouched",
        );
    }

    #[test]
    fn recover_interrupted_import_fails_closed_on_mutually_inconsistent_canonical_modes() {
        // FIX (defense in depth): even if a destructive op rewrote one canonical DB's at-rest
        // mode in the unrecovered window (so the rollback's "leave untouched" branch leaves the
        // two canonical DBs at DIFFERENT modes that no single config can serve), recovery must
        // NOT silently remove the marker and commit a mode-drifted config. It must restore what
        // it can, then refuse — leaving the marker + surfacing a recoverable error.
        let (_dir, paths) = fresh_paths();
        ensure_archive_dir(&paths);
        let ts = "2026-06-30T09-00-00Z";

        // history-vault: a NEW (encrypted-looking) half-install over a PLAINTEXT `.bak`, so the
        // rollback restores it to plaintext.
        let history = paths.archive_database_path.clone();
        fs::write(&history, [9u8; 32]).expect("seed NEW encrypted-looking history-vault");
        let history_bak = backup_sidecar_path(&history, ts);
        fs::write(&history_bak, b"SQLite format 3\0OLD plaintext history")
            .expect("seed plaintext bak");

        // source-evidence: PRESENT and ENCRYPTED with NO `.bak`, so the rollback's
        // "had_previous, no `.bak` -> leave untouched" branch keeps it encrypted.
        let evidence = paths.source_evidence_database_path.clone();
        fs::write(&evidence, [7u8; 32]).expect("seed encrypted-looking source-evidence");

        let journal = ImportJournal {
            version: 1,
            timestamp: ts.to_string(),
            canonical: vec![
                ImportJournalEntry { target: history.clone(), had_previous: true },
                ImportJournalEntry { target: evidence.clone(), had_previous: true },
            ],
            subtrees: Vec::new(),
            previous_config: None,
        };
        write_import_journal(&paths, &journal).expect("write journal");

        let err = recover_interrupted_import(&paths)
            .expect_err("recovery must fail closed on an unreconcilable canonical-DB mode drift");
        let rendered = format!("{err:#}");
        assert!(
            rendered.contains("single consistent at-rest mode")
                && rendered.contains("source-evidence.sqlite"),
            "the error must name the unreconcilable drift, got: {rendered}",
        );

        // The history-vault `.bak` was still restored (rollback is best-effort up to the
        // refusal), but the marker is LEFT so the state stays flagged + retry-able and NO
        // drifted config is committed.
        assert_eq!(
            detect_disk_encryption_mode(&history),
            DiskEncryptionMode::Plaintext,
            "history-vault is restored to its plaintext .bak",
        );
        assert_eq!(
            detect_disk_encryption_mode(&evidence),
            DiskEncryptionMode::Encrypted,
            "source-evidence stays encrypted (the irreconcilable half)",
        );
        assert!(
            import_journal_path(&paths).exists(),
            "a fail-closed recovery must LEAVE the marker, never silently brick + clear it",
        );
        assert!(!paths.config_path.exists(), "no drifted config may be committed on the refusal");
    }

    #[test]
    fn disk_mode_for_maps_each_archive_mode() {
        // Both arms of the config-mode -> on-disk-header mapping the fail-closed guard relies on.
        assert_eq!(disk_mode_for(&ArchiveMode::Plaintext), DiskEncryptionMode::Plaintext);
        assert_eq!(disk_mode_for(&ArchiveMode::Encrypted), DiskEncryptionMode::Encrypted);
    }
}
