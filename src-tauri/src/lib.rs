mod file_manager;
mod session;
mod worker_bridge;

use anyhow::Result;
#[cfg(not(test))]
use session::SessionState;
use std::io::Write;
#[cfg(not(test))]
use tauri::State;
#[cfg(not(test))]
use tauri_plugin_autostart::MacosLauncher;
#[cfg(not(test))]
use vault_core::{
    AiAssistantRequest, AiIndexRequest, AiProviderSecretInput, AiSearchRequest, AppConfig,
    ExplainInsightRequest, ExportRequest, HistoryQuery, RunInsightsRequest, S3CredentialInput,
    SchedulePlan, TakeoutRequest,
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
        .setup(|app| {
            let salt_path =
                vault_core::project_paths().map_err(tauri::Error::Anyhow)?.stronghold_salt_path;
            app.handle()
                .plugin(tauri_plugin_stronghold::Builder::with_argon2(&salt_path).build())?;
            Ok(())
        })
        .manage(SessionState::default())
        .invoke_handler(tauri::generate_handler![
            app_build_info,
            app_snapshot,
            save_config,
            initialize_archive,
            rekey_archive,
            set_session_database_key,
            clear_session_database_key,
            run_backup_now,
            query_history,
            load_dashboard_snapshot,
            load_audit_run_detail,
            export_history,
            preview_remote_backup,
            run_remote_backup,
            inspect_takeout,
            import_takeout,
            preview_import_batch,
            revert_import_batch,
            preview_schedule,
            apply_schedule,
            doctor_report,
            keyring_status,
            keyring_get_database_key,
            keyring_store_database_key,
            keyring_clear_database_key,
            store_s3_credentials,
            clear_s3_credentials,
            store_ai_provider_api_key,
            clear_ai_provider_api_key,
            build_ai_index,
            search_ai_history,
            ask_ai_assistant,
            run_insights_now,
            load_insights,
            load_thread_detail,
            explain_insight,
            preview_ai_integrations,
            reset_local_secret_vault,
            open_path_in_file_manager
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
    due_only: bool,
    state: State<'_, SessionState>,
) -> Result<vault_core::BackupReport, String> {
    worker_bridge::run_backup_now_impl(due_only, state.get_key().as_deref())
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
fn doctor_report(state: State<'_, SessionState>) -> Result<vault_core::HealthReport, String> {
    worker_bridge::doctor_report_impl(state.get_key().as_deref())
}

#[cfg(not(test))]
#[tauri::command]
fn keyring_status() -> vault_core::KeyringStatusReport {
    worker_bridge::keyring_status_impl()
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
