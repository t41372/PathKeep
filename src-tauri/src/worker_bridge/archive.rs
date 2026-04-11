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
/// Loads the dashboard snapshot for the current archive.
pub(crate) fn dashboard_snapshot_impl(
    session_database_key: Option<&str>,
) -> Result<vault_core::DashboardSnapshot, String> {
    worker_result(vault_worker::dashboard_snapshot(session_database_key))
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
