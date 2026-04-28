//! Runtime configuration for the dev-only localhost IPC bridge.
//!
//! ## Responsibilities
//!
//! - Parse the feature-gated bridge environment variables.
//! - Keep localhost port and CORS origin defaults in one focused owner.
//! - Return a small immutable config object for the listener/router setup.
//!
//! ## Not responsible for
//!
//! - Starting sockets or registering HTTP routes.
//! - Dispatching desktop commands or interpreting command payloads.
//! - Providing any production remote-control configuration surface.
//!
//! ## Dependencies
//!
//! - `anyhow` for startup diagnostics when env vars are malformed.
//! - Parent-module constants for the public env-var names.
//!
//! ## Performance notes
//!
//! Config parsing happens once during dev bridge startup and does not sit on a
//! large-history or repeated command path.

use anyhow::{Context, Result};

use super::{
    DEFAULT_DEV_IPC_BRIDGE_PORT, DEV_IPC_BRIDGE_ALLOWED_ORIGINS_ENV, DEV_IPC_BRIDGE_ENABLED_ENV,
    DEV_IPC_BRIDGE_PORT_ENV,
};

/// Holds the listener port and allowed browser origins after env parsing.
///
/// The bridge is development-only, but this object keeps the security boundary
/// explicit: callers receive a concrete localhost port plus an allow-list used
/// by the router CORS layer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct DevIpcBridgeConfig {
    pub(super) port: u16,
    pub(super) allowed_origins: Vec<String>,
}

/// Reports whether the bridge should be launched for this process.
///
/// Only explicit truthy values enable the HTTP mirror, which prevents accidental
/// exposure when the `devtools-bridge` feature is compiled but the local dev
/// script did not opt in.
pub(super) fn bridge_enabled() -> bool {
    matches!(
        std::env::var(DEV_IPC_BRIDGE_ENABLED_ENV),
        Ok(value)
            if matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
    )
}

/// Resolves bridge startup config from process environment.
///
/// `PATHKEEP_DEV_IPC_PORT` must parse as `u16`; invalid values fail startup so
/// local automation does not silently attach to the wrong port. Empty CORS
/// origin entries are ignored after comma splitting.
pub(super) fn resolve_bridge_config_from_env() -> Result<DevIpcBridgeConfig> {
    let port = match std::env::var(DEV_IPC_BRIDGE_PORT_ENV) {
        Ok(value) => value
            .trim()
            .parse::<u16>()
            .with_context(|| format!("parsing {DEV_IPC_BRIDGE_PORT_ENV} as u16"))?,
        Err(_) => DEFAULT_DEV_IPC_BRIDGE_PORT,
    };

    let allowed_origins = std::env::var(DEV_IPC_BRIDGE_ALLOWED_ORIGINS_ENV)
        .unwrap_or_else(|_| "http://127.0.0.1:1420,http://localhost:1420".to_string())
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .collect::<Vec<_>>();

    Ok(DevIpcBridgeConfig { port, allowed_origins })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_bridge_config_from_env_with_defaults() {
        unsafe {
            std::env::remove_var(DEV_IPC_BRIDGE_ENABLED_ENV);
            std::env::remove_var(DEV_IPC_BRIDGE_PORT_ENV);
            std::env::remove_var(DEV_IPC_BRIDGE_ALLOWED_ORIGINS_ENV);
        }

        assert!(!bridge_enabled());
        let config = resolve_bridge_config_from_env().expect("resolve config");

        assert_eq!(config.port, DEFAULT_DEV_IPC_BRIDGE_PORT);
        assert_eq!(
            config.allowed_origins,
            vec!["http://127.0.0.1:1420".to_string(), "http://localhost:1420".to_string()]
        );

        unsafe {
            std::env::set_var(DEV_IPC_BRIDGE_ENABLED_ENV, " YES ");
            std::env::set_var(DEV_IPC_BRIDGE_PORT_ENV, "43118");
            std::env::set_var(
                DEV_IPC_BRIDGE_ALLOWED_ORIGINS_ENV,
                " http://127.0.0.1:1420, ,http://localhost:1420 ",
            );
        }

        assert!(bridge_enabled());
        let config = resolve_bridge_config_from_env().expect("resolve overridden config");
        assert_eq!(config.port, 43_118);
        assert_eq!(
            config.allowed_origins,
            vec!["http://127.0.0.1:1420".to_string(), "http://localhost:1420".to_string()]
        );

        unsafe {
            std::env::set_var(DEV_IPC_BRIDGE_ENABLED_ENV, "false");
            std::env::set_var(DEV_IPC_BRIDGE_PORT_ENV, "not-a-port");
        }

        assert!(!bridge_enabled());
        let invalid_port = resolve_bridge_config_from_env().expect_err("invalid bridge port");
        assert!(format!("{invalid_port:#}").contains("PATHKEEP_DEV_IPC_PORT"));

        unsafe {
            std::env::remove_var(DEV_IPC_BRIDGE_ENABLED_ENV);
            std::env::remove_var(DEV_IPC_BRIDGE_PORT_ENV);
            std::env::remove_var(DEV_IPC_BRIDGE_ALLOWED_ORIGINS_ENV);
        }
    }
}
