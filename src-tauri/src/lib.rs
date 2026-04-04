use anyhow::Result;
use std::sync::Mutex;
use tauri::State;
use tauri_plugin_autostart::MacosLauncher;
use vault_core::{
    AppConfig, ExportRequest, HistoryQuery, S3CredentialInput, SchedulePlan, TakeoutRequest,
};
use vault_worker::{self, RekeyRequest};

#[derive(Default)]
struct SessionState {
    database_key: Mutex<Option<String>>,
}

impl SessionState {
    fn get_key(&self) -> Option<String> {
        self.database_key.lock().ok().and_then(|guard| guard.clone())
    }
}

pub fn entrypoint() -> Result<()> {
    let arguments = std::env::args().collect::<Vec<_>>();
    if arguments.get(1).map(String::as_str) == Some("--worker") {
        let payload = vault_worker::run_worker_cli(&arguments[2..])?;
        println!("{payload}");
        return Ok(());
    }

    run_app()
}

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
            app_snapshot,
            save_config,
            initialize_archive,
            rekey_archive,
            set_session_database_key,
            clear_session_database_key,
            run_backup_now,
            query_history,
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
            reset_local_secret_vault
        ])
        .run(tauri::generate_context!())?;
    Ok(())
}

#[tauri::command]
fn app_snapshot(state: State<'_, SessionState>) -> Result<vault_core::AppSnapshot, String> {
    vault_worker::app_snapshot(state.get_key().as_deref()).map_err(|error| error.to_string())
}

#[tauri::command]
fn save_config(
    config: AppConfig,
    state: State<'_, SessionState>,
) -> Result<vault_core::AppSnapshot, String> {
    vault_worker::save_user_config(&config, state.get_key().as_deref())
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn initialize_archive(
    config: AppConfig,
    database_key: Option<String>,
    state: State<'_, SessionState>,
) -> Result<vault_core::AppSnapshot, String> {
    let snapshot = vault_worker::initialize_archive_database(&config, database_key.as_deref())
        .map_err(|error| error.to_string())?;
    *state.database_key.lock().map_err(|error| error.to_string())? = database_key;
    Ok(snapshot)
}

#[tauri::command]
fn rekey_archive(
    request: RekeyRequest,
    state: State<'_, SessionState>,
) -> Result<vault_core::AppSnapshot, String> {
    let old_key = state.get_key();
    let snapshot = vault_worker::rekey_archive_database(old_key.as_deref(), &request)
        .map_err(|error| error.to_string())?;
    *state.database_key.lock().map_err(|error| error.to_string())? = request.new_key;
    Ok(snapshot)
}

#[tauri::command]
fn set_session_database_key(
    database_key: String,
    state: State<'_, SessionState>,
) -> Result<(), String> {
    *state.database_key.lock().map_err(|error| error.to_string())? = Some(database_key);
    Ok(())
}

#[tauri::command]
fn clear_session_database_key(state: State<'_, SessionState>) -> Result<(), String> {
    *state.database_key.lock().map_err(|error| error.to_string())? = None;
    Ok(())
}

#[tauri::command]
fn run_backup_now(
    due_only: bool,
    state: State<'_, SessionState>,
) -> Result<vault_core::BackupReport, String> {
    vault_worker::run_backup_now(state.get_key().as_deref(), due_only)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn query_history(
    query: HistoryQuery,
    state: State<'_, SessionState>,
) -> Result<vault_core::HistoryQueryResponse, String> {
    vault_worker::query_history(state.get_key().as_deref(), query)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn export_history(
    request: ExportRequest,
    state: State<'_, SessionState>,
) -> Result<vault_core::ExportResult, String> {
    vault_worker::export_query(state.get_key().as_deref(), request)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn preview_remote_backup() -> Result<vault_core::RemoteBackupPreview, String> {
    vault_worker::preview_remote_backup_bundle().map_err(|error| error.to_string())
}

#[tauri::command]
fn run_remote_backup(
    state: State<'_, SessionState>,
) -> Result<vault_core::RemoteBackupResult, String> {
    vault_worker::upload_remote_backup_bundle(state.get_key().as_deref())
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn inspect_takeout(request: TakeoutRequest) -> Result<vault_core::TakeoutInspection, String> {
    vault_worker::inspect_takeout_source(&request).map_err(|error| error.to_string())
}

#[tauri::command]
fn import_takeout(
    request: TakeoutRequest,
    state: State<'_, SessionState>,
) -> Result<vault_core::TakeoutInspection, String> {
    vault_worker::import_takeout_source(state.get_key().as_deref(), &request)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn preview_import_batch(
    batch_id: i64,
    state: State<'_, SessionState>,
) -> Result<vault_core::ImportBatchDetail, String> {
    vault_worker::preview_import_batch_detail(state.get_key().as_deref(), batch_id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn revert_import_batch(
    batch_id: i64,
    state: State<'_, SessionState>,
) -> Result<vault_core::ImportBatchDetail, String> {
    vault_worker::revert_import_batch_detail(state.get_key().as_deref(), batch_id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn preview_schedule(platform: Option<String>) -> Result<SchedulePlan, String> {
    vault_worker::preview_schedule_plan(platform.as_deref(), None)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn apply_schedule(plan: SchedulePlan) -> Result<vault_core::ApplyResult, String> {
    vault_worker::apply_schedule_plan(&plan).map_err(|error| error.to_string())
}

#[tauri::command]
fn doctor_report(state: State<'_, SessionState>) -> Result<vault_core::HealthReport, String> {
    vault_worker::doctor_report(state.get_key().as_deref()).map_err(|error| error.to_string())
}

#[tauri::command]
fn keyring_status() -> vault_core::KeyringStatusReport {
    vault_worker::keyring_report()
}

#[tauri::command]
fn keyring_get_database_key() -> Result<Option<String>, String> {
    vault_worker::read_database_key_from_keyring().map_err(|error| error.to_string())
}

#[tauri::command]
fn keyring_store_database_key(value: String) -> Result<vault_core::KeyringStatusReport, String> {
    vault_worker::write_database_key_to_keyring(&value).map_err(|error| error.to_string())
}

#[tauri::command]
fn keyring_clear_database_key() -> Result<vault_core::KeyringStatusReport, String> {
    vault_worker::clear_database_key_from_keyring().map_err(|error| error.to_string())
}

#[tauri::command]
fn store_s3_credentials(credentials: S3CredentialInput) -> Result<(), String> {
    vault_worker::store_s3_credentials(&credentials).map_err(|error| error.to_string())
}

#[tauri::command]
fn clear_s3_credentials() -> Result<(), String> {
    vault_worker::clear_s3_credentials().map_err(|error| error.to_string())
}

#[tauri::command]
fn reset_local_secret_vault() -> Result<(), String> {
    vault_worker::reset_local_secret_vault().map_err(|error| error.to_string())
}
