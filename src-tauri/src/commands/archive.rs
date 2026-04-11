use crate::{session::SessionState, worker_bridge};
use tauri::{AppHandle, Emitter, State};
use vault_worker::RekeyRequest;

#[cfg(not(test))]
#[tauri::command]
pub(crate) fn initialize_archive(
    config: vault_core::AppConfig,
    database_key: Option<String>,
    state: State<'_, SessionState>,
) -> Result<vault_core::AppSnapshot, String> {
    worker_bridge::initialize_archive_impl(config, database_key, &state)
}

#[cfg(not(test))]
#[tauri::command]
pub(crate) fn rekey_archive(
    request: RekeyRequest,
    state: State<'_, SessionState>,
) -> Result<vault_core::AppSnapshot, String> {
    worker_bridge::rekey_archive_impl(request, &state)
}

#[cfg(not(test))]
#[tauri::command]
pub(crate) fn preview_rekey_archive(
    request: RekeyRequest,
    state: State<'_, SessionState>,
) -> Result<vault_core::RekeyPreview, String> {
    worker_bridge::preview_rekey_archive_impl(request, &state)
}

#[cfg(not(test))]
#[tauri::command]
pub(crate) fn preview_snapshot_restore(
    request: vault_core::SnapshotRestoreRequest,
    state: State<'_, SessionState>,
) -> Result<vault_core::SnapshotRestorePreview, String> {
    worker_bridge::preview_snapshot_restore_impl(request, &state)
}

#[cfg(not(test))]
#[tauri::command]
pub(crate) fn run_snapshot_restore(
    request: vault_core::SnapshotRestoreRequest,
    state: State<'_, SessionState>,
) -> Result<vault_core::BackupReport, String> {
    worker_bridge::run_snapshot_restore_impl(request, &state)
}

#[cfg(not(test))]
#[tauri::command]
pub(crate) fn preview_retention_prune(
    state: State<'_, SessionState>,
) -> Result<vault_core::RetentionPreview, String> {
    worker_bridge::preview_retention_prune_impl(&state)
}

#[cfg(not(test))]
#[tauri::command]
pub(crate) fn run_retention_prune(
    request: vault_core::RetentionPruneRequest,
    state: State<'_, SessionState>,
) -> Result<vault_core::RetentionPruneResult, String> {
    worker_bridge::run_retention_prune_impl(request, &state)
}

#[cfg(not(test))]
#[tauri::command]
pub(crate) fn run_backup_now(
    app: AppHandle,
    due_only: bool,
    state: State<'_, SessionState>,
) -> Result<vault_core::BackupReport, String> {
    worker_bridge::run_backup_now_impl(due_only, state.get_key().as_deref(), |event| {
        let _ = app.emit("pathkeep://backup-progress", &event);
    })
}

#[cfg(not(test))]
#[tauri::command]
pub(crate) fn query_history(
    query: vault_core::HistoryQuery,
    state: State<'_, SessionState>,
) -> Result<vault_core::HistoryQueryResponse, String> {
    worker_bridge::query_history_impl(query, state.get_key().as_deref())
}

#[cfg(not(test))]
#[tauri::command]
pub(crate) fn load_dashboard_snapshot(
    state: State<'_, SessionState>,
) -> Result<vault_core::DashboardSnapshot, String> {
    worker_bridge::dashboard_snapshot_impl(state.get_key().as_deref())
}

#[cfg(not(test))]
#[tauri::command]
pub(crate) fn load_audit_run_detail(
    run_id: i64,
    state: State<'_, SessionState>,
) -> Result<vault_core::AuditRunDetail, String> {
    worker_bridge::audit_run_detail_impl(run_id, state.get_key().as_deref())
}

#[cfg(not(test))]
#[tauri::command]
pub(crate) fn export_history(
    request: vault_core::ExportRequest,
    state: State<'_, SessionState>,
) -> Result<vault_core::ExportResult, String> {
    worker_bridge::export_history_impl(request, state.get_key().as_deref())
}

#[cfg(not(test))]
#[tauri::command]
pub(crate) fn doctor_report(
    state: State<'_, SessionState>,
) -> Result<vault_core::HealthReport, String> {
    worker_bridge::doctor_report_impl(state.get_key().as_deref())
}

#[cfg(not(test))]
#[tauri::command]
pub(crate) fn repair_health(
    state: State<'_, SessionState>,
) -> Result<vault_core::HealthRepairReport, String> {
    worker_bridge::repair_health_impl(state.get_key().as_deref())
}

#[cfg(not(test))]
#[tauri::command]
pub(crate) fn clear_derived_intelligence(
    state: State<'_, SessionState>,
) -> Result<vault_core::ClearDerivedIntelligenceReport, String> {
    worker_bridge::clear_derived_intelligence_impl(state.get_key().as_deref())
}
