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
#[cfg(not(test))]
use anyhow::Context;
use anyhow::{Result, anyhow};
#[cfg(not(test))]
use std::net::{Ipv4Addr, SocketAddr};
use std::sync::{Arc, RwLock};
use tauri::AppHandle;

mod config;
mod dispatch;
mod payloads;
mod router;

use config::{bridge_enabled, resolve_bridge_config_from_env};
#[cfg(not(test))]
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
    app: Arc<RwLock<Option<DevIpcAppHandle>>>,
    session: SessionState,
    port: u16,
}

impl DevIpcBridgeState {
    fn new(app: Arc<RwLock<Option<DevIpcAppHandle>>>, session: SessionState, port: u16) -> Self {
        Self { app, session, port }
    }

    #[cfg(test)]
    fn without_app(session: SessionState, port: u16) -> Self {
        Self::new(Arc::new(RwLock::new(None)), session, port)
    }

    #[cfg(test)]
    fn with_app(app: DevIpcAppHandle, session: SessionState, port: u16) -> Self {
        Self::new(Arc::new(RwLock::new(Some(app))), session, port)
    }
}

/// Holds the mutable AppHandle slot shared by the preflight bridge thread.
///
/// The desktop truth gate needs the localhost command mirror to become ready
/// even when macOS delays or blocks Tauri window setup. Commands that do not
/// require a live `AppHandle` can run immediately; updater-only commands keep
/// failing honestly until setup attaches the handle.
#[derive(Clone)]
pub(crate) struct DevIpcBridgeHandle {
    app: Arc<RwLock<Option<DevIpcAppHandle>>>,
}

impl DevIpcBridgeHandle {
    fn new(app: Arc<RwLock<Option<DevIpcAppHandle>>>) -> Self {
        Self { app }
    }

    /// Attaches the live Tauri handle once GUI setup reaches the app boundary.
    pub(crate) fn attach_app(&self, app: DevIpcAppHandle) -> Result<()> {
        let mut guard = self
            .app
            .write()
            .map_err(|_| anyhow!("PathKeep dev IPC bridge AppHandle slot is poisoned"))?;
        *guard = Some(app);
        Ok(())
    }
}

#[cfg(not(test))]
type DevIpcAppHandle = AppHandle;
#[cfg(test)]
type DevIpcAppHandle = AppHandle<tauri::test::MockRuntime>;

/// Starts the localhost dev bridge when the feature flag and env vars allow it.
#[cfg(not(test))]
pub(crate) fn maybe_launch(session: SessionState) -> Result<Option<DevIpcBridgeHandle>> {
    if !bridge_enabled() {
        return Ok(None);
    }

    let config = resolve_bridge_config_from_env()?;
    let listener =
        std::net::TcpListener::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, config.port)))
            .with_context(|| format!("binding PathKeep dev IPC bridge on port {}", config.port))?;
    listener
        .set_nonblocking(true)
        .context("marking PathKeep dev IPC bridge listener as non-blocking")?;

    let app = Arc::new(RwLock::new(None));
    let bridge_handle = DevIpcBridgeHandle::new(app.clone());
    let app_state = DevIpcBridgeState::new(app, session, config.port);
    let app_router = build_router(app_state, &config)?;
    let port = config.port;

    std::thread::Builder::new()
        .name("pathkeep-dev-ipc-bridge".to_string())
        .spawn(move || {
            let runtime = match tokio::runtime::Builder::new_current_thread().enable_all().build() {
                Ok(runtime) => runtime,
                Err(error) => {
                    log::error!("PathKeep dev IPC bridge failed to build runtime: {error:#}");
                    return;
                }
            };

            runtime.block_on(async move {
                let listener = match tokio::net::TcpListener::from_std(listener) {
                    Ok(listener) => listener,
                    Err(error) => {
                        log::error!(
                            "PathKeep dev IPC bridge failed to adopt TCP listener: {error:#}"
                        );
                        return;
                    }
                };

                if let Err(error) = axum::serve(listener, app_router).await {
                    log::error!("PathKeep dev IPC bridge crashed: {error:#}");
                }
            });
        })
        .context("spawning PathKeep dev IPC bridge thread")?;

    log::info!(
        "PathKeep dev IPC bridge listening on http://127.0.0.1:{} for Chrome/Playwright automation.",
        port
    );
    Ok(Some(bridge_handle))
}

#[cfg(test)]
pub(crate) fn maybe_launch(_session: SessionState) -> Result<Option<DevIpcBridgeHandle>> {
    if bridge_enabled() {
        resolve_bridge_config_from_env()?;
        return Ok(Some(DevIpcBridgeHandle::new(Arc::new(RwLock::new(None)))));
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maybe_launch_noops_when_bridge_env_is_disabled() {
        unsafe {
            std::env::remove_var(DEV_IPC_BRIDGE_ENABLED_ENV);
            std::env::remove_var(DEV_IPC_BRIDGE_PORT_ENV);
            std::env::remove_var(DEV_IPC_BRIDGE_ALLOWED_ORIGINS_ENV);
        }
        assert!(maybe_launch(SessionState::default()).expect("disabled bridge no-op").is_none());
    }

    #[test]
    fn bridge_handle_attaches_app_and_reports_poisoned_slots() {
        let app = tauri::test::mock_builder()
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .expect("mock app");
        let app_slot = Arc::new(RwLock::new(None));
        let handle = DevIpcBridgeHandle::new(app_slot.clone());

        handle.attach_app(app.handle().clone()).expect("attach app handle");

        assert!(app_slot.read().expect("read slot").is_some());

        let poisoned_slot = Arc::new(RwLock::new(None));
        let slot_for_panic = poisoned_slot.clone();
        let _ = std::panic::catch_unwind(move || {
            let _guard = slot_for_panic.write().expect("write slot");
            panic!("poison app handle slot");
        });
        let poisoned_handle = DevIpcBridgeHandle::new(poisoned_slot);

        let error = poisoned_handle
            .attach_app(app.handle().clone())
            .expect_err("poisoned slot should fail");

        assert!(format!("{error:#}").contains("AppHandle slot is poisoned"));
    }
}
