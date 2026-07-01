//! Worker-bridge helpers for archive, backup, export, and repair flows.

use crate::session::{SessionState, session_key, update_session_key};
use vault_core::{AppConfig, ExportRequest, HistoryQuery};
use vault_worker::RekeyRequest;

use super::worker_result;

/// Initializes the archive and synchronizes the session key with the chosen key.
pub(crate) fn initialize_archive_impl(
    config: AppConfig,
    database_key: Option<String>,
    state: &SessionState,
) -> Result<vault_core::AppSnapshot, String> {
    let snapshot =
        worker_result(vault_worker::initialize_archive_database(&config, database_key.as_deref()))?;
    update_session_key(state, database_key)?;
    Ok(snapshot)
}

/// Rekeys the archive and updates the cached session key to the new key.
pub(crate) fn rekey_archive_impl(
    request: RekeyRequest,
    state: &SessionState,
) -> Result<vault_core::AppSnapshot, String> {
    let old_key = session_key(state);
    let snapshot =
        worker_result(vault_worker::rekey_archive_database(old_key.as_deref(), &request))?;
    update_session_key(state, request.new_key)?;
    Ok(snapshot)
}

/// Self-heals a drifted encryption-at-rest state (e.g. an encrypted config left
/// over a plaintext source-evidence by an archive-only rekey) using the unlocked
/// session key. Safe no-op when already consistent.
pub(crate) fn reconcile_archive_encryption_impl(
    state: &SessionState,
) -> Result<vault_core::ReconcileReport, String> {
    worker_result(vault_worker::reconcile_archive_encryption(session_key(state).as_deref()))
}

/// Previews the impact of an archive rekey/mode switch.
pub(crate) fn preview_rekey_archive_impl(
    request: RekeyRequest,
    state: &SessionState,
) -> Result<vault_core::RekeyPreview, String> {
    worker_result(vault_worker::preview_rekey_archive(session_key(state).as_deref(), &request))
}

#[cfg_attr(test, allow(dead_code))]
/// Previews a checkpoint restore against the current archive state.
pub(crate) fn preview_snapshot_restore_impl(
    request: vault_core::SnapshotRestoreRequest,
    state: &SessionState,
) -> Result<vault_core::SnapshotRestorePreview, String> {
    worker_result(vault_worker::preview_snapshot_restore_plan(
        session_key(state).as_deref(),
        &request,
    ))
}

#[cfg_attr(test, allow(dead_code))]
/// Executes a checkpoint restore and returns the resulting backup-style report.
pub(crate) fn run_snapshot_restore_impl(
    request: vault_core::SnapshotRestoreRequest,
    state: &SessionState,
) -> Result<vault_core::BackupReport, String> {
    worker_result(vault_worker::run_snapshot_restore_plan(session_key(state).as_deref(), &request))
}

#[cfg_attr(test, allow(dead_code))]
/// Lists verified full-archive safety snapshots for the recovery GUI (keyless FS scan).
pub(crate) fn list_recovery_snapshots_impl() -> Result<Vec<vault_core::RecoverySnapshot>, String> {
    worker_result(vault_worker::list_recovery_snapshots())
}

#[cfg_attr(test, allow(dead_code))]
/// Runs the one-click full-archive restore from a verified safety snapshot.
///
/// Prefers an explicit user-entered archive key (the recovery/unlock escape hatch holds no
/// ambient session key), falling back to the session key when none is supplied. The key is
/// never logged or echoed back in the report.
pub(crate) fn run_full_archive_restore_impl(
    request: vault_core::SnapshotRestoreRequest,
    key: Option<String>,
    state: &SessionState,
) -> Result<vault_core::FullArchiveRestoreReport, String> {
    let effective_key = key.or_else(|| session_key(state));
    worker_result(vault_worker::run_full_archive_restore(effective_key.as_deref(), &request))
}

#[cfg_attr(test, allow(dead_code))]
/// Previews the current retention-prune plan.
pub(crate) fn preview_retention_prune_impl(
    state: &SessionState,
) -> Result<vault_core::RetentionPreview, String> {
    worker_result(vault_worker::preview_retention_plan(session_key(state).as_deref()))
}

#[cfg_attr(test, allow(dead_code))]
/// Executes the selected retention-prune plan.
pub(crate) fn run_retention_prune_impl(
    request: vault_core::RetentionPruneRequest,
    state: &SessionState,
) -> Result<vault_core::RetentionPruneResult, String> {
    worker_result(vault_worker::run_retention_plan(session_key(state).as_deref(), &request))
}

/// Runs a backup immediately and forwards progress to the UI callback.
pub(crate) fn run_backup_now_impl(
    due_only: bool,
    session_database_key: Option<&str>,
    report_progress: impl FnMut(vault_core::BackupProgressEvent),
) -> Result<vault_core::BackupReport, String> {
    worker_result(vault_worker::run_backup_now_with_progress(
        session_database_key,
        due_only,
        report_progress,
    ))
}

/// Executes one history query against visible archive facts.
pub(crate) fn query_history_impl(
    query: HistoryQuery,
    session_database_key: Option<&str>,
) -> Result<vault_core::HistoryQueryResponse, String> {
    worker_result(vault_worker::query_history(session_database_key, query))
}

#[cfg_attr(test, allow(dead_code))]
/// Loads favicon payloads for already-visible Explorer rows after the primary
/// history query has painted.
pub(crate) fn load_history_favicons_impl(
    entries: Vec<vault_core::HistoryFaviconLookupEntry>,
    session_database_key: Option<&str>,
) -> Result<Vec<vault_core::HistoryFaviconLookupResult>, String> {
    worker_result(vault_worker::load_history_favicons(session_database_key, entries))
}

#[cfg_attr(test, allow(dead_code))]
/// Loads cached og:image payloads for already-visible card-mode rows.
pub(crate) fn load_history_og_images_impl(
    entries: Vec<vault_core::HistoryOgImageLookupEntry>,
    session_database_key: Option<&str>,
) -> Result<Vec<vault_core::HistoryOgImageLookupResult>, String> {
    worker_result(vault_worker::load_history_og_images(session_database_key, entries))
}

#[cfg_attr(test, allow(dead_code))]
/// Bumps `last_shown_at` for the supplied page URLs (LRU eviction signal).
pub(crate) fn mark_og_images_shown_impl(
    urls: Vec<String>,
    session_database_key: Option<&str>,
) -> Result<(), String> {
    worker_result(vault_worker::mark_og_images_shown(session_database_key, urls))
}

#[cfg_attr(test, allow(dead_code))]
/// Fetches og:image previews for the given URLs and persists each outcome.
///
/// This is the IPC entrypoint hit by the implicit on-demand path (the
/// `useExplorerOgImages` hook). It must short-circuit when the user has
/// chosen `OgImageFetchMode::Off` so the data-sovereignty promise on the
/// Settings → Link previews copy ("Off — No fetching anywhere.") holds
/// regardless of what the frontend does. The explicit "Rebuild now"
/// affordance uses `prefetch_og_images_impl` instead, which intentionally
/// ignores `fetch_mode` because clicking Rebuild *is* the explicit
/// override.
pub(crate) fn refetch_og_images_impl(
    urls: Vec<String>,
    session_database_key: Option<&str>,
) -> Result<u32, String> {
    match vault_worker::effective_og_image_fetch_mode() {
        Ok(vault_core::OgImageFetchMode::Off) => return Ok(0),
        Ok(_) => {}
        Err(error) => return Err(error.to_string()),
    }
    worker_result(vault_worker::refetch_og_images(session_database_key, urls))
}

#[cfg_attr(test, allow(dead_code))]
/// Powers the "Rebuild now" affordance in Settings → Link Previews by
/// enqueuing visited URLs that have no `og_images` row yet, capped at
/// `budget`. Returns the `(enqueued, succeeded)` pair so the UI can
/// report progress honestly.
pub(crate) fn prefetch_og_images_impl(
    budget: u32,
    session_database_key: Option<&str>,
) -> Result<(u32, u32), String> {
    worker_result(vault_worker::prefetch_og_images_on_demand(session_database_key, budget))
}

#[cfg_attr(test, allow(dead_code))]
/// Reports the current og:image cache footprint to Settings → Storage.
pub(crate) fn og_image_storage_stats_impl(
    session_database_key: Option<&str>,
) -> Result<vault_core::OgImageStorageStats, String> {
    worker_result(vault_worker::og_image_storage_stats(session_database_key))
}

#[cfg_attr(test, allow(dead_code))]
/// Reports og:image coverage (share of web pages with a preview) to Settings.
pub(crate) fn og_image_coverage_stats_impl(
    session_database_key: Option<&str>,
) -> Result<vault_core::OgImageCoverageStats, String> {
    worker_result(vault_worker::og_image_coverage_stats(session_database_key))
}

#[cfg_attr(test, allow(dead_code))]
/// Empties both og:image cache tables (behind the Settings confirm dialog).
pub(crate) fn clear_og_image_cache_impl(
    session_database_key: Option<&str>,
) -> Result<vault_core::OgImageCleanupReport, String> {
    worker_result(vault_worker::clear_og_image_cache(session_database_key))
}

#[cfg_attr(test, allow(dead_code))]
/// Runs one eviction pass using the user's configured cleanup mode.
pub(crate) fn run_og_image_cleanup_impl(
    session_database_key: Option<&str>,
) -> Result<vault_core::OgImageCleanupReport, String> {
    worker_result(vault_worker::run_og_image_cleanup(session_database_key))
}

#[cfg_attr(test, allow(dead_code))]
/// Loads the dashboard snapshot for the current archive.
pub(crate) fn dashboard_snapshot_impl(
    session_database_key: Option<&str>,
) -> Result<vault_core::DashboardSnapshot, String> {
    worker_result(vault_worker::dashboard_snapshot(session_database_key))
}

#[cfg_attr(test, allow(dead_code))]
/// Aggregates one local-day Browse insights panel from the full archive.
pub(crate) fn browse_day_insights_impl(
    session_database_key: Option<&str>,
    request: vault_core::BrowseDayInsightsRequest,
) -> Result<vault_core::BrowseDayInsights, String> {
    worker_result(vault_worker::browse_day_insights(session_database_key, request))
}

#[cfg_attr(test, allow(dead_code))]
/// Loads the full audit detail for one run ID.
pub(crate) fn audit_run_detail_impl(
    run_id: i64,
    session_database_key: Option<&str>,
) -> Result<vault_core::AuditRunDetail, String> {
    worker_result(vault_worker::audit_run_detail(session_database_key, run_id))
}

/// Exports a query result to the requested file format.
pub(crate) fn export_history_impl(
    request: ExportRequest,
    session_database_key: Option<&str>,
) -> Result<vault_core::ExportResult, String> {
    worker_result(vault_worker::export_query(session_database_key, request))
}

/// Runs the archive doctor read path through the worker.
pub(crate) fn doctor_report_impl(
    session_database_key: Option<&str>,
) -> Result<vault_core::HealthReport, String> {
    worker_result(vault_worker::doctor_report(session_database_key))
}

#[cfg_attr(test, allow(dead_code))]
/// Applies conservative repair steps for doctor-detected health issues.
pub(crate) fn repair_health_impl(
    session_database_key: Option<&str>,
) -> Result<vault_core::HealthRepairReport, String> {
    worker_result(vault_worker::repair_health(session_database_key))
}
