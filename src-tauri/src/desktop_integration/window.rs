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
//!   the menu bar. With the icon off, closing the window quits, as before,
//!   unless work someone is waiting on is running: then the window stays and
//!   asks first (`quit.rs`).
//! - On Windows and Linux the window has no system title bar: the app draws
//!   its window buttons (`src/app/shell/window-controls.tsx`), so the window
//!   is the same two layers as on macOS (`back-forward-window-frame-tradeoff.md`
//!   D). It is turned off before the window first shows.
//!
//! ## Responsibilities
//! - Startup visibility, the close-button policy, and bringing the window back.
//!
//! ## Not responsible for
//! - Creating the icon (`tray.rs`), reading the preference (`mod.rs`), or
//!   whether a quit may go ahead (`quit.rs`).
//!
//! Failure modes: background startup creates a webview; a later foreground launch
//! cannot create it. Tests: owner_process and startup_window below.

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
                frame(&window);
                let _ = window.minimize();
                let _ = window.show();
                let _ = window.minimize();
            }
        }
    }
}

/// Shows, un-minimizes and focuses the main window.
pub(crate) fn show_main_window<R: Runtime>(app: &AppHandle<R>) {
    // Background owners create their first webview only when explicitly opened.
    let window = app.get_webview_window(MAIN_WINDOW).or_else(|| {
        let config = app.config().app.windows.iter().find(|window| window.label == MAIN_WINDOW)?;
        tauri::WebviewWindowBuilder::from_config(app, config)
            .and_then(|builder| builder.build())
            .inspect_err(|error| log::error!("opening main window: {error}"))
            .ok()
    });
    if let Some(window) = window {
        frame(&window);
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}

/// Off macOS, the window without the system's title bar (see the module header). macOS keeps its
/// overlay title bar and traffic lights (`titleBarStyle: "Overlay"`).
fn frame<R: Runtime>(window: &tauri::WebviewWindow<R>) {
    #[cfg(not(target_os = "macos"))]
    if let Err(error) = window.set_decorations(false) {
        log::warn!("could not turn off the system title bar: {error}");
    }
    #[cfg(target_os = "macos")]
    let _ = window;
}

/// The close button: while the menu bar icon exists it hides the main window instead of closing
/// it; without the icon closing quits, so running work first gets its question (`quit.rs`),
/// asked in this window, which therefore stays open.
pub(crate) fn on_window_event<R: Runtime>(window: &Window<R>, event: &WindowEvent) {
    if let WindowEvent::CloseRequested { api, .. } = event
        && window.label() == MAIN_WINDOW
    {
        if window.app_handle().tray_by_id(TRAY_ID).is_some() {
            api.prevent_close();
            let _ = window.hide();
        } else if super::quit::hold_quit(window.app_handle()) {
            api.prevent_close();
        }
    }
}
