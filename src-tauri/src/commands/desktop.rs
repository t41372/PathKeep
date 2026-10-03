//! Tauri commands for open at login and the menu bar icon.
//!
//! They touch login items, config and the tray, and the tray calls wait for
//! the main thread, so they run on the blocking pool.

#[cfg(not(test))]
use super::blocking::run_blocking_command;
#[cfg(not(test))]
use crate::{
    command_error::CommandError,
    desktop_integration::{self, DesktopIntegration},
};
#[cfg(not(test))]
use tauri::AppHandle;

#[cfg(not(test))]
#[tauri::command]
/// Reports whether PathKeep opens at login and shows a menu bar icon.
pub(crate) async fn get_desktop_integration(
    app: AppHandle,
) -> Result<DesktopIntegration, CommandError> {
    run_blocking_command("get_desktop_integration", move || {
        desktop_integration::desktop_integration(&app)
    })
    .await
}

#[cfg(not(test))]
#[tauri::command]
/// Registers or removes the login item; returns the state the OS reports after.
pub(crate) async fn set_launch_at_login(
    app: AppHandle,
    enabled: bool,
) -> Result<DesktopIntegration, CommandError> {
    run_blocking_command("set_launch_at_login", move || {
        desktop_integration::set_launch_at_login(&app, enabled)
    })
    .await
}

#[cfg(not(test))]
#[tauri::command]
/// Shows or removes the menu bar icon and saves the preference.
pub(crate) async fn set_menu_bar_icon(
    app: AppHandle,
    enabled: bool,
) -> Result<DesktopIntegration, CommandError> {
    run_blocking_command("set_menu_bar_icon", move || {
        desktop_integration::set_menu_bar_icon(&app, enabled)
    })
    .await
}
