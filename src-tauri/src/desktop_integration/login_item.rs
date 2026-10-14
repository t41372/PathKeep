//! "Open at login": read and change what the OS has registered.
//!
//! macOS (and every OS while the debug sandbox is on) uses the LaunchAgent in
//! `vault_platform::login_item`; see there for why macOS does not use the
//! autostart plugin. Windows and Linux use `tauri-plugin-autostart` (a `Run`
//! registry value / an XDG autostart entry), registered in `lib.rs` with
//! [`LAUNCHED_AT_LOGIN_ARG`].
//!
//! ## Responsibilities
//! - Report and change whether PathKeep starts at login.
//!
//! ## Not responsible for
//! - What a login launch does (`window.rs`).

use anyhow::{Context, Result, anyhow};
use tauri::{AppHandle, Manager, Runtime, State};
use tauri_plugin_autostart::AutoLaunchManager;

pub(crate) use vault_platform::LAUNCHED_AT_LOGIN_ARG;

/// Whether PathKeep is registered to start at login.
pub(crate) fn enabled<R: Runtime>(app: &AppHandle<R>) -> Result<bool> {
    match vault_platform::login_item_store()? {
        Some(store) => Ok(store.is_enabled()),
        None => plugin(app)?.is_enabled().map_err(|error| anyhow!(error.to_string())),
    }
}

/// Registers or unregisters PathKeep to start at login.
pub(crate) fn set_enabled<R: Runtime>(app: &AppHandle<R>, enabled: bool) -> Result<()> {
    match vault_platform::login_item_store()? {
        Some(store) if enabled => store.enable(&std::env::current_exe()?),
        Some(store) => store.disable(),
        None => {
            let manager = plugin(app)?;
            let result = if enabled { manager.enable() } else { manager.disable() };
            result.map_err(|error| anyhow!(error.to_string()))
        }
    }
}

/// Points an existing login item at this copy of the app, in case the app
/// was moved since it was registered. Windows and Linux re-register through
/// the plugin when the user toggles the setting; nothing to do here.
pub(crate) fn refresh() -> Result<()> {
    if let Some(store) = vault_platform::login_item_store()? {
        store.refresh(&std::env::current_exe()?)?;
    }
    Ok(())
}

fn plugin<R: Runtime>(app: &AppHandle<R>) -> Result<State<'_, AutoLaunchManager>> {
    app.try_state::<AutoLaunchManager>().context("the autostart plugin is not registered")
}
