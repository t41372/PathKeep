//! Responsibilities: test owner dispatch with the real worker and private endpoint.
//! Not responsible for: Tauri window lifecycle (desktop owner_process tests).
//! Failure modes: unlisted methods or invalid params execute; MCP bypasses consent;
//! due-only backup loses ledger semantics; clients read databases themselves.
use super::*;
use crate::tests::{
    PROJECT_ROOT_OVERRIDE_ENV, TEST_KEYRING_OVERRIDE_ENV, initialized_config, lock_env,
    restore_env_var,
};
use serde_json::json;

#[test]
fn owner_rejects_unknown_methods_and_invalid_parameters_before_work() {
    let root = tempfile::tempdir().unwrap();
    let vault = vault_core::Vault::open_temp().unwrap();
    let _server = vault_owner_ipc::Server::start(root.path(), move |request, progress| {
        dispatch(request, progress, Ok(None), || Ok(vault.clone()))
    })
    .unwrap();
    let rt = tokio::runtime::Runtime::new().unwrap();
    let client = vault_owner_ipc::Client::new(root.path());
    for (method, params, code) in [
        ("query_history", json!({}), ErrorCode::UnknownMethod),
        ("backup", json!({"dueOnly":"yes"}), ErrorCode::InvalidRequest),
        ("doctor", json!({"sql":"SELECT 1"}), ErrorCode::InvalidRequest),
    ] {
        let error = rt.block_on(client.request(Request::new(method, params), |_| {})).unwrap_err();
        assert_eq!(error.code, code);
    }
}

#[test]
fn owner_serves_ping_due_backup_and_mcp_consent_through_endpoint() {
    let _guard = lock_env();
    let root = tempfile::tempdir().unwrap();
    let original_root = std::env::var_os(PROJECT_ROOT_OVERRIDE_ENV);
    let original_keyring = std::env::var_os(TEST_KEYRING_OVERRIDE_ENV);
    unsafe {
        std::env::set_var(PROJECT_ROOT_OVERRIDE_ENV, root.path());
        std::env::set_var(TEST_KEYRING_OVERRIDE_ENV, root.path().join("keyring"));
    }
    let overrides = ["CHB_CHROME_USER_DATA_DIR", "CHB_FIREFOX_PROFILES_DIR", "CHB_SAFARI_ROOT"];
    let old = overrides.map(std::env::var_os);
    for name in overrides {
        unsafe {
            std::env::set_var(name, root.path().join(name));
        }
    }
    let chrome = crate::tests::chrome_user_data_fixture(root.path());
    unsafe {
        std::env::set_var("CHB_CHROME_USER_DATA_DIR", chrome);
    }
    let mut config = initialized_config();
    config.og_image.fetch_enabled = false;
    crate::initialize_archive_database(&config, None).unwrap();
    let reservation = std::sync::Arc::new(vault_store::OwnerLock::acquire(root.path()).unwrap());
    let vault = vault_core::Vault::open_owned(
        vault_core::project_paths().unwrap(),
        config.clone(),
        None,
        Some(reservation),
    )
    .unwrap();
    let retained = vault.clone();
    assert!(vault_store::OwnerLock::acquire(root.path()).is_err());
    let _server = vault_owner_ipc::Server::start(root.path(), move |request, progress| {
        dispatch(request, progress, Ok(None), || Ok(vault.clone()))
    })
    .unwrap();
    let rt = tokio::runtime::Runtime::new().unwrap();
    let client = vault_owner_ipc::Client::new(root.path());
    let ping = rt.block_on(client.request(Request::new("ping", json!({})), |_| {})).unwrap();
    assert_eq!(ping["pid"], std::process::id());
    let backup = rt
        .block_on(client.request(Request::new("backup", json!({"dueOnly":true})), |_| {}))
        .unwrap();
    assert_eq!(backup["run"]["status"], "success");
    retained
        .store()
        .read(vault_store::Db::Archive, |s| {
            assert_eq!(
                s.query_one("SELECT COUNT(*) FROM visits", (), |r| Ok(r.get::<i64>(0)?))?,
                1
            );
            assert_eq!(
                s.query_one(
                    "SELECT status FROM runs WHERE run_type='backup' ORDER BY id DESC LIMIT 1",
                    (),
                    |r| Ok(r.get::<String>(0)?)
                )?,
                "success"
            );
            Ok(())
        })
        .unwrap();
    let skipped = rt
        .block_on(client.request(Request::new("backup", json!({"dueOnly":true})), |_| {}))
        .unwrap();
    assert_eq!(skipped["dueSkipped"], true);
    let error = rt
        .block_on(
            client
                .request(Request::new("mcp", json!({"tool":"usage-guide","arguments":{}})), |_| {}),
        )
        .unwrap_err();
    let startup = rt
        .block_on(
            client
                .request(Request::new("mcp", json!({"tool":"initialize","arguments":{}})), |_| {}),
        )
        .unwrap_err();
    assert!(startup.message.contains("Enable AI and the MCP server"));
    assert_eq!(error.code, ErrorCode::OperationFailed);
    assert!(error.message.contains("Enable AI and the MCP server"));
    drop(_server);
    for (name, value) in overrides.into_iter().zip(old) {
        restore_env_var(name, value.as_deref());
    }
    restore_env_var(PROJECT_ROOT_OVERRIDE_ENV, original_root.as_deref());
    restore_env_var(TEST_KEYRING_OVERRIDE_ENV, original_keyring.as_deref());
}

#[test]
fn scheduled_backup_uses_the_open_owners_key_when_keyring_is_unavailable() {
    let _guard = lock_env();
    let root = tempfile::tempdir().unwrap();
    let overrides = [
        PROJECT_ROOT_OVERRIDE_ENV,
        TEST_KEYRING_OVERRIDE_ENV,
        "CHB_CHROME_USER_DATA_DIR",
        "CHB_FIREFOX_PROFILES_DIR",
        "CHB_SAFARI_ROOT",
    ];
    let old = overrides.map(std::env::var_os);
    for name in overrides {
        unsafe {
            std::env::set_var(name, root.path().join(name));
        }
    }
    unsafe {
        std::env::set_var(PROJECT_ROOT_OVERRIDE_ENV, root.path());
    }
    let chrome = crate::tests::chrome_user_data_fixture(root.path());
    unsafe {
        std::env::set_var("CHB_CHROME_USER_DATA_DIR", chrome);
    }
    let paths = vault_core::project_paths().unwrap();
    let mut config = initialized_config();
    config.archive_mode = vault_core::ArchiveMode::Encrypted;
    config.og_image.fetch_enabled = false;
    crate::initialize_archive_database(&config, Some("typed password")).unwrap();
    let session = crate::VaultSession::default();
    session.open_password(paths, config, "typed password".into()).unwrap();
    crate::write_database_key_to_keyring("typed password").unwrap();
    session.close_vault().unwrap();
    session.unlock_from_keychain().unwrap();
    assert_eq!(session.get_key().unwrap(), None);
    let retained = session.background_vault().unwrap();
    // The open session retains its data key even if the OS entry later becomes unreadable.
    let secret = root
        .path()
        .join(TEST_KEYRING_OVERRIDE_ENV)
        .join(format!("{}-database-key.secret", vault_platform::test_support::keyring_service()));
    std::fs::remove_file(&secret).unwrap();
    std::fs::create_dir(&secret).unwrap();
    session.unlock_from_keychain().expect("an already open owner reuses its data key");
    let server = vault_owner_ipc::Server::start(root.path(), move |request, progress| {
        dispatch(request, progress, session.get_key(), || {
            session.background_vault().map_err(anyhow::Error::msg)
        })
    })
    .unwrap();
    let report = tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(
            vault_owner_ipc::Client::new(root.path())
                .request(Request::new("backup", json!({"dueOnly":true})), |_| {}),
        )
        .unwrap();
    assert_eq!(report["run"]["status"], "success");
    assert_eq!(
        retained
            .store()
            .read(vault_store::Db::Archive, |s| s.query_one(
                "SELECT COUNT(*) FROM visits",
                (),
                |r| Ok(r.get::<i64>(0)?)
            ))
            .unwrap(),
        1
    );
    drop(server);
    for (name, value) in overrides.into_iter().zip(old) {
        restore_env_var(name, value.as_deref());
    }
}

#[test]
fn due_only_request_waits_for_startup_session_open() {
    let _guard = lock_env();
    let root = tempfile::tempdir().unwrap();
    let old_root = std::env::var_os(PROJECT_ROOT_OVERRIDE_ENV);
    let old_ring = std::env::var_os(TEST_KEYRING_OVERRIDE_ENV);
    unsafe {
        std::env::set_var(PROJECT_ROOT_OVERRIDE_ENV, root.path());
        std::env::set_var(TEST_KEYRING_OVERRIDE_ENV, root.path().join("keyring"));
    }
    let mut config = initialized_config();
    config.og_image.fetch_enabled = false;
    config.selected_profile_ids = vec!["chrome:Default".into()];
    crate::initialize_archive_database(&config, None).unwrap();
    let old_chrome = std::env::var_os("CHB_CHROME_USER_DATA_DIR");
    let chrome = crate::tests::chrome_user_data_fixture(root.path());
    unsafe {
        std::env::set_var("CHB_CHROME_USER_DATA_DIR", &chrome);
    }
    let session = crate::VaultSession::default();
    session
        .reserve_owner(std::sync::Arc::new(vault_store::OwnerLock::acquire(root.path()).unwrap()));
    session.begin_startup();
    let (received, arrived) = std::sync::mpsc::channel();
    let retained = session.clone();
    let _server = vault_owner_ipc::Server::start(root.path(), move |request, progress| {
        received.send(()).unwrap();
        dispatch(request, progress, session.get_key(), || {
            session.background_vault().map_err(anyhow::Error::msg)
        })
    })
    .unwrap();
    let client = vault_owner_ipc::Client::new(root.path());
    let request = std::thread::spawn(move || {
        tokio::runtime::Runtime::new().unwrap().block_on(client.request_with_start(
            Request::new("backup", json!({"dueOnly":true})),
            |_| {},
            std::time::Duration::from_millis(10),
            || panic!("endpoint is already bound"),
        ))
    });
    arrived.recv().unwrap();
    let mcp_client = vault_owner_ipc::Client::new(root.path());
    let mcp = std::thread::spawn(move || {
        tokio::runtime::Runtime::new().unwrap().block_on(mcp_client.request_with_start(
            Request::new("mcp", json!({"tool":"initialize","arguments":{}})),
            |_| {},
            std::time::Duration::from_millis(10),
            || panic!("endpoint is already bound"),
        ))
    });
    arrived.recv().unwrap();
    std::thread::sleep(std::time::Duration::from_millis(40));
    assert!(!mcp.is_finished());
    assert!(!request.is_finished(), "a connected request must outlive the connect deadline");
    retained.finish_startup().unwrap();
    assert!(mcp.join().unwrap().unwrap_err().message.contains("Enable AI and the MCP server"));
    let report = request.join().unwrap().unwrap();
    assert_eq!(report["run"]["status"], "success");
    assert!(retained.background_vault().is_ok());
    let (prepared, entered) = std::sync::mpsc::channel();
    let (release, wait) = std::sync::mpsc::channel();
    let busy_vault = retained.background_vault().unwrap();
    let running = std::thread::spawn(move || {
        crate::run_backup_now_with_progress(&busy_vault, None, false, |event| {
            if event.phase == "prepare" {
                prepared.send(()).unwrap();
                wait.recv().unwrap();
            }
        })
        .unwrap()
    });
    entered.recv().unwrap();
    let deferred = tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(
            vault_owner_ipc::Client::new(root.path())
                .request(Request::new("backup", json!({"dueOnly":true})), |_| {}),
        )
        .unwrap();
    assert_eq!(deferred["reasonCode"], "write-lock");
    assert_eq!(
        vault_core::load_schedule_attempt_ledger(&vault_core::project_paths().unwrap(), 1.0)
            .unwrap()
            .attempts[0]
            .outcome,
        vault_core::ScheduledBackupAttemptOutcome::Deferred
    );
    release.send(()).unwrap();
    running.join().unwrap();
    drop(_server);
    retained.close_vault().unwrap();
    restore_env_var("CHB_CHROME_USER_DATA_DIR", old_chrome.as_deref());
    restore_env_var(PROJECT_ROOT_OVERRIDE_ENV, old_root.as_deref());
    restore_env_var(TEST_KEYRING_OVERRIDE_ENV, old_ring.as_deref());
}

#[test]
fn owner_queue_pages_use_the_supplied_vault_without_a_password() {
    for key in [None, Some([88; 32])] {
        let vault = vault_core::Vault::open_temp_with_key(key).unwrap();
        for _ in 0..13 {
            vault_core::ai_queue::enqueue_index_job(&vault, &Default::default(), false).unwrap();
        }
        let endpoint = tempfile::tempdir().unwrap();
        let owner = vault.clone();
        let _server = vault_owner_ipc::Server::start(endpoint.path(), move |request, progress| {
            dispatch(request, progress, Ok(None), || Ok(owner.clone()))
        })
        .unwrap();
        let client = vault_owner_ipc::Client::new(endpoint.path());
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let mut cursor = serde_json::Value::Null;
        let mut ids = std::collections::BTreeSet::new();
        loop {
            let page =
                runtime
                    .block_on(client.request(
                        Request::new("ai-jobs", json!({"limit":4,"cursor":cursor})),
                        |_| {},
                    ))
                    .unwrap();
            for item in page["items"].as_array().unwrap() {
                assert!(ids.insert(item["id"].as_i64().unwrap()));
            }
            cursor = page["nextCursor"].clone();
            if cursor.is_null() {
                break;
            }
        }
        assert_eq!(ids.len(), 13);
    }
}
