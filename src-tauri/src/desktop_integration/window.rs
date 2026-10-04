//! When the main window shows, hides, or closes the app.
//!
//! Rules:
//! - The window starts hidden (`"visible": false` in `tauri.conf.json`) and
//!   setup decides what to do with it.
//! - A normal launch shows it.
//! - A login launch never puts the window in front of the user:
//!   - with the menu bar icon on, it stays hidden — the app runs in the menu
//!     bar only, as the Settings copy promises;
//!   - with the icon off on macOS, it stays hidden too; the Dock icon is
//!     there, and clicking it brings the window back (`RunEvent::Reopen`);
//!   - with the icon off on Windows / Linux, it opens minimized, because a
//!     hidden window with no tray icon and no Dock could not be reached.
//! - With the icon on, closing the window hides it; the app keeps running in
//!   the menu bar. With the icon off, closing the window quits, as before.
//!
//! ## Responsibilities
//! - Startup visibility, the close-button policy, and bringing the window back.
//!
//! ## Not responsible for
//! - Creating the icon (`tray.rs`) or reading the preference (`mod.rs`).

use tauri::{AppHandle, Manager, Runtime, Window, WindowEvent};

use super::tray::TRAY_ID;

/// Label of the main window in `tauri.conf.json`.
pub(crate) const MAIN_WINDOW: &str = "main";

/// What setup does with the (initially hidden) main window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StartupWindow {
    Show,
    StayHidden,
    Minimized,
}

/// Applies the startup rules in the module header.
pub(crate) fn startup_window(
    launched_at_login: bool,
    menu_bar_icon_shown: bool,
    has_dock: bool,
) -> StartupWindow {
    match (launched_at_login, menu_bar_icon_shown || has_dock) {
        (false, _) => StartupWindow::Show,
        (true, true) => StartupWindow::StayHidden,
        (true, false) => StartupWindow::Minimized,
    }
}

/// Carries out [`startup_window`] for this launch.
pub(crate) fn apply_startup_window<R: Runtime>(app: &AppHandle<R>, startup: StartupWindow) {
    match startup {
        StartupWindow::Show => show_main_window(app),
        StartupWindow::StayHidden => {}
        StartupWindow::Minimized => {
            if let Some(window) = app.get_webview_window(MAIN_WINDOW) {
                let _ = window.minimize();
                let _ = window.show();
                let _ = window.minimize();
            }
        }
    }
}

/// Shows, un-minimizes and focuses the main window.
pub(crate) fn show_main_window<R: Runtime>(app: &AppHandle<R>) {
    if let Some(window) = app.get_webview_window(MAIN_WINDOW) {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}

/// Hides the main window instead of closing it while the menu bar icon exists.
pub(crate) fn on_window_event<R: Runtime>(window: &Window<R>, event: &WindowEvent) {
    if let WindowEvent::CloseRequested { api, .. } = event
        && window.label() == MAIN_WINDOW
        && window.app_handle().tray_by_id(TRAY_ID).is_some()
    {
        api.prevent_close();
        let _ = window.hide();
    }
}

#[cfg(test)]
mod tests {
    use super::{StartupWindow::*, startup_window};

    #[test]
    fn a_login_launch_never_puts_the_window_in_front() {
        // (launched at login, icon shown, has a Dock)
        assert_eq!(startup_window(false, false, false), Show);
        assert_eq!(startup_window(false, true, true), Show);
        assert_eq!(startup_window(true, true, false), StayHidden);
        assert_eq!(startup_window(true, false, true), StayHidden, "macOS: the Dock brings it back");
        assert_eq!(startup_window(true, false, false), Minimized, "otherwise unreachable");
    }
}
