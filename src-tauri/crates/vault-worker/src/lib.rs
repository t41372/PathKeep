use anyhow::{Context, Result};
use rmcp::{
    ServerHandler, ServiceExt,
    handler::server::{
        router::tool::ToolRouter,
        wrapper::{Json, Parameters},
    },
    schemars,
    schemars::JsonSchema,
    tool, tool_handler, tool_router,
};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tokio::runtime::Runtime;
use vault_core::{
    AiAssistantRequest, AiAssistantResponse, AiIndexReport, AiIndexRequest, AiIndexStatus,
    AiIntegrationPreview, AiProviderConfig, AiProviderPurpose, AiProviderRuntime,
    AiProviderSecretInput, AiSearchRequest, AiSearchResponse, AppConfig, AppSnapshot, ArchiveMode,
    ExportRequest, HealthReport, HistoryQuery, HistoryQueryResponse, ImportBatchDetail,
    KeyringStatusReport, RemoteBackupPreview, RemoteBackupResult, S3CredentialInput, SchedulePlan,
    TakeoutInspection, TakeoutRequest, ai_index_status, answer_history_question, archive_status,
    build_ai_index, doctor, ensure_archive_initialized, export_history, import_takeout,
    inspect_takeout, list_history, load_config, load_import_batches, load_recent_runs,
    preview_ai_integrations, preview_import_batch, preview_remote_backup, project_paths,
    rekey_archive, revert_import_batch, run_backup, run_remote_backup, save_config,
    semantic_search_history,
};
use vault_platform::{
    ScheduleParameters, apply_schedule, keyring_clear_database_key, keyring_clear_provider_api_key,
    keyring_clear_s3_credentials, keyring_get_database_key, keyring_get_provider_api_key,
    keyring_get_s3_credentials, keyring_set_database_key, keyring_set_provider_api_key,
    keyring_set_s3_credentials, keyring_status, preview_schedule, provider_api_key_saved,
    s3_credentials_saved,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RekeyRequest {
    pub new_mode: ArchiveMode,
    pub new_key: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
struct McpSearchRequest {
    query: String,
    profile_id: Option<String>,
    domain: Option<String>,
    limit: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
struct McpSearchResult {
    total: usize,
    provider_id: String,
    model: String,
    items: Vec<McpSearchItem>,
    notes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
struct McpSearchItem {
    history_id: i64,
    profile_id: String,
    url: String,
    title: Option<String>,
    domain: String,
    visited_at: String,
    score: f32,
    match_reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
struct McpArchiveStatus {
    initialized: bool,
    encrypted: bool,
    unlocked: bool,
    ai_enabled: bool,
    assistant_enabled: bool,
    semantic_index_enabled: bool,
    indexed_items: usize,
    warning: Option<String>,
}

#[derive(Debug, Clone)]
struct BrowserHistoryMcpServer {
    database_key: Option<String>,
    tool_router: ToolRouter<Self>,
}

impl BrowserHistoryMcpServer {
    fn new(database_key: Option<String>) -> Self {
        Self { database_key, tool_router: Self::tool_router() }
    }
}

fn tokio_runtime() -> Result<Runtime> {
    Runtime::new().context("creating tokio runtime for Browser History Backup worker")
}

fn hydrate_provider_collection(providers: &mut [AiProviderConfig]) {
    for provider in providers {
        provider.api_key_saved = provider_api_key_saved(&provider.id);
    }
}

fn hydrate_derived_config_state(config: &mut AppConfig) {
    config.remote_backup.credentials_saved = s3_credentials_saved();
    hydrate_provider_collection(&mut config.ai.llm_providers);
    hydrate_provider_collection(&mut config.ai.embedding_providers);
}

fn derive_ai_status(
    paths: &vault_core::ProjectPaths,
    config: &AppConfig,
    session_database_key: Option<&str>,
) -> AiIndexStatus {
    match ai_index_status(paths, config, session_database_key) {
        Ok(status) => status,
        Err(error) => AiIndexStatus {
            enabled: config.ai.enabled,
            assistant_enabled: config.ai.assistant_enabled,
            mcp_enabled: config.ai.mcp_enabled,
            skill_enabled: config.ai.skill_enabled,
            llm_provider_id: config.ai.llm_provider_id.clone(),
            embedding_provider_id: config.ai.embedding_provider_id.clone(),
            warning: Some(error.to_string()),
            ..AiIndexStatus::default()
        },
    }
}

fn resolve_provider_runtime(
    providers: &[AiProviderConfig],
    provider_id: &str,
    expected_purpose: AiProviderPurpose,
) -> Result<AiProviderRuntime> {
    let config = providers
        .iter()
        .find(|provider| provider.id == provider_id)
        .cloned()
        .with_context(|| format!("provider {provider_id} was not found in Settings"))?;
    if config.purpose != expected_purpose {
        anyhow::bail!(
            "Provider {} is configured for {:?}, not {:?}.",
            config.name,
            config.purpose,
            expected_purpose
        );
    }
    let api_key = keyring_get_provider_api_key(provider_id)?
        .with_context(|| format!("store an API key for provider {}", config.name))?;
    Ok(AiProviderRuntime { config, api_key })
}

fn selected_embedding_provider_runtime(
    config: &AppConfig,
    preferred_id: Option<&str>,
) -> Result<AiProviderRuntime> {
    let provider_id = preferred_id
        .or(config.ai.embedding_provider_id.as_deref())
        .context("select an embedding provider in Settings before building the semantic index")?;
    resolve_provider_runtime(
        &config.ai.embedding_providers,
        provider_id,
        AiProviderPurpose::Embedding,
    )
}

fn selected_llm_provider_runtime(
    config: &AppConfig,
    preferred_id: Option<&str>,
) -> Result<AiProviderRuntime> {
    let provider_id = preferred_id
        .or(config.ai.llm_provider_id.as_deref())
        .context("select an LLM provider in Settings before using the assistant")?;
    resolve_provider_runtime(&config.ai.llm_providers, provider_id, AiProviderPurpose::Llm)
}

fn selected_optional_embedding_runtime(config: &AppConfig) -> Result<Option<AiProviderRuntime>> {
    match config.ai.embedding_provider_id.as_deref() {
        Some(provider_id) => resolve_provider_runtime(
            &config.ai.embedding_providers,
            provider_id,
            AiProviderPurpose::Embedding,
        )
        .map(Some),
        None => Ok(None),
    }
}

fn search_response_with_resolution_note(
    mut response: AiSearchResponse,
    resolution_error: Option<anyhow::Error>,
) -> AiSearchResponse {
    if let Some(error) = resolution_error {
        response.notes.push(format!(
            "Semantic retrieval is unavailable right now: {}. Showing lexical results only.",
            error
        ));
    }
    response
}

pub fn app_snapshot(session_database_key: Option<&str>) -> Result<AppSnapshot> {
    let paths = project_paths()?;
    let mut config = load_config(&paths)?;
    hydrate_derived_config_state(&mut config);
    let browser_profiles = vault_core::discover_profiles()?;
    let archive_status = archive_status(&paths, &config, session_database_key)?;
    let ai_status = derive_ai_status(&paths, &config, session_database_key);
    let recent_runs = load_recent_runs(&paths, &config, session_database_key).unwrap_or_default();
    let recent_import_batches =
        load_import_batches(&paths, &config, session_database_key).unwrap_or_default();

    Ok(AppSnapshot {
        directories: vault_core::AppDirectories {
            app_root: paths.app_root.display().to_string(),
            config_path: paths.config_path.display().to_string(),
            archive_database_path: paths.archive_database_path.display().to_string(),
            audit_repo_path: paths.audit_repo_path.display().to_string(),
            manifests_dir: paths.manifests_dir.display().to_string(),
            exports_dir: paths.exports_dir.display().to_string(),
            raw_snapshots_dir: paths.raw_snapshots_dir.display().to_string(),
            staging_dir: paths.staging_dir.display().to_string(),
            quarantine_dir: paths.quarantine_dir.display().to_string(),
            schedule_dir: paths.schedule_dir.display().to_string(),
            stronghold_path: paths.stronghold_path.display().to_string(),
            stronghold_salt_path: paths.stronghold_salt_path.display().to_string(),
        },
        config,
        archive_status,
        keyring_status: keyring_status(),
        ai_status,
        browser_profiles,
        recent_runs,
        recent_import_batches,
    })
}

pub fn save_user_config(
    config: &AppConfig,
    session_database_key: Option<&str>,
) -> Result<AppSnapshot> {
    let paths = project_paths()?;
    let mut next_config = config.clone();
    hydrate_derived_config_state(&mut next_config);
    save_config(&paths, &next_config)?;
    app_snapshot(session_database_key)
}

pub fn initialize_archive_database(
    config: &AppConfig,
    database_key: Option<&str>,
) -> Result<AppSnapshot> {
    let paths = project_paths()?;
    let mut next_config = config.clone();
    hydrate_derived_config_state(&mut next_config);
    save_config(&paths, &next_config)?;
    ensure_archive_initialized(&paths, &next_config, database_key)?;
    app_snapshot(database_key)
}

pub fn rekey_archive_database(
    old_key: Option<&str>,
    request: &RekeyRequest,
) -> Result<AppSnapshot> {
    let paths = project_paths()?;
    let config = load_config(&paths)?;
    rekey_archive(&paths, &config, old_key, request.new_mode.clone(), request.new_key.as_deref())?;
    let mut next_config = config;
    next_config.archive_mode = request.new_mode.clone();
    next_config.initialized = true;
    save_config(&paths, &next_config)?;
    app_snapshot(request.new_key.as_deref().or(old_key))
}

pub fn run_backup_now(
    session_database_key: Option<&str>,
    due_only: bool,
) -> Result<vault_core::BackupReport> {
    let paths = project_paths()?;
    let config = load_config(&paths)?;
    let mut report = run_backup(&paths, &config, session_database_key, due_only)?;
    if !report.due_skipped
        && config.remote_backup.enabled
        && config.remote_backup.upload_after_backup
    {
        match keyring_get_s3_credentials()? {
            Some(credentials) => {
                let remote = run_remote_backup(&paths, &config, session_database_key, &credentials)?;
                if remote.uploaded {
                    report.remote_backup = Some(remote);
                } else {
                    report.warnings.push(remote.message.clone());
                    report.remote_backup = Some(remote);
                }
            }
            None => report
                .warnings
                .push("Remote backup is enabled, but S3 credentials are not stored in the system keyring.".to_string()),
        }
    }
    if !report.due_skipped
        && config.ai.enabled
        && config.ai.semantic_index_enabled
        && config.ai.auto_index_after_backup
    {
        match selected_embedding_provider_runtime(&config, None) {
            Ok(provider) => {
                if let Err(error) = tokio_runtime()?.block_on(build_ai_index(
                    &paths,
                    &config,
                    session_database_key,
                    &provider,
                    &AiIndexRequest::default(),
                )) {
                    report.warnings.push(format!("AI index refresh after backup failed: {error}"));
                }
            }
            Err(error) => report.warnings.push(format!(
                "AI auto-index is enabled, but the embedding provider is not ready: {error}"
            )),
        }
    }
    Ok(report)
}

pub fn query_history(
    session_database_key: Option<&str>,
    query: HistoryQuery,
) -> Result<HistoryQueryResponse> {
    let paths = project_paths()?;
    let config = load_config(&paths)?;
    list_history(&paths, &config, session_database_key, query)
}

pub fn export_query(
    session_database_key: Option<&str>,
    request: ExportRequest,
) -> Result<vault_core::ExportResult> {
    let paths = project_paths()?;
    let config = load_config(&paths)?;
    export_history(&paths, &config, session_database_key, request)
}

pub fn preview_remote_backup_bundle() -> Result<RemoteBackupPreview> {
    let paths = project_paths()?;
    let mut config = load_config(&paths)?;
    hydrate_derived_config_state(&mut config);
    preview_remote_backup(&paths, &config)
}

pub fn upload_remote_backup_bundle(
    session_database_key: Option<&str>,
) -> Result<RemoteBackupResult> {
    let paths = project_paths()?;
    let mut config = load_config(&paths)?;
    hydrate_derived_config_state(&mut config);
    let credentials = keyring_get_s3_credentials()?
        .context("store S3 credentials in Settings before running a remote backup")?;
    run_remote_backup(&paths, &config, session_database_key, &credentials)
}

pub fn inspect_takeout_source(request: &TakeoutRequest) -> Result<TakeoutInspection> {
    let paths = project_paths()?;
    inspect_takeout(&paths, request)
}

pub fn import_takeout_source(
    session_database_key: Option<&str>,
    request: &TakeoutRequest,
) -> Result<TakeoutInspection> {
    let paths = project_paths()?;
    let config = load_config(&paths)?;
    import_takeout(&paths, &config, session_database_key, request)
}

pub fn preview_import_batch_detail(
    session_database_key: Option<&str>,
    batch_id: i64,
) -> Result<ImportBatchDetail> {
    let paths = project_paths()?;
    let config = load_config(&paths)?;
    preview_import_batch(&paths, &config, session_database_key, batch_id)
}

pub fn revert_import_batch_detail(
    session_database_key: Option<&str>,
    batch_id: i64,
) -> Result<ImportBatchDetail> {
    let paths = project_paths()?;
    let config = load_config(&paths)?;
    revert_import_batch(&paths, &config, session_database_key, batch_id)
}

pub fn doctor_report(session_database_key: Option<&str>) -> Result<HealthReport> {
    let paths = project_paths()?;
    let config = load_config(&paths)?;
    doctor(&paths, &config, session_database_key)
}

pub fn preview_schedule_plan(
    platform: Option<&str>,
    executable_path: Option<PathBuf>,
) -> Result<SchedulePlan> {
    let paths = project_paths()?;
    let config = load_config(&paths)?;
    let executable = executable_path
        .or_else(|| std::env::current_exe().ok())
        .context("resolving executable path for schedule preview")?;
    preview_schedule(
        platform,
        executable.as_path(),
        &paths,
        &ScheduleParameters {
            due_after_hours: config.due_after_hours,
            check_interval_hours: config.schedule_check_interval_hours,
        },
    )
}

pub fn apply_schedule_plan(plan: &SchedulePlan) -> Result<vault_core::ApplyResult> {
    let paths = project_paths()?;
    apply_schedule(plan, &paths)
}

pub fn read_database_key_from_keyring() -> Result<Option<String>> {
    keyring_get_database_key()
}

pub fn write_database_key_to_keyring(key: &str) -> Result<KeyringStatusReport> {
    keyring_set_database_key(key)?;
    Ok(keyring_status())
}

pub fn clear_database_key_from_keyring() -> Result<KeyringStatusReport> {
    keyring_clear_database_key()?;
    Ok(keyring_status())
}

pub fn reset_local_secret_vault() -> Result<()> {
    let paths = project_paths()?;
    if paths.stronghold_path.exists() {
        std::fs::remove_file(&paths.stronghold_path)
            .with_context(|| format!("removing {}", paths.stronghold_path.display()))?;
    }
    Ok(())
}

pub fn keyring_report() -> KeyringStatusReport {
    keyring_status()
}

pub fn store_s3_credentials(credentials: &S3CredentialInput) -> Result<()> {
    keyring_set_s3_credentials(credentials)
}

pub fn clear_s3_credentials() -> Result<()> {
    keyring_clear_s3_credentials()
}

pub fn store_ai_provider_api_key(
    input: &AiProviderSecretInput,
    session_database_key: Option<&str>,
) -> Result<AppSnapshot> {
    keyring_set_provider_api_key(&input.provider_id, &input.api_key)?;
    app_snapshot(session_database_key)
}

pub fn clear_ai_provider_api_key(
    provider_id: &str,
    session_database_key: Option<&str>,
) -> Result<AppSnapshot> {
    keyring_clear_provider_api_key(provider_id)?;
    app_snapshot(session_database_key)
}

pub fn build_ai_index_now(
    session_database_key: Option<&str>,
    request: &AiIndexRequest,
) -> Result<AiIndexReport> {
    let paths = project_paths()?;
    let mut config = load_config(&paths)?;
    hydrate_derived_config_state(&mut config);
    let provider = selected_embedding_provider_runtime(&config, request.provider_id.as_deref())?;
    tokio_runtime()?.block_on(build_ai_index(
        &paths,
        &config,
        session_database_key,
        &provider,
        request,
    ))
}

pub fn search_ai_history(
    session_database_key: Option<&str>,
    request: &AiSearchRequest,
) -> Result<AiSearchResponse> {
    let paths = project_paths()?;
    let mut config = load_config(&paths)?;
    hydrate_derived_config_state(&mut config);
    let embedding_provider = match selected_optional_embedding_runtime(&config) {
        Ok(provider) => provider,
        Err(error) => {
            let lexical = tokio_runtime()?.block_on(semantic_search_history(
                &paths,
                &config,
                session_database_key,
                None,
                request,
            ))?;
            return Ok(search_response_with_resolution_note(lexical, Some(error)));
        }
    };
    tokio_runtime()?.block_on(semantic_search_history(
        &paths,
        &config,
        session_database_key,
        embedding_provider.as_ref(),
        request,
    ))
}

pub fn ask_ai_assistant(
    session_database_key: Option<&str>,
    request: &AiAssistantRequest,
) -> Result<AiAssistantResponse> {
    let paths = project_paths()?;
    let mut config = load_config(&paths)?;
    hydrate_derived_config_state(&mut config);
    let llm_provider = selected_llm_provider_runtime(&config, None)?;
    let embedding_provider = selected_optional_embedding_runtime(&config).ok().flatten();
    tokio_runtime()?.block_on(answer_history_question(
        &paths,
        &config,
        session_database_key,
        &llm_provider,
        embedding_provider.as_ref(),
        request,
    ))
}

pub fn preview_ai_integration_files() -> Result<AiIntegrationPreview> {
    let paths = project_paths()?;
    let mut config = load_config(&paths)?;
    hydrate_derived_config_state(&mut config);
    preview_ai_integrations(&paths, &config)
}

#[tool_router]
impl BrowserHistoryMcpServer {
    #[tool(
        name = "search-history",
        description = "Search Browser History Backup for relevant visits, URLs, titles, profiles, or domains."
    )]
    async fn search_history(
        &self,
        Parameters(request): Parameters<McpSearchRequest>,
    ) -> Result<Json<McpSearchResult>, rmcp::ErrorData> {
        let response = search_ai_history(
            self.database_key.as_deref(),
            &AiSearchRequest {
                query: request.query,
                profile_id: request.profile_id,
                domain: request.domain,
                limit: request.limit,
            },
        )
        .map_err(|error| rmcp::ErrorData::internal_error(error.to_string(), None))?;
        Ok(Json(McpSearchResult {
            total: response.total,
            provider_id: response.provider_id,
            model: response.model,
            items: response
                .items
                .into_iter()
                .map(|item| McpSearchItem {
                    history_id: item.history_id,
                    profile_id: item.profile_id,
                    url: item.url,
                    title: item.title,
                    domain: item.domain,
                    visited_at: item.visited_at,
                    score: item.score,
                    match_reason: item.match_reason,
                })
                .collect(),
            notes: response.notes,
        }))
    }

    #[tool(
        name = "archive-status",
        description = "Report whether Browser History Backup is initialized, unlocked, and AI-ready."
    )]
    async fn archive_status(&self) -> Result<Json<McpArchiveStatus>, rmcp::ErrorData> {
        let snapshot = app_snapshot(self.database_key.as_deref())
            .map_err(|error| rmcp::ErrorData::internal_error(error.to_string(), None))?;
        Ok(Json(McpArchiveStatus {
            initialized: snapshot.archive_status.initialized,
            encrypted: snapshot.archive_status.encrypted,
            unlocked: snapshot.archive_status.unlocked,
            ai_enabled: snapshot.ai_status.enabled,
            assistant_enabled: snapshot.ai_status.assistant_enabled,
            semantic_index_enabled: snapshot.config.ai.semantic_index_enabled,
            indexed_items: snapshot.ai_status.indexed_items,
            warning: snapshot.ai_status.warning.or(snapshot.archive_status.warning),
        }))
    }
}

#[tool_handler]
impl ServerHandler for BrowserHistoryMcpServer {}

fn run_mcp_stdio_server() -> Result<()> {
    let paths = project_paths()?;
    let config = load_config(&paths)?;
    if !config.ai.enabled || !config.ai.mcp_enabled {
        anyhow::bail!(
            "Enable AI and the MCP server in Settings before starting the MCP server worker."
        );
    }

    let database_key = read_database_key_from_keyring()?;
    tokio_runtime()?.block_on(async move {
        let service =
            BrowserHistoryMcpServer::new(database_key).serve(rmcp::transport::io::stdio()).await?;
        service.waiting().await?;
        anyhow::Ok(())
    })
}

pub fn run_worker_cli(arguments: &[String]) -> Result<String> {
    let command = arguments.first().map(String::as_str).unwrap_or("snapshot");
    match command {
        "backup" => {
            let due_only = arguments.iter().any(|arg| arg == "--due-only");
            let key = read_database_key_from_keyring()?;
            let report = run_backup_now(key.as_deref(), due_only)?;
            Ok(serde_json::to_string_pretty(&report)?)
        }
        "doctor" => {
            let key = read_database_key_from_keyring()?;
            let report = doctor_report(key.as_deref())?;
            Ok(serde_json::to_string_pretty(&report)?)
        }
        "remote-backup" => {
            let key = read_database_key_from_keyring()?;
            let report = upload_remote_backup_bundle(key.as_deref())?;
            Ok(serde_json::to_string_pretty(&report)?)
        }
        "ai-index" => {
            let key = read_database_key_from_keyring()?;
            let report = build_ai_index_now(key.as_deref(), &AiIndexRequest::default())?;
            Ok(serde_json::to_string_pretty(&report)?)
        }
        "mcp-server" => {
            run_mcp_stdio_server()?;
            Ok(String::new())
        }
        other => anyhow::bail!("unknown worker command: {other}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::{Mutex, OnceLock};
    use tempfile::tempdir;
    use vault_core::{
        AiProviderConfig, AiProviderPurpose, AiRequestFormat, ArchiveMode, ExportFormat,
        TakeoutRequest, project_paths,
    };

    const PROJECT_ROOT_OVERRIDE_ENV: &str = "CHB_PROJECT_ROOT";
    const CHROME_USER_DATA_OVERRIDE_ENV: &str = "CHB_CHROME_USER_DATA_DIR";
    const TEST_KEYRING_OVERRIDE_ENV: &str = "CHB_TEST_KEYRING_DIR";

    fn env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    fn initialized_config() -> AppConfig {
        AppConfig {
            initialized: true,
            archive_mode: ArchiveMode::Plaintext,
            git_enabled: false,
            due_after_hours: 72,
            checkpoint_days: 1,
            selected_profile_ids: vec!["chrome:Default".to_string()],
            ..AppConfig::default()
        }
    }

    fn configured_ai_config() -> AppConfig {
        let mut config = initialized_config();
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
                 VALUES (1, 1, 'chrome history', 'chrome history')",
                [],
            )
            .expect("insert search term");

        chrome_root
    }

    fn takeout_fixture(root: &Path) -> String {
        let source_dir = root.join("takeout-source");
        fs::create_dir_all(&source_dir).expect("create takeout source");
        fs::write(
            source_dir.join("takeout.jsonl"),
            r#"{"url":"https://example.com/imported","title":"Imported","visitedAt":"2026-04-01T10:00:00+00:00"}"#,
        )
        .expect("write takeout jsonl");
        source_dir.display().to_string()
    }

    #[test]
    fn app_snapshot_and_worker_cli_cover_main_local_flows() {
        let _guard = env_lock().lock().expect("env lock");
        let dir = tempdir().expect("tempdir");
        let chrome_root = chrome_user_data_fixture(dir.path());
        let keyring_root = dir.path().join("test-keyring");

        unsafe {
            std::env::set_var(PROJECT_ROOT_OVERRIDE_ENV, dir.path());
            std::env::set_var(CHROME_USER_DATA_OVERRIDE_ENV, &chrome_root);
            std::env::set_var(TEST_KEYRING_OVERRIDE_ENV, &keyring_root);
        }

        let config = initialized_config();
        let snapshot = initialize_archive_database(&config, None).expect("initialize archive");
        assert!(snapshot.archive_status.initialized);
        assert_eq!(snapshot.browser_profiles.len(), 1);
        assert_eq!(snapshot.browser_profiles[0].profile_id, "chrome:Default");

        let backup_json = run_worker_cli(&["backup".to_string()]).expect("backup json");
        let backup: vault_core::BackupReport =
            serde_json::from_str(&backup_json).expect("parse backup report");
        assert_eq!(backup.run.expect("run").new_visits, 1);

        let doctor_json = run_worker_cli(&["doctor".to_string()]).expect("doctor json");
        let doctor: HealthReport = serde_json::from_str(&doctor_json).expect("parse doctor report");
        assert!(!doctor.checks.is_empty());

        let paths = project_paths().expect("project paths");
        assert!(paths.archive_database_path.exists());
    }

    #[test]
    fn worker_cli_rejects_unknown_commands() {
        let error = run_worker_cli(&["wat".to_string()]).expect_err("unknown command should fail");
        assert!(error.to_string().contains("unknown worker command"));
    }

    #[test]
    fn worker_cli_rejects_mcp_server_until_explicitly_enabled() {
        let _guard = env_lock().lock().expect("env lock");
        let dir = tempdir().expect("tempdir");
        let chrome_root = chrome_user_data_fixture(dir.path());
        let keyring_root = dir.path().join("test-keyring");

        unsafe {
            std::env::set_var(PROJECT_ROOT_OVERRIDE_ENV, dir.path());
            std::env::set_var(CHROME_USER_DATA_OVERRIDE_ENV, &chrome_root);
            std::env::set_var(TEST_KEYRING_OVERRIDE_ENV, &keyring_root);
        }

        let config = initialized_config();
        initialize_archive_database(&config, None).expect("initialize archive");

        let error =
            run_worker_cli(&["mcp-server".to_string()]).expect_err("mcp server should be gated");
        assert!(error.to_string().contains("Enable AI and the MCP server"));

        unsafe {
            std::env::remove_var(PROJECT_ROOT_OVERRIDE_ENV);
            std::env::remove_var(CHROME_USER_DATA_OVERRIDE_ENV);
            std::env::remove_var(TEST_KEYRING_OVERRIDE_ENV);
        }
    }

    #[test]
    fn ai_worker_helpers_cover_preview_secret_and_lexical_search_flows() {
        let _guard = env_lock().lock().expect("env lock");
        let dir = tempdir().expect("tempdir");
        let chrome_root = chrome_user_data_fixture(dir.path());
        let keyring_root = dir.path().join("test-keyring");

        unsafe {
            std::env::set_var(PROJECT_ROOT_OVERRIDE_ENV, dir.path());
            std::env::set_var(CHROME_USER_DATA_OVERRIDE_ENV, &chrome_root);
            std::env::set_var(TEST_KEYRING_OVERRIDE_ENV, &keyring_root);
        }

        let config = configured_ai_config();
        initialize_archive_database(&config, None).expect("initialize archive");
        let backup = run_backup_now(None, false).expect("backup");
        assert_eq!(backup.run.expect("run").new_visits, 1);

        let preview = preview_ai_integration_files().expect("preview integrations");
        assert!(preview.mcp_command.contains("mcp-server"));
        assert_eq!(preview.generated_files.len(), 2);

        let stored_snapshot = store_ai_provider_api_key(
            &AiProviderSecretInput {
                provider_id: "llm-primary".to_string(),
                api_key: "secret-1".to_string(),
            },
            None,
        )
        .expect("store llm key");
        assert!(stored_snapshot.config.ai.llm_providers[0].api_key_saved);

        let cleared_snapshot =
            clear_ai_provider_api_key("llm-primary", None).expect("clear llm key");
        assert!(!cleared_snapshot.config.ai.llm_providers[0].api_key_saved);

        let search = search_ai_history(
            None,
            &vault_core::AiSearchRequest {
                query: "example".to_string(),
                profile_id: None,
                domain: None,
                limit: Some(5),
            },
        )
        .expect("search history");
        assert_eq!(search.total, 1);
        assert!(!search.items.is_empty());
        assert!(!search.notes.is_empty());

        let index_error = build_ai_index_now(None, &AiIndexRequest::default())
            .expect_err("build index should require a saved key");
        assert!(index_error.to_string().contains("API key"));

        let assistant_error = ask_ai_assistant(
            None,
            &AiAssistantRequest {
                question: "What did I search?".to_string(),
                profile_id: None,
                domain: None,
            },
        )
        .expect_err("assistant should require a saved key");
        assert!(assistant_error.to_string().contains("API key"));

        unsafe {
            std::env::remove_var(PROJECT_ROOT_OVERRIDE_ENV);
            std::env::remove_var(CHROME_USER_DATA_OVERRIDE_ENV);
            std::env::remove_var(TEST_KEYRING_OVERRIDE_ENV);
        }
    }

    #[test]
    fn worker_support_helpers_cover_schedule_takeout_and_keyring_flows() {
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

        let mut config = initialized_config();
        config.remote_backup.enabled = true;
        config.remote_backup.bucket = "worker-tests".to_string();
        config.remote_backup.region = "us-west-2".to_string();
        config.remote_backup.prefix = "archives".to_string();
        initialize_archive_database(&config, None).expect("initialize archive");
        save_user_config(&config, None).expect("save user config");
        run_backup_now(None, false).expect("backup");

        let queried = query_history(
            None,
            HistoryQuery {
                q: Some("example".to_string()),
                profile_id: None,
                domain: None,
                limit: Some(10),
            },
        )
        .expect("query history");
        assert_eq!(queried.total, 1);

        let exported = export_query(
            None,
            ExportRequest { query: HistoryQuery::default(), format: ExportFormat::Text },
        )
        .expect("export history");
        assert_eq!(exported.count, 1);

        let preview = preview_schedule_plan(Some("windows"), Some(PathBuf::from("/tmp/bhb")))
            .expect("preview schedule");
        assert_eq!(preview.platform, "windows");
        let applied = apply_schedule_plan(&preview).expect("apply schedule");
        assert!(!applied.applied);

        let takeout_preview = inspect_takeout_source(&TakeoutRequest {
            source_path: takeout_source.clone(),
            dry_run: true,
        })
        .expect("inspect takeout");
        assert_eq!(takeout_preview.candidate_items, 1);

        let imported = import_takeout_source(
            None,
            &TakeoutRequest { source_path: takeout_source, dry_run: false },
        )
        .expect("import takeout");
        let batch_id = imported.import_batch.expect("import batch").id;
        assert_eq!(imported.imported_items, 1);
        let import_preview = preview_import_batch_detail(None, batch_id).expect("preview batch");
        assert_eq!(import_preview.batch.status, "imported");
        let reverted = revert_import_batch_detail(None, batch_id).expect("revert batch");
        assert_eq!(reverted.batch.status, "reverted");

        assert_eq!(read_database_key_from_keyring().expect("read empty db key"), None);
        let stored_report = write_database_key_to_keyring("db-secret").expect("store db key");
        assert!(stored_report.stored_secret);
        assert_eq!(
            read_database_key_from_keyring().expect("read db key"),
            Some("db-secret".to_string())
        );
        assert!(keyring_report().stored_secret);
        assert!(!clear_database_key_from_keyring().expect("clear db key").stored_secret);

        store_s3_credentials(&S3CredentialInput {
            access_key_id: "akid".to_string(),
            secret_access_key: "secret".to_string(),
        })
        .expect("store s3");
        let remote_preview = preview_remote_backup_bundle().expect("remote preview");
        assert!(remote_preview.upload_url.contains("worker-tests"));
        clear_s3_credentials().expect("clear s3");
        let remote_error =
            upload_remote_backup_bundle(None).expect_err("remote backup should fail");
        assert!(remote_error.to_string().contains("S3 credentials"));

        let paths = project_paths().expect("project paths");
        fs::write(&paths.stronghold_path, "hold").expect("write stronghold");
        reset_local_secret_vault().expect("reset local secret vault");
        assert!(!paths.stronghold_path.exists());

        unsafe {
            std::env::remove_var(PROJECT_ROOT_OVERRIDE_ENV);
            std::env::remove_var(CHROME_USER_DATA_OVERRIDE_ENV);
            std::env::remove_var(TEST_KEYRING_OVERRIDE_ENV);
        }
    }
}
