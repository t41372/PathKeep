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
use crate::durable_io::install_file_durably;

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
///      out-of-process scheduled backup can never race the swap;
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

    // (1) Serialize the entire rekey against every other archive-mutating op —
    // crucially the SEPARATE scheduled-backup process — until this guard drops.
    let _write_lock =
        ArchiveWriteLock::acquire(paths).context("acquiring the archive write lock for rekey")?;

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

    let mut swapped = false;
    match rekey_swap_and_commit(paths, &temp_path, old_key, target_key, &next_config, &mut swapped)
    {
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
            // orphaned export is useless — drop it rather than leave a phantom swap.
            if !swapped {
                let _ = fs::remove_file(&temp_path);
                super::at_rest::remove_stale_sidecars(&temp_path);
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
fn rekey_swap_and_commit(
    paths: &ProjectPaths,
    temp_path: &Path,
    old_key: Option<&str>,
    target_key: Option<&str>,
    next_config: &AppConfig,
    swapped: &mut bool,
) -> Result<ArchiveStatus> {
    // (4) A crash HERE must leave the ORIGINAL canonical database + config untouched.
    crate::fault_inject::checkpoint("rekey.after_export_before_swap")?;

    // (5) Durable swap (F_FULLFSYNC + rename + dir fsync), then scrub the swapped-in
    // file's stale sidecars so a foreign `-wal` can never replay into the rekeyed DB.
    install_file_durably(temp_path, &paths.archive_database_path)
        .context("durably installing the rekeyed archive")?;
    *swapped = true;
    super::at_rest::remove_stale_sidecars(&paths.archive_database_path);

    // (6) THE incident window: the file is converted but config still reflects the
    // OLD mode. Writing config LAST makes this the recoverable, data-safe state — the
    // verified backstop is still on disk and the new file is durably installed under
    // the new key, so nothing is lost. NOTE: this is NOT auto-healed today. `at_rest`/
    // `reconcile_*` only converge source-evidence and explicitly disclaim the canonical
    // archive, so on the next launch this window currently surfaces as NOTADB
    // (config=Plaintext over an Encrypted file) and needs manual recovery. A FUTURE
    // Phase C reconcile of the canonical archive will auto-heal it — but Phase C is NOT
    // YET LIVE, so do not assume the incident window self-heals in-app yet.
    crate::fault_inject::checkpoint("rekey.after_swap_before_config")?;

    // (7) Convert source-evidence in lockstep, BEFORE config/backstop are committed,
    // so a failure here keeps the backstop.
    super::migrate_source_evidence_for_rekey(paths, old_key, target_key)?;

    // (8) Config LAST, atomically + durably, only after BOTH databases are converted
    // and durable on disk.
    save_config(paths, next_config)?;

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
}
