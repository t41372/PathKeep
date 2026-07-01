//! Archive maintenance flows.
//!
//! This module owns the explicit maintenance operations that are not part of
//! the ordinary backup ingest path:
//!
//! - checkpoint replay (`snapshot_restore`)
//! - local retention preview/prune
//! - archive rekey / mode switch
//!
//! These flows are all recoverability-sensitive. They therefore keep the
//! run-ledger, manifest, and safety-snapshot story explicit instead of hiding
//! maintenance side effects behind ad-hoc file operations.

use super::{
    ingest::{
        persist_source_evidence_plans, preview_snapshot_counts, process_profile_snapshot,
        snapshot_source_hashes,
    },
    *,
};
use crate::durable_io::{atomic_durable_write, install_file_durably, remove_file_durably};
use serde::{Deserialize, Serialize};

/// Previews replaying one saved checkpoint or explains why the snapshot is manual-only.
pub fn preview_snapshot_restore(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    request: &SnapshotRestoreRequest,
) -> Result<SnapshotRestorePreview> {
    ensure_paths(paths)?;
    let connection = open_archive_connection(paths, config, key)?;
    let snapshot = load_snapshot_record(&connection, &request.snapshot_path)?
        .with_context(|| format!("snapshot {} was not found", request.snapshot_path))?;
    let snapshot_path = PathBuf::from(&snapshot.file_path);

    if snapshot_path.is_file() {
        return Ok(SnapshotRestorePreview {
            snapshot_path: snapshot.file_path,
            snapshot_kind: "archive-safety-snapshot".to_string(),
            source_run_id: Some(snapshot.run_id),
            source_profile_id: snapshot.profile_scope.first().cloned(),
            source_browser_name: None,
            created_at: Some(snapshot.created_at),
            reason: snapshot.reason,
            execute_supported: true,
            estimated_visits: 0,
            estimated_urls: 0,
            estimated_downloads: 0,
            warnings: vec![
                "This is a full-archive safety snapshot. One-click restore QUARANTINES the current archive files (kept under quarantine/ for recovery), installs this verified snapshot as the canonical archive, rebuilds an empty source-evidence, and reconciles config to match — a point-in-time restore of the whole archive.".to_string(),
            ],
        });
    }

    let checkpoint = load_checkpoint_profile_snapshot(&connection, &snapshot_path, &snapshot)?;
    let (estimated_visits, estimated_urls, estimated_downloads) =
        preview_snapshot_counts(&checkpoint, config)?;

    Ok(SnapshotRestorePreview {
        snapshot_path: snapshot.file_path,
        snapshot_kind: "raw-source-checkpoint".to_string(),
        source_run_id: Some(snapshot.run_id),
        source_profile_id: Some(checkpoint.profile.profile_id.clone()),
        source_browser_name: Some(checkpoint.profile.browser_name.clone()),
        created_at: Some(snapshot.created_at),
        reason: snapshot.reason,
        execute_supported: true,
        estimated_visits,
        estimated_urls,
        estimated_downloads,
        warnings: vec![
            "Snapshot restore replays the saved browser checkpoint into the current archive. Existing visible archive facts stay in place and duplicate rows are skipped.".to_string(),
        ],
    })
}

/// Replays one saved raw-source checkpoint into the canonical archive.
///
/// TOP-LEVEL destructive entry (lock-completion block): it takes the in-process
/// [`ArchiveOpGate`] (FIRST — excludes a SECOND same-process top-level op, CRIT-5's
/// trigger) + the cross-process [`ArchiveWriteLock`] (excludes the SEPARATE scheduled
/// backup), then RECOVERS any interrupted whole-app import BEFORE opening the archive
/// (recover-first). Recover-first does not fight the restore: it reverts a crashed
/// import's half-state to a single consistent at-rest mode so the archive opens cleanly,
/// and the restore then replays its checkpoint over the (now-coherent) visible facts —
/// which it would overwrite regardless. Reentrancy-safe: recovery re-takes the reentrant
/// lock as a nested guard and NEVER the non-reentrant gate.
pub fn run_snapshot_restore(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    request: &SnapshotRestoreRequest,
) -> Result<BackupReport> {
    ensure_paths(paths)?;
    if !config.initialized {
        anyhow::bail!("archive has not been initialized");
    }

    let _op_gate = ArchiveOpGate::acquire(paths);
    let _write_lock = ArchiveWriteLock::acquire(paths)
        .context("acquiring the archive write lock for snapshot restore")?;
    crate::migration::recover_interrupted_import(paths)?;

    run_snapshot_restore_locked(paths, config, key, request)
}

/// Replays the requested checkpoint, assuming the caller already holds the top-level
/// [`ArchiveOpGate`] + [`ArchiveWriteLock`] and has recovered any interrupted import.
fn run_snapshot_restore_locked(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    request: &SnapshotRestoreRequest,
) -> Result<BackupReport> {
    let mut connection = open_archive_connection(paths, config, key)?;
    let snapshot = load_snapshot_record(&connection, &request.snapshot_path)?
        .with_context(|| format!("snapshot {} was not found", request.snapshot_path))?;
    let snapshot_path = PathBuf::from(&snapshot.file_path);
    if snapshot_path.is_file() {
        anyhow::bail!(
            "automatic restore is only supported for saved browser source checkpoints right now"
        );
    }

    let checkpoint = load_checkpoint_profile_snapshot(&connection, &snapshot_path, &snapshot)?;
    let started_at = now_rfc3339();
    let timezone = current_timezone_name();
    let profile_scope = vec![checkpoint.profile.profile_id.clone()];

    connection.execute(
        "INSERT INTO runs (run_type, trigger, started_at, timezone, status, profile_scope_json, warnings_json, stats_json, due_only)
         VALUES ('snapshot_restore', 'manual', ?1, ?2, 'running', ?3, '[]', '{}', 0)",
        params![started_at, timezone, serde_json::to_string(&profile_scope)?],
    )?;
    let run_id = connection.last_insert_rowid();
    let parent_manifest = latest_manifest_row(&connection)?;

    let mut snapshot_artifacts = Vec::new();
    let mut source_evidence_plans = Vec::new();
    let mut source_evidence = open_source_evidence_connection(paths, config, key)?;
    let restore_result = (|| -> Result<BackupProfileSummary> {
        let transaction = connection.transaction()?;
        let profile_summary = process_profile_snapshot(
            &transaction,
            run_id,
            paths,
            config,
            &checkpoint,
            &mut snapshot_artifacts,
            &mut source_evidence_plans,
            false,
            false,
        )?;
        transaction.commit()?;
        Ok(profile_summary)
    })();

    let profile_summary = match restore_result {
        Ok(profile_summary) => profile_summary,
        Err(error) => {
            finalize_failed_run(&connection, run_id, &[], &[], &error)?;
            return Err(error);
        }
    };
    persist_source_evidence_plans(&mut source_evidence, &connection, &source_evidence_plans)?;

    record_snapshot_reference(
        &connection,
        run_id,
        &snapshot_path,
        "restored-source-checkpoint",
        &snapshot.created_at,
    )?;

    let finished_at = now_rfc3339();
    let summary = backup_run_summary(
        "snapshot_restore",
        run_id,
        &started_at,
        &finished_at,
        "manual",
        &profile_scope,
        std::slice::from_ref(&profile_summary),
    );
    let row_counts = archive_row_counts(&connection)?;
    let manifest = BackupManifest {
        created_at: finished_at.clone(),
        run_id,
        timezone: timezone.clone(),
        due_only: false,
        database_path: paths.archive_database_path.display().to_string(),
        summary: summary.clone(),
        profiles: vec![profile_summary.clone()],
        warnings: Vec::new(),
        source_hashes: BTreeMap::from([(
            checkpoint.profile.profile_id.clone(),
            snapshot_source_hashes(&checkpoint),
        )]),
        snapshots: snapshot_artifacts,
        row_counts: row_counts.clone(),
        parent_manifest_hash: parent_manifest.as_ref().map(|row| row.hash.clone()),
    };
    let manifest_json = serde_json::to_string_pretty(&manifest)?;
    let manifest_hash = sha256_hex(manifest_json.as_bytes());
    let manifest_path =
        write_manifest_artifact(paths, run_id, &finished_at, &manifest_hash, &manifest_json)?;
    persist_manifest_row(
        &connection,
        run_id,
        parent_manifest.as_ref(),
        &manifest_hash,
        &manifest_path,
        &finished_at,
        &row_counts,
    )?;
    let (git_commit, warnings) = if config.git_enabled {
        let (git_commit, git_warning) = git_audit::commit_all_optional(
            &paths.audit_repo_path,
            &format!("snapshot restore run {run_id}"),
        );
        (git_commit, git_warning.into_iter().collect::<Vec<_>>())
    } else {
        (None, Vec::new())
    };
    finalize_successful_run(
        &connection,
        run_id,
        &finished_at,
        &summary,
        &warnings,
        &manifest_hash,
    )?;

    Ok(BackupReport {
        due_skipped: false,
        reason: None,
        run: Some(BackupRunOverview { manifest_hash: Some(manifest_hash), ..summary }),
        profiles: vec![profile_summary],
        manifest_path: Some(manifest_path.display().to_string()),
        git_commit,
        warnings,
    })
}

/// One-click full-archive restore from a VERIFIED safety snapshot (Phase D1, the HEADLINE recovery
/// flow).
///
/// TOP-LEVEL destructive entry (lock-completion block): it takes the in-process [`ArchiveOpGate`]
/// FIRST (excludes a SECOND same-process top-level op), then the cross-process [`ArchiveWriteLock`]
/// (excludes the SEPARATE scheduled backup). Released in reverse order.
///
/// It is the remedy for a BROKEN canonical archive — a history-vault that will not open under the
/// current config (the 2026-06-30 incident shape: encrypted-on-disk under a Plaintext config, or a
/// zeroed/partial file). The order is chosen so the live archive is only ever touched AFTER the
/// chosen snapshot is proven restorable:
///   1. VALIDATE the snapshot path BY FILE (never the ledger — the archive may be un-openable) and
///      VERIFY it opens + passes `quick_check` BEFORE touching anything;
///   2. QUARANTINE the current canonical files + stale crash markers (move, never delete);
///   3. INSTALL the verified snapshot as the canonical history-vault (durably);
///   4. reconcile config LAST to the restored file's REAL at-rest mode, rebuild an empty
///      source-evidence, verify-after, and record an `archive_restore` audit run.
///
/// CRASH-RECOVERABLE COMMIT UNIT: steps 3–7 are guarded by a durable `.pk-restore-journal.json`
/// written (durably) BEFORE the quarantine move and cleared (durably) AFTER the post-restore
/// verify. Between quarantine and install the canonical history-vault is ABSENT for a multi-second
/// window; a crash there would otherwise let the next launch boot a brand-new EMPTY archive. The
/// marker's crash-twin [`recover_interrupted_restore`] — woven into the launch recovery
/// ([`crate::archive::recover_archive_on_launch`]) — instead COMPLETES the restore from the
/// still-available snapshot or ROLLS BACK to the quarantined originals, so a crash in that window
/// never boots an empty archive.
///
/// DIVERGENCE from the other top-level ops: it deliberately does NOT call
/// [`crate::migration::recover_interrupted_import`] first. D1 SUPERSEDES any interrupted
/// import/rekey — recovering-first would `?`-abort on exactly the `InterruptedImportModeDrift` /
/// un-openable state D1 exists to remedy. Instead step 2 quarantines any stale crash markers so a
/// superseded marker cannot drive a confusing recovery on the next launch.
pub fn run_full_archive_snapshot_restore(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    request: &SnapshotRestoreRequest,
) -> Result<FullArchiveRestoreReport> {
    ensure_paths(paths)?;

    let _op_gate = ArchiveOpGate::acquire(paths);
    let _write_lock = ArchiveWriteLock::acquire(paths)
        .context("acquiring the archive write lock for full-archive restore")?;

    // (1) Resolve + validate the snapshot BY FILE. SECURITY GUARD: only a regular `.sqlite` file
    // UNDER `raw_snapshots_dir` may ever be installed as the canonical archive.
    let snapshot_path = resolve_restore_snapshot_path(paths, &request.snapshot_path)?;

    // (2) Detect the snapshot's at-rest mode + VERIFY it opens BEFORE touching the live archive.
    // A failure here aborts with the archive untouched and NOTHING quarantined.
    let restored_mode = match detect_disk_encryption_mode(&snapshot_path) {
        DiskEncryptionMode::Encrypted => ArchiveMode::Encrypted,
        DiskEncryptionMode::Plaintext => ArchiveMode::Plaintext,
        DiskEncryptionMode::Absent => anyhow::bail!(
            "the chosen snapshot {} is missing or too small to be a database",
            snapshot_path.display()
        ),
    };
    let restored_key = match restored_mode {
        ArchiveMode::Encrypted => Some(
            key.context("the chosen snapshot is encrypted; unlock with the archive key first")?,
        ),
        ArchiveMode::Plaintext => None,
    };
    verify_database_integrity(&snapshot_path, restored_key)
        .with_context(|| format!("verifying the chosen snapshot {}", snapshot_path.display()))?;

    // (3) Quarantine the current canonical files + stale crash markers (move, never delete). Capture
    // whether a source-evidence existed BEFORE the move so the report can report the rebuild.
    let had_source_evidence = paths.source_evidence_database_path.exists();
    let timestamp = now_rfc3339().replace(':', "-");

    // (3a) DURABLE RESTORE JOURNAL — commit-unit start. From here until the post-install verify the
    // canonical history-vault is ABSENT (quarantined, snapshot not yet installed); a crash in that
    // window would otherwise let the next launch boot a brand-new EMPTY archive. Write the durable
    // marker BEFORE the quarantine move so its mere presence at next launch drives
    // `recover_interrupted_restore` to COMPLETE the restore from the still-available snapshot (it
    // stays put under raw-snapshots/ — quarantine never moves it) or ROLL BACK to the quarantined
    // originals (mirrors the rekey/import journal commit units).
    let quarantine_dir_path = paths.quarantine_dir.join(&timestamp);
    let journal = RestoreJournal {
        version: 1,
        timestamp: timestamp.clone(),
        snapshot_ref: snapshot_path.display().to_string(),
        restored_mode: restored_mode.clone(),
        quarantine_dir: quarantine_dir_path.display().to_string(),
        previous_config: fs::read_to_string(&paths.config_path).ok(),
    };
    write_restore_journal(paths, &journal)?;

    // `quarantine_canonical_archive` recomputes + returns the same dated dir; keep using its return
    // value as the report's quarantine dir.
    let quarantine_dir = quarantine_canonical_archive(paths, &timestamp)?;

    // (4) Install the snapshot as the canonical history-vault. `install_file_durably` requires the
    // built file in the SAME dir as `dest`, so copy into a temp beside the canonical archive first.
    let restore_temp = paths.archive_database_path.with_extension("restore.sqlite");
    if let Err(error) = install_restored_archive(&snapshot_path, &restore_temp, paths) {
        // The canonical file was already quarantined in step 3, so the broken originals are safe.
        // Scrub the half-staged temp + any sidecars so no phantom file is left behind.
        let _ = fs::remove_file(&restore_temp);
        super::at_rest::remove_stale_sidecars(&restore_temp);
        // The restore journal is INTENTIONALLY LEFT in place: the next launch's
        // `recover_interrupted_restore` then COMPLETES the restore from the still-good snapshot, or
        // ROLLS BACK the quarantined originals — rather than stranding the user with an absent
        // canonical archive that nothing heals.
        return Err(error);
    }

    // (5) Reconcile config LAST (durably): base on the passed config, force the restored file's REAL
    // at-rest mode + initialized=true (Phase-C invariant — config matches the file header → no NOTADB).
    let mut restored_config = config.clone();
    restored_config.initialized = true;
    restored_config.archive_mode = restored_mode.clone();
    save_config(paths, &restored_config)
        .context("reconciling config to the restored archive's at-rest mode")?;

    // (6) Rebuild an empty source-evidence consistent with the restored history-vault + config (the
    // previous one was quarantined in step 3). A point-in-time restore intentionally drops
    // source-evidence postdating the snapshot; it is rebuildable-empty evidence, so this is safe and
    // guarantees a readable source-evidence — never a NOTADB from a mode-mismatched leftover.
    drop(
        open_source_evidence_connection(paths, &restored_config, restored_key)
            .context("rebuilding an empty source-evidence after restore")?,
    );

    // (7) Verify AFTER install + reconcile. A failure here is serious (the snapshot was verified
    // pre-install); surface it — the broken originals are already in quarantine.
    drop(
        open_archive_connection(paths, &restored_config, restored_key)
            .context("opening the restored archive after install")?,
    );
    verify_database_integrity(&paths.archive_database_path, restored_key)
        .context("verifying the restored archive after install")?;

    // (7a) COMMIT POINT: the restored archive is durably installed + reconciled + verified. Clearing
    // the durable restore journal is what COMMITS the restore — a power loss can no longer resurrect
    // it, so launch recovery is a no-op exactly when the restore succeeded (mirrors the rekey/import
    // marker-clear commit points). Bookkeeping (the audit run below) is post-commit and downgradable.
    remove_file_durably(&restore_journal_path(paths))
        .context("clearing the restore journal after commit")?;
    // The canonical is now definitively the verified snapshot, so any rekey/import marker the
    // quarantine move could not relocate is stale — purge it so a later launch never acts on it.
    clear_superseded_crash_markers(paths);

    // (8) Record the `archive_restore` audit run + restored-snapshot reference in the now-canonical
    // archive for PME transparency. The archive is ALREADY installed + reconciled + verified-healthy
    // by here, so a bookkeeping failure must NOT fail an already-healed restore — downgrade it to a
    // warning and return `run_id: None` (the field already anticipates this).
    let mut warnings = Vec::new();
    let run_id = match record_full_archive_restore_run(
        paths,
        &restored_config,
        restored_key,
        &snapshot_path,
        &restored_mode,
    ) {
        Ok(run_id) => Some(run_id),
        Err(error) => {
            warnings.push(format!(
                "the archive was restored, but recording the audit run failed: {error:#}"
            ));
            None
        }
    };

    Ok(FullArchiveRestoreReport {
        run_id,
        restored_snapshot_path: snapshot_path.display().to_string(),
        restored_mode,
        quarantine_dir: quarantine_dir.display().to_string(),
        source_evidence_rebuilt: had_source_evidence,
        warnings,
    })
}

/// Resolves + validates a restore snapshot path BY FILE (the canonical archive may be un-openable,
/// so we never consult the ledger). SECURITY: refuses anything that is not a regular `.sqlite` file
/// located UNDER `raw_snapshots_dir` — never install an arbitrary path as the canonical archive.
fn resolve_restore_snapshot_path(paths: &ProjectPaths, requested: &str) -> Result<PathBuf> {
    let requested_path = PathBuf::from(requested);
    let canonical = requested_path
        .canonicalize()
        .with_context(|| format!("resolving the chosen snapshot {}", requested_path.display()))?;
    if !canonical.is_file() {
        anyhow::bail!("the chosen snapshot {} is not a regular file", canonical.display());
    }
    if canonical.extension().and_then(|ext| ext.to_str()) != Some("sqlite") {
        anyhow::bail!("the chosen snapshot {} is not a .sqlite file", canonical.display());
    }
    let snapshots_root = paths.raw_snapshots_dir.canonicalize().with_context(|| {
        format!("resolving the snapshots directory {}", paths.raw_snapshots_dir.display())
    })?;
    if !canonical.starts_with(&snapshots_root) {
        anyhow::bail!(
            "the chosen snapshot {} is not under the snapshots directory {}; refusing to install \
             an arbitrary path as the canonical archive",
            canonical.display(),
            snapshots_root.display(),
        );
    }
    // Return the path re-rooted under the NON-canonical base so it string-matches the entries
    // `list_recovery_snapshots`/`prune_snapshot_bucket` build from `read_dir(paths.raw_snapshots_dir)`.
    // Under a symlinked parent (macOS `/var` -> `/private/var`, which is exactly what `tempdir()` and
    // the real data dir use), the canonical and non-canonical strings differ, so D1 recording the
    // canonical `file_path` would let retention's `file_path != <non-canonical protected>` guard
    // DELETE the protected snapshot's ledger row. The security check above stays on the CANONICAL
    // form (a symlink escape is still rejected); only the RETURNED path is de-canonicalized. It points
    // at the same file, so copy/verify/install all work fine through the non-canonical base.
    let relative = canonical
        .strip_prefix(&snapshots_root)
        .expect("canonical starts_with snapshots_root was just checked");
    Ok(paths.raw_snapshots_dir.join(relative))
}

/// Copies the verified snapshot into a temp beside the canonical archive, then durably installs it
/// and scrubs any foreign WAL. Split out so the caller's early-return cleanup path stays small.
fn install_restored_archive(
    snapshot_path: &Path,
    restore_temp: &Path,
    paths: &ProjectPaths,
) -> Result<()> {
    // Crash-window seam: lets a test drive the install to fail AFTER the originals are quarantined
    // (step 3) so the caller's cleanup branch — scrub the temp, leave the broken originals safely in
    // quarantine — is exercised. A true no-op in production (nothing ever arms a fault).
    crate::fault_inject::checkpoint("restore.install")?;
    fs::copy(snapshot_path, restore_temp)
        .with_context(|| format!("staging the restore snapshot at {}", restore_temp.display()))?;
    install_file_durably(restore_temp, &paths.archive_database_path)
        .context("durably installing the restored archive")?;
    super::at_rest::remove_stale_sidecars(&paths.archive_database_path);
    Ok(())
}

/// Moves the current canonical files (history-vault + source-evidence + their `-wal`/`-shm`/
/// `-journal` sidecars) and any stale crash markers beside the archive into a dated
/// `quarantine/<ts>/` directory, preserving the broken state (rename, never delete) so the user can
/// always recover the pre-restore originals. Each move is best-effort — a missing file is fine.
/// Returns the dated quarantine directory.
fn quarantine_canonical_archive(paths: &ProjectPaths, timestamp: &str) -> Result<PathBuf> {
    let quarantine_dir = paths.quarantine_dir.join(timestamp);
    fs::create_dir_all(&quarantine_dir)
        .with_context(|| format!("creating quarantine dir {}", quarantine_dir.display()))?;

    // Move any superseded rekey/import markers FIRST — before the canonical DBs — so the
    // absent-canonical install window never coexists with a competing crash marker that launch
    // recovery would act on instead of the (auto-completable) restore. Best-effort: a marker that
    // cannot be moved is harmless (it is superseded, not load-bearing), and the
    // canonical-is-restored purge (`clear_superseded_crash_markers`) is the durable backstop.
    let _ = quarantine_single_file(&rekey_journal_path(paths), &quarantine_dir);
    let _ = quarantine_single_file(&crate::migration::import_journal_path(paths), &quarantine_dir);

    for canonical in [&paths.archive_database_path, &paths.source_evidence_database_path] {
        // PROPAGATE a REAL rename failure for the CANONICAL DBs: if a broken original cannot be
        // moved out of the way (permission / cross-device), the restore MUST abort BEFORE step 4's
        // install overwrites a still-in-place broken original — silently destroying the pre-restore
        // state the recovery flow promised to preserve. A genuinely-absent file is Ok(false).
        quarantine_single_file(canonical, &quarantine_dir).with_context(|| {
            format!("quarantining the canonical database {}", canonical.display())
        })?;
        for suffix in ["-wal", "-shm", "-journal"] {
            let sidecar = PathBuf::from(format!("{}{}", canonical.display(), suffix));
            // Sidecars are NOT load-bearing (the post-install sidecar scrub clears any leftover),
            // so a failure to move one must not abort the restore — best-effort.
            let _ = quarantine_single_file(&sidecar, &quarantine_dir);
        }
    }
    // MEDIUM-1: make the cross-directory move DURABLE. `fs::rename` of the canonical DBs into the
    // quarantine subdir is a cross-dir move whose LINK half (the new entry in `quarantine/<ts>/`) is
    // only crash-durable once that directory is fsynced — otherwise a power loss can leave the original
    // in NEITHER location, breaking the "move, never delete; always recover the originals" guarantee.
    crate::durable_io::fsync_dir(&quarantine_dir)
        .with_context(|| format!("fsyncing quarantine dir {}", quarantine_dir.display()))?;
    Ok(quarantine_dir)
}

/// Moves one file into `quarantine_dir`, keeping its file name. Returns `Ok(true)` when the file was
/// moved, `Ok(false)` when it was genuinely absent (nothing to do), and `Err` for a REAL rename
/// failure (permission / cross-device). The caller MUST propagate the `Err` for the canonical DBs so
/// a broken original is never silently overwritten; sidecars/markers stay best-effort.
fn quarantine_single_file(file: &Path, quarantine_dir: &Path) -> Result<bool> {
    let Some(name) = file.file_name() else {
        return Ok(false);
    };
    if !file.exists() {
        return Ok(false);
    }
    fs::rename(file, quarantine_dir.join(name))
        .with_context(|| format!("moving {} into quarantine", file.display()))?;
    Ok(true)
}

/// Records the `archive_restore` audit run (+ the restored-snapshot reference) in the now-canonical
/// restored archive, mirroring [`run_retention_prune_locked`]'s ledger shape but without profile
/// processing. Returns the new run id.
fn record_full_archive_restore_run(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    snapshot_path: &Path,
    restored_mode: &ArchiveMode,
) -> Result<i64> {
    // Crash-window seam: lets a test drive the audit-run bookkeeping to fail AFTER the archive is
    // already installed + reconciled + verified-healthy, exercising the caller's F4 path that
    // downgrades a recording failure to a warning rather than failing an already-healed restore. A
    // true no-op in production (nothing ever arms a fault).
    crate::fault_inject::checkpoint("restore.record_run")?;
    let connection = open_archive_connection(paths, config, key)?;
    let started_at = now_rfc3339();
    let timezone = current_timezone_name();
    connection.execute(
        "INSERT INTO runs (run_type, trigger, started_at, timezone, status, profile_scope_json, warnings_json, stats_json, due_only)
         VALUES ('archive_restore', 'manual', ?1, ?2, 'running', '[]', '[]', '{}', 0)",
        params![started_at, timezone],
    )?;
    let run_id = connection.last_insert_rowid();
    record_snapshot_reference(
        &connection,
        run_id,
        snapshot_path,
        "restored-archive-safety-snapshot",
        &started_at,
    )?;

    let finished_at = now_rfc3339();
    let manifest_payload = json!({
        "runType": "archive_restore",
        "runId": run_id,
        "createdAt": finished_at,
        "restoredMode": restored_mode,
        "snapshotPath": snapshot_path.display().to_string(),
    });
    let (manifest_hash, _manifest_path) =
        persist_structured_manifest(&connection, paths, run_id, &finished_at, &manifest_payload)?;
    let stats = stats_with_archive_totals(
        &connection,
        json!({
            "restoredMode": restored_mode,
            "snapshotPath": snapshot_path.display().to_string(),
            "manifestHash": manifest_hash,
        }),
    )?;
    connection.execute(
        "UPDATE runs
         SET finished_at = ?1,
             status = 'success',
             stats_json = ?2,
             warnings_json = ?3,
             error_message = NULL
         WHERE id = ?4",
        params![
            finished_at,
            serde_json::to_string(&stats)?,
            serde_json::to_string(&Vec::<String>::new())?,
            run_id,
        ],
    )?;
    Ok(run_id)
}

/// Builds the manual-first retention preview for local rebuildable artifacts.
pub fn preview_retention(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
) -> Result<RetentionPreview> {
    ensure_paths(paths)?;
    let snapshot_bucket = retention_snapshot_bucket(paths, config, key)?;
    let export_bucket = retention_directory_bucket("exports", &paths.exports_dir);
    let staging_bucket = retention_directory_bucket("staging", &paths.staging_dir);
    let quarantine_bucket = retention_directory_bucket("quarantine", &paths.quarantine_dir);

    Ok(RetentionPreview {
        buckets: vec![snapshot_bucket, export_bucket, staging_bucket, quarantine_bucket],
        warnings: vec![
            "Pruning snapshots removes saved restore checkpoints from future Audit review. Manifest and run summaries stay in place.".to_string(),
            "Export pruning only removes local files under the PathKeep data directory. Remote objects are unchanged.".to_string(),
        ],
    })
}

/// Executes the local retention prune flow for the selected buckets.
///
/// TOP-LEVEL destructive entry (lock-completion block): it serializes against every
/// other archive-mutating op via the in-process [`ArchiveOpGate`] (acquired FIRST, so a
/// SECOND same-process top-level op — CRIT-5's trigger — is excluded) + the cross-process
/// [`ArchiveWriteLock`] (so the SEPARATE scheduled backup defers), then RECOVERS any
/// interrupted whole-app import BEFORE it opens the archive (recover-first), reverting a
/// crashed import's half-state to a consistent archive. Reentrancy-safe: recovery re-takes
/// the reentrant lock as a nested guard and NEVER the non-reentrant gate. The cheap
/// "no buckets selected" guard runs WITHOUT the lock — it neither opens nor mutates the
/// archive — so only work that touches the archive runs under the lock + after recovery.
pub fn run_retention_prune(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    request: &RetentionPruneRequest,
) -> Result<RetentionPruneResult> {
    ensure_paths(paths)?;
    if !config.initialized {
        anyhow::bail!("initialize the archive before pruning retention artifacts");
    }
    if request.bucket_ids.is_empty() {
        return Ok(RetentionPruneResult {
            warnings: vec![
                "Choose at least one retention bucket before executing prune.".to_string(),
            ],
            ..RetentionPruneResult::default()
        });
    }

    let _op_gate = ArchiveOpGate::acquire(paths);
    let _write_lock = ArchiveWriteLock::acquire(paths)
        .context("acquiring the archive write lock for retention prune")?;
    crate::migration::recover_interrupted_import(paths)?;

    run_retention_prune_locked(paths, config, key, request)
}

/// Prunes the selected retention buckets, assuming the caller already holds the top-level
/// [`ArchiveOpGate`] + [`ArchiveWriteLock`] and has recovered any interrupted import.
fn run_retention_prune_locked(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    request: &RetentionPruneRequest,
) -> Result<RetentionPruneResult> {
    let preview = preview_retention(paths, config, key)?;
    let selected = preview
        .buckets
        .iter()
        .filter(|bucket| request.bucket_ids.iter().any(|id| id == &bucket.id))
        .cloned()
        .collect::<Vec<_>>();
    if selected.is_empty() {
        return Ok(RetentionPruneResult {
            warnings: vec!["No matching retention buckets were selected for prune.".to_string()],
            ..RetentionPruneResult::default()
        });
    }

    let connection = open_archive_connection(paths, config, key)?;
    let started_at = now_rfc3339();
    let timezone = current_timezone_name();
    connection.execute(
        "INSERT INTO runs (run_type, trigger, started_at, timezone, status, profile_scope_json, warnings_json, stats_json, due_only)
         VALUES ('retention_prune', 'manual', ?1, ?2, 'running', '[]', '[]', '{}', 0)",
        params![started_at, timezone],
    )?;
    let run_id = connection.last_insert_rowid();

    let mut deleted_bytes = 0u64;
    let mut deleted_files = 0usize;
    for bucket in &selected {
        if bucket.id == "snapshots" {
            let (bytes, files) = prune_snapshot_bucket(&connection, paths)?;
            deleted_bytes += bytes;
            deleted_files += files;
        } else if bucket.id == "exports" {
            let (bytes, files) = remove_directory_contents(&paths.exports_dir)?;
            deleted_bytes += bytes;
            deleted_files += files;
        } else if bucket.id == "staging" {
            let (bytes, files) = remove_directory_contents(&paths.staging_dir)?;
            deleted_bytes += bytes;
            deleted_files += files;
        } else if bucket.id == "quarantine" {
            let (bytes, files) = remove_directory_contents(&paths.quarantine_dir)?;
            deleted_bytes += bytes;
            deleted_files += files;
        }
    }

    let finished_at = now_rfc3339();
    let manifest_payload = json!({
        "runType": "retention_prune",
        "runId": run_id,
        "createdAt": finished_at,
        "deletedBytes": deleted_bytes,
        "deletedFiles": deleted_files,
        "buckets": selected,
    });
    let (manifest_hash, manifest_path) =
        persist_structured_manifest(&connection, paths, run_id, &finished_at, &manifest_payload)?;
    let stats = stats_with_archive_totals(
        &connection,
        json!({
            "deletedBytes": deleted_bytes,
            "deletedFiles": deleted_files,
            "buckets": request.bucket_ids,
            "manifestHash": manifest_hash,
        }),
    )?;
    let mut warnings = preview.warnings.clone();
    if config.git_enabled {
        let (_, git_warning) = git_audit::commit_all_optional(
            &paths.audit_repo_path,
            &format!("retention prune run {run_id}"),
        );
        warnings.extend(git_warning);
    }
    connection.execute(
        "UPDATE runs
         SET finished_at = ?1,
             status = 'success',
             stats_json = ?2,
             warnings_json = ?3,
             error_message = NULL
         WHERE id = ?4",
        params![
            finished_at,
            serde_json::to_string(&stats)?,
            serde_json::to_string(&warnings)?,
            run_id,
        ],
    )?;

    let _ = manifest_path;

    Ok(RetentionPruneResult {
        run_id: Some(run_id),
        deleted_bytes,
        deleted_files,
        buckets: selected,
        warnings,
    })
}

/// Rekeys or rewrites the archive into a different at-rest mode.
///
/// Crash-safety contract (the 2026-06-30 incident + the data-integrity audit's
/// CRIT-1/4/5 and incident-rootcause HIGHs). The steps are ordered so that an
/// interruption at ANY point leaves a recoverable archive, and config is written
/// LAST so a stale config is always the self-healable end state rather than a
/// brick:
///   1. hold the cross-process [`ArchiveWriteLock`] for the whole operation, so the
///      out-of-process scheduled backup can never race the swap, then RECOVER any
///      interrupted whole-app import FIRST (`recover_interrupted_import`): a plaintext
///      archive never reaches the encryption-gated launch reconcile, so rekeying a
///      half-applied import would brick it on a config↔source-evidence mode drift;
///   2. take a VERIFIED safety snapshot of the canonical archive BEFORE any
///      destructive write (checkpointed so the copy is WAL-complete, then re-opened
///      and `quick_check`ed with the CURRENT key — an un-restorable backstop is
///      worthless);
///   3. export the new-keyed database to a temp in the SAME directory as the
///      canonical file and VERIFY it (a KEYED `quick_check` with the TARGET key, which
///      subsumes any salt check) before any swap — a bad/partial export aborts here;
///   4. durably install the temp onto the canonical path (F_FULLFSYNC + rename +
///      dir fsync) and scrub the swapped-in file's stale `-wal`/`-shm` so no foreign
///      WAL can replay into the rekeyed database;
///   5. migrate source-evidence to the new key in lockstep, then write config LAST
///      and only then drop the backstop.
///
/// Run-ledger bookkeeping (`run_type='rekey'`, the before-rekey snapshot artifact,
/// and the finalize manifest) is preserved for PME transparency. The fault-injection
/// checkpoints are no-ops in production and let crash-window tests prove the
/// recoverability invariant at each step.
pub fn rekey_archive(
    paths: &ProjectPaths,
    current_config: &AppConfig,
    old_key: Option<&str>,
    new_mode: ArchiveMode,
    new_key: Option<&str>,
) -> Result<ArchiveStatus> {
    ensure_paths(paths)?;
    if !paths.archive_database_path.exists() {
        anyhow::bail!("archive database does not exist");
    }

    // (1) Serialize the entire rekey against every other archive-mutating op until these
    // guards drop. The in-process top-level [`ArchiveOpGate`] (acquired FIRST) excludes a
    // SECOND destructive op dispatched in THIS process — e.g. a manual backup mid-
    // transaction (CRIT-5's same-process trigger), which the reentrant flock alone cannot
    // exclude — and the cross-process [`ArchiveWriteLock`] excludes the SEPARATE scheduled-
    // backup process. Released in reverse order (lock first, then gate).
    let _op_gate = ArchiveOpGate::acquire(paths);
    let _write_lock =
        ArchiveWriteLock::acquire(paths).context("acquiring the archive write lock for rekey")?;

    // (1a) Heal a whole-app import whose commit phase was cut by a crash BEFORE this
    // rekey opens / snapshots / rewrites anything. A PLAINTEXT archive never reaches the
    // encryption-gated launch reconcile, so a half-applied import (e.g. new history-vault
    // installed, source-evidence not yet) can still be on disk when the user enables
    // encryption. Rekeying that half-state would convert ONE canonical DB to Encrypted
    // while leaving the marker + the other DB's plaintext `.bak` behind, so the NEXT
    // `recover_interrupted_import` would restore one DB to plaintext and leave the other
    // Encrypted — a permanent config↔source-evidence mode-drift brick. Recovering FIRST
    // restores the consistent pre-import state so the rekey operates on a coherent
    // archive (the post-rekey archive is then already a recovered, consistent state).
    // Reentrancy-safe: the write-lock manager is process-reentrant, so re-acquiring the
    // lock we already hold yields a nested guard sharing the one fd (no self-deadlock);
    // and a clean archive makes this a cheap marker `stat` no-op.
    crate::migration::recover_interrupted_import(paths)?;

    let started_at = now_rfc3339();
    let timezone = current_timezone_name();
    let target_key = match new_mode {
        ArchiveMode::Encrypted => Some(new_key.context("new encryption key is required")?),
        ArchiveMode::Plaintext => None,
    };
    let mut next_config = current_config.clone();
    next_config.initialized = true;
    next_config.archive_mode = new_mode.clone();

    let temp_path = paths.archive_database_path.with_extension("rekey.sqlite");
    // Clear leftovers from a prior interrupted rekey. The canonical archive is
    // confirmed present above, so a stale `.rekey.sqlite`/`.backup.sqlite` is a
    // redundant remnant (the new flow installs in place and keeps no `.backup`),
    // never the only live copy.
    let _ = fs::remove_file(&temp_path);
    let _ = fs::remove_file(paths.archive_database_path.with_extension("backup.sqlite"));

    let source = open_archive_connection(paths, current_config, old_key)?;

    // (2) Verified backstop of the canonical archive BEFORE any destructive write.
    let snapshot_path =
        create_verified_safety_snapshot(paths, &source, current_config, old_key, "rekey")?;

    // (3) Export the new-keyed database into a temp beside the canonical file, then
    // VERIFY it opens with the target key + passes quick_check before any swap.
    export_archive_database(&source, &temp_path, target_key)?;
    drop(source);
    verify_rekey_export(&temp_path, target_key)?;

    // Seed the rekey run + before-rekey snapshot reference into the NEW database (it
    // becomes canonical after the swap), folding its WAL into the main file so the
    // durable install captures every row, then scrub the temp's WAL/-shm sidecars.
    let run_id = seed_rekey_run(&temp_path, target_key, &started_at, &timezone, &snapshot_path)?;
    super::at_rest::remove_stale_sidecars(&temp_path);

    // Phase C: capture the durable rekey journal now that the verified snapshot + verified
    // export exist. Its presence (written just before the swap inside `rekey_swap_and_commit`)
    // SIGNALS to the launch-time `recover_archive_on_launch` that a swap may have happened, and
    // its contents drive `recover_interrupted_rekey`'s key-free reconciliation: complete the
    // config if the swap landed, roll back to the captured pre-rekey config if it did not.
    let journal = RekeyJournal {
        version: 1,
        timestamp: started_at.clone(),
        from_mode: current_config.archive_mode.clone(),
        to_mode: new_mode.clone(),
        snapshot_ref: snapshot_path.display().to_string(),
        // Best-effort capture (mirrors the rollback's tolerance for a missing config): an
        // absent or unreadable config.json yields `None`, so a rollback removes config rather
        // than restoring it — never aborting the rekey on a transient config read.
        previous_config: fs::read_to_string(&paths.config_path).ok(),
    };

    let mut swapped = false;
    match rekey_swap_and_commit(
        paths,
        &temp_path,
        old_key,
        target_key,
        &next_config,
        &journal,
        &mut swapped,
    ) {
        Ok(status) => {
            let connection = open_archive_connection(paths, &next_config, target_key)?;
            finalize_rekey_run(
                &connection,
                paths,
                run_id,
                &current_config.archive_mode,
                &new_mode,
                &snapshot_path,
                "success",
                None,
            )?;
            Ok(status)
        }
        Err(error) => {
            // Pre-swap failure: the canonical archive + config are untouched, so the
            // orphaned export is useless — drop it rather than leave a phantom swap. Also
            // clear any rekey marker the swap step may have written before failing, so a
            // recoverable pre-swap failure leaves no stale marker for the launch reconcile.
            if !swapped {
                let _ = fs::remove_file(&temp_path);
                super::at_rest::remove_stale_sidecars(&temp_path);
                let _ = remove_file_durably(&rekey_journal_path(paths));
            }
            // Post-swap failure: the run row now lives in the (canonical) new
            // database, so record the failure there for PME transparency. The backstop
            // snapshot is intentionally KEPT for recovery.
            if swapped
                && let Ok(connection) = open_archive_connection(paths, &next_config, target_key)
            {
                let _ = finalize_rekey_run(
                    &connection,
                    paths,
                    run_id,
                    &current_config.archive_mode,
                    &new_mode,
                    &snapshot_path,
                    "failed",
                    Some(format!("{error:#}")),
                );
            }
            Err(error)
        }
    }
}

/// Performs the irreversible half of a rekey: durably install the verified export,
/// scrub stale sidecars, convert source-evidence, and write config LAST.
///
/// `*swapped` is set the instant the canonical file has been replaced, so the
/// caller can distinguish a pre-swap failure (original archive + config intact)
/// from a post-swap one (new file on disk; a stale config is the self-healable lag).
/// The fault-injection checkpoints mark the exact crash windows the audit calls out.
///
/// `journal` is the durable rekey marker (Phase C): it is written just BEFORE the swap
/// so its mere presence at next launch means "a swap may have happened" and is removed
/// AFTER the config commit, so a power loss in the incident window leaves the marker as
/// the crash signal `recover_interrupted_rekey` keys on.
fn rekey_swap_and_commit(
    paths: &ProjectPaths,
    temp_path: &Path,
    old_key: Option<&str>,
    target_key: Option<&str>,
    next_config: &AppConfig,
    journal: &RekeyJournal,
    swapped: &mut bool,
) -> Result<ArchiveStatus> {
    // (4) A crash HERE must leave the ORIGINAL canonical database + config untouched.
    crate::fault_inject::checkpoint("rekey.after_export_before_swap")?;

    // (4a) Phase C: write the durable rekey marker BEFORE the swap. Its presence at next
    // launch tells `recover_archive_on_launch` a swap may have landed, so the launch-time
    // reconcile keys the canonical archive's REAL on-disk mode against this journal rather
    // than dead-ending on a stale config.
    write_rekey_journal(paths, journal)?;

    // (5) Durable swap (F_FULLFSYNC + rename + dir fsync), then scrub the swapped-in
    // file's stale sidecars so a foreign `-wal` can never replay into the rekeyed DB.
    install_file_durably(temp_path, &paths.archive_database_path)
        .context("durably installing the rekeyed archive")?;
    *swapped = true;
    super::at_rest::remove_stale_sidecars(&paths.archive_database_path);

    // (6) THE incident window: the file is converted but config still reflects the OLD
    // mode. Writing config LAST makes this the recoverable, data-safe state — the verified
    // backstop is still on disk and the new file is durably installed under the new key, so
    // nothing is lost. Phase C now AUTO-HEALS this window: the durable rekey marker written
    // at (4a) + the launch-time `recover_archive_on_launch` (`recover_interrupted_rekey`)
    // detect the config↔file at-rest drift key-free and converge config to the installed
    // file's real mode, so a crash here no longer surfaces as a NOTADB brick — it self-heals
    // to the locked unlock-prompt (encrypted) or plaintext-open (plaintext) state.
    crate::fault_inject::checkpoint("rekey.after_swap_before_config")?;

    // (7) Convert source-evidence in lockstep, BEFORE config/backstop are committed,
    // so a failure here keeps the backstop.
    super::migrate_source_evidence_for_rekey(paths, old_key, target_key)?;

    // (8) Config LAST, atomically + durably, only after BOTH databases are converted
    // and durable on disk.
    save_config(paths, next_config)?;

    // (8a) Commit point: clearing the durable marker is what commits the rekey — a power
    // loss can no longer resurrect it, so the launch reconcile is a no-op exactly when the
    // rekey succeeded (mirrors `apply_import`'s marker-clear commit point).
    remove_file_durably(&rekey_journal_path(paths))
        .context("clearing the rekey marker after commit")?;

    // (9) The new state is now fully durable; finalize may drop the backstop.
    crate::fault_inject::checkpoint("rekey.after_config")?;
    archive_status(paths, next_config, target_key)
}

/// Captures a VERIFIED full-archive safety snapshot of the canonical history-vault into
/// `raw-snapshots/<op>/` BEFORE a whole-archive REWRITE, and proves it is restorable.
///
/// Checkpoints the live archive (TRUNCATE) so the copy is not WAL-incomplete, copies
/// it, then re-opens the copy with the appropriate key and runs `quick_check`. A snapshot
/// that cannot be re-opened is worthless as a backstop, so a verification failure
/// aborts the caller's rewrite with the original archive still untouched.
///
/// Generalizes the original rekey-only snapshot so the three whole-archive rewrite ops — rekey,
/// at-rest reconcile, and whole-app import — share the identical "prove the backstop restores"
/// guarantee. `op` (`"rekey"`/`"reconcile"`/`"import"`) selects the `raw-snapshots/<op>/` bucket
/// the recovery GUI later lists by. Takes NO locks (every caller is a top-level op already holding
/// the gate + flock).
pub(crate) fn create_verified_safety_snapshot(
    paths: &ProjectPaths,
    source: &Connection,
    config: &AppConfig,
    key: Option<&str>,
    op: &str,
) -> Result<PathBuf> {
    checkpoint_truncate(source, "the archive before the safety snapshot")?;
    let snapshot_path = create_safety_snapshot(paths, op)?;
    let snapshot_key =
        if matches!(config.archive_mode, ArchiveMode::Encrypted) { key } else { None };
    // `verify_database_integrity` already names the snapshot file in its error, so no
    // extra wrapping closure is needed (and none is left only-reachable-on-failure).
    if let Err(error) = verify_database_integrity(&snapshot_path, snapshot_key) {
        // The copy landed but is unsound — remove it (and any sidecars) so a corrupt file never
        // lingers under `raw-snapshots/<op>/` cluttering the recovery surface or, worse, getting
        // advertised as a restore backstop. Keep the original error intact.
        let _ = fs::remove_file(&snapshot_path);
        super::at_rest::remove_stale_sidecars(&snapshot_path);
        return Err(error);
    }
    super::at_rest::remove_stale_sidecars(&snapshot_path);
    Ok(snapshot_path)
}

/// Seeds the `rekey` run row + before-rekey snapshot reference into the freshly
/// exported database, then folds its WAL into the main file so the durable swap
/// captures the bookkeeping. Returns the new run id.
fn seed_rekey_run(
    temp_path: &Path,
    target_key: Option<&str>,
    started_at: &str,
    timezone: &str,
    snapshot_path: &Path,
) -> Result<i64> {
    let connection =
        Connection::open(temp_path).with_context(|| format!("opening {}", temp_path.display()))?;
    connection.busy_timeout(StdDuration::from_secs(5))?;
    if let Some(key) = target_key {
        apply_cipher_key(&connection, key)?;
    }
    connection.pragma_update(None, "foreign_keys", true)?;
    create_schema(&connection)?;
    connection.execute(
        "INSERT INTO runs (run_type, trigger, started_at, timezone, status, profile_scope_json, warnings_json, stats_json, due_only)
         VALUES ('rekey', 'manual', ?1, ?2, 'running', '[]', '[]', '{}', 0)",
        params![started_at, timezone],
    )?;
    let run_id = connection.last_insert_rowid();
    record_snapshot_reference(&connection, run_id, snapshot_path, "before-rekey", started_at)?;
    checkpoint_truncate(&connection, "the rekey export before swap")?;
    Ok(run_id)
}

/// Folds a WAL-mode database's log back into its main file so a subsequent
/// file-level copy/rename captures every committed row (and leaves no `-wal` behind
/// for a swap to either miss or wrongly replay).
fn checkpoint_truncate(connection: &Connection, what: &str) -> Result<()> {
    connection
        .query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |_row| Ok(()))
        .with_context(|| format!("checkpointing {what}"))?;
    Ok(())
}

/// Verifies a freshly exported rekey database is sound for `target_key` BEFORE it is
/// allowed to overwrite the canonical archive.
///
/// The check is a KEYED `quick_check`: it opens the export with the exact key it will
/// be served under and walks the b-tree, so a partial/zeroed export — the 2026-06-30
/// incident, where an un-fsynced temp was swapped in and bricked every later open —
/// fails here (it cannot be decrypted/read) with the original archive still intact.
///
/// (No explicit "salt is non-zero" assertion is made here — but NOT because zeros are
/// normal. `sqlcipher_export` writes a RANDOM, non-zero 16-byte salt into page 1; a
/// zeroed/partial salt is the CORRUPTION SIGNATURE of the 2026-06-30 incident — a
/// stale `-wal` replay clobbering page 1 / un-fsynced page-1 writes — never a healthy
/// export. The check is omitted for two reasons: (a) the keyed `quick_check` STRICTLY
/// SUBSUMES it — a corrupt, partial, or wrong-key export cannot be decrypted and fails
/// quick_check; and (b) the salt-zeroing in the incident happens at power-loss /
/// next-open WAL replay, AFTER this pre-swap verify, so a salt check here could not
/// have caught it anyway. The real defenses are the durability barrier
/// (`install_file_durably`'s F_FULLFSYNC), the sidecar scrub, and this keyed
/// quick_check.)
fn verify_rekey_export(temp_path: &Path, target_key: Option<&str>) -> Result<()> {
    verify_database_integrity(temp_path, target_key)
        .with_context(|| format!("verifying the rekey export {}", temp_path.display()))
}

/// Confirms `path` opens with `key` and passes `PRAGMA quick_check`, using the SAME
/// key the file will be served with so a wrong-key/corrupt file is caught here
/// instead of bricking the next open.
fn verify_database_integrity(path: &Path, key: Option<&str>) -> Result<()> {
    let connection = Connection::open(path)
        .with_context(|| format!("opening {} for verification", path.display()))?;
    connection.busy_timeout(StdDuration::from_secs(5))?;
    if let Some(key) = key {
        apply_cipher_key(&connection, key)?;
    }
    let status: String = connection
        .query_row("PRAGMA quick_check", [], |row| row.get(0))
        .with_context(|| format!("running quick_check on {}", path.display()))?;
    if status != "ok" {
        anyhow::bail!("integrity check of {} failed: {status}", path.display());
    }
    Ok(())
}

/// Creates the safety-snapshot FILE for a whole-archive rewrite `op`, under `raw-snapshots/<op>/`.
///
/// The filename pattern (`archive-before-<op>-<ts>.sqlite`) is what [`list_recovery_snapshots`]
/// and the legacy `available_verified_snapshots` scan for. Copy only — verification is the
/// caller's ([`create_verified_safety_snapshot`]) job.
fn create_safety_snapshot(paths: &ProjectPaths, op: &str) -> Result<PathBuf> {
    let snapshot_dir = paths.raw_snapshots_dir.join(op);
    fs::create_dir_all(&snapshot_dir)?;
    let snapshot_path = snapshot_dir
        .join(format!("archive-before-{op}-{}.sqlite", now_rfc3339().replace(':', "-")));
    fs::copy(&paths.archive_database_path, &snapshot_path)
        .with_context(|| format!("creating {op} safety snapshot at {}", snapshot_path.display()))?;
    Ok(snapshot_path)
}

/// Finalizes the run-ledger + manifest story for one rekey execution.
fn finalize_rekey_run(
    connection: &Connection,
    paths: &ProjectPaths,
    run_id: i64,
    from_mode: &ArchiveMode,
    to_mode: &ArchiveMode,
    snapshot_path: &Path,
    status_label: &str,
    run_error: Option<String>,
) -> Result<()> {
    let finished_at = now_rfc3339();
    let manifest_payload = json!({
        "runType": "rekey",
        "runId": run_id,
        "createdAt": finished_at,
        "fromMode": from_mode,
        "toMode": to_mode,
        "snapshotPath": snapshot_path.display().to_string(),
        "status": status_label,
        "error": run_error,
    });
    let (manifest_hash, _manifest_path) =
        persist_structured_manifest(connection, paths, run_id, &finished_at, &manifest_payload)?;
    let stats = stats_with_archive_totals(
        connection,
        json!({
            "fromMode": from_mode,
            "toMode": to_mode,
            "snapshotPath": snapshot_path.display().to_string(),
            "manifestHash": manifest_hash,
        }),
    )?;
    connection.execute(
        "UPDATE runs
         SET finished_at = ?1,
             status = ?2,
             stats_json = ?3,
             warnings_json = ?4,
             error_message = ?5
         WHERE id = ?6",
        params![
            finished_at,
            status_label,
            serde_json::to_string(&stats)?,
            serde_json::to_string(&Vec::<String>::new())?,
            manifest_payload.get("error").and_then(Value::as_str),
            run_id,
        ],
    )?;
    Ok(())
}

// --- Phase C: durable rekey journal + crash-recovery twin ----------------------------------------

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
fn rekey_journal_path(paths: &ProjectPaths) -> PathBuf {
    paths
        .archive_database_path
        .parent()
        .expect("archive database path has a parent directory")
        .join(REKEY_JOURNAL_FILE)
}

/// `true` when the durable interrupted-rekey marker is present beside the canonical archive —
/// a single `stat`, no lock. The launch-time fast path
/// ([`crate::archive::recover_archive_on_launch`]) uses it to decide whether to take the gate +
/// flock at all, so the overwhelmingly common no-marker launch never touches a lock.
pub(crate) fn interrupted_rekey_marker_present(paths: &ProjectPaths) -> bool {
    rekey_journal_path(paths).exists()
}

/// Durably writes `journal` to the marker path (atomic temp + F_FULLFSYNC + rename + dir
/// fsync), so its presence is itself durable before the swap runs.
fn write_rekey_journal(paths: &ProjectPaths, journal: &RekeyJournal) -> Result<()> {
    let bytes = serde_json::to_vec(journal).context("serializing the interrupted-rekey journal")?;
    atomic_durable_write(&rekey_journal_path(paths), &bytes)
        .context("writing the interrupted-rekey journal")
}

/// Reads the interrupted-rekey marker. `Ok(None)` when it is absent OR unparseable.
///
/// A corrupt marker we cannot act on is best-effort removed and treated as absent (mirrors
/// `read_import_journal`): the on-disk archive is left exactly as it is, so a damaged marker
/// can never block opens forever.
fn read_rekey_journal(paths: &ProjectPaths) -> Result<Option<RekeyJournal>> {
    let path = rekey_journal_path(paths);
    if !path.exists() {
        return Ok(None);
    }
    let bytes = fs::read(&path).with_context(|| format!("reading {}", path.display()))?;
    match serde_json::from_slice::<RekeyJournal>(&bytes) {
        Ok(journal) => Ok(Some(journal)),
        Err(_) => {
            let _ = remove_file_durably(&path);
            Ok(None)
        }
    }
}

/// Maps a configured [`ArchiveMode`] to the on-disk header a file in that mode presents.
///
/// A private copy of `migration.rs`'s identical mapping, kept local so the rekey recovery can
/// compare the canonical archive's REAL header to the journal's recorded modes without
/// coupling the two recovery families.
fn disk_mode_for(mode: &ArchiveMode) -> DiskEncryptionMode {
    match mode {
        ArchiveMode::Plaintext => DiskEncryptionMode::Plaintext,
        ArchiveMode::Encrypted => DiskEncryptionMode::Encrypted,
    }
}

/// Crash-recovery twin of `rekey_swap_and_commit`'s in-band commit.
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
/// non-reentrant top-level [`ArchiveOpGate`]: re-acquiring the flock its caller already holds
/// yields a nested guard (no self-deadlock), while taking the gate its caller already holds
/// WOULD self-deadlock — so this nested helper must never touch it.
pub(crate) fn recover_interrupted_rekey(paths: &ProjectPaths) -> Result<bool> {
    if !rekey_journal_path(paths).exists() {
        return Ok(false);
    }
    let _lock = ArchiveWriteLock::acquire(paths)
        .context("acquiring the archive write lock for interrupted-rekey recovery")?;
    let Some(journal) = read_rekey_journal(paths)? else {
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
        _ => crate::config::load_config(paths).unwrap_or_default(),
    }
}

/// Rolls back an un-landed rekey: the `from_mode` original is intact (the atomic swap never
/// replaced it), so drop the orphaned export temp + its sidecars and restore config to the
/// captured pre-rekey bytes (`None` = none existed pre-rekey).
fn rollback_rekey(paths: &ProjectPaths, journal: &RekeyJournal) -> Result<()> {
    let temp_path = paths.archive_database_path.with_extension("rekey.sqlite");
    let _ = fs::remove_file(&temp_path);
    super::at_rest::remove_stale_sidecars(&temp_path);
    match &journal.previous_config {
        Some(bytes) => atomic_durable_write(&paths.config_path, bytes.as_bytes())
            .context("restoring the pre-rekey config")?,
        None => {
            let _ = remove_file_durably(&paths.config_path);
        }
    }
    Ok(())
}

// --- Phase D: durable restore journal + crash-recovery twin --------------------------------------
//
// Responsibilities: make [`run_full_archive_snapshot_restore`] a CRASH-RECOVERABLE commit unit.
// Its quarantine→install→commit window leaves the canonical history-vault ABSENT for a multi-second
// span (quarantined, snapshot not yet installed); a crash there would otherwise let the next launch
// boot a brand-new EMPTY archive. This section adds the durable marker + its launch-recovery twin
// so a crash in that window always COMPLETES the restore from the still-available snapshot or ROLLS
// BACK to the quarantined originals — never an empty archive.
// Not responsible for: taking the top-level [`ArchiveOpGate`] (the nested twin takes only the
// reentrant flock) or rebuilding source-evidence at recovery (deferred to the next keyed open,
// mirroring the rekey source-evidence deferral).

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
    /// re-install from). Already de-canonicalized by `resolve_restore_snapshot_path`.
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
/// ([`crate::archive::recover_archive_on_launch`]) uses it to decide whether to take the gate +
/// flock at all, so the overwhelmingly common no-marker launch never touches a lock.
pub(crate) fn interrupted_restore_marker_present(paths: &ProjectPaths) -> bool {
    restore_journal_path(paths).exists()
}

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
/// DIVERGENCE from `read_rekey_journal`/`read_import_journal`, which silently DELETE a corrupt
/// marker: unlike rekey/import (whose canonical history-vault is never absent), a restore can be
/// mid-window with the canonical ABSENT. Dropping an unparseable marker HERE would then let launch
/// step (4) see an Absent canonical, return `Healthy`, and boot a brand-new EMPTY archive over the
/// quarantined originals. So this LEAVES an unparseable marker in place and the canonical-aware
/// drop/fail-closed decision is the CALLER's ([`recover_interrupted_restore`]): drop it only when
/// the canonical is present, fail closed (leave it) when the canonical is absent.
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

/// Crash-recovery twin of [`run_full_archive_snapshot_restore`]'s quarantine→install→commit phase.
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
/// top-level [`ArchiveOpGate`]: re-acquiring the flock its caller already holds yields a nested
/// guard (no self-deadlock), while taking the gate its caller already holds WOULD self-deadlock — so
/// this nested helper must never touch it.
pub(crate) fn recover_interrupted_restore(paths: &ProjectPaths) -> Result<bool> {
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
        super::at_rest::remove_stale_sidecars(&restore_temp);
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
                && verify_database_integrity(&paths.archive_database_path, None).is_ok()
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
            restored_mode == &ArchiveMode::Plaintext
                && verify_database_integrity(snapshot_ref, None).is_ok()
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
        _ => crate::config::load_config(paths).unwrap_or_default(),
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
    let _ = remove_file_durably(&rekey_journal_path(paths));
    let _ = remove_file_durably(&crate::migration::import_journal_path(paths));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::project_paths_with_root;
    use std::panic::{AssertUnwindSafe, catch_unwind};
    use tempfile::tempdir;

    const VERIFY_KEY: &str = "rekey-verify-test-key";

    /// Writes a valid SQLite database file at `path`, encrypted when `key` is set,
    /// big enough to span several pages so a data-page corruption test has real
    /// b-tree pages to damage beyond page 1 (header + schema).
    fn seed_verifiable_database(path: &Path, key: Option<&str>) {
        let connection = Connection::open(path).expect("open seed db");
        connection.pragma_update(None, "page_size", 512).expect("small page size");
        if let Some(key) = key {
            apply_cipher_key(&connection, key).expect("apply key");
        }
        connection.execute_batch("CREATE TABLE t(a TEXT);").expect("create table");
        for _ in 0..200 {
            connection
                .execute("INSERT INTO t VALUES (?1)", params!["x".repeat(100)])
                .expect("seed");
        }
        drop(connection);
    }

    #[test]
    fn verify_database_integrity_accepts_valid_plaintext_and_encrypted_files() {
        let dir = tempdir().expect("tempdir");
        let plain = dir.path().join("plain.sqlite");
        seed_verifiable_database(&plain, None);
        verify_database_integrity(&plain, None).expect("a valid plaintext db verifies");

        let encrypted = dir.path().join("enc.sqlite");
        seed_verifiable_database(&encrypted, Some(VERIFY_KEY));
        verify_database_integrity(&encrypted, Some(VERIFY_KEY))
            .expect("a valid encrypted db verifies");
    }

    #[test]
    fn verify_database_integrity_rejects_a_corrupt_but_openable_database() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("corrupt.sqlite");
        seed_verifiable_database(&path, None);

        // Keep page 1 (header + sqlite_master) intact and garble every data page, so
        // the file still OPENS but quick_check walks the table b-tree into corruption
        // and returns a non-"ok" row — the branch that aborts a bad swap.
        let mut bytes = fs::read(&path).expect("read seeded db");
        for byte in bytes.iter_mut().skip(512) {
            *byte = 0xAA;
        }
        fs::write(&path, &bytes).expect("write corrupted db");

        let error =
            verify_database_integrity(&path, None).expect_err("a corrupt database must not verify");
        assert!(format!("{error:#}").contains("integrity check"), "got: {error:#}");
    }

    #[test]
    fn verify_database_integrity_rejects_an_encrypted_file_opened_with_the_wrong_key() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("enc.sqlite");
        seed_verifiable_database(&path, Some(VERIFY_KEY));

        let error = verify_database_integrity(&path, Some("the-wrong-key"))
            .expect_err("a wrong key must surface as a verification error");
        assert!(format!("{error:#}").contains("quick_check"), "got: {error:#}");
    }

    #[test]
    fn verify_rekey_export_keyed_quick_check_accepts_valid_and_rejects_corrupt() {
        let dir = tempdir().expect("tempdir");

        // Plaintext target: opens unkeyed + quick_check ok.
        let plain = dir.path().join("plain.sqlite");
        seed_verifiable_database(&plain, None);
        verify_rekey_export(&plain, None).expect("a valid plaintext export verifies");

        // Encrypted target: opens with the key + quick_check ok (the leading salt slot
        // being zero or not is irrelevant — the keyed read is what matters).
        let encrypted = dir.path().join("enc.sqlite");
        seed_verifiable_database(&encrypted, Some(VERIFY_KEY));
        verify_rekey_export(&encrypted, Some(VERIFY_KEY))
            .expect("a valid encrypted export verifies");

        // A corrupt body is rejected before any swap could happen.
        let mut bytes = fs::read(&plain).expect("read seeded db");
        for byte in bytes.iter_mut().skip(512) {
            *byte = 0xAA;
        }
        fs::write(&plain, &bytes).expect("write corrupted db");
        let error =
            verify_rekey_export(&plain, None).expect_err("a corrupt export must be rejected");
        assert!(format!("{error:#}").contains("verifying the rekey export"), "got: {error:#}");
    }

    #[test]
    fn checkpoint_truncate_folds_the_wal_into_the_main_file() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("wal.sqlite");
        let connection = Connection::open(&path).expect("open db");
        connection.pragma_update(None, "journal_mode", "WAL").expect("wal mode");
        connection.execute_batch("CREATE TABLE t(a); INSERT INTO t VALUES (1);").expect("seed");
        checkpoint_truncate(&connection, "the test database").expect("checkpoint truncate");
    }

    #[test]
    fn create_safety_snapshot_reports_copy_failure_with_target_path() {
        let dir = tempdir().expect("tempdir");
        let paths = project_paths_with_root(dir.path());

        let error =
            create_safety_snapshot(&paths, "rekey").expect_err("missing archive cannot be copied");

        assert!(format!("{error:#}").contains("creating rekey safety snapshot"));
    }

    #[test]
    fn create_safety_snapshot_routes_each_op_to_its_own_bucket() {
        // The generalized helper writes under `raw-snapshots/<op>/`, so reconcile/import backstops
        // land in their own buckets the recovery GUI lists by — not all under `rekey/`.
        let dir = tempdir().expect("tempdir");
        let paths = project_paths_with_root(dir.path());
        ensure_paths(&paths).expect("ensure paths");
        // A real (tiny) plaintext archive so the copy succeeds.
        drop(
            open_archive_connection(&paths, &plaintext_archive_config(), None)
                .expect("seed archive"),
        );

        for op in ["reconcile", "import"] {
            let snapshot = create_safety_snapshot(&paths, op).expect("snapshot");
            assert_eq!(
                snapshot.parent().expect("bucket parent"),
                paths.raw_snapshots_dir.join(op),
                "the {op} snapshot must land in raw-snapshots/{op}/",
            );
            assert!(
                snapshot
                    .file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.starts_with(&format!("archive-before-{op}-"))),
                "the snapshot filename must encode its op",
            );
        }
    }

    // --- Phase C: rekey journal + recover_interrupted_rekey -------------------------------------

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
    fn rekey_journal_round_trips_through_disk() {
        let dir = tempdir().expect("tempdir");
        let paths = project_paths_with_root(dir.path());
        fs::create_dir_all(paths.archive_database_path.parent().expect("archive parent"))
            .expect("archive dir");

        // Absent marker reads as None.
        assert!(read_rekey_journal(&paths).expect("absent read").is_none());

        let journal =
            sample_journal(ArchiveMode::Plaintext, ArchiveMode::Encrypted, Some("cfg".to_string()));
        write_rekey_journal(&paths, &journal).expect("write");
        let loaded = read_rekey_journal(&paths).expect("read").expect("present");
        assert_eq!(loaded.version, 1);
        assert!(matches!(loaded.from_mode, ArchiveMode::Plaintext));
        assert!(matches!(loaded.to_mode, ArchiveMode::Encrypted));
        assert_eq!(loaded.snapshot_ref, "/snapshots/rekey/before.sqlite");
        assert_eq!(loaded.previous_config.as_deref(), Some("cfg"));
    }

    #[test]
    fn recover_interrupted_rekey_without_a_marker_is_a_noop() {
        let dir = tempdir().expect("tempdir");
        let paths = project_paths_with_root(dir.path());
        assert!(
            !recover_interrupted_rekey(&paths).expect("no-marker recovery must not error"),
            "a missing marker must be a cheap no-op",
        );
    }

    #[test]
    fn recover_interrupted_rekey_removes_a_corrupt_marker() {
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
    }

    #[test]
    fn recover_interrupted_rekey_completes_a_landed_swap() {
        let dir = tempdir().expect("tempdir");
        let paths = project_paths_with_root(dir.path());
        // history-vault is at the NEW (encrypted) at-rest mode: the swap landed. previous_config
        // is None so the recovery falls back to the live config (the `_` arm + load_config).
        write_encrypted_db(&paths.archive_database_path);
        write_rekey_journal(
            &paths,
            &sample_journal(ArchiveMode::Plaintext, ArchiveMode::Encrypted, None),
        )
        .expect("write marker");

        assert!(recover_interrupted_rekey(&paths).expect("recover"));
        assert!(!rekey_journal_path(&paths).exists(), "the marker must be cleared after recovery");
        let config = crate::config::load_config(&paths).expect("load");
        assert!(matches!(config.archive_mode, ArchiveMode::Encrypted));
        assert!(config.initialized);
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
        let config = crate::config::load_config(&paths).expect("load");
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

    #[test]
    fn recover_interrupted_rekey_rollback_removes_config_when_none_was_captured() {
        let dir = tempdir().expect("tempdir");
        let paths = project_paths_with_root(dir.path());
        write_plaintext_db(&paths.archive_database_path);
        save_config(&paths, &encrypted_archive_config()).expect("seed config to remove");
        write_rekey_journal(
            &paths,
            &sample_journal(ArchiveMode::Plaintext, ArchiveMode::Encrypted, None),
        )
        .expect("write marker");

        assert!(recover_interrupted_rekey(&paths).expect("recover"));
        assert!(
            !paths.config_path.exists(),
            "a None previous_config must leave config absent after rollback",
        );
        assert!(!rekey_journal_path(&paths).exists());
    }

    #[test]
    fn recover_interrupted_rekey_fails_closed_when_history_vault_is_absent() {
        let dir = tempdir().expect("tempdir");
        let paths = project_paths_with_root(dir.path());
        fs::create_dir_all(paths.archive_database_path.parent().expect("archive parent"))
            .expect("archive dir");
        // No history-vault file at all -> detect == Absent -> fail closed.
        write_rekey_journal(
            &paths,
            &sample_journal(ArchiveMode::Plaintext, ArchiveMode::Encrypted, None),
        )
        .expect("write marker");

        let error = recover_interrupted_rekey(&paths)
            .expect_err("an absent history-vault must fail closed");
        assert!(format!("{error:#}").contains("missing or unreadable"), "got: {error:#}");
        assert!(
            rekey_journal_path(&paths).exists(),
            "the marker must be LEFT in place on a fail-closed bail",
        );
    }

    #[test]
    fn recover_interrupted_rekey_fails_closed_on_a_landed_decrypt_with_encrypted_source_evidence() {
        // FIX 2: an Encrypted->Plaintext rekey whose history-vault swap landed (now Plaintext) but
        // left source-evidence still Encrypted must NOT be silently completed — decrypting source-
        // evidence needs the now-unprompted old key. Fail closed: Err + marker LEFT + config NOT
        // committed to Plaintext, so the launch surfaces it as Unrecoverable.
        let dir = tempdir().expect("tempdir");
        let paths = project_paths_with_root(dir.path());
        write_plaintext_db(&paths.archive_database_path); // history-vault swapped to Plaintext
        write_encrypted_db(&paths.source_evidence_database_path); // source-evidence still Encrypted
        save_config(&paths, &encrypted_archive_config()).expect("seed encrypted config");
        write_rekey_journal(
            &paths,
            &sample_journal(ArchiveMode::Encrypted, ArchiveMode::Plaintext, None),
        )
        .expect("write marker");

        let error = recover_interrupted_rekey(&paths)
            .expect_err("a still-encrypted source-evidence must fail the decrypt completion");
        assert!(format!("{error:#}").contains("decrypt direction"), "got: {error:#}");
        assert!(
            rekey_journal_path(&paths).exists(),
            "the marker must be LEFT for the recovery GUI",
        );
        assert!(
            matches!(
                crate::config::load_config(&paths).expect("load").archive_mode,
                ArchiveMode::Encrypted,
            ),
            "config must NOT be committed to Plaintext on the fail-closed bail",
        );
    }

    #[test]
    fn recover_interrupted_rekey_completes_a_landed_decrypt_when_source_evidence_is_consistent() {
        // FIX 2's safe path: a landed Encrypted->Plaintext rekey whose source-evidence is NOT still
        // Encrypted (here Absent) is consistent, so the decrypt-direction completion proceeds —
        // config converges to Plaintext and the marker clears. We must not OVER-block: only a
        // still-Encrypted source-evidence fails closed.
        let dir = tempdir().expect("tempdir");
        let paths = project_paths_with_root(dir.path());
        write_plaintext_db(&paths.archive_database_path); // history Plaintext, source-evidence Absent
        write_rekey_journal(
            &paths,
            &sample_journal(ArchiveMode::Encrypted, ArchiveMode::Plaintext, None),
        )
        .expect("write marker");

        assert!(recover_interrupted_rekey(&paths).expect("a consistent decrypt completes"));
        assert!(!rekey_journal_path(&paths).exists(), "the marker clears on a completed decrypt",);
        assert!(matches!(
            crate::config::load_config(&paths).expect("load").archive_mode,
            ArchiveMode::Plaintext,
        ));
    }

    // --- D1: full-archive one-click restore -----------------------------------------------------

    const RESTORE_KEY: &str = "full-archive-restore-test-key";

    /// Seeds one `runs` row so a later `record_snapshot_reference` satisfies the snapshots FK.
    fn seed_runs_row(connection: &Connection) -> i64 {
        connection
            .execute(
                "INSERT INTO runs (run_type, trigger, started_at, timezone, status, profile_scope_json, warnings_json, stats_json, due_only)
                 VALUES ('rekey', 'manual', '2026-06-30T00:00:00Z', 'UTC', 'success', '[]', '[]', '{}', 0)",
                [],
            )
            .expect("seed run");
        connection.last_insert_rowid()
    }

    #[test]
    fn full_archive_restore_revives_a_broken_encrypted_archive() {
        // Mirrors the real incident: a canonical encrypted-on-disk history-vault that will NOT open
        // under the current config, plus a VERIFIED full-archive safety snapshot to restore from.
        let dir = tempdir().expect("tempdir");
        let paths = project_paths_with_root(dir.path());
        ensure_paths(&paths).expect("ensure paths");
        let config = encrypted_archive_config();

        // Materialise a REAL encrypted archive (the snapshot source) and copy it into the rekey
        // snapshot bucket as the verified safety snapshot.
        drop(
            open_archive_connection(&paths, &config, Some(RESTORE_KEY)).expect("seed enc archive"),
        );
        let snapshot_dir = paths.raw_snapshots_dir.join("rekey");
        fs::create_dir_all(&snapshot_dir).expect("snapshot dir");
        let snapshot_path = snapshot_dir.join("archive-before-rekey-good.sqlite");
        fs::copy(&paths.archive_database_path, &snapshot_path).expect("copy snapshot");
        super::at_rest::remove_stale_sidecars(&snapshot_path);
        // A pre-restore source-evidence is present so the rebuild path runs.
        drop(open_source_evidence_connection(&paths, &config, Some(RESTORE_KEY)).expect("seed SE"));

        // BREAK the canonical history-vault: zero its bytes so it can never decrypt/open.
        fs::write(&paths.archive_database_path, vec![0u8; 4096]).expect("brick canonical archive");
        super::at_rest::remove_stale_sidecars(&paths.archive_database_path);

        let request = SnapshotRestoreRequest { snapshot_path: snapshot_path.display().to_string() };
        let report =
            run_full_archive_snapshot_restore(&paths, &config, Some(RESTORE_KEY), &request)
                .expect("the one-click restore must succeed");

        // The broken canonical archive is now under quarantine/<ts>/.
        let quarantine_dir = PathBuf::from(&report.quarantine_dir);
        assert!(quarantine_dir.starts_with(&paths.quarantine_dir), "quarantine under quarantine/");
        assert!(
            quarantine_dir.join("history-vault.sqlite").exists(),
            "the broken archive is quarantined (moved, never deleted)",
        );
        assert!(
            report.source_evidence_rebuilt,
            "the prior source-evidence was quarantined + rebuilt"
        );
        assert!(matches!(report.restored_mode, ArchiveMode::Encrypted));
        assert!(report.run_id.is_some(), "an audit run was recorded");

        // The canonical archive OPENS + quick_checks ok with the key (real SQLCipher round-trip).
        drop(
            open_archive_connection(&paths, &config, Some(RESTORE_KEY))
                .expect("the restored archive opens"),
        );
        verify_database_integrity(&paths.archive_database_path, Some(RESTORE_KEY))
            .expect("the restored archive passes quick_check");
        // config reconciled to Encrypted.
        assert!(matches!(
            crate::config::load_config(&paths).expect("config").archive_mode,
            ArchiveMode::Encrypted,
        ));
        // E1 post-condition: the config (read through the REAL load_config) matches the restored
        // files' actual on-disk at-rest mode — the invariant the 2026-06-30 incident violated.
        crate::archive::check_config_disk_consistency(&paths)
            .expect("a successful full-archive restore must leave config matching disk");
        // A fresh source-evidence opens.
        drop(
            open_source_evidence_connection(&paths, &config, Some(RESTORE_KEY))
                .expect("a fresh source-evidence opens"),
        );
        // An archive_restore audit run exists.
        let connection =
            open_archive_connection(&paths, &config, Some(RESTORE_KEY)).expect("open restored");
        let restore_runs: i64 = connection
            .query_row("SELECT COUNT(*) FROM runs WHERE run_type = 'archive_restore'", [], |row| {
                row.get(0)
            })
            .expect("count restore runs");
        assert_eq!(restore_runs, 1, "the restore records an archive_restore audit run");
    }

    #[test]
    fn full_archive_restore_refuses_an_unverifiable_snapshot() {
        let dir = tempdir().expect("tempdir");
        let paths = project_paths_with_root(dir.path());
        ensure_paths(&paths).expect("ensure paths");
        let config = plaintext_archive_config();

        // A real, GOOD live archive that must stay untouched.
        drop(open_archive_connection(&paths, &config, None).expect("seed live archive"));
        let original = fs::read(&paths.archive_database_path).expect("read live bytes");

        // A corrupt snapshot under raw-snapshots/rekey/ (SQLite magic + garbage -> fails quick_check).
        let snapshot_dir = paths.raw_snapshots_dir.join("rekey");
        fs::create_dir_all(&snapshot_dir).expect("snapshot dir");
        let snapshot_path = snapshot_dir.join("archive-before-rekey-corrupt.sqlite");
        fs::write(&snapshot_path, b"SQLite format 3\0corrupt body, not a real database")
            .expect("write corrupt snapshot");

        let request = SnapshotRestoreRequest { snapshot_path: snapshot_path.display().to_string() };
        let error = run_full_archive_snapshot_restore(&paths, &config, None, &request)
            .expect_err("an unverifiable snapshot must be refused");
        let rendered = format!("{error:#}");
        assert!(
            rendered.contains("verifying the chosen snapshot") && rendered.contains("quick_check"),
            "the error must name the integrity failure, got: {rendered}",
        );
        assert_eq!(
            fs::read(&paths.archive_database_path).expect("read live archive"),
            original,
            "the live archive is byte-for-byte UNTOUCHED",
        );
        let quarantine_children: Vec<_> = fs::read_dir(&paths.quarantine_dir)
            .map(|entries| entries.flatten().collect())
            .unwrap_or_default();
        assert!(
            quarantine_children.is_empty(),
            "no dated quarantine dir is created when verify fails"
        );
    }

    #[test]
    fn full_archive_restore_rejects_a_snapshot_outside_the_snapshots_dir() {
        let dir = tempdir().expect("tempdir");
        let paths = project_paths_with_root(dir.path());
        ensure_paths(&paths).expect("ensure paths");
        let config = plaintext_archive_config();
        drop(open_archive_connection(&paths, &config, None).expect("seed live archive"));
        let original = fs::read(&paths.archive_database_path).expect("read live bytes");

        // A real `.sqlite` OUTSIDE raw-snapshots/ (a sibling under app_root).
        let outside = paths.app_root.join("outside.sqlite");
        {
            let connection = Connection::open(&outside).expect("open outside db");
            connection.execute_batch("CREATE TABLE t(a);").expect("seed outside");
        }

        let request = SnapshotRestoreRequest { snapshot_path: outside.display().to_string() };
        let error = run_full_archive_snapshot_restore(&paths, &config, None, &request)
            .expect_err("a path outside the snapshots dir must be refused");
        assert!(
            format!("{error:#}").contains("not under the snapshots directory"),
            "got: {error:#}",
        );
        assert_eq!(
            fs::read(&paths.archive_database_path).expect("read live archive"),
            original,
            "the live archive is UNTOUCHED",
        );
    }

    #[cfg(unix)]
    #[test]
    fn full_archive_restore_blocks_on_a_foreign_write_lock() {
        // gate+lock proof: with a foreign process holding the write lock the restore BLOCKS until
        // it releases, then completes — proving the top-level entry takes the cross-process lock.
        use std::sync::mpsc;
        use std::time::Duration;

        let dir = tempdir().expect("tempdir");
        let paths = project_paths_with_root(dir.path());
        ensure_paths(&paths).expect("ensure paths");
        let config = plaintext_archive_config();
        drop(open_archive_connection(&paths, &config, None).expect("seed live archive"));

        let snapshot_dir = paths.raw_snapshots_dir.join("rekey");
        fs::create_dir_all(&snapshot_dir).expect("snapshot dir");
        let snapshot_path = snapshot_dir.join("archive-before-rekey-good.sqlite");
        fs::copy(&paths.archive_database_path, &snapshot_path).expect("copy snapshot");

        let foreign =
            crate::archive::write_lock::hold_write_lock_as_foreign_process_for_test(&paths);

        let worker_paths = paths.clone();
        let (done_tx, done_rx) = mpsc::channel();
        let worker = std::thread::spawn(move || {
            let request =
                SnapshotRestoreRequest { snapshot_path: snapshot_path.display().to_string() };
            let outcome = run_full_archive_snapshot_restore(
                &worker_paths,
                &plaintext_archive_config(),
                None,
                &request,
            )
            .map(|report| report.run_id.is_some())
            .map_err(|error| format!("{error:#}"));
            done_tx.send(outcome).expect("worker signals completion");
        });

        assert!(
            matches!(
                done_rx.recv_timeout(Duration::from_millis(300)),
                Err(mpsc::RecvTimeoutError::Timeout)
            ),
            "the restore must BLOCK on the foreign write lock, proving it takes the lock",
        );

        drop(foreign);
        let completed = done_rx
            .recv_timeout(Duration::from_secs(10))
            .expect("the restore must complete once the foreign lock releases")
            .expect("the restore must not error");
        assert!(completed, "the restore completes once it acquires the lock");
        worker.join().expect("worker thread");
    }

    // --- D3: retention keeps the last-good verified safety snapshot ------------------------------

    #[test]
    fn retention_prune_keeps_the_last_good_verified_snapshot() {
        let dir = tempdir().expect("tempdir");
        let paths = project_paths_with_root(dir.path());
        ensure_paths(&paths).expect("ensure paths");
        let connection = open_archive_connection(&paths, &plaintext_archive_config(), None)
            .expect("open archive");

        let rekey_dir = paths.raw_snapshots_dir.join("rekey");
        fs::create_dir_all(&rekey_dir).expect("rekey dir");
        // A VERIFIED snapshot (real plaintext db) = the last-good backstop to protect.
        let protected = rekey_dir.join("archive-before-rekey-good.sqlite");
        {
            let snapshot = Connection::open(&protected).expect("open snapshot db");
            snapshot
                .execute_batch("CREATE TABLE t(a); INSERT INTO t VALUES (1);")
                .expect("seed protected snapshot");
        }
        // A second, NON-verified file (corrupt) that prune SHOULD remove.
        let disposable = rekey_dir.join("archive-before-rekey-bad.sqlite");
        fs::write(&disposable, b"SQLite format 3\0not a database").expect("write disposable");

        // Ledger rows: protected (must survive) + disposable (pruned).
        let run_id = seed_runs_row(&connection);
        record_snapshot_reference(
            &connection,
            run_id,
            &protected,
            "before-rekey",
            "2026-06-30T00:00:00Z",
        )
        .expect("record protected");
        record_snapshot_reference(
            &connection,
            run_id,
            &disposable,
            "before-rekey",
            "2026-06-29T00:00:00Z",
        )
        .expect("record disposable");

        crate::archive::prune_snapshot_bucket(&connection, &paths).expect("prune");

        assert!(protected.exists(), "the last-good verified snapshot survives the prune");
        assert!(!disposable.exists(), "a non-verified snapshot is pruned");
        let surviving: i64 = connection
            .query_row("SELECT COUNT(*) FROM snapshots", [], |row| row.get(0))
            .expect("count snapshots");
        assert_eq!(surviving, 1, "only the protected ledger row survives");
        let kept_path: String = connection
            .query_row("SELECT file_path FROM snapshots", [], |row| row.get(0))
            .expect("kept path");
        assert_eq!(kept_path, protected.display().to_string(), "the protected row is the one kept");
    }

    #[test]
    fn retention_prune_clears_everything_when_no_verified_snapshot_survives() {
        // The keep=None fallback: with only NON-verified snapshots present, prune behaves like the
        // original delete-everything (no backstop to protect).
        let dir = tempdir().expect("tempdir");
        let paths = project_paths_with_root(dir.path());
        ensure_paths(&paths).expect("ensure paths");
        let connection = open_archive_connection(&paths, &plaintext_archive_config(), None)
            .expect("open archive");

        let rekey_dir = paths.raw_snapshots_dir.join("rekey");
        fs::create_dir_all(&rekey_dir).expect("rekey dir");
        let corrupt = rekey_dir.join("archive-before-rekey-bad.sqlite");
        fs::write(&corrupt, b"SQLite format 3\0not a database").expect("write corrupt");
        let run_id = seed_runs_row(&connection);
        record_snapshot_reference(
            &connection,
            run_id,
            &corrupt,
            "before-rekey",
            "2026-06-29T00:00:00Z",
        )
        .expect("record corrupt");

        crate::archive::prune_snapshot_bucket(&connection, &paths).expect("prune");

        assert!(!corrupt.exists(), "a non-verified-only bucket is fully pruned");
        let surviving: i64 = connection
            .query_row("SELECT COUNT(*) FROM snapshots", [], |row| row.get(0))
            .expect("count snapshots");
        assert_eq!(surviving, 0, "with no protected backstop, all ledger rows are cleared");
    }

    #[test]
    fn full_archive_restore_requires_a_key_for_an_encrypted_snapshot() {
        let dir = tempdir().expect("tempdir");
        let paths = project_paths_with_root(dir.path());
        ensure_paths(&paths).expect("ensure paths");
        let config = plaintext_archive_config();
        drop(open_archive_connection(&paths, &config, None).expect("seed live archive"));
        let original = fs::read(&paths.archive_database_path).expect("read live bytes");

        // An encrypted-looking snapshot (non-SQLite header) under raw-snapshots/rekey/.
        let snapshot_dir = paths.raw_snapshots_dir.join("rekey");
        fs::create_dir_all(&snapshot_dir).expect("snapshot dir");
        let snapshot_path = snapshot_dir.join("archive-before-rekey-enc.sqlite");
        fs::write(&snapshot_path, vec![0xAB; 600]).expect("write encrypted snapshot");

        let request = SnapshotRestoreRequest { snapshot_path: snapshot_path.display().to_string() };
        let error = run_full_archive_snapshot_restore(&paths, &config, None, &request)
            .expect_err("an encrypted snapshot with no key must be refused");
        assert!(
            format!("{error:#}").contains("encrypted; unlock with the archive key"),
            "got: {error:#}",
        );
        assert_eq!(
            fs::read(&paths.archive_database_path).expect("read live archive"),
            original,
            "the live archive is UNTOUCHED",
        );
    }

    #[test]
    fn full_archive_restore_rejects_a_too_small_snapshot() {
        let dir = tempdir().expect("tempdir");
        let paths = project_paths_with_root(dir.path());
        ensure_paths(&paths).expect("ensure paths");
        let config = plaintext_archive_config();
        drop(open_archive_connection(&paths, &config, None).expect("seed live archive"));

        // A `.sqlite` under raw-snapshots/rekey/ that is too small to be a database (Absent header).
        let snapshot_dir = paths.raw_snapshots_dir.join("rekey");
        fs::create_dir_all(&snapshot_dir).expect("snapshot dir");
        let snapshot_path = snapshot_dir.join("archive-before-rekey-tiny.sqlite");
        fs::write(&snapshot_path, b"tiny").expect("write tiny snapshot");

        let request = SnapshotRestoreRequest { snapshot_path: snapshot_path.display().to_string() };
        let error = run_full_archive_snapshot_restore(&paths, &config, None, &request)
            .expect_err("a too-small snapshot must be refused");
        assert!(
            format!("{error:#}").contains("missing or too small to be a database"),
            "got: {error:#}",
        );
    }

    #[test]
    fn full_archive_restore_cleans_up_when_the_install_fails() {
        // Drive the install (step 4) to fail AFTER the originals are quarantined (step 3), exercising
        // the cleanup branch: the half-staged `restore.sqlite` temp is scrubbed and the broken
        // originals are preserved under quarantine/<ts>/ (never silently overwritten).
        let dir = tempdir().expect("tempdir");
        let paths = project_paths_with_root(dir.path());
        ensure_paths(&paths).expect("ensure paths");
        let config = plaintext_archive_config();

        // A present canonical archive + a valid verified snapshot under raw-snapshots/rekey/.
        drop(open_archive_connection(&paths, &config, None).expect("seed live archive"));
        let snapshot_dir = paths.raw_snapshots_dir.join("rekey");
        fs::create_dir_all(&snapshot_dir).expect("snapshot dir");
        let snapshot_path = snapshot_dir.join("archive-before-rekey-good.sqlite");
        fs::copy(&paths.archive_database_path, &snapshot_path).expect("copy snapshot");
        super::at_rest::remove_stale_sidecars(&snapshot_path);

        let request = SnapshotRestoreRequest { snapshot_path: snapshot_path.display().to_string() };
        let error = {
            // The must-fire guard also proves the install checkpoint is actually reached.
            let _guard = crate::fault_inject::FaultGuard::error_at_must_fire("restore.install");
            run_full_archive_snapshot_restore(&paths, &config, None, &request)
                .expect_err("the injected install fault must propagate")
        };
        assert!(
            format!("{error:#}").contains("restore.install"),
            "the injected checkpoint error must surface, got: {error:#}",
        );

        // The half-staged temp left no phantom file.
        let restore_temp = paths.archive_database_path.with_extension("restore.sqlite");
        assert!(!restore_temp.exists(), "the restore.sqlite temp is cleaned up");

        // The broken originals are preserved under quarantine/<ts>/ — the broken state is kept.
        let quarantined: Vec<_> =
            fs::read_dir(&paths.quarantine_dir).expect("quarantine dir exists").flatten().collect();
        assert_eq!(quarantined.len(), 1, "exactly one dated quarantine dir was created");
        assert!(
            quarantined[0].path().join("history-vault.sqlite").exists(),
            "the original archive is preserved in quarantine (never overwritten)",
        );
        // The restore journal is intentionally LEFT so launch recovery completes/rolls-back the
        // half-done restore instead of stranding the user with an absent canonical.
        assert!(
            restore_journal_path(&paths).exists(),
            "the restore journal is intentionally LEFT so launch recovery completes/rolls-back the half-done restore",
        );
    }

    #[test]
    fn install_restored_archive_reports_a_copy_failure_with_the_staging_path() {
        // The `.with_context` copy closure: copying a nonexistent snapshot fails and the error names
        // the staging step.
        let dir = tempdir().expect("tempdir");
        let paths = project_paths_with_root(dir.path());
        ensure_paths(&paths).expect("ensure paths");
        let missing = paths.raw_snapshots_dir.join("rekey").join("does-not-exist.sqlite");
        let restore_temp = paths.archive_database_path.with_extension("restore.sqlite");
        let error = install_restored_archive(&missing, &restore_temp, &paths)
            .expect_err("copying a nonexistent snapshot must fail");
        assert!(format!("{error:#}").contains("staging the restore snapshot"), "got: {error:#}",);
    }

    #[test]
    fn full_archive_restore_rejects_a_directory_named_like_a_snapshot() {
        // Security guard (a): a DIRECTORY whose name ends in `.sqlite` is not a regular file.
        let dir = tempdir().expect("tempdir");
        let paths = project_paths_with_root(dir.path());
        ensure_paths(&paths).expect("ensure paths");
        let config = plaintext_archive_config();
        drop(open_archive_connection(&paths, &config, None).expect("seed live archive"));
        let original = fs::read(&paths.archive_database_path).expect("read live bytes");

        let bogus = paths.raw_snapshots_dir.join("rekey").join("x.sqlite");
        fs::create_dir_all(&bogus).expect("create directory named like a snapshot");

        let request = SnapshotRestoreRequest { snapshot_path: bogus.display().to_string() };
        let error = run_full_archive_snapshot_restore(&paths, &config, None, &request)
            .expect_err("a directory must be refused");
        assert!(format!("{error:#}").contains("not a regular file"), "got: {error:#}");
        assert_eq!(
            fs::read(&paths.archive_database_path).expect("read live archive"),
            original,
            "the live archive is UNTOUCHED",
        );
    }

    #[test]
    fn full_archive_restore_rejects_a_non_sqlite_file() {
        // Security guard (b): a regular file under raw-snapshots/ without a `.sqlite` extension.
        let dir = tempdir().expect("tempdir");
        let paths = project_paths_with_root(dir.path());
        ensure_paths(&paths).expect("ensure paths");
        let config = plaintext_archive_config();
        drop(open_archive_connection(&paths, &config, None).expect("seed live archive"));
        let original = fs::read(&paths.archive_database_path).expect("read live bytes");

        let bogus = paths.raw_snapshots_dir.join("rekey").join("foo.txt");
        fs::create_dir_all(bogus.parent().expect("bucket parent")).expect("bucket dir");
        fs::write(&bogus, b"not a database").expect("write foo.txt");

        let request = SnapshotRestoreRequest { snapshot_path: bogus.display().to_string() };
        let error = run_full_archive_snapshot_restore(&paths, &config, None, &request)
            .expect_err("a non-.sqlite file must be refused");
        assert!(format!("{error:#}").contains("not a .sqlite file"), "got: {error:#}");
        assert_eq!(
            fs::read(&paths.archive_database_path).expect("read live archive"),
            original,
            "the live archive is UNTOUCHED",
        );
    }

    #[test]
    fn full_archive_restore_rejects_a_nonexistent_snapshot_path() {
        // Security guard (c): a path that does not exist fails `canonicalize`.
        let dir = tempdir().expect("tempdir");
        let paths = project_paths_with_root(dir.path());
        ensure_paths(&paths).expect("ensure paths");
        let config = plaintext_archive_config();
        drop(open_archive_connection(&paths, &config, None).expect("seed live archive"));
        let original = fs::read(&paths.archive_database_path).expect("read live bytes");

        let missing = paths.raw_snapshots_dir.join("rekey").join("missing.sqlite");
        fs::create_dir_all(missing.parent().expect("bucket parent")).expect("bucket dir");

        let request = SnapshotRestoreRequest { snapshot_path: missing.display().to_string() };
        let error = run_full_archive_snapshot_restore(&paths, &config, None, &request)
            .expect_err("a nonexistent snapshot path must be refused");
        assert!(format!("{error:#}").contains("resolving the chosen snapshot"), "got: {error:#}");
        assert_eq!(
            fs::read(&paths.archive_database_path).expect("read live archive"),
            original,
            "the live archive is UNTOUCHED",
        );
    }

    #[test]
    fn quarantine_single_file_is_a_noop_for_a_rootless_path() {
        // `Path::new("/")` has no `file_name()`, so the helper returns `Ok(false)` and moves nothing
        // (the defensive guard the canonical-DB loop relies on — exercised directly).
        let dir = tempdir().expect("tempdir");
        let quarantine_dir = dir.path().join("quarantine");
        fs::create_dir_all(&quarantine_dir).expect("quarantine dir");
        let moved = quarantine_single_file(Path::new("/"), &quarantine_dir)
            .expect("a rootless path must not error");
        assert!(!moved, "a path with no file name moves nothing");
        assert_eq!(
            fs::read_dir(&quarantine_dir).expect("read quarantine").count(),
            0,
            "the quarantine dir stays empty",
        );
    }

    #[test]
    fn full_archive_restore_heals_a_drifted_plaintext_config_to_encrypted() {
        // The 2026-06-30 incident heal: the archive is ENCRYPTED on disk while config says Plaintext
        // + uninitialized. Restoring an encrypted snapshot must FLIP the on-disk config to Encrypted
        // + initialized — proving D1 reconciles config from the restored file's REAL at-rest mode,
        // not the trivial already-equal case the other tests exercise.
        let dir = tempdir().expect("tempdir");
        let paths = project_paths_with_root(dir.path());
        ensure_paths(&paths).expect("ensure paths");

        // A real encrypted archive (snapshot source), copied into the rekey bucket.
        drop(
            open_archive_connection(&paths, &encrypted_archive_config(), Some(RESTORE_KEY))
                .expect("seed enc archive"),
        );
        let snapshot_dir = paths.raw_snapshots_dir.join("rekey");
        fs::create_dir_all(&snapshot_dir).expect("snapshot dir");
        let snapshot_path = snapshot_dir.join("archive-before-rekey-good.sqlite");
        fs::copy(&paths.archive_database_path, &snapshot_path).expect("copy snapshot");
        super::at_rest::remove_stale_sidecars(&snapshot_path);
        fs::write(&paths.archive_database_path, vec![0u8; 4096]).expect("brick archive");
        super::at_rest::remove_stale_sidecars(&paths.archive_database_path);

        // Persist the DRIFTED config: Plaintext + uninitialized (the incident shape).
        let drifted = AppConfig {
            archive_mode: ArchiveMode::Plaintext,
            initialized: false,
            ..AppConfig::default()
        };
        crate::config::save_config(&paths, &drifted).expect("save drifted config");

        let request = SnapshotRestoreRequest { snapshot_path: snapshot_path.display().to_string() };
        let report =
            run_full_archive_snapshot_restore(&paths, &drifted, Some(RESTORE_KEY), &request)
                .expect("the restore must heal the config drift");
        assert!(matches!(report.restored_mode, ArchiveMode::Encrypted));

        // The ON-DISK config FLIPPED to Encrypted + initialized.
        let healed = crate::config::load_config(&paths).expect("load healed config");
        assert!(
            matches!(healed.archive_mode, ArchiveMode::Encrypted),
            "config archive_mode flipped Plaintext -> Encrypted",
        );
        assert!(healed.initialized, "config flipped to initialized");

        // Real SQLCipher round-trip: the restored archive opens with the key + quick_check ok.
        drop(
            open_archive_connection(&paths, &healed, Some(RESTORE_KEY))
                .expect("the restored archive opens with the key"),
        );
        verify_database_integrity(&paths.archive_database_path, Some(RESTORE_KEY))
            .expect("the restored archive passes quick_check");
    }

    #[test]
    fn full_archive_restore_records_a_retention_consistent_snapshot_path() {
        // F3 regression: D1 must record a snapshot `file_path` that STRING-MATCHES what
        // `list_recovery_snapshots` reports (both rooted at the NON-canonical `raw_snapshots_dir`), so
        // retention's last-good guard (`DELETE ... WHERE file_path != <protected>`) keeps the restored
        // snapshot's ledger row. The macOS tempdir is itself under a symlink (/var -> /private/var),
        // so a canonical path here would NOT match and the row would be wrongly pruned.
        let dir = tempdir().expect("tempdir");
        let paths = project_paths_with_root(dir.path());
        ensure_paths(&paths).expect("ensure paths");
        let config = encrypted_archive_config();

        drop(
            open_archive_connection(&paths, &config, Some(RESTORE_KEY)).expect("seed enc archive"),
        );
        let snapshot_dir = paths.raw_snapshots_dir.join("rekey");
        fs::create_dir_all(&snapshot_dir).expect("snapshot dir");
        let snapshot_path = snapshot_dir.join("archive-before-rekey-good.sqlite");
        fs::copy(&paths.archive_database_path, &snapshot_path).expect("copy snapshot");
        super::at_rest::remove_stale_sidecars(&snapshot_path);
        fs::write(&paths.archive_database_path, vec![0u8; 4096]).expect("brick archive");
        super::at_rest::remove_stale_sidecars(&paths.archive_database_path);

        let request = SnapshotRestoreRequest { snapshot_path: snapshot_path.display().to_string() };
        let report =
            run_full_archive_snapshot_restore(&paths, &config, Some(RESTORE_KEY), &request)
                .expect("restore");

        // The path D1 recorded must equal what list_recovery_snapshots reports for the same file.
        let listed = super::at_rest::list_recovery_snapshots(&paths);
        let listed_path =
            listed.iter().find(|s| s.verified_openable).expect("a verified snapshot").path.clone();
        assert_eq!(
            report.restored_snapshot_path, listed_path,
            "the recorded restored path must match the listed (non-canonical) path",
        );

        // Retention prune keeps the restored snapshot's FILE and its ledger row.
        let connection =
            open_archive_connection(&paths, &config, Some(RESTORE_KEY)).expect("open restored");
        crate::archive::prune_snapshot_bucket(&connection, &paths).expect("prune");
        assert!(snapshot_path.exists(), "the restored snapshot file survives retention prune");
        let kept: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM snapshots WHERE file_path = ?1",
                params![listed_path],
                |row| row.get(0),
            )
            .expect("count kept ledger rows");
        assert_eq!(kept, 1, "the restored snapshot's ledger row survives (path-consistent)");
    }

    #[test]
    fn full_archive_restore_downgrades_a_recording_failure_to_a_warning() {
        // F4: by the time the audit run is recorded, the archive is already installed + reconciled +
        // verified-healthy, so a bookkeeping failure must NOT fail the restore — it returns
        // `run_id: None` plus an explanatory warning, and the archive stays healed.
        let dir = tempdir().expect("tempdir");
        let paths = project_paths_with_root(dir.path());
        ensure_paths(&paths).expect("ensure paths");
        let config = encrypted_archive_config();

        drop(
            open_archive_connection(&paths, &config, Some(RESTORE_KEY)).expect("seed enc archive"),
        );
        let snapshot_dir = paths.raw_snapshots_dir.join("rekey");
        fs::create_dir_all(&snapshot_dir).expect("snapshot dir");
        let snapshot_path = snapshot_dir.join("archive-before-rekey-good.sqlite");
        fs::copy(&paths.archive_database_path, &snapshot_path).expect("copy snapshot");
        super::at_rest::remove_stale_sidecars(&snapshot_path);
        fs::write(&paths.archive_database_path, vec![0u8; 4096]).expect("brick archive");
        super::at_rest::remove_stale_sidecars(&paths.archive_database_path);

        let request = SnapshotRestoreRequest { snapshot_path: snapshot_path.display().to_string() };
        let report = {
            let _guard = crate::fault_inject::FaultGuard::error_at_must_fire("restore.record_run");
            run_full_archive_snapshot_restore(&paths, &config, Some(RESTORE_KEY), &request)
                .expect("a recording failure must NOT fail an already-healed restore")
        };

        assert!(report.run_id.is_none(), "run_id is None when the audit run could not be recorded");
        assert!(
            report
                .warnings
                .iter()
                .any(|warning| warning.contains("recording the audit run failed")),
            "a warning explains the bookkeeping failure, got: {:?}",
            report.warnings,
        );
        // The archive is still healed: it opens with the key + passes quick_check.
        verify_database_integrity(&paths.archive_database_path, Some(RESTORE_KEY))
            .expect("the restored archive is healed despite the recording failure");
    }

    #[test]
    fn create_verified_safety_snapshot_removes_a_copy_that_fails_verification() {
        // F5: when the copy lands but the post-copy keyed verify fails, the corrupt snapshot file is
        // removed so it never clutters the recovery surface (or gets advertised as a backstop).
        let dir = tempdir().expect("tempdir");
        let paths = project_paths_with_root(dir.path());
        ensure_paths(&paths).expect("ensure paths");

        // A real PLAINTEXT archive as the snapshot source.
        let source = open_archive_connection(&paths, &plaintext_archive_config(), None)
            .expect("seed archive");
        // Force the post-copy verify to FAIL: claim Encrypted + a key, so the keyed quick_check
        // cannot decrypt the plaintext copy.
        let error = create_verified_safety_snapshot(
            &paths,
            &source,
            &encrypted_archive_config(),
            Some("a-key-the-plaintext-copy-cannot-satisfy"),
            "reconcile",
        )
        .expect_err("a copy that fails verification must error");
        assert!(
            format!("{error:#}").contains("quick_check")
                || format!("{error:#}").contains("integrity"),
            "got: {error:#}",
        );
        let bucket = paths.raw_snapshots_dir.join("reconcile");
        let remaining = fs::read_dir(&bucket).map(|entries| entries.flatten().count()).unwrap_or(0);
        assert_eq!(remaining, 0, "the corrupt snapshot copy is removed on a verify failure");
    }

    #[cfg(unix)]
    #[test]
    fn quarantine_canonical_archive_aborts_on_a_canonical_rename_failure() {
        // F1: a REAL rename failure on a CANONICAL DB must ABORT the quarantine (and so the restore
        // before install), never silently leaving the broken original to be overwritten.
        use std::os::unix::fs::PermissionsExt;
        let dir = tempdir().expect("tempdir");
        let paths = project_paths_with_root(dir.path());
        ensure_paths(&paths).expect("ensure paths");
        fs::write(&paths.archive_database_path, b"broken-original")
            .expect("write canonical archive");

        // Pre-create the dated quarantine dir read-only, so `create_dir_all` is an idempotent no-op
        // but the canonical rename INTO it fails with a permission error (a genuine failure, not
        // "absent").
        let timestamp = "2026-06-30T00-00-00Z";
        let dated = paths.quarantine_dir.join(timestamp);
        fs::create_dir_all(&dated).expect("dated quarantine dir");
        fs::set_permissions(&dated, fs::Permissions::from_mode(0o555)).expect("chmod read-only");

        let error = quarantine_canonical_archive(&paths, timestamp)
            .expect_err("a real canonical rename failure must abort the quarantine");
        assert!(
            format!("{error:#}").contains("quarantining the canonical database"),
            "got: {error:#}",
        );
        // The broken original is STILL in place (never silently overwritten).
        assert!(paths.archive_database_path.exists(), "the broken original is preserved");

        // Restore perms so the tempdir can be cleaned up.
        fs::set_permissions(&dated, fs::Permissions::from_mode(0o755)).expect("restore perms");
    }

    #[test]
    fn resolve_restore_snapshot_path_errors_when_the_snapshots_dir_is_missing() {
        // Covers the `raw_snapshots_dir.canonicalize()` failure context: a valid `.sqlite` requested
        // path whose snapshots ROOT does not exist, so the security re-rooting cannot proceed.
        let dir = tempdir().expect("tempdir");
        let paths = project_paths_with_root(dir.path());
        // Deliberately DO NOT `ensure_paths`, so `raw_snapshots_dir` is absent.
        let requested = dir.path().join("loose.sqlite");
        {
            let connection = Connection::open(&requested).expect("open requested db");
            connection.execute_batch("CREATE TABLE t(a);").expect("seed requested db");
        }
        let error = resolve_restore_snapshot_path(&paths, &requested.display().to_string())
            .expect_err("a missing snapshots directory must error");
        assert!(
            format!("{error:#}").contains("resolving the snapshots directory"),
            "got: {error:#}",
        );
    }

    // --- D1 (Phase D): crash-recoverable restore commit unit ------------------------------------

    /// Seeds a real archive (with a sentinel `backup` run) + a verified snapshot copy, persists a
    /// matching config, then drives `run_full_archive_snapshot_restore` to PANIC at the install
    /// checkpoint — which fires AFTER quarantine + the durable journal write but BEFORE the install.
    /// Returns the snapshot path. After it returns the canonical history-vault is ABSENT, the restore
    /// marker is present, and the snapshot is untouched: the exact mid-restore crash state launch
    /// recovery must heal without ever booting an empty archive.
    fn seed_and_crash_mid_restore(
        paths: &ProjectPaths,
        config: &AppConfig,
        key: Option<&str>,
    ) -> PathBuf {
        crate::config::save_config(paths, config).expect("save config");
        {
            let connection = open_archive_connection(paths, config, key).expect("seed archive");
            connection
                .execute(
                    "INSERT INTO runs (run_type, trigger, started_at, timezone, status, profile_scope_json, warnings_json, stats_json, due_only)
                     VALUES ('backup','manual','t','UTC','success','[]','[]','{}',0)",
                    [],
                )
                .expect("seed sentinel run");
            checkpoint_truncate(&connection, "seed archive").expect("checkpoint");
        }
        let snapshot_dir = paths.raw_snapshots_dir.join("rekey");
        fs::create_dir_all(&snapshot_dir).expect("snapshot dir");
        let snapshot_path = snapshot_dir.join("snap.sqlite");
        fs::copy(&paths.archive_database_path, &snapshot_path).expect("copy snapshot");
        super::at_rest::remove_stale_sidecars(&snapshot_path);

        let request = SnapshotRestoreRequest { snapshot_path: snapshot_path.display().to_string() };
        crate::fault_inject::arm_panic_at("restore.install");
        let crashed = catch_unwind(AssertUnwindSafe(|| {
            run_full_archive_snapshot_restore(paths, config, key, &request)
        }));
        assert!(crashed.is_err(), "the armed install panic must unwind the restore");

        // The crash window: canonical quarantined (absent), marker durable, snapshot untouched.
        assert_eq!(
            detect_disk_encryption_mode(&paths.archive_database_path),
            DiskEncryptionMode::Absent,
            "the canonical history-vault is absent in the crash window",
        );
        assert!(restore_journal_path(paths).exists(), "the restore marker survived the crash");
        assert!(snapshot_path.exists(), "the snapshot is untouched by quarantine");
        snapshot_path
    }

    /// Counts the sentinel `backup` runs in the canonical archive (proves data survived vs. an empty
    /// rebuild).
    fn sentinel_backup_run_count(
        paths: &ProjectPaths,
        config: &AppConfig,
        key: Option<&str>,
    ) -> i64 {
        let connection =
            open_archive_connection(paths, config, key).expect("open restored archive");
        connection
            .query_row("SELECT COUNT(*) FROM runs WHERE run_type = 'backup'", [], |row| row.get(0))
            .expect("count sentinel runs")
    }

    #[test]
    fn recover_interrupted_restore_completes_an_absent_canonical_from_the_snapshot() {
        // HEADLINE: a crash in the absent-canonical install window must NOT leave an empty archive —
        // launch recovery COMPLETES the restore from the still-available snapshot, data intact.
        let dir = tempdir().expect("tempdir");
        let paths = project_paths_with_root(dir.path());
        ensure_paths(&paths).expect("ensure paths");
        let config = plaintext_archive_config();
        let snapshot_path = seed_and_crash_mid_restore(&paths, &config, None);

        let outcome = recover_archive_on_launch(&paths, &plaintext_archive_config(), None)
            .expect("launch recovery must not error");
        assert!(
            !matches!(outcome, LaunchRecovery::Unrecoverable(_)),
            "recovery must complete the restore, not surface Unrecoverable: {outcome:?}",
        );

        // The canonical history-vault is restored (present + sound), the marker is cleared, and the
        // sentinel run SURVIVED — proving data was restored, not wiped to an empty archive.
        assert_ne!(
            detect_disk_encryption_mode(&paths.archive_database_path),
            DiskEncryptionMode::Absent,
            "the canonical history-vault is restored, not absent",
        );
        verify_database_integrity(&paths.archive_database_path, None)
            .expect("the restored canonical archive passes quick_check");
        assert!(!restore_journal_path(&paths).exists(), "the marker is cleared after recovery");
        // E1 post-condition: recovering an interrupted restore leaves config matching the files.
        crate::archive::check_config_disk_consistency(&paths)
            .expect("recovering an interrupted restore must leave config matching disk");
        assert_eq!(
            sentinel_backup_run_count(&paths, &plaintext_archive_config(), None),
            1,
            "the sentinel run row survived — data was restored, not wiped",
        );
        assert!(snapshot_path.exists(), "the snapshot is preserved (never moved by quarantine)");
    }

    #[test]
    fn recover_interrupted_restore_completes_an_absent_canonical_encrypted() {
        // Encrypted variant: recovery is KEY-FREE (re-installs the keyed-verified snapshot by header),
        // converges config to Encrypted, and the archive opens + keyed-quick_checks with the key.
        let dir = tempdir().expect("tempdir");
        let paths = project_paths_with_root(dir.path());
        ensure_paths(&paths).expect("ensure paths");
        let config = encrypted_archive_config();
        seed_and_crash_mid_restore(&paths, &config, Some(RESTORE_KEY));

        let outcome = recover_archive_on_launch(&paths, &encrypted_archive_config(), None)
            .expect("key-free launch recovery must not error");
        assert!(!matches!(outcome, LaunchRecovery::Unrecoverable(_)), "got: {outcome:?}");

        assert_eq!(
            detect_disk_encryption_mode(&paths.archive_database_path),
            DiskEncryptionMode::Encrypted,
            "the restored canonical is encrypted on disk",
        );
        assert!(
            matches!(
                crate::config::load_config(&paths).expect("config").archive_mode,
                ArchiveMode::Encrypted
            ),
            "config reconciled to Encrypted",
        );
        assert!(!restore_journal_path(&paths).exists(), "the marker is cleared after recovery");

        // Opening WITH the key + keyed quick_check succeeds, and the sentinel run survived.
        assert_eq!(
            sentinel_backup_run_count(&paths, &encrypted_archive_config(), Some(RESTORE_KEY)),
            1,
            "data survived the keyed re-install",
        );
        verify_database_integrity(&paths.archive_database_path, Some(RESTORE_KEY))
            .expect("the restored encrypted archive passes a keyed quick_check");
    }

    #[test]
    fn recover_interrupted_restore_rolls_back_when_the_snapshot_is_gone() {
        // The snapshot vanished after the crash: recovery must ROLL BACK the quarantined original
        // (move, never delete) rather than strand the user — the original + its data come back.
        let dir = tempdir().expect("tempdir");
        let paths = project_paths_with_root(dir.path());
        ensure_paths(&paths).expect("ensure paths");
        let config = plaintext_archive_config();
        let snapshot_path = seed_and_crash_mid_restore(&paths, &config, None);
        fs::remove_file(&snapshot_path).expect("delete the snapshot so rollback is forced");

        let outcome = recover_archive_on_launch(&paths, &plaintext_archive_config(), None)
            .expect("launch recovery must not error");
        assert!(!matches!(outcome, LaunchRecovery::Unrecoverable(_)), "got: {outcome:?}");

        // The pre-restore original was moved back from quarantine; it verifies + the sentinel is there.
        assert_ne!(
            detect_disk_encryption_mode(&paths.archive_database_path),
            DiskEncryptionMode::Absent,
            "the pre-restore original is restored from quarantine",
        );
        verify_database_integrity(&paths.archive_database_path, None)
            .expect("the rolled-back original passes quick_check");
        assert!(!restore_journal_path(&paths).exists(), "the marker is cleared after rollback");
        assert_eq!(
            sentinel_backup_run_count(&paths, &plaintext_archive_config(), None),
            1,
            "the original (with its sentinel run) is restored, not an empty archive",
        );
    }

    #[test]
    fn recover_interrupted_restore_is_unrecoverable_when_snapshot_and_originals_are_both_gone() {
        // FAIL-CLOSED: the snapshot AND the quarantined originals are both gone, so recovery cannot
        // reach any canonical archive. It must surface Unrecoverable + LEAVE the marker for a retry,
        // and must NEVER recreate the canonical as an empty archive.
        let dir = tempdir().expect("tempdir");
        let paths = project_paths_with_root(dir.path());
        ensure_paths(&paths).expect("ensure paths");
        let config = plaintext_archive_config();
        let snapshot_path = seed_and_crash_mid_restore(&paths, &config, None);
        fs::remove_file(&snapshot_path).expect("delete the snapshot");

        // Delete the quarantined original history-vault too (the only other backstop).
        let dated = fs::read_dir(&paths.quarantine_dir)
            .expect("quarantine dir")
            .flatten()
            .next()
            .expect("a dated quarantine dir exists");
        fs::remove_file(dated.path().join("history-vault.sqlite"))
            .expect("delete the quarantined original history-vault");

        let outcome = recover_archive_on_launch(&paths, &plaintext_archive_config(), None)
            .expect("recovery returns Ok carrying an Unrecoverable report");
        let report = match outcome {
            LaunchRecovery::Unrecoverable(report) => report,
            other => panic!("expected Unrecoverable, got {other:?}"),
        };
        assert!(matches!(report.kind, ArchiveRecoveryKind::InterruptedRestoreUnresolved));
        assert!(
            report.detail.contains("not recoverable from quarantine"),
            "the report names the fail-closed cause, got: {}",
            report.detail,
        );
        // The marker is LEFT for a retry, and the canonical was NOT recreated as an empty archive.
        assert!(restore_journal_path(&paths).exists(), "the marker is left for retry");
        assert_eq!(
            detect_disk_encryption_mode(&paths.archive_database_path),
            DiskEncryptionMode::Absent,
            "no empty archive was booted",
        );
    }

    #[test]
    fn recover_interrupted_restore_reconciles_when_the_canonical_already_landed() {
        // The crash hit BETWEEN the post-restore verify and the marker-clear commit: the restored
        // canonical is already in place, so recovery must JUST reconcile config + clear the marker,
        // never re-copy (there is no snapshot on disk to copy from, proving it took this branch).
        let dir = tempdir().expect("tempdir");
        let paths = project_paths_with_root(dir.path());
        ensure_paths(&paths).expect("ensure paths");
        drop(
            open_archive_connection(&paths, &plaintext_archive_config(), None)
                .expect("seed the already-landed plaintext archive"),
        );
        let landed_bytes = fs::read(&paths.archive_database_path).expect("read landed archive");

        // previous_config = None exercises the `load_config` fallback in reconcile_restore_config.
        let journal = RestoreJournal {
            version: 1,
            timestamp: "2026-06-30T00-00-00Z".to_string(),
            snapshot_ref: paths
                .raw_snapshots_dir
                .join("rekey")
                .join("gone.sqlite")
                .display()
                .to_string(),
            restored_mode: ArchiveMode::Plaintext,
            quarantine_dir: paths.quarantine_dir.join("2026-06-30T00-00-00Z").display().to_string(),
            previous_config: None,
        };
        write_restore_journal(&paths, &journal).expect("seed the restore marker");

        assert!(recover_interrupted_restore(&paths).expect("recovery must succeed"));
        // The canonical is byte-for-byte UNCHANGED — reconcile-only, never re-copied.
        assert_eq!(
            fs::read(&paths.archive_database_path).expect("read canonical"),
            landed_bytes,
            "the already-landed canonical is not re-copied",
        );
        verify_database_integrity(&paths.archive_database_path, None).expect("still sound");
        let reconciled = crate::config::load_config(&paths).expect("load reconciled config");
        assert!(matches!(reconciled.archive_mode, ArchiveMode::Plaintext), "config converged");
        assert!(reconciled.initialized, "config marked initialized");
        assert!(!restore_journal_path(&paths).exists(), "the marker is cleared");
    }

    #[test]
    fn recover_interrupted_restore_without_a_marker_is_a_noop() {
        let dir = tempdir().expect("tempdir");
        let paths = project_paths_with_root(dir.path());
        assert!(
            !recover_interrupted_restore(&paths).expect("a missing marker must not error"),
            "a missing marker is a cheap no-op",
        );
        assert!(!restore_journal_path(&paths).exists(), "no marker file is created");
    }

    #[test]
    fn recover_interrupted_restore_fails_closed_on_a_corrupt_marker_when_canonical_is_absent() {
        // The headline invariant: an UNPARSEABLE marker beside an ABSENT canonical must NOT be
        // silently dropped (which would let launch boot an empty archive over the quarantined
        // originals). It fails closed — leaves the marker + surfaces Unrecoverable.
        let dir = tempdir().expect("tempdir");
        let paths = project_paths_with_root(dir.path());
        ensure_paths(&paths).expect("ensure paths");
        fs::write(restore_journal_path(&paths), b"not valid json").expect("seed corrupt marker");

        let error = recover_interrupted_restore(&paths)
            .expect_err("a corrupt marker over an absent canonical must fail closed");
        let rendered = format!("{error:#}");
        assert!(
            rendered.contains("present but unreadable") && rendered.contains("absent"),
            "the error must name the fail-closed cause, got: {rendered}",
        );
        assert!(restore_journal_path(&paths).exists(), "the corrupt marker is LEFT for the GUI");

        // The launch path surfaces it as Unrecoverable and never recreates an empty canonical.
        let outcome = recover_archive_on_launch(&paths, &plaintext_archive_config(), None)
            .expect("recovery returns Ok carrying an Unrecoverable report");
        let report = match outcome {
            LaunchRecovery::Unrecoverable(report) => report,
            other => panic!("expected Unrecoverable, got {other:?}"),
        };
        assert!(matches!(report.kind, ArchiveRecoveryKind::InterruptedRestoreUnresolved));
        assert_eq!(
            detect_disk_encryption_mode(&paths.archive_database_path),
            DiskEncryptionMode::Absent,
            "no empty archive was booted",
        );
        assert!(restore_journal_path(&paths).exists(), "the marker is still present after launch");
    }

    #[test]
    fn recover_interrupted_restore_drops_a_corrupt_marker_when_the_canonical_is_present() {
        // A corrupt marker over a PRESENT, sound canonical is unusable but safe to drop: the canonical
        // is the source of truth (step 4 reconciles config to it). Recovery drops it untouched.
        let dir = tempdir().expect("tempdir");
        let paths = project_paths_with_root(dir.path());
        ensure_paths(&paths).expect("ensure paths");
        seed_verifiable_database(&paths.archive_database_path, None);
        let landed_bytes = fs::read(&paths.archive_database_path).expect("read canonical");
        fs::write(restore_journal_path(&paths), b"not valid json").expect("seed corrupt marker");

        assert!(
            recover_interrupted_restore(&paths)
                .expect("dropping a usable-canonical corrupt marker"),
            "a corrupt marker over a present canonical is dropped (returns true)",
        );
        assert!(!restore_journal_path(&paths).exists(), "the corrupt marker is removed");
        assert_eq!(
            fs::read(&paths.archive_database_path).expect("read canonical"),
            landed_bytes,
            "the present canonical is byte-for-byte untouched",
        );
    }

    #[test]
    fn restore_journal_round_trips_through_disk() {
        let dir = tempdir().expect("tempdir");
        let paths = project_paths_with_root(dir.path());
        fs::create_dir_all(paths.archive_database_path.parent().expect("archive parent"))
            .expect("archive dir");

        assert!(read_restore_journal(&paths).expect("absent read").is_none());
        assert!(!interrupted_restore_marker_present(&paths));

        let journal = RestoreJournal {
            version: 1,
            timestamp: "2026-06-30T00-00-00Z".to_string(),
            snapshot_ref: "/snapshots/rekey/snap.sqlite".to_string(),
            restored_mode: ArchiveMode::Encrypted,
            quarantine_dir: "/quarantine/2026-06-30T00-00-00Z".to_string(),
            previous_config: Some("cfg".to_string()),
        };
        write_restore_journal(&paths, &journal).expect("write");
        assert!(interrupted_restore_marker_present(&paths));
        let loaded = read_restore_journal(&paths).expect("read").expect("present");
        assert_eq!(loaded.version, 1);
        assert!(matches!(loaded.restored_mode, ArchiveMode::Encrypted));
        assert_eq!(loaded.snapshot_ref, "/snapshots/rekey/snap.sqlite");
        assert_eq!(loaded.quarantine_dir, "/quarantine/2026-06-30T00-00-00Z");
        assert_eq!(loaded.previous_config.as_deref(), Some("cfg"));
    }

    #[test]
    fn canonical_restore_already_landed_judges_each_on_disk_state() {
        let dir = tempdir().expect("tempdir");
        let paths = project_paths_with_root(dir.path());
        fs::create_dir_all(paths.archive_database_path.parent().expect("archive parent"))
            .expect("archive dir");
        let archive = &paths.archive_database_path;

        // Absent: the install never landed.
        assert!(!canonical_restore_already_landed(&paths, &ArchiveMode::Plaintext));

        // A real, sound plaintext archive matching the restored mode: landed.
        seed_verifiable_database(archive, None);
        assert!(canonical_restore_already_landed(&paths, &ArchiveMode::Plaintext));
        // A mode MISMATCH (restored Encrypted over a plaintext file) is NOT landed.
        assert!(!canonical_restore_already_landed(&paths, &ArchiveMode::Encrypted));

        // A plaintext HEADER over a non-database body fails quick_check -> not landed.
        write_plaintext_db(archive);
        assert!(!canonical_restore_already_landed(&paths, &ArchiveMode::Plaintext));

        // An encrypted-on-disk canonical cannot be key-free-verified -> never "landed" key-free.
        write_encrypted_db(archive);
        assert!(!canonical_restore_already_landed(&paths, &ArchiveMode::Encrypted));
    }

    #[test]
    fn snapshot_still_restorable_judges_each_on_disk_state() {
        let dir = tempdir().expect("tempdir");
        let snap = dir.path().join("snap.sqlite");

        // Absent: gone or truncated.
        assert!(!snapshot_still_restorable(&snap, &ArchiveMode::Plaintext));

        // A real plaintext snapshot matching the restored mode: restorable.
        seed_verifiable_database(&snap, None);
        assert!(snapshot_still_restorable(&snap, &ArchiveMode::Plaintext));
        // Mode mismatch (restored Encrypted) is not restorable.
        assert!(!snapshot_still_restorable(&snap, &ArchiveMode::Encrypted));

        // A plaintext header over a non-database body fails quick_check.
        fs::write(&snap, b"SQLite format 3\0not a database").expect("write fake plaintext");
        assert!(!snapshot_still_restorable(&snap, &ArchiveMode::Plaintext));

        // An encrypted-header snapshot matches by header (it was keyed-verified at restore start).
        fs::write(&snap, [7u8; 32]).expect("write encrypted header");
        assert!(snapshot_still_restorable(&snap, &ArchiveMode::Encrypted));
        // ...but an encrypted snapshot for a Plaintext restore is a mode mismatch.
        assert!(!snapshot_still_restorable(&snap, &ArchiveMode::Plaintext));
    }

    #[test]
    fn rollback_restore_moves_back_originals_and_drops_config_when_none_was_captured() {
        let dir = tempdir().expect("tempdir");
        let paths = project_paths_with_root(dir.path());
        ensure_paths(&paths).expect("ensure paths");

        // A dated quarantine dir holding the pre-restore originals + a hot -wal sidecar.
        let quarantine_dir = paths.quarantine_dir.join("2026-06-30T00-00-00Z");
        fs::create_dir_all(&quarantine_dir).expect("quarantine dir");
        seed_verifiable_database(&quarantine_dir.join("history-vault.sqlite"), None);
        fs::write(quarantine_dir.join("history-vault.sqlite-wal"), b"hot wal").expect("seed wal");
        seed_verifiable_database(&quarantine_dir.join("source-evidence.sqlite"), None);

        // A config the rollback must REMOVE (previous_config = None: none existed pre-restore).
        crate::config::save_config(&paths, &plaintext_archive_config()).expect("save config");

        let journal = RestoreJournal {
            version: 1,
            timestamp: "2026-06-30T00-00-00Z".to_string(),
            snapshot_ref: "/snapshots/rekey/gone.sqlite".to_string(),
            restored_mode: ArchiveMode::Plaintext,
            quarantine_dir: quarantine_dir.display().to_string(),
            previous_config: None,
        };
        rollback_restore(&paths, &journal)
            .expect("rollback succeeds when the originals are present");

        // Both canonical DBs moved back, the hot -wal followed the main file, the config was removed.
        assert!(paths.archive_database_path.exists(), "the history-vault original is restored");
        assert!(
            PathBuf::from(format!("{}-wal", paths.archive_database_path.display())).exists(),
            "the original's hot -wal followed it back",
        );
        assert!(
            paths.source_evidence_database_path.exists(),
            "the source-evidence original is restored",
        );
        assert!(!paths.config_path.exists(), "previous_config=None removes the config");
    }

    #[test]
    fn restore_quarantined_original_is_a_noop_for_a_rootless_canonical() {
        let dir = tempdir().expect("tempdir");
        let quarantine_dir = dir.path().join("quarantine");
        fs::create_dir_all(&quarantine_dir).expect("quarantine dir");
        // A rootless canonical path has no file_name -> nothing to move (the defensive guard).
        restore_quarantined_original(&quarantine_dir, Path::new("/"));
        assert_eq!(
            fs::read_dir(&quarantine_dir).expect("read quarantine").count(),
            0,
            "a path with no file name moves nothing",
        );
    }

    #[test]
    fn quarantine_durably_fsyncs_so_originals_survive_a_crash() {
        // MEDIUM-1: `quarantine_canonical_archive` fsyncs the dated quarantine dir, so the cross-dir
        // move of the canonical DBs is crash-durable. Driving it directly proves the fsync'd move
        // completed + surfaced no error (the fsync error arm is covered by durable_io's fsync_dir tests).
        let dir = tempdir().expect("tempdir");
        let paths = project_paths_with_root(dir.path());
        ensure_paths(&paths).expect("ensure paths");
        drop(
            open_archive_connection(&paths, &plaintext_archive_config(), None)
                .expect("seed live archive"),
        );

        let quarantine_dir = quarantine_canonical_archive(&paths, "2026-06-30T00-00-00Z")
            .expect("the durably-fsynced quarantine must succeed");
        assert!(
            quarantine_dir.join("history-vault.sqlite").exists(),
            "the original is in the fsync'd quarantine dir (the durable cross-dir move completed)",
        );
    }

    #[cfg(unix)]
    #[test]
    fn quarantine_canonical_archive_surfaces_a_dir_fsync_failure() {
        // MEDIUM-1, behaviourally: the quarantine dir fsync is load-bearing. Inject a fatal dir-fsync
        // errno and assert the quarantine ABORTS with the fsync context — so deleting the `fsync_dir`
        // call (or swallowing its error) would fail this test, not just lower coverage.
        let dir = tempdir().expect("tempdir");
        let paths = project_paths_with_root(dir.path());
        ensure_paths(&paths).expect("ensure paths");
        drop(
            open_archive_connection(&paths, &plaintext_archive_config(), None)
                .expect("seed live archive"),
        );

        // The only `full_fsync` in `quarantine_canonical_archive` is the dir fsync, so one queued
        // fault hits exactly it. Inject immediately before the call (nothing between issues a fsync).
        crate::durable_io::inject_fsync_faults(vec![Some(libc::EIO)]);
        let timestamp = now_rfc3339().replace(':', "-");
        let error = quarantine_canonical_archive(&paths, &timestamp)
            .expect_err("a fatal dir-fsync errno must abort the quarantine");
        assert!(
            format!("{error:#}").contains("fsyncing quarantine dir"),
            "the error must name the dir fsync, got: {error:#}",
        );
    }

    #[test]
    fn launch_recovery_lets_a_pending_restore_supersede_a_stale_rekey_marker() {
        // A full-archive restore SUPERSEDES any interrupted rekey (it replaces the whole canonical).
        // A stale rekey marker beside the absent canonical must NOT starve the auto-completable
        // restore: launch recovery skips rekey recovery while the restore is pending, completes the
        // restore, and purges the stale rekey marker. Without the precedence skip this would surface
        // Unrecoverable(InterruptedRekeyUnresolved).
        let dir = tempdir().expect("tempdir");
        let paths = project_paths_with_root(dir.path());
        ensure_paths(&paths).expect("ensure paths");
        let config = plaintext_archive_config();
        seed_and_crash_mid_restore(&paths, &config, None);

        // Plant a stale rekey marker beside the (now absent) canonical — the competing crash marker.
        write_rekey_journal(
            &paths,
            &sample_journal(ArchiveMode::Plaintext, ArchiveMode::Encrypted, None),
        )
        .expect("plant a stale rekey marker");
        assert!(rekey_journal_path(&paths).exists(), "the stale rekey marker is planted");

        let outcome = recover_archive_on_launch(&paths, &plaintext_archive_config(), None)
            .expect("launch recovery must not error");
        assert!(
            !matches!(outcome, LaunchRecovery::Unrecoverable(_)),
            "a pending restore must supersede the stale rekey marker, not surface Unrecoverable: {outcome:?}",
        );

        // The restore completed from the snapshot — data survived, both markers are cleared.
        assert_ne!(
            detect_disk_encryption_mode(&paths.archive_database_path),
            DiskEncryptionMode::Absent,
            "the canonical history-vault is restored",
        );
        verify_database_integrity(&paths.archive_database_path, None)
            .expect("the restored canonical passes quick_check");
        assert_eq!(
            sentinel_backup_run_count(&paths, &plaintext_archive_config(), None),
            1,
            "the sentinel run survived — the restore completed, not an empty archive",
        );
        assert!(!restore_journal_path(&paths).exists(), "the restore marker is cleared");
        assert!(
            !rekey_journal_path(&paths).exists(),
            "the superseded rekey marker is purged (clear_superseded_crash_markers)",
        );
    }
}
