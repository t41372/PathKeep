//! Tauri commands for canonical archive flows.

#[cfg(not(test))]
use super::blocking::run_blocking_command;
#[cfg(not(test))]
use crate::{session::SessionState, worker_bridge};
#[cfg(not(test))]
use tauri::{AppHandle, Emitter, State};
#[cfg(not(test))]
use vault_worker::RekeyRequest;

#[cfg(not(test))]
#[tauri::command]
/// Initializes the archive and optionally seeds the first session key.
pub(crate) async fn initialize_archive(
    config: vault_core::AppConfig,
    database_key: Option<String>,
    state: State<'_, SessionState>,
) -> Result<vault_core::AppSnapshot, String> {
    let session = state.inner().clone();
    run_blocking_command("initialize_archive", move || {
        worker_bridge::initialize_archive_impl(config, database_key, &session)
    })
    .await
}

#[cfg(not(test))]
#[tauri::command]
/// Executes an archive rekey/mode switch and returns the refreshed app snapshot.
pub(crate) fn rekey_archive(
    request: RekeyRequest,
    state: State<'_, SessionState>,
) -> Result<vault_core::AppSnapshot, String> {
    worker_bridge::rekey_archive_impl(request, &state)
}

#[cfg(not(test))]
#[tauri::command]
/// Previews the archive rekey plan before any encryption-mode mutation happens.
pub(crate) fn preview_rekey_archive(
    request: RekeyRequest,
    state: State<'_, SessionState>,
) -> Result<vault_core::RekeyPreview, String> {
    worker_bridge::preview_rekey_archive_impl(request, &state)
}

#[cfg(not(test))]
#[tauri::command]
/// Previews a checkpoint restore without replaying it yet.
pub(crate) fn preview_snapshot_restore(
    request: vault_core::SnapshotRestoreRequest,
    state: State<'_, SessionState>,
) -> Result<vault_core::SnapshotRestorePreview, String> {
    worker_bridge::preview_snapshot_restore_impl(request, &state)
}

#[cfg(not(test))]
#[tauri::command]
/// Replays a checkpoint restore and records it in the archive ledger.
pub(crate) fn run_snapshot_restore(
    request: vault_core::SnapshotRestoreRequest,
    state: State<'_, SessionState>,
) -> Result<vault_core::BackupReport, String> {
    worker_bridge::run_snapshot_restore_impl(request, &state)
}

#[cfg(not(test))]
#[tauri::command]
/// Shows what retention pruning would delete or preserve.
pub(crate) fn preview_retention_prune(
    state: State<'_, SessionState>,
) -> Result<vault_core::RetentionPreview, String> {
    worker_bridge::preview_retention_prune_impl(&state)
}

#[cfg(not(test))]
#[tauri::command]
/// Executes retention pruning for the selected buckets.
pub(crate) fn run_retention_prune(
    request: vault_core::RetentionPruneRequest,
    state: State<'_, SessionState>,
) -> Result<vault_core::RetentionPruneResult, String> {
    worker_bridge::run_retention_prune_impl(request, &state)
}

#[cfg(not(test))]
#[tauri::command]
/// Starts a backup run and streams progress events back to the renderer.
pub(crate) async fn run_backup_now(
    app: AppHandle,
    due_only: bool,
    state: State<'_, SessionState>,
) -> Result<vault_core::BackupReport, String> {
    let session_database_key = state.get_key();
    run_blocking_command("run_backup_now", move || {
        worker_bridge::run_backup_now_impl(due_only, session_database_key.as_deref(), |event| {
            let _ = app.emit("pathkeep://backup-progress", &event);
        })
    })
    .await
}

#[cfg(not(test))]
#[tauri::command]
/// Queries visible history facts from the canonical archive.
pub(crate) async fn query_history(
    query: vault_core::HistoryQuery,
    state: State<'_, SessionState>,
) -> Result<vault_core::HistoryQueryResponse, String> {
    let session_database_key = state.get_key();
    run_blocking_command("query_history", move || {
        worker_bridge::query_history_impl(query, session_database_key.as_deref())
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
) -> Result<Vec<vault_core::HistoryFaviconLookupResult>, String> {
    let session_database_key = state.get_key();
    run_blocking_command("load_history_favicons", move || {
        worker_bridge::load_history_favicons_impl(entries, session_database_key.as_deref())
    })
    .await
}

#[cfg(not(test))]
#[tauri::command]
/// Loads cached og:image payloads for already-visible card-mode rows.
pub(crate) async fn load_history_og_images(
    entries: Vec<vault_core::HistoryOgImageLookupEntry>,
    state: State<'_, SessionState>,
) -> Result<Vec<vault_core::HistoryOgImageLookupResult>, String> {
    let session_database_key = state.get_key();
    run_blocking_command("load_history_og_images", move || {
        worker_bridge::load_history_og_images_impl(entries, session_database_key.as_deref())
    })
    .await
}

#[cfg(not(test))]
#[tauri::command]
/// Bumps `last_shown_at` so LRU eviction has a fresh signal.
pub(crate) async fn mark_og_images_shown(
    urls: Vec<String>,
    state: State<'_, SessionState>,
) -> Result<(), String> {
    let session_database_key = state.get_key();
    run_blocking_command("mark_og_images_shown", move || {
        worker_bridge::mark_og_images_shown_impl(urls, session_database_key.as_deref())
    })
    .await
}

#[cfg(not(test))]
#[tauri::command]
/// Fetches og:image previews for the supplied URLs and persists each outcome.
pub(crate) async fn trigger_og_image_refetch(
    urls: Vec<String>,
    state: State<'_, SessionState>,
) -> Result<u32, String> {
    let session_database_key = state.get_key();
    run_blocking_command("trigger_og_image_refetch", move || {
        worker_bridge::refetch_og_images_impl(urls, session_database_key.as_deref())
    })
    .await
}

#[cfg(not(test))]
#[tauri::command]
/// Sweeps `urls` minus their existing og:image rows and runs the
/// difference through the worker pool, capped at `budget`. Powers the
/// Settings → Link Previews → "Rebuild now" button so the user can warm
/// the cache for the whole archive without waiting for the daily
/// post-backup pass. `budget` lands behind the worker's hard cap so a
/// pathological request can't tie up the pool.
pub(crate) async fn prefetch_og_images(
    budget: u32,
    state: State<'_, SessionState>,
) -> Result<(u32, u32), String> {
    let session_database_key = state.get_key();
    run_blocking_command("prefetch_og_images", move || {
        worker_bridge::prefetch_og_images_impl(budget, session_database_key.as_deref())
    })
    .await
}

#[cfg(not(test))]
#[tauri::command]
/// Reports the current og:image cache footprint to the Settings panel.
pub(crate) async fn get_og_image_storage_stats(
    state: State<'_, SessionState>,
) -> Result<vault_core::OgImageStorageStats, String> {
    let session_database_key = state.get_key();
    run_blocking_command("get_og_image_storage_stats", move || {
        worker_bridge::og_image_storage_stats_impl(session_database_key.as_deref())
    })
    .await
}

#[cfg(not(test))]
#[tauri::command]
/// Empties both og:image cache tables (behind the Settings confirm dialog).
pub(crate) async fn clear_og_image_cache(
    state: State<'_, SessionState>,
) -> Result<vault_core::OgImageCleanupReport, String> {
    let session_database_key = state.get_key();
    run_blocking_command("clear_og_image_cache", move || {
        worker_bridge::clear_og_image_cache_impl(session_database_key.as_deref())
    })
    .await
}

#[cfg(not(test))]
#[tauri::command]
/// Runs one eviction pass using the user's configured cleanup mode.
pub(crate) async fn run_og_image_cleanup(
    state: State<'_, SessionState>,
) -> Result<vault_core::OgImageCleanupReport, String> {
    let session_database_key = state.get_key();
    run_blocking_command("run_og_image_cleanup", move || {
        worker_bridge::run_og_image_cleanup_impl(session_database_key.as_deref())
    })
    .await
}

#[cfg(not(test))]
#[tauri::command]
/// Loads the dashboard summary shown on the archive home surface.
pub(crate) fn load_dashboard_snapshot(
    state: State<'_, SessionState>,
) -> Result<vault_core::DashboardSnapshot, String> {
    worker_bridge::dashboard_snapshot_impl(state.get_key().as_deref())
}

#[cfg(not(test))]
#[tauri::command]
/// Aggregates one local-day Browse insights panel from the full archive
/// (sparkline, top domains, top URLs, search queries, activity tallies,
/// session stats). Replaces the previous scroll-coupled client-side
/// `aggregateDayInsights` — see feedback-2026-05-25 §3.1.
pub(crate) fn get_browse_day_insights(
    request: vault_core::BrowseDayInsightsRequest,
    state: State<'_, SessionState>,
) -> Result<vault_core::BrowseDayInsights, String> {
    worker_bridge::browse_day_insights_impl(state.get_key().as_deref(), request)
}

#[cfg(not(test))]
#[tauri::command]
/// Loads the full audit detail for one archived run.
pub(crate) fn load_audit_run_detail(
    run_id: i64,
    state: State<'_, SessionState>,
) -> Result<vault_core::AuditRunDetail, String> {
    worker_bridge::audit_run_detail_impl(run_id, state.get_key().as_deref())
}

#[cfg(not(test))]
#[tauri::command]
/// Exports a history query result to the requested artifact format.
pub(crate) fn export_history(
    request: vault_core::ExportRequest,
    state: State<'_, SessionState>,
) -> Result<vault_core::ExportResult, String> {
    worker_bridge::export_history_impl(request, state.get_key().as_deref())
}

#[cfg(not(test))]
#[tauri::command]
/// Runs the archive doctor read path without mutating canonical facts.
pub(crate) fn doctor_report(
    state: State<'_, SessionState>,
) -> Result<vault_core::HealthReport, String> {
    worker_bridge::doctor_report_impl(state.get_key().as_deref())
}

#[cfg(not(test))]
#[tauri::command]
/// Applies conservative archive repair steps for doctor-detected issues.
pub(crate) fn repair_health(
    state: State<'_, SessionState>,
) -> Result<vault_core::HealthRepairReport, String> {
    worker_bridge::repair_health_impl(state.get_key().as_deref())
}

#[cfg(not(test))]
#[tauri::command]
/// Clears rebuildable intelligence state without touching canonical visits.
pub(crate) fn clear_derived_intelligence(
    state: State<'_, SessionState>,
) -> Result<vault_core::ClearDerivedIntelligenceReport, String> {
    worker_bridge::clear_derived_intelligence_impl(state.get_key().as_deref())
}
