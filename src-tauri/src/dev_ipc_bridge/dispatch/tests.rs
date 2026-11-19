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

#[path = "app_surface_tests.rs"]
mod app_surface;

#[path = "backup_wire_tests.rs"]
mod backup_wire;

#[test]
fn model_download_attach_routes_one_terminal_to_the_window_and_bridge() {
    check_model_download_terminal(true);
}

#[test]
fn get_model_download_state_finds_another_windows_transfer_while_the_archive_is_busy() {
    let _guard = lock_env();
    let root = tempdir().unwrap();
    unsafe { std::env::set_var(PROJECT_ROOT_OVERRIDE_ENV, root.path()) };
    let paths = vault_core::project_paths_with_root(root.path());
    let make_window =
        || DevIpcBridgeState::without_app(SessionState::default(), DEFAULT_DEV_IPC_BRIDGE_PORT);
    let first = make_window();
    ready_block_on(async {
        let get = "get_model_download_state";
        assert_eq!(dispatch_command(&first, get, json!({})).await.unwrap(), Value::Null);
        assert!(!paths.models_dir.exists(), "a state read must not create a model directory");
        let mut config = test_config();
        config.ai.enabled = true;
        config.ai.semantic_index_enabled = true;
        vault_core::save_config(&vault_core::ControlGate::acquire(&paths), &config).unwrap();
        // Avoid network access even if a regression allows the download to proceed.
        let repo = vault_core::model_dir_for_repo(&paths, vault_core::DEFAULT_STATIC_MODEL_REPO);
        std::fs::create_dir_all(repo.join("current")).unwrap();

        let mut events = model_download_events(&first).await;
        let gate = vault_core::archive::ArchiveOpGate::acquire(&paths);
        let start =
            dispatch_command(&first, "download_static_embedding_model", json!({})).await.unwrap();
        let id = start["transferId"].as_str().unwrap();
        assert_eq!(
            next_model_download_event(&mut events).await,
            json!({"kind":"waiting","seq":start["seq"],"reason":"archive-busy","transferId":id})
        );
        assert!(start["seq"].as_u64().unwrap() > 0);
        let late = make_window();
        let snapshot = dispatch_command(&late, get, json!({})).await.unwrap();
        assert_eq!(
            snapshot,
            serde_json::to_value(crate::worker_bridge::get_model_download_state_impl().unwrap())
                .unwrap()
        );
        assert_eq!(
            snapshot,
            json!({
                "transferId":id,"seq":start["seq"], "phase":"waiting","reason":"archive-busy", "currentFile":null,
                "downloadedBytes":0, "totalBytes":0, "files":{}
            })
        );
        assert_eq!(
            dispatch_command(&late, "cancel_ai_embedding_model_download", start.clone())
                .await
                .unwrap(),
            true
        );
        drop(gate);
        assert_eq!(next_model_download_terminal(&mut events, id).await["kind"], "cancelled");
        assert_eq!(dispatch_command(&late, get, json!({})).await.unwrap(), Value::Null);
    });
}

#[test]
fn model_download_failure_reaches_the_window_and_bridge_with_its_diagnostic() {
    check_model_download_terminal(false);
}

fn check_model_download_terminal(request_cancel: bool) {
    use std::{sync::mpsc, time::Duration};
    use tauri::{Emitter, Listener};

    let _guard = lock_env();
    let root = tempdir().unwrap();
    unsafe { std::env::set_var(PROJECT_ROOT_OVERRIDE_ENV, root.path()) };
    let paths = vault_core::project_paths_with_root(root.path());
    let mut config = test_config();
    config.ai.enabled = true;
    config.ai.semantic_index_enabled = true;
    vault_core::save_config(&vault_core::ControlGate::acquire(&paths), &config).unwrap();
    let repo = vault_core::model_dir_for_repo(&paths, vault_core::DEFAULT_STATIC_MODEL_REPO);
    std::fs::create_dir_all(&repo).unwrap();
    // A deterministic terminal error before any network access.
    std::fs::create_dir(repo.join("current")).unwrap();
    let state =
        DevIpcBridgeState::without_app(SessionState::default(), DEFAULT_DEV_IPC_BRIDGE_PORT);
    let app = tauri::test::mock_app();
    let name = vault_core::MODEL_DOWNLOAD_PROGRESS_EVENT;
    let (sink, window) = mpsc::channel();
    app.listen_any(name, move |event| sink.send(event.payload().to_string()).unwrap());

    ready_block_on(async {
        let mut sse = model_download_events(&state).await;
        let gate = vault_core::archive::ArchiveOpGate::acquire(&paths);
        let start =
            dispatch_command(&state, "download_static_embedding_model", json!({})).await.unwrap();
        let id = start["transferId"].as_str().unwrap();
        assert!(start["seq"].as_u64().unwrap() > 0);

        // The dev bridge started before a window existed. Attach the window and join the
        // same transfer from its native backend while the installer is still parked.
        super::super::DevIpcBridgeHandle::new(state.app.clone(), state.events.clone())
            .attach_app(app.handle().clone())
            .unwrap();
        let handle = app.handle().clone();
        let attached = crate::worker_bridge::download_static_embedding_model_impl(move |event| {
            handle.emit(name, event).unwrap();
        })
        .unwrap();
        assert_eq!(attached.transfer_id, id);
        let cancel = "cancel_ai_embedding_model_download";
        assert!(dispatch_command(&state, cancel, json!({})).await.is_err());
        if request_cancel {
            assert_eq!(
                dispatch_command(&state, cancel, json!({"transferId":id})).await.unwrap(),
                true
            );
        }
        drop(gate);
        let terminal = next_model_download_terminal(&mut sse, id).await;
        let native = loop {
            let event: Value =
                serde_json::from_str(&window.recv_timeout(Duration::from_secs(5)).unwrap())
                    .unwrap();
            assert_eq!(event["transferId"], id);
            if event["kind"] != "waiting" {
                break event;
            }
        };
        assert_eq!(native, terminal);
        if request_cancel {
            assert_eq!(terminal, json!({"kind":"cancelled","seq":start["seq"],"transferId":id}));
        } else {
            assert_eq!(
                terminal,
                json!({"kind":"error","seq":start["seq"],"transferId":id,"message":format!("invalid model pointer in {}", repo.display())})
            );
        }
        assert_eq!(
            dispatch_command(&state, cancel, json!({"transferId":id})).await.unwrap(),
            false
        );
    });
}

#[test]
fn saving_semantic_off_through_the_bridge_cancels_the_model_download_without_a_cancel_command() {
    check_off_save(SaveFault::None);
}

#[cfg(unix)]
#[test]
fn model_download_save_returns_the_applied_snapshot_and_directory_warning() {
    check_off_save(SaveFault::ParentSync);
}

#[cfg(debug_assertions)]
#[test]
fn model_download_off_save_warns_through_the_debug_fault_file() {
    // injectFault affects a whole process. Isolate the marker from other test threads.
    const CHILD: &str = "PATHKEEP_CONFIG_SYNC_TEST_CHILD";
    if std::env::var_os(CHILD).is_none() {
        let faults = tempdir().unwrap();
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact","dev_ipc_bridge::dispatch::tests::model_download_off_save_warns_through_the_debug_fault_file"])
            .env("PATHKEEP_TEST_FAULTS_DIR",faults.path()).env(CHILD,"1").status().unwrap();
        assert!(status.success());
        return;
    }
    check_off_save(SaveFault::FaultFile);
}

enum SaveFault {
    None,
    #[cfg(unix)]
    ParentSync,
    #[cfg(debug_assertions)]
    FaultFile,
}

fn check_off_save(fault: SaveFault) {
    let _guard = lock_env();
    let root = tempdir().unwrap();
    unsafe {
        std::env::set_var(PROJECT_ROOT_OVERRIDE_ENV, root.path());
        std::env::set_var(TEST_KEYRING_OVERRIDE_ENV, root.path().join("keyring"));
        std::env::set_var(CHROME_USER_DATA_OVERRIDE_ENV, root.path().join("chrome"));
        std::env::set_var(vault_platform::SANDBOX_DIR_ENV, root.path().join("platform"));
    }
    let paths = vault_core::project_paths_with_root(root.path());
    let session = SessionState::default();
    let mut config = test_config();
    config.ai.enabled = true;
    config.ai.semantic_index_enabled = true;
    crate::worker_bridge::initialize_archive_with_progress_impl(
        config.clone(),
        None,
        &session,
        |_| {},
    )
    .unwrap();
    let state = DevIpcBridgeState::without_app(session, DEFAULT_DEV_IPC_BRIDGE_PORT);
    let repo = vault_core::model_dir_for_repo(&paths, vault_core::DEFAULT_STATIC_MODEL_REPO);
    std::fs::create_dir_all(&repo).unwrap();
    // If the save fails to cancel, stop the test installer locally before network access.
    std::fs::create_dir(repo.join("current")).unwrap();

    ready_block_on(async {
        let mut sse = model_download_events(&state).await;
        let parked = vault_core::archive::ArchiveOpGate::acquire(&paths);
        let start =
            dispatch_command(&state, "download_static_embedding_model", json!({})).await.unwrap();
        let id = start["transferId"].as_str().unwrap();
        let base = config.clone();
        config.ai.semantic_index_enabled = false;
        let marker: Option<std::path::PathBuf> = match fault {
            SaveFault::None => None,
            #[cfg(unix)]
            SaveFault::ParentSync => {
                vault_core::durable_io::fail_next_atomic_parent_sync();
                None
            }
            #[cfg(debug_assertions)]
            SaveFault::FaultFile => {
                let dir = std::env::var_os("PATHKEEP_TEST_FAULTS_DIR").unwrap();
                let path = std::path::PathBuf::from(dir).join("config-save-parent-sync");
                std::fs::write(&path, []).unwrap();
                Some(path)
            }
        };
        let saved =
            dispatch_command(&state, "save_config", json!({"config":config,"baseConfig":base}))
                .await
                .unwrap();
        assert_eq!(saved["config"]["ai"]["semanticIndexEnabled"], false);
        assert_eq!(
            saved
                .get("configDurabilityWarning")
                .and_then(Value::as_str)
                .is_some_and(|warning| warning.contains("syncing")),
            !matches!(fault, SaveFault::None)
        );
        if let Some(marker) = marker {
            std::fs::remove_file(marker).unwrap();
            let again =
                dispatch_command(&state, "save_config", json!({"config":config})).await.unwrap();
            assert!(again.get("configDurabilityWarning").is_none());
        }
        assert!(
            dispatch_command(&state, "download_static_embedding_model", json!({})).await.is_err()
        );
        drop(parked);
        assert_eq!(
            next_model_download_terminal(&mut sse, id).await,
            json!({"kind":"cancelled","seq":start["seq"],"transferId":id})
        );
    });
    let jobs: i64 = state
        .session
        .current_vault()
        .unwrap()
        .store()
        .read(vault_store::Db::Intelligence, |s| {
            s.query_one("SELECT count(*) FROM ai_jobs WHERE job_type='index-build'", (), |row| {
                Ok(row.get(0)?)
            })
        })
        .unwrap();
    assert_eq!(jobs, 0, "the cancelled setup did not enqueue an index build");
    state.session.close_vault().unwrap();
}

async fn model_download_events(state: &DevIpcBridgeState) -> axum::body::BodyDataStream {
    use axum::{extract::State, response::IntoResponse};
    super::super::events::events(State(state.clone()))
        .await
        .into_response()
        .into_body()
        .into_data_stream()
}

async fn next_model_download_terminal(stream: &mut axum::body::BodyDataStream, id: &str) -> Value {
    loop {
        let event = next_model_download_event(stream).await;
        assert_eq!(event["transferId"], id);
        if event["kind"] != "waiting" {
            return event;
        }
        assert_eq!(event["reason"], "archive-busy");
    }
}

async fn next_model_download_event(stream: &mut axum::body::BodyDataStream) -> Value {
    use futures_core::Stream;
    use std::{future::poll_fn, pin::Pin, time::Duration};
    let bytes = tokio::time::timeout(
        Duration::from_secs(10),
        poll_fn(|cx| Pin::new(&mut *stream).poll_next(cx)),
    )
    .await
    .unwrap()
    .unwrap()
    .unwrap();
    let frame = std::str::from_utf8(&bytes).unwrap();
    let name = vault_core::MODEL_DOWNLOAD_PROGRESS_EVENT;
    assert!(frame.lines().any(|line| line == format!("event: {name}")), "{frame}");
    let data = frame.lines().find_map(|line| line.strip_prefix("data: ")).unwrap();
    serde_json::from_str(data).unwrap()
}

#[cfg(debug_assertions)]
fn setup_build_session() -> (tempfile::TempDir, DevIpcBridgeState, AppConfig) {
    let dir = tempdir().unwrap();
    unsafe {
        std::env::set_var(PROJECT_ROOT_OVERRIDE_ENV, dir.path());
        std::env::set_var(TEST_KEYRING_OVERRIDE_ENV, dir.path().join("keyring"));
        std::env::set_var(CHROME_USER_DATA_OVERRIDE_ENV, dir.path().join("chrome"));
        std::env::set_var(vault_platform::SANDBOX_DIR_ENV, dir.path().join("platform"));
    }
    let session = SessionState::default();
    let mut config = test_config();
    config.ai.enabled = true;
    config.ai.semantic_index_enabled = true;
    config.ai.job_queue_paused = true;
    crate::worker_bridge::initialize_archive_with_progress_impl(
        config.clone(),
        None,
        &session,
        |_| {},
    )
    .unwrap();
    (dir, DevIpcBridgeState::without_app(session, DEFAULT_DEV_IPC_BRIDGE_PORT), config)
}

#[cfg(debug_assertions)]
fn setup_build_payload(state: &DevIpcBridgeState) -> Value {
    // Sideload the real pinned fixture, then obtain the ID from the actual start/done
    // sequence before issuing the request a setup listener would send.
    unsafe {
        std::env::set_var("PATHKEEP_TEST_MODEL_SOURCE", "http://127.0.0.1:1/v1");
    }
    let paths = vault_core::project_paths().unwrap();
    let repository = vault_core::model_dir_for_repo(&paths, vault_core::DEFAULT_STATIC_MODEL_REPO);
    std::fs::create_dir_all(&repository).unwrap();
    let fixtures = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("crates/vault-core/tests/fixtures/builtin-model");
    for (folder, name) in
        [("common", "config.json"), ("common", "tokenizer.json"), ("v1", "model.safetensors")]
    {
        std::fs::copy(fixtures.join(folder).join(name), repository.join(name)).unwrap();
    }
    let id = ready_block_on(async {
        let mut stream = model_download_events(state).await;
        let start =
            dispatch_command(state, "download_static_embedding_model", json!({})).await.unwrap();
        let id = start["transferId"].as_str().unwrap();
        assert_eq!(
            next_model_download_terminal(&mut stream, id).await,
            json!({"kind":"done","seq":start["seq"],"transferId":id})
        );
        id.to_owned()
    });
    json!({"request":{"transferId":id,"providerId":"static-in-app","clearOnly":false,"fullRebuild":false,"scope":"incremental"}})
}

#[test]
#[cfg(debug_assertions)]
fn model_download_setup_never_builds_the_newly_selected_external_service() {
    let _guard = lock_env();
    let (_root, state, mut config) = setup_build_session();
    let original = setup_build_payload(&state);
    let external = vault_core::AiProviderConfig {
        id: "external".into(),
        enabled: true,
        default_model: "external-model".into(),
        ..Default::default()
    };
    config.ai.embedding_provider_id = Some(external.id.clone());
    config.ai.embedding_providers.push(external);
    let paths = vault_core::project_paths().unwrap();
    vault_core::save_config(&vault_core::ControlGate::acquire(&paths), &config).unwrap();
    for provider in [Some("static-in-app"), None, Some("external")] {
        let mut payload = original.clone();
        payload["request"]["providerId"] = json!(provider);
        let result = ready_block_on(dispatch_command(&state, "build_ai_index", payload));
        let error = result.expect_err("setup escaped to the external service");
        if provider == Some("static-in-app") {
            assert!(
                error
                    .message
                    .contains("Index builds must use the embedding provider chosen in Settings"),
                "{error:?}"
            );
        }
    }
    let n: i64 = state
        .session
        .vault()
        .unwrap()
        .store()
        .read(vault_store::Db::Intelligence, |s| {
            s.query_one("SELECT count(*) FROM ai_jobs WHERE job_type='index-build'", (), |r| {
                Ok(r.get(0)?)
            })
        })
        .unwrap();
    assert_eq!(n, 0);
}

#[test]
#[cfg(debug_assertions)]
fn model_download_setup_reuses_the_finished_job_across_transports() {
    let _guard = lock_env();
    let (_root, state, _config) = setup_build_session();
    let payload = setup_build_payload(&state);
    let first =
        ready_block_on(dispatch_command(&state, "build_ai_index", payload.clone())).unwrap();
    let id = first["jobId"].as_i64().unwrap();
    let vault = state.session.vault().unwrap();
    vault_core::ai_queue::resume_paused_jobs(&vault).unwrap();
    assert_eq!(vault_core::ai_queue::claim_ai_job_by_id(&vault, id, 300).unwrap().unwrap().id, id);
    assert!(vault_core::ai_queue::mark_ai_job_succeeded(&vault, id, None, None).unwrap());
    let second = crate::worker_bridge::build_ai_index_impl(
        serde_json::from_value(payload["request"].clone()).unwrap(),
        &state.session,
    )
    .unwrap();
    assert_eq!(second.job_id, Some(id), "completed setup queued twice");
}
