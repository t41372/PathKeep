//! Open at login and the menu bar icon (Settings → General).
//!
//! Commands: `get_desktop_integration`, `set_launch_at_login`,
//! `set_menu_bar_icon`, each returning [`DesktopIntegration`] as the OS has it
//! after the call. "Open at login" is not stored in PathKeep's config — the OS
//! login item is the only record. The menu bar preference is stored as
//! `AppConfig::menu_bar_icon` and restored at launch.
//!
//! ## Responsibilities
//! - Launch-time setup: restore the icon, decide whether the window shows.
//! - The commands above, and keeping the icon in step with config changes.
//! - The shared state the menu reads (status line, language).
//!
//! ## Not responsible for
//! - Drawing the menu (`tray.rs`), its text (`copy.rs`), backup status
//!   bookkeeping (`status.rs`), login items (`login_item.rs`) or window
//!   behaviour (`window.rs`).
//!
//! ## Threading
//! Menu updates are dispatched to the main thread and wait for it, and they
//! run while holding [`DesktopIntegrationState`]'s lock. So that lock must
//! never be taken on the main thread after setup: the commands are async,
//! and the menu click handler hands its work to another thread.

mod copy;
mod login_item;
mod status;
mod tray;
mod window;

use std::{sync::Mutex, time::Duration};

use serde::Serialize;
use tauri::{AppHandle, Manager, Runtime};
use vault_core::AppConfig;

use crate::{command_error::CommandError, session::SessionState};
use copy::MenuLanguage;
use status::{ArchiveReadiness, BackupStatus, read_backup_status_from_disk};
use tray::{TRAY_ID, TrayMenu};

// Used by `lib.rs` and the commands, which are compiled out of unit tests.
#[cfg_attr(test, allow(unused_imports))]
pub(crate) use login_item::LAUNCHED_AT_LOGIN_ARG;
#[cfg_attr(test, allow(unused_imports))]
pub(crate) use status::{BackupSource, run_backup};
#[cfg_attr(test, allow(unused_imports))]
pub(crate) use window::{on_window_event, show_main_window};

/// How often the "12 min ago" text is redrawn (no I/O).
const RELABEL_INTERVAL: Duration = Duration::from_secs(60);
/// Every this many relabels, re-read the scheduled-backup ledger, which the
/// worker sidecar updates from another process.
const LEDGER_EVERY_N_RELABELS: u32 = 5;

/// State of the two Settings toggles, as the OS has it.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DesktopIntegration {
    pub(crate) launch_at_login: bool,
    pub(crate) menu_bar_icon: bool,
    pub(crate) launch_at_login_supported: bool,
    pub(crate) menu_bar_icon_supported: bool,
}

/// What the menu shows. See the module header for the locking rule.
pub(crate) struct DesktopIntegrationState<R: Runtime>(Mutex<MenuState<R>>);

struct MenuState<R: Runtime> {
    menu: Option<TrayMenu<R>>,
    status: BackupStatus,
    language: MenuLanguage,
}

impl<R: Runtime> MenuState<R> {
    fn render(&self) {
        if let Some(menu) = &self.menu {
            menu.render(self.language, &self.status);
        }
    }
}

fn with_state<R: Runtime, T>(app: &AppHandle<R>, f: impl FnOnce(&mut MenuState<R>) -> T) -> T {
    if app.try_state::<DesktopIntegrationState<R>>().is_none() {
        app.manage(DesktopIntegrationState(Mutex::new(MenuState::<R> {
            menu: None,
            status: BackupStatus::default(),
            language: MenuLanguage::default(),
        })));
    }
    let state = app.state::<DesktopIntegrationState<R>>();
    let mut guard = state.0.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    f(&mut guard)
}

/// Applies `change` to the backup status and redraws the menu.
pub(crate) fn update_status<R: Runtime>(
    app: &AppHandle<R>,
    change: impl FnOnce(&mut BackupStatus),
) {
    with_state(app, |state| {
        change(&mut state.status);
        state.render();
    });
}

fn menu_language(config: &AppConfig) -> MenuLanguage {
    MenuLanguage::resolve(&config.preferred_language, &vault_platform::preferred_ui_languages())
}

/// Launch-time setup, called from `lib.rs` before the window is shown.
pub(crate) fn setup<R: Runtime>(app: &AppHandle<R>, config: &AppConfig, launched_at_login: bool) {
    with_state(app, |state| state.language = menu_language(config));
    let icon_shown = config.menu_bar_icon
        && show_icon(app)
            .inspect_err(|error| log::warn!("menu bar icon: {}", error.message))
            .is_ok();
    if !window::start_hidden(launched_at_login, icon_shown) {
        window::show_main_window(app);
    }
    if let Err(error) = login_item::refresh() {
        log::warn!("could not update the login item: {error:#}");
    }

    let app = app.clone();
    std::thread::spawn(move || {
        refresh_status_from_disk(&app);
        for tick in 1u32.. {
            std::thread::sleep(RELABEL_INTERVAL);
            if app.tray_by_id(TRAY_ID).is_none() {
                continue;
            }
            if tick % LEDGER_EVERY_N_RELABELS == 0 {
                refresh_status_from_disk(&app);
            } else {
                with_state(&app, |state| state.render());
            }
        }
    });
}

/// Reads backup status from disk into the menu. Blocking; not on the main thread.
///
/// The archive is only opened while its state is not yet known to be ready
/// (first launch before onboarding, or locked); after that the ledger and
/// in-app backups keep the status current.
fn refresh_status_from_disk<R: Runtime>(app: &AppHandle<R>) {
    let include_archive =
        with_state(app, |state| state.status.archive() != ArchiveReadiness::Ready);
    let key = app.try_state::<SessionState>().and_then(|session| session.get_key());
    let (archive, successes) = read_backup_status_from_disk(key.as_deref(), include_archive);
    update_status(app, |status| {
        if let Some(archive) = archive {
            status.set_archive(archive);
        }
        for at in successes {
            status.record_success(at);
        }
    });
}

/// Keeps the menu in step after Settings saves the config: language, and the
/// icon itself if the saved value differs from what is on screen.
pub(crate) fn config_saved<R: Runtime>(app: &AppHandle<R>, config: &AppConfig) {
    let icon_shown = app.tray_by_id(TRAY_ID).is_some();
    if config.menu_bar_icon && !icon_shown {
        if let Err(error) = show_icon(app) {
            log::warn!("menu bar icon: {}", error.message);
        }
    } else if !config.menu_bar_icon && icon_shown {
        hide_icon(app);
    }
    with_state(app, |state| {
        state.language = menu_language(config);
        state.render();
    });
}

fn show_icon<R: Runtime>(app: &AppHandle<R>) -> Result<(), CommandError> {
    if app.tray_by_id(TRAY_ID).is_some() {
        return Ok(());
    }
    if !vault_platform::menu_bar_icon_supported() {
        return Err(CommandError::internal(
            "This desktop has no menu bar / tray area PathKeep can use.",
        ));
    }
    let menu = tray::create(app)
        .map_err(|error| CommandError::internal(format!("creating the menu bar icon: {error}")))?;
    with_state(app, |state| {
        state.menu = Some(menu);
        state.render();
    });
    Ok(())
}

fn hide_icon<R: Runtime>(app: &AppHandle<R>) {
    with_state(app, |state| state.menu = None);
    tray::remove(app);
    // With the icon gone, a hidden window would leave no way back in.
    window::show_main_window(app);
}

/// `get_desktop_integration`. Blocking; not on the main thread.
pub(crate) fn desktop_integration<R: Runtime>(
    app: &AppHandle<R>,
) -> Result<DesktopIntegration, CommandError> {
    Ok(DesktopIntegration {
        launch_at_login: login_item::enabled(app).map_err(|error| {
            CommandError::internal(format!("reading the login item: {error:#}"))
        })?,
        menu_bar_icon: app.tray_by_id(TRAY_ID).is_some(),
        launch_at_login_supported: true,
        menu_bar_icon_supported: vault_platform::menu_bar_icon_supported(),
    })
}

/// `set_launch_at_login`. Blocking; not on the main thread.
pub(crate) fn set_launch_at_login<R: Runtime>(
    app: &AppHandle<R>,
    enabled: bool,
) -> Result<DesktopIntegration, CommandError> {
    login_item::set_enabled(app, enabled)
        .map_err(|error| CommandError::internal(format!("changing the login item: {error:#}")))?;
    desktop_integration(app)
}

/// `set_menu_bar_icon`: shows or removes the icon, then saves the preference.
/// If saving fails the icon change is undone, so screen and config agree.
/// Blocking; not on the main thread.
pub(crate) fn set_menu_bar_icon<R: Runtime>(
    app: &AppHandle<R>,
    enabled: bool,
) -> Result<DesktopIntegration, CommandError> {
    let was_shown = app.tray_by_id(TRAY_ID).is_some();
    if enabled {
        show_icon(app)?;
    } else {
        hide_icon(app);
    }
    if let Err(error) = save_menu_bar_icon(enabled) {
        if was_shown && !enabled {
            let _ = show_icon(app);
        } else if !was_shown && enabled {
            hide_icon(app);
        }
        return Err(CommandError::internal(format!("saving the menu bar setting: {error:#}")));
    }
    desktop_integration(app)
}

fn save_menu_bar_icon(enabled: bool) -> anyhow::Result<()> {
    let paths = vault_core::project_paths()?;
    let mut config = vault_core::load_config(&paths)?;
    if config.menu_bar_icon != enabled {
        config.menu_bar_icon = enabled;
        vault_core::save_config(&paths, &config)?;
    }
    Ok(())
}
