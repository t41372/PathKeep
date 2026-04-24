//! HTTP router for the dev-only IPC bridge.
//!
//! ## Responsibilities
//!
//! - Build the localhost HTTP routes used by browser automation.
//! - Decode JSON request bodies into `serde_json::Value` before dispatch.
//! - Shape HTTP-level error envelopes for invalid JSON and command failures.
//!
//! ## Not responsible for
//!
//! - Choosing which desktop command names exist.
//! - Calling worker bridge implementations directly.
//! - Starting the TCP listener or deciding whether the bridge is enabled.
//!
//! ## Dependencies
//!
//! - `axum` for the feature-gated local HTTP surface.
//! - `tower-http` CORS middleware for the local browser allow-list.
//! - Parent-module dispatch/state types for the command mirror.
//!
//! ## Performance notes
//!
//! The router only handles small command envelopes. Long-running archive,
//! import, and intelligence work must remain behind the worker bridge and its
//! off-main-thread contracts.

use anyhow::{Context, Result};
use axum::{
    Router,
    body::Bytes,
    extract::{Path, State},
    http::{HeaderValue, Method, StatusCode},
    routing::{get, post},
};
use serde_json::{Value, json};
use tower_http::cors::{AllowOrigin, Any, CorsLayer};

use super::{DevIpcBridgeState, config::DevIpcBridgeConfig, dispatch::dispatch_command};
use crate::PRODUCT_DISPLAY_NAME;

/// Builds the health and command routes for the local browser mirror.
///
/// Invalid CORS origins fail startup rather than weakening the localhost-only
/// development boundary. The returned router carries cloned bridge state for
/// each request; it does not own any long-running task execution.
pub(super) fn build_router(
    state: DevIpcBridgeState,
    config: &DevIpcBridgeConfig,
) -> Result<Router> {
    let allowed_origins = config
        .allowed_origins
        .iter()
        .map(|origin| HeaderValue::from_str(origin))
        .collect::<Result<Vec<_>, _>>()
        .context("parsing PathKeep dev IPC allowed origins")?;

    let cors = CorsLayer::new()
        .allow_methods([Method::GET, Method::POST])
        .allow_headers(Any)
        .allow_origin(AllowOrigin::list(allowed_origins));

    Ok(Router::new()
        .route("/health", get(bridge_health))
        .route("/commands/{command}", post(bridge_invoke))
        .layer(cors)
        .with_state(state))
}

/// Returns a tiny readiness payload for browser automation.
///
/// The health route intentionally exposes only product/runtime/port metadata,
/// not archive status or user data, because it is meant to identify the dev
/// transport rather than become another read model.
async fn bridge_health(
    State(state): State<DevIpcBridgeState>,
) -> Result<axum::Json<Value>, (StatusCode, axum::Json<Value>)> {
    Ok(axum::Json(json!({
        "ok": true,
        "productName": PRODUCT_DISPLAY_NAME,
        "runtime": "browser-desktop-bridge",
        "port": state.port,
    })))
}

/// Accepts one mirrored desktop command invocation from a local browser.
///
/// Empty request bodies map to an empty JSON object so no-argument commands can
/// use the same endpoint as typed commands. Malformed JSON is rejected before
/// dispatch, while command-level errors keep the existing string envelope.
async fn bridge_invoke(
    State(state): State<DevIpcBridgeState>,
    Path(command): Path<String>,
    body: Bytes,
) -> Result<axum::Json<Value>, (StatusCode, axum::Json<Value>)> {
    let payload = if body.is_empty() {
        Value::Object(Default::default())
    } else {
        serde_json::from_slice(&body)
            .map_err(|error| bad_request(format!("Invalid JSON payload: {error}")))?
    };

    dispatch_command(&state, &command, payload).await.map(axum::Json).map_err(internal_error)
}

/// Shapes client-side request decoding failures as JSON.
fn bad_request(message: String) -> (StatusCode, axum::Json<Value>) {
    (StatusCode::BAD_REQUEST, axum::Json(json!({ "error": message })))
}

/// Shapes command dispatch failures as JSON.
fn internal_error(message: String) -> (StatusCode, axum::Json<Value>) {
    (StatusCode::INTERNAL_SERVER_ERROR, axum::Json(json!({ "error": message })))
}
