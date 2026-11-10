//! Responsibilities: hold the security promises of the dev IPC command dispatcher, which any
//! process that can reach the localhost port can call.
//! Not responsible for: routing every mirrored command (E2E goes through this bridge), starting
//! the HTTP router or binding a port.
//! Failure modes: a rekey that skips the current password, or a locked snapshot that reads the
//! Keychain.

use crate::session::{SessionState, session_key};
use crate::test_support::{
    CHROME_USER_DATA_OVERRIDE_ENV, PROJECT_ROOT_OVERRIDE_ENV, TEST_KEYRING_OVERRIDE_ENV, lock_env,
};
use serde_json::{Value, json};
use std::future::Future;
use tempfile::tempdir;
use vault_core::{AppConfig, ArchiveMode};

use super::super::{DEFAULT_DEV_IPC_BRIDGE_PORT, DevIpcBridgeState};
use super::dispatch_command;

#[test]
fn dispatch_rekey_requires_the_current_password_and_keeps_keychain_unlock() {
    // The dev bridge is the path an automation (or anyone with the localhost port) uses.
    // Holding an unlocked session must not be enough to change or remove the encryption:
    // a wrong or missing `currentKey` is refused even though the session holds the right
    // key, and the right `currentKey` works even when the session holds none.
    let _guard = lock_env();
    let dir = tempdir().expect("tempdir");
    let chrome_root = dir.path().join("chrome-user-data");
    std::fs::create_dir_all(&chrome_root).expect("chrome root");
    unsafe {
        std::env::set_var(PROJECT_ROOT_OVERRIDE_ENV, dir.path());
        std::env::set_var(CHROME_USER_DATA_OVERRIDE_ENV, &chrome_root);
        std::env::set_var(TEST_KEYRING_OVERRIDE_ENV, dir.path().join("test-keyring"));
    }
    let password = "correct horse battery";
    let state =
        DevIpcBridgeState::without_app(SessionState::default(), DEFAULT_DEV_IPC_BRIDGE_PORT);
    let config = AppConfig { archive_mode: ArchiveMode::Encrypted, ..test_config() };
    ready_block_on(dispatch_command(
        &state,
        "initialize_archive",
        json!({ "config": config, "databaseKey": password }),
    ))
    .expect("initialize encrypted archive");
    assert_eq!(session_key(&state.session).unwrap().as_deref(), Some(password));
    let snapshot = ready_block_on(dispatch_command(&state, "app_snapshot", json!({}))).unwrap();
    assert_eq!(snapshot["archiveStatus"]["unlocked"], true);

    let refusals = [
        ("preview_rekey_archive", json!({ "newMode": "Plaintext", "newKey": null })),
        ("rekey_archive", json!({ "newMode": "Plaintext", "newKey": null })),
        (
            "rekey_archive",
            json!({ "newMode": "Encrypted", "newKey": "intruder password", "currentKey": "guess" }),
        ),
    ];
    for (command, request) in refusals {
        let expected = if request.get("currentKey").is_some() {
            "archive-password-wrong"
        } else {
            "archive-password-required"
        };
        let error =
            ready_block_on(dispatch_command(&state, command, json!({ "request": request })))
                .expect_err("refused without the current password");
        assert_eq!(error.code.as_deref(), Some(expected), "{command}: {}", error.message);
        assert!(!error.message.contains("intruder password") && !error.message.contains("guess"));
    }
    assert_eq!(session_key(&state.session).unwrap().as_deref(), Some(password));

    ready_block_on(dispatch_command(&state, "clear_session_database_key", json!({})))
        .expect("clear session key");
    ready_block_on(dispatch_command(
        &state,
        "rekey_archive",
        json!({"request":{"newMode":"Encrypted","newKey":"next password","currentKey":password}}),
    ))
    .expect("the verified current password permits rewrap without a pre-existing session");
    assert!(
        vault_core::Vault::unlock_data_key(&vault_core::project_paths().unwrap(), password)
            .is_err()
    );
    let password = "next password";
    ready_block_on(dispatch_command(&state, "clear_session_database_key", json!({}))).unwrap();
    ready_block_on(dispatch_command(
        &state,
        "keyring_store_database_key",
        json!({"value": password}),
    ))
    .unwrap();
    assert_eq!(
        ready_block_on(dispatch_command(&state, "keyring_get_database_key", json!({}))).unwrap(),
        json!(true)
    );
    assert_eq!(
        ready_block_on(dispatch_command(&state, "unlock_archive_from_keychain", json!({})))
            .unwrap(),
        Value::Null
    );
    assert!(state.session.background_vault().is_ok());
    let snapshot = ready_block_on(dispatch_command(&state, "app_snapshot", json!({}))).unwrap();
    assert_eq!(snapshot["archiveStatus"]["unlocked"], true);
    assert!(snapshot["recentRuns"].as_array().unwrap().is_empty());
    ready_block_on(dispatch_command(&state, "preview_wipe_all_data", json!({}))).unwrap();
    ready_block_on(dispatch_command(
        &state,
        "set_session_database_key",
        json!({"databaseKey": password}),
    ))
    .unwrap();
    assert_eq!(
        ready_block_on(dispatch_command(&state, "app_snapshot", json!({}))).unwrap()["archiveStatus"]
            ["unlocked"],
        true
    );
    state.session.close_vault().unwrap();
}

fn assert_agent_crud_after_keychain_unlock(state: &DevIpcBridgeState) {
    for id in ["chat-a", "chat-b", "chat-c"] {
        let messages: Vec<_> = (0..3)
            .map(|seq| {
                json!({
                    "id":format!("{id}-{seq}"), "role":"user", "content":format!("message {seq}")
                })
            })
            .collect();
        ready_block_on(dispatch_command(
            state,
            "save_ai_conversation",
            json!({"request":{
                "id":id, "messages":messages
            }}),
        ))
        .unwrap();
    }
    let mut cursor = Value::Null;
    let mut ids = Vec::new();
    loop {
        let page = ready_block_on(dispatch_command(
            state,
            "list_ai_conversations",
            json!({"request":{"limit":1,"cursor":cursor}}),
        ))
        .unwrap();
        ids.extend(
            page["conversations"]
                .as_array()
                .unwrap()
                .iter()
                .map(|row| row["id"].as_str().unwrap().to_owned()),
        );
        cursor = page["nextCursor"].clone();
        if cursor.is_null() {
            break;
        }
    }
    ids.sort();
    assert_eq!(ids, ["chat-a", "chat-b", "chat-c"]);
    let mut cursor = Value::Null;
    let mut messages = Vec::new();
    loop {
        let page = ready_block_on(dispatch_command(
            state,
            "list_ai_messages",
            json!({"request":{"conversationId":"chat-a","limit":1,"cursor":cursor}}),
        ))
        .unwrap();
        messages.extend(
            page["messages"]
                .as_array()
                .unwrap()
                .iter()
                .map(|row| row["id"].as_str().unwrap().to_owned()),
        );
        cursor = page["nextCursor"].clone();
        if cursor.is_null() {
            break;
        }
    }
    assert_eq!(messages, ["chat-a-0", "chat-a-1", "chat-a-2"]);
    let loaded = ready_block_on(dispatch_command(
        state,
        "load_ai_conversation",
        json!({"conversationId":"chat-a"}),
    ))
    .unwrap();
    assert_eq!(loaded["messages"].as_array().unwrap().len(), 3);
    let renamed = ready_block_on(dispatch_command(
        state,
        "rename_ai_conversation",
        json!({"request":{"id":"chat-a","title":"Saved title"}}),
    ))
    .unwrap();
    assert_eq!(renamed["title"], "Saved title");
    let deleted = ready_block_on(dispatch_command(
        state,
        "delete_ai_conversation",
        json!({"conversationId":"chat-a"}),
    ))
    .unwrap();
    assert_eq!(deleted["deleted"], true);
    assert!(
        ready_block_on(dispatch_command(
            state,
            "load_ai_conversation",
            json!({"conversationId":"chat-a"})
        ))
        .unwrap()
        .is_null()
    );
}

fn test_config() -> AppConfig {
    AppConfig {
        initialized: true,
        archive_mode: ArchiveMode::Plaintext,
        selected_profile_ids: Vec::new(),
        git_enabled: false,
        ..AppConfig::default()
    }
}

// Some dispatch arms intentionally yield (e.g. spawn_blocking for the
// og:image refetch reqwest client). A current-thread tokio runtime
// drives every arm to completion without bootstrapping archives or
// intelligence rebuilds.
fn ready_block_on<F: Future>(future: F) -> F::Output {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("dispatch test tokio runtime")
        .block_on(future)
}

#[test]
fn locked_snapshots_do_not_read_keychain_and_explicit_unlock_reads_once() {
    let _guard = lock_env();
    let root = tempdir().unwrap();
    let names = [
        PROJECT_ROOT_OVERRIDE_ENV,
        TEST_KEYRING_OVERRIDE_ENV,
        CHROME_USER_DATA_OVERRIDE_ENV,
        vault_platform::SANDBOX_DIR_ENV,
    ];
    let old = names.map(std::env::var_os);
    let keyring = root.path().join("keyring");
    unsafe {
        std::env::set_var(PROJECT_ROOT_OVERRIDE_ENV, root.path());
        std::env::set_var(TEST_KEYRING_OVERRIDE_ENV, &keyring);
        std::env::set_var(CHROME_USER_DATA_OVERRIDE_ENV, root.path().join("chrome"));
        std::env::set_var(vault_platform::SANDBOX_DIR_ENV, root.path().join("platform"));
    }
    let state =
        DevIpcBridgeState::without_app(SessionState::default(), DEFAULT_DEV_IPC_BRIDGE_PORT);
    let config = AppConfig { archive_mode: ArchiveMode::Encrypted, ..test_config() };
    ready_block_on(dispatch_command(
        &state,
        "initialize_archive",
        json!({"config":config,"databaseKey":"password"}),
    ))
    .unwrap();
    state.session.close_vault().unwrap();
    vault_platform::keyring_clear_database_key().unwrap();
    for stored in [false, true] {
        if stored {
            vault_worker::write_database_key_to_keyring("password").unwrap();
        }
        let before = vault_platform::test_support::keyring_secret_read_count(&keyring);
        for _ in 0..2 {
            let snapshot =
                ready_block_on(dispatch_command(&state, "app_snapshot", json!({}))).unwrap();
            assert_eq!(snapshot["archiveStatus"]["encrypted"], true);
            assert_eq!(snapshot["archiveStatus"]["unlocked"], false);
            assert!(state.session.current_vault().is_none());
        }
        assert_eq!(vault_platform::test_support::keyring_secret_read_count(&keyring), before);
    }
    let before = vault_platform::test_support::keyring_secret_read_count(&keyring);
    ready_block_on(dispatch_command(&state, "unlock_archive_from_keychain", json!({}))).unwrap();
    assert_eq!(vault_platform::test_support::keyring_secret_read_count(&keyring), before + 1);
    assert!(state.session.get_key().unwrap().is_none());
    assert_agent_crud_after_keychain_unlock(&state);
    let snapshot = ready_block_on(dispatch_command(&state, "app_snapshot", json!({}))).unwrap();
    assert_eq!(snapshot["archiveStatus"]["unlocked"], true);
    ready_block_on(dispatch_command(&state, "schedule_status", json!({}))).unwrap();
    ready_block_on(dispatch_command(&state, "ai_chat_cancel", json!({"runId":"no-live-run"})))
        .unwrap();
    assert_eq!(vault_platform::test_support::keyring_secret_read_count(&keyring), before + 1);
    ready_block_on(dispatch_command(&state, "wipe_all_data", json!({"confirmation":"DELETE"})))
        .unwrap();
    for (name, value) in names.into_iter().zip(old) {
        unsafe {
            match value {
                Some(value) => std::env::set_var(name, value),
                None => std::env::remove_var(name),
            }
        }
    }
}

#[path = "native_reads_tests.rs"]
mod native_reads;
