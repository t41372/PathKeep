//! Bridge from the Tauri command facade into `vault-worker`.
//!
//! The command modules are intentionally transport-shaped, while
//! `vault-worker` is task-shaped. This layer adapts between them:
//! session-state lookups, transient key updates, progress callbacks, and
//! uniform string error shaping.

mod app;
mod archive;
mod import;
mod intelligence;
mod remote;
mod schedule;
mod security;

pub(crate) use self::{
    app::*, archive::*, import::*, intelligence::*, remote::*, schedule::*, security::*,
};

/// Normalizes worker/core errors into the string transport contract used by Tauri commands.
fn worker_result<T, E: ToString>(result: Result<T, E>) -> Result<T, String> {
    result.map_err(|error| error.to_string())
}

#[cfg(test)]
use crate::PRODUCT_DISPLAY_NAME;
#[cfg(test)]
use crate::session::{SessionState, session_key};
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
use vault_core::*;
#[cfg(test)]
use vault_worker::RekeyRequest;

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
        let ai_index_report = build_ai_index_impl(
            AiIndexRequest {
                provider_id: None,
                full_rebuild: false,
                clear_only: false,
                limit: Some(5),
            },
            session_key(&session).as_deref(),
        )
        .expect("index build should queue a background job report");
        assert!(ai_index_report.job_id.is_some());
        assert!(ai_index_report.run_id.is_none());
        assert!(
            ai_index_report
                .notes
                .iter()
                .any(|note| note.contains("processing it in the background"))
        );

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
        let runtime_before_rekey = (0..50)
            .find_map(|attempt| {
                let runtime = load_intelligence_runtime_impl(session_key(&session).as_deref())
                    .expect("load intelligence runtime before rekey");
                if runtime.queue.queued == 0 && runtime.queue.running == 0 {
                    Some(runtime)
                } else if attempt == 49 {
                    panic!("intelligence runtime did not go idle before rekey: {runtime:?}");
                } else {
                    std::thread::sleep(std::time::Duration::from_millis(50));
                    None
                }
            })
            .expect("idle intelligence runtime before rekey");
        assert_eq!(runtime_before_rekey.queue.queued, 0);
        assert_eq!(runtime_before_rekey.queue.running, 0);
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
