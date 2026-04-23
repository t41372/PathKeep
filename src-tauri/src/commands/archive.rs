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
/// Loads the dashboard summary shown on the archive home surface.
pub(crate) fn load_dashboard_snapshot(
    state: State<'_, SessionState>,
) -> Result<vault_core::DashboardSnapshot, String> {
    worker_bridge::dashboard_snapshot_impl(state.get_key().as_deref())
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
