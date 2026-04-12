//! Archive-facing worker flows.
//!
//! This module owns the worker entrypoints that talk to the canonical archive:
//! backup, query/export, snapshot restore, retention, import/rollback, doctor,
//! and remote backup upload/verify.
//!
//! The worker layer is allowed to chain follow-up actions such as remote backup
//! upload or AI queue work, but it must keep those steps honest. In practice
//! that means archive mutations still finish first, follow-up failures are
//! surfaced as warnings instead of being hidden, and the explicit/manual
//! boundaries described in the design docs remain visible in the returned
//! payloads.

use crate::{
    context::{
        ai_archive_connection, load_unlocked_config, resolved_app_lock_status,
        selected_optional_embedding_runtime,
    },
    intelligence::maybe_spawn_ai_queue_drain,
};
use anyhow::{Context, Result};
use serde_json::json;
use vault_core::{
    AiIndexRequest, BackupProgressEvent, ClearDerivedIntelligenceReport, DashboardSnapshot,
    ExportRequest, HealthRepairReport, HealthReport, HistoryQuery, HistoryQueryResponse,
    ImportBatchDetail, RemoteBackupPreview, RemoteBackupResult, RemoteBackupVerification,
    RunInsightsRequest, TakeoutInspection, TakeoutRequest, ai_queue,
    clear_derived_intelligence_state, doctor, export_history, import_takeout, inspect_takeout,
    intelligence_runtime::{
        claim_deterministic_rebuild_job, enqueue_deterministic_rebuild_job,
        mark_intelligence_job_failed, mark_intelligence_job_succeeded,
        update_intelligence_job_artifact,
    },
    list_history, load_audit_run_detail, load_dashboard_snapshot, preview_import_batch,
    preview_remote_backup, repair_health_issues, restore_import_batch, revert_import_batch,
    run_backup_with_progress, run_insights_with_progress, run_remote_backup, verify_remote_backup,
};
use vault_platform::keyring_get_s3_credentials;

/// Previews replaying a saved browser-source checkpoint into the canonical archive.
pub fn preview_snapshot_restore_plan(
    session_database_key: Option<&str>,
    request: &vault_core::SnapshotRestoreRequest,
) -> Result<vault_core::SnapshotRestorePreview> {
    let paths = vault_core::project_paths()?;
    let config = load_unlocked_config(&paths)?;
    vault_core::preview_snapshot_restore(&paths, &config, session_database_key, request)
}

/// Executes the saved-checkpoint replay flow and returns the resulting backup report.
pub fn run_snapshot_restore_plan(
    session_database_key: Option<&str>,
    request: &vault_core::SnapshotRestoreRequest,
) -> Result<vault_core::BackupReport> {
    let paths = vault_core::project_paths()?;
    let config = load_unlocked_config(&paths)?;
    vault_core::run_snapshot_restore(&paths, &config, session_database_key, request)
}

/// Builds the manual-first retention preview for local rebuildable artifacts.
pub fn preview_retention_plan(
    session_database_key: Option<&str>,
) -> Result<vault_core::RetentionPreview> {
    let paths = vault_core::project_paths()?;
    let config = load_unlocked_config(&paths).unwrap_or_default();
    vault_core::preview_retention(&paths, &config, session_database_key)
}

/// Executes the explicit retention prune request.
pub fn run_retention_plan(
    session_database_key: Option<&str>,
    request: &vault_core::RetentionPruneRequest,
) -> Result<vault_core::RetentionPruneResult> {
    let paths = vault_core::project_paths()?;
    let config = load_unlocked_config(&paths)?;
    vault_core::run_retention_prune(&paths, &config, session_database_key, request)
}

/// Runs a backup without exposing per-profile progress.
pub fn run_backup_now(
    session_database_key: Option<&str>,
    due_only: bool,
) -> Result<vault_core::BackupReport> {
    run_backup_now_with_progress(session_database_key, due_only, |_| {})
}

/// Runs a backup and optionally executes configured post-backup follow-ups.
///
/// Archive ingestion completes first. Remote backup upload and AI auto-index are
/// best-effort follow-ups whose failures are returned as warnings so the caller
/// can distinguish “archive write succeeded” from “secondary automation also
/// succeeded”.
pub fn run_backup_now_with_progress<F>(
    session_database_key: Option<&str>,
    due_only: bool,
    mut report_progress: F,
) -> Result<vault_core::BackupReport>
where
    F: FnMut(BackupProgressEvent),
{
    let paths = vault_core::project_paths()?;
    let config = load_unlocked_config(&paths)?;
    let mut report =
        run_backup_with_progress(&paths, &config, session_database_key, due_only, |event| {
            report_progress(event);
        })?;
    if !report.due_skipped
        && config.remote_backup.enabled
        && config.remote_backup.upload_after_backup
    {
        match keyring_get_s3_credentials()? {
            Some(credentials) => {
                let remote = run_remote_backup(&paths, &config, session_database_key, &credentials)?;
                if remote.uploaded {
                    report.remote_backup = Some(remote);
                } else {
                    report.warnings.push(remote.message.clone());
                    report.remote_backup = Some(remote);
                }
            }
            None => report
                .warnings
                .push("Remote backup is enabled, but S3 credentials are not stored in the system keyring.".to_string()),
        }
    }
    if !report.due_skipped
        && config.ai.enabled
        && config.ai.semantic_index_enabled
        && config.ai.auto_index_after_backup
    {
        let auto_index_request = config
            .ai
            .embedding_provider_id
            .as_ref()
            .map(|provider_id| AiIndexRequest {
                provider_id: Some(provider_id.clone()),
                ..AiIndexRequest::default()
            })
            .unwrap_or_default();
        match ai_archive_connection(&paths, &config, session_database_key) {
            Ok(connection) => match ai_queue::enqueue_index_job(
                &connection,
                &auto_index_request,
                config.ai.job_queue_paused,
            ) {
                Ok(job) if config.ai.job_queue_paused => report.warnings.push(format!(
                    "AI auto-index queued job {} while the AI queue is paused.",
                    job.id
                )),
                Ok(_job) => {
                    maybe_spawn_ai_queue_drain(&paths, &config, session_database_key, 1);
                }
                Err(error) => report
                    .warnings
                    .push(format!("AI auto-index could not enqueue a follow-up job: {error}")),
            },
            Err(error) => report.warnings.push(format!(
                "AI auto-index is enabled, but the embedding provider is not ready: {error}"
            )),
        }
    }
    if !report.due_skipped && backup_changed_archive(report.run.as_ref()) {
        if let Err(error) =
            enqueue_and_spawn_deterministic_refresh(&paths, &config, session_database_key)
        {
            report
                .warnings
                .push(format!("Deterministic insights could not refresh after backup: {error}"));
        }
    }
    Ok(report)
}

/// Queries visible archive history rows with the canonical filter contract.
pub fn query_history(
    session_database_key: Option<&str>,
    query: HistoryQuery,
) -> Result<HistoryQueryResponse> {
    let paths = vault_core::project_paths()?;
    let config = load_unlocked_config(&paths)?;
    list_history(&paths, &config, session_database_key, query)
}

/// Loads the dashboard snapshot read model for the current unlocked session.
pub fn dashboard_snapshot(session_database_key: Option<&str>) -> Result<DashboardSnapshot> {
    let paths = vault_core::project_paths()?;
    let config = load_unlocked_config(&paths)?;
    load_dashboard_snapshot(&paths, &config, session_database_key)
}

/// Loads audit detail for a specific run id.
pub fn audit_run_detail(
    session_database_key: Option<&str>,
    run_id: i64,
) -> Result<vault_core::AuditRunDetail> {
    let paths = vault_core::project_paths()?;
    let config = load_unlocked_config(&paths)?;
    load_audit_run_detail(&paths, &config, session_database_key, run_id)
}

/// Exports the currently visible history query in the requested format.
pub fn export_query(
    session_database_key: Option<&str>,
    request: ExportRequest,
) -> Result<vault_core::ExportResult> {
    let paths = vault_core::project_paths()?;
    let config = load_unlocked_config(&paths)?;
    export_history(&paths, &config, session_database_key, request)
}

/// Builds the remote-backup preview bundle metadata without uploading anything.
pub fn preview_remote_backup_bundle() -> Result<RemoteBackupPreview> {
    let paths = vault_core::project_paths()?;
    let config = load_unlocked_config(&paths)?;
    preview_remote_backup(&paths, &config)
}

/// Uploads the latest remote-backup bundle with the stored S3 credentials.
pub fn upload_remote_backup_bundle(
    session_database_key: Option<&str>,
) -> Result<RemoteBackupResult> {
    let paths = vault_core::project_paths()?;
    let config = load_unlocked_config(&paths)?;
    let credentials = keyring_get_s3_credentials()?
        .context("store S3 credentials in Settings before running a remote backup")?;
    run_remote_backup(&paths, &config, session_database_key, &credentials)
}

/// Verifies a built remote-backup bundle against the v1 restore contract.
pub fn verify_remote_backup_bundle(
    session_database_key: Option<&str>,
    bundle_path: &str,
) -> Result<RemoteBackupVerification> {
    let paths = vault_core::project_paths()?;
    let config = load_unlocked_config(&paths)?;
    let _ = resolved_app_lock_status(&paths, &config)?;
    verify_remote_backup(std::path::Path::new(bundle_path), session_database_key)
}

/// Clears rebuildable intelligence state while leaving canonical archive facts intact.
pub fn clear_derived_intelligence(
    session_database_key: Option<&str>,
) -> Result<ClearDerivedIntelligenceReport> {
    let paths = vault_core::project_paths()?;
    let config = load_unlocked_config(&paths)?;
    clear_derived_intelligence_state(&paths, &config, session_database_key)
}

/// Inspects a Takeout source without mutating the archive.
pub fn inspect_takeout_source(request: &TakeoutRequest) -> Result<TakeoutInspection> {
    let paths = vault_core::project_paths()?;
    inspect_takeout(&paths, request)
}

/// Imports a Takeout source into the canonical archive.
pub fn import_takeout_source(
    session_database_key: Option<&str>,
    request: &TakeoutRequest,
) -> Result<TakeoutInspection> {
    let paths = vault_core::project_paths()?;
    let config = load_unlocked_config(&paths)?;
    let mut inspection = import_takeout(&paths, &config, session_database_key, request)?;
    if inspection.imported_items > 0 {
        match enqueue_and_spawn_deterministic_refresh(&paths, &config, session_database_key) {
            Ok(job_id) => inspection.notes.push(format!(
                "Deterministic insights refresh job {job_id} was queued automatically after import and will finish in the background."
            )),
            Err(error) => inspection.notes.push(format!(
                "Deterministic insights could not refresh automatically after import: {error}"
            )),
        }
    }
    Ok(inspection)
}

/// Loads the preview/read-model detail for one import batch.
pub fn preview_import_batch_detail(
    session_database_key: Option<&str>,
    batch_id: i64,
) -> Result<ImportBatchDetail> {
    let paths = vault_core::project_paths()?;
    let config = load_unlocked_config(&paths)?;
    preview_import_batch(&paths, &config, session_database_key, batch_id)
}

/// Soft-hides one imported batch and returns its updated detail.
pub fn revert_import_batch_detail(
    session_database_key: Option<&str>,
    batch_id: i64,
) -> Result<ImportBatchDetail> {
    let paths = vault_core::project_paths()?;
    let config = load_unlocked_config(&paths)?;
    revert_import_batch(&paths, &config, session_database_key, batch_id)
}

/// Restores one previously reverted import batch to visible status.
pub fn restore_import_batch_detail(
    session_database_key: Option<&str>,
    batch_id: i64,
) -> Result<ImportBatchDetail> {
    let paths = vault_core::project_paths()?;
    let config = load_unlocked_config(&paths)?;
    restore_import_batch(&paths, &config, session_database_key, batch_id)
}

/// Runs doctor checks against the current archive.
pub fn doctor_report(session_database_key: Option<&str>) -> Result<HealthReport> {
    let paths = vault_core::project_paths()?;
    let config = load_unlocked_config(&paths)?;
    doctor(&paths, &config, session_database_key)
}

/// Runs the repair path for doctor-detected issues.
pub fn repair_health(session_database_key: Option<&str>) -> Result<HealthRepairReport> {
    let paths = vault_core::project_paths()?;
    let config = load_unlocked_config(&paths)?;
    repair_health_issues(&paths, &config, session_database_key)
}

fn backup_changed_archive(run: Option<&vault_core::BackupRunOverview>) -> bool {
    run.is_some_and(|run| run.new_visits > 0 || run.new_urls > 0 || run.new_downloads > 0)
}

fn enqueue_and_spawn_deterministic_refresh(
    paths: &vault_core::ProjectPaths,
    config: &vault_core::AppConfig,
    session_database_key: Option<&str>,
) -> Result<i64> {
    let connection = ai_archive_connection(paths, config, session_database_key)?;
    let request = RunInsightsRequest::default();
    let job_id = enqueue_deterministic_rebuild_job(
        &connection,
        &request,
        "Archive data changed and deterministic insights need a refresh.",
    )?;
    if !config.ai.job_queue_paused {
        spawn_deterministic_refresh(paths.clone(), config.clone(), session_database_key, job_id);
    }
    Ok(job_id)
}

fn spawn_deterministic_refresh(
    paths: vault_core::ProjectPaths,
    config: vault_core::AppConfig,
    session_database_key: Option<&str>,
    job_id: i64,
) {
    let session_database_key = session_database_key.map(str::to_owned);
    let _ = std::thread::Builder::new()
        .name(format!("pathkeep-deterministic-refresh-{job_id}"))
        .spawn(move || {
            if let Err(error) = execute_deterministic_refresh_job(
                &paths,
                &config,
                session_database_key.as_deref(),
                job_id,
            ) {
                eprintln!(
                    "PathKeep could not finish deterministic refresh job {} in the background: {error:#}",
                    job_id
                );
            }
        });
}

fn execute_deterministic_refresh_job(
    paths: &vault_core::ProjectPaths,
    config: &vault_core::AppConfig,
    session_database_key: Option<&str>,
    job_id: i64,
) -> Result<vault_core::RunInsightsReport> {
    let connection = ai_archive_connection(paths, config, session_database_key)?;
    let payload = claim_deterministic_rebuild_job(&connection, job_id)?
        .with_context(|| format!("deterministic rebuild job {job_id} was not ready to run"))?;
    let mut refresh_config = config.clone();
    // Automatic post-backup/import refresh must not hold the caller hostage
    // while tens of thousands of derived enrichment jobs are scheduled or run.
    refresh_config.ai.enrichment_enabled = false;
    let embedding_provider = selected_optional_embedding_runtime(&refresh_config).ok().flatten();
    let initial_profile =
        payload.request.profile_id.as_deref().unwrap_or("all profiles").to_string();
    let _ = update_intelligence_job_artifact(
        &connection,
        job_id,
        &json!({
            "kind": "deterministic-rebuild",
            "phase": "Queued work started",
            "detail": format!("Preparing a deterministic rebuild for {initial_profile}."),
            "completedSteps": 0,
            "totalSteps": 8,
            "progressPercent": 0.0
        }),
    );
    match run_insights_with_progress(
        paths,
        &refresh_config,
        session_database_key,
        embedding_provider.as_ref(),
        &payload.request,
        |progress| {
            let artifact = json!({
                "kind": "deterministic-rebuild",
                "phase": progress.phase_label,
                "detail": progress.detail,
                "completedSteps": progress.phase_step,
                "totalSteps": progress.phase_count,
                "processedItems": progress.processed_items,
                "totalItems": progress.total_items,
                "progressPercent": progress.percent(),
            });
            let _ = update_intelligence_job_artifact(&connection, job_id, &artifact);
        },
    ) {
        Ok(report) => {
            mark_intelligence_job_succeeded(
                &connection,
                job_id,
                &json!({
                    "kind": "deterministic-rebuild",
                    "phase": "Completed",
                    "detail": format!(
                        "{} visits processed, {} cards ready.",
                        report.processed_visits,
                        report.card_count
                    ),
                    "completedSteps": 8,
                    "totalSteps": 8,
                    "processedItems": report.processed_visits,
                    "totalItems": report.processed_visits,
                    "progressPercent": 100.0,
                    "processedVisits": report.processed_visits,
                    "cardCount": report.card_count,
                    "queryGroupCount": report.query_group_count,
                    "threadCount": report.thread_count,
                    "notes": report.notes,
                }),
            )?;
            Ok(report)
        }
        Err(error) => {
            mark_intelligence_job_failed(&connection, job_id, &error.to_string())?;
            Err(error)
        }
    }
}
