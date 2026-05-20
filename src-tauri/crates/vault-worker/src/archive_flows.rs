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
        selected_embedding_provider_runtime,
    },
    intelligence::{maybe_spawn_ai_queue_drain, maybe_spawn_intelligence_queue_drain},
};
use anyhow::{Context, Result};
use vault_core::{
    AiIndexRequest, AiQueueJob, BackupProgressEvent, BrowserHistoryImportRequest,
    ClearDerivedIntelligenceReport, CoreIntelligenceRebuildRequest, DashboardSnapshot,
    ExportRequest, HealthRepairReport, HealthReport, HistoryQuery, HistoryQueryResponse,
    ImportBatchDetail, ImportProgressEvent, RemoteBackupPreview, RemoteBackupResult,
    RemoteBackupVerification, TakeoutInspection, TakeoutRequest, ai_queue,
    clear_derived_intelligence_state, doctor, export_history, import_browser_history_with_progress,
    import_takeout_with_progress, inspect_browser_history, inspect_takeout,
    intelligence_runtime::{
        DAILY_ROLLUP_JOB_TYPE, STRUCTURAL_REBUILD_JOB_TYPE, VISIT_DERIVE_JOB_TYPE,
        enqueue_core_intelligence_job, mark_all_deterministic_modules_stale,
    },
    list_history, load_audit_run_detail, load_dashboard_snapshot, preview_import_batch,
    preview_remote_backup, repair_health_issues, restore_import_batch, revert_import_batch,
    run_backup_with_progress, run_remote_backup, verify_remote_backup,
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
        match selected_embedding_provider_runtime(&config, None) {
            Ok(provider) => {
                let auto_index_request = AiIndexRequest {
                    provider_id: Some(provider.config.id),
                    ..AiIndexRequest::default()
                };
                if append_ai_auto_index_archive_result(
                    &mut report.warnings,
                    ai_archive_connection(&paths, &config, session_database_key),
                    &auto_index_request,
                    config.ai.job_queue_paused,
                ) {
                    maybe_spawn_ai_queue_drain(&paths, &config, session_database_key, 1);
                }
            }
            Err(error) => append_ai_auto_index_provider_warning(&mut report.warnings, error),
        }
    }
    if !report.due_skipped && backup_changed_archive(report.run.as_ref()) {
        let dirty_profiles = report
            .profiles
            .iter()
            .filter(|profile| {
                profile.new_visits > 0 || profile.new_urls > 0 || profile.new_downloads > 0
            })
            .map(|profile| profile.profile_id.clone())
            .collect::<Vec<_>>();
        append_core_refresh_backup_result(
            &mut report.warnings,
            enqueue_and_spawn_deterministic_refresh(
                &paths,
                &config,
                session_database_key,
                &dirty_profiles,
            ),
        );
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

/// Loads favicon payloads for already-visible Explorer rows without blocking the
/// main history query path on image bytes.
pub fn load_history_favicons(
    session_database_key: Option<&str>,
    entries: Vec<vault_core::HistoryFaviconLookupEntry>,
) -> Result<Vec<vault_core::HistoryFaviconLookupResult>> {
    let paths = vault_core::project_paths()?;
    let config = load_unlocked_config(&paths)?;
    vault_core::load_history_favicons(&paths, &config, session_database_key, entries)
}

/// Loads cached og:image payloads for already-visible card-mode rows.
pub fn load_history_og_images(
    session_database_key: Option<&str>,
    entries: Vec<vault_core::HistoryOgImageLookupEntry>,
) -> Result<Vec<vault_core::HistoryOgImageLookupResult>> {
    let paths = vault_core::project_paths()?;
    let config = load_unlocked_config(&paths)?;
    vault_core::og_images::load_og_images(&paths, &config, session_database_key, entries)
}

/// Bumps `last_shown_at` for the given page URLs. Frontend calls this in
/// a debounced batch after cards settle into view so user-configured
/// LRU eviction has a real signal.
pub fn mark_og_images_shown(session_database_key: Option<&str>, urls: Vec<String>) -> Result<()> {
    let paths = vault_core::project_paths()?;
    let config = load_unlocked_config(&paths)?;
    let connection =
        vault_core::archive::open_archive_connection(&paths, &config, session_database_key)?;
    vault_core::og_images::mark_og_images_shown(&connection, &urls)
}

/// Reports cache footprint to the Settings → Storage panel.
pub fn og_image_storage_stats(
    session_database_key: Option<&str>,
) -> Result<vault_core::OgImageStorageStats> {
    let paths = vault_core::project_paths()?;
    let config = load_unlocked_config(&paths)?;
    let connection =
        vault_core::archive::open_archive_connection(&paths, &config, session_database_key)?;
    vault_core::og_images::storage_stats(&connection)
}

/// Drops every og:image cache row and its blob bytes. Behind the
/// Settings → "Clear all link previews" confirm dialog.
pub fn clear_og_image_cache(
    session_database_key: Option<&str>,
) -> Result<vault_core::OgImageCleanupReport> {
    let paths = vault_core::project_paths()?;
    let config = load_unlocked_config(&paths)?;
    let connection =
        vault_core::archive::open_archive_connection(&paths, &config, session_database_key)?;
    vault_core::og_images::clear_cache(&connection)
}

/// Runs one cleanup pass using the user's configured eviction mode.
/// Settings exposes this as "Run cleanup now"; the daily schedule tick
/// will also call this in C5.
pub fn run_og_image_cleanup(
    session_database_key: Option<&str>,
) -> Result<vault_core::OgImageCleanupReport> {
    let paths = vault_core::project_paths()?;
    let config = load_unlocked_config(&paths)?;
    let connection =
        vault_core::archive::open_archive_connection(&paths, &config, session_database_key)?;
    vault_core::og_images::run_cleanup(&connection, config.og_image.cleanup)
}

/// Fetches og:image previews for the given URLs synchronously, one at
/// a time, and persists each outcome. Skips URLs whose host is on the
/// user's blocklist and short-circuits the whole call when the user
/// has disabled fetching globally.
///
/// This is the entry point Tauri commands hand to `spawn_blocking` so
/// the UI thread never sees a network stall. A future iteration can
/// add parallelism + per-host rate limiting; for now we keep the
/// politest possible default of strict serial fetching.
pub fn refetch_og_images(session_database_key: Option<&str>, urls: Vec<String>) -> Result<u32> {
    let paths = vault_core::project_paths()?;
    let config = load_unlocked_config(&paths)?;
    if !config.og_image.fetch_enabled {
        return Ok(0);
    }
    let connection =
        vault_core::archive::open_archive_connection(&paths, &config, session_database_key)?;
    let client = vault_core::og_images_fetch::build_fetch_client()
        .context("building og:image fetch client")?;
    let mut successful = 0_u32;
    for url in urls {
        if vault_core::og_images_fetch::is_host_blocked(&config.og_image.blocked_hosts, &url) {
            let outcome = vault_core::og_images_fetch::blocked_outcome(&url);
            vault_core::og_images::upsert_og_image(&connection, &outcome.as_insert(&url))?;
            continue;
        }
        let outcome = vault_core::og_images_fetch::fetch_og_image_for(&client, &url);
        if outcome.is_ok() {
            successful += 1;
        }
        vault_core::og_images::upsert_og_image(&connection, &outcome.as_insert(&url))?;
    }
    Ok(successful)
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

/// Inspects one local browser history database without mutating the archive.
pub fn inspect_browser_history_source(
    request: &BrowserHistoryImportRequest,
) -> Result<TakeoutInspection> {
    let paths = vault_core::project_paths()?;
    inspect_browser_history(&paths, request)
}

/// Imports a Takeout source into the canonical archive.
pub fn import_takeout_source(
    session_database_key: Option<&str>,
    request: &TakeoutRequest,
) -> Result<TakeoutInspection> {
    import_takeout_source_with_progress(session_database_key, request, |_| {})
}

pub fn import_takeout_source_with_progress<F>(
    session_database_key: Option<&str>,
    request: &TakeoutRequest,
    report_progress: F,
) -> Result<TakeoutInspection>
where
    F: FnMut(ImportProgressEvent),
{
    let paths = vault_core::project_paths()?;
    let config = load_unlocked_config(&paths)?;
    let mut inspection = import_takeout_with_progress(
        &paths,
        &config,
        session_database_key,
        request,
        report_progress,
    )?;
    if inspection.imported_items > 0 {
        append_core_refresh_import_result(
            &mut inspection.notes,
            enqueue_and_spawn_deterministic_refresh(
                &paths,
                &config,
                session_database_key,
                &["takeout::browser-history".to_string()],
            ),
        );
    }
    Ok(inspection)
}

/// Imports one local browser history database into the canonical archive.
pub fn import_browser_history_source(
    session_database_key: Option<&str>,
    request: &BrowserHistoryImportRequest,
) -> Result<TakeoutInspection> {
    import_browser_history_source_with_progress(session_database_key, request, |_| {})
}

pub fn import_browser_history_source_with_progress<F>(
    session_database_key: Option<&str>,
    request: &BrowserHistoryImportRequest,
    report_progress: F,
) -> Result<TakeoutInspection>
where
    F: FnMut(ImportProgressEvent),
{
    let paths = vault_core::project_paths()?;
    let config = load_unlocked_config(&paths)?;
    let mut inspection = import_browser_history_with_progress(
        &paths,
        &config,
        session_database_key,
        request,
        report_progress,
    )?;
    if inspection.imported_items > 0 {
        if let Some(profile_id) =
            inspection.import_batch.as_ref().map(|batch| batch.profile_id.clone())
        {
            append_core_refresh_import_result(
                &mut inspection.notes,
                enqueue_and_spawn_deterministic_refresh(
                    &paths,
                    &config,
                    session_database_key,
                    &[profile_id],
                ),
            );
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
    dirty_profiles: &[String],
) -> Result<Vec<i64>> {
    let connection = ai_archive_connection(paths, config, session_database_key)?;
    mark_all_deterministic_modules_stale(
        &connection,
        "Archive data changed and Core Intelligence refresh jobs were queued.",
    )?;
    let rebuild_scopes = core_refresh_rebuild_scopes(dirty_profiles);
    let mut job_ids = Vec::with_capacity(rebuild_scopes.len() * 3);
    for request in &rebuild_scopes {
        let scope_label = request.profile_id.as_deref().unwrap_or("all profiles").to_string();
        job_ids.push(enqueue_core_intelligence_job(
            &connection,
            VISIT_DERIVE_JOB_TYPE,
            request,
            &format!(
                "Archive data changed and visit-derived facts need a refresh for {scope_label}."
            ),
        )?);
        job_ids.push(enqueue_core_intelligence_job(
            &connection,
            DAILY_ROLLUP_JOB_TYPE,
            request,
            &format!("Archive data changed and daily rollups need a refresh for {scope_label}."),
        )?);
        job_ids.push(enqueue_core_intelligence_job(
            &connection,
            STRUCTURAL_REBUILD_JOB_TYPE,
            request,
            &format!("Archive data changed and structural Core Intelligence entities need a refresh for {scope_label}."),
        )?);
    }
    if !config.ai.job_queue_paused {
        maybe_spawn_intelligence_queue_drain(paths, config, session_database_key, job_ids.len());
    }
    Ok(job_ids)
}

fn append_core_refresh_backup_result(warnings: &mut Vec<String>, result: Result<Vec<i64>>) {
    if let Err(error) = result {
        warnings.push(format!("Core Intelligence could not refresh after backup: {error}"));
    }
}

fn append_ai_auto_index_enqueue_result(
    warnings: &mut Vec<String>,
    result: Result<AiQueueJob>,
    queue_paused: bool,
) -> bool {
    match result {
        Ok(job) if queue_paused => {
            warnings
                .push(format!("AI auto-index queued job {} while the AI queue is paused.", job.id));
            false
        }
        Ok(_) => true,
        Err(error) => {
            warnings.push(format!("AI auto-index could not enqueue a follow-up job: {error}"));
            false
        }
    }
}

fn append_ai_auto_index_archive_result(
    warnings: &mut Vec<String>,
    connection: Result<rusqlite::Connection>,
    request: &AiIndexRequest,
    queue_paused: bool,
) -> bool {
    match connection {
        Ok(connection) => append_ai_auto_index_enqueue_result(
            warnings,
            ai_queue::enqueue_index_job(&connection, request, queue_paused),
            queue_paused,
        ),
        Err(error) => {
            append_ai_auto_index_provider_warning(warnings, error);
            false
        }
    }
}

fn append_ai_auto_index_provider_warning(warnings: &mut Vec<String>, error: anyhow::Error) {
    warnings.push(format!(
        "AI auto-index is enabled, but the embedding provider is not ready: {error}"
    ));
}

fn append_core_refresh_import_result(notes: &mut Vec<String>, result: Result<Vec<i64>>) {
    match result {
        Ok(job_ids) => notes.push(core_refresh_import_note(&job_ids)),
        Err(error) => notes.push(format!(
            "Core Intelligence could not refresh automatically after import: {error}"
        )),
    }
}

fn core_refresh_import_note(job_ids: &[i64]) -> String {
    format!(
        "Core Intelligence refresh jobs {} were queued automatically after import and will finish in the background.",
        job_ids.iter().map(ToString::to_string).collect::<Vec<_>>().join(", ")
    )
}

fn core_refresh_rebuild_scopes(dirty_profiles: &[String]) -> Vec<CoreIntelligenceRebuildRequest> {
    if dirty_profiles.is_empty() {
        vec![CoreIntelligenceRebuildRequest::default()]
    } else {
        dirty_profiles
            .iter()
            .map(|profile_id| CoreIntelligenceRebuildRequest {
                profile_id: Some(profile_id.clone()),
                ..CoreIntelligenceRebuildRequest::default()
            })
            .collect::<Vec<_>>()
    }
}

#[cfg(test)]
mod tests {
    use super::{
        append_ai_auto_index_archive_result, append_ai_auto_index_enqueue_result,
        append_ai_auto_index_provider_warning, append_core_refresh_backup_result,
        append_core_refresh_import_result, core_refresh_import_note, core_refresh_rebuild_scopes,
    };
    use vault_core::AiQueueJob;

    #[test]
    fn core_refresh_note_helpers_cover_success_error_and_scope_edges() {
        assert!(core_refresh_import_note(&[1, 2, 3]).contains("1, 2, 3"));

        let mut notes = Vec::new();
        append_core_refresh_import_result(&mut notes, Ok(vec![7, 8]));
        append_core_refresh_import_result(&mut notes, Err(anyhow::anyhow!("queue offline")));
        assert!(notes.iter().any(|note| note.contains("7, 8")));
        assert!(notes.iter().any(|note| note.contains("queue offline")));

        let mut warnings = Vec::new();
        append_core_refresh_backup_result(&mut warnings, Ok(vec![1]));
        append_core_refresh_backup_result(&mut warnings, Err(anyhow::anyhow!("archive locked")));
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("archive locked"));

        let mut ai_warnings = Vec::new();
        assert!(!append_ai_auto_index_enqueue_result(
            &mut ai_warnings,
            Ok(AiQueueJob { id: 42, ..AiQueueJob::default() }),
            true,
        ));
        assert!(ai_warnings[0].contains("queued job 42"));
        assert!(append_ai_auto_index_enqueue_result(
            &mut ai_warnings,
            Ok(AiQueueJob::default()),
            false,
        ));
        assert!(!append_ai_auto_index_enqueue_result(
            &mut ai_warnings,
            Err(anyhow::anyhow!("queue offline")),
            false,
        ));
        append_ai_auto_index_provider_warning(
            &mut ai_warnings,
            anyhow::anyhow!("provider missing"),
        );
        assert!(!append_ai_auto_index_archive_result(
            &mut ai_warnings,
            Err(anyhow::anyhow!("archive unavailable")),
            &vault_core::AiIndexRequest::default(),
            false,
        ));
        assert!(ai_warnings.iter().any(|warning| warning.contains("queue offline")));
        assert!(ai_warnings.iter().any(|warning| warning.contains("provider missing")));
        assert!(ai_warnings.iter().any(|warning| warning.contains("archive unavailable")));

        let all_profiles = core_refresh_rebuild_scopes(&[]);
        assert_eq!(all_profiles.len(), 1);
        assert!(all_profiles[0].profile_id.is_none());
        let scoped = core_refresh_rebuild_scopes(&["chrome:Default".to_string()]);
        assert_eq!(scoped[0].profile_id.as_deref(), Some("chrome:Default"));
    }
}
