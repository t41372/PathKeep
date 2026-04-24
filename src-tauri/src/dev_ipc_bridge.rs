#![cfg(feature = "devtools-bridge")]
#![cfg_attr(test, allow(dead_code))]

//! Dev-only HTTP bridge for browser automation and local tooling.
//!
//! ## Responsibilities
//!
//! - Start the feature-gated localhost listener used by browser automation.
//! - Keep bridge enablement, port binding, and router construction in the desktop layer.
//! - Carry the live app handle and session state into focused bridge submodules.
//!
//! ## Not responsible for
//!
//! - Defining command payload DTOs or command dispatch behavior.
//! - Owning archive, import, intelligence, updater, or file-manager domain logic.
//! - Exposing any production HTTP or remote-control API.
//!
//! ## Dependencies
//!
//! - Tauri app/session state for the local desktop runtime.
//! - `axum` for the dev-only HTTP listener.
//! - Focused `config`, `router`, `payloads`, and `dispatch` submodules.
//!
//! ## Performance notes
//!
//! The bridge accepts small command envelopes only. Long-running commands must
//! continue to execute behind existing off-main-thread worker bridge contracts.

use crate::session::SessionState;
use anyhow::{Context, Result};
use std::net::{Ipv4Addr, SocketAddr};
use tauri::AppHandle;

mod config;
mod dispatch;
mod payloads;
mod router;

use config::{bridge_enabled, resolve_bridge_config_from_env};
use router::build_router;

/// Env var flag that enables the localhost dev IPC bridge.
pub(crate) const DEV_IPC_BRIDGE_ENABLED_ENV: &str = "PATHKEEP_ENABLE_DEV_IPC_BRIDGE";
/// Env var override for the localhost dev IPC bridge port.
pub(crate) const DEV_IPC_BRIDGE_PORT_ENV: &str = "PATHKEEP_DEV_IPC_PORT";
/// Env var override for allowed CORS origins on the dev IPC bridge.
pub(crate) const DEV_IPC_BRIDGE_ALLOWED_ORIGINS_ENV: &str = "PATHKEEP_DEV_IPC_ALLOWED_ORIGINS";
/// Default localhost port used by the dev IPC bridge.
pub(crate) const DEFAULT_DEV_IPC_BRIDGE_PORT: u16 = 43_117;

#[derive(Clone)]
struct DevIpcBridgeState {
    app: Option<AppHandle>,
    session: SessionState,
    port: u16,
}

/// Starts the localhost dev bridge when the feature flag and env vars allow it.
pub(crate) fn maybe_launch(app: AppHandle, session: SessionState) -> Result<()> {
    if !bridge_enabled() {
        return Ok(());
    }

    let config = resolve_bridge_config_from_env()?;
    let listener =
        std::net::TcpListener::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, config.port)))
            .with_context(|| format!("binding PathKeep dev IPC bridge on port {}", config.port))?;
    listener
        .set_nonblocking(true)
        .context("marking PathKeep dev IPC bridge listener as non-blocking")?;

    let app_state = DevIpcBridgeState { app: Some(app), session, port: config.port };
    let app_router = build_router(app_state, &config)?;

    tauri::async_runtime::spawn(async move {
        let listener = match tokio::net::TcpListener::from_std(listener) {
            Ok(listener) => listener,
            Err(error) => {
                log::error!("PathKeep dev IPC bridge failed to adopt TCP listener: {error:#}");
                return;
            }
        };

        if let Err(error) = axum::serve(listener, app_router).await {
            log::error!("PathKeep dev IPC bridge crashed: {error:#}");
        }
    });

    log::info!(
        "PathKeep dev IPC bridge listening on http://127.0.0.1:{} for Chrome/Playwright automation.",
        config.port
    );
    Ok(())
}
