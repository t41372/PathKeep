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

use super::*;

/// Previews replaying one saved checkpoint or explains why the snapshot is manual-only.
pub fn preview_snapshot_restore(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    request: &SnapshotRestoreRequest,
) -> Result<SnapshotRestorePreview> {
    ensure_paths(paths)?;
    let connection = open_archive_connection(paths, config, key)?;
    create_schema(&connection)?;
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
    let parsed = parse_profile_snapshot(&checkpoint, config, &Watermark::default())?;

    Ok(SnapshotRestorePreview {
        snapshot_path: snapshot.file_path,
        snapshot_kind: "raw-source-checkpoint".to_string(),
        source_run_id: Some(snapshot.run_id),
        source_profile_id: Some(checkpoint.profile.profile_id.clone()),
        source_browser_name: Some(checkpoint.profile.browser_name.clone()),
        created_at: Some(snapshot.created_at),
        reason: snapshot.reason,
        execute_supported: true,
        estimated_visits: parsed.history.visits.len(),
        estimated_urls: parsed.history.urls.len(),
        estimated_downloads: parsed.history.downloads.len(),
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
    create_schema(&connection)?;
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
    let restore_result = (|| -> Result<BackupProfileSummary> {
        let transaction = connection.transaction()?;
        let profile_summary = process_profile_snapshot(
            &transaction,
            run_id,
            paths,
            config,
            &checkpoint,
            &mut snapshot_artifacts,
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
    finalize_successful_run(&connection, run_id, &finished_at, &summary, &[], &manifest_hash)?;

    let git_commit = if config.git_enabled {
        git_audit::ensure_repo(&paths.audit_repo_path)?;
        git_audit::commit_all(&paths.audit_repo_path, &format!("snapshot restore run {run_id}"))?
    } else {
        None
    };

    Ok(BackupReport {
        due_skipped: false,
        reason: None,
        run: Some(BackupRunOverview { manifest_hash: Some(manifest_hash), ..summary }),
        profiles: vec![profile_summary],
        manifest_path: Some(manifest_path.display().to_string()),
        git_commit,
        warnings: Vec::new(),
        remote_backup: None,
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
    create_schema(&connection)?;
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
        match bucket.id.as_str() {
            "snapshots" => {
                let (bytes, files) = prune_snapshot_bucket(&connection, paths)?;
                deleted_bytes += bytes;
                deleted_files += files;
            }
            "exports" => {
                let (bytes, files) = remove_directory_contents(&paths.exports_dir)?;
                deleted_bytes += bytes;
                deleted_files += files;
            }
            "staging" => {
                let (bytes, files) = remove_directory_contents(&paths.staging_dir)?;
                deleted_bytes += bytes;
                deleted_files += files;
            }
            "quarantine" => {
                let (bytes, files) = remove_directory_contents(&paths.quarantine_dir)?;
                deleted_bytes += bytes;
                deleted_files += files;
            }
            _ => {}
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
            serde_json::to_string(&preview.warnings)?,
            run_id,
        ],
    )?;

    if config.git_enabled {
        git_audit::ensure_repo(&paths.audit_repo_path)?;
        let _ = git_audit::commit_all(
            &paths.audit_repo_path,
            &format!("retention prune run {run_id}"),
        )?;
    }

    let _ = manifest_path;

    Ok(RetentionPruneResult {
        run_id: Some(run_id),
        deleted_bytes,
        deleted_files,
        buckets: selected,
        warnings: preview.warnings,
    })
}

/// Rekeys or rewrites the archive into a different at-rest mode.
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

    let started_at = now_rfc3339();
    let timezone = current_timezone_name();
    let source = open_archive_connection(paths, current_config, old_key)?;
    let snapshot_path = create_rekey_snapshot(paths)?;
    let temp_path = paths.archive_database_path.with_extension("rekey.sqlite");
    let backup_path = paths.archive_database_path.with_extension("backup.sqlite");
    if temp_path.exists() {
        fs::remove_file(&temp_path)?;
    }
    if backup_path.exists() {
        fs::remove_file(&backup_path)?;
    }
    let target_key = match new_mode {
        ArchiveMode::Encrypted => Some(new_key.context("new encryption key is required")?),
        ArchiveMode::Plaintext => None,
    };
    let mut next_config = current_config.clone();
    next_config.initialized = true;
    next_config.archive_mode = new_mode.clone();
    export_archive_database(&source, &temp_path, target_key)?;
    drop(source);

    let temp_connection =
        Connection::open(&temp_path).with_context(|| format!("opening {}", temp_path.display()))?;
    temp_connection.busy_timeout(StdDuration::from_secs(5))?;
    temp_connection.pragma_update(None, "foreign_keys", true)?;
    if let Some(key) = target_key {
        apply_cipher_key(&temp_connection, key)?;
    }
    create_schema(&temp_connection)?;
    temp_connection.execute(
        "INSERT INTO runs (run_type, trigger, started_at, timezone, status, profile_scope_json, warnings_json, stats_json, due_only)
         VALUES ('rekey', 'manual', ?1, ?2, 'running', '[]', '[]', '{}', 0)",
        params![started_at, timezone],
    )?;
    let run_id = temp_connection.last_insert_rowid();
    record_snapshot_reference(
        &temp_connection,
        run_id,
        &snapshot_path,
        "before-rekey",
        &started_at,
    )?;
    drop(temp_connection);

    if let Err(error) = fs::rename(&paths.archive_database_path, &backup_path) {
        let _ = fs::remove_file(&temp_path);
        return Err(error).context("preparing archive database swap for rekey export");
    }

    if let Err(error) = fs::rename(&temp_path, &paths.archive_database_path) {
        let _ = fs::rename(&backup_path, &paths.archive_database_path);
        let _ = fs::remove_file(&temp_path);
        return Err(error).context("replacing archive database after rekey export");
    }

    let _ = fs::remove_file(&backup_path);

    match save_config(paths, &next_config)
        .and_then(|_| archive_status(paths, &next_config, target_key))
    {
        Ok(status) => {
            let connection = open_archive_connection(paths, &next_config, target_key)?;
            create_schema(&connection)?;
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
            let run_error = format!("{error:#}");
            if let Ok(connection) = open_archive_connection(paths, &next_config, target_key) {
                if create_schema(&connection).is_ok() {
                    let _ = finalize_rekey_run(
                        &connection,
                        paths,
                        run_id,
                        &current_config.archive_mode,
                        &new_mode,
                        &snapshot_path,
                        "failed",
                        Some(run_error.clone()),
                    );
                }
            }
            Err(error)
        }
    }
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
