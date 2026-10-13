//! Responsibilities: adapt worker operations to desktop sessions and serialized command results.
//! Not responsible for: domain rules, storage engines or frontend rendering.
//! Failure modes: session ownership lost, typed errors flattened or imported archives left unpublished.
//!
//! Bridge from the Tauri command facade into `vault-worker`.
//!
//! The command modules are intentionally transport-shaped, while
//! `vault-worker` is task-shaped. This layer adapts between them:
//! session-state lookups, transient key updates, progress callbacks, and
//! uniform string error shaping.

use crate::command_error::CommandError;
#[cfg(test)]
use crate::command_error::{ACTION_HINT_UNLOCK, ERROR_CODE_LOCK_REQUIRED, RETRY_HINT_AFTER_ACTION};

mod annotations;
mod app;
mod archive;
mod import;
mod intelligence;
mod migration;
mod schedule;
mod search;
mod security;
mod stars;
// The Tauri command layer, which calls most of these, is compiled out of unit tests.
#[cfg_attr(test, allow(unused_imports))]
pub(crate) use self::{
    annotations::*, app::*, archive::*, import::*, intelligence::*, migration::*, schedule::*,
    search::*, security::*, stars::*,
};

/// Normalizes worker/core errors into the [`CommandError`] envelope used by Tauri commands.
///
/// Uses the `{:#}` alternate Display formatter so `anyhow::Error` chains
/// surface as `"top: cause: root"` — the full chain stays in `message`
/// because PathKeep is a local-only app and the user is *also* the bug
/// reporter. The structured `code` / `action_hint` / `retry_hint` fields
/// (required by `module-boundary-map.md`) are classified here, in the one
/// layer that owns the backend's canonical error markers, so the frontend
/// keys remediation and i18n on `code` instead of sniffing message text.
/// Non-anyhow types fall back to plain Display, which `{:#}` reduces to for
/// any type that does not honour the alternate flag.
fn worker_result<T, E: std::fmt::Display>(result: Result<T, E>) -> Result<T, CommandError> {
    result.map_err(|error| {
        let message = format!("{error:#}");
        log::warn!(target: "pathkeep::worker_bridge", "{message}");
        CommandError::classified(message)
    })
}

#[cfg(test)]
use crate::session::SessionState;
#[cfg(test)]
use rusqlite::Connection;
#[cfg(test)]
use std::{
    fs,
    path::{Path, PathBuf},
};
#[cfg(test)]
use tempfile::tempdir;
#[cfg(test)]
use vault_core::*;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{
        CHROME_USER_DATA_OVERRIDE_ENV, PROJECT_ROOT_OVERRIDE_ENV, TEST_KEYRING_OVERRIDE_ENV,
        lock_env,
    };

    fn initialized_config() -> AppConfig {
        let mut config = AppConfig {
            initialized: true,
            archive_mode: ArchiveMode::Plaintext,
            git_enabled: false,
            due_after_hours: 72.0,
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
        // Point at a loopback discard port so that a run without a key fails fast with a
        // connection error instead of reaching a real cloud endpoint (api.openai.com).
        config.ai.llm_providers = vec![AiProviderConfig {
            id: "llm-primary".to_string(),
            name: "Primary LLM".to_string(),
            purpose: AiProviderPurpose::Llm,
            request_format: AiRequestFormat::OpenAi,
            enabled: true,
            base_url: Some("http://127.0.0.1:9/v1".to_string()),
            default_model: "gpt-4.1-mini".to_string(),
            ..AiProviderConfig::default()
        }];
        config.ai.embedding_providers = vec![AiProviderConfig {
            id: "embed-primary".to_string(),
            name: "Primary embedding".to_string(),
            purpose: AiProviderPurpose::Embedding,
            request_format: AiRequestFormat::OpenAi,
            enabled: true,
            base_url: Some("http://127.0.0.1:9/v1".to_string()),
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

    #[test]
    fn app_lock_bridge_guards_desktop_read_models_until_unlock() {
        let session = SessionState::default();

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
        initialize_archive_with_progress_impl(config.clone(), None, &session, |_| {})
            .expect("initialize archive");
        run_backup_now_impl(false, &session, |_| {}).expect("backup");

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
            save_config_with_base_impl(config, None, &session).expect("enable app lock");
        assert!(saved_snapshot.config.app_lock.enabled);

        let owner = session.vault().unwrap().store().clone();
        let key_before_lock = session.get_key();
        let chat_token = vault_core::register_ai_chat_run("app-lock-bridge-chat");
        let locked = lock_app_session_impl(Some("manual".to_string())).expect("lock app session");
        assert!(locked.locked);
        assert_eq!(session.get_key(), key_before_lock);
        assert!(
            !chat_token.load(std::sync::atomic::Ordering::SeqCst),
            "App Lock must not cancel a running chat"
        );
        vault_core::deregister_ai_chat_run("app-lock-bridge-chat");

        assert!(std::sync::Arc::ptr_eq(&owner, session.background_vault().unwrap().store()));
        assert_eq!(locked.lock_reason.as_deref(), Some("manual"));

        let snapshot_error = app_snapshot_impl(&session).expect_err("snapshot should block");
        assert!(snapshot_error.message.contains("currently locked"));
        // The frontend routes to the unlock gate off the machine-readable code,
        // never by sniffing this message — so the envelope must classify it.
        assert_eq!(snapshot_error.code.as_deref(), Some(ERROR_CODE_LOCK_REQUIRED));
        assert_eq!(snapshot_error.action_hint.as_deref(), Some(ACTION_HINT_UNLOCK));
        assert_eq!(snapshot_error.retry_hint.as_deref(), Some(RETRY_HINT_AFTER_ACTION));

        let dashboard_error =
            dashboard_snapshot_impl(&session).expect_err("dashboard should block");
        assert!(dashboard_error.message.contains("currently locked"));
        assert_eq!(dashboard_error.code.as_deref(), Some(ERROR_CODE_LOCK_REQUIRED));

        let unlock_error = unlock_app_session_impl(UnlockAppSessionRequest {
            passcode: Some("9999".to_string()),
            use_biometric: false,
        })
        .expect_err("wrong passcode should fail");
        assert!(unlock_error.message.contains("did not match"));
        // A wrong passcode is a user input error, not an "unlock first" refusal:
        // classifying it as lock-required would loop the unlock gate on itself.
        assert_eq!(unlock_error.code, None);

        let unlocked = unlock_app_session_impl(UnlockAppSessionRequest {
            passcode: Some("2468".to_string()),
            use_biometric: false,
        })
        .expect("unlock app session");
        assert!(!unlocked.locked);

        let snapshot = app_snapshot_impl(&session).expect("snapshot after unlock");
        assert!(snapshot.archive_status.initialized);
        let dashboard = dashboard_snapshot_impl(&session).expect("dashboard");
        assert_eq!(
            dashboard.recent_runs.len(),
            1,
            "the dashboard reads the backup from the archive"
        );
        assert_eq!(
            owner
                .read(vault_store::Db::Archive, |s| s.query_one(
                    "SELECT COUNT(*) FROM runs WHERE run_type='backup'",
                    (),
                    |r| Ok(r.get::<i64>(0)?)
                ))
                .unwrap(),
            1
        );
        let cleared = clear_app_lock_passcode_impl().expect("clear app lock passcode");
        assert!(!cleared.enabled);
        assert!(!cleared.passcode_configured);

        unsafe {
            std::env::remove_var(PROJECT_ROOT_OVERRIDE_ENV);
            std::env::remove_var(CHROME_USER_DATA_OVERRIDE_ENV);
            std::env::remove_var(TEST_KEYRING_OVERRIDE_ENV);
        }
    }
}

#[cfg(test)]
mod recovery_tests;
