//! Regression tests for the worker orchestration facade.
use super::*;
use rusqlite::Connection;
use std::fs;
#[cfg(coverage)]
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, OnceLock};
use tempfile::tempdir;
use vault_core::{
    AiAssistantRequest, AiIndexRequest, AiProviderConfig, AiProviderPurpose, AiProviderSecretInput,
    AiRequestFormat, AiSearchResponse, AppConfig, ArchiveMode, ExportFormat, ExportRequest,
    HealthReport, HistoryQuery, PagedDateRangeRequest, S3CredentialInput,
    SetAppLockPasscodeRequest, TakeoutRequest, project_paths, utils::iso_to_chrome_time_micros,
};
#[cfg(coverage)]
use vault_core::{
    AiIndexReport, AiProviderConnectionTestRequest, AiSearchRequest,
    CoreIntelligenceRebuildRequest, RemoteBackupResult,
};
use vault_platform::keyring_set_provider_api_key;

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
fn app_snapshot_degrades_when_browser_discovery_fails() {
    let _guard = lock_env();
    let dir = tempdir().expect("tempdir");
    let broken_chrome_root = broken_chrome_user_data_fixture(dir.path());
    let keyring_root = dir.path().join("test-keyring");

    unsafe {
        std::env::set_var(PROJECT_ROOT_OVERRIDE_ENV, dir.path());
        std::env::set_var(CHROME_USER_DATA_OVERRIDE_ENV, &broken_chrome_root);
        std::env::set_var(TEST_KEYRING_OVERRIDE_ENV, &keyring_root);
    }

    let config = initialized_config();
    let snapshot = initialize_archive_database(&config, None).expect("initialize archive");
    assert!(snapshot.archive_status.initialized);
    assert!(snapshot.browser_profiles.is_empty());
    assert_eq!(
        snapshot.directories.config_path,
        dir.path().join("config.json").display().to_string()
    );

    unsafe {
        std::env::remove_var(PROJECT_ROOT_OVERRIDE_ENV);
        std::env::remove_var(CHROME_USER_DATA_OVERRIDE_ENV);
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
    let applied = apply_schedule_plan(&preview).expect("apply schedule");
    assert!(!applied.applied);
    let removed = remove_schedule_plan(&preview).expect("remove schedule");
    assert!(!removed.applied);
    let schedule = schedule_status(None, Some("windows"), Some(PathBuf::from("/tmp/bhb")))
        .expect("schedule status");
    assert_eq!(schedule.install_state, "manual-review");
    assert!(!schedule.warnings.is_empty());

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
    assert!(answer.answer.contains("stub answer"));

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

    let remote_json = run_worker_cli(&["remote-backup".to_string()]).expect("remote backup cli");
    let remote: RemoteBackupResult =
        serde_json::from_str(&remote_json).expect("parse remote result");
    assert!(remote.uploaded);

    let index_json = run_worker_cli(&["ai-index".to_string()]).expect("ai-index cli");
    let index_report: AiIndexReport =
        serde_json::from_str(&index_json).expect("parse ai index report");
    assert!(!index_report.provider_id.is_empty());

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

    unsafe {
        std::env::remove_var(PROJECT_ROOT_OVERRIDE_ENV);
        std::env::remove_var(CHROME_USER_DATA_OVERRIDE_ENV);
    }
}
