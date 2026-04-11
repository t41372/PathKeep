//! Browser path and override helpers.
//!
//! This module resolves where browser profile roots may live on the current
//! host. It does not inspect or copy databases itself; it only explains where
//! discovery should look and how test/dev overrides narrow that search.

use super::*;

/// Returns whether a Chromium root override is active.
pub(super) fn chromium_override_active() -> bool {
    std::env::var_os(CHROME_USER_DATA_OVERRIDE_ENV).is_some()
}

/// Returns whether a Firefox profiles-root override is active.
pub(super) fn firefox_override_active() -> bool {
    std::env::var_os(FIREFOX_PROFILES_OVERRIDE_ENV).is_some()
}

/// Returns whether a Safari root override is active.
pub(super) fn safari_override_active() -> bool {
    std::env::var_os(SAFARI_ROOT_OVERRIDE_ENV).is_some()
}

/// Reports whether any discovery override is active.
pub(super) fn discovery_overrides_active_with(
    chrome_override_active: bool,
    firefox_override_active: bool,
    safari_override_active: bool,
) -> bool {
    chrome_override_active || firefox_override_active || safari_override_active
}

/// Test-only shorthand for the current override state.
#[cfg(test)]
pub(super) fn discovery_overrides_active() -> bool {
    discovery_overrides_active_with(
        chromium_override_active(),
        firefox_override_active(),
        safari_override_active(),
    )
}

/// Limits Chromium discovery to the canonical Chrome override when overrides are active.
pub(super) fn should_discover_chromium_definition(
    overrides_active: bool,
    definition: ChromiumBrowserDefinition,
) -> bool {
    !overrides_active || definition.key == "chrome"
}

/// Decides whether Firefox discovery should run under the current override state.
pub(super) fn should_discover_firefox(
    overrides_active: bool,
    firefox_override_active: bool,
) -> bool {
    !overrides_active || firefox_override_active
}

/// Decides whether Safari discovery should run under the current override state.
pub(super) fn should_discover_safari(
    overrides_active: bool,
    safari_override_active: bool,
) -> bool {
    !overrides_active || safari_override_active
}

/// Returns the canonical Chrome user-data directory for the current host.
pub fn chrome_user_data_dir() -> Result<PathBuf> {
    if let Some(path) = std::env::var_os(CHROME_USER_DATA_OVERRIDE_ENV) {
        return Ok(PathBuf::from(path));
    }

    let home =
        directories::UserDirs::new().context("resolving home directory")?.home_dir().to_path_buf();
    default_chrome_user_data_dir(&home)
}

/// Returns candidate Chromium roots for one browser definition.
pub(super) fn chromium_root_candidates(
    definition: ChromiumBrowserDefinition,
) -> Result<Vec<PathBuf>> {
    if definition.key == "chrome"
        && let Some(path) = std::env::var_os(CHROME_USER_DATA_OVERRIDE_ENV)
    {
        return Ok(vec![PathBuf::from(path)]);
    }

    let home = user_home_dir()?;
    let relative_paths = current_chromium_relative_paths(definition.key);

    #[cfg(target_os = "windows")]
    {
        let mut candidates = Vec::new();
        for base in windows_data_dirs()? {
            for relative in &relative_paths {
                candidates.push(base.join(relative));
            }
        }
        Ok(candidates)
    }

    #[cfg(not(target_os = "windows"))]
    {
        Ok(relative_paths.into_iter().map(|relative| home.join(relative)).collect())
    }
}

/// Returns relative profile-root patterns for one Chromium browser key across supported OSes.
pub(super) fn chromium_relative_paths(
    browser_key: &str,
) -> (Vec<&'static str>, Vec<&'static str>, Vec<&'static str>) {
    match browser_key {
        "chrome" => (
            vec!["Library/Application Support/Google/Chrome"],
            vec![".config/google-chrome", ".var/app/com.google.Chrome/config/google-chrome"],
            vec!["Google/Chrome/User Data"],
        ),
        "chromium" => (
            vec!["Library/Application Support/Chromium"],
            vec![".config/chromium", ".var/app/org.chromium.Chromium/config/chromium"],
            vec!["Chromium/User Data"],
        ),
        "edge" => (
            vec!["Library/Application Support/Microsoft Edge"],
            vec![".config/microsoft-edge", ".var/app/com.microsoft.Edge/config/microsoft-edge"],
            vec!["Microsoft/Edge/User Data"],
        ),
        "edge-dev" => (
            vec!["Library/Application Support/Microsoft Edge Dev"],
            vec![".config/microsoft-edge-dev"],
            vec!["Microsoft/Edge Dev/User Data"],
        ),
        "brave" => (
            vec!["Library/Application Support/BraveSoftware/Brave-Browser"],
            vec![
                ".config/BraveSoftware/Brave-Browser",
                ".var/app/com.brave.Browser/config/BraveSoftware/Brave-Browser",
            ],
            vec!["BraveSoftware/Brave-Browser/User Data"],
        ),
        "vivaldi" => (
            vec!["Library/Application Support/Vivaldi"],
            vec![".config/vivaldi"],
            vec!["Vivaldi/User Data"],
        ),
        "arc" => (
            vec!["Library/Application Support/Arc/User Data"],
            vec![".config/Arc/User Data"],
            vec!["Packages/TheBrowserCompany.Arc_ttt1ap7aakyb4/LocalCache/Local/Arc/User Data"],
        ),
        "opera" => (
            vec!["Library/Application Support/com.operasoftware.Opera"],
            vec![".config/opera"],
            vec!["Opera Software/Opera Stable"],
        ),
        "opera-gx" => (
            vec!["Library/Application Support/com.operasoftware.OperaGX"],
            vec![".config/opera-gx"],
            vec!["Opera Software/Opera GX Stable"],
        ),
        _ => (Vec::new(), Vec::new(), Vec::new()),
    }
}

/// Reads the Windows roaming/local appdata roots when that host family is relevant.
#[cfg(any(target_os = "windows", test))]
pub(super) fn windows_data_dirs() -> Result<Vec<PathBuf>> {
    let mut roots = Vec::new();
    for variable in ["LOCALAPPDATA", "APPDATA"] {
        if let Some(value) = std::env::var_os(variable) {
            let path = PathBuf::from(value);
            if !roots.contains(&path) {
                roots.push(path);
            }
        }
    }

    if roots.is_empty() {
        anyhow::bail!("reading LOCALAPPDATA or APPDATA")
    }

    Ok(roots)
}

/// Returns candidate Firefox roots for one browser definition.
pub(super) fn firefox_root_candidates(
    definition: FirefoxBrowserDefinition,
) -> Result<Vec<PathBuf>> {
    if let Some(path) = std::env::var_os(FIREFOX_PROFILES_OVERRIDE_ENV) {
        return Ok(vec![PathBuf::from(path)]);
    }

    let home = user_home_dir()?;
    let relative_paths = current_firefox_relative_paths(definition.key);

    #[cfg(target_os = "windows")]
    {
        let mut candidates = Vec::new();
        for base in windows_data_dirs()? {
            for relative in &relative_paths {
                candidates.push(base.join(relative));
            }
        }
        Ok(candidates)
    }

    #[cfg(not(target_os = "windows"))]
    {
        Ok(relative_paths.into_iter().map(|relative| home.join(relative)).collect())
    }
}

/// Returns relative profile-root patterns for one Firefox-family browser key across supported OSes.
pub(super) fn firefox_relative_paths(
    browser_key: &str,
) -> (Vec<&'static str>, Vec<&'static str>, Vec<&'static str>) {
    match browser_key {
        "firefox" => (
            vec!["Library/Application Support/Firefox/Profiles"],
            vec![
                ".mozilla/firefox",
                ".var/app/org.mozilla.firefox/.mozilla/firefox",
                "snap/firefox/common/.mozilla/firefox",
            ],
            vec!["Mozilla/Firefox/Profiles"],
        ),
        "librewolf" => (
            vec!["Library/Application Support/LibreWolf/Profiles"],
            vec![".librewolf", ".var/app/io.gitlab.librewolf-community/.librewolf"],
            vec!["librewolf/Profiles"],
        ),
        "floorp" => (
            vec!["Library/Application Support/Floorp/Profiles"],
            vec![".floorp"],
            vec!["Floorp/Profiles"],
        ),
        "waterfox" => {
            (vec!["Library/Application Support/Waterfox/Profiles"], vec![".waterfox"], Vec::new())
        }
        _ => (Vec::new(), Vec::new(), Vec::new()),
    }
}

/// Resolves the current user's home directory.
pub(super) fn user_home_dir() -> Result<PathBuf> {
    Ok(directories::UserDirs::new().context("resolving home directory")?.home_dir().to_path_buf())
}

/// Returns the default Chrome user-data directory for macOS.
#[cfg(target_os = "macos")]
pub(super) fn default_chrome_user_data_dir(home: &Path) -> Result<PathBuf> {
    Ok(home.join("Library/Application Support/Google/Chrome"))
}

/// Returns the default Chrome user-data directory for Windows.
#[cfg(target_os = "windows")]
pub(super) fn default_chrome_user_data_dir(_home: &Path) -> Result<PathBuf> {
    Ok(std::env::var("LOCALAPPDATA")
        .map(PathBuf::from)
        .context("reading LOCALAPPDATA")?
        .join("Google/Chrome/User Data"))
}

/// Returns the default Chrome user-data directory for Linux-like hosts.
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
pub(super) fn default_chrome_user_data_dir(home: &Path) -> Result<PathBuf> {
    Ok(home.join(".config/google-chrome"))
}

/// Returns the host-specific Chromium profile-root patterns.
#[cfg(target_os = "macos")]
pub(super) fn current_chromium_relative_paths(browser_key: &str) -> Vec<&'static str> {
    chromium_relative_paths(browser_key).0
}

/// Returns the host-specific Chromium profile-root patterns.
#[cfg(target_os = "windows")]
pub(super) fn current_chromium_relative_paths(browser_key: &str) -> Vec<&'static str> {
    chromium_relative_paths(browser_key).2
}

/// Returns the host-specific Chromium profile-root patterns.
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
pub(super) fn current_chromium_relative_paths(browser_key: &str) -> Vec<&'static str> {
    chromium_relative_paths(browser_key).1
}

/// Returns the host-specific Firefox profile-root patterns.
#[cfg(target_os = "macos")]
pub(super) fn current_firefox_relative_paths(browser_key: &str) -> Vec<&'static str> {
    firefox_relative_paths(browser_key).0
}

/// Returns the host-specific Firefox profile-root patterns.
#[cfg(target_os = "windows")]
pub(super) fn current_firefox_relative_paths(browser_key: &str) -> Vec<&'static str> {
    firefox_relative_paths(browser_key).2
}

/// Returns the host-specific Firefox profile-root patterns.
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
pub(super) fn current_firefox_relative_paths(browser_key: &str) -> Vec<&'static str> {
    firefox_relative_paths(browser_key).1
}

/// Returns the default Safari root when the host platform supports Safari.
#[cfg(target_os = "macos")]
pub(super) fn default_safari_root() -> Result<Option<PathBuf>> {
    Ok(Some(
        directories::UserDirs::new()
            .context("resolving home directory")?
            .home_dir()
            .join("Library/Safari"),
    ))
}

/// Returns the default Safari root for non-macOS hosts.
#[cfg(not(target_os = "macos"))]
pub(super) fn default_safari_root() -> Result<Option<PathBuf>> {
    Ok(None)
}
