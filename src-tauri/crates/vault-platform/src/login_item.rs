//! "Open at login" as a LaunchAgent plist, for macOS and for the debug sandbox.
//!
//! On macOS PathKeep does not use `tauri-plugin-autostart` for this. The
//! plugin's LaunchAgent has launchd exec `PathKeep.app/Contents/MacOS/...`
//! directly, and since macOS 26 that is killed with "Launch Constraint
//! Violation" (the same crash fixed for scheduled backups in `167d1aa8`).
//! This agent runs `/usr/bin/open -g -a PathKeep.app --args
//! --launched-at-login` instead, so Launch Services starts the app. It is not
//! loaded with `launchctl`; launchd picks it up at the next login.
//!
//! In a debug build with the sandbox on (`crate::sandbox`), the same plist is
//! written under `<sandbox>/login-items` on every OS, so dev runs and tests
//! never register a real login item. Windows and Linux outside the sandbox
//! use the autostart plugin in the desktop crate.
//!
//! ## Responsibilities
//! - Build the login LaunchAgent and decide where it lives.
//! - Report whether it is installed, install it, remove it, and rewrite it
//!   when the app has moved.
//!
//! ## Not responsible for
//! - What the app does when started with [`LAUNCHED_AT_LOGIN_ARG`].
//! - Windows / Linux login items outside the sandbox.

use anyhow::{Context, Result};
use std::{
    fs,
    path::{Path, PathBuf},
};

use crate::sandbox::{LOGIN_ITEMS_SUBDIR, sandbox_dir};

/// Argument the login item passes, so the app knows it was started at login.
pub const LAUNCHED_AT_LOGIN_ARG: &str = "--launched-at-login";

/// launchd label of the login item (separate from the backup schedule's label).
pub const LOGIN_ITEM_LABEL: &str = "com.yi-ting.pathkeep.login";

/// A directory holding PathKeep's login LaunchAgent.
#[derive(Debug, Clone)]
pub struct LoginItemStore {
    dir: PathBuf,
}

/// Returns where the login item lives, or `None` when the caller should use
/// the platform's own mechanism (Windows / Linux outside the sandbox).
pub fn login_item_store() -> Result<Option<LoginItemStore>> {
    if let Some(sandbox) = sandbox_dir() {
        return Ok(Some(LoginItemStore::at(sandbox.join(LOGIN_ITEMS_SUBDIR))));
    }
    native_login_item_store()
}

#[cfg(target_os = "macos")]
fn native_login_item_store() -> Result<Option<LoginItemStore>> {
    let home = directories::UserDirs::new().context("resolving home dir")?.home_dir().to_owned();
    Ok(Some(LoginItemStore::at(home.join("Library/LaunchAgents"))))
}

#[cfg(not(target_os = "macos"))]
fn native_login_item_store() -> Result<Option<LoginItemStore>> {
    Ok(None)
}

impl LoginItemStore {
    /// Uses `dir` as the LaunchAgents directory.
    pub fn at(dir: PathBuf) -> Self {
        Self { dir }
    }

    /// Path of the plist this store reads and writes.
    pub fn plist_path(&self) -> PathBuf {
        self.dir.join(format!("{LOGIN_ITEM_LABEL}.plist"))
    }

    /// Whether the login item is installed.
    pub fn is_enabled(&self) -> bool {
        self.plist_path().is_file()
    }

    /// Installs the login item for `executable`.
    pub fn enable(&self, executable: &Path) -> Result<()> {
        fs::create_dir_all(&self.dir)
            .with_context(|| format!("creating {}", self.dir.display()))?;
        let path = self.plist_path();
        fs::write(&path, login_item_plist(executable)?)
            .with_context(|| format!("writing {}", path.display()))
    }

    /// Removes the login item if it is installed.
    pub fn disable(&self) -> Result<()> {
        let path = self.plist_path();
        match fs::remove_file(&path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error).with_context(|| format!("removing {}", path.display())),
        }
    }

    /// Rewrites an installed login item whose contents no longer match
    /// `executable`, e.g. after the app was moved or updated in place.
    /// Returns whether it rewrote anything.
    pub fn refresh(&self, executable: &Path) -> Result<bool> {
        let Ok(current) = fs::read(self.plist_path()) else {
            return Ok(false);
        };
        if current == login_item_plist(executable)? {
            return Ok(false);
        }
        self.enable(executable)?;
        Ok(true)
    }
}

/// Arguments launchd runs at login for `executable`.
///
/// Inside an `.app` bundle this goes through `open` so Launch Services is
/// the parent (see the module header). Outside a bundle (dev builds) the
/// binary is run directly.
pub fn login_item_program_arguments(executable: &Path) -> Vec<String> {
    match crate::scheduler::enclosing_app_bundle(executable) {
        Some(bundle) => vec![
            "/usr/bin/open".to_string(),
            "-g".to_string(),
            "-a".to_string(),
            bundle.display().to_string(),
            "--args".to_string(),
            LAUNCHED_AT_LOGIN_ARG.to_string(),
        ],
        None => vec![executable.display().to_string(), LAUNCHED_AT_LOGIN_ARG.to_string()],
    }
}

fn login_item_plist(executable: &Path) -> Result<Vec<u8>> {
    let mut dict = plist::Dictionary::new();
    dict.insert("Label".into(), LOGIN_ITEM_LABEL.into());
    dict.insert(
        "ProgramArguments".into(),
        plist::Value::Array(
            login_item_program_arguments(executable).into_iter().map(Into::into).collect(),
        ),
    );
    dict.insert("RunAtLoad".into(), true.into());
    // Only in a graphical login session, never at an SSH login.
    dict.insert("LimitLoadToSessionType".into(), "Aqua".into());
    dict.insert("ProcessType".into(), "Interactive".into());
    let mut bytes = Vec::new();
    plist::Value::Dictionary(dict)
        .to_writer_xml(&mut bytes)
        .context("serializing the login item plist")?;
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn bundled_app_starts_at_login_through_launch_services() {
        let args = login_item_program_arguments(Path::new(
            "/Applications/PathKeep.app/Contents/MacOS/pathkeep-desktop",
        ));
        assert_eq!(
            args,
            [
                "/usr/bin/open",
                "-g",
                "-a",
                "/Applications/PathKeep.app",
                "--args",
                LAUNCHED_AT_LOGIN_ARG
            ]
        );
    }

    #[test]
    fn unbundled_binary_runs_directly() {
        let args = login_item_program_arguments(Path::new("/repo/target/debug/pathkeep-desktop"));
        assert_eq!(args, ["/repo/target/debug/pathkeep-desktop", LAUNCHED_AT_LOGIN_ARG]);
    }

    #[test]
    fn enable_refresh_and_disable_round_trip() {
        let dir = tempdir().expect("tempdir");
        let store = LoginItemStore::at(dir.path().join("agents"));
        let old = Path::new("/Applications/PathKeep.app/Contents/MacOS/pathkeep-desktop");
        let moved = Path::new("/Users/me/Apps/PathKeep.app/Contents/MacOS/pathkeep-desktop");

        assert!(!store.is_enabled());
        assert!(!store.refresh(old).expect("refresh while off"), "refresh must not enable");
        assert!(!store.is_enabled());

        store.enable(old).expect("enable");
        assert!(store.is_enabled());
        let value = plist::Value::from_file(store.plist_path()).expect("valid plist");
        let dict = value.as_dictionary().expect("dict");
        assert_eq!(dict.get("Label").and_then(plist::Value::as_string), Some(LOGIN_ITEM_LABEL));
        assert_eq!(dict.get("RunAtLoad").and_then(plist::Value::as_boolean), Some(true));

        assert!(!store.refresh(old).expect("refresh unchanged"));
        assert!(store.refresh(moved).expect("refresh moved"));
        let rewritten = fs::read_to_string(store.plist_path()).expect("read");
        assert!(rewritten.contains("/Users/me/Apps/PathKeep.app"));

        store.disable().expect("disable");
        assert!(!store.is_enabled());
        store.disable().expect("disable twice is fine");
    }

    #[test]
    #[cfg(debug_assertions)]
    fn sandbox_moves_the_login_item_out_of_the_real_launch_agents() {
        let _guard =
            crate::test_support::env_lock().lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let dir = tempdir().expect("tempdir");
        let original = std::env::var_os(crate::SANDBOX_DIR_ENV);
        unsafe { std::env::set_var(crate::SANDBOX_DIR_ENV, dir.path()) };
        let store = login_item_store();
        crate::test_support::restore_env_var(crate::SANDBOX_DIR_ENV, original.as_deref());

        let store = store.expect("store").expect("sandbox store on every OS");
        assert!(store.plist_path().starts_with(dir.path().join(LOGIN_ITEMS_SUBDIR)));
    }
}
