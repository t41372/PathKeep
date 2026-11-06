//! Tauri commands for open at login, the menu bar icon, and the question
//! PathKeep asks when someone quits while work is running.
//!
//! The first three touch login items, config and the tray, and the tray calls
//! wait for the main thread, so they run on the blocking pool. The quit
//! commands only read and change memory.

#[cfg(not(test))]
use super::blocking::run_blocking_command;
#[cfg(not(test))]
use crate::{
    command_error::CommandError,
    desktop_integration::{self, DesktopIntegration, QuitAnswer, QuitRequest},
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

#[cfg(not(test))]
#[tauri::command]
/// The open quit question, if a quit is held while work runs; a window that opens or reloads
/// after it was sent reads it here.
pub(crate) fn get_quit_request(app: AppHandle) -> Option<QuitRequest> {
    desktop_integration::quit_request(&app)
}

#[cfg(not(test))]
#[tauri::command]
/// Answers quit question `id` (wait, quit now, keep open); returns the question as it stands
/// after.
pub(crate) fn answer_quit_request(
    app: AppHandle,
    id: u64,
    answer: QuitAnswer,
) -> Option<QuitRequest> {
    desktop_integration::answer_quit_request(&app, id, answer)
}
