//! Responsibilities: bring an archive of the old SQLCipher format to a consistent state at
//! launch, before it is converted: finish or roll back an import, rekey or full restore that a
//! crash cut, then make `config.json` match the canonical file's real at-rest mode.
//! Not responsible for: native archives (`vault_core::recover_archive_on_launch`; the worker
//! picks one or the other), opening or converting the archive, or anything needing a key.
//! Failure modes: booting an empty archive over quarantined originals, a config that bricks the
//! next open with NOTADB, blocking a healthy launch on a lock, clearing a marker it could not act
//! on. Tests: legacy_recovery::tests and each journal module's tests.
//!
//! The old app wrote these journals; nothing writes them any more. This module exists so that
//! someone who updates straight after a crash in an older version loses nothing, and it goes
//! away with the converter.
#[cfg(test)]
mod fixtures;
mod import;
mod rekey;
mod restore;
mod snapshots;

pub use snapshots::list_legacy_snapshots;

use anyhow::{Context, Result};
use std::{fs, path::Path};
use vault_core::{
    AppConfig, ArchiveMode, ArchiveRecoveryKind, ArchiveRecoveryReport, DiskEncryptionMode,
    LaunchRecovery, ProjectPaths,
    archive::{ArchiveOpGate, ArchiveWriteLock},
    detect_disk_encryption_mode, load_config, save_config,
};

/// Recovers an old-format archive at launch. Header reads only: never opens a database and
/// needs no key.
///
/// The common healthy launch takes NO lock: a cheap unlocked pre-check (three marker `stat`s,
/// the canonical file's 16-byte header and `config.json`) returns [`LaunchRecovery::Healthy`]
/// when no marker is present and the config matches the file. Only otherwise does it take the
/// in-process [`ArchiveOpGate`], then the cross-process [`ArchiveWriteLock`], and re-read every
/// signal under them.
///
/// Source evidence is not reconciled here: conversion recreates it.
pub fn recover_on_launch(paths: &ProjectPaths, config: &AppConfig) -> Result<LaunchRecovery> {
    if launch_is_provably_healthy(paths, config) {
        return Ok(LaunchRecovery::Healthy);
    }
    let _op_gate = ArchiveOpGate::acquire(paths);
    let _write_lock = ArchiveWriteLock::acquire(paths)
        .context("acquiring the archive write lock for launch-time recovery")?;
    recover_locked(paths, config)
}

/// `true` when no crash marker is present and `config.json` (on disk, else the passed config)
/// already matches the canonical file's real at-rest mode. A `false` only means "let the locked
/// body decide"; it never heals anything itself. The marker check comes first, so an absent
/// canonical left by a crash mid-restore never reads as a fresh install.
fn launch_is_provably_healthy(paths: &ProjectPaths, config: &AppConfig) -> bool {
    if import::interrupted_import_marker_present(paths)
        || rekey::interrupted_rekey_marker_present(paths)
        || restore::interrupted_restore_marker_present(paths)
    {
        return false;
    }
    let recorded = load_config(paths)
        .map(|on_disk| on_disk.archive_mode)
        .unwrap_or_else(|_| config.archive_mode.clone());
    match detect_disk_encryption_mode(&paths.archive_database_path) {
        DiskEncryptionMode::Absent => true,
        DiskEncryptionMode::Plaintext => recorded == ArchiveMode::Plaintext,
        DiskEncryptionMode::Encrypted => recorded == ArchiveMode::Encrypted,
    }
}

/// The recovery proper, run with the gate and the write lock held. The journal recoveries take
/// only the reentrant write lock, never the gate, so they cannot deadlock against it.
fn recover_locked(paths: &ProjectPaths, config: &AppConfig) -> Result<LaunchRecovery> {
    // A pending full restore replaces the whole canonical archive, so it supersedes an
    // interrupted import or rekey. Running those first could fail closed on the restore's
    // absent-canonical window and starve a restore that would complete; the restore clears
    // their markers once it lands.
    if !restore::interrupted_restore_marker_present(paths) {
        // (1) An import whose commit phase was cut. The marker stays on error.
        if let Err(error) = import::recover_interrupted_import(paths) {
            return unrecoverable(
                paths,
                config,
                ArchiveRecoveryKind::InterruptedImportModeDrift,
                &error,
            );
        }
        // (2) A rekey cut between the swap and the config write. The marker stays on error.
        if let Err(error) = rekey::recover_interrupted_rekey(paths) {
            return unrecoverable(
                paths,
                config,
                ArchiveRecoveryKind::InterruptedRekeyUnresolved,
                &error,
            );
        }
    }
    // (3) A full restore cut while the canonical was quarantined. Completes it from the snapshot
    // or rolls back to the originals; never lets step 4 boot an empty archive.
    if let Err(error) = restore::recover_interrupted_restore(paths) {
        return unrecoverable(
            paths,
            config,
            ArchiveRecoveryKind::InterruptedRestoreUnresolved,
            &error,
        );
    }

    // (4) Config to the canonical file's real at-rest mode. Steps 1–3 may have rewritten
    // config.json, so compare against what is on disk now, and write only when it still differs.
    let target = match detect_disk_encryption_mode(&paths.archive_database_path) {
        DiskEncryptionMode::Absent => return Ok(LaunchRecovery::Healthy),
        DiskEncryptionMode::Plaintext => ArchiveMode::Plaintext,
        DiskEncryptionMode::Encrypted => ArchiveMode::Encrypted,
    };
    let on_disk = load_config(paths).unwrap_or_else(|_| config.clone());
    if on_disk.archive_mode != target {
        let mut healed = on_disk;
        healed.archive_mode = target.clone();
        save_config(paths, &healed)?;
    }
    // Report a heal whenever the final mode differs from what the caller passed in, so it never
    // goes on with a stale-mode config.
    if config.archive_mode == target {
        Ok(LaunchRecovery::Healthy)
    } else {
        Ok(LaunchRecovery::Healed { from_mode: config.archive_mode.clone(), to_mode: target })
    }
}

/// Drops an old-app marker that cannot be read, or refuses to. Without its journal nothing can
/// be finished or rolled back, which is safe only when the canonical archive and source evidence
/// agree on their at-rest mode. When they disagree, the cut operation left a mixed archive: the
/// marker stays and launch fails closed (quality-matrix 鐵律 3), so the recovery screen takes over.
fn drop_unreadable_marker(paths: &ProjectPaths, marker: &Path) -> Result<()> {
    let archive = detect_disk_encryption_mode(&paths.archive_database_path);
    let evidence = detect_disk_encryption_mode(&paths.source_evidence_database_path);
    if evidence != DiskEncryptionMode::Absent && archive != evidence {
        anyhow::bail!(
            "the recovery marker {} cannot be read, and the archive ({archive:?}) and source \
             evidence ({evidence:?}) are in different at-rest modes, so what it recorded can be \
             neither finished nor undone automatically. Restore from a safety snapshot.",
            marker.display(),
        );
    }
    vault_core::durable_io::remove_file_durably(marker)
        .context("removing the unreadable recovery marker")
}

fn unrecoverable(
    paths: &ProjectPaths,
    config: &AppConfig,
    kind: ArchiveRecoveryKind,
    error: &anyhow::Error,
) -> Result<LaunchRecovery> {
    let report = ArchiveRecoveryReport {
        kind,
        config_mode: config.archive_mode.clone(),
        history_vault_mode: archive_mode(detect_disk_encryption_mode(&paths.archive_database_path)),
        source_evidence_mode: archive_mode(detect_disk_encryption_mode(
            &paths.source_evidence_database_path,
        )),
        available_snapshots: snapshots::available_verified_snapshots(paths)
            .context("listing snapshots for the recovery report")?,
        recovery_snapshots: list_legacy_snapshots(paths)
            .context("listing snapshots for the recovery report")?,
        detail: format!("{error:#}"),
    };
    Ok(LaunchRecovery::Unrecoverable(report))
}

fn archive_mode(mode: DiskEncryptionMode) -> Option<ArchiveMode> {
    match mode {
        DiskEncryptionMode::Plaintext => Some(ArchiveMode::Plaintext),
        DiskEncryptionMode::Encrypted => Some(ArchiveMode::Encrypted),
        DiskEncryptionMode::Absent => None,
    }
}

/// Removes `-wal`/`-shm`/`-journal` next to `path`. A file put in place by a swap is
/// self-contained, so a sidecar there belongs to the file it replaced and must not replay.
fn remove_stale_sidecars(path: &Path) {
    for suffix in ["-wal", "-shm", "-journal"] {
        let _ = fs::remove_file(format!("{}{}", path.display(), suffix));
    }
}

#[cfg(test)]
mod tests {
    use super::fixtures::*;
    use super::*;
    use std::{sync::mpsc, time::Duration};
    use tempfile::tempdir;
    use vault_core::{config::ensure_paths, project_paths_with_root};

    fn paths() -> (tempfile::TempDir, ProjectPaths) {
        let dir = tempdir().unwrap();
        let paths = project_paths_with_root(dir.path());
        ensure_paths(&paths).unwrap();
        (dir, paths)
    }

    fn import_journal(paths: &ProjectPaths, entries: &[(&Path, bool)], previous: Option<String>) {
        let canonical: Vec<_> = entries
            .iter()
            .map(|(target, had)| serde_json::json!({"target": target, "had_previous": had}))
            .collect();
        let journal = serde_json::json!({
            "version": 1,
            "timestamp": CRASH_TIMESTAMP,
            "canonical": canonical,
            "subtrees": [],
            "previous_config": previous,
        });
        fs::write(marker(paths, ".pk-import-journal.json"), serde_json::to_vec(&journal).unwrap())
            .unwrap();
    }

    fn unrecoverable_report(outcome: LaunchRecovery) -> ArchiveRecoveryReport {
        match outcome {
            LaunchRecovery::Unrecoverable(report) => report,
            other => panic!("expected Unrecoverable, got {other:?}"),
        }
    }

    #[test]
    fn heals_a_stale_plaintext_config_over_encrypted_files() {
        // The 2026-06-30 incident: encrypted files, a Plaintext config, no journal.
        let (_dir, paths) = paths();
        legacy_db(&paths.archive_database_path, Some(KEY));
        legacy_db(&paths.source_evidence_database_path, Some(KEY));
        save_config(&paths, &plaintext_config()).unwrap();

        let outcome = recover_on_launch(&paths, &plaintext_config()).unwrap();
        assert!(
            matches!(
                outcome,
                LaunchRecovery::Healed {
                    from_mode: ArchiveMode::Plaintext,
                    to_mode: ArchiveMode::Encrypted
                }
            ),
            "{outcome:?}"
        );
        assert!(matches!(load_config(&paths).unwrap().archive_mode, ArchiveMode::Encrypted));
        assert_config_matches_disk(&paths);
        assert!(!marker(&paths, ".pk-rekey-journal.json").exists(), "no journal invented");
        assert!(!marker(&paths, ".pk-import-journal.json").exists(), "no journal invented");
    }

    #[test]
    fn completes_a_rekey_cut_after_the_swap() {
        let (_dir, paths) = paths();
        crash_mid_rekey(&paths, ArchiveMode::Plaintext, ArchiveMode::Encrypted);

        let outcome = recover_on_launch(&paths, &load_config(&paths).unwrap()).unwrap();
        assert!(matches!(outcome, LaunchRecovery::Healed { .. }), "{outcome:?}");
        assert!(!marker(&paths, ".pk-rekey-journal.json").exists());
        assert!(matches!(load_config(&paths).unwrap().archive_mode, ArchiveMode::Encrypted));
        assert_eq!(sentinel_runs(&paths.archive_database_path, Some(KEY)), 1);
        assert_config_matches_disk(&paths);
    }

    #[test]
    fn rolls_back_a_fresh_import_and_reports_an_uninitialized_archive() {
        let (_dir, paths) = paths();
        legacy_db(&paths.archive_database_path, None);
        import_journal(&paths, &[(&paths.archive_database_path, false)], None);

        let outcome = recover_on_launch(&paths, &plaintext_config()).unwrap();
        assert!(matches!(outcome, LaunchRecovery::Healthy), "{outcome:?}");
        assert!(!marker(&paths, ".pk-import-journal.json").exists());
        assert_eq!(
            detect_disk_encryption_mode(&paths.archive_database_path),
            DiskEncryptionMode::Absent
        );
    }

    #[test]
    fn reports_an_import_that_left_two_modes_and_keeps_its_marker() {
        let (_dir, paths) = paths();
        legacy_db(&paths.archive_database_path, None);
        legacy_db(&paths.source_evidence_database_path, Some(KEY));
        save_config(&paths, &plaintext_config()).unwrap();
        let previous = serde_json::to_string(&plaintext_config()).unwrap();
        import_journal(
            &paths,
            &[(&paths.archive_database_path, true), (&paths.source_evidence_database_path, true)],
            Some(previous),
        );
        let config_before = fs::read(&paths.config_path).unwrap();

        let report = unrecoverable_report(recover_on_launch(&paths, &plaintext_config()).unwrap());
        assert!(matches!(report.kind, ArchiveRecoveryKind::InterruptedImportModeDrift));
        assert!(!report.detail.is_empty());
        assert!(report.available_snapshots.is_empty());
        assert!(marker(&paths, ".pk-import-journal.json").exists(), "left for a retry");
        assert_eq!(fs::read(&paths.config_path).unwrap(), config_before, "config untouched");
    }

    #[test]
    fn falls_back_to_the_passed_config_when_config_json_is_corrupt() {
        let (_dir, paths) = paths();
        legacy_db(&paths.archive_database_path, Some(KEY));
        save_config(&paths, &plaintext_config()).unwrap();
        fs::write(&paths.config_path, b"{ not valid json").unwrap();

        let outcome = recover_on_launch(&paths, &plaintext_config()).unwrap();
        assert!(matches!(outcome, LaunchRecovery::Healed { to_mode: ArchiveMode::Encrypted, .. }));
        assert!(matches!(load_config(&paths).unwrap().archive_mode, ArchiveMode::Encrypted));
        assert_config_matches_disk(&paths);
    }

    #[test]
    fn reports_a_rekey_whose_archive_is_gone_and_offers_its_snapshot() {
        let (_dir, paths) = paths();
        crash_mid_rekey(&paths, ArchiveMode::Plaintext, ArchiveMode::Encrypted);
        legacy_db(&paths.raw_snapshots_dir.join("rekey/archive-before-rekey-x.sqlite"), None);
        fs::remove_file(&paths.archive_database_path).unwrap();
        let config_before = fs::read(&paths.config_path).unwrap();

        let report =
            unrecoverable_report(recover_on_launch(&paths, &load_config(&paths).unwrap()).unwrap());
        assert!(matches!(report.kind, ArchiveRecoveryKind::InterruptedRekeyUnresolved));
        assert_eq!(report.available_snapshots.len(), 1, "the rekey snapshot is offered");
        assert_eq!(report.recovery_snapshots.len(), 1);
        assert!(report.history_vault_mode.is_none());
        assert!(marker(&paths, ".pk-rekey-journal.json").exists(), "left for a retry");
        assert_eq!(fs::read(&paths.config_path).unwrap(), config_before);
    }

    #[test]
    fn does_nothing_on_a_consistent_or_empty_archive() {
        let (_dir, paths) = paths();
        legacy_db(&paths.archive_database_path, None);
        save_config(&paths, &plaintext_config()).unwrap();
        let before = fs::read(&paths.config_path).unwrap();
        let outcome = recover_on_launch(&paths, &plaintext_config()).unwrap();
        assert!(matches!(outcome, LaunchRecovery::Healthy), "{outcome:?}");
        assert_eq!(fs::read(&paths.config_path).unwrap(), before);

        let (_empty_dir, empty) = super::tests::paths();
        let outcome = recover_on_launch(&empty, &plaintext_config()).unwrap();
        assert!(matches!(outcome, LaunchRecovery::Healthy), "{outcome:?}");
    }

    #[test]
    fn journal_recovery_runs_while_launch_holds_the_gate_and_lock() {
        // The journal recoveries take only the reentrant write lock, never the gate, so they
        // cannot deadlock against the launch that holds both.
        let (_dir, paths) = paths();
        crash_mid_rekey(&paths, ArchiveMode::Plaintext, ArchiveMode::Encrypted);
        let _gate = ArchiveOpGate::acquire(&paths);
        let _lock = ArchiveWriteLock::acquire(&paths).unwrap();

        let worker_paths = paths.clone();
        let (done, finished) = mpsc::channel();
        let worker = std::thread::spawn(move || {
            done.send(
                rekey::recover_interrupted_rekey(&worker_paths).map_err(|e| format!("{e:#}")),
            )
            .unwrap();
        });
        let recovered = finished
            .recv_timeout(Duration::from_secs(10))
            .expect("the nested recovery must not wait for the gate")
            .expect("recovery under a held gate and lock");
        assert!(recovered);
        worker.join().unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn a_healthy_launch_does_not_wait_for_a_lock_held_elsewhere() {
        let (_dir, paths) = paths();
        legacy_db(&paths.archive_database_path, Some(KEY));
        save_config(&paths, &encrypted_config()).unwrap();
        let _elsewhere = hold_write_lock_elsewhere(&paths);

        let worker_paths = paths.clone();
        let (done, finished) = mpsc::channel();
        let worker = std::thread::spawn(move || {
            let outcome = recover_on_launch(&worker_paths, &encrypted_config()).unwrap();
            done.send(matches!(outcome, LaunchRecovery::Healthy)).unwrap();
        });
        assert!(
            finished.recv_timeout(Duration::from_secs(10)).expect("a healthy launch takes no lock"),
            "healthy",
        );
        worker.join().unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn a_launch_with_drift_waits_for_the_lock_then_heals() {
        let (_dir, paths) = paths();
        legacy_db(&paths.archive_database_path, Some(KEY));
        save_config(&paths, &plaintext_config()).unwrap();
        let elsewhere = hold_write_lock_elsewhere(&paths);

        let worker_paths = paths.clone();
        let (done, finished) = mpsc::channel();
        let worker = std::thread::spawn(move || {
            let outcome = recover_on_launch(&worker_paths, &plaintext_config()).unwrap();
            done.send(matches!(outcome, LaunchRecovery::Healed { .. })).unwrap();
        });
        assert!(
            matches!(
                finished.recv_timeout(Duration::from_millis(300)),
                Err(mpsc::RecvTimeoutError::Timeout)
            ),
            "drift recovery waits for the lock",
        );
        drop(elsewhere);
        assert!(finished.recv_timeout(Duration::from_secs(10)).expect("heals once released"));
        worker.join().unwrap();
        assert!(matches!(load_config(&paths).unwrap().archive_mode, ArchiveMode::Encrypted));
        assert_config_matches_disk(&paths);
    }

    #[test]
    fn is_healthy_after_an_import_rollback_that_leaves_a_consistent_archive() {
        let (_dir, paths) = paths();
        legacy_db(&paths.archive_database_path, None);
        save_config(&paths, &plaintext_config()).unwrap();
        let previous = serde_json::to_string(&plaintext_config()).unwrap();
        import_journal(&paths, &[(&paths.archive_database_path, true)], Some(previous));

        let outcome = recover_on_launch(&paths, &plaintext_config()).unwrap();
        assert!(matches!(outcome, LaunchRecovery::Healthy), "{outcome:?}");
        assert!(!marker(&paths, ".pk-import-journal.json").exists());
        assert_eq!(sentinel_runs(&paths.archive_database_path, None), 1);
    }

    #[test]
    fn reports_a_decrypt_cut_before_source_evidence_was_decrypted() {
        let (_dir, paths) = paths();
        crash_mid_rekey(&paths, ArchiveMode::Encrypted, ArchiveMode::Plaintext);
        legacy_db(&paths.source_evidence_database_path, Some(KEY));
        let config_before = fs::read(&paths.config_path).unwrap();

        let report =
            unrecoverable_report(recover_on_launch(&paths, &load_config(&paths).unwrap()).unwrap());
        assert!(matches!(report.kind, ArchiveRecoveryKind::InterruptedRekeyUnresolved));
        assert!(report.detail.contains("decrypt direction"), "{}", report.detail);
        assert!(marker(&paths, ".pk-rekey-journal.json").exists());
        assert_eq!(fs::read(&paths.config_path).unwrap(), config_before);
    }

    #[test]
    fn a_restore_marker_is_never_provably_healthy() {
        let (_dir, paths) = paths();
        plaintext_header(&paths.archive_database_path);
        save_config(&paths, &plaintext_config()).unwrap();
        assert!(launch_is_provably_healthy(&paths, &plaintext_config()));
        fs::write(marker(&paths, ".pk-restore-journal.json"), b"{}").unwrap();
        assert!(!launch_is_provably_healthy(&paths, &plaintext_config()));
        fs::remove_file(&paths.archive_database_path).unwrap();
        assert!(
            !launch_is_provably_healthy(&paths, &plaintext_config()),
            "an absent canonical next to a restore marker is not a fresh install",
        );
    }

    #[test]
    fn completes_a_restore_cut_in_its_install_window() {
        let (_dir, paths) = paths();
        crash_mid_restore(&paths, &plaintext_config(), None);
        let outcome = recover_on_launch(&paths, &plaintext_config()).unwrap();
        assert!(!matches!(outcome, LaunchRecovery::Unrecoverable(_)), "{outcome:?}");
        assert_eq!(sentinel_runs(&paths.archive_database_path, None), 1);
        assert!(!marker(&paths, ".pk-restore-journal.json").exists());
        assert_config_matches_disk(&paths);
    }

    #[test]
    fn reports_a_restore_with_nothing_left_to_restore_from() {
        let (_dir, paths) = paths();
        let snapshot = crash_mid_restore(&paths, &plaintext_config(), None);
        fs::remove_file(snapshot).unwrap();
        fs::remove_file(paths.quarantine_dir.join(CRASH_TIMESTAMP).join("history-vault.sqlite"))
            .unwrap();
        let report = unrecoverable_report(recover_on_launch(&paths, &plaintext_config()).unwrap());
        assert!(matches!(report.kind, ArchiveRecoveryKind::InterruptedRestoreUnresolved));
        assert!(marker(&paths, ".pk-restore-journal.json").exists());
        assert_eq!(
            detect_disk_encryption_mode(&paths.archive_database_path),
            DiskEncryptionMode::Absent,
            "no empty archive appears",
        );
    }

    #[test]
    fn a_pending_restore_supersedes_a_stale_rekey_marker() {
        let (_dir, paths) = paths();
        crash_mid_restore(&paths, &plaintext_config(), None);
        let rekey = serde_json::json!({
            "version": 1,
            "timestamp": CRASH_TIMESTAMP,
            "from_mode": "Plaintext",
            "to_mode": "Encrypted",
            "snapshot_ref": "/snapshots/rekey/before.sqlite",
            "previous_config": null,
        });
        fs::write(marker(&paths, ".pk-rekey-journal.json"), serde_json::to_vec(&rekey).unwrap())
            .unwrap();

        let outcome = recover_on_launch(&paths, &plaintext_config()).unwrap();
        assert!(!matches!(outcome, LaunchRecovery::Unrecoverable(_)), "{outcome:?}");
        assert_eq!(sentinel_runs(&paths.archive_database_path, None), 1);
        assert!(!marker(&paths, ".pk-restore-journal.json").exists());
        assert!(!marker(&paths, ".pk-rekey-journal.json").exists(), "the stale marker is cleared");
        assert_config_matches_disk(&paths);
    }
}
