//! Responsibilities: verify worker requests against their real storage and service boundaries.
//! Not responsible for: frontend rendering, desktop window lifecycle or provider network integration.
//! Failure modes: incorrect owner routing, lost progress or audit records, leaked locked
//! sessions, a search assertion races index publication, reverted facts remain visible,
//! startup skips required conversion, App Lock blocks scheduled backups, or
//! password/keychain changes lose access.
use super::*;
use crate::owner_requests::test_client::run_worker_cli;
use rusqlite::Connection;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, OnceLock};
use tempfile::tempdir;
use vault_core::{
    AiIndexRequest, AiProviderConfig, AiProviderPurpose, AiRequestFormat, AppConfig, ArchiveMode,
    BrowserHistoryImportRequest, SetAppLockPasscodeRequest, UnlockAppSessionRequest, project_paths,
    utils::iso_to_chrome_time_micros,
};
use vault_platform::{keyring_set_provider_api_key, provider_api_key_saved};

pub(crate) const PROJECT_ROOT_OVERRIDE_ENV: &str = "CHB_PROJECT_ROOT";
const CHROME_USER_DATA_OVERRIDE_ENV: &str = "CHB_CHROME_USER_DATA_DIR";
pub(crate) const TEST_KEYRING_OVERRIDE_ENV: &str = "CHB_TEST_KEYRING_DIR";

fn env_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

/// Restores the caller's isolated roots before releasing the process environment lock.
pub(crate) struct EnvGuard {
    _lock: MutexGuard<'static, ()>,
    saved: Vec<(&'static str, Option<std::ffi::OsString>)>,
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        for (name, value) in &self.saved {
            restore_env_var(name, value.as_deref());
        }
    }
}

pub(crate) fn lock_env() -> EnvGuard {
    let lock = env_lock().lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let saved = [
        PROJECT_ROOT_OVERRIDE_ENV,
        CHROME_USER_DATA_OVERRIDE_ENV,
        TEST_KEYRING_OVERRIDE_ENV,
        "CHB_FIREFOX_PROFILES_DIR",
        "CHB_SAFARI_ROOT",
        "PATHKEEP_PLATFORM_TEST_SANDBOX_DIR",
        "PATHKEEP_TEST_SAVE_DIR",
    ]
    .into_iter()
    .map(|name| (name, std::env::var_os(name)))
    .collect();
    EnvGuard { _lock: lock, saved }
}

pub(crate) fn restore_env_var(name: &str, value: Option<&std::ffi::OsStr>) {
    unsafe {
        if let Some(value) = value {
            std::env::set_var(name, value);
        } else {
            std::env::remove_var(name);
        }
    }
}

pub(crate) fn initialized_config() -> AppConfig {
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

/// An archive still in the old SQLCipher format, as an install from before
/// Turso has it; launch recovery and conversion start from this.
fn legacy_archive(config: &AppConfig, key: Option<&str>) {
    let paths = project_paths().unwrap();
    drop(vault_convert_sqlcipher::test_support::open_archive(&paths, config, key));
}

pub(crate) fn configured_ai_config() -> AppConfig {
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

pub(crate) fn chrome_user_data_fixture(root: &Path) -> PathBuf {
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

#[test]
fn scheduled_worker_records_keyring_config_and_archive_open_failures_before_archive_ledger() {
    let _guard = lock_env();
    let root = tempdir().expect("tempdir");
    let keyring_root = root.path().join("keyring");
    let original_project_root = std::env::var_os(PROJECT_ROOT_OVERRIDE_ENV);
    let original_keyring_root = std::env::var_os(TEST_KEYRING_OVERRIDE_ENV);
    unsafe {
        std::env::set_var(PROJECT_ROOT_OVERRIDE_ENV, root.path());
        std::env::set_var(TEST_KEYRING_OVERRIDE_ENV, &keyring_root);
    }

    let encrypted_config =
        AppConfig { initialized: true, archive_mode: ArchiveMode::Encrypted, ..Default::default() };
    vault_core::save_config(&project_paths().unwrap(), &encrypted_config).unwrap();
    fs::create_dir_all(&keyring_root).expect("keyring root");
    let secret_path = keyring_root
        .join(format!("{}-database-key.secret", vault_platform::test_support::keyring_service()));
    fs::create_dir(&secret_path).expect("directory blocks secret read");
    let keyring_error = run_worker_cli(&["backup".to_string(), "--due-only".to_string()])
        .expect_err("keyring read must fail");
    assert!(keyring_error.to_string().contains("reading"));
    let paths = project_paths().expect("paths");
    let keyring_ledger =
        vault_core::load_schedule_attempt_ledger(&paths, 1.0, None).expect("keyring ledger");
    assert_eq!(keyring_ledger.attempts[0].reason_code.as_deref(), Some("keyring"));
    fs::remove_dir(&secret_path).expect("remove blocking secret");

    fs::create_dir_all(root.path()).expect("root");
    fs::write(&paths.config_path, b"{broken-json").expect("corrupt config");
    let config_error = run_worker_cli(&["backup".to_string(), "--due-only".to_string()])
        .expect_err("config parse must fail");
    assert!(config_error.to_string().contains("parsing config json"));
    let config_ledger =
        vault_core::load_schedule_attempt_ledger(&paths, 1.0, None).expect("config ledger");
    assert_eq!(config_ledger.attempts[0].reason_code.as_deref(), Some("config"));

    let config = AppConfig {
        initialized: true,
        selected_profile_ids: vec!["chrome:Default".to_string()],
        ..AppConfig::default()
    };
    vault_core::save_config(&paths, &config).expect("valid config");
    fs::create_dir_all(paths.legacy.archive.parent().expect("archive parent"))
        .expect("archive parent");
    fs::write(paths.app_root.join("archive/history-vault.db"), b"not turso")
        .expect("invalid archive");
    let archive_error = run_worker_cli(&["backup".to_string(), "--due-only".to_string()])
        .expect_err("archive open must fail");
    assert!(!archive_error.to_string().is_empty());
    let archive_ledger =
        vault_core::load_schedule_attempt_ledger(&paths, 1.0, None).expect("archive ledger");
    assert_eq!(archive_ledger.attempts[0].reason_code.as_deref(), Some("archive-open"));

    restore_env_var(PROJECT_ROOT_OVERRIDE_ENV, original_project_root.as_deref());
    restore_env_var(TEST_KEYRING_OVERRIDE_ENV, original_keyring_root.as_deref());
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
}

#[test]
fn mcp_surface_respects_native_visibility_and_locked_app_sessions() {
    let vault = vault_core::Vault::open_temp().unwrap();
    let _guard = lock_env();
    let dir = tempdir().expect("tempdir");
    let chrome_root = chrome_user_data_fixture(dir.path());
    let keyring_root = dir.path().join("test-keyring");

    unsafe {
        std::env::set_var(PROJECT_ROOT_OVERRIDE_ENV, &vault.paths().app_root);
        std::env::set_var(CHROME_USER_DATA_OVERRIDE_ENV, &chrome_root);
        std::env::set_var(TEST_KEYRING_OVERRIDE_ENV, &keyring_root);
    }

    let mut config = configured_ai_config();
    config.ai.job_queue_paused = true;
    vault_core::save_config(vault.paths(), &config).expect("save native config");
    Connection::open(chrome_root.join("Default/History"))
        .unwrap()
        .execute("UPDATE urls SET title='Imported',url='https://example.com/import'", [])
        .unwrap();
    let imported = import_browser_history_source(
        &vault,
        None,
        &BrowserHistoryImportRequest {
            source_path: chrome_root.join("Default/History").display().to_string(),
            dry_run: false,
            browser_family: Some("chromium".into()),
            profile_id: Some("chrome:Default".into()),
            browser_name: Some("Chrome".into()),
            profile_name: Some("Default".into()),
        },
    )
    .expect("native browser import");
    let batch_id = imported.import_batch.unwrap().id;
    // Publish the imported document before asserting lexical visibility. The
    // revert below must still hide it before the next search-index cycle.
    vault.search().cycle().unwrap();
    let visible = mcp_search_result(
        &vault,
        None,
        McpSearchRequest {
            cursor: None,
            query: "Imported".to_string(),
            profile_id: None,
            domain: None,
            limit: Some(10),
        },
    )
    .expect("visible mcp search");
    assert_eq!(visible.total, 1);

    revert_import_batch_detail(&vault, batch_id).expect("hide native import");
    let hidden = mcp_search_result(
        &vault,
        None,
        McpSearchRequest {
            cursor: None,
            query: "Imported".to_string(),
            profile_id: None,
            domain: None,
            limit: Some(10),
        },
    )
    .expect("hidden mcp search");
    assert_eq!(hidden.total, 0);

    // Transparency contract: every external touch — both tools — lands an
    // auditable `mcp_query` run the user can inspect. The status probe audits
    // too, so this call seeds an `archive-status` row alongside the two
    // `search-history` rows above.
    let unlocked_status =
        mcp_archive_status_result(Some(&vault), None).expect("unlocked archive status");
    assert!(unlocked_status.unlocked, "archive must read while unlocked");

    {
        let entries = vault
            .store()
            .read(vault_store::Db::Archive, |s| {
                s.query(
                    "SELECT trigger,stats_json FROM runs WHERE run_type='mcp_query' ORDER BY id",
                    (),
                    |row| Ok((row.get::<String>(0)?, row.get::<String>(1)?)),
                )
            })
            .unwrap();
        // Two searches + one status probe = three external touches recorded.
        assert_eq!(entries.len(), 3, "every external MCP touch must be audited");
        assert!(
            entries.iter().all(|(trigger, _)| trigger == "external"),
            "MCP runs must be tagged as external triggers"
        );
        let tools: Vec<String> = entries
            .iter()
            .map(|(_, stats_json)| {
                let stats: serde_json::Value =
                    serde_json::from_str(stats_json).expect("parse stats json");
                stats["tool"].as_str().expect("tool label").to_string()
            })
            .collect();
        assert_eq!(
            tools,
            vec!["search-history", "search-history", "archive-status"],
            "audit must distinguish which tool ran"
        );
        // The bounded query summary is present, but the SQLCipher key (and any
        // raw archive row) never crosses into the audit trace.
        let first_search: serde_json::Value =
            serde_json::from_str(&entries[0].1).expect("parse search stats");
        assert_eq!(first_search["query"], "Imported");
        assert_eq!(first_search["total"], 1);
        for (_, stats_json) in &entries {
            let lowered = stats_json.to_lowercase();
            assert!(
                !lowered.contains("databasekey")
                    && !lowered.contains("apikey")
                    && !lowered.contains("secret")
                    && !lowered.contains("passcode"),
                "audit must never record the database key or any secret material"
            );
        }
    }

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

    // Server-side lock enforcement: while locked, the credential-mutation and
    // settings-save commands must be refused, so the lock cannot be replaced,
    // removed, or disabled without first unlocking with the current passcode.
    assert!(
        configure_app_lock_passcode(&SetAppLockPasscodeRequest {
            passcode: "9999".to_string(),
            recovery_hint: None,
        })
        .is_err(),
        "configuring a passcode must be refused while locked"
    );
    assert!(
        remove_app_lock_passcode().is_err(),
        "removing the passcode must be refused while locked"
    );
    assert!(
        save_user_config(&locked_config, None).is_err(),
        "saving settings must be refused while locked"
    );
    let biometric_unlock =
        unlock_app_ui_session(&UnlockAppSessionRequest { passcode: None, use_biometric: true })
            .expect_err("unsupported biometric unlock should fail");
    assert!(biometric_unlock.to_string().contains("Biometric"));

    let search_error = mcp_search_result(
        &vault,
        None,
        McpSearchRequest {
            cursor: None,
            query: "example".to_string(),
            profile_id: None,
            domain: None,
            limit: Some(10),
        },
    )
    .expect_err("locked app should reject mcp search");
    assert!(search_error.to_string().contains("currently locked"));

    let archive_status =
        mcp_archive_status_result(Some(&vault), None).expect("locked archive status");
    assert!(!archive_status.unlocked);
    assert_eq!(archive_status.warning.as_deref(), Some("PathKeep is currently locked."));

    // The CLI connects to this already-open owner, as it does in the app.
    let owner = vault.clone();
    let _server =
        vault_owner_ipc::Server::start(&vault.paths().app_root, move |request, progress| {
            crate::owner_requests::dispatch(request, progress, Ok(None), || Ok(owner.clone()))
        })
        .unwrap();
    let cli_error = crate::run_worker_cli(&["mcp-server".to_string()])
        .expect_err("locked mcp server should fail");
    assert!(cli_error.to_string().contains("Unlock PathKeep"));

    let scheduled_json = crate::run_worker_cli(&["backup".to_string(), "--due-only".to_string()])
        .expect("App Lock must not block the OS-scheduled backup executor");
    let scheduled: vault_core::BackupReport =
        serde_json::from_str(&scheduled_json).expect("scheduled backup report");
    assert!(
        scheduled.due_skipped || scheduled.run.is_some(),
        "scheduled executor must reach a real skip or canonical run"
    );
    let paths = project_paths().expect("project paths");
    let ledger =
        vault_core::load_schedule_attempt_ledger(&paths, 1.0, None).expect("attempt ledger");
    assert!(
        matches!(
            ledger.attempts[0].outcome,
            vault_core::ScheduledBackupAttemptOutcome::Success
                | vault_core::ScheduledBackupAttemptOutcome::Skipped
        ),
        "App Lock must not turn a scheduled backup into a failure: {:?}",
        ledger.attempts[0]
    );
}

#[test]
fn mcp_per_call_gate_refuses_search_and_status_when_mcp_consent_is_revoked_mid_session() {
    let vault = vault_core::Vault::open_temp().unwrap();
    // The MCP face re-reads config fresh per call, so a user who turns OFF the MCP server in
    // Settings while an external tool still holds the stdio connection must stop being served. With a
    // working archive + an active connection, flipping `mcp_enabled` false makes the SAME in-flight
    // search/status entry points refuse with an honest message rather than continuing to return
    // history.
    let _guard = lock_env();
    let dir = tempdir().expect("tempdir");
    let chrome_root = chrome_user_data_fixture(dir.path());
    let keyring_root = dir.path().join("test-keyring");
    unsafe {
        std::env::set_var(PROJECT_ROOT_OVERRIDE_ENV, &vault.paths().app_root);
        std::env::set_var(CHROME_USER_DATA_OVERRIDE_ENV, &chrome_root);
        std::env::set_var(TEST_KEYRING_OVERRIDE_ENV, &keyring_root);
    }

    // MCP on + archive initialized: search and status serve normally.
    let mut config = configured_ai_config();
    config.ai.job_queue_paused = true;
    assert!(config.ai.mcp_enabled);
    vault_core::save_config(vault.paths(), &config).expect("save native config");
    save_user_config(&config, None).expect("save mcp-on config");

    let served = mcp_search_result(
        &vault,
        None,
        McpSearchRequest {
            cursor: None,
            query: "example".to_string(),
            profile_id: None,
            domain: None,
            limit: Some(5),
        },
    )
    .expect("search served while mcp consent is on");
    // It returned a result envelope (not an error), proving the served baseline.
    assert!(served.total >= served.items.len());
    mcp_archive_status_result(Some(&vault), None).expect("status served while mcp consent is on");

    // Revoke MCP consent mid-session (the same way Settings would persist the toggle).
    config.ai.mcp_enabled = false;
    save_user_config(&config, None).expect("save mcp-off config");

    let search_error = mcp_search_result(
        &vault,
        None,
        McpSearchRequest {
            cursor: None,
            query: "example".to_string(),
            profile_id: None,
            domain: None,
            limit: Some(5),
        },
    )
    .expect_err("search must refuse once MCP consent is revoked");
    let message = search_error.to_string();
    assert!(
        message.contains("Enable AI") && message.contains("MCP"),
        "honest, actionable refusal naming the MCP server: {message}"
    );

    let status_error = mcp_archive_status_result(Some(&vault), None)
        .expect_err("status must refuse once MCP consent is revoked");
    assert!(status_error.to_string().contains("MCP"), "status refusal names MCP");

    let guide_error = mcp_usage_guide_result(Some(&vault), None)
        .expect_err("usage guide must refuse once MCP consent is revoked");
    assert!(guide_error.to_string().contains("MCP"), "usage-guide refusal names MCP");

    // Turning the master AI switch off (even with the MCP sub-flag still on) also seals the face.
    config.ai.mcp_enabled = true;
    config.ai.enabled = false;
    save_user_config(&config, None).expect("save ai-off config");
    assert!(
        mcp_search_result(
            &vault,
            None,
            McpSearchRequest {
                cursor: None,
                query: "example".to_string(),
                profile_id: None,
                domain: None,
                limit: Some(5),
            }
        )
        .is_err(),
        "master AI off must seal the MCP search face even with the sub-flag on"
    );
}

#[test]
fn build_ai_index_refuses_a_reembed_without_semantic_consent_but_allows_clear() {
    let vault = vault_core::Vault::open_temp().unwrap();
    // A re-embed (build) must require the master AI switch AND the semantic-index sub-flag, so a
    // user with master ON but Smart search OFF cannot trigger an embedding job (token cost/egress +
    // ~59 GB derived vectors). A `clear_only` job is pure cleanup and stays allowed (reclaiming the
    // vectors after turning Smart search off is a legitimate action).
    let _guard = lock_env();
    let dir = tempdir().expect("tempdir");
    let original_root = std::env::var_os(PROJECT_ROOT_OVERRIDE_ENV);
    let original_keyring = std::env::var_os(TEST_KEYRING_OVERRIDE_ENV);
    unsafe {
        std::env::set_var(PROJECT_ROOT_OVERRIDE_ENV, dir.path());
        std::env::set_var(TEST_KEYRING_OVERRIDE_ENV, dir.path().join("test-keyring"));
    }

    let mut config = configured_ai_config();
    // Master AI ON, but the semantic-index (Smart search) sub-flag deliberately OFF.
    config.ai.semantic_index_enabled = false;
    config.ai.job_queue_paused = true; // keep the enqueue side-effect bounded; no background drain.
    initialize_archive_database(&config, None).expect("initialize archive");
    save_user_config(&config, None).expect("save config");
    keyring_set_provider_api_key("embed-primary", "embed-secret").expect("store embedding key");

    // The build path refuses with the honest, actionable message.
    let build_error = build_ai_index_now(&vault, None, &AiIndexRequest::default())
        .expect_err("re-embed without semantic consent must refuse");
    let message = build_error.to_string();
    assert!(
        message.contains("Enable AI") && message.contains("semantic"),
        "honest, actionable refusal naming the semantic index: {message}"
    );

    // The clear-only path is pure cleanup and stays permitted (no embedding job is enqueued).
    let cleared = build_ai_index_now(
        &vault,
        None,
        &AiIndexRequest { clear_only: true, ..AiIndexRequest::default() },
    )
    .expect("clear-only must be allowed even without the semantic sub-flag");
    assert!(cleared.job_id.is_some(), "a clear job was enqueued");

    restore_env_var(PROJECT_ROOT_OVERRIDE_ENV, original_root.as_deref());
    restore_env_var(TEST_KEYRING_OVERRIDE_ENV, original_keyring.as_deref());
}

/// Failure modes 1–3 of `current_password` at the worker entry points both
/// transports call: a wrong or missing current password changes nothing, and
/// the right one rekeys so that only the new password opens the archive.
#[test]
fn rekey_requires_the_current_password_and_rewraps_the_native_envelope() {
    let _guard = lock_env();
    let root = tempdir().unwrap();
    let old_env = std::env::var_os(PROJECT_ROOT_OVERRIDE_ENV);
    unsafe {
        std::env::set_var(PROJECT_ROOT_OVERRIDE_ENV, root.path());
    }
    let paths = project_paths().unwrap();
    let config = AppConfig {
        archive_mode: ArchiveMode::Encrypted,
        remember_database_key_in_keyring: false,
        ..initialized_config()
    };
    vault_core::save_config(&paths, &config).unwrap();
    let session = crate::VaultSession::default();
    session.open_password(paths.clone(), config, "old password".into()).unwrap();
    for current in [None, Some("wrong password")] {
        let request = RekeyRequest {
            new_mode: ArchiveMode::Encrypted,
            new_key: Some("new password".into()),
            current_key: current.map(str::to_owned),
            remember_in_keychain: None,
        };
        assert!(preview_rekey_archive(&request).is_err());
        assert!(rekey_archive_database(&session, &request).is_err());
    }
    let request = RekeyRequest {
        new_mode: ArchiveMode::Encrypted,
        new_key: Some("new password".into()),
        current_key: Some("old password".into()),
        remember_in_keychain: None,
    };
    assert!(preview_rekey_archive(&request).unwrap().snapshot_path.is_empty());
    rekey_archive_database(&session, &request).unwrap();
    assert!(vault_core::Vault::unlock_data_key(&paths, "old password").is_err());
    assert!(vault_core::Vault::unlock_data_key(&paths, "new password").is_ok());
    restore_env_var(PROJECT_ROOT_OVERRIDE_ENV, old_env.as_deref());
}

#[test]
fn launch_recovery_heals_a_stale_plaintext_config_so_the_archive_is_not_bricked() {
    // THE 2026-06-30 incident at the real launch entry point: an encrypted archive on disk while
    // config.json says Plaintext and there is NO journal. On HEAD `ensure_archive_initialized`
    // opens the encrypted file with no key -> NOTADB -> dead-end error page. With Phase C wired,
    // `initialize_archive_database` heals config -> Encrypted and surfaces the locked unlock
    // prompt instead of bricking.
    let _guard = lock_env();
    let dir = tempdir().expect("tempdir");
    let keyring_root = dir.path().join("test-keyring");
    let original_root = std::env::var_os(PROJECT_ROOT_OVERRIDE_ENV);
    let original_keyring = std::env::var_os(TEST_KEYRING_OVERRIDE_ENV);
    unsafe {
        std::env::set_var(PROJECT_ROOT_OVERRIDE_ENV, dir.path());
        std::env::set_var(TEST_KEYRING_OVERRIDE_ENV, &keyring_root);
    }

    // Materialise an encrypted archive on disk.
    let mut encrypted = initialized_config();
    encrypted.archive_mode = ArchiveMode::Encrypted;
    legacy_archive(&encrypted, Some("000000"));
    // The password opens it.
    drop(vault_convert_sqlcipher::test_support::open_archive(
        &project_paths().unwrap(),
        &encrypted,
        Some("000000"),
    ));

    // Overwrite config.json back to Plaintext while the file on disk stays encrypted (no journal):
    // the interrupted-rekey end state that bricked the open.
    let paths = project_paths().expect("project paths");
    let stale = initialized_config(); // Plaintext
    vault_core::save_config(&paths, &stale).expect("overwrite config to plaintext");

    // Re-launch with NO key: launch recovery heals config and surfaces the unlock prompt.
    let snapshot =
        initialize_archive_database(&stale, None).expect("launch recovery heals, no brick");
    assert!(snapshot.config.initialized);
    assert!(snapshot.archive_status.initialized);
    assert!(snapshot.archive_status.encrypted, "config healed to the encrypted file's real mode");
    assert!(!snapshot.archive_status.unlocked, "no key in hand -> locked, never bricked");
    assert!(snapshot.archive_status.warning.is_none(), "locked is a prompt state, not an error");
    assert!(matches!(
        vault_core::load_config(&paths).expect("load healed config").archive_mode,
        ArchiveMode::Encrypted,
    ));

    restore_env_var(PROJECT_ROOT_OVERRIDE_ENV, original_root.as_deref());
    restore_env_var(TEST_KEYRING_OVERRIDE_ENV, original_keyring.as_deref());
}

#[test]
fn launch_recovery_surfaces_an_unrecoverable_interrupted_rekey_to_the_worker() {
    // Fail-closed at the launch entry point: a rekey marker claims a Plaintext->Encrypted swap but
    // the canonical history-vault is gone, so the worker must return a STRUCTURED, prefix-tagged
    // error Phase D can route to the recovery screen — not an opaque bootstrap failure.
    let _guard = lock_env();
    let dir = tempdir().expect("tempdir");
    let keyring_root = dir.path().join("test-keyring");
    let original_root = std::env::var_os(PROJECT_ROOT_OVERRIDE_ENV);
    let original_keyring = std::env::var_os(TEST_KEYRING_OVERRIDE_ENV);
    unsafe {
        std::env::set_var(PROJECT_ROOT_OVERRIDE_ENV, dir.path());
        std::env::set_var(TEST_KEYRING_OVERRIDE_ENV, &keyring_root);
    }

    let config = initialized_config();
    legacy_archive(&config, None);
    let paths = project_paths().expect("project paths");

    let archive_dir = paths.legacy.archive.parent().expect("archive parent");
    let rekey_marker = archive_dir.join(".pk-rekey-journal.json");
    let snapshot_dir = paths.raw_snapshots_dir.join("rekey");
    fs::create_dir_all(&snapshot_dir).expect("snapshot dir");
    let snapshot_path = snapshot_dir.join("archive-before-rekey-fixture.sqlite");
    fs::write(&snapshot_path, b"SQLite format 3\0snapshot").expect("seed snapshot");
    let journal = serde_json::json!({
        "version": 1,
        "timestamp": "2026-06-30T00-00-00Z",
        "from_mode": "Plaintext",
        "to_mode": "Encrypted",
        "snapshot_ref": snapshot_path.to_string_lossy(),
        "previous_config": serde_json::Value::Null,
    });
    fs::write(&rekey_marker, serde_json::to_vec(&journal).expect("serialize journal"))
        .expect("seed rekey marker");
    fs::remove_file(&paths.legacy.archive).expect("remove history-vault");

    let error = initialize_archive_database(&config, None)
        .expect_err("a fail-closed interrupted rekey must surface as an error");
    let rendered = error.to_string();
    assert!(
        rendered.starts_with(vault_core::ARCHIVE_RECOVERY_REQUIRED_PREFIX),
        "the worker must tag the launch-recovery failure with the recovery-required prefix, got: {rendered}",
    );

    restore_env_var(PROJECT_ROOT_OVERRIDE_ENV, original_root.as_deref());
    restore_env_var(TEST_KEYRING_OVERRIDE_ENV, original_keyring.as_deref());
}

// --- D4: recovery worker commands + startup recovery surface ------------------------------------

/// The shell asks for the upgrade assessment before anything else, so an archive the old app
/// left half-way through a restore (its file quarantined, the journal written) has to be
/// brought back there, not only by initialize.
fn seed_restore_cut_mid_install(paths: &vault_core::ProjectPaths, keep_snapshot: bool) {
    let config = initialized_config();
    legacy_archive(&config, None);
    let snapshot = paths.raw_snapshots_dir.join("rekey").join("snap.sqlite");
    fs::create_dir_all(snapshot.parent().unwrap()).unwrap();
    if keep_snapshot {
        fs::copy(&paths.legacy.archive, &snapshot).unwrap();
    }
    let quarantine = paths.quarantine_dir.join("2026-06-30T00-00-00Z");
    let journal = serde_json::json!({
        "version": 1,
        "timestamp": "2026-06-30T00-00-00Z",
        "snapshot_ref": snapshot.to_string_lossy(),
        "restored_mode": "Plaintext",
        "quarantine_dir": quarantine.to_string_lossy(),
        "previous_config": fs::read_to_string(&paths.config_path).ok(),
    });
    let marker = paths.legacy.archive.parent().unwrap().join(".pk-restore-journal.json");
    fs::write(marker, serde_json::to_vec(&journal).unwrap()).unwrap();
    fs::remove_file(&paths.legacy.archive).unwrap();
}

#[test]
fn assessing_an_old_archive_first_finishes_a_restore_the_old_app_left_half_done() {
    let _guard = lock_env();
    let dir = tempdir().expect("tempdir");
    let original_root = std::env::var_os(PROJECT_ROOT_OVERRIDE_ENV);
    unsafe { std::env::set_var(PROJECT_ROOT_OVERRIDE_ENV, dir.path()) };
    let paths = project_paths().unwrap();
    seed_restore_cut_mid_install(&paths, true);

    let assessment = assess_archive_upgrade(None).expect("assessment after recovery");
    assert!(assessment.pending, "the restored old-format archive still needs converting");
    assert!(paths.legacy.archive.exists(), "the snapshot is back in place");
    assert!(!paths.legacy.archive.with_file_name(".pk-restore-journal.json").exists());

    restore_env_var(PROJECT_ROOT_OVERRIDE_ENV, original_root.as_deref());
}

#[test]
fn assessing_an_old_archive_that_cannot_be_brought_back_asks_for_the_recovery_screen() {
    let _guard = lock_env();
    let dir = tempdir().expect("tempdir");
    let original_root = std::env::var_os(PROJECT_ROOT_OVERRIDE_ENV);
    unsafe { std::env::set_var(PROJECT_ROOT_OVERRIDE_ENV, dir.path()) };
    let paths = project_paths().unwrap();
    seed_restore_cut_mid_install(&paths, false);

    let error = assess_archive_upgrade(None).expect_err("neither snapshot nor original is left");
    let report = crate::parse_archive_recovery_required(&error.to_string())
        .expect("the error carries the recovery report");
    assert!(matches!(report.kind, vault_core::ArchiveRecoveryKind::InterruptedRestoreUnresolved));
    assert!(!paths.legacy.archive.exists(), "no empty archive was created");

    restore_env_var(PROJECT_ROOT_OVERRIDE_ENV, original_root.as_deref());
}

/// A restore replaces the archive with the file it is given; a copy of a real snapshot in
/// another folder, or a path that walks out of the snapshot folder, must not be accepted.
#[test]
fn full_restore_takes_only_a_snapshot_pathkeep_listed() {
    let _guard = lock_env();
    let root = tempdir().unwrap();
    let old_env = std::env::var_os(PROJECT_ROOT_OVERRIDE_ENV);
    unsafe {
        std::env::set_var(PROJECT_ROOT_OVERRIDE_ENV, root.path().join("app"));
    }
    let paths = project_paths().unwrap();
    vault_core::save_config(&paths, &initialized_config()).unwrap();
    let session = crate::VaultSession::default();
    let vault = session.vault().unwrap();
    let elsewhere = root.path().join("elsewhere/snapshot");
    vault_core::maintenance::snapshot(&vault, &elsewhere).unwrap();
    let escaping = paths.raw_snapshots_dir.join("maintenance/../../../elsewhere/snapshot");
    for path in [elsewhere, escaping] {
        let error = crate::run_full_archive_restore(
            &session,
            None,
            &vault_core::SnapshotRestoreRequest { snapshot_path: path.display().to_string() },
        )
        .unwrap_err();
        assert!(error.to_string().starts_with("snapshot-not-listed:"), "{error:#}");
    }
    assert!(session.vault().unwrap().store().is_open());
    restore_env_var(PROJECT_ROOT_OVERRIDE_ENV, old_env.as_deref());
}

#[test]
fn delete_all_data_needs_the_word_then_returns_the_app_to_onboarding() {
    let _guard = lock_env();
    let dir = tempdir().expect("tempdir");
    let chrome_root = chrome_user_data_fixture(dir.path());
    let original_root = std::env::var_os(PROJECT_ROOT_OVERRIDE_ENV);
    let original_chrome = std::env::var_os(CHROME_USER_DATA_OVERRIDE_ENV);
    let original_keyring = std::env::var_os(TEST_KEYRING_OVERRIDE_ENV);
    let original_sandbox = std::env::var_os(vault_platform::SANDBOX_DIR_ENV);
    let app_root = dir.path().join("app");
    unsafe {
        std::env::set_var(PROJECT_ROOT_OVERRIDE_ENV, &app_root);
        std::env::set_var(CHROME_USER_DATA_OVERRIDE_ENV, &chrome_root);
        std::env::set_var(TEST_KEYRING_OVERRIDE_ENV, dir.path().join("test-keyring"));
        // The wipe asks the OS scheduler what is installed; never the developer's real one.
        std::env::set_var(vault_platform::SANDBOX_DIR_ENV, dir.path().join("os-sandbox"));
    }
    let browser_history = fs::read(chrome_root.join("Default").join("History")).expect("history");

    let config = configured_ai_config();
    initialize_archive_database(&config, None).expect("initialize");
    vault_platform::keyring_set_database_key("archive-key").expect("store archive key");
    keyring_set_provider_api_key("llm-primary", "sk-test").expect("store provider key");
    // A manual backup starts the intelligence queue workers in the background; the wipe has to
    // stop them before it deletes the databases they write.
    let session = crate::VaultSession::default();
    let vault = session.background_vault().expect("open the archive");
    let backup = run_backup_now(&vault, None, false).expect("backup");
    assert_eq!(backup.run.expect("run").new_visits, 1);

    let preview = preview_data_wipe().expect("preview");
    assert_eq!(preview.visit_count, 1);
    assert!(preview.clears_keychain);
    assert!(preview.items.iter().any(|item| item.path.ends_with("history-vault.db")));

    for wrong in ["", "delete", "DELETE ", "Delete"] {
        let error = wipe_all_data(wrong, || Ok(())).expect_err("only the exact word is accepted");
        assert!(error.to_string().contains("DELETE"), "{error:#}");
    }
    assert!(app_snapshot(None).expect("snapshot").archive_status.initialized, "nothing deleted");

    drop(vault);
    let report =
        wipe_all_data(WIPE_CONFIRMATION_WORD, || session.close_vault().map_err(anyhow::Error::msg))
            .expect("wipe");
    assert_eq!(report, vault_core::WipeReport::default(), "no automatic backup was installed");
    assert_eq!(crate::intelligence::running_background_workers(), 0);
    let snapshot = app_snapshot(None).expect("snapshot after wipe");
    assert!(!snapshot.config.initialized);
    assert!(!snapshot.archive_status.initialized);
    assert!(!snapshot.keyring_status.stored_secret, "the archive key must leave the keychain");
    assert!(!provider_api_key_saved("llm-primary"), "provider keys must leave the keychain");
    assert_eq!(
        fs::read(chrome_root.join("Default").join("History")).expect("history after"),
        browser_history,
        "the browser's own history must be untouched"
    );

    wipe_all_data(WIPE_CONFIRMATION_WORD, || Ok(())).expect("a second wipe is a no-op");
    assert!(!finish_interrupted_data_wipe().expect("nothing interrupted"));

    // Onboarding again works in the same process.
    initialize_archive_database(&initialized_config(), None).expect("initialize again");
    let session = crate::VaultSession::default();
    let vault = session.background_vault().expect("open the new archive");
    let again = run_backup_now(&vault, None, false).expect("backup again");
    assert_eq!(again.run.expect("run").new_visits, 1);
    drop(vault);
    session.close_vault().unwrap();

    restore_env_var(PROJECT_ROOT_OVERRIDE_ENV, original_root.as_deref());
    restore_env_var(CHROME_USER_DATA_OVERRIDE_ENV, original_chrome.as_deref());
    restore_env_var(TEST_KEYRING_OVERRIDE_ENV, original_keyring.as_deref());
    restore_env_var(vault_platform::SANDBOX_DIR_ENV, original_sandbox.as_deref());
}

#[test]
fn mode_changes_require_the_rekey_command_instead_of_a_config_edit() {
    let _guard = lock_env();
    let root = tempdir().unwrap();
    let old = std::env::var_os(PROJECT_ROOT_OVERRIDE_ENV);
    unsafe {
        std::env::set_var(PROJECT_ROOT_OVERRIDE_ENV, root.path());
    }
    let paths = project_paths().unwrap();
    let config = initialized_config();
    vault_core::save_config(&paths, &config).unwrap();
    let mut changed = config.clone();
    changed.archive_mode = ArchiveMode::Encrypted;
    let error = crate::save_user_config(&changed, None).unwrap_err();
    assert!(error.to_string().contains("encryption-mode-change-requires-rekey"));
    assert_eq!(vault_core::load_config(&paths).unwrap().archive_mode, config.archive_mode);
    restore_env_var(PROJECT_ROOT_OVERRIDE_ENV, old.as_deref());
}

#[path = "mcp_search_tests.rs"]
mod search_contract;
