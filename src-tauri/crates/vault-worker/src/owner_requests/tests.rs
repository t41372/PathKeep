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
    let _server = vault_owner_ipc::Server::start(root.path(), |request, progress| {
        dispatch(request, progress, None)
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
    let _server = vault_owner_ipc::Server::start(root.path(), |request, progress| {
        dispatch(request, progress, None)
    })
    .unwrap();
    let rt = tokio::runtime::Runtime::new().unwrap();
    let client = vault_owner_ipc::Client::new(root.path());
    let ping = rt.block_on(client.request(Request::new("ping", json!({})), |_| {})).unwrap();
    assert_eq!(ping["pid"], std::process::id());
    let backup = rt
        .block_on(client.request(Request::new("backup", json!({"dueOnly":true})), |_| {}))
        .unwrap();
    assert!(backup.is_object());
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
