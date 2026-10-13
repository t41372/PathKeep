//! Responsibilities: expose archive operations and progress off the UI thread.
//! Not responsible for: storage, conversion or maintenance policy.
//! Failure modes: changed payload names, blocked UI thread or lost progress events.
//! Tests: desktop bridge dispatch and worker contract tests.

#[cfg(not(test))]
use super::blocking::run_blocking_command;
#[cfg(not(test))]
use crate::command_error::CommandError;
#[cfg(not(test))]
use crate::{session::SessionState, worker_bridge};
#[cfg(not(test))]
use tauri::{AppHandle, Emitter, State};
#[cfg(not(test))]
use vault_worker::RekeyRequest;

#[cfg(not(test))]
#[tauri::command]
/// Initializes the archive and optionally seeds the first session key, streaming
/// first-run upgrade progress on `pathkeep://archive-upgrade`.
pub(crate) async fn initialize_archive(
    app: AppHandle,
    config: vault_core::AppConfig,
    database_key: Option<String>,
    state: State<'_, SessionState>,
) -> Result<vault_core::AppSnapshot, CommandError> {
    let session = state.inner().clone();
    run_blocking_command("initialize_archive", move || {
        worker_bridge::initialize_archive_with_progress_impl(
            config,
            database_key,
            &session,
            |event| {
                let _ = app.emit(vault_core::ARCHIVE_UPGRADE_EVENT, &event);
            },
        )
    })
    .await
}

#[cfg(not(test))]
#[tauri::command]
/// Reports source counts, retained files and conversion work off the UI thread.
pub(crate) async fn assess_archive_upgrade(
    state: State<'_, SessionState>,
    database_key: Option<String>,
) -> Result<vault_core::ArchiveUpgradeAssessment, CommandError> {
    let session_database_key_state = state.inner().clone();
    run_blocking_command("assess_archive_upgrade", move || {
        worker_bridge::assess_archive_upgrade_impl(
            &session_database_key_state,
            database_key.or(session_database_key_state.get_key()?).as_deref(),
        )
    })
    .await
}

#[cfg(not(test))]
#[tauri::command]
/// Executes an archive rekey/mode switch and returns the refreshed app snapshot.
pub(crate) async fn rekey_archive(
    request: RekeyRequest,
    state: State<'_, SessionState>,
) -> Result<vault_core::AppSnapshot, CommandError> {
    let session = state.inner().clone();
    run_blocking_command("rekey_archive", move || {
        worker_bridge::rekey_archive_impl(request, &session)
    })
    .await
}

#[cfg(not(test))]
#[tauri::command]
/// Self-heals a drifted encryption-at-rest state after unlock, off the UI thread.
pub(crate) async fn reconcile_archive_encryption(
    state: State<'_, SessionState>,
) -> Result<vault_core::ReconcileReport, CommandError> {
    let session = state.inner().clone();
    run_blocking_command("reconcile_archive_encryption", move || {
        worker_bridge::reconcile_archive_encryption_impl(&session)
    })
    .await
}

#[cfg(not(test))]
#[tauri::command]
/// Previews the archive rekey plan before any encryption-mode mutation happens, off the UI thread.
pub(crate) async fn preview_rekey_archive(
    request: RekeyRequest,
) -> Result<vault_core::RekeyPreview, CommandError> {
    run_blocking_command("preview_rekey_archive", move || {
        worker_bridge::preview_rekey_archive_impl(request)
    })
    .await
}

#[cfg(not(test))]
#[tauri::command]
/// Previews a checkpoint restore without replaying it yet, off the UI thread.
pub(crate) async fn preview_snapshot_restore(
    request: vault_core::SnapshotRestoreRequest,
    state: State<'_, SessionState>,
) -> Result<vault_core::SnapshotRestorePreview, CommandError> {
    let session = state.inner().clone();
    run_blocking_command("preview_snapshot_restore", move || {
        worker_bridge::preview_snapshot_restore_impl(request, &session)
    })
    .await
}

#[cfg(not(test))]
#[tauri::command]
/// Replays a checkpoint restore and records it in the archive ledger, off the UI thread.
pub(crate) async fn run_snapshot_restore(
    request: vault_core::SnapshotRestoreRequest,
    state: State<'_, SessionState>,
) -> Result<vault_core::BackupReport, CommandError> {
    let session = state.inner().clone();
    run_blocking_command("run_snapshot_restore", move || {
        worker_bridge::run_snapshot_restore_impl(request, &session)
    })
    .await
}

#[cfg(not(test))]
#[tauri::command]
/// Lists verified full-archive safety snapshots for the recovery GUI, off the UI thread.
pub(crate) async fn list_recovery_snapshots()
-> Result<Vec<vault_core::RecoverySnapshot>, CommandError> {
    run_blocking_command("list_recovery_snapshots", move || {
        worker_bridge::list_recovery_snapshots_impl()
    })
    .await
}

#[cfg(not(test))]
#[tauri::command]
/// Runs the one-click full-archive restore from a verified snapshot, off the UI thread.
pub(crate) async fn run_full_archive_restore(
    request: vault_core::SnapshotRestoreRequest,
    key: Option<String>,
    state: State<'_, SessionState>,
) -> Result<vault_core::FullArchiveRestoreReport, CommandError> {
    let session = state.inner().clone();
    run_blocking_command("run_full_archive_restore", move || {
        worker_bridge::run_full_archive_restore_impl(request, key, &session)
    })
    .await
}

#[cfg(not(test))]
#[tauri::command]
/// Shows what retention pruning would delete or preserve, off the UI thread.
pub(crate) async fn preview_retention_prune(
    state: State<'_, SessionState>,
) -> Result<vault_core::RetentionPreview, CommandError> {
    let session = state.inner().clone();
    run_blocking_command("preview_retention_prune", move || {
        worker_bridge::preview_retention_prune_impl(&session)
    })
    .await
}

#[cfg(not(test))]
#[tauri::command]
/// Executes retention pruning for the selected buckets, off the UI thread.
pub(crate) async fn run_retention_prune(
    request: vault_core::RetentionPruneRequest,
    state: State<'_, SessionState>,
) -> Result<vault_core::RetentionPruneResult, CommandError> {
    let session = state.inner().clone();
    run_blocking_command("run_retention_prune", move || {
        worker_bridge::run_retention_prune_impl(request, &session)
    })
    .await
}

#[cfg(not(test))]
#[tauri::command]
/// Starts a backup run and streams progress events back to the renderer.
///
/// Shares one path with the menu bar's "Back up now", so the menu's status
/// line and `pathkeep://backup-finished` cover backups started here too.
pub(crate) async fn run_backup_now(
    app: AppHandle,
    due_only: bool,
) -> Result<vault_core::BackupReport, CommandError> {
    run_blocking_command("run_backup_now", move || {
        crate::desktop_integration::run_backup(
            &app,
            crate::desktop_integration::BackupSource::App,
            due_only,
        )
    })
    .await
}

#[cfg(not(test))]
#[tauri::command]
/// Queries visible history facts from the canonical archive.
pub(crate) async fn query_history(
    query: vault_core::HistoryQuery,
    state: State<'_, SessionState>,
) -> Result<vault_core::HistoryQueryResponse, CommandError> {
    let session_database_key_state = state.inner().clone();
    run_blocking_command("query_history", move || {
        worker_bridge::query_history_impl(query, &session_database_key_state)
    })
    .await
}

#[cfg(not(test))]
#[tauri::command]
/// Loads the icons of the sites a screen lists, off the UI thread.
pub(crate) async fn load_site_favicons(
    domains: Vec<String>,
    state: State<'_, SessionState>,
) -> Result<Vec<vault_core::SiteFavicon>, CommandError> {
    let session = state.inner().clone();
    run_blocking_command("load_site_favicons", move || {
        worker_bridge::load_site_favicons_impl(domains, &session)
    })
    .await
}

#[cfg(not(test))]
#[tauri::command]
/// Loads favicon payloads for already-visible Explorer rows after the primary
/// page content has rendered.
pub(crate) async fn load_history_favicons(
    entries: Vec<vault_core::HistoryFaviconLookupEntry>,
    state: State<'_, SessionState>,
) -> Result<Vec<vault_core::HistoryFaviconLookupResult>, CommandError> {
    let session_database_key_state = state.inner().clone();
    run_blocking_command("load_history_favicons", move || {
        worker_bridge::load_history_favicons_impl(entries, &session_database_key_state)
    })
    .await
}

#[cfg(not(test))]
#[tauri::command]
/// Loads cached og:image payloads for already-visible card-mode rows.
pub(crate) async fn load_history_og_images(
    entries: Vec<vault_core::HistoryOgImageLookupEntry>,
    state: State<'_, SessionState>,
) -> Result<Vec<vault_core::HistoryOgImageLookupResult>, CommandError> {
    let state = state.inner().clone();
    run_blocking_command("load_history_og_images", move || {
        worker_bridge::load_history_og_images_impl(entries, &state)
    })
    .await
}

#[cfg(not(test))]
#[tauri::command]
/// Bumps `last_shown_at` so LRU eviction has a fresh signal.
pub(crate) async fn mark_og_images_shown(
    urls: Vec<String>,
    state: State<'_, SessionState>,
) -> Result<(), CommandError> {
    let state = state.inner().clone();
    run_blocking_command("mark_og_images_shown", move || {
        worker_bridge::mark_og_images_shown_impl(urls, &state)
    })
    .await
}

#[cfg(not(test))]
#[tauri::command]
/// Fetches og:image previews for the supplied URLs and persists each outcome.
pub(crate) async fn trigger_og_image_refetch(
    urls: Vec<String>,
    state: State<'_, SessionState>,
) -> Result<u32, CommandError> {
    let state = state.inner().clone();
    run_blocking_command("trigger_og_image_refetch", move || {
        worker_bridge::refetch_og_images_impl(urls, &state)
    })
    .await
}

#[cfg(not(test))]
#[tauri::command]
/// Reports the current og:image cache footprint to the Settings panel.
pub(crate) async fn get_og_image_storage_stats(
    state: State<'_, SessionState>,
) -> Result<vault_core::OgImageStorageStats, CommandError> {
    let state = state.inner().clone();
    run_blocking_command("get_og_image_storage_stats", move || {
        worker_bridge::og_image_storage_stats_impl(&state)
    })
    .await
}

#[cfg(not(test))]
#[tauri::command]
/// Reports og:image coverage (share of web pages with a preview image), off the UI thread.
pub(crate) async fn get_og_image_coverage_stats(
    state: State<'_, SessionState>,
) -> Result<vault_core::OgImageCoverageStats, CommandError> {
    let state = state.inner().clone();
    run_blocking_command("get_og_image_coverage_stats", move || {
        worker_bridge::og_image_coverage_stats_impl(&state)
    })
    .await
}

#[cfg(not(test))]
#[tauri::command]
/// Empties both og:image cache tables (behind the Settings confirm dialog).
pub(crate) async fn clear_og_image_cache(
    state: State<'_, SessionState>,
) -> Result<vault_core::OgImageCleanupReport, CommandError> {
    let state = state.inner().clone();
    run_blocking_command("clear_og_image_cache", move || {
        worker_bridge::clear_og_image_cache_impl(&state)
    })
    .await
}

#[cfg(not(test))]
#[tauri::command]
/// Runs one eviction pass using the user's configured cleanup mode.
pub(crate) async fn run_og_image_cleanup(
    state: State<'_, SessionState>,
) -> Result<vault_core::OgImageCleanupReport, CommandError> {
    let state = state.inner().clone();
    run_blocking_command("run_og_image_cleanup", move || {
        worker_bridge::run_og_image_cleanup_impl(&state)
    })
    .await
}

#[cfg(not(test))]
#[tauri::command]
/// Loads the dashboard summary shown on the archive home surface, off the UI thread.
pub(crate) async fn load_dashboard_snapshot(
    state: State<'_, SessionState>,
) -> Result<vault_core::DashboardSnapshot, CommandError> {
    let key_state = state.inner().clone();
    run_blocking_command("load_dashboard_snapshot", move || {
        worker_bridge::dashboard_snapshot_impl(&key_state)
    })
    .await
}

#[cfg(not(test))]
#[tauri::command]
/// Loads per-source visit counts and date spans, off the UI thread.
pub(crate) async fn load_source_stats(
    state: State<'_, SessionState>,
) -> Result<Vec<vault_core::SourceStats>, CommandError> {
    let key_state = state.inner().clone();
    run_blocking_command("load_source_stats", move || worker_bridge::source_stats_impl(&key_state))
        .await
}

#[cfg(not(test))]
#[tauri::command]
/// Loads the History detail panel data for one URL, off the UI thread.
pub(crate) async fn get_url_detail(
    url: String,
    state: State<'_, SessionState>,
) -> Result<vault_core::UrlDetail, CommandError> {
    let key_state = state.inner().clone();
    run_blocking_command("get_url_detail", move || worker_bridge::url_detail_impl(&url, &key_state))
        .await
}

#[cfg(not(test))]
#[tauri::command]
/// Loads the full audit detail for one archived run, off the UI thread.
pub(crate) async fn load_audit_run_detail(
    run_id: i64,
    state: State<'_, SessionState>,
) -> Result<vault_core::AuditRunDetail, CommandError> {
    let key_state = state.inner().clone();
    run_blocking_command("load_audit_run_detail", move || {
        worker_bridge::audit_run_detail_impl(run_id, &key_state)
    })
    .await
}

#[cfg(not(test))]
#[tauri::command]
/// Exports a history query result to the requested artifact format, off the UI thread.
pub(crate) async fn export_history(
    request: vault_core::ExportRequest,
    state: State<'_, SessionState>,
) -> Result<vault_core::ExportResult, CommandError> {
    let key_state = state.inner().clone();
    run_blocking_command("export_history", move || {
        worker_bridge::export_history_impl(request, &key_state)
    })
    .await
}

#[cfg(not(test))]
#[tauri::command]
/// Polls export state without waiting for the blocking export command.
pub(crate) fn get_export_progress(export_id: String) -> Option<vault_core::ExportProgress> {
    worker_bridge::get_export_progress_impl(&export_id)
}

#[cfg(not(test))]
#[tauri::command]
/// Stops a running export before its durable publication barrier.
pub(crate) fn cancel_export(export_id: String) -> bool {
    worker_bridge::cancel_export_impl(&export_id)
}

#[cfg(not(test))]
#[tauri::command]
/// Runs the archive doctor read path without mutating canonical facts, off the UI thread.
pub(crate) async fn doctor_report(
    state: State<'_, SessionState>,
) -> Result<vault_core::HealthReport, CommandError> {
    let key_state = state.inner().clone();
    run_blocking_command("doctor_report", move || worker_bridge::doctor_report_impl(&key_state))
        .await
}

#[cfg(not(test))]
#[tauri::command]
/// Applies conservative archive repair steps for doctor-detected issues, off the UI thread.
pub(crate) async fn repair_health(
    state: State<'_, SessionState>,
) -> Result<vault_core::HealthRepairReport, CommandError> {
    let state = state.inner().clone();
    run_blocking_command("repair_health", move || worker_bridge::repair_health_impl(&state)).await
}

#[cfg(not(test))]
#[tauri::command]
/// Counts what `clear_derived_intelligence` would remove, off the UI thread, so the clear can
/// be previewed before the user confirms it.
pub(crate) async fn preview_clear_derived_intelligence(
    state: State<'_, SessionState>,
) -> Result<vault_core::ClearDerivedIntelligenceReport, CommandError> {
    let key_state = state.inner().clone();
    run_blocking_command("preview_clear_derived_intelligence", move || {
        worker_bridge::preview_clear_derived_intelligence_impl(&key_state)
    })
    .await
}

#[cfg(not(test))]
#[tauri::command]
/// Clears rebuildable intelligence state without touching canonical visits, off the UI thread.
pub(crate) async fn clear_derived_intelligence(
    state: State<'_, SessionState>,
) -> Result<vault_core::ClearDerivedIntelligenceReport, CommandError> {
    let key_state = state.inner().clone();
    run_blocking_command("clear_derived_intelligence", move || {
        worker_bridge::clear_derived_intelligence_impl(&key_state)
    })
    .await
}

#[cfg(not(test))]
#[tauri::command]
/// Reads another page of the audit ledger on the session's reader pool.
pub(crate) async fn load_recent_runs(
    cursor: Option<i64>,
    limit: Option<usize>,
    state: State<'_, SessionState>,
) -> Result<vault_core::RunPage, CommandError> {
    let state = state.inner().clone();
    run_blocking_command("load_recent_runs", move || {
        worker_bridge::load_recent_runs_impl(cursor, limit, &state)
    })
    .await
}

#[cfg(not(test))]
#[tauri::command]
/// Reads another page of import history on the session's reader pool.
pub(crate) async fn load_import_batches(
    cursor: Option<String>,
    limit: Option<usize>,
    state: State<'_, SessionState>,
) -> Result<vault_core::ImportBatchPage, CommandError> {
    let state = state.inner().clone();
    run_blocking_command("load_import_batches", move || {
        worker_bridge::load_import_batches_impl(cursor, limit, &state)
    })
    .await
}
