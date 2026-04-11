//! Platform-level browser discovery entrypoint.
//!
//! Today this adapter simply delegates to `vault-core`'s discovery logic, but
//! it keeps the platform crate's public surface stable in case discovery ever
//! needs additional host-specific behavior.

use anyhow::Result;

/// Discovers browser profiles visible on the current host.
pub fn discover_browser_profiles() -> Result<Vec<vault_core::BrowserProfile>> {
    vault_core::discover_profiles()
}
