//! The menu bar (macOS) / system tray (Windows, Linux) icon and its menu.
//!
//! ## Responsibilities
//! - Create and remove the icon, and keep the menu text current.
//! - Turn menu clicks into actions: back up, search, open the window, quit.
//!
//! ## Not responsible for
//! - Whether the icon should exist (`mod.rs` reads the config) or what the
//!   status line says (`status.rs`, `copy.rs`).

use tauri::{
    AppHandle, Emitter, Manager, Runtime,
    image::Image,
    menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem},
    tray::TrayIconBuilder,
};

use super::copy::{MenuLanguage, menu_labels, status_text};
use super::status::{BackupSource, BackupStatus, run_backup};
use crate::session::SessionState;

/// Id of PathKeep's one tray icon.
pub(crate) const TRAY_ID: &str = "pathkeep";

/// Event asking the window to open the command palette ("Search history…").
pub(crate) const OPEN_COMMAND_PALETTE_EVENT: &str = "pathkeep://open-command-palette";

const ITEM_STATUS: &str = "tray-status";
const ITEM_BACK_UP: &str = "tray-back-up";
const ITEM_SEARCH: &str = "tray-search";
const ITEM_OPEN: &str = "tray-open";
const ITEM_QUIT: &str = "tray-quit";

/// Handles to the menu items whose text or state changes.
pub(crate) struct TrayMenu<R: Runtime> {
    status: MenuItem<R>,
    back_up: MenuItem<R>,
    search: MenuItem<R>,
    open: MenuItem<R>,
    quit: MenuItem<R>,
}

impl<R: Runtime> TrayMenu<R> {
    /// Rewrites every label for `language` and the current backup status.
    pub(crate) fn render(&self, language: MenuLanguage, status: &BackupStatus) {
        let labels = menu_labels(language);
        let line = status_text(language, &status.line(), chrono::Utc::now());
        // set_text only fails if the menu is gone, which a later render or
        // removal handles; there is nothing useful to report here.
        let _ = self.status.set_text(line);
        let _ = self.back_up.set_text(labels.back_up_now);
        let _ = self.back_up.set_enabled(status.can_back_up());
        let _ = self.search.set_text(labels.search_history);
        let _ = self.open.set_text(labels.open);
        let _ = self.quit.set_text(labels.quit);
    }
}

/// Creates the icon. The caller renders the labels right after.
pub(crate) fn create<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<TrayMenu<R>> {
    let item = |id: &str, enabled: bool| MenuItem::with_id(app, id, "", enabled, None::<&str>);
    let menu = TrayMenu {
        status: item(ITEM_STATUS, false)?,
        back_up: item(ITEM_BACK_UP, true)?,
        search: item(ITEM_SEARCH, true)?,
        open: item(ITEM_OPEN, true)?,
        quit: item(ITEM_QUIT, true)?,
    };
    let items = Menu::with_items(
        app,
        &[
            &menu.status,
            &menu.back_up,
            &menu.search,
            &menu.open,
            &PredefinedMenuItem::separator(app)?,
            &menu.quit,
        ],
    )?;

    let builder = TrayIconBuilder::with_id(TRAY_ID)
        .icon(icon())
        .tooltip("PathKeep")
        .menu(&items)
        .on_menu_event(on_menu_event);
    // macOS tints a template image for light and dark menu bars and opens
    // the menu on any click. Windows convention is a click opens the app and
    // a right-click shows the menu. Linux (libappindicator) only has the menu.
    #[cfg(target_os = "macos")]
    let builder = builder.icon_as_template(true);
    #[cfg(target_os = "windows")]
    let builder = builder.show_menu_on_left_click(false).on_tray_icon_event(|tray, event| {
        use tauri::tray::{MouseButton, MouseButtonState, TrayIconEvent};
        if let TrayIconEvent::Click {
            button: MouseButton::Left,
            button_state: MouseButtonState::Up,
            ..
        } = event
        {
            super::window::show_main_window(tray.app_handle());
        }
    });
    builder.build(app)?;
    Ok(menu)
}

/// Removes the icon if it exists.
pub(crate) fn remove<R: Runtime>(app: &AppHandle<R>) {
    // Dropping the returned handle is what takes the icon off the menu bar.
    drop(app.remove_tray_by_id(TRAY_ID));
}

fn icon() -> Image<'static> {
    // The 36 px template glyph (docs/design/brand-icon/tray, "A4"); macOS
    // draws it at 18 pt, so it stays sharp on Retina. A black glyph would
    // vanish on a dark Windows taskbar or Linux panel, so they get the
    // colour app icon.
    #[cfg(target_os = "macos")]
    {
        tauri::include_image!("icons/tray-Template@2x.png")
    }
    #[cfg(not(target_os = "macos"))]
    {
        tauri::include_image!("icons/32x32.png")
    }
}

fn on_menu_event<R: Runtime>(app: &AppHandle<R>, event: MenuEvent) {
    match event.id().as_ref() {
        ITEM_BACK_UP => {
            let app = app.clone();
            let key = app.try_state::<SessionState>().and_then(|state| state.get_key());
            std::thread::spawn(move || {
                // The result reaches the user through the status line and
                // the backup-finished event; log failures for diagnostics.
                if let Err(error) = run_backup(&app, BackupSource::MenuBar, false, key.as_deref()) {
                    log::warn!("menu bar backup failed: {}", error.message);
                }
            });
        }
        ITEM_SEARCH => {
            super::window::show_main_window(app);
            let _ = app.emit(OPEN_COMMAND_PALETTE_EVENT, ());
        }
        ITEM_OPEN => super::window::show_main_window(app),
        ITEM_QUIT => app.exit(0),
        _ => {}
    }
}
