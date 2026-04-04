use anyhow::Result;
use std::{
    path::{Path, PathBuf},
    process::Command,
    sync::Mutex,
};
#[cfg(not(test))]
use tauri::State;
#[cfg(not(test))]
use tauri_plugin_autostart::MacosLauncher;
use vault_core::{
    AiAssistantRequest, AiIndexRequest, AiProviderSecretInput, AiSearchRequest, AppConfig,
    ExportRequest, HistoryQuery, S3CredentialInput, SchedulePlan, TakeoutRequest,
};
use vault_worker::{self, RekeyRequest};

const PRODUCT_DISPLAY_NAME: &str = "Browser History Backup";

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
    if let Some(payload) = run_with_arguments(&arguments)? {
        println!("{payload}");
    }
    Ok(())
}

fn run_with_arguments(arguments: &[String]) -> Result<Option<String>> {
    if arguments.get(1).map(String::as_str) == Some("--worker") {
        return vault_worker::run_worker_cli(&arguments[2..]).map(Some);
    }
    run_app()?;
    Ok(None)
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

fn update_session_key(state: &SessionState, database_key: Option<String>) -> Result<(), String> {
    *state.database_key.lock().map_err(|error| error.to_string())? = database_key;
    Ok(())
}

fn session_key(state: &SessionState) -> Option<String> {
    state.get_key()
}

fn app_build_info_impl() -> vault_core::AppBuildInfo {
    vault_core::AppBuildInfo {
        product_name: PRODUCT_DISPLAY_NAME.to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        git_commit_short: option_env!("BHB_GIT_COMMIT_SHORT").unwrap_or("unknown").to_string(),
        git_commit_full: option_env!("BHB_GIT_COMMIT_FULL").unwrap_or("unknown").to_string(),
        git_dirty: option_env!("BHB_GIT_DIRTY").unwrap_or("false") == "true",
    }
}

fn resolve_file_manager_target(path: &str) -> Result<PathBuf, String> {
    let candidate = PathBuf::from(path);
    let target = if candidate.is_absolute() {
        candidate
    } else {
        std::env::current_dir().map_err(|error| error.to_string())?.join(candidate)
    };

    if target.is_dir() {
        return Ok(target);
    }
    if target.is_file() {
        return target
            .parent()
            .map(Path::to_path_buf)
            .ok_or_else(|| format!("Unable to open a parent directory for {}", target.display()));
    }

    Err(format!("Path does not exist: {}", target.display()))
}

#[cfg_attr(test, allow(dead_code))]
#[cfg(target_os = "macos")]
fn file_manager_command(target: &Path) -> (&'static str, Vec<String>) {
    ("open", vec![target.display().to_string()])
}

#[cfg_attr(test, allow(dead_code))]
#[cfg(target_os = "windows")]
fn file_manager_command(target: &Path) -> (&'static str, Vec<String>) {
    ("explorer", vec![target.display().to_string()])
}

#[cfg_attr(test, allow(dead_code))]
#[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
fn file_manager_command(target: &Path) -> (&'static str, Vec<String>) {
    ("xdg-open", vec![target.display().to_string()])
}

#[cfg_attr(test, allow(dead_code))]
fn open_path_in_file_manager_impl(path: String) -> Result<String, String> {
    let target = resolve_file_manager_target(&path)?;
    let (program, arguments) = file_manager_command(&target);
    Command::new(program)
        .args(arguments)
        .spawn()
        .map_err(|error| format!("Failed to open {}: {error}", target.display()))?;
    Ok(target.display().to_string())
}

fn app_snapshot_impl(
    session_database_key: Option<&str>,
) -> Result<vault_core::AppSnapshot, String> {
    vault_worker::app_snapshot(session_database_key).map_err(|error| error.to_string())
}

fn save_config_impl(
    config: AppConfig,
    session_database_key: Option<&str>,
) -> Result<vault_core::AppSnapshot, String> {
    vault_worker::save_user_config(&config, session_database_key).map_err(|error| error.to_string())
}

fn initialize_archive_impl(
    config: AppConfig,
    database_key: Option<String>,
    state: &SessionState,
) -> Result<vault_core::AppSnapshot, String> {
    let snapshot = vault_worker::initialize_archive_database(&config, database_key.as_deref())
        .map_err(|error| error.to_string())?;
    update_session_key(state, database_key)?;
    Ok(snapshot)
}

fn rekey_archive_impl(
    request: RekeyRequest,
    state: &SessionState,
) -> Result<vault_core::AppSnapshot, String> {
    let old_key = session_key(state);
    let snapshot = vault_worker::rekey_archive_database(old_key.as_deref(), &request)
        .map_err(|error| error.to_string())?;
    update_session_key(state, request.new_key)?;
    Ok(snapshot)
}

fn set_session_database_key_impl(database_key: String, state: &SessionState) -> Result<(), String> {
    update_session_key(state, Some(database_key))
}

fn clear_session_database_key_impl(state: &SessionState) -> Result<(), String> {
    update_session_key(state, None)
}

fn run_backup_now_impl(
    due_only: bool,
    session_database_key: Option<&str>,
) -> Result<vault_core::BackupReport, String> {
    vault_worker::run_backup_now(session_database_key, due_only).map_err(|error| error.to_string())
}

fn query_history_impl(
    query: HistoryQuery,
    session_database_key: Option<&str>,
) -> Result<vault_core::HistoryQueryResponse, String> {
    vault_worker::query_history(session_database_key, query).map_err(|error| error.to_string())
}

fn export_history_impl(
    request: ExportRequest,
    session_database_key: Option<&str>,
) -> Result<vault_core::ExportResult, String> {
    vault_worker::export_query(session_database_key, request).map_err(|error| error.to_string())
}

fn preview_remote_backup_impl() -> Result<vault_core::RemoteBackupPreview, String> {
    vault_worker::preview_remote_backup_bundle().map_err(|error| error.to_string())
}

fn run_remote_backup_impl(
    session_database_key: Option<&str>,
) -> Result<vault_core::RemoteBackupResult, String> {
    vault_worker::upload_remote_backup_bundle(session_database_key)
        .map_err(|error| error.to_string())
}

fn inspect_takeout_impl(request: TakeoutRequest) -> Result<vault_core::TakeoutInspection, String> {
    vault_worker::inspect_takeout_source(&request).map_err(|error| error.to_string())
}

fn import_takeout_impl(
    request: TakeoutRequest,
    session_database_key: Option<&str>,
) -> Result<vault_core::TakeoutInspection, String> {
    vault_worker::import_takeout_source(session_database_key, &request)
        .map_err(|error| error.to_string())
}

fn preview_import_batch_impl(
    batch_id: i64,
    session_database_key: Option<&str>,
) -> Result<vault_core::ImportBatchDetail, String> {
    vault_worker::preview_import_batch_detail(session_database_key, batch_id)
        .map_err(|error| error.to_string())
}

fn revert_import_batch_impl(
    batch_id: i64,
    session_database_key: Option<&str>,
) -> Result<vault_core::ImportBatchDetail, String> {
    vault_worker::revert_import_batch_detail(session_database_key, batch_id)
        .map_err(|error| error.to_string())
}

fn preview_schedule_impl(platform: Option<String>) -> Result<SchedulePlan, String> {
    vault_worker::preview_schedule_plan(platform.as_deref(), None)
        .map_err(|error| error.to_string())
}

fn apply_schedule_impl(plan: SchedulePlan) -> Result<vault_core::ApplyResult, String> {
    vault_worker::apply_schedule_plan(&plan).map_err(|error| error.to_string())
}

fn doctor_report_impl(
    session_database_key: Option<&str>,
) -> Result<vault_core::HealthReport, String> {
    vault_worker::doctor_report(session_database_key).map_err(|error| error.to_string())
}

fn keyring_status_impl() -> vault_core::KeyringStatusReport {
    vault_worker::keyring_report()
}

fn keyring_get_database_key_impl() -> Result<Option<String>, String> {
    vault_worker::read_database_key_from_keyring().map_err(|error| error.to_string())
}

fn keyring_store_database_key_impl(
    value: String,
) -> Result<vault_core::KeyringStatusReport, String> {
    vault_worker::write_database_key_to_keyring(&value).map_err(|error| error.to_string())
}

fn keyring_clear_database_key_impl() -> Result<vault_core::KeyringStatusReport, String> {
    vault_worker::clear_database_key_from_keyring().map_err(|error| error.to_string())
}

fn store_s3_credentials_impl(credentials: S3CredentialInput) -> Result<(), String> {
    vault_worker::store_s3_credentials(&credentials).map_err(|error| error.to_string())
}

fn clear_s3_credentials_impl() -> Result<(), String> {
    vault_worker::clear_s3_credentials().map_err(|error| error.to_string())
}

fn store_ai_provider_api_key_impl(
    input: AiProviderSecretInput,
    session_database_key: Option<&str>,
) -> Result<vault_core::AppSnapshot, String> {
    vault_worker::store_ai_provider_api_key(&input, session_database_key)
        .map_err(|error| error.to_string())
}

fn clear_ai_provider_api_key_impl(
    provider_id: String,
    session_database_key: Option<&str>,
) -> Result<vault_core::AppSnapshot, String> {
    vault_worker::clear_ai_provider_api_key(&provider_id, session_database_key)
        .map_err(|error| error.to_string())
}

fn build_ai_index_impl(
    request: AiIndexRequest,
    session_database_key: Option<&str>,
) -> Result<vault_core::AiIndexReport, String> {
    vault_worker::build_ai_index_now(session_database_key, &request)
        .map_err(|error| error.to_string())
}

fn search_ai_history_impl(
    request: AiSearchRequest,
    session_database_key: Option<&str>,
) -> Result<vault_core::AiSearchResponse, String> {
    vault_worker::search_ai_history(session_database_key, &request)
        .map_err(|error| error.to_string())
}

fn ask_ai_assistant_impl(
    request: AiAssistantRequest,
    session_database_key: Option<&str>,
) -> Result<vault_core::AiAssistantResponse, String> {
    vault_worker::ask_ai_assistant(session_database_key, &request)
        .map_err(|error| error.to_string())
}

fn preview_ai_integrations_impl() -> Result<vault_core::AiIntegrationPreview, String> {
    vault_worker::preview_ai_integration_files().map_err(|error| error.to_string())
}

fn reset_local_secret_vault_impl() -> Result<(), String> {
    vault_worker::reset_local_secret_vault().map_err(|error| error.to_string())
}

#[cfg(not(test))]
#[tauri::command]
fn app_build_info() -> vault_core::AppBuildInfo {
    app_build_info_impl()
}

#[cfg(not(test))]
#[tauri::command]
fn app_snapshot(state: State<'_, SessionState>) -> Result<vault_core::AppSnapshot, String> {
    app_snapshot_impl(state.get_key().as_deref())
}

#[cfg(not(test))]
#[tauri::command]
fn save_config(
    config: AppConfig,
    state: State<'_, SessionState>,
) -> Result<vault_core::AppSnapshot, String> {
    save_config_impl(config, state.get_key().as_deref())
}

#[cfg(not(test))]
#[tauri::command]
fn initialize_archive(
    config: AppConfig,
    database_key: Option<String>,
    state: State<'_, SessionState>,
) -> Result<vault_core::AppSnapshot, String> {
    initialize_archive_impl(config, database_key, &state)
}

#[cfg(not(test))]
#[tauri::command]
fn rekey_archive(
    request: RekeyRequest,
    state: State<'_, SessionState>,
) -> Result<vault_core::AppSnapshot, String> {
    rekey_archive_impl(request, &state)
}

#[cfg(not(test))]
#[tauri::command]
fn set_session_database_key(
    database_key: String,
    state: State<'_, SessionState>,
) -> Result<(), String> {
    set_session_database_key_impl(database_key, &state)
}

#[cfg(not(test))]
#[tauri::command]
fn clear_session_database_key(state: State<'_, SessionState>) -> Result<(), String> {
    clear_session_database_key_impl(&state)
}

#[cfg(not(test))]
#[tauri::command]
fn run_backup_now(
    due_only: bool,
    state: State<'_, SessionState>,
) -> Result<vault_core::BackupReport, String> {
    run_backup_now_impl(due_only, state.get_key().as_deref())
}

#[cfg(not(test))]
#[tauri::command]
fn query_history(
    query: HistoryQuery,
    state: State<'_, SessionState>,
) -> Result<vault_core::HistoryQueryResponse, String> {
    query_history_impl(query, state.get_key().as_deref())
}

#[cfg(not(test))]
#[tauri::command]
fn export_history(
    request: ExportRequest,
    state: State<'_, SessionState>,
) -> Result<vault_core::ExportResult, String> {
    export_history_impl(request, state.get_key().as_deref())
}

#[cfg(not(test))]
#[tauri::command]
fn preview_remote_backup() -> Result<vault_core::RemoteBackupPreview, String> {
    preview_remote_backup_impl()
}

#[cfg(not(test))]
#[tauri::command]
fn run_remote_backup(
    state: State<'_, SessionState>,
) -> Result<vault_core::RemoteBackupResult, String> {
    run_remote_backup_impl(state.get_key().as_deref())
}

#[cfg(not(test))]
#[tauri::command]
fn inspect_takeout(request: TakeoutRequest) -> Result<vault_core::TakeoutInspection, String> {
    inspect_takeout_impl(request)
}

#[cfg(not(test))]
#[tauri::command]
fn import_takeout(
    request: TakeoutRequest,
    state: State<'_, SessionState>,
) -> Result<vault_core::TakeoutInspection, String> {
    import_takeout_impl(request, state.get_key().as_deref())
}

#[cfg(not(test))]
#[tauri::command]
fn preview_import_batch(
    batch_id: i64,
    state: State<'_, SessionState>,
) -> Result<vault_core::ImportBatchDetail, String> {
    preview_import_batch_impl(batch_id, state.get_key().as_deref())
}

#[cfg(not(test))]
#[tauri::command]
fn revert_import_batch(
    batch_id: i64,
    state: State<'_, SessionState>,
) -> Result<vault_core::ImportBatchDetail, String> {
    revert_import_batch_impl(batch_id, state.get_key().as_deref())
}

#[cfg(not(test))]
#[tauri::command]
fn preview_schedule(platform: Option<String>) -> Result<SchedulePlan, String> {
    preview_schedule_impl(platform)
}

#[cfg(not(test))]
#[tauri::command]
fn apply_schedule(plan: SchedulePlan) -> Result<vault_core::ApplyResult, String> {
    apply_schedule_impl(plan)
}

#[cfg(not(test))]
#[tauri::command]
fn doctor_report(state: State<'_, SessionState>) -> Result<vault_core::HealthReport, String> {
    doctor_report_impl(state.get_key().as_deref())
}

#[cfg(not(test))]
#[tauri::command]
fn keyring_status() -> vault_core::KeyringStatusReport {
    keyring_status_impl()
}

#[cfg(not(test))]
#[tauri::command]
fn keyring_get_database_key() -> Result<Option<String>, String> {
    keyring_get_database_key_impl()
}

#[cfg(not(test))]
#[tauri::command]
fn keyring_store_database_key(value: String) -> Result<vault_core::KeyringStatusReport, String> {
    keyring_store_database_key_impl(value)
}

#[cfg(not(test))]
#[tauri::command]
fn keyring_clear_database_key() -> Result<vault_core::KeyringStatusReport, String> {
    keyring_clear_database_key_impl()
}

#[cfg(not(test))]
#[tauri::command]
fn store_s3_credentials(credentials: S3CredentialInput) -> Result<(), String> {
    store_s3_credentials_impl(credentials)
}

#[cfg(not(test))]
#[tauri::command]
fn clear_s3_credentials() -> Result<(), String> {
    clear_s3_credentials_impl()
}

#[cfg(not(test))]
#[tauri::command]
fn store_ai_provider_api_key(
    input: AiProviderSecretInput,
    state: State<'_, SessionState>,
) -> Result<vault_core::AppSnapshot, String> {
    store_ai_provider_api_key_impl(input, state.get_key().as_deref())
}

#[cfg(not(test))]
#[tauri::command]
fn clear_ai_provider_api_key(
    provider_id: String,
    state: State<'_, SessionState>,
) -> Result<vault_core::AppSnapshot, String> {
    clear_ai_provider_api_key_impl(provider_id, state.get_key().as_deref())
}

#[cfg(not(test))]
#[tauri::command]
fn build_ai_index(
    request: AiIndexRequest,
    state: State<'_, SessionState>,
) -> Result<vault_core::AiIndexReport, String> {
    build_ai_index_impl(request, state.get_key().as_deref())
}

#[cfg(not(test))]
#[tauri::command]
fn search_ai_history(
    request: AiSearchRequest,
    state: State<'_, SessionState>,
) -> Result<vault_core::AiSearchResponse, String> {
    search_ai_history_impl(request, state.get_key().as_deref())
}

#[cfg(not(test))]
#[tauri::command]
fn ask_ai_assistant(
    request: AiAssistantRequest,
    state: State<'_, SessionState>,
) -> Result<vault_core::AiAssistantResponse, String> {
    ask_ai_assistant_impl(request, state.get_key().as_deref())
}

#[cfg(not(test))]
#[tauri::command]
fn preview_ai_integrations() -> Result<vault_core::AiIntegrationPreview, String> {
    preview_ai_integrations_impl()
}

#[cfg(not(test))]
#[tauri::command]
fn reset_local_secret_vault() -> Result<(), String> {
    reset_local_secret_vault_impl()
}

#[cfg(not(test))]
#[tauri::command]
fn open_path_in_file_manager(path: String) -> Result<String, String> {
    open_path_in_file_manager_impl(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;
    use std::{
        fs,
        path::{Path, PathBuf},
        sync::{Mutex, OnceLock},
    };
    use tempfile::tempdir;
    use vault_core::{
        AiProviderConfig, AiProviderPurpose, AiRequestFormat, AppConfig, ArchiveMode,
    };

    const PROJECT_ROOT_OVERRIDE_ENV: &str = "CHB_PROJECT_ROOT";
    const CHROME_USER_DATA_OVERRIDE_ENV: &str = "CHB_CHROME_USER_DATA_DIR";
    const TEST_KEYRING_OVERRIDE_ENV: &str = "CHB_TEST_KEYRING_DIR";

    fn env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    fn initialized_config() -> AppConfig {
        let mut config = AppConfig {
            initialized: true,
            archive_mode: ArchiveMode::Plaintext,
            git_enabled: false,
            due_after_hours: 72,
            checkpoint_days: 1,
            selected_profile_ids: vec!["chrome:Default".to_string()],
            ..AppConfig::default()
        };
        config.ai.enabled = true;
        config.ai.assistant_enabled = true;
        config.ai.semantic_index_enabled = true;
        config.ai.mcp_enabled = true;
        config.ai.skill_enabled = true;
        config.ai.llm_provider_id = Some("llm-primary".to_string());
        config.ai.embedding_provider_id = Some("embed-primary".to_string());
        config.ai.llm_providers = vec![AiProviderConfig {
            id: "llm-primary".to_string(),
            name: "Primary LLM".to_string(),
            purpose: AiProviderPurpose::Llm,
            request_format: AiRequestFormat::OpenAi,
            enabled: true,
            default_model: "gpt-4.1-mini".to_string(),
            ..AiProviderConfig::default()
        }];
        config.ai.embedding_providers = vec![AiProviderConfig {
            id: "embed-primary".to_string(),
            name: "Primary embedding".to_string(),
            purpose: AiProviderPurpose::Embedding,
            request_format: AiRequestFormat::OpenAi,
            enabled: true,
            default_model: "text-embedding-3-large".to_string(),
            dimensions: Some(1536),
            ..AiProviderConfig::default()
        }];
        config
    }

    fn chrome_user_data_fixture(root: &Path) -> PathBuf {
        let chrome_root = root.join("chrome-user-data");
        let profile_dir = chrome_root.join("Default");
        fs::create_dir_all(&profile_dir).expect("create chrome profile dir");
        fs::write(chrome_root.join("Last Version"), "135.0.0.0").expect("write version");
        fs::write(
            chrome_root.join("Local State"),
            r#"{"profile":{"info_cache":{"Default":{"name":"Default","user_name":"tester@example.com"}}}}"#,
        )
        .expect("write local state");

        let history = Connection::open(profile_dir.join("History")).expect("open source history");
        history
            .execute_batch(
                "
                CREATE TABLE urls (
                  id INTEGER PRIMARY KEY,
                  url TEXT NOT NULL,
                  title TEXT,
                  visit_count INTEGER NOT NULL,
                  typed_count INTEGER NOT NULL,
                  last_visit_time INTEGER NOT NULL,
                  hidden INTEGER NOT NULL
                );
                CREATE TABLE visits (
                  id INTEGER PRIMARY KEY,
                  url INTEGER NOT NULL,
                  visit_time INTEGER NOT NULL,
                  from_visit INTEGER,
                  transition INTEGER,
                  visit_duration INTEGER,
                  is_known_to_sync INTEGER,
                  visited_link_id INTEGER,
                  external_referrer_url TEXT,
                  app_id TEXT
                );
                CREATE TABLE downloads (
                  id INTEGER PRIMARY KEY,
                  guid TEXT,
                  current_path TEXT,
                  target_path TEXT,
                  start_time INTEGER,
                  received_bytes INTEGER,
                  total_bytes INTEGER,
                  state INTEGER,
                  mime_type TEXT,
                  original_mime_type TEXT
                );
                CREATE TABLE keyword_search_terms (
                  keyword_id INTEGER,
                  url_id INTEGER,
                  term TEXT,
                  normalized_term TEXT
                );",
            )
            .expect("create history schema");
        history
            .execute(
                "INSERT INTO urls (id, url, title, visit_count, typed_count, last_visit_time, hidden)
                 VALUES (1, 'https://example.com', 'Example', 1, 1, 1, 0)",
                [],
            )
            .expect("insert url");
        history
            .execute(
                "INSERT INTO visits (id, url, visit_time, from_visit, transition, visit_duration, is_known_to_sync, visited_link_id, external_referrer_url, app_id)
                 VALUES (1, 1, 1, NULL, 805306368, 24000, 1, 3, 'https://ref.example', 'com.example.app')",
                [],
            )
            .expect("insert visit");
        history
            .execute(
                "INSERT INTO downloads (id, guid, current_path, target_path, start_time, received_bytes, total_bytes, state, mime_type, original_mime_type)
                 VALUES (1, 'guid-1', '/tmp/current', '/tmp/target', 1, 1, 2, 3, 'text/html', 'text/plain')",
                [],
            )
            .expect("insert download");
        history
            .execute(
                "INSERT INTO keyword_search_terms (keyword_id, url_id, term, normalized_term)
                 VALUES (1, 1, 'browser history backup', 'browser history backup')",
                [],
            )
            .expect("insert search term");

        chrome_root
    }

    fn takeout_fixture(root: &Path) -> String {
        let source_dir = root.join("takeout-source");
        fs::create_dir_all(&source_dir).expect("takeout dir");
        fs::write(
            source_dir.join("takeout.jsonl"),
            r#"{"url":"https://example.com/takeout","title":"Takeout","visitedAt":"2026-04-01T10:00:00+00:00"}"#,
        )
        .expect("write takeout");
        source_dir.display().to_string()
    }

    #[test]
    fn command_helpers_cover_local_desktop_flows() {
        let _guard = env_lock().lock().expect("env lock");
        let dir = tempdir().expect("tempdir");
        let chrome_root = chrome_user_data_fixture(dir.path());
        let keyring_root = dir.path().join("test-keyring");
        let takeout_source = takeout_fixture(dir.path());

        unsafe {
            std::env::set_var(PROJECT_ROOT_OVERRIDE_ENV, dir.path());
            std::env::set_var(CHROME_USER_DATA_OVERRIDE_ENV, &chrome_root);
            std::env::set_var(TEST_KEYRING_OVERRIDE_ENV, &keyring_root);
        }

        let session = SessionState::default();
        let config = initialized_config();

        let snapshot =
            initialize_archive_impl(config.clone(), None, &session).expect("initialize archive");
        assert!(snapshot.archive_status.initialized);
        assert_eq!(
            save_config_impl(config.clone(), session_key(&session).as_deref())
                .expect("save config")
                .config
                .selected_profile_ids,
            config.selected_profile_ids
        );

        let backup = run_backup_now_impl(false, session_key(&session).as_deref()).expect("backup");
        assert_eq!(backup.run.expect("run").new_visits, 1);

        let history = query_history_impl(
            HistoryQuery {
                q: Some("example".to_string()),
                profile_id: None,
                domain: None,
                limit: Some(10),
            },
            session_key(&session).as_deref(),
        )
        .expect("query history");
        assert_eq!(history.total, 1);

        let export = export_history_impl(
            ExportRequest {
                query: HistoryQuery::default(),
                format: vault_core::ExportFormat::Jsonl,
            },
            session_key(&session).as_deref(),
        )
        .expect("export history");
        assert_eq!(export.count, 1);

        let inspected = inspect_takeout_impl(TakeoutRequest {
            source_path: takeout_source.clone(),
            dry_run: true,
        })
        .expect("inspect takeout");
        assert_eq!(inspected.candidate_items, 1);

        let imported = import_takeout_impl(
            TakeoutRequest { source_path: takeout_source, dry_run: false },
            session_key(&session).as_deref(),
        )
        .expect("import takeout");
        let batch_id = imported.import_batch.expect("import batch").id;
        assert_eq!(imported.imported_items, 1);
        let import_preview = preview_import_batch_impl(batch_id, session_key(&session).as_deref())
            .expect("preview import batch");
        assert_eq!(import_preview.batch.status, "imported");
        assert_eq!(import_preview.preview_entries.len(), 1);
        let reverted_batch = revert_import_batch_impl(batch_id, session_key(&session).as_deref())
            .expect("revert import batch");
        assert_eq!(reverted_batch.batch.status, "reverted");
        assert_eq!(reverted_batch.batch.visible_items, 0);

        let keyring = keyring_store_database_key_impl("session-secret".to_string())
            .expect("store database key");
        assert!(keyring.stored_secret);
        assert_eq!(keyring_status_impl().stored_secret, keyring.stored_secret);
        assert_eq!(
            keyring_get_database_key_impl().expect("read database key"),
            Some("session-secret".to_string())
        );
        assert!(!keyring_clear_database_key_impl().expect("clear keyring key").stored_secret);

        store_s3_credentials_impl(S3CredentialInput {
            access_key_id: "test-access".to_string(),
            secret_access_key: "test-secret".to_string(),
        })
        .expect("store s3 credentials");

        let mut remote_config = config.clone();
        remote_config.remote_backup.enabled = true;
        remote_config.remote_backup.bucket = "browser-history-backup-tests".to_string();
        remote_config.remote_backup.region = "us-west-2".to_string();
        remote_config.remote_backup.prefix = "archives".to_string();
        let saved_snapshot =
            save_config_impl(remote_config.clone(), session_key(&session).as_deref())
                .expect("save remote config");
        assert!(saved_snapshot.config.remote_backup.credentials_saved);

        let remote_preview = preview_remote_backup_impl().expect("preview remote backup");
        assert!(remote_preview.preview_command.contains("curl"));
        clear_s3_credentials_impl().expect("clear s3 credentials");
        let remote_error = run_remote_backup_impl(session_key(&session).as_deref())
            .expect_err("remote backup should require stored credentials");
        assert!(remote_error.contains("S3"));

        let provider_snapshot = store_ai_provider_api_key_impl(
            AiProviderSecretInput {
                provider_id: "llm-primary".to_string(),
                api_key: "secret".to_string(),
            },
            session_key(&session).as_deref(),
        )
        .expect("store provider key");
        assert!(provider_snapshot.config.ai.llm_providers[0].api_key_saved);
        let cleared_provider_snapshot = clear_ai_provider_api_key_impl(
            "llm-primary".to_string(),
            session_key(&session).as_deref(),
        )
        .expect("clear provider key");
        assert!(!cleared_provider_snapshot.config.ai.llm_providers[0].api_key_saved);
        let ai_index_error = build_ai_index_impl(
            AiIndexRequest { provider_id: None, full_rebuild: false, limit: Some(5) },
            session_key(&session).as_deref(),
        )
        .expect_err("index build should require saved embedding provider key");
        assert!(ai_index_error.contains("API key") || ai_index_error.contains("embedding"));

        let ai_search = search_ai_history_impl(
            AiSearchRequest {
                query: "example".to_string(),
                profile_id: None,
                domain: None,
                limit: Some(5),
            },
            session_key(&session).as_deref(),
        )
        .expect("ai search");
        assert_eq!(ai_search.total, 1);

        let assistant_error = ask_ai_assistant_impl(
            AiAssistantRequest {
                question: "What did I visit?".to_string(),
                profile_id: None,
                domain: None,
            },
            session_key(&session).as_deref(),
        )
        .expect_err("assistant should require saved provider key");
        assert!(assistant_error.contains("API key"));

        let preview = preview_ai_integrations_impl().expect("preview ai integrations");
        assert!(preview.mcp_command.contains("mcp-server"));

        let plan = preview_schedule_impl(Some("linux".to_string())).expect("schedule preview");
        assert_eq!(plan.platform, "linux");
        let applied = apply_schedule_impl(plan).expect("apply schedule");
        assert!(!applied.applied);

        let doctor = doctor_report_impl(session_key(&session).as_deref()).expect("doctor");
        assert!(!doctor.checks.is_empty());
        let rekeyed_snapshot = rekey_archive_impl(
            RekeyRequest {
                new_mode: ArchiveMode::Encrypted,
                new_key: Some("vault-passphrase".to_string()),
            },
            &session,
        )
        .expect("rekey archive");
        assert!(rekeyed_snapshot.archive_status.encrypted);

        let snapshot_again =
            app_snapshot_impl(session_key(&session).as_deref()).expect("app snapshot");
        assert_eq!(snapshot_again.browser_profiles.len(), 1);
        assert!(snapshot_again.archive_status.encrypted);
        fs::write(dir.path().join("vault.hold"), "secret vault").expect("write stronghold fixture");
        reset_local_secret_vault_impl().expect("reset local secret vault");
        assert!(!dir.path().join("vault.hold").exists());
        let worker_payload = run_with_arguments(&[
            "browser-history-backup".to_string(),
            "--worker".to_string(),
            "doctor".to_string(),
        ])
        .expect("run worker doctor")
        .expect("worker payload");
        assert!(worker_payload.contains("checks"));
        assert!(run_with_arguments(&["browser-history-backup".to_string()]).is_ok());

        unsafe {
            std::env::remove_var(PROJECT_ROOT_OVERRIDE_ENV);
            std::env::remove_var(CHROME_USER_DATA_OVERRIDE_ENV);
            std::env::remove_var(TEST_KEYRING_OVERRIDE_ENV);
        }
    }

    #[test]
    fn session_helpers_round_trip_database_key() {
        let session = SessionState::default();
        assert_eq!(session_key(&session), None);
        set_session_database_key_impl("abc".to_string(), &session).expect("set key");
        assert_eq!(session_key(&session), Some("abc".to_string()));
        clear_session_database_key_impl(&session).expect("clear key");
        assert_eq!(session_key(&session), None);
    }

    #[test]
    fn build_info_exposes_version_and_git_metadata() {
        let info = app_build_info_impl();
        assert_eq!(info.product_name, PRODUCT_DISPLAY_NAME);
        assert!(!info.version.is_empty());
        assert!(!info.git_commit_short.is_empty());
        assert!(!info.git_commit_full.is_empty());
    }

    #[test]
    fn resolve_file_manager_target_prefers_directory_and_parent_folder() {
        let dir = tempdir().expect("tempdir");
        let nested_dir = dir.path().join("nested");
        fs::create_dir_all(&nested_dir).expect("create nested dir");
        let file_path = nested_dir.join("archive.sqlite");
        fs::write(&file_path, "sqlite").expect("write file");

        assert_eq!(
            resolve_file_manager_target(&nested_dir.display().to_string()).expect("resolve dir"),
            nested_dir
        );
        assert_eq!(
            resolve_file_manager_target(&file_path.display().to_string())
                .expect("resolve file parent"),
            nested_dir
        );
    }

    #[test]
    fn resolve_file_manager_target_rejects_missing_paths() {
        let error = resolve_file_manager_target("/tmp/browser-history-backup-does-not-exist")
            .expect_err("missing path should fail");
        assert!(error.contains("Path does not exist"));
    }
}
