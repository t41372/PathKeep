//! The macOS menu bar's menus, in the app's language, with a Quit (⌘Q) that asks first while
//! work is running.
//!
//! Tauri's default menu quits through AppKit's `terminate:`, which ends the process without
//! asking the app (tao 0.35 has no `applicationShouldTerminate:`), so ⌘Q threw away a running
//! export or import without a word. These are the same menus with PathKeep's own Quit, which
//! calls `AppHandle::exit` and so goes through `quit::hold_quit` like every other way to quit.
//! Tauri writes its menus in English; these take their words from `copy.rs` and are built again
//! when Settings changes the language (`mod.rs`, `config_saved`).
//!
//! They are installed at setup, once the config says which language to use; until then the app
//! has no menu (`enable_macos_default_menu(false)` in `lib.rs`).
//!
//! ## Responsibilities
//! - Build and install the menu bar menus, and handle their Quit.
//!
//! ## Not responsible for
//! - Whether the quit goes ahead (`quit.rs`), the menu bar icon's menu (`tray.rs`), or the
//!   labels' words (`copy.rs`).
//!
//! Failure modes: menus that cannot be built, which leaves Tauri's default menus (English, and
//! their Quit cannot ask; still better than no menu, which also loses Edit's copy and paste, and
//! the log says which is in use); ⌘Q bound twice; a language change that keeps the old labels.
//! Tests: none can build a menu off the main thread; the E2E spec raises the same
//! `AppHandle::exit` this item calls.

use super::copy::{MenuLanguage, app_menu_labels};
use tauri::{
    AppHandle, Runtime,
    menu::{
        AboutMetadata, HELP_SUBMENU_ID, Menu, MenuEvent, MenuItem, PredefinedMenuItem, Submenu,
        WINDOW_SUBMENU_ID,
    },
};

/// Id of the app menu's Quit item.
const QUIT_ITEM: &str = "app-quit";

/// Sets the menu bar to PathKeep's menus in `language`, or to Tauri's default menus if they
/// cannot be built. Call on the main thread.
pub(crate) fn install<R: Runtime>(app: &AppHandle<R>, language: MenuLanguage) {
    let menu = build(app, language).or_else(|error| {
        log::warn!("could not build the menu bar, using Tauri's; ⌘Q quits without asking: {error}");
        Menu::default(app)
    });
    if let Err(error) = menu.and_then(|menu| app.set_menu(menu)) {
        log::error!("could not set the menu bar: {error}");
    }
}

/// Tauri's default menus (`Menu::default`), item for item, with `labels` and PathKeep's Quit.
fn build<R: Runtime>(app: &AppHandle<R>, language: MenuLanguage) -> tauri::Result<Menu<R>> {
    let labels = app_menu_labels(language);
    let package = app.package_info();
    let config = app.config();
    let about = AboutMetadata {
        name: Some(package.name.clone()),
        version: Some(package.version.to_string()),
        copyright: config.bundle.copyright.clone(),
        authors: config.bundle.publisher.clone().map(|publisher| vec![publisher]),
        ..Default::default()
    };
    let separator = || PredefinedMenuItem::separator(app);
    let quit = MenuItem::with_id(app, QUIT_ITEM, labels.quit, true, Some("CmdOrCtrl+Q"))?;
    Menu::with_items(
        app,
        &[
            &Submenu::with_items(
                app,
                &package.name,
                true,
                &[
                    &PredefinedMenuItem::about(app, Some(labels.about), Some(about))?,
                    &separator()?,
                    &PredefinedMenuItem::services(app, Some(labels.services))?,
                    &separator()?,
                    &PredefinedMenuItem::hide(app, Some(labels.hide))?,
                    &PredefinedMenuItem::hide_others(app, Some(labels.hide_others))?,
                    &separator()?,
                    &quit,
                ],
            )?,
            &Submenu::with_items(
                app,
                labels.file,
                true,
                &[&PredefinedMenuItem::close_window(app, Some(labels.close_window))?],
            )?,
            &Submenu::with_items(
                app,
                labels.edit,
                true,
                &[
                    &PredefinedMenuItem::undo(app, Some(labels.undo))?,
                    &PredefinedMenuItem::redo(app, Some(labels.redo))?,
                    &separator()?,
                    &PredefinedMenuItem::cut(app, Some(labels.cut))?,
                    &PredefinedMenuItem::copy(app, Some(labels.copy))?,
                    &PredefinedMenuItem::paste(app, Some(labels.paste))?,
                    &PredefinedMenuItem::select_all(app, Some(labels.select_all))?,
                ],
            )?,
            &Submenu::with_items(
                app,
                labels.view,
                true,
                &[&PredefinedMenuItem::fullscreen(app, Some(labels.full_screen))?],
            )?,
            &Submenu::with_id_and_items(
                app,
                WINDOW_SUBMENU_ID,
                labels.window,
                true,
                &[
                    &PredefinedMenuItem::minimize(app, Some(labels.minimize))?,
                    &PredefinedMenuItem::maximize(app, Some(labels.zoom))?,
                    &separator()?,
                    &PredefinedMenuItem::close_window(app, Some(labels.close_window))?,
                ],
            )?,
            &Submenu::with_id_and_items(app, HELP_SUBMENU_ID, labels.help, true, &[])?,
        ],
    )
}

/// Menu bar clicks; only Quit is PathKeep's own.
pub(crate) fn on_menu_event<R: Runtime>(app: &AppHandle<R>, event: MenuEvent) {
    if event.id().as_ref() == QUIT_ITEM {
        app.exit(0);
    }
}
