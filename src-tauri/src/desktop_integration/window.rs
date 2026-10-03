//! When the main window shows, hides, or closes the app.
//!
//! Rules:
//! - The window starts hidden (`"visible": false` in `tauri.conf.json`) and
//!   setup shows it, except after a login launch with the menu bar icon on.
//!   That launch runs in the menu bar only, as the Settings copy promises.
//! - A login launch with the icon off shows the window as usual: on Windows
//!   and Linux nothing else would let the user reach a hidden app.
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

/// Whether the main window should start hidden.
pub(crate) fn start_hidden(launched_at_login: bool, menu_bar_icon_shown: bool) -> bool {
    launched_at_login && menu_bar_icon_shown
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
    use super::start_hidden;

    #[test]
    fn only_a_login_launch_with_the_icon_starts_hidden() {
        assert!(start_hidden(true, true));
        assert!(!start_hidden(true, false), "no icon: the window is the only way in");
        assert!(!start_hidden(false, true));
        assert!(!start_hidden(false, false));
    }
}
