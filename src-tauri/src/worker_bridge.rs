use crate::{
    PRODUCT_DISPLAY_NAME,
    session::{SessionState, session_key, update_session_key},
};
use vault_core::{
    AiAssistantRequest, AiIndexRequest, AiProviderConnectionTestRequest, AiProviderSecretInput,
    AiSearchRequest, AppConfig, ExplainInsightRequest, ExportRequest, HistoryQuery,
    RunInsightsRequest, S3CredentialInput, SchedulePlan, SetAppLockPasscodeRequest, TakeoutRequest,
    UnlockAppSessionRequest,
};
use vault_worker::{self, RekeyRequest};

#[cfg(test)]
use crate::{entrypoint, run_with_arguments, write_payload};
#[cfg(test)]
use rusqlite::Connection;
#[cfg(test)]
use std::{
    fs,
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock},
};
#[cfg(test)]
use tempfile::tempdir;
#[cfg(test)]
use vault_core::{AiProviderConfig, AiProviderPurpose, AiRequestFormat, ArchiveMode};

fn worker_result<T, E: ToString>(result: Result<T, E>) -> Result<T, String> {
    result.map_err(|error| error.to_string())
}

pub(crate) fn app_build_info_impl() -> vault_core::AppBuildInfo {
    vault_core::AppBuildInfo {
        product_name: PRODUCT_DISPLAY_NAME.to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        git_commit_short: option_env!("BHB_GIT_COMMIT_SHORT").unwrap_or("unknown").to_string(),
        git_commit_full: option_env!("BHB_GIT_COMMIT_FULL").unwrap_or("unknown").to_string(),
        git_dirty: option_env!("BHB_GIT_DIRTY").unwrap_or("false") == "true",
    }
}

pub(crate) fn app_snapshot_impl(
    session_database_key: Option<&str>,
) -> Result<vault_core::AppSnapshot, String> {
    worker_result(vault_worker::app_snapshot(session_database_key))
}

pub(crate) fn save_config_impl(
    config: AppConfig,
    session_database_key: Option<&str>,
) -> Result<vault_core::AppSnapshot, String> {
    worker_result(vault_worker::save_user_config(&config, session_database_key))
}

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

pub(crate) fn preview_rekey_archive_impl(
    request: RekeyRequest,
    state: &SessionState,
) -> Result<vault_core::RekeyPreview, String> {
    worker_result(vault_worker::preview_rekey_archive(session_key(state).as_deref(), &request))
}

#[cfg_attr(test, allow(dead_code))]
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
pub(crate) fn run_snapshot_restore_impl(
    request: vault_core::SnapshotRestoreRequest,
    state: &SessionState,
) -> Result<vault_core::BackupReport, String> {
    worker_result(vault_worker::run_snapshot_restore_plan(session_key(state).as_deref(), &request))
}

#[cfg_attr(test, allow(dead_code))]
pub(crate) fn preview_retention_prune_impl(
    state: &SessionState,
) -> Result<vault_core::RetentionPreview, String> {
    worker_result(vault_worker::preview_retention_plan(session_key(state).as_deref()))
}

#[cfg_attr(test, allow(dead_code))]
pub(crate) fn run_retention_prune_impl(
    request: vault_core::RetentionPruneRequest,
    state: &SessionState,
) -> Result<vault_core::RetentionPruneResult, String> {
    worker_result(vault_worker::run_retention_plan(session_key(state).as_deref(), &request))
}

pub(crate) fn set_session_database_key_impl(
    database_key: String,
    state: &SessionState,
) -> Result<(), String> {
    update_session_key(state, Some(database_key))
}

pub(crate) fn clear_session_database_key_impl(state: &SessionState) -> Result<(), String> {
    update_session_key(state, None)
}

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

pub(crate) fn query_history_impl(
    query: HistoryQuery,
    session_database_key: Option<&str>,
) -> Result<vault_core::HistoryQueryResponse, String> {
    worker_result(vault_worker::query_history(session_database_key, query))
}

#[cfg_attr(test, allow(dead_code))]
pub(crate) fn dashboard_snapshot_impl(
    session_database_key: Option<&str>,
) -> Result<vault_core::DashboardSnapshot, String> {
    worker_result(vault_worker::dashboard_snapshot(session_database_key))
}

#[cfg_attr(test, allow(dead_code))]
pub(crate) fn audit_run_detail_impl(
    run_id: i64,
    session_database_key: Option<&str>,
) -> Result<vault_core::AuditRunDetail, String> {
    worker_result(vault_worker::audit_run_detail(session_database_key, run_id))
}

pub(crate) fn export_history_impl(
    request: ExportRequest,
    session_database_key: Option<&str>,
) -> Result<vault_core::ExportResult, String> {
    worker_result(vault_worker::export_query(session_database_key, request))
}

pub(crate) fn preview_remote_backup_impl() -> Result<vault_core::RemoteBackupPreview, String> {
    worker_result(vault_worker::preview_remote_backup_bundle())
}

pub(crate) fn run_remote_backup_impl(
    session_database_key: Option<&str>,
) -> Result<vault_core::RemoteBackupResult, String> {
    worker_result(vault_worker::upload_remote_backup_bundle(session_database_key))
}

#[cfg_attr(test, allow(dead_code))]
pub(crate) fn verify_remote_backup_impl(
    bundle_path: String,
    session_database_key: Option<&str>,
) -> Result<vault_core::RemoteBackupVerification, String> {
    worker_result(vault_worker::verify_remote_backup_bundle(session_database_key, &bundle_path))
}

pub(crate) fn inspect_takeout_impl(
    request: TakeoutRequest,
) -> Result<vault_core::TakeoutInspection, String> {
    worker_result(vault_worker::inspect_takeout_source(&request))
}

pub(crate) fn import_takeout_impl(
    request: TakeoutRequest,
    session_database_key: Option<&str>,
) -> Result<vault_core::TakeoutInspection, String> {
    worker_result(vault_worker::import_takeout_source(session_database_key, &request))
}

pub(crate) fn preview_import_batch_impl(
    batch_id: i64,
    session_database_key: Option<&str>,
) -> Result<vault_core::ImportBatchDetail, String> {
    worker_result(vault_worker::preview_import_batch_detail(session_database_key, batch_id))
}

pub(crate) fn revert_import_batch_impl(
    batch_id: i64,
    session_database_key: Option<&str>,
) -> Result<vault_core::ImportBatchDetail, String> {
    worker_result(vault_worker::revert_import_batch_detail(session_database_key, batch_id))
}

#[cfg_attr(test, allow(dead_code))]
pub(crate) fn restore_import_batch_impl(
    batch_id: i64,
    session_database_key: Option<&str>,
) -> Result<vault_core::ImportBatchDetail, String> {
    worker_result(vault_worker::restore_import_batch_detail(session_database_key, batch_id))
}

pub(crate) fn preview_schedule_impl(platform: Option<String>) -> Result<SchedulePlan, String> {
    worker_result(vault_worker::preview_schedule_plan(platform.as_deref(), None))
}

pub(crate) fn apply_schedule_impl(plan: SchedulePlan) -> Result<vault_core::ApplyResult, String> {
    worker_result(vault_worker::apply_schedule_plan(&plan))
}

#[cfg_attr(test, allow(dead_code))]
pub(crate) fn remove_schedule_impl(plan: SchedulePlan) -> Result<vault_core::ApplyResult, String> {
    worker_result(vault_worker::remove_schedule_plan(&plan))
}

pub(crate) fn schedule_status_impl(
    platform: Option<String>,
    session_database_key: Option<&str>,
) -> Result<vault_core::ScheduleStatus, String> {
    worker_result(vault_worker::schedule_status(session_database_key, platform.as_deref(), None))
}

pub(crate) fn doctor_report_impl(
    session_database_key: Option<&str>,
) -> Result<vault_core::HealthReport, String> {
    worker_result(vault_worker::doctor_report(session_database_key))
}

#[cfg_attr(test, allow(dead_code))]
pub(crate) fn repair_health_impl(
    session_database_key: Option<&str>,
) -> Result<vault_core::HealthRepairReport, String> {
    worker_result(vault_worker::repair_health(session_database_key))
}

#[cfg_attr(test, allow(dead_code))]
pub(crate) fn clear_derived_intelligence_impl(
    session_database_key: Option<&str>,
) -> Result<vault_core::ClearDerivedIntelligenceReport, String> {
    worker_result(vault_worker::clear_derived_intelligence(session_database_key))
}

pub(crate) fn keyring_status_impl() -> vault_core::KeyringStatusReport {
    vault_worker::keyring_report()
}

pub(crate) fn security_status_impl(
    session_database_key: Option<&str>,
) -> Result<vault_core::SecurityStatus, String> {
    worker_result(vault_worker::security_status(session_database_key))
}

#[cfg_attr(test, allow(dead_code))]
pub(crate) fn app_lock_status_impl() -> Result<vault_core::AppLockStatus, String> {
    worker_result(vault_worker::load_app_lock_status())
}

#[cfg_attr(test, allow(dead_code))]
pub(crate) fn set_app_lock_passcode_impl(
    request: SetAppLockPasscodeRequest,
) -> Result<vault_core::AppLockStatus, String> {
    worker_result(vault_worker::configure_app_lock_passcode(&request))
}

#[cfg_attr(test, allow(dead_code))]
pub(crate) fn clear_app_lock_passcode_impl() -> Result<vault_core::AppLockStatus, String> {
    worker_result(vault_worker::remove_app_lock_passcode())
}

#[cfg_attr(test, allow(dead_code))]
pub(crate) fn lock_app_session_impl(
    reason: Option<String>,
) -> Result<vault_core::AppLockStatus, String> {
    worker_result(vault_worker::lock_app_ui_session(reason.as_deref()))
}

#[cfg_attr(test, allow(dead_code))]
pub(crate) fn unlock_app_session_impl(
    request: UnlockAppSessionRequest,
) -> Result<vault_core::AppLockStatus, String> {
    worker_result(vault_worker::unlock_app_ui_session(&request))
}

pub(crate) fn keyring_get_database_key_impl() -> Result<Option<String>, String> {
    worker_result(vault_worker::read_database_key_from_keyring())
}

pub(crate) fn keyring_store_database_key_impl(
    value: String,
) -> Result<vault_core::KeyringStatusReport, String> {
    worker_result(vault_worker::write_database_key_to_keyring(&value))
}

pub(crate) fn keyring_clear_database_key_impl() -> Result<vault_core::KeyringStatusReport, String> {
    worker_result(vault_worker::clear_database_key_from_keyring())
}

pub(crate) fn store_s3_credentials_impl(credentials: S3CredentialInput) -> Result<(), String> {
    worker_result(vault_worker::store_s3_credentials(&credentials))
}

pub(crate) fn clear_s3_credentials_impl() -> Result<(), String> {
    worker_result(vault_worker::clear_s3_credentials())
}

pub(crate) fn store_ai_provider_api_key_impl(
    input: AiProviderSecretInput,
    session_database_key: Option<&str>,
) -> Result<vault_core::AppSnapshot, String> {
    worker_result(vault_worker::store_ai_provider_api_key(&input, session_database_key))
}

pub(crate) fn clear_ai_provider_api_key_impl(
    provider_id: String,
    session_database_key: Option<&str>,
) -> Result<vault_core::AppSnapshot, String> {
    worker_result(vault_worker::clear_ai_provider_api_key(&provider_id, session_database_key))
}

#[cfg_attr(test, allow(dead_code))]
pub(crate) fn test_ai_provider_connection_impl(
    request: AiProviderConnectionTestRequest,
    session_database_key: Option<&str>,
) -> Result<vault_core::AiProviderConnectionTestReport, String> {
    worker_result(vault_worker::test_ai_provider_connection_report(session_database_key, &request))
}

#[cfg_attr(test, allow(dead_code))]
pub(crate) fn load_ai_queue_status_impl(
    session_database_key: Option<&str>,
) -> Result<vault_core::AiQueueStatus, String> {
    worker_result(vault_worker::load_ai_queue(session_database_key))
}

#[cfg_attr(test, allow(dead_code))]
pub(crate) fn run_ai_queue_jobs_impl(
    max_jobs: Option<u32>,
    session_database_key: Option<&str>,
) -> Result<vault_core::AiQueueStatus, String> {
    worker_result(vault_worker::run_ai_queue_jobs(session_database_key, max_jobs))
}

#[cfg_attr(test, allow(dead_code))]
pub(crate) fn replay_ai_job_impl(
    job_id: i64,
    session_database_key: Option<&str>,
) -> Result<vault_core::AiQueueJob, String> {
    worker_result(vault_worker::replay_ai_job(session_database_key, job_id))
}

#[cfg_attr(test, allow(dead_code))]
pub(crate) fn cancel_ai_job_impl(
    job_id: i64,
    session_database_key: Option<&str>,
) -> Result<vault_core::AiQueueJob, String> {
    worker_result(vault_worker::cancel_ai_job(session_database_key, job_id))
}

#[cfg_attr(test, allow(dead_code))]
pub(crate) fn load_ai_assistant_job_impl(
    job_id: i64,
    session_database_key: Option<&str>,
) -> Result<vault_core::AiAssistantResponse, String> {
    worker_result(vault_worker::load_ai_assistant_job(session_database_key, job_id))
}

pub(crate) fn build_ai_index_impl(
    request: AiIndexRequest,
    session_database_key: Option<&str>,
) -> Result<vault_core::AiIndexReport, String> {
    worker_result(vault_worker::build_ai_index_now(session_database_key, &request))
}

pub(crate) fn search_ai_history_impl(
    request: AiSearchRequest,
    session_database_key: Option<&str>,
) -> Result<vault_core::AiSearchResponse, String> {
    worker_result(vault_worker::search_ai_history(session_database_key, &request))
}

pub(crate) fn ask_ai_assistant_impl(
    request: AiAssistantRequest,
    session_database_key: Option<&str>,
) -> Result<vault_core::AiAssistantResponse, String> {
    worker_result(vault_worker::ask_ai_assistant(session_database_key, &request))
}

pub(crate) fn preview_ai_integrations_impl() -> Result<vault_core::AiIntegrationPreview, String> {
    worker_result(vault_worker::preview_ai_integration_files())
}

#[cfg_attr(test, allow(dead_code))]
pub(crate) fn run_insights_now_impl(
    request: RunInsightsRequest,
    session_database_key: Option<&str>,
) -> Result<vault_core::RunInsightsReport, String> {
    worker_result(vault_worker::run_insights_now(session_database_key, &request))
}

#[cfg_attr(test, allow(dead_code))]
pub(crate) fn load_insights_impl(
    request: RunInsightsRequest,
    session_database_key: Option<&str>,
) -> Result<vault_core::InsightSnapshot, String> {
    worker_result(vault_worker::load_insights_snapshot(session_database_key, &request))
}

#[cfg_attr(test, allow(dead_code))]
pub(crate) fn load_thread_detail_impl(
    thread_id: String,
    session_database_key: Option<&str>,
) -> Result<vault_core::InsightThreadDetail, String> {
    worker_result(vault_worker::load_insight_thread(session_database_key, &thread_id))
}

#[cfg_attr(test, allow(dead_code))]
pub(crate) fn explain_insight_impl(
    request: ExplainInsightRequest,
    session_database_key: Option<&str>,
) -> Result<vault_core::InsightExplanation, String> {
    worker_result(vault_worker::explain_insight_now(session_database_key, &request))
}

#[cfg_attr(test, allow(dead_code))]
pub(crate) fn load_intelligence_runtime_impl(
    session_database_key: Option<&str>,
) -> Result<vault_core::IntelligenceRuntimeSnapshot, String> {
    worker_result(vault_worker::load_intelligence_runtime_snapshot(session_database_key))
}

#[cfg_attr(test, allow(dead_code))]
pub(crate) fn retry_intelligence_job_impl(
    job_id: i64,
    session_database_key: Option<&str>,
) -> Result<vault_core::IntelligenceRuntimeSnapshot, String> {
    worker_result(vault_worker::retry_intelligence_job_now(session_database_key, job_id))
}

#[cfg_attr(test, allow(dead_code))]
pub(crate) fn cancel_intelligence_job_impl(
    job_id: i64,
    session_database_key: Option<&str>,
) -> Result<vault_core::IntelligenceRuntimeSnapshot, String> {
    worker_result(vault_worker::cancel_intelligence_job_now(session_database_key, job_id))
}

pub(crate) fn reset_local_secret_vault_impl() -> Result<(), String> {
    worker_result(vault_worker::reset_local_secret_vault())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::MutexGuard;

    const PROJECT_ROOT_OVERRIDE_ENV: &str = "CHB_PROJECT_ROOT";
    const CHROME_USER_DATA_OVERRIDE_ENV: &str = "CHB_CHROME_USER_DATA_DIR";
    const TEST_KEYRING_OVERRIDE_ENV: &str = "CHB_TEST_KEYRING_DIR";

    fn env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    fn lock_env() -> MutexGuard<'static, ()> {
        env_lock().lock().unwrap_or_else(|poisoned| poisoned.into_inner())
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
        let now_chrome_micros = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("current time")
            .as_micros() as i64
            + 11_644_473_600_000_000i64;
        fs::create_dir_all(&profile_dir).expect("create chrome profile dir");
        fs::write(chrome_root.join("Last Version"), "135.0.0.0").expect("write version");
        fs::write(
            chrome_root.join("Local State"),
            r#"{"profile":{"info_cache":{"Default":{"name":"Default","user_name":"fixture@example.test"}}}}"#,
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
                 VALUES (1, 'https://www.google.com/search?q=example', 'example - Google Search', 1, 1, ?1, 0)",
                [now_chrome_micros],
            )
            .expect("insert url");
        history
            .execute(
                "INSERT INTO visits (id, url, visit_time, from_visit, transition, visit_duration, is_known_to_sync, visited_link_id, external_referrer_url, app_id)
                 VALUES (1, 1, ?1, NULL, 805306368, 24000, 1, 3, 'https://ref.example', 'com.example.app')",
                [now_chrome_micros],
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
        let _guard = lock_env();
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

        set_session_database_key_impl("transient-session-key".to_string(), &session)
            .expect("set transient session key");
        assert_eq!(session_key(&session), Some("transient-session-key".to_string()));
        clear_session_database_key_impl(&session).expect("clear transient session key");
        assert_eq!(session_key(&session), None);

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

        let backup =
            run_backup_now_impl(false, session_key(&session).as_deref(), |_| {}).expect("backup");
        assert_eq!(backup.run.expect("run").new_visits, 1);
        let backup_run_id = app_snapshot_impl(session_key(&session).as_deref())
            .expect("snapshot after backup")
            .recent_runs[0]
            .id;
        let backup_detail = audit_run_detail_impl(backup_run_id, session_key(&session).as_deref())
            .expect("backup audit detail");
        let snapshot_path = backup_detail.artifacts[0].path.clone();
        let restore_preview = preview_snapshot_restore_impl(
            vault_core::SnapshotRestoreRequest { snapshot_path: snapshot_path.clone() },
            &session,
        )
        .expect("preview snapshot restore");
        assert!(restore_preview.execute_supported);
        let restore_run = run_snapshot_restore_impl(
            vault_core::SnapshotRestoreRequest { snapshot_path },
            &session,
        )
        .expect("run snapshot restore");
        assert_eq!(restore_run.run.expect("snapshot restore run").run_type, "snapshot_restore");
        let retention_preview =
            preview_retention_prune_impl(&session).expect("preview retention prune");
        assert!(retention_preview.buckets.iter().any(|bucket| bucket.id == "snapshots"));
        let retention_result = run_retention_prune_impl(
            vault_core::RetentionPruneRequest { bucket_ids: vec!["snapshots".to_string()] },
            &session,
        )
        .expect("run retention prune");
        assert!(retention_result.deleted_files > 0);

        let history = query_history_impl(
            HistoryQuery {
                q: Some("example".to_string()),
                limit: Some(10),
                ..HistoryQuery::default()
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
        let restored_batch = restore_import_batch_impl(batch_id, session_key(&session).as_deref())
            .expect("restore import batch");
        assert_eq!(restored_batch.batch.status, "imported");
        assert_eq!(restored_batch.batch.visible_items, 1);

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
        remote_config.remote_backup.bucket = "pathkeep-tests".to_string();
        remote_config.remote_backup.region = "us-west-2".to_string();
        remote_config.remote_backup.prefix = "archives".to_string();
        let saved_snapshot =
            save_config_impl(remote_config.clone(), session_key(&session).as_deref())
                .expect("save remote config");
        assert!(saved_snapshot.config.remote_backup.credentials_saved);

        let remote_preview = preview_remote_backup_impl().expect("preview remote backup");
        assert!(remote_preview.preview_command.contains("curl"));
        let remote_verify_error = verify_remote_backup_impl(
            "/tmp/pathkeep-missing-bundle.zip".to_string(),
            session_key(&session).as_deref(),
        )
        .expect_err("missing bundle should fail verification");
        assert!(
            remote_verify_error.contains("opening")
                && remote_verify_error.contains("pathkeep-missing-bundle.zip")
        );
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
            AiIndexRequest {
                provider_id: None,
                full_rebuild: false,
                clear_only: false,
                limit: Some(5),
            },
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
                cursor: None,
            },
            session_key(&session).as_deref(),
        )
        .expect("ai search");
        assert_eq!(ai_search.total, 2);

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
        let removed = remove_schedule_impl(
            preview_schedule_impl(Some("linux".to_string())).expect("schedule preview for remove"),
        )
        .expect("remove schedule");
        assert!(!removed.applied);
        let schedule_status =
            schedule_status_impl(Some("linux".to_string()), session_key(&session).as_deref())
                .expect("schedule status");
        assert_eq!(schedule_status.install_state, "manual-review");

        let doctor = doctor_report_impl(session_key(&session).as_deref()).expect("doctor");
        assert!(!doctor.checks.is_empty());
        let rekey_preview = preview_rekey_archive_impl(
            RekeyRequest { new_mode: ArchiveMode::Encrypted, new_key: None },
            &session,
        )
        .expect("preview rekey archive");
        assert!(rekey_preview.requires_new_key);
        let rekeyed_snapshot = rekey_archive_impl(
            RekeyRequest {
                new_mode: ArchiveMode::Encrypted,
                new_key: Some("vault-passphrase".to_string()),
            },
            &session,
        )
        .expect("rekey archive");
        assert!(rekeyed_snapshot.archive_status.encrypted);
        let cleared_derived = clear_derived_intelligence_impl(session_key(&session).as_deref())
            .expect("clear derived intelligence");
        assert!(!cleared_derived.notes.is_empty());

        let snapshot_again =
            app_snapshot_impl(session_key(&session).as_deref()).expect("app snapshot");
        assert_eq!(snapshot_again.browser_profiles.len(), 1);
        assert!(snapshot_again.archive_status.encrypted);
        let security_status =
            security_status_impl(session_key(&session).as_deref()).expect("security status");
        assert_eq!(security_status.mode, "encrypted");
        fs::write(dir.path().join("vault.hold"), "secret vault").expect("write stronghold fixture");
        reset_local_secret_vault_impl().expect("reset local secret vault");
        assert!(!dir.path().join("vault.hold").exists());
        let worker_payload = run_with_arguments(&[
            "pathkeep".to_string(),
            "--worker".to_string(),
            "doctor".to_string(),
        ])
        .expect("run worker doctor")
        .expect("worker payload");
        assert!(worker_payload.contains("checks"));
        assert!(run_with_arguments(&["pathkeep".to_string()]).is_ok());

        unsafe {
            std::env::remove_var(PROJECT_ROOT_OVERRIDE_ENV);
            std::env::remove_var(CHROME_USER_DATA_OVERRIDE_ENV);
            std::env::remove_var(TEST_KEYRING_OVERRIDE_ENV);
        }
    }

    #[test]
    fn worker_bridge_covers_dashboard_audit_and_ai_wrapper_edges() {
        let _guard = lock_env();
        let dir = tempdir().expect("tempdir");
        let chrome_root = chrome_user_data_fixture(dir.path());
        let keyring_root = dir.path().join("test-keyring");

        unsafe {
            std::env::set_var(PROJECT_ROOT_OVERRIDE_ENV, dir.path());
            std::env::set_var(CHROME_USER_DATA_OVERRIDE_ENV, &chrome_root);
            std::env::set_var(TEST_KEYRING_OVERRIDE_ENV, &keyring_root);
        }

        let session = SessionState::default();
        let config = initialized_config();
        initialize_archive_impl(config.clone(), Some("vault-passphrase".to_string()), &session)
            .expect("initialize archive");
        save_config_impl(config, session_key(&session).as_deref()).expect("save config");
        let report = run_backup_now_impl(false, session_key(&session).as_deref(), |_| {})
            .expect("run backup");
        let run_id = report.run.expect("backup run").id;

        let dashboard =
            dashboard_snapshot_impl(session_key(&session).as_deref()).expect("dashboard snapshot");
        assert_eq!(dashboard.total_visits, 1);
        assert!(!dashboard.recent_runs.is_empty());

        let detail = audit_run_detail_impl(run_id, session_key(&session).as_deref())
            .expect("audit run detail");
        assert_eq!(detail.run.id, run_id);

        let repair =
            repair_health_impl(session_key(&session).as_deref()).expect("repair health report");
        assert!(repair.run_id.is_none() || repair.run_id > Some(run_id));

        let connection_probe = test_ai_provider_connection_impl(
            AiProviderConnectionTestRequest {
                provider_id: "embed-primary".to_string(),
                purpose: AiProviderPurpose::Embedding,
            },
            session_key(&session).as_deref(),
        )
        .expect("missing provider key should surface in the report");
        assert!(!connection_probe.ok);
        assert_eq!(connection_probe.error_code.as_deref(), Some("secret-missing"));

        let queue = load_ai_queue_status_impl(session_key(&session).as_deref())
            .expect("load ai queue status");
        assert!(queue.recent_jobs.is_empty());
        let drained = run_ai_queue_jobs_impl(None, session_key(&session).as_deref())
            .expect("run empty ai queue");
        assert!(drained.recent_jobs.is_empty());

        let replay = replay_ai_job_impl(999, session_key(&session).as_deref())
            .expect_err("missing ai job should not replay");
        assert!(replay.contains("999"));
        let cancel = cancel_ai_job_impl(999, session_key(&session).as_deref())
            .expect_err("missing ai job should not cancel");
        assert!(cancel.contains("999"));
        let assistant_job = load_ai_assistant_job_impl(999, session_key(&session).as_deref())
            .expect_err("missing assistant job should not load");
        assert!(assistant_job.contains("999"));

        let insights_run =
            run_insights_now_impl(RunInsightsRequest::default(), session_key(&session).as_deref())
                .expect("insights run should fall back when no embedding provider secret is ready");
        assert!(!insights_run.last_run_at.is_empty());
        assert!(insights_run.notes.iter().any(|note| note.contains("fell back to lexical")));
        let insights_snapshot =
            load_insights_impl(RunInsightsRequest::default(), session_key(&session).as_deref())
                .expect("insight snapshot should load after the fallback run");
        assert!(insights_snapshot.status.runs >= 1);
        assert!(
            !insights_snapshot.cards.is_empty()
                || !insights_snapshot.template_summaries.is_empty()
                || !insights_snapshot.query_groups.is_empty()
                || !insights_snapshot.threads.is_empty()
        );
        assert!(insights_snapshot.notes.iter().any(|note| note.contains("fell back to lexical")));
        if let Some(thread) = insights_snapshot.threads.first() {
            let detail =
                load_thread_detail_impl(thread.thread_id.clone(), session_key(&session).as_deref())
                    .expect("thread detail should load when deterministic threads exist");
            assert_eq!(detail.summary.thread_id, thread.thread_id);
        } else {
            let thread_detail =
                load_thread_detail_impl("thread-001".to_string(), session_key(&session).as_deref())
                    .expect_err("thread detail should not load without deterministic threads");
            assert!(!thread_detail.is_empty());
        }
        let (insight_id, insight_kind) = if let Some(card) = insights_snapshot.cards.first() {
            (card.card_id.clone(), "card".to_string())
        } else if let Some(summary) = insights_snapshot.template_summaries.first() {
            (summary.summary_id.clone(), "template-summary".to_string())
        } else if let Some(group) = insights_snapshot.query_groups.first() {
            (group.query_group_id.clone(), "query-group".to_string())
        } else if let Some(reference_page) = insights_snapshot.reference_pages.first() {
            (reference_page.reference_page_id.clone(), "reference-page".to_string())
        } else if let Some(thread) = insights_snapshot.threads.first() {
            (thread.thread_id.clone(), "thread".to_string())
        } else {
            let summary = insights_snapshot.topics.first().expect(
                "fallback run should persist at least one explainable deterministic surface",
            );
            (summary.topic_id.clone(), "topic".to_string())
        };
        let explanation = explain_insight_impl(
            ExplainInsightRequest {
                insight_id,
                insight_kind,
                profile_id: None,
                window_days: Some(30),
            },
            session_key(&session).as_deref(),
        )
        .expect("insight explain should work from the persisted deterministic surface");
        assert!(!explanation.explanation.is_empty());
        assert!(!explanation.used_llm);

        unsafe {
            std::env::remove_var(PROJECT_ROOT_OVERRIDE_ENV);
            std::env::remove_var(CHROME_USER_DATA_OVERRIDE_ENV);
            std::env::remove_var(TEST_KEYRING_OVERRIDE_ENV);
        }
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
    fn app_lock_bridge_guards_desktop_read_models_until_unlock() {
        let _guard = lock_env();
        let dir = tempdir().expect("tempdir");
        let chrome_root = chrome_user_data_fixture(dir.path());
        let keyring_root = dir.path().join("test-keyring");

        unsafe {
            std::env::set_var(PROJECT_ROOT_OVERRIDE_ENV, dir.path());
            std::env::set_var(CHROME_USER_DATA_OVERRIDE_ENV, &chrome_root);
            std::env::set_var(TEST_KEYRING_OVERRIDE_ENV, &keyring_root);
        }

        let session = SessionState::default();
        let mut config = initialized_config();
        initialize_archive_impl(config.clone(), None, &session).expect("initialize archive");
        run_backup_now_impl(false, session_key(&session).as_deref(), |_| {}).expect("backup");

        let passcode_status = set_app_lock_passcode_impl(SetAppLockPasscodeRequest {
            passcode: "2468".to_string(),
            recovery_hint: Some("desk drawer".to_string()),
        })
        .expect("set app lock passcode");
        assert!(passcode_status.passcode_configured);
        let app_lock_status = app_lock_status_impl().expect("load app lock status");
        assert!(app_lock_status.passcode_configured);

        config.app_lock.enabled = true;
        let saved_snapshot =
            save_config_impl(config, session_key(&session).as_deref()).expect("enable app lock");
        assert!(saved_snapshot.config.app_lock.enabled);

        let locked = lock_app_session_impl(Some("manual".to_string())).expect("lock app session");
        assert!(locked.locked);
        assert_eq!(locked.lock_reason.as_deref(), Some("manual"));

        let snapshot_error =
            app_snapshot_impl(session_key(&session).as_deref()).expect_err("snapshot should block");
        assert!(snapshot_error.contains("currently locked"));

        let dashboard_error = dashboard_snapshot_impl(session_key(&session).as_deref())
            .expect_err("dashboard should block");
        assert!(dashboard_error.contains("currently locked"));

        let unlock_error = unlock_app_session_impl(UnlockAppSessionRequest {
            passcode: Some("9999".to_string()),
            use_biometric: false,
        })
        .expect_err("wrong passcode should fail");
        assert!(unlock_error.contains("did not match"));

        let unlocked = unlock_app_session_impl(UnlockAppSessionRequest {
            passcode: Some("2468".to_string()),
            use_biometric: false,
        })
        .expect("unlock app session");
        assert!(!unlocked.locked);

        let snapshot =
            app_snapshot_impl(session_key(&session).as_deref()).expect("snapshot after unlock");
        assert!(snapshot.archive_status.initialized);
        let dashboard =
            dashboard_snapshot_impl(session_key(&session).as_deref()).expect("dashboard");
        assert!(!dashboard.recent_runs.is_empty());
        let cleared = clear_app_lock_passcode_impl().expect("clear app lock passcode");
        assert!(!cleared.enabled);
        assert!(!cleared.passcode_configured);

        unsafe {
            std::env::remove_var(PROJECT_ROOT_OVERRIDE_ENV);
            std::env::remove_var(CHROME_USER_DATA_OVERRIDE_ENV);
            std::env::remove_var(TEST_KEYRING_OVERRIDE_ENV);
        }
    }

    #[test]
    fn entrypoint_and_payload_writer_cover_stdout_paths() {
        entrypoint().expect("entrypoint");

        let mut output = Vec::new();
        write_payload(&mut output, Some("worker-payload".to_string())).expect("write payload");
        write_payload(&mut output, None).expect("write empty payload");
        assert_eq!(String::from_utf8(output).expect("utf8"), "worker-payload\n");
    }
}
