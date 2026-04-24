//! Regression coverage for the dev IPC command dispatcher.
//!
//! ## Responsibilities
//!
//! - Prove session-only commands still work without a live Tauri app handle.
//! - Preserve the unknown-command error contract used by browser automation.
//!
//! ## Not responsible for
//!
//! - Testing every mirrored desktop command implementation.
//! - Starting the HTTP router or binding a localhost port.
//!
//! ## Dependencies
//!
//! - `SessionState` for in-memory command round trips.
//! - `serde_json` for browser-shaped command envelopes.
//!
//! ## Performance notes
//!
//! These tests run only tiny dispatch paths and must not bootstrap archives,
//! browser fixtures, or intelligence rebuilds.

use crate::session::{SessionState, session_key};
use serde_json::{Value, json};

use super::super::{DEFAULT_DEV_IPC_BRIDGE_PORT, DevIpcBridgeState};
use super::dispatch_command;

#[tokio::test]
async fn dispatch_command_handles_session_round_trip_without_tauri_app() {
    let state = DevIpcBridgeState {
        app: None,
        session: SessionState::default(),
        port: DEFAULT_DEV_IPC_BRIDGE_PORT,
    };

    let set =
        dispatch_command(&state, "set_session_database_key", json!({ "databaseKey": "secret" }))
            .await
            .expect("set session key");
    assert_eq!(set, Value::Null);
    assert_eq!(session_key(&state.session), Some("secret".to_string()));

    let clear = dispatch_command(&state, "clear_session_database_key", json!({}))
        .await
        .expect("clear session key");
    assert_eq!(clear, Value::Null);
    assert_eq!(session_key(&state.session), None);
}

#[tokio::test]
async fn dispatch_command_rejects_unknown_commands() {
    let state = DevIpcBridgeState {
        app: None,
        session: SessionState::default(),
        port: DEFAULT_DEV_IPC_BRIDGE_PORT,
    };

    let error = dispatch_command(&state, "missing", json!({}))
        .await
        .expect_err("missing command should fail");

    assert!(error.contains("does not recognize"));
}
