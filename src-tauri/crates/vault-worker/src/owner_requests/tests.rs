//! Responsibilities: test owner dispatch with the real worker and private endpoint.
//! Not responsible for: Tauri window lifecycle (desktop owner_process tests).
//! Failure modes: unlisted methods or invalid params execute; MCP bypasses consent;
//! due-only backup loses ledger semantics; clients read databases themselves;
//! startup loses the converted archive or an open owner's data key.
use super::*;
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
