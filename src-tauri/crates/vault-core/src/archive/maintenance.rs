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
            execute_supported: false,
            estimated_visits: 0,
            estimated_urls: 0,
            estimated_downloads: 0,
            warnings: vec![
                "This snapshot is a full archive safety copy. PathKeep currently automates restore only for saved browser source checkpoints; keep this file for manual recovery review.".to_string(),
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
    let snapshot_path = create_verified_rekey_snapshot(paths, &source, current_config, old_key)?;

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

/// Creates the rekey safety snapshot and proves it is restorable.
///
/// Checkpoints the live archive (TRUNCATE) so the copy is not WAL-incomplete, copies
/// it, then re-opens the copy with the CURRENT key and runs `quick_check`. A snapshot
/// that cannot be re-opened is worthless as a backstop, so a verification failure
/// aborts the rekey with the original archive still untouched.
fn create_verified_rekey_snapshot(
    paths: &ProjectPaths,
    source: &Connection,
    current_config: &AppConfig,
    old_key: Option<&str>,
) -> Result<PathBuf> {
    checkpoint_truncate(source, "the archive before the rekey safety snapshot")?;
    let snapshot_path = create_rekey_snapshot(paths)?;
    let snapshot_key =
        if matches!(current_config.archive_mode, ArchiveMode::Encrypted) { old_key } else { None };
    // `verify_database_integrity` already names the snapshot file in its error, so no
    // extra wrapping closure is needed (and none is left only-reachable-on-failure).
    verify_database_integrity(&snapshot_path, snapshot_key)?;
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

/// Creates the safety snapshot used by the rekey flow.
fn create_rekey_snapshot(paths: &ProjectPaths) -> Result<PathBuf> {
    let snapshot_dir = paths.raw_snapshots_dir.join("rekey");
    fs::create_dir_all(&snapshot_dir)?;
    let snapshot_path = snapshot_dir
        .join(format!("archive-before-rekey-{}.sqlite", now_rfc3339().replace(':', "-")));
    fs::copy(&paths.archive_database_path, &snapshot_path).with_context(|| {
        format!("creating rekey safety snapshot at {}", snapshot_path.display())
    })?;
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::project_paths_with_root;
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
    fn create_rekey_snapshot_reports_copy_failure_with_target_path() {
        let dir = tempdir().expect("tempdir");
        let paths = project_paths_with_root(dir.path());

        let error = create_rekey_snapshot(&paths).expect_err("missing archive cannot be copied");

        assert!(format!("{error:#}").contains("creating rekey safety snapshot"));
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
}
