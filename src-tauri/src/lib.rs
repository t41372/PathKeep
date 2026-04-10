mod file_manager;
mod session;
mod worker_bridge;

use anyhow::Result;
#[cfg(not(test))]
use session::SessionState;
use std::io::Write;
#[cfg(not(test))]
use tauri::{AppHandle, Emitter, State};
#[cfg(not(test))]
use tauri_plugin_autostart::MacosLauncher;
#[cfg(not(test))]
use vault_core::{
    AiAssistantRequest, AiIndexRequest, AiProviderConnectionTestRequest, AiProviderSecretInput,
    AiSearchRequest, AppConfig, ExplainInsightRequest, ExportRequest, HistoryQuery,
    RetentionPruneRequest, RunInsightsRequest, S3CredentialInput, SchedulePlan, ScheduleStatus,
    SecurityStatus, SetAppLockPasscodeRequest, SnapshotRestoreRequest, TakeoutRequest,
    UnlockAppSessionRequest,
};
#[cfg(not(test))]
use vault_worker::RekeyRequest;

const PRODUCT_DISPLAY_NAME: &str = "PathKeep";

pub fn entrypoint() -> Result<()> {
    let arguments = std::env::args().collect::<Vec<_>>();
    let stdout = std::io::stdout();
    let mut handle = stdout.lock();
    write_payload(&mut handle, run_with_arguments(&arguments)?)
}

fn run_with_arguments(arguments: &[String]) -> Result<Option<String>> {
    if arguments.get(1).map(String::as_str) == Some("--worker") {
        return vault_worker::run_worker_cli(&arguments[2..]).map(Some);
    }
    run_app()?;
    Ok(None)
}

fn write_payload<W: Write>(writer: &mut W, payload: Option<String>) -> Result<()> {
    if let Some(payload) = payload {
        writeln!(writer, "{payload}")?;
    }
    Ok(())
}

#[cfg(not(test))]
fn run_app() -> Result<()> {
    tauri::Builder::default()
        .plugin(tauri_plugin_autostart::init(MacosLauncher::LaunchAgent, Some(vec!["--windowed"])))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_process::init())
        .setup(|app| {
            let paths = vault_core::project_paths().map_err(tauri::Error::Anyhow)?;
            let mut config = vault_core::load_config(&paths).map_err(tauri::Error::Anyhow)?;
            vault_core::hydrate_app_lock_config(&paths, &mut config)
                .map_err(tauri::Error::Anyhow)?;
            vault_core::initialize_app_lock_session(&paths, &config)
                .map_err(tauri::Error::Anyhow)?;
            app.handle().plugin(
                tauri_plugin_stronghold::Builder::with_argon2(&paths.stronghold_salt_path).build(),
            )?;
            app.handle().plugin(tauri_plugin_updater::Builder::new().build())?;
            Ok(())
        })
        .manage(SessionState::default())
        .invoke_handler(tauri::generate_handler![
            app_build_info,
            app_snapshot,
            app_lock_status,
            save_config,
            initialize_archive,
            preview_rekey_archive,
            rekey_archive,
            preview_snapshot_restore,
            run_snapshot_restore,
            preview_retention_prune,
            run_retention_prune,
            set_session_database_key,
            clear_session_database_key,
            set_app_lock_passcode,
            clear_app_lock_passcode,
            lock_app_session,
            unlock_app_session,
            run_backup_now,
            query_history,
            load_dashboard_snapshot,
            load_audit_run_detail,
            export_history,
            preview_remote_backup,
            run_remote_backup,
            verify_remote_backup,
            inspect_takeout,
            import_takeout,
            preview_import_batch,
            revert_import_batch,
            restore_import_batch,
            preview_schedule,
            apply_schedule,
            remove_schedule,
            schedule_status,
            doctor_report,
            repair_health,
            clear_derived_intelligence,
            keyring_status,
            security_status,
            keyring_get_database_key,
            keyring_store_database_key,
            keyring_clear_database_key,
            store_s3_credentials,
            clear_s3_credentials,
            store_ai_provider_api_key,
            clear_ai_provider_api_key,
            test_ai_provider_connection,
            load_ai_queue_status,
            run_ai_queue_jobs,
            replay_ai_job,
            cancel_ai_job,
            load_ai_assistant_job,
            build_ai_index,
            search_ai_history,
            ask_ai_assistant,
            run_insights_now,
            load_insights,
            load_thread_detail,
            explain_insight,
            preview_ai_integrations,
            reset_local_secret_vault,
            open_path_in_file_manager,
            open_external_url
        ])
        .run(tauri::generate_context!())?;
    Ok(())
}

#[cfg(test)]
fn run_app() -> Result<()> {
    Ok(())
}

#[cfg(not(test))]
#[tauri::command]
fn app_build_info() -> vault_core::AppBuildInfo {
    worker_bridge::app_build_info_impl()
}

#[cfg(not(test))]
#[tauri::command]
fn app_snapshot(state: State<'_, SessionState>) -> Result<vault_core::AppSnapshot, String> {
    worker_bridge::app_snapshot_impl(state.get_key().as_deref())
}

#[cfg(not(test))]
#[tauri::command]
fn save_config(
    config: AppConfig,
    state: State<'_, SessionState>,
) -> Result<vault_core::AppSnapshot, String> {
    worker_bridge::save_config_impl(config, state.get_key().as_deref())
}

#[cfg(not(test))]
#[tauri::command]
fn initialize_archive(
    config: AppConfig,
    database_key: Option<String>,
    state: State<'_, SessionState>,
) -> Result<vault_core::AppSnapshot, String> {
    worker_bridge::initialize_archive_impl(config, database_key, &state)
}

#[cfg(not(test))]
#[tauri::command]
fn rekey_archive(
    request: RekeyRequest,
    state: State<'_, SessionState>,
) -> Result<vault_core::AppSnapshot, String> {
    worker_bridge::rekey_archive_impl(request, &state)
}

#[cfg(not(test))]
#[tauri::command]
fn preview_rekey_archive(
    request: RekeyRequest,
    state: State<'_, SessionState>,
) -> Result<vault_core::RekeyPreview, String> {
    worker_bridge::preview_rekey_archive_impl(request, &state)
}

#[cfg(not(test))]
#[tauri::command]
fn preview_snapshot_restore(
    request: SnapshotRestoreRequest,
    state: State<'_, SessionState>,
) -> Result<vault_core::SnapshotRestorePreview, String> {
    worker_bridge::preview_snapshot_restore_impl(request, &state)
}

#[cfg(not(test))]
#[tauri::command]
fn run_snapshot_restore(
    request: SnapshotRestoreRequest,
    state: State<'_, SessionState>,
) -> Result<vault_core::BackupReport, String> {
    worker_bridge::run_snapshot_restore_impl(request, &state)
}

#[cfg(not(test))]
#[tauri::command]
fn preview_retention_prune(
    state: State<'_, SessionState>,
) -> Result<vault_core::RetentionPreview, String> {
    worker_bridge::preview_retention_prune_impl(&state)
}

#[cfg(not(test))]
#[tauri::command]
fn run_retention_prune(
    request: RetentionPruneRequest,
    state: State<'_, SessionState>,
) -> Result<vault_core::RetentionPruneResult, String> {
    worker_bridge::run_retention_prune_impl(request, &state)
}

#[cfg(not(test))]
#[tauri::command]
fn set_session_database_key(
    database_key: String,
    state: State<'_, SessionState>,
) -> Result<(), String> {
    worker_bridge::set_session_database_key_impl(database_key, &state)
}

#[cfg(not(test))]
#[tauri::command]
fn clear_session_database_key(state: State<'_, SessionState>) -> Result<(), String> {
    worker_bridge::clear_session_database_key_impl(&state)
}

#[cfg(not(test))]
#[tauri::command]
fn run_backup_now(
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
fn query_history(
    query: HistoryQuery,
    state: State<'_, SessionState>,
) -> Result<vault_core::HistoryQueryResponse, String> {
    worker_bridge::query_history_impl(query, state.get_key().as_deref())
}

#[cfg(not(test))]
#[tauri::command]
fn load_dashboard_snapshot(
    state: State<'_, SessionState>,
) -> Result<vault_core::DashboardSnapshot, String> {
    worker_bridge::dashboard_snapshot_impl(state.get_key().as_deref())
}

#[cfg(not(test))]
#[tauri::command]
fn load_audit_run_detail(
    run_id: i64,
    state: State<'_, SessionState>,
) -> Result<vault_core::AuditRunDetail, String> {
    worker_bridge::audit_run_detail_impl(run_id, state.get_key().as_deref())
}

#[cfg(not(test))]
#[tauri::command]
fn export_history(
    request: ExportRequest,
    state: State<'_, SessionState>,
) -> Result<vault_core::ExportResult, String> {
    worker_bridge::export_history_impl(request, state.get_key().as_deref())
}

#[cfg(not(test))]
#[tauri::command]
fn preview_remote_backup() -> Result<vault_core::RemoteBackupPreview, String> {
    worker_bridge::preview_remote_backup_impl()
}

#[cfg(not(test))]
#[tauri::command]
fn run_remote_backup(
    state: State<'_, SessionState>,
) -> Result<vault_core::RemoteBackupResult, String> {
    worker_bridge::run_remote_backup_impl(state.get_key().as_deref())
}

#[cfg(not(test))]
#[tauri::command]
fn verify_remote_backup(
    bundle_path: String,
    state: State<'_, SessionState>,
) -> Result<vault_core::RemoteBackupVerification, String> {
    worker_bridge::verify_remote_backup_impl(bundle_path, state.get_key().as_deref())
}

#[cfg(not(test))]
#[tauri::command]
fn inspect_takeout(request: TakeoutRequest) -> Result<vault_core::TakeoutInspection, String> {
    worker_bridge::inspect_takeout_impl(request)
}

#[cfg(not(test))]
#[tauri::command]
fn import_takeout(
    request: TakeoutRequest,
    state: State<'_, SessionState>,
) -> Result<vault_core::TakeoutInspection, String> {
    worker_bridge::import_takeout_impl(request, state.get_key().as_deref())
}

#[cfg(not(test))]
#[tauri::command]
fn preview_import_batch(
    batch_id: i64,
    state: State<'_, SessionState>,
) -> Result<vault_core::ImportBatchDetail, String> {
    worker_bridge::preview_import_batch_impl(batch_id, state.get_key().as_deref())
}

#[cfg(not(test))]
#[tauri::command]
fn revert_import_batch(
    batch_id: i64,
    state: State<'_, SessionState>,
) -> Result<vault_core::ImportBatchDetail, String> {
    worker_bridge::revert_import_batch_impl(batch_id, state.get_key().as_deref())
}

#[cfg(not(test))]
#[tauri::command]
fn restore_import_batch(
    batch_id: i64,
    state: State<'_, SessionState>,
) -> Result<vault_core::ImportBatchDetail, String> {
    worker_bridge::restore_import_batch_impl(batch_id, state.get_key().as_deref())
}

#[cfg(not(test))]
#[tauri::command]
fn preview_schedule(platform: Option<String>) -> Result<SchedulePlan, String> {
    worker_bridge::preview_schedule_impl(platform)
}

#[cfg(not(test))]
#[tauri::command]
fn apply_schedule(plan: SchedulePlan) -> Result<vault_core::ApplyResult, String> {
    worker_bridge::apply_schedule_impl(plan)
}

#[cfg(not(test))]
#[tauri::command]
fn remove_schedule(plan: SchedulePlan) -> Result<vault_core::ApplyResult, String> {
    worker_bridge::remove_schedule_impl(plan)
}

#[cfg(not(test))]
#[tauri::command]
fn schedule_status(
    platform: Option<String>,
    state: State<'_, SessionState>,
) -> Result<ScheduleStatus, String> {
    worker_bridge::schedule_status_impl(platform, state.get_key().as_deref())
}

#[cfg(not(test))]
#[tauri::command]
fn doctor_report(state: State<'_, SessionState>) -> Result<vault_core::HealthReport, String> {
    worker_bridge::doctor_report_impl(state.get_key().as_deref())
}

#[cfg(not(test))]
#[tauri::command]
fn repair_health(state: State<'_, SessionState>) -> Result<vault_core::HealthRepairReport, String> {
    worker_bridge::repair_health_impl(state.get_key().as_deref())
}

#[cfg(not(test))]
#[tauri::command]
fn clear_derived_intelligence(
    state: State<'_, SessionState>,
) -> Result<vault_core::ClearDerivedIntelligenceReport, String> {
    worker_bridge::clear_derived_intelligence_impl(state.get_key().as_deref())
}

#[cfg(not(test))]
#[tauri::command]
fn keyring_status() -> vault_core::KeyringStatusReport {
    worker_bridge::keyring_status_impl()
}

#[cfg(not(test))]
#[tauri::command]
fn security_status(state: State<'_, SessionState>) -> Result<SecurityStatus, String> {
    worker_bridge::security_status_impl(state.get_key().as_deref())
}

#[cfg(not(test))]
#[tauri::command]
fn app_lock_status() -> Result<vault_core::AppLockStatus, String> {
    worker_bridge::app_lock_status_impl()
}

#[cfg(not(test))]
#[tauri::command]
fn set_app_lock_passcode(
    request: SetAppLockPasscodeRequest,
) -> Result<vault_core::AppLockStatus, String> {
    worker_bridge::set_app_lock_passcode_impl(request)
}

#[cfg(not(test))]
#[tauri::command]
fn clear_app_lock_passcode() -> Result<vault_core::AppLockStatus, String> {
    worker_bridge::clear_app_lock_passcode_impl()
}

#[cfg(not(test))]
#[tauri::command]
fn lock_app_session(reason: Option<String>) -> Result<vault_core::AppLockStatus, String> {
    worker_bridge::lock_app_session_impl(reason)
}

#[cfg(not(test))]
#[tauri::command]
fn unlock_app_session(
    request: UnlockAppSessionRequest,
) -> Result<vault_core::AppLockStatus, String> {
    worker_bridge::unlock_app_session_impl(request)
}

#[cfg(not(test))]
#[tauri::command]
fn keyring_get_database_key() -> Result<Option<String>, String> {
    worker_bridge::keyring_get_database_key_impl()
}

#[cfg(not(test))]
#[tauri::command]
fn keyring_store_database_key(value: String) -> Result<vault_core::KeyringStatusReport, String> {
    worker_bridge::keyring_store_database_key_impl(value)
}

#[cfg(not(test))]
#[tauri::command]
fn keyring_clear_database_key() -> Result<vault_core::KeyringStatusReport, String> {
    worker_bridge::keyring_clear_database_key_impl()
}

#[cfg(not(test))]
#[tauri::command]
fn store_s3_credentials(credentials: S3CredentialInput) -> Result<(), String> {
    worker_bridge::store_s3_credentials_impl(credentials)
}

#[cfg(not(test))]
#[tauri::command]
fn clear_s3_credentials() -> Result<(), String> {
    worker_bridge::clear_s3_credentials_impl()
}

#[cfg(not(test))]
#[tauri::command]
fn store_ai_provider_api_key(
    input: AiProviderSecretInput,
    state: State<'_, SessionState>,
) -> Result<vault_core::AppSnapshot, String> {
    worker_bridge::store_ai_provider_api_key_impl(input, state.get_key().as_deref())
}

#[cfg(not(test))]
#[tauri::command]
fn clear_ai_provider_api_key(
    provider_id: String,
    state: State<'_, SessionState>,
) -> Result<vault_core::AppSnapshot, String> {
    worker_bridge::clear_ai_provider_api_key_impl(provider_id, state.get_key().as_deref())
}

#[cfg(not(test))]
#[tauri::command]
fn test_ai_provider_connection(
    request: AiProviderConnectionTestRequest,
    state: State<'_, SessionState>,
) -> Result<vault_core::AiProviderConnectionTestReport, String> {
    worker_bridge::test_ai_provider_connection_impl(request, state.get_key().as_deref())
}

#[cfg(not(test))]
#[tauri::command]
fn load_ai_queue_status(
    state: State<'_, SessionState>,
) -> Result<vault_core::AiQueueStatus, String> {
    worker_bridge::load_ai_queue_status_impl(state.get_key().as_deref())
}

#[cfg(not(test))]
#[tauri::command]
fn run_ai_queue_jobs(
    max_jobs: Option<u32>,
    state: State<'_, SessionState>,
) -> Result<vault_core::AiQueueStatus, String> {
    worker_bridge::run_ai_queue_jobs_impl(max_jobs, state.get_key().as_deref())
}

#[cfg(not(test))]
#[tauri::command]
fn replay_ai_job(
    job_id: i64,
    state: State<'_, SessionState>,
) -> Result<vault_core::AiQueueJob, String> {
    worker_bridge::replay_ai_job_impl(job_id, state.get_key().as_deref())
}

#[cfg(not(test))]
#[tauri::command]
fn cancel_ai_job(
    job_id: i64,
    state: State<'_, SessionState>,
) -> Result<vault_core::AiQueueJob, String> {
    worker_bridge::cancel_ai_job_impl(job_id, state.get_key().as_deref())
}

#[cfg(not(test))]
#[tauri::command]
fn load_ai_assistant_job(
    job_id: i64,
    state: State<'_, SessionState>,
) -> Result<vault_core::AiAssistantResponse, String> {
    worker_bridge::load_ai_assistant_job_impl(job_id, state.get_key().as_deref())
}

#[cfg(not(test))]
#[tauri::command]
fn build_ai_index(
    request: AiIndexRequest,
    state: State<'_, SessionState>,
) -> Result<vault_core::AiIndexReport, String> {
    worker_bridge::build_ai_index_impl(request, state.get_key().as_deref())
}

#[cfg(not(test))]
#[tauri::command]
fn search_ai_history(
    request: AiSearchRequest,
    state: State<'_, SessionState>,
) -> Result<vault_core::AiSearchResponse, String> {
    worker_bridge::search_ai_history_impl(request, state.get_key().as_deref())
}

#[cfg(not(test))]
#[tauri::command]
fn ask_ai_assistant(
    request: AiAssistantRequest,
    state: State<'_, SessionState>,
) -> Result<vault_core::AiAssistantResponse, String> {
    worker_bridge::ask_ai_assistant_impl(request, state.get_key().as_deref())
}

#[cfg(not(test))]
#[tauri::command]
fn run_insights_now(
    request: RunInsightsRequest,
    state: State<'_, SessionState>,
) -> Result<vault_core::RunInsightsReport, String> {
    worker_bridge::run_insights_now_impl(request, state.get_key().as_deref())
}

#[cfg(not(test))]
#[tauri::command]
fn load_insights(
    request: RunInsightsRequest,
    state: State<'_, SessionState>,
) -> Result<vault_core::InsightSnapshot, String> {
    worker_bridge::load_insights_impl(request, state.get_key().as_deref())
}

#[cfg(not(test))]
#[tauri::command]
fn load_thread_detail(
    thread_id: String,
    state: State<'_, SessionState>,
) -> Result<vault_core::InsightThreadDetail, String> {
    worker_bridge::load_thread_detail_impl(thread_id, state.get_key().as_deref())
}

#[cfg(not(test))]
#[tauri::command]
fn explain_insight(
    request: ExplainInsightRequest,
    state: State<'_, SessionState>,
) -> Result<vault_core::InsightExplanation, String> {
    worker_bridge::explain_insight_impl(request, state.get_key().as_deref())
}

#[cfg(not(test))]
#[tauri::command]
fn preview_ai_integrations() -> Result<vault_core::AiIntegrationPreview, String> {
    worker_bridge::preview_ai_integrations_impl()
}

#[cfg(not(test))]
#[tauri::command]
fn reset_local_secret_vault() -> Result<(), String> {
    worker_bridge::reset_local_secret_vault_impl()
}

#[cfg(not(test))]
#[tauri::command]
fn open_path_in_file_manager(path: String) -> Result<String, String> {
    file_manager::open_path_in_file_manager_impl(path)
}

#[cfg(not(test))]
#[tauri::command]
fn open_external_url(url: String) -> Result<String, String> {
    file_manager::open_external_url_impl(url)
}
