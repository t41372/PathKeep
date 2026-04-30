//! Regression tests for the worker orchestration facade.
use super::*;
#[cfg(coverage)]
use crate::intelligence::{
    drain_one_ai_queue_job, drain_one_enrichment_intelligence_job,
    drain_one_priority_intelligence_job,
};
use rusqlite::Connection;
use std::fs;
#[cfg(coverage)]
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, OnceLock};
use tempfile::tempdir;
#[cfg(coverage)]
use vault_core::AiQueueJobType;
#[cfg(coverage)]
use vault_core::TITLE_NORMALIZATION_PLUGIN_ID;
#[cfg(coverage)]
use vault_core::intelligence_runtime::{
    DAILY_ROLLUP_JOB_TYPE, FULL_REBUILD_JOB_TYPE, STRUCTURAL_REBUILD_JOB_TYPE,
    VISIT_DERIVE_JOB_TYPE, enqueue_core_intelligence_job,
};
use vault_core::{
    AiAssistantRequest, AiIndexRequest, AiProviderConfig, AiProviderPurpose, AiProviderSecretInput,
    AiRequestFormat, AiSearchResponse, AppConfig, ArchiveMode, BrowserHistoryImportRequest,
    ExportFormat, ExportRequest, HealthReport, HistoryQuery, PagedDateRangeRequest,
    S3CredentialInput, SetAppLockPasscodeRequest, TakeoutRequest, UnlockAppSessionRequest,
    project_paths, utils::iso_to_chrome_time_micros,
};
#[cfg(coverage)]
use vault_core::{
    AiIndexReport, AiProviderConnectionTestRequest, AiQueueStatus, AiRunControl, AiSearchRequest,
    CoreIntelligenceRebuildRequest, RemoteBackupResult,
};
use vault_platform::keyring_set_provider_api_key;

const PROJECT_ROOT_OVERRIDE_ENV: &str = "CHB_PROJECT_ROOT";
const CHROME_USER_DATA_OVERRIDE_ENV: &str = "CHB_CHROME_USER_DATA_DIR";
const FIREFOX_PROFILES_OVERRIDE_ENV: &str = "CHB_FIREFOX_PROFILES_DIR";
const SAFARI_ROOT_OVERRIDE_ENV: &str = "CHB_SAFARI_ROOT";
const TEST_KEYRING_OVERRIDE_ENV: &str = "CHB_TEST_KEYRING_DIR";

fn env_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

pub(crate) fn lock_env() -> MutexGuard<'static, ()> {
    env_lock().lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn restore_env_var(name: &str, value: Option<&std::ffi::OsStr>) {
    unsafe {
        if let Some(value) = value {
            std::env::set_var(name, value);
        } else {
            std::env::remove_var(name);
        }
    }
}

fn block_on_ready<F: std::future::Future>(future: F) -> F::Output {
    use std::{
        pin::pin,
        ptr,
        task::{Context, Poll, RawWaker, RawWakerVTable, Waker},
    };

    fn no_op(_: *const ()) {}
    fn clone(_: *const ()) -> RawWaker {
        RawWaker::new(ptr::null(), &VTABLE)
    }
    static VTABLE: RawWakerVTable = RawWakerVTable::new(clone, no_op, no_op, no_op);

    let waker = unsafe { Waker::from_raw(RawWaker::new(ptr::null(), &VTABLE)) };
    let mut context = Context::from_waker(&waker);
    let mut future = pin!(future);
    match future.as_mut().poll(&mut context) {
        Poll::Ready(value) => value,
        Poll::Pending => panic!("expected worker future to be immediately ready"),
    }
}

#[test]
fn block_on_ready_covers_clone_path() {
    struct CloneWakerFuture;

    impl std::future::Future for CloneWakerFuture {
        type Output = usize;

        fn poll(
            self: std::pin::Pin<&mut Self>,
            cx: &mut std::task::Context<'_>,
        ) -> std::task::Poll<Self::Output> {
            let _ = cx.waker().clone();
            std::task::Poll::Ready(7)
        }
    }

    assert_eq!(block_on_ready(CloneWakerFuture), 7);
}

#[test]
#[should_panic(expected = "expected worker future to be immediately ready")]
fn block_on_ready_panics_for_pending_futures() {
    block_on_ready(std::future::pending::<()>());
}

#[test]
fn restore_env_var_sets_and_clears_values() {
    let _guard = lock_env();
    let value = std::ffi::OsString::from("worker-fixture");
    restore_env_var(PROJECT_ROOT_OVERRIDE_ENV, Some(value.as_os_str()));
    assert_eq!(std::env::var_os(PROJECT_ROOT_OVERRIDE_ENV), Some(value));
    restore_env_var(PROJECT_ROOT_OVERRIDE_ENV, None);
    assert!(std::env::var_os(PROJECT_ROOT_OVERRIDE_ENV).is_none());
}

#[test]
fn queue_failure_classifier_preserves_retry_and_manual_review_semantics() {
    let cases = [
        ("provider returned 429 rate limit", "rate-limited", true, 300),
        ("DNS network refused", "network-error", true, 30),
        ("store an API key before use", "secret-missing", false, 0),
        ("model not found", "bad-model", false, 0),
        ("enable provider in Settings", "provider-disabled", false, 0),
        ("not configured for embedding", "unsupported-capability", false, 0),
        ("unexpected provider failure", "provider-error", false, 0),
    ];

    for (message, expected_code, retryable, retry_after_seconds) in cases {
        let failure = crate::context::queue_failure_from_error(&anyhow::anyhow!(message));

        assert_eq!(failure.error_code.as_deref(), Some(expected_code), "{message}");
        assert_eq!(failure.retryable, retryable, "{message}");
        assert_eq!(failure.retry_after_seconds, retry_after_seconds, "{message}");
        assert_eq!(failure.error_message, message);
        assert!(failure.summary.as_deref().is_some_and(|summary| !summary.is_empty()));
    }
}

fn initialized_config() -> AppConfig {
    AppConfig {
        initialized: true,
        archive_mode: ArchiveMode::Plaintext,
        git_enabled: false,
        due_after_hours: 72.0,
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
    let recent_visit = chrono::Utc::now().to_rfc3339();
    let recent_visit_time = iso_to_chrome_time_micros(&recent_visit).expect("chrome time");
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
             VALUES (1, 'https://example.com', 'Example', 1, 1, ?1, 0)",
            [recent_visit_time],
        )
        .expect("insert url");
    history
        .execute(
            "INSERT INTO visits (id, url, visit_time, from_visit, transition, visit_duration, is_known_to_sync, visited_link_id, external_referrer_url, app_id)
             VALUES (1, 1, ?1, NULL, 805306368, 24000, 1, 3, 'https://ref.example', 'com.example.app')",
            [recent_visit_time],
        )
        .expect("insert visit");
    history
        .execute(
            "INSERT INTO downloads (id, guid, current_path, target_path, start_time, received_bytes, total_bytes, state, mime_type, original_mime_type)
             VALUES (1, 'guid-1', '/tmp/current', '/tmp/target', ?1, 1, 2, 3, 'text/html', 'text/plain')",
            [recent_visit_time],
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

fn broken_chrome_user_data_fixture(root: &Path) -> PathBuf {
    let chrome_root = root.join("broken-chrome-user-data");
    fs::create_dir_all(&chrome_root).expect("create broken chrome root");
    fs::write(chrome_root.join("Last Version"), "135.0.0.0").expect("write version");
    fs::write(chrome_root.join("Local State"), "{not-json").expect("write broken local state");
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

#[cfg(coverage)]
fn install_fake_curl(bin_dir: &Path, body: &str) -> PathBuf {
    let script_path = bin_dir.join("curl");
    fs::create_dir_all(bin_dir).expect("create fake curl dir");
    fs::write(&script_path, body).expect("write fake curl");
    let mut permissions = fs::metadata(&script_path).expect("metadata").permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&script_path, permissions).expect("chmod");
    script_path
}

#[test]
fn app_snapshot_and_worker_cli_cover_main_local_flows() {
    let _guard = lock_env();
    let dir = tempdir().expect("tempdir");
    let chrome_root = chrome_user_data_fixture(dir.path());
    let keyring_root = dir.path().join("test-keyring");
    let original_project_root = std::env::var_os(PROJECT_ROOT_OVERRIDE_ENV);
    let original_chrome_root = std::env::var_os(CHROME_USER_DATA_OVERRIDE_ENV);
    let original_firefox_root = std::env::var_os(FIREFOX_PROFILES_OVERRIDE_ENV);
    let original_safari_root = std::env::var_os(SAFARI_ROOT_OVERRIDE_ENV);
    let original_keyring_root = std::env::var_os(TEST_KEYRING_OVERRIDE_ENV);

    unsafe {
        std::env::set_var(PROJECT_ROOT_OVERRIDE_ENV, dir.path());
        std::env::set_var(CHROME_USER_DATA_OVERRIDE_ENV, &chrome_root);
        std::env::remove_var(FIREFOX_PROFILES_OVERRIDE_ENV);
        std::env::remove_var(SAFARI_ROOT_OVERRIDE_ENV);
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

    restore_env_var(PROJECT_ROOT_OVERRIDE_ENV, original_project_root.as_deref());
    restore_env_var(CHROME_USER_DATA_OVERRIDE_ENV, original_chrome_root.as_deref());
    restore_env_var(FIREFOX_PROFILES_OVERRIDE_ENV, original_firefox_root.as_deref());
    restore_env_var(SAFARI_ROOT_OVERRIDE_ENV, original_safari_root.as_deref());
    restore_env_var(TEST_KEYRING_OVERRIDE_ENV, original_keyring_root.as_deref());
}

#[test]
fn app_snapshot_degrades_when_browser_discovery_fails() {
    let _guard = lock_env();
    let dir = tempdir().expect("tempdir");
    let broken_chrome_root = broken_chrome_user_data_fixture(dir.path());
    let empty_firefox_root = dir.path().join("firefox-profiles");
    let empty_safari_root = dir.path().join("safari-root");
    let keyring_root = dir.path().join("test-keyring");
    fs::create_dir_all(&empty_firefox_root).expect("create empty firefox root");
    fs::create_dir_all(&empty_safari_root).expect("create empty safari root");

    unsafe {
        std::env::set_var(PROJECT_ROOT_OVERRIDE_ENV, dir.path());
        std::env::set_var(CHROME_USER_DATA_OVERRIDE_ENV, &broken_chrome_root);
        std::env::set_var(FIREFOX_PROFILES_OVERRIDE_ENV, &empty_firefox_root);
        std::env::set_var(SAFARI_ROOT_OVERRIDE_ENV, &empty_safari_root);
        std::env::set_var(TEST_KEYRING_OVERRIDE_ENV, &keyring_root);
    }

    let config = initialized_config();
    let snapshot = initialize_archive_database(&config, None).expect("initialize archive");
    assert!(snapshot.archive_status.initialized);
    assert!(snapshot.browser_profiles.iter().all(|profile| !profile.history_readable));
    assert_eq!(
        snapshot.directories.config_path,
        dir.path().join("config.json").display().to_string()
    );

    unsafe {
        std::env::remove_var(PROJECT_ROOT_OVERRIDE_ENV);
        std::env::remove_var(CHROME_USER_DATA_OVERRIDE_ENV);
        std::env::remove_var(FIREFOX_PROFILES_OVERRIDE_ENV);
        std::env::remove_var(SAFARI_ROOT_OVERRIDE_ENV);
        std::env::remove_var(TEST_KEYRING_OVERRIDE_ENV);
    }
}

#[test]
fn app_snapshot_stays_usable_when_archive_is_initialized_but_locked() {
    let _guard = lock_env();
    let dir = tempdir().expect("tempdir");
    let chrome_root = chrome_user_data_fixture(dir.path());
    let keyring_root = dir.path().join("test-keyring");

    unsafe {
        std::env::set_var(PROJECT_ROOT_OVERRIDE_ENV, dir.path());
        std::env::set_var(CHROME_USER_DATA_OVERRIDE_ENV, &chrome_root);
        std::env::set_var(TEST_KEYRING_OVERRIDE_ENV, &keyring_root);
    }

    let mut config = initialized_config();
    config.archive_mode = ArchiveMode::Encrypted;
    let snapshot =
        initialize_archive_database(&config, Some("000000")).expect("initialize encrypted archive");
    assert!(snapshot.archive_status.unlocked);

    let locked_snapshot = app_snapshot(None).expect("locked archive snapshot");
    assert!(locked_snapshot.config.initialized);
    assert!(locked_snapshot.archive_status.initialized);
    assert!(locked_snapshot.archive_status.encrypted);
    assert!(!locked_snapshot.archive_status.unlocked);
    assert!(
        locked_snapshot
            .archive_status
            .warning
            .as_deref()
            .is_some_and(|warning| warning.contains("database key is required"))
    );
    assert!(locked_snapshot.recent_runs.is_empty());
    assert!(locked_snapshot.recent_import_batches.is_empty());

    unsafe {
        std::env::remove_var(PROJECT_ROOT_OVERRIDE_ENV);
        std::env::remove_var(CHROME_USER_DATA_OVERRIDE_ENV);
        std::env::remove_var(TEST_KEYRING_OVERRIDE_ENV);
    }
}

#[test]
fn worker_cli_rejects_unknown_commands() {
    let error = run_worker_cli(&["wat".to_string()]).expect_err("unknown command should fail");
    assert!(error.to_string().contains("unknown worker command"));
}

#[test]
fn worker_cli_rejects_mcp_server_until_explicitly_enabled() {
    let _guard = lock_env();
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
fn mcp_surface_respects_visibility_and_locked_app_sessions() {
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

    let mut config = configured_ai_config();
    config.ai.job_queue_paused = true;
    initialize_archive_database(&config, None).expect("initialize archive");
    run_backup_now(None, false).expect("backup");

    let imported = import_takeout_source(
        None,
        &TakeoutRequest { source_path: takeout_source, dry_run: false },
    )
    .expect("import takeout");
    let batch_id = imported.import_batch.expect("import batch").id;

    let visible = mcp_search_result(
        None,
        McpSearchRequest {
            query: "Imported".to_string(),
            profile_id: None,
            domain: None,
            limit: Some(10),
        },
    )
    .expect("visible mcp search");
    assert_eq!(visible.total, 1);

    revert_import_batch_detail(None, batch_id).expect("revert takeout batch");
    let hidden = mcp_search_result(
        None,
        McpSearchRequest {
            query: "Imported".to_string(),
            profile_id: None,
            domain: None,
            limit: Some(10),
        },
    )
    .expect("hidden mcp search");
    assert_eq!(hidden.total, 0);

    configure_app_lock_passcode(&SetAppLockPasscodeRequest {
        passcode: "2468".to_string(),
        recovery_hint: Some("desk drawer".to_string()),
    })
    .expect("configure app lock passcode");
    let mut locked_config = config.clone();
    locked_config.app_lock.enabled = true;
    save_user_config(&locked_config, None).expect("enable app lock");
    let locked = lock_app_ui_session(Some("manual")).expect("lock app session");
    assert!(locked.locked);
    let biometric_unlock =
        unlock_app_ui_session(&UnlockAppSessionRequest { passcode: None, use_biometric: true })
            .expect_err("unsupported biometric unlock should fail");
    assert!(biometric_unlock.to_string().contains("Biometric"));

    let search_error = mcp_search_result(
        None,
        McpSearchRequest {
            query: "example".to_string(),
            profile_id: None,
            domain: None,
            limit: Some(10),
        },
    )
    .expect_err("locked app should reject mcp search");
    assert!(search_error.to_string().contains("currently locked"));

    let archive_status = mcp_archive_status_result(None).expect("locked archive status");
    assert!(!archive_status.unlocked);
    assert_eq!(archive_status.warning.as_deref(), Some("PathKeep is currently locked."));

    let cli_error =
        run_worker_cli(&["mcp-server".to_string()]).expect_err("locked mcp server should fail");
    assert!(cli_error.to_string().contains("Unlock PathKeep"));

    unsafe {
        std::env::remove_var(PROJECT_ROOT_OVERRIDE_ENV);
        std::env::remove_var(CHROME_USER_DATA_OVERRIDE_ENV);
        std::env::remove_var(TEST_KEYRING_OVERRIDE_ENV);
    }
}

#[test]
fn ai_worker_helpers_cover_preview_secret_and_lexical_search_flows() {
    let _guard = lock_env();
    let dir = tempdir().expect("tempdir");
    let chrome_root = chrome_user_data_fixture(dir.path());
    let keyring_root = dir.path().join("test-keyring");

    unsafe {
        std::env::set_var(PROJECT_ROOT_OVERRIDE_ENV, dir.path());
        std::env::set_var(CHROME_USER_DATA_OVERRIDE_ENV, &chrome_root);
        std::env::set_var(TEST_KEYRING_OVERRIDE_ENV, &keyring_root);
    }

    let mut config = configured_ai_config();
    config.ai.job_queue_paused = true;
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

    let cleared_snapshot = clear_ai_provider_api_key("llm-primary", None).expect("clear llm key");
    assert!(!cleared_snapshot.config.ai.llm_providers[0].api_key_saved);

    let search = search_ai_history(
        None,
        &vault_core::AiSearchRequest {
            query: "example".to_string(),
            profile_id: None,
            domain: None,
            limit: Some(5),
            cursor: None,
        },
    )
    .expect("search history");
    assert_eq!(search.total, 1);
    assert!(!search.items.is_empty());
    assert!(!search.notes.is_empty());

    let index_report =
        build_ai_index_now(None, &AiIndexRequest::default()).expect("queue paused index");
    assert!(index_report.job_id.is_some());
    assert!(index_report.run_id.is_none());
    assert!(index_report.notes.iter().any(|note| note.contains("Resume the AI queue")));

    let assistant_report = ask_ai_assistant(
        None,
        &AiAssistantRequest {
            question: "What did I search?".to_string(),
            profile_id: None,
            domain: None,
        },
    )
    .expect("queue paused assistant");
    assert_eq!(assistant_report.state, "queued");
    assert!(assistant_report.job_id.is_some());
    assert!(assistant_report.notes.iter().any(|note| note.contains("The AI queue is paused")));

    unsafe {
        std::env::remove_var(PROJECT_ROOT_OVERRIDE_ENV);
        std::env::remove_var(CHROME_USER_DATA_OVERRIDE_ENV);
        std::env::remove_var(TEST_KEYRING_OVERRIDE_ENV);
    }
}

#[test]
fn queued_assistant_jobs_keep_their_enqueued_provider_snapshot() {
    let _guard = lock_env();
    let dir = tempdir().expect("tempdir");
    let chrome_root = chrome_user_data_fixture(dir.path());
    let keyring_root = dir.path().join("test-keyring");

    unsafe {
        std::env::set_var(PROJECT_ROOT_OVERRIDE_ENV, dir.path());
        std::env::set_var(CHROME_USER_DATA_OVERRIDE_ENV, &chrome_root);
        std::env::set_var(TEST_KEYRING_OVERRIDE_ENV, &keyring_root);
    }

    let mut config = configured_ai_config();
    config.ai.job_queue_paused = true;
    initialize_archive_database(&config, None).expect("initialize archive");
    save_user_config(&config, None).expect("save initial config");

    let queued = ask_ai_assistant(
        None,
        &AiAssistantRequest {
            question: "What changed?".to_string(),
            profile_id: None,
            domain: None,
        },
    )
    .expect("queue assistant request");
    assert_eq!(queued.provider_id, "llm-primary");
    assert_eq!(queued.embedding_provider_id, "embed-primary");

    let mut changed = config.clone();
    changed.ai.llm_provider_id = Some("llm-secondary".to_string());
    changed.ai.embedding_provider_id = Some("embed-secondary".to_string());
    save_user_config(&changed, None).expect("save changed config");

    let loaded =
        load_ai_assistant_job(None, queued.job_id.expect("queued job id")).expect("load job");
    assert_eq!(loaded.state, "paused");
    assert_eq!(loaded.provider_id, "llm-primary");
    assert_eq!(loaded.embedding_provider_id, "embed-primary");

    unsafe {
        std::env::remove_var(PROJECT_ROOT_OVERRIDE_ENV);
        std::env::remove_var(CHROME_USER_DATA_OVERRIDE_ENV);
        std::env::remove_var(TEST_KEYRING_OVERRIDE_ENV);
    }
}

#[test]
fn build_ai_index_returns_a_background_job_report_without_blocking_the_caller() {
    let _guard = lock_env();
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
    save_user_config(&config, None).expect("save config");

    let report =
        build_ai_index_now(None, &AiIndexRequest::default()).expect("queue background index");
    let job_id = report.job_id.expect("queued job id");
    assert!(report.run_id.is_none());
    assert!(report.notes.iter().any(|note| note.contains("processing it in the background")));

    let _ = run_ai_queue_jobs(None, None).expect("drain queued background job");
    let mut queue = load_ai_queue(None).expect("load ai queue");
    for _ in 0..20 {
        if queue.queued == 0 && queue.running == 0 {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
        queue = load_ai_queue(None).expect("reload ai queue");
    }
    assert!(queue.recent_jobs.iter().any(|job| job.id == job_id
        && matches!(job.state.as_str(), "queued" | "running" | "succeeded" | "failed")));

    unsafe {
        std::env::remove_var(PROJECT_ROOT_OVERRIDE_ENV);
        std::env::remove_var(CHROME_USER_DATA_OVERRIDE_ENV);
        std::env::remove_var(TEST_KEYRING_OVERRIDE_ENV);
    }
}

#[test]
fn manual_backup_refreshes_deterministic_insights_automatically() {
    let _guard = lock_env();
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
    assert_eq!(backup.run.expect("backup run").new_visits, 1);
    assert!(
        backup
            .warnings
            .iter()
            .all(|warning| !warning.contains("Deterministic insights could not refresh"))
    );

    let mut runtime =
        load_intelligence_runtime_snapshot(None).expect("load intelligence runtime snapshot");
    for _ in 0..50 {
        if runtime
            .recent_jobs
            .iter()
            .any(|job| job.job_type == "structural-rebuild" && job.state == "succeeded")
        {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
        runtime =
            load_intelligence_runtime_snapshot(None).expect("reload intelligence runtime snapshot");
    }

    let sessions = get_sessions(
        None,
        &PagedDateRangeRequest {
            date_range: vault_core::DateRange {
                start: "1970-01-01".to_string(),
                end: "2100-01-01".to_string(),
            },
            profile_id: None,
            page: 0,
            page_size: 10,
        },
    )
    .expect("load sessions");
    assert!(sessions.total >= 1);
    assert!(runtime.recent_jobs.iter().any(|job| {
        job.job_type == "visit-derive"
            && job.state == "succeeded"
            && job.progress_percent == Some(100.0)
    }));
    assert!(runtime.recent_jobs.iter().any(|job| {
        job.job_type == "daily-rollup"
            && job.state == "succeeded"
            && job.progress_percent == Some(100.0)
    }));
    assert!(runtime.recent_jobs.iter().any(|job| {
        job.job_type == "structural-rebuild"
            && job.state == "succeeded"
            && job.progress_percent == Some(100.0)
    }));

    unsafe {
        std::env::remove_var(PROJECT_ROOT_OVERRIDE_ENV);
        std::env::remove_var(CHROME_USER_DATA_OVERRIDE_ENV);
        std::env::remove_var(TEST_KEYRING_OVERRIDE_ENV);
    }
}

#[test]
fn worker_support_helpers_cover_schedule_takeout_and_keyring_flows() {
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
        HistoryQuery { q: Some("example".to_string()), limit: Some(10), ..HistoryQuery::default() },
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
    assert!(preview.apply_supported);
    let linux_preview = preview_schedule_plan(Some("linux"), Some(PathBuf::from("/tmp/bhb")))
        .expect("preview linux schedule");
    let applied = apply_schedule_plan(&linux_preview).expect("apply schedule");
    assert!(!applied.applied);
    let removed = remove_schedule_plan(&linux_preview).expect("remove schedule");
    assert!(!removed.applied);
    let schedule = schedule_status(None, Some("linux"), Some(PathBuf::from("/tmp/bhb")))
        .expect("schedule status");
    assert_eq!(schedule.install_state, "manual-review");
    assert!(!schedule.warnings.is_empty());

    let takeout_preview = inspect_takeout_source(&TakeoutRequest {
        source_path: takeout_source.clone(),
        dry_run: true,
    })
    .expect("inspect takeout");
    assert_eq!(takeout_preview.candidate_items, 1);

    let browser_history_request = BrowserHistoryImportRequest {
        source_path: chrome_root.join("Default").join("History").display().to_string(),
        dry_run: true,
        browser_family: Some("chromium".to_string()),
        profile_id: Some("chrome:Default".to_string()),
        browser_name: Some("Chrome".to_string()),
        profile_name: Some("Default".to_string()),
    };
    let browser_history_preview =
        inspect_browser_history_source(&browser_history_request).expect("inspect browser history");
    assert_eq!(browser_history_preview.candidate_items, 1);
    let browser_history_import = import_browser_history_source(None, &browser_history_request)
        .expect("dry-run browser import");
    assert_eq!(browser_history_import.candidate_items, 1);

    let imported = import_takeout_source(
        None,
        &TakeoutRequest { source_path: takeout_source, dry_run: false },
    )
    .expect("import takeout");
    let batch_id = imported.import_batch.expect("import batch").id;
    assert_eq!(imported.imported_items, 1);
    assert!(imported.notes.iter().any(|note| { note.contains("Core Intelligence refresh jobs") }));
    let import_preview = preview_import_batch_detail(None, batch_id).expect("preview batch");
    assert_eq!(import_preview.batch.status, "imported");
    let reverted = revert_import_batch_detail(None, batch_id).expect("revert batch");
    assert_eq!(reverted.batch.status, "reverted");
    let restored = restore_import_batch_detail(None, batch_id).expect("restore batch");
    assert_eq!(restored.batch.status, "imported");

    assert_eq!(read_database_key_from_keyring().expect("read empty db key"), None);
    let stored_report = write_database_key_to_keyring("db-secret").expect("store db key");
    assert!(stored_report.stored_secret);
    assert_eq!(
        read_database_key_from_keyring().expect("read db key"),
        Some("db-secret".to_string())
    );
    assert!(keyring_report().stored_secret);
    assert!(!clear_database_key_from_keyring().expect("clear db key").stored_secret);

    let security = security_status(None).expect("security status");
    assert_eq!(security.mode, "plaintext");
    assert!(security.initialized);

    let rekey_preview = preview_rekey_archive(
        None,
        &RekeyRequest { new_mode: ArchiveMode::Encrypted, new_key: None },
    )
    .expect("preview rekey");
    assert!(rekey_preview.requires_new_key);
    assert!(rekey_preview.snapshot_path.contains("raw-snapshots/rekey"));
    assert!(
        rekey_preview
            .warnings
            .iter()
            .any(|warning| warning.contains("requires a new database key"))
    );
    let same_mode_rekey_preview = preview_rekey_archive(
        None,
        &RekeyRequest { new_mode: ArchiveMode::Plaintext, new_key: None },
    )
    .expect("same-mode rekey preview");
    assert!(
        same_mode_rekey_preview
            .warnings
            .iter()
            .any(|warning| { warning.contains("target mode matches the current mode") })
    );

    store_s3_credentials(&S3CredentialInput {
        access_key_id: "akid".to_string(),
        secret_access_key: "secret".to_string(),
    })
    .expect("store s3");
    let remote_preview = preview_remote_backup_bundle().expect("remote preview");
    assert!(remote_preview.upload_url.contains("worker-tests"));
    clear_s3_credentials().expect("clear s3");
    let remote_error = upload_remote_backup_bundle(None).expect_err("remote backup should fail");
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

#[test]
fn security_status_reports_uninitialized_archive_mode() {
    let _guard = lock_env();
    let dir = tempdir().expect("tempdir");
    let original_project_root = std::env::var_os(PROJECT_ROOT_OVERRIDE_ENV);
    let original_keyring_root = std::env::var_os(TEST_KEYRING_OVERRIDE_ENV);
    unsafe {
        std::env::set_var(PROJECT_ROOT_OVERRIDE_ENV, dir.path());
        std::env::set_var(TEST_KEYRING_OVERRIDE_ENV, dir.path().join("test-keyring"));
    }

    let security = security_status(None).expect("uninitialized security status");
    let preview_error = preview_rekey_archive(
        None,
        &RekeyRequest { new_mode: ArchiveMode::Encrypted, new_key: Some("next".to_string()) },
    )
    .expect_err("uninitialized archive cannot preview rekey");
    assert!(preview_error.to_string().contains("initialize the archive"));

    restore_env_var(PROJECT_ROOT_OVERRIDE_ENV, original_project_root.as_deref());
    restore_env_var(TEST_KEYRING_OVERRIDE_ENV, original_keyring_root.as_deref());

    assert_eq!(security.mode, "uninitialized");
    assert!(!security.initialized);
}

#[test]
fn provider_resolution_helpers_cover_error_success_and_note_paths() {
    let _guard = lock_env();
    let dir = tempdir().expect("tempdir");
    let original_project_root = std::env::var_os(PROJECT_ROOT_OVERRIDE_ENV);
    let original_keyring_root = std::env::var_os(TEST_KEYRING_OVERRIDE_ENV);
    unsafe {
        std::env::set_var(PROJECT_ROOT_OVERRIDE_ENV, dir.path());
        std::env::set_var(TEST_KEYRING_OVERRIDE_ENV, dir.path().join("test-keyring"));
    }

    let server = BrowserHistoryMcpServer::new(None);
    assert!(server.database_key.is_none());

    let mut config = configured_ai_config();
    hydrate_provider_collection(&mut config.ai.llm_providers);
    assert!(!config.ai.llm_providers[0].api_key_saved);

    let paths = project_paths().expect("project paths");
    fs::create_dir_all(paths.archive_database_path.parent().expect("archive parent"))
        .expect("create archive parent");
    fs::write(&paths.archive_database_path, "not-a-database").expect("write invalid archive");
    let derive_status = derive_ai_status(&paths, &config, Some("wrong-key"));
    assert!(derive_status.warning.is_some());

    let missing_provider = resolve_provider_runtime(
        &config.ai.llm_providers,
        "missing-provider",
        AiProviderPurpose::Llm,
    )
    .expect_err("missing provider should fail");
    assert!(missing_provider.to_string().contains("was not found"));

    let wrong_purpose = resolve_provider_runtime(
        &config.ai.embedding_providers,
        "embed-primary",
        AiProviderPurpose::Llm,
    )
    .expect_err("wrong purpose should fail");
    assert!(wrong_purpose.to_string().contains("configured for"));

    keyring_set_provider_api_key("embed-primary", "embed-secret").expect("set provider key");
    let resolved = resolve_provider_runtime(
        &config.ai.embedding_providers,
        "embed-primary",
        AiProviderPurpose::Embedding,
    )
    .expect("resolve provider");
    assert_eq!(resolved.api_key, "embed-secret");

    config.ai.embedding_provider_id = None;
    assert!(selected_optional_embedding_runtime(&config).expect("optional embedding").is_none());

    let response = search_response_with_resolution_note(
        AiSearchResponse::default(),
        Some(anyhow::anyhow!("semantic backend offline")),
    );
    assert!(
        response
            .notes
            .iter()
            .any(|note| note.contains("Semantic retrieval is unavailable right now"))
    );

    restore_env_var(PROJECT_ROOT_OVERRIDE_ENV, original_project_root.as_deref());
    restore_env_var(TEST_KEYRING_OVERRIDE_ENV, original_keyring_root.as_deref());
}

#[cfg(coverage)]
#[test]
fn coverage_worker_flows_cover_successful_ai_remote_and_mcp_paths() {
    let _guard = lock_env();
    let dir = tempdir().expect("tempdir");
    let chrome_root = chrome_user_data_fixture(dir.path());
    let keyring_root = dir.path().join("test-keyring");
    let bin_dir = dir.path().join("fake-bin");
    let curl_path = install_fake_curl(&bin_dir, "#!/bin/sh\nexit 0\n");

    unsafe {
        std::env::set_var(PROJECT_ROOT_OVERRIDE_ENV, dir.path());
        std::env::set_var(CHROME_USER_DATA_OVERRIDE_ENV, &chrome_root);
        std::env::set_var(TEST_KEYRING_OVERRIDE_ENV, &keyring_root);
        std::env::set_var("BHB_TEST_CURL_BIN", &curl_path);
    }

    let mut config = configured_ai_config();
    config.remote_backup.enabled = true;
    config.remote_backup.bucket = "worker-tests".to_string();
    config.remote_backup.region = "us-west-2".to_string();
    config.remote_backup.prefix = "archives".to_string();
    config.remote_backup.upload_after_backup = true;
    config.ai.auto_index_after_backup = true;
    initialize_archive_database(&config, None).expect("initialize archive");
    save_user_config(&config, None).expect("save config");

    store_ai_provider_api_key(
        &AiProviderSecretInput {
            provider_id: "embed-primary".to_string(),
            api_key: "embed-secret".to_string(),
        },
        None,
    )
    .expect("store embed key");
    store_ai_provider_api_key(
        &AiProviderSecretInput {
            provider_id: "llm-primary".to_string(),
            api_key: "llm-secret".to_string(),
        },
        None,
    )
    .expect("store llm key");
    store_s3_credentials(&S3CredentialInput {
        access_key_id: "akid".to_string(),
        secret_access_key: "secret".to_string(),
    })
    .expect("store s3 creds");

    let backup = run_backup_now(None, false).expect("backup with follow-up tasks");
    assert!(backup.remote_backup.is_some());

    let index = build_ai_index_now(None, &AiIndexRequest::default()).expect("build ai index");
    assert!(!index.provider_id.is_empty());
    for _ in 0..50 {
        let queue = load_ai_queue(None).expect("load ai queue");
        if queue.queued == 0 && queue.running == 0 {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }

    let search = search_ai_history(
        None,
        &AiSearchRequest {
            query: "example".to_string(),
            profile_id: None,
            domain: None,
            limit: Some(5),
            cursor: None,
        },
    )
    .expect("semantic search");
    assert_eq!(search.provider_id, "embed-primary");

    let answer = ask_ai_assistant(
        None,
        &AiAssistantRequest {
            question: "What did I visit?".to_string(),
            profile_id: None,
            domain: None,
        },
    )
    .expect("assistant answer");
    assert_eq!(answer.state, "insufficient-evidence");
    assert!(
        answer.answer.contains("I couldn't find enough matching history evidence"),
        "{}",
        answer.answer
    );

    let tool_result = mcp_search_result(
        None,
        McpSearchRequest {
            query: "example".to_string(),
            profile_id: None,
            domain: None,
            limit: Some(5),
        },
    )
    .expect("search history tool");
    assert!(tool_result.total >= 1);

    let archive_status = mcp_archive_status_result(None).expect("archive status tool");
    assert!(archive_status.initialized);

    let mcp_server = BrowserHistoryMcpServer::new(None);
    let routed_status = block_on_ready(mcp_server.archive_status()).expect("routed archive status");
    assert!(routed_status.0.initialized);
    let routed_search = block_on_ready(mcp_server.search_history(
        rmcp::handler::server::wrapper::Parameters(McpSearchRequest {
            query: "example".to_string(),
            profile_id: None,
            domain: None,
            limit: Some(5),
        }),
    ))
    .expect("routed search history");
    assert!(routed_search.0.total >= 1);

    let remote_json = run_worker_cli(&["remote-backup".to_string()]).expect("remote backup cli");
    let remote: RemoteBackupResult =
        serde_json::from_str(&remote_json).expect("parse remote result");
    assert!(remote.uploaded);

    let index_json = run_worker_cli(&["ai-index".to_string()]).expect("ai-index cli");
    let index_report: AiIndexReport =
        serde_json::from_str(&index_json).expect("parse ai index report");
    assert!(!index_report.provider_id.is_empty());
    let queue_json = run_worker_cli(&["ai-queue".to_string()]).expect("ai-queue cli");
    let queue_status: AiQueueStatus =
        serde_json::from_str(&queue_json).expect("parse ai queue status");
    assert!(queue_status.concurrency >= 1);

    let doctor_json = run_worker_cli(&["doctor".to_string()]).expect("doctor cli");
    let doctor: HealthReport = serde_json::from_str(&doctor_json).expect("parse doctor");
    assert!(!doctor.checks.is_empty());

    assert_eq!(run_worker_cli(&["mcp-server".to_string()]).expect("mcp cli"), "");

    unsafe {
        std::env::remove_var(PROJECT_ROOT_OVERRIDE_ENV);
        std::env::remove_var(CHROME_USER_DATA_OVERRIDE_ENV);
        std::env::remove_var(TEST_KEYRING_OVERRIDE_ENV);
        std::env::remove_var("BHB_TEST_CURL_BIN");
    }
}

#[cfg(coverage)]
#[test]
fn coverage_run_backup_now_reports_missing_follow_up_requirements() {
    let _guard = lock_env();
    let dir = tempdir().expect("tempdir");
    let chrome_root = chrome_user_data_fixture(dir.path());
    let keyring_root = dir.path().join("test-keyring");

    unsafe {
        std::env::set_var(PROJECT_ROOT_OVERRIDE_ENV, dir.path());
        std::env::set_var(CHROME_USER_DATA_OVERRIDE_ENV, &chrome_root);
        std::env::set_var(TEST_KEYRING_OVERRIDE_ENV, &keyring_root);
    }

    let mut config = configured_ai_config();
    config.remote_backup.enabled = true;
    config.remote_backup.bucket = "worker-tests".to_string();
    config.remote_backup.region = "us-west-2".to_string();
    config.remote_backup.upload_after_backup = true;
    config.ai.auto_index_after_backup = true;
    config.ai.embedding_provider_id = None;
    initialize_archive_database(&config, None).expect("initialize archive");
    save_user_config(&config, None).expect("save config");

    let report = run_backup_now(None, false).expect("backup with missing follow-ups");
    assert!(report.warnings.iter().any(|warning| warning.contains("S3 credentials")));
    assert!(report.warnings.iter().any(|warning| warning.contains("embedding provider")));

    unsafe {
        std::env::remove_var(PROJECT_ROOT_OVERRIDE_ENV);
        std::env::remove_var(CHROME_USER_DATA_OVERRIDE_ENV);
        std::env::remove_var(TEST_KEYRING_OVERRIDE_ENV);
    }
}

#[cfg(coverage)]
#[test]
fn coverage_ai_queue_paused_and_semantic_fallback_paths_stay_truthful() {
    let _guard = lock_env();
    let dir = tempdir().expect("tempdir");
    let chrome_root = chrome_user_data_fixture(dir.path());
    let keyring_root = dir.path().join("test-keyring");

    unsafe {
        std::env::set_var(PROJECT_ROOT_OVERRIDE_ENV, dir.path());
        std::env::set_var(CHROME_USER_DATA_OVERRIDE_ENV, &chrome_root);
        std::env::set_var(TEST_KEYRING_OVERRIDE_ENV, &keyring_root);
    }

    let mut config = configured_ai_config();
    config.ai.job_queue_paused = true;
    initialize_archive_database(&config, None).expect("initialize archive");
    save_user_config(&config, None).expect("save config");
    let paths = project_paths().expect("project paths");
    assert!(
        !drain_one_ai_queue_job(&paths, None).expect("paused AI queue drain is idle"),
        "paused AI queue must not claim work"
    );
    assert!(
        !drain_one_priority_intelligence_job(&paths, None)
            .expect("paused deterministic queue drain is idle"),
        "paused deterministic queue must not claim work"
    );
    assert!(
        !drain_one_enrichment_intelligence_job(&paths, None)
            .expect("paused enrichment queue drain is idle"),
        "paused enrichment queue must not claim work"
    );

    let paused_index =
        build_ai_index_now(None, &AiIndexRequest::default()).expect("queue paused index job");
    let index_job_id = paused_index.job_id.expect("paused index job id");
    assert!(paused_index.run_id.is_none());
    assert!(paused_index.notes.iter().any(|note| note.contains("Resume the AI queue")));

    let drained = run_ai_queue_jobs(None, Some(2)).expect("paused drain should only report status");
    assert!(drained.paused);
    assert!(drained.queued >= 1);

    let queued_rebuild = queue_core_intelligence_rebuild(
        None,
        &CoreIntelligenceRebuildRequest { profile_id: None, full_rebuild: true, limit: None },
    )
    .expect("queue paused deterministic rebuild");
    assert!(queued_rebuild.notes.iter().any(|note| note.contains("runtime queue is paused")));
    let runtime_snapshot =
        load_intelligence_runtime_snapshot(None).expect("load paused runtime snapshot");
    assert!(runtime_snapshot.queue.queued >= 1);
    let cancelled_runtime = cancel_intelligence_job_now(None, queued_rebuild.job_id)
        .expect("cancel queued deterministic rebuild");
    assert!(cancelled_runtime.recent_jobs.iter().any(|job| job.id == queued_rebuild.job_id));
    let retried_runtime =
        retry_intelligence_job_now(None, queued_rebuild.job_id).expect("retry runtime snapshot");
    assert!(retried_runtime.recent_jobs.iter().any(|job| job.id == queued_rebuild.job_id));

    let index_as_assistant =
        load_ai_assistant_job(None, index_job_id).expect("load non-assistant job through facade");
    assert_eq!(index_as_assistant.job_id, Some(index_job_id));
    assert_eq!(index_as_assistant.provider_id, "llm-primary");
    assert_eq!(index_as_assistant.embedding_provider_id, "embed-primary");
    assert!(index_as_assistant.notes.iter().any(|note| note.contains("has not finished yet")));

    config.ai.job_queue_paused = false;
    save_user_config(&config, None).expect("unpause config");
    let fallback_search = search_ai_history(
        None,
        &AiSearchRequest {
            query: "example".to_string(),
            profile_id: None,
            domain: None,
            limit: Some(3),
            cursor: None,
        },
    )
    .expect("semantic search degrades to lexical when embedding secret is unavailable");
    assert_eq!(fallback_search.provider_id, "lexical-fallback");
    assert!(
        fallback_search
            .notes
            .iter()
            .any(|note| note.contains("Semantic retrieval is unavailable right now"))
    );

    unsafe {
        std::env::remove_var(PROJECT_ROOT_OVERRIDE_ENV);
        std::env::remove_var(CHROME_USER_DATA_OVERRIDE_ENV);
        std::env::remove_var(TEST_KEYRING_OVERRIDE_ENV);
    }
}

#[cfg(coverage)]
#[test]
fn coverage_ai_queue_completion_helpers_cover_cancel_and_payload_edges() {
    let _guard = lock_env();
    let dir = tempdir().expect("tempdir");
    let chrome_root = chrome_user_data_fixture(dir.path());
    let keyring_root = dir.path().join("test-keyring");

    unsafe {
        std::env::set_var(PROJECT_ROOT_OVERRIDE_ENV, dir.path());
        std::env::set_var(CHROME_USER_DATA_OVERRIDE_ENV, &chrome_root);
        std::env::set_var(TEST_KEYRING_OVERRIDE_ENV, &keyring_root);
    }

    let mut config = configured_ai_config();
    let mut llm_error_provider = config.ai.llm_providers[0].clone();
    llm_error_provider.id = "llm-error".to_string();
    llm_error_provider.name = "Failing coverage LLM".to_string();
    config.ai.llm_providers.push(llm_error_provider);
    let mut batch_error_provider = config.ai.embedding_providers[0].clone();
    batch_error_provider.id = "embed-batch-error-single-error".to_string();
    batch_error_provider.name = "Failing batch embedding".to_string();
    config.ai.embedding_providers.push(batch_error_provider);
    initialize_archive_database(&config, None).expect("initialize archive");
    save_user_config(&config, None).expect("save config");
    run_backup_now(None, false).expect("backup");
    store_ai_provider_api_key(
        &AiProviderSecretInput {
            provider_id: "embed-primary".to_string(),
            api_key: "embed-secret".to_string(),
        },
        None,
    )
    .expect("store embed key");
    store_ai_provider_api_key(
        &AiProviderSecretInput {
            provider_id: "llm-primary".to_string(),
            api_key: "llm-secret".to_string(),
        },
        None,
    )
    .expect("store llm key");
    store_ai_provider_api_key(
        &AiProviderSecretInput {
            provider_id: "llm-error".to_string(),
            api_key: "llm-error-secret".to_string(),
        },
        None,
    )
    .expect("store llm error key");
    store_ai_provider_api_key(
        &AiProviderSecretInput {
            provider_id: "embed-batch-error-single-error".to_string(),
            api_key: "embed-batch-error-single-error-secret".to_string(),
        },
        None,
    )
    .expect("store batch error key");

    let paths = project_paths().expect("project paths");
    let connection = vault_core::archive::open_intelligence_connection(&paths, &config, None)
        .expect("intelligence connection");
    let job_state = |job_id: i64| -> String {
        connection
            .query_row("SELECT state FROM ai_jobs WHERE id = ?1", [job_id], |row| row.get(0))
            .expect("ai job state")
    };

    let control_job =
        vault_core::ai_queue::enqueue_index_job(&connection, &AiIndexRequest::default(), false)
            .expect("enqueue control job");
    let _claimed_control =
        vault_core::ai_queue::claim_ai_job_by_id(&connection, control_job.id, 300)
            .expect("claim control job")
            .expect("claimed control job");
    vault_core::ai_queue::cancel_ai_job(&connection, control_job.id)
        .expect("request control cancellation");
    let control = start_ai_job_control(&paths, &config, None, control_job.id);
    for _ in 0..100 {
        if control.cancelled() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    assert!(control.cancelled());
    assert!(
        control
            .checkpoint("AI queue control noticed cancellation")
            .expect_err("control checkpoint should fail")
            .to_string()
            .contains("cancellation")
    );
    control.shutdown();

    let index_request = AiIndexRequest {
        provider_id: Some("embed-primary".to_string()),
        clear_only: true,
        ..AiIndexRequest::default()
    };
    let index_job = vault_core::ai_queue::enqueue_index_job(&connection, &index_request, false)
        .expect("enqueue index job");
    let claimed_index = vault_core::ai_queue::claim_ai_job_by_id(&connection, index_job.id, 300)
        .expect("claim index")
        .expect("claimed index");
    vault_core::ai_queue::cancel_ai_job(&connection, index_job.id)
        .expect("request index cancellation");
    connection
        .execute(
            "UPDATE ai_jobs SET state = 'running', stop_requested = 1 WHERE id = ?1",
            [index_job.id],
        )
        .expect("pin claimed index job to running stop-requested state");
    let _ = complete_claimed_index_job(
        &connection,
        &paths,
        &config,
        None,
        claimed_index,
        &index_request,
    );
    assert_eq!(job_state(index_job.id), "cancelled");

    let cancelled_error_request = AiIndexRequest {
        provider_id: Some("embed-primary".to_string()),
        ..AiIndexRequest::default()
    };
    let cancelled_error_index =
        vault_core::ai_queue::enqueue_index_job(&connection, &cancelled_error_request, false)
            .expect("enqueue cancelled failing index job");
    let claimed_cancelled_error_index =
        vault_core::ai_queue::claim_ai_job_by_id(&connection, cancelled_error_index.id, 300)
            .expect("claim cancelled failing index")
            .expect("claimed cancelled failing index");
    vault_core::ai_queue::cancel_ai_job(&connection, cancelled_error_index.id)
        .expect("request failing index cancellation");
    let mut encrypted_without_key = config.clone();
    encrypted_without_key.archive_mode = ArchiveMode::Encrypted;
    let cancelled_index_error = complete_claimed_index_job(
        &connection,
        &paths,
        &encrypted_without_key,
        None,
        claimed_cancelled_error_index,
        &cancelled_error_request,
    )
    .expect_err("cancelled failing index should still surface the provider error");
    assert!(cancelled_index_error.to_string().contains("database key"));
    assert_eq!(job_state(cancelled_error_index.id), "cancelled");

    let assistant_request = AiAssistantRequest {
        question: "What did I visit?".to_string(),
        profile_id: None,
        domain: None,
    };
    let assistant_job = vault_core::ai_queue::enqueue_assistant_job(
        &connection,
        &assistant_request,
        "llm-primary",
        Some("embed-primary"),
        false,
    )
    .expect("enqueue assistant job");
    let claimed_assistant =
        vault_core::ai_queue::claim_ai_job_by_id(&connection, assistant_job.id, 300)
            .expect("claim assistant")
            .expect("claimed assistant");
    vault_core::ai_queue::cancel_ai_job(&connection, assistant_job.id)
        .expect("request assistant cancellation");
    let _ = complete_claimed_assistant_job(
        &connection,
        &paths,
        &config,
        None,
        claimed_assistant,
        &assistant_request,
    );
    assert_eq!(job_state(assistant_job.id), "cancelled");
    let replayed = replay_ai_job(None, assistant_job.id).expect("replay cancelled assistant job");
    assert_eq!(replayed.state, "queued");

    let wrong_payload = vault_core::ai_queue::StoredAiJob {
        id: 99_999,
        job_type: AiQueueJobType::IndexBuild,
        attempt: 1,
        max_attempts: 1,
        payload: vault_core::ai_queue::AiJobPayload::Index { request: AiIndexRequest::default() },
    };
    let payload_error = complete_claimed_assistant_job(
        &connection,
        &paths,
        &config,
        None,
        wrong_payload,
        &assistant_request,
    )
    .expect_err("index payload cannot execute as assistant");
    assert!(payload_error.to_string().contains("did not contain an assistant payload"));

    let failing_assistant_job = vault_core::ai_queue::enqueue_assistant_job(
        &connection,
        &assistant_request,
        "llm-error",
        Some("embed-primary"),
        false,
    )
    .expect("enqueue failing assistant job");
    let claimed_failing_assistant =
        vault_core::ai_queue::claim_ai_job_by_id(&connection, failing_assistant_job.id, 300)
            .expect("claim failing assistant")
            .expect("claimed failing assistant");
    let assistant_error = complete_claimed_assistant_job(
        &connection,
        &paths,
        &config,
        None,
        claimed_failing_assistant,
        &assistant_request,
    )
    .expect_err("failing assistant should mark the queue row failed");
    assert!(assistant_error.to_string().contains("forced coverage LLM error"));
    assert_eq!(job_state(failing_assistant_job.id), "failed");

    let cancelled_error_assistant = vault_core::ai_queue::enqueue_assistant_job(
        &connection,
        &assistant_request,
        "llm-error",
        Some("embed-primary"),
        false,
    )
    .expect("enqueue cancelled failing assistant job");
    let claimed_cancelled_error_assistant =
        vault_core::ai_queue::claim_ai_job_by_id(&connection, cancelled_error_assistant.id, 300)
            .expect("claim cancelled failing assistant")
            .expect("claimed cancelled failing assistant");
    vault_core::ai_queue::cancel_ai_job(&connection, cancelled_error_assistant.id)
        .expect("request failing assistant cancellation");
    let cancelled_assistant_error = complete_claimed_assistant_job(
        &connection,
        &paths,
        &config,
        None,
        claimed_cancelled_error_assistant,
        &assistant_request,
    )
    .expect_err("cancelled failing assistant should still surface the provider error");
    let cancelled_error_text = cancelled_assistant_error.to_string();
    assert!(
        cancelled_error_text.contains("forced coverage LLM error")
            || cancelled_error_text.contains("cancel")
    );
    assert_eq!(job_state(cancelled_error_assistant.id), "cancelled");

    let answered = ask_ai_assistant(None, &assistant_request).expect("assistant answer");
    let loaded = load_ai_assistant_job(None, answered.job_id.expect("assistant job id"))
        .expect("load completed assistant job");
    assert_eq!(loaded.state, "succeeded");
    assert!(loaded.run_id.is_some());
    assert_eq!(loaded.provider_id, "llm-primary");

    let foreground_index = vault_core::ai_queue::enqueue_index_job(
        &connection,
        &AiIndexRequest {
            provider_id: Some("embed-primary".to_string()),
            clear_only: true,
            ..AiIndexRequest::default()
        },
        false,
    )
    .expect("enqueue foreground index job");
    assert!(drain_one_ai_queue_job(&paths, None).expect("drain one foreground index job"));
    assert!(
        matches!(job_state(foreground_index.id).as_str(), "succeeded" | "failed" | "cancelled"),
        "foreground index should leave the runnable queue"
    );

    let foreground_job = vault_core::ai_queue::enqueue_assistant_job(
        &connection,
        &assistant_request,
        "llm-primary",
        Some("embed-primary"),
        false,
    )
    .expect("enqueue foreground assistant job");
    assert!(drain_one_ai_queue_job(&paths, None).expect("drain one foreground assistant job"));
    assert!(
        matches!(job_state(foreground_job.id).as_str(), "succeeded" | "failed" | "cancelled"),
        "foreground assistant should leave the runnable queue"
    );
    assert!(!drain_one_ai_queue_job(&paths, None).expect("idle AI queue drain"));

    let explicit_index = build_ai_index_now(
        None,
        &AiIndexRequest {
            provider_id: Some("embed-primary".to_string()),
            ..AiIndexRequest::default()
        },
    )
    .expect("queue explicit provider index");
    assert_eq!(explicit_index.provider_id, "embed-primary");

    let provider_report = test_ai_provider_connection_report(
        None,
        &AiProviderConnectionTestRequest {
            provider_id: "embed-primary".to_string(),
            purpose: AiProviderPurpose::Embedding,
        },
    )
    .expect("provider connection report");
    assert!(provider_report.ok);
    let llm_report = test_ai_provider_connection_report(
        None,
        &AiProviderConnectionTestRequest {
            provider_id: "llm-primary".to_string(),
            purpose: AiProviderPurpose::Llm,
        },
    )
    .expect("llm provider connection report");
    assert!(llm_report.ok);

    unsafe {
        std::env::remove_var(PROJECT_ROOT_OVERRIDE_ENV);
        std::env::remove_var(CHROME_USER_DATA_OVERRIDE_ENV);
        std::env::remove_var(TEST_KEYRING_OVERRIDE_ENV);
    }
}

#[cfg(coverage)]
#[test]
fn coverage_queue_spawn_and_foreground_drains_cover_worker_loops() {
    let _guard = lock_env();
    let dir = tempdir().expect("tempdir");
    let chrome_root = chrome_user_data_fixture(dir.path());
    let keyring_root = dir.path().join("test-keyring");

    unsafe {
        std::env::set_var(PROJECT_ROOT_OVERRIDE_ENV, dir.path());
        std::env::set_var(CHROME_USER_DATA_OVERRIDE_ENV, &chrome_root);
        std::env::set_var(TEST_KEYRING_OVERRIDE_ENV, &keyring_root);
    }

    let mut config = configured_ai_config();
    config.ai.job_queue_concurrency = 2;
    initialize_archive_database(&config, None).expect("initialize archive");
    save_user_config(&config, None).expect("save config");
    run_backup_now(None, false).expect("backup");
    store_ai_provider_api_key(
        &AiProviderSecretInput {
            provider_id: "embed-primary".to_string(),
            api_key: "embed-secret".to_string(),
        },
        None,
    )
    .expect("store embed key");
    store_ai_provider_api_key(
        &AiProviderSecretInput {
            provider_id: "llm-primary".to_string(),
            api_key: "llm-secret".to_string(),
        },
        None,
    )
    .expect("store llm key");

    let paths = project_paths().expect("project paths");
    let connection = vault_core::archive::open_intelligence_connection(&paths, &config, None)
        .expect("intelligence connection");
    let index = vault_core::ai_queue::enqueue_index_job(
        &connection,
        &AiIndexRequest {
            provider_id: Some("embed-primary".to_string()),
            clear_only: true,
            ..AiIndexRequest::default()
        },
        false,
    )
    .expect("enqueue foreground index");
    let assistant_request = AiAssistantRequest {
        question: "What did I visit?".to_string(),
        profile_id: None,
        domain: None,
    };
    let assistant = vault_core::ai_queue::enqueue_assistant_job(
        &connection,
        &assistant_request,
        "llm-primary",
        Some("embed-primary"),
        false,
    )
    .expect("enqueue foreground assistant");
    let status = run_ai_queue_jobs(None, Some(2)).expect("foreground drain");
    assert!(status.queued <= 1);
    for job_id in [index.id, assistant.id] {
        let state: String = connection
            .query_row("SELECT state FROM ai_jobs WHERE id = ?1", [job_id], |row| row.get(0))
            .expect("foreground job state");
        assert_ne!(state, "queued");
    }

    let invalid_root = dir.path().join("invalid-spawn-config");
    fs::create_dir_all(&invalid_root).expect("invalid spawn root");
    fs::write(invalid_root.join("config.json"), "{not-json").expect("invalid spawn config");
    let invalid_paths = vault_core::config::project_paths_with_root(&invalid_root);
    maybe_spawn_ai_queue_drain(&invalid_paths, &config, None, 1);
    maybe_spawn_intelligence_queue_drain(&invalid_paths, &config, None, 1);

    let mut single_lane_config = config.clone();
    single_lane_config.ai.job_queue_concurrency = 1;
    maybe_spawn_intelligence_queue_drain(&paths, &single_lane_config, None, 1);
    std::thread::sleep(std::time::Duration::from_millis(100));

    unsafe {
        std::env::remove_var(PROJECT_ROOT_OVERRIDE_ENV);
        std::env::remove_var(CHROME_USER_DATA_OVERRIDE_ENV);
        std::env::remove_var(TEST_KEYRING_OVERRIDE_ENV);
    }
}

#[cfg(coverage)]
#[test]
fn coverage_runtime_direct_execution_covers_claim_mismatch_and_success() {
    let _guard = lock_env();
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
    save_user_config(&config, None).expect("save config");
    let paths = project_paths().expect("project paths");
    assert!(
        !drain_one_priority_intelligence_job(&paths, None).expect("idle priority drain"),
        "empty priority queue must stay idle"
    );
    assert!(
        !drain_one_enrichment_intelligence_job(&paths, None).expect("idle enrichment drain"),
        "empty enrichment queue must stay idle"
    );

    let missing =
        execute_core_intelligence_job(&paths, &config, None, 99_999, FULL_REBUILD_JOB_TYPE)
            .expect("missing job is a no-op");
    assert!(!missing);

    let mismatch_id = {
        let connection = vault_core::archive::open_intelligence_connection(&paths, &config, None)
            .expect("intelligence");
        enqueue_core_intelligence_job(
            &connection,
            FULL_REBUILD_JOB_TYPE,
            &CoreIntelligenceRebuildRequest {
                profile_id: Some("chrome:mismatch".to_string()),
                full_rebuild: true,
                limit: Some(1),
            },
            "coverage mismatch",
        )
        .expect("enqueue mismatch job")
    };
    let mismatched =
        execute_core_intelligence_job(&paths, &config, None, mismatch_id, VISIT_DERIVE_JOB_TYPE)
            .expect("mismatched job type is a no-op");
    assert!(!mismatched);

    let success_id = {
        let connection = vault_core::archive::open_intelligence_connection(&paths, &config, None)
            .expect("intelligence");
        let job_id = enqueue_core_intelligence_job(
            &connection,
            VISIT_DERIVE_JOB_TYPE,
            &CoreIntelligenceRebuildRequest {
                profile_id: Some("chrome:direct-success".to_string()),
                full_rebuild: true,
                limit: Some(1),
            },
            "coverage direct success",
        )
        .expect("enqueue direct success job");
        let (job_type, state, payload_json): (String, String, String) = connection
            .query_row(
                "SELECT job_type, state, payload_json FROM intelligence_jobs WHERE id = ?1",
                [job_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .expect("queued direct success job");
        assert_eq!(job_type, VISIT_DERIVE_JOB_TYPE);
        assert!(payload_json.contains(VISIT_DERIVE_JOB_TYPE), "{payload_json}");
        assert_eq!(state, "queued");
        job_id
    };
    let succeeded =
        execute_core_intelligence_job(&paths, &config, None, success_id, VISIT_DERIVE_JOB_TYPE)
            .expect("execute direct success job");
    if !succeeded {
        let connection = vault_core::archive::open_intelligence_connection(&paths, &config, None)
            .expect("intelligence");
        let debug: (String, String, String) = connection
            .query_row(
                "SELECT job_type, state, payload_json FROM intelligence_jobs WHERE id = ?1",
                [success_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .expect("debug unclaimed success job");
        panic!("direct success job was not claimed: {debug:?}");
    }

    let connection = vault_core::archive::open_intelligence_connection(&paths, &config, None)
        .expect("intelligence");
    let state: String = connection
        .query_row("SELECT state FROM intelligence_jobs WHERE id = ?1", [success_id], |row| {
            row.get(0)
        })
        .expect("success job state");
    assert_eq!(state, "succeeded");

    for job_type in [DAILY_ROLLUP_JOB_TYPE, STRUCTURAL_REBUILD_JOB_TYPE] {
        let job_id = enqueue_core_intelligence_job(
            &connection,
            job_type,
            &CoreIntelligenceRebuildRequest {
                profile_id: Some(format!("chrome:{job_type}")),
                full_rebuild: true,
                limit: Some(1),
            },
            "coverage direct typed label",
        )
        .expect("enqueue typed direct job");
        let completed = execute_core_intelligence_job(&paths, &config, None, job_id, job_type)
            .expect("execute typed direct job");
        assert!(completed);
    }

    let drained_job_id = enqueue_core_intelligence_job(
        &connection,
        VISIT_DERIVE_JOB_TYPE,
        &CoreIntelligenceRebuildRequest {
            profile_id: Some("chrome:drained-priority".to_string()),
            full_rebuild: true,
            limit: Some(1),
        },
        "coverage priority drain",
    )
    .expect("enqueue priority drain job");
    assert!(
        drain_one_priority_intelligence_job(&paths, None).expect("drain one priority job"),
        "priority drain must claim queued deterministic work"
    );
    let drained_state: String = connection
        .query_row("SELECT state FROM intelligence_jobs WHERE id = ?1", [drained_job_id], |row| {
            row.get(0)
        })
        .expect("drained priority job state");
    assert_ne!(drained_state, "queued");

    let now = chrono::Utc::now().to_rfc3339();
    let unknown_payload = serde_json::json!({
        "jobType": "unknown-runtime-kind",
        "request": {
            "profileId": "chrome:unknown-runtime-kind",
            "fullRebuild": true,
            "limit": 1
        },
        "reason": "coverage unknown job type"
    });
    connection
        .execute(
            "INSERT INTO intelligence_jobs (
                job_type, state, priority, attempt, dedupe_key, payload_json,
                artifact_json, created_at, scheduled_at, updated_at
             )
             VALUES ('unknown-runtime-kind', 'queued', 1, 0, 'coverage-unknown-runtime-kind', ?1, '{}', ?2, ?2, ?2)",
            rusqlite::params![unknown_payload.to_string(), now],
        )
        .expect("insert unknown direct job");
    let unknown_id = connection.last_insert_rowid();
    let unknown_error =
        execute_core_intelligence_job(&paths, &config, None, unknown_id, "unknown-runtime-kind")
            .expect_err("unknown job type should fail and mark the row failed");
    assert!(unknown_error.to_string().contains("unknown-runtime-kind"));
    let unknown_state: String = connection
        .query_row("SELECT state FROM intelligence_jobs WHERE id = ?1", [unknown_id], |row| {
            row.get(0)
        })
        .expect("unknown job state");
    assert_eq!(unknown_state, "failed");

    unsafe {
        std::env::remove_var(PROJECT_ROOT_OVERRIDE_ENV);
        std::env::remove_var(CHROME_USER_DATA_OVERRIDE_ENV);
        std::env::remove_var(TEST_KEYRING_OVERRIDE_ENV);
    }
}

#[cfg(coverage)]
#[test]
fn coverage_intelligence_queue_drain_covers_error_and_enrichment_lanes() {
    let _guard = lock_env();
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
    save_user_config(&config, None).expect("save config");
    let paths = project_paths().expect("project paths");

    let invalid_root = dir.path().join("invalid-runtime-config");
    fs::create_dir_all(&invalid_root).expect("invalid root");
    fs::write(invalid_root.join("config.json"), "{not-json").expect("invalid config");
    let invalid_paths = vault_core::config::project_paths_with_root(&invalid_root);
    assert!(drain_one_priority_intelligence_job(&invalid_paths, None).is_err());
    assert!(drain_one_enrichment_intelligence_job(&invalid_paths, None).is_err());

    let connection = vault_core::archive::open_intelligence_connection(&paths, &config, None)
        .expect("intelligence connection");
    let now = chrono::Utc::now().to_rfc3339();
    connection
        .execute(
            "INSERT INTO intelligence_jobs (
                job_type, plugin_id, run_id, state, priority, attempt, dedupe_key,
                payload_json, artifact_json, created_at, scheduled_at, updated_at
             )
             VALUES (?1, ?2, ?3, 'queued', 10, 0, ?4, ?5, '{}', ?6, ?6, ?6)",
            rusqlite::params![
                "enrichment-plugin",
                TITLE_NORMALIZATION_PLUGIN_ID,
                777_i64,
                "coverage-enrichment:999",
                serde_json::json!({
                    "historyId": 999,
                    "profileId": "chrome:Default",
                    "url": "https://example.com/missing",
                    "title": "Missing history row"
                })
                .to_string(),
                now,
            ],
        )
        .expect("insert queued enrichment job");

    assert!(
        drain_one_enrichment_intelligence_job(&paths, None).expect("drain one enrichment job"),
        "enrichment drain must claim queued enrichment work"
    );
    let state: String = connection
        .query_row(
            "SELECT state FROM intelligence_jobs WHERE dedupe_key = ?1",
            ["coverage-enrichment:999"],
            |row| row.get(0),
        )
        .expect("enrichment job state");
    assert_ne!(state, "queued");

    unsafe {
        std::env::remove_var(PROJECT_ROOT_OVERRIDE_ENV);
        std::env::remove_var(CHROME_USER_DATA_OVERRIDE_ENV);
        std::env::remove_var(TEST_KEYRING_OVERRIDE_ENV);
    }
}

#[cfg(coverage)]
#[test]
fn coverage_run_backup_now_surfaces_remote_and_index_failures_as_warnings() {
    let _guard = lock_env();
    let dir = tempdir().expect("tempdir");
    let chrome_root = chrome_user_data_fixture(dir.path());
    let keyring_root = dir.path().join("test-keyring");
    let bin_dir = dir.path().join("fake-bin");
    let curl_path =
        install_fake_curl(&bin_dir, "#!/bin/sh\necho 'upload failed from worker' >&2\nexit 23\n");

    unsafe {
        std::env::set_var(PROJECT_ROOT_OVERRIDE_ENV, dir.path());
        std::env::set_var(CHROME_USER_DATA_OVERRIDE_ENV, &chrome_root);
        std::env::set_var(TEST_KEYRING_OVERRIDE_ENV, &keyring_root);
        std::env::set_var("BHB_TEST_CURL_BIN", &curl_path);
    }

    let mut config = configured_ai_config();
    config.remote_backup.enabled = true;
    config.remote_backup.bucket = "worker-tests".to_string();
    config.remote_backup.region = "us-west-2".to_string();
    config.remote_backup.upload_after_backup = true;
    config.ai.auto_index_after_backup = true;
    config.ai.embedding_providers[0].default_model.clear();
    initialize_archive_database(&config, None).expect("initialize archive");
    save_user_config(&config, None).expect("save config");

    store_ai_provider_api_key(
        &AiProviderSecretInput {
            provider_id: "embed-primary".to_string(),
            api_key: "embed-secret".to_string(),
        },
        None,
    )
    .expect("store embed key");
    store_s3_credentials(&S3CredentialInput {
        access_key_id: "akid".to_string(),
        secret_access_key: "secret".to_string(),
    })
    .expect("store s3 creds");

    let report = run_backup_now(None, false).expect("backup with failing follow-up tasks");
    let remote = report.remote_backup.expect("remote backup report");
    assert!(!remote.uploaded);
    assert!(report.warnings.iter().any(|warning| warning.contains("upload failed from worker")));
    let mut queue = load_ai_queue(None).expect("load ai queue");
    for _ in 0..50 {
        if queue.failed > 0 {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
        queue = load_ai_queue(None).expect("reload ai queue");
    }
    assert!(queue.failed > 0);

    unsafe {
        std::env::remove_var(PROJECT_ROOT_OVERRIDE_ENV);
        std::env::remove_var(CHROME_USER_DATA_OVERRIDE_ENV);
        std::env::remove_var(TEST_KEYRING_OVERRIDE_ENV);
        std::env::remove_var("BHB_TEST_CURL_BIN");
    }
}

#[cfg(coverage)]
#[test]
fn coverage_browser_import_and_paused_auto_index_cover_archive_followups() {
    let _guard = lock_env();
    let dir = tempdir().expect("tempdir");
    let chrome_root = chrome_user_data_fixture(dir.path());
    let keyring_root = dir.path().join("test-keyring");

    unsafe {
        std::env::set_var(PROJECT_ROOT_OVERRIDE_ENV, dir.path());
        std::env::set_var(CHROME_USER_DATA_OVERRIDE_ENV, &chrome_root);
        std::env::set_var(TEST_KEYRING_OVERRIDE_ENV, &keyring_root);
    }

    let mut config = configured_ai_config();
    config.ai.auto_index_after_backup = true;
    config.ai.job_queue_paused = true;
    initialize_archive_database(&config, None).expect("initialize archive");
    save_user_config(&config, None).expect("save config");
    store_ai_provider_api_key(
        &AiProviderSecretInput {
            provider_id: "embed-primary".to_string(),
            api_key: "embed-secret".to_string(),
        },
        None,
    )
    .expect("store embed key");

    let browser_history_request = BrowserHistoryImportRequest {
        source_path: chrome_root.join("Default").join("History").display().to_string(),
        dry_run: false,
        browser_family: Some("chromium".to_string()),
        profile_id: Some("chrome:Default".to_string()),
        browser_name: Some("Chrome".to_string()),
        profile_name: Some("Default".to_string()),
    };
    let imported = import_browser_history_source(None, &browser_history_request)
        .expect("browser direct import");
    assert_eq!(imported.imported_items, 1);
    assert!(imported.notes.iter().any(|note| note.contains("Core Intelligence refresh jobs")));

    let backup = run_backup_now(None, false).expect("backup with paused auto-index");
    assert!(backup.warnings.iter().any(|warning| warning.contains("AI auto-index queued job")));

    unsafe {
        std::env::remove_var(PROJECT_ROOT_OVERRIDE_ENV);
        std::env::remove_var(CHROME_USER_DATA_OVERRIDE_ENV);
        std::env::remove_var(TEST_KEYRING_OVERRIDE_ENV);
    }
}

#[cfg(coverage)]
#[test]
fn coverage_dashboard_and_ai_follow_up_helpers_cover_success_and_error_paths() {
    let _guard = lock_env();
    let dir = tempdir().expect("tempdir");
    let chrome_root = chrome_user_data_fixture(dir.path());
    let keyring_root = dir.path().join("test-keyring");

    unsafe {
        std::env::set_var(PROJECT_ROOT_OVERRIDE_ENV, dir.path());
        std::env::set_var(CHROME_USER_DATA_OVERRIDE_ENV, &chrome_root);
        std::env::set_var(TEST_KEYRING_OVERRIDE_ENV, &keyring_root);
    }

    let config = configured_ai_config();
    initialize_archive_database(&config, Some("vault-passphrase")).expect("initialize archive");
    save_user_config(&config, Some("vault-passphrase")).expect("save config");
    let backup = run_backup_now(Some("vault-passphrase"), false).expect("backup");
    let run_id = backup.run.expect("backup run").id;

    let dashboard = dashboard_snapshot(Some("vault-passphrase")).expect("load dashboard snapshot");
    assert_eq!(dashboard.total_visits, 1);
    let detail = audit_run_detail(Some("vault-passphrase"), run_id).expect("load audit detail");
    assert_eq!(detail.run.id, run_id);

    let doctor = doctor_report(Some("vault-passphrase")).expect("doctor report");
    assert!(!doctor.checks.is_empty());
    let repair = repair_health(Some("vault-passphrase")).expect("repair health");
    assert!(repair.run_id.is_none() || repair.run_id >= Some(run_id));

    let provider_error = test_ai_provider_connection_report(
        Some("vault-passphrase"),
        &AiProviderConnectionTestRequest {
            provider_id: "embed-primary".to_string(),
            purpose: AiProviderPurpose::Embedding,
        },
    )
    .expect("provider connection should surface missing secrets in the report");
    assert!(!provider_error.ok);
    assert_eq!(provider_error.error_code.as_deref(), Some("secret-missing"));

    let queue = load_ai_queue(Some("vault-passphrase")).expect("load ai queue");
    assert_eq!(queue.queued, 0);
    let drained = run_ai_queue_jobs(Some("vault-passphrase"), None).expect("run empty ai queue");
    assert_eq!(drained.queued, 0);

    let replay = replay_ai_job(Some("vault-passphrase"), 999)
        .expect_err("replay should fail for a missing job");
    assert!(replay.to_string().contains("999"));
    let cancel = cancel_ai_job(Some("vault-passphrase"), 999)
        .expect_err("cancel should fail for a missing job");
    assert!(cancel.to_string().contains("999"));
    let assistant_job = load_ai_assistant_job(Some("vault-passphrase"), 999)
        .expect_err("assistant job should not exist");
    assert!(assistant_job.to_string().contains("999"));

    let run_report = run_core_intelligence_now(
        Some("vault-passphrase"),
        &CoreIntelligenceRebuildRequest::default(),
    )
    .expect("core intelligence run should complete");
    assert!(!run_report.last_run_at.is_empty());
    let sessions = get_sessions(
        Some("vault-passphrase"),
        &PagedDateRangeRequest {
            date_range: vault_core::DateRange {
                start: "1970-01-01".to_string(),
                end: "2100-01-01".to_string(),
            },
            profile_id: None,
            page: 0,
            page_size: 10,
        },
    )
    .expect("sessions should load after core intelligence rebuild");
    assert!(sessions.total >= 1);

    let date_range =
        vault_core::DateRange { start: "1970-01-01".to_string(), end: "2100-01-01".to_string() };
    let scoped =
        vault_core::ScopedDateRangeRequest { date_range: date_range.clone(), profile_id: None };
    let paged = vault_core::PagedDateRangeRequest {
        date_range: date_range.clone(),
        profile_id: None,
        page: 0,
        page_size: 10,
    };
    let top_sites = vault_core::TopSitesRequest {
        date_range: date_range.clone(),
        profile_id: None,
        sort_by: None,
        limit: Some(10),
    };
    let embed_cards = vault_core::IntelligenceEmbedCardsRequest {
        date_range: date_range.clone(),
        profile_id: None,
        limit: Some(4),
    };

    let _ = get_search_trails(
        Some("vault-passphrase"),
        &vault_core::SearchTrailQueryRequest {
            date_range: date_range.clone(),
            profile_id: None,
            engine: None,
            page: 0,
            page_size: 10,
        },
    );
    let _ = get_trail_detail(Some("vault-passphrase"), "missing-trail");
    let _ = get_session_detail(Some("vault-passphrase"), "missing-session");
    let _ = get_navigation_path(Some("vault-passphrase"), 1);
    let _ = get_hub_pages(Some("vault-passphrase"), &top_sites);
    let _ = get_search_engine_ranking(Some("vault-passphrase"), &scoped);
    let _ = list_search_engine_rules(Some("vault-passphrase"));
    let custom_rules = upsert_search_engine_rule(
        Some("vault-passphrase"),
        &vault_core::SearchEngineRuleInput {
            rule_id: Some("fixture-search".to_string()),
            engine_id: "fixture".to_string(),
            display_name: "Fixture Search".to_string(),
            host_pattern: "search.example".to_string(),
            path_prefix: Some("/search".to_string()),
            query_param_key: "q".to_string(),
            enabled: true,
            note: Some("coverage fixture".to_string()),
            example_url: Some("https://search.example/search?q=pathkeep".to_string()),
        },
    )
    .expect("upsert search rule");
    assert!(custom_rules.iter().any(|rule| rule.rule_id == "fixture-search"));
    let deleted_rules = delete_search_engine_rule(Some("vault-passphrase"), "fixture-search")
        .expect("delete search rule");
    assert!(!deleted_rules.iter().any(|rule| rule.rule_id == "fixture-search"));
    let _ = get_intelligence_primary_overview(Some("vault-passphrase"), &scoped);
    let _ = get_top_search_concepts(
        Some("vault-passphrase"),
        &vault_core::TopSearchConceptsRequest {
            date_range: date_range.clone(),
            profile_id: None,
            limit: Some(10),
        },
    );
    let _ = get_search_queries(
        Some("vault-passphrase"),
        &vault_core::SearchQueryListRequest {
            date_range: date_range.clone(),
            profile_id: None,
            browser_kind: None,
            engine: None,
            domain: None,
            query: None,
            sort: None,
            page: 0,
            page_size: 10,
        },
    );
    let _ = get_query_families(Some("vault-passphrase"), &paged);
    let _ = get_query_family_detail(
        Some("vault-passphrase"),
        &vault_core::QueryFamilyDetailRequest {
            family_id: "missing-family".to_string(),
            date_range: date_range.clone(),
            profile_id: None,
        },
    );
    let _ = get_top_sites(Some("vault-passphrase"), &top_sites);
    let _ = get_domain_trend(
        Some("vault-passphrase"),
        &vault_core::DomainTrendRequest {
            registrable_domain: "example.com".to_string(),
            date_range: date_range.clone(),
        },
    );
    let _ = get_refind_page_detail(
        Some("vault-passphrase"),
        &vault_core::RefindPageDetailRequest {
            canonical_url: "https://example.com/".to_string(),
            date_range: date_range.clone(),
            profile_id: None,
        },
    );
    let _ = explain_refind(
        Some("vault-passphrase"),
        &vault_core::ExplainRefindRequest { canonical_url: "https://example.com/".to_string() },
    );
    let _ = explain_entity(
        Some("vault-passphrase"),
        &vault_core::EntityExplanationRequest {
            entity_type: "domain".to_string(),
            entity_id: "example.com".to_string(),
        },
    );
    let _ = get_activity_mix(Some("vault-passphrase"), &scoped);
    let _ = get_activity_mix_trend(
        Some("vault-passphrase"),
        &vault_core::GranularityDateRangeRequest {
            date_range: date_range.clone(),
            profile_id: None,
            granularity: "day".to_string(),
        },
    );
    let _ = get_digest_summary(Some("vault-passphrase"), &scoped);
    let _ = get_stable_sources(Some("vault-passphrase"), &scoped);
    let _ = get_search_effectiveness(
        Some("vault-passphrase"),
        &vault_core::SearchEffectivenessRequest {
            date_range: date_range.clone(),
            profile_id: None,
            engine: None,
        },
    );
    let _ = get_friction_signals(Some("vault-passphrase"), &scoped);
    let _ = get_reopened_investigations(Some("vault-passphrase"), &scoped);
    let _ = get_domain_deep_dive(
        Some("vault-passphrase"),
        &vault_core::DomainDeepDiveRequest {
            registrable_domain: "example.com".to_string(),
            date_range: date_range.clone(),
            profile_id: None,
        },
    );
    let _ = get_day_insights(
        Some("vault-passphrase"),
        &vault_core::DayInsightsRequest { date: "2026-04-01".to_string(), profile_id: None },
    );
    let _ = get_browsing_rhythm(
        Some("vault-passphrase"),
        &vault_core::CategoryFilteredDateRangeRequest {
            date_range: date_range.clone(),
            profile_id: None,
            category: None,
        },
    );
    let _ = get_discovery_trend(
        Some("vault-passphrase"),
        &vault_core::GranularityDateRangeRequest {
            date_range: date_range.clone(),
            profile_id: None,
            granularity: "week".to_string(),
        },
    );
    let _ = get_on_this_day(Some("vault-passphrase"), None);
    let _ = get_intelligence_embed_cards(Some("vault-passphrase"), &embed_cards);
    let _ = get_intelligence_widget_snapshot(Some("vault-passphrase"), &embed_cards);
    let _ = get_intelligence_public_snapshot(Some("vault-passphrase"), &scoped);
    let local_host_request = vault_core::IntelligenceLocalHostRequest {
        date_range: date_range.clone(),
        profile_id: None,
        locale: "en".to_string(),
    };
    let _ = preview_intelligence_local_host(Some("vault-passphrase"), &local_host_request);
    let _ = build_intelligence_local_host(Some("vault-passphrase"), &local_host_request);
    let _ = get_breadth_index(Some("vault-passphrase"), &scoped);
    let _ = get_habit_patterns(Some("vault-passphrase"), &scoped);
    let _ = get_interrupted_habits(
        Some("vault-passphrase"),
        &vault_core::ProfileScopedRequest { profile_id: None },
    );
    let _ = get_path_flows(
        Some("vault-passphrase"),
        &vault_core::PathFlowRequest {
            date_range: date_range.clone(),
            profile_id: None,
            step_count: 2,
            limit: Some(10),
        },
    );
    let _ = get_observed_interactions(Some("vault-passphrase"), &scoped);
    let _ = get_compare_sets(Some("vault-passphrase"), &scoped);
    let _ = get_compare_set_detail(
        Some("vault-passphrase"),
        &vault_core::CompareSetDetailRequest {
            compare_set_id: "missing-compare-set".to_string(),
            date_range: date_range.clone(),
            profile_id: None,
        },
    );
    let _ = get_multi_browser_diff(Some("vault-passphrase"), &scoped);
    let _ = get_intelligence_secondary_overview(Some("vault-passphrase"), &scoped);

    let refind_pages = get_refind_pages(
        Some("vault-passphrase"),
        &vault_core::RefindPagesRequest {
            date_range: vault_core::DateRange {
                start: "1970-01-01".to_string(),
                end: "2100-01-01".to_string(),
            },
            profile_id: None,
            limit: Some(10),
        },
    )
    .expect("refind pages should load");
    if let Some(page) = refind_pages.data.first() {
        let explain_report = explain_refind(
            Some("vault-passphrase"),
            &vault_core::ExplainRefindRequest { canonical_url: page.canonical_url.clone() },
        )
        .expect("explain refind should work from the persisted surface");
        assert!(!explain_report.factors.is_empty());
    }

    unsafe {
        std::env::remove_var(PROJECT_ROOT_OVERRIDE_ENV);
        std::env::remove_var(CHROME_USER_DATA_OVERRIDE_ENV);
        std::env::remove_var(TEST_KEYRING_OVERRIDE_ENV);
    }
}

#[test]
fn security_status_keeps_last_rekey_review_visible_when_archive_is_locked() {
    let _guard = lock_env();
    let dir = tempdir().expect("tempdir");
    let chrome_root = chrome_user_data_fixture(dir.path());

    unsafe {
        std::env::set_var(PROJECT_ROOT_OVERRIDE_ENV, dir.path());
        std::env::set_var(CHROME_USER_DATA_OVERRIDE_ENV, &chrome_root);
    }

    let config = initialized_config();
    initialize_archive_database(&config, None).expect("initialize archive");
    save_user_config(&config, None).expect("save config");

    rekey_archive_database(
        None,
        &RekeyRequest {
            new_mode: ArchiveMode::Encrypted,
            new_key: Some("vault-passphrase".to_string()),
        },
    )
    .expect("rekey archive");

    let security = security_status(None).expect("security status while locked");
    assert_eq!(security.mode, "locked");
    assert!(security.last_rekey_run_id.is_some());
    assert!(security.last_rekey_at.is_some());
    assert!(
        security
            .last_rekey_snapshot_path
            .as_deref()
            .is_some_and(|path| path.contains("archive-before-rekey"))
    );
    let locked_preview = preview_rekey_archive(
        None,
        &RekeyRequest { new_mode: ArchiveMode::Plaintext, new_key: None },
    )
    .expect("locked rekey preview");
    assert!(locked_preview.warnings.iter().any(|warning| { warning.contains("currently locked") }));

    unsafe {
        std::env::remove_var(PROJECT_ROOT_OVERRIDE_ENV);
        std::env::remove_var(CHROME_USER_DATA_OVERRIDE_ENV);
    }
}
