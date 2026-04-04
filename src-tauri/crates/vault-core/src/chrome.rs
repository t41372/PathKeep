use crate::{
    config::ProjectPaths,
    models::BrowserProfile,
    utils::{file_sha256_hex, now_rfc3339},
};
use anyhow::{Context, Result};
use serde_json::Value;
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};
use tempfile::TempDir;

// Detection coverage is informed by the browser location patterns used in 1History
// and browserexport, then adapted to this archive's Rust-native data model.
const CHROME_USER_DATA_OVERRIDE_ENV: &str = "CHB_CHROME_USER_DATA_DIR";
const FIREFOX_PROFILES_OVERRIDE_ENV: &str = "CHB_FIREFOX_PROFILES_DIR";
const SAFARI_ROOT_OVERRIDE_ENV: &str = "CHB_SAFARI_ROOT";

#[derive(Debug)]
pub struct FileFingerprint {
    pub path: String,
    pub sha256: String,
}

#[derive(Debug)]
pub struct ProfileSnapshot {
    pub profile: BrowserProfile,
    pub temp_dir: TempDir,
    pub history_path: PathBuf,
    pub favicons_path: Option<PathBuf>,
    pub source_hashes: Vec<FileFingerprint>,
}

#[derive(Clone, Copy)]
struct ChromiumBrowserDefinition {
    key: &'static str,
    family: &'static str,
    name: &'static str,
}

#[derive(Clone, Copy)]
struct FirefoxBrowserDefinition {
    key: &'static str,
    name: &'static str,
}

const CHROMIUM_BROWSERS: [ChromiumBrowserDefinition; 9] = [
    ChromiumBrowserDefinition { key: "chrome", family: "chromium", name: "Google Chrome" },
    ChromiumBrowserDefinition { key: "chromium", family: "chromium", name: "Chromium" },
    ChromiumBrowserDefinition { key: "edge", family: "chromium", name: "Microsoft Edge" },
    ChromiumBrowserDefinition { key: "edge-dev", family: "chromium", name: "Microsoft Edge Dev" },
    ChromiumBrowserDefinition { key: "brave", family: "chromium", name: "Brave" },
    ChromiumBrowserDefinition { key: "vivaldi", family: "chromium", name: "Vivaldi" },
    ChromiumBrowserDefinition { key: "arc", family: "chromium", name: "Arc" },
    ChromiumBrowserDefinition { key: "opera", family: "chromium", name: "Opera" },
    ChromiumBrowserDefinition { key: "opera-gx", family: "chromium", name: "Opera GX" },
];

const FIREFOX_BROWSERS: [FirefoxBrowserDefinition; 4] = [
    FirefoxBrowserDefinition { key: "firefox", name: "Firefox" },
    FirefoxBrowserDefinition { key: "librewolf", name: "LibreWolf" },
    FirefoxBrowserDefinition { key: "floorp", name: "Floorp" },
    FirefoxBrowserDefinition { key: "waterfox", name: "Waterfox" },
];

pub fn discover_profiles() -> Result<Vec<BrowserProfile>> {
    let mut profiles = Vec::new();
    for definition in CHROMIUM_BROWSERS {
        if discovery_overrides_active() && definition.key != "chrome" {
            continue;
        }
        profiles.extend(discover_chromium_profiles(definition)?);
    }
    if !discovery_overrides_active() || std::env::var_os(FIREFOX_PROFILES_OVERRIDE_ENV).is_some() {
        for definition in FIREFOX_BROWSERS {
            profiles.extend(discover_firefox_profiles(definition)?);
        }
    }
    if (!discovery_overrides_active() || std::env::var_os(SAFARI_ROOT_OVERRIDE_ENV).is_some())
        && let Some(profile) = discover_safari_profile()?
    {
        profiles.push(profile);
    }

    let mut seen = BTreeSet::new();
    profiles.retain(|profile| seen.insert(profile.profile_id.clone()));
    profiles.sort_by(|left, right| {
        left.browser_name
            .cmp(&right.browser_name)
            .then(left.profile_name.cmp(&right.profile_name))
            .then(left.profile_id.cmp(&right.profile_id))
    });
    Ok(profiles)
}

fn discovery_overrides_active() -> bool {
    std::env::var_os(CHROME_USER_DATA_OVERRIDE_ENV).is_some()
        || std::env::var_os(FIREFOX_PROFILES_OVERRIDE_ENV).is_some()
        || std::env::var_os(SAFARI_ROOT_OVERRIDE_ENV).is_some()
}

pub fn stage_profile_snapshot(
    paths: &ProjectPaths,
    profile: &BrowserProfile,
) -> Result<ProfileSnapshot> {
    let temp_dir = tempfile::Builder::new()
        .prefix(&format!("{}-{}", profile.profile_id, now_rfc3339().replace(':', "-")))
        .tempdir_in(&paths.staging_dir)
        .with_context(|| format!("creating temp dir in {}", paths.staging_dir.display()))?;
    let source_dir = PathBuf::from(&profile.profile_path);
    let history_path =
        copy_database_with_sidecars(&source_dir, &profile.history_file_name, temp_dir.path())?;
    let favicons_path = profile
        .favicons_path
        .as_ref()
        .and_then(|_| copy_database_with_sidecars(&source_dir, "Favicons", temp_dir.path()).ok());

    let mut source_hashes = Vec::new();
    for path in [Some(history_path.clone()), favicons_path.clone()].into_iter().flatten() {
        source_hashes.push(FileFingerprint {
            sha256: file_sha256_hex(&path)?,
            path: path.display().to_string(),
        });
    }

    Ok(ProfileSnapshot {
        profile: profile.clone(),
        temp_dir,
        history_path,
        favicons_path,
        source_hashes,
    })
}

pub fn chrome_user_data_dir() -> Result<PathBuf> {
    let home =
        directories::UserDirs::new().context("resolving home directory")?.home_dir().to_path_buf();

    if let Some(path) = std::env::var_os(CHROME_USER_DATA_OVERRIDE_ENV) {
        return Ok(PathBuf::from(path));
    }

    let path = if cfg!(target_os = "macos") {
        home.join("Library/Application Support/Google/Chrome")
    } else if cfg!(target_os = "windows") {
        std::env::var("LOCALAPPDATA")
            .map(PathBuf::from)
            .context("reading LOCALAPPDATA")?
            .join("Google/Chrome/User Data")
    } else {
        home.join(".config/google-chrome")
    };

    Ok(path)
}

fn discover_chromium_profiles(
    definition: ChromiumBrowserDefinition,
) -> Result<Vec<BrowserProfile>> {
    let mut profiles = Vec::new();
    for root in chromium_root_candidates(definition)? {
        if !root.exists() {
            continue;
        }
        let chrome_version = fs::read_to_string(root.join("Last Version"))
            .ok()
            .map(|content| content.trim().to_string());
        let info_cache = read_chromium_info_cache(&root).unwrap_or_default();

        if info_cache.is_empty() {
            profiles.extend(
                fallback_chromium_profiles(definition, &root, chrome_version.as_deref())?
                    .into_iter(),
            );
            continue;
        }

        for (raw_profile_id, details) in info_cache {
            let profile_path = root.join(&raw_profile_id);
            let history_path = profile_path.join("History");
            let favicons_path = profile_path.join("Favicons");
            profiles.push(BrowserProfile {
                profile_id: format!("{}:{}", definition.key, raw_profile_id),
                profile_name: details
                    .get("name")
                    .and_then(Value::as_str)
                    .unwrap_or(&raw_profile_id)
                    .to_string(),
                browser_family: definition.family.to_string(),
                browser_name: definition.name.to_string(),
                user_name: details
                    .get("user_name")
                    .and_then(Value::as_str)
                    .map(ToString::to_string),
                profile_path: profile_path.display().to_string(),
                history_path: history_path.exists().then(|| history_path.display().to_string()),
                favicons_path: favicons_path.exists().then(|| favicons_path.display().to_string()),
                history_exists: history_path.exists(),
                browser_version: chrome_version.clone(),
                history_file_name: "History".to_string(),
            });
        }
    }
    Ok(profiles)
}

fn chromium_root_candidates(definition: ChromiumBrowserDefinition) -> Result<Vec<PathBuf>> {
    if definition.key == "chrome" {
        if let Some(path) = std::env::var_os(CHROME_USER_DATA_OVERRIDE_ENV) {
            return Ok(vec![PathBuf::from(path)]);
        }
    }

    let home =
        directories::UserDirs::new().context("resolving home directory")?.home_dir().to_path_buf();
    let relative_paths = if cfg!(target_os = "macos") {
        chromium_relative_paths(definition.key).0
    } else if cfg!(target_os = "windows") {
        chromium_relative_paths(definition.key).2
    } else {
        chromium_relative_paths(definition.key).1
    };

    if cfg!(target_os = "windows") {
        let mut candidates = Vec::new();
        for base in windows_data_dirs()? {
            for relative in &relative_paths {
                candidates.push(base.join(relative));
            }
        }
        return Ok(candidates);
    }

    Ok(relative_paths.into_iter().map(|relative| home.join(relative)).collect())
}

fn chromium_relative_paths(
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

fn windows_data_dirs() -> Result<Vec<PathBuf>> {
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

fn read_chromium_info_cache(root: &Path) -> Result<BTreeMap<String, Value>> {
    let local_state_path = root.join("Local State");
    if !local_state_path.exists() {
        return Ok(BTreeMap::new());
    }

    let local_state = fs::read_to_string(&local_state_path)
        .with_context(|| format!("reading {}", local_state_path.display()))?;
    let json: Value = serde_json::from_str(&local_state)?;
    Ok(json
        .get("profile")
        .and_then(|profile| profile.get("info_cache"))
        .and_then(Value::as_object)
        .map(|cache| {
            cache
                .iter()
                .map(|(key, value)| (key.clone(), value.clone()))
                .collect::<BTreeMap<_, _>>()
        })
        .unwrap_or_default())
}

fn fallback_chromium_profiles(
    definition: ChromiumBrowserDefinition,
    root: &Path,
    browser_version: Option<&str>,
) -> Result<Vec<BrowserProfile>> {
    let mut profiles = Vec::new();
    if let Some(profile) = direct_root_chromium_profile(definition, root, browser_version) {
        profiles.push(profile);
    }
    for entry in fs::read_dir(root).with_context(|| format!("reading {}", root.display()))? {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let profile_path = entry.path();
        let history_path = profile_path.join("History");
        if !history_path.exists() {
            continue;
        }

        let raw_profile_id = entry.file_name().to_string_lossy().to_string();
        let favicons_path = profile_path.join("Favicons");
        profiles.push(BrowserProfile {
            profile_id: format!("{}:{}", definition.key, raw_profile_id),
            profile_name: raw_profile_id.clone(),
            browser_family: definition.family.to_string(),
            browser_name: definition.name.to_string(),
            user_name: None,
            profile_path: profile_path.display().to_string(),
            history_path: Some(history_path.display().to_string()),
            favicons_path: favicons_path.exists().then(|| favicons_path.display().to_string()),
            history_exists: true,
            browser_version: browser_version.map(ToString::to_string),
            history_file_name: "History".to_string(),
        });
    }
    Ok(profiles)
}

fn direct_root_chromium_profile(
    definition: ChromiumBrowserDefinition,
    root: &Path,
    browser_version: Option<&str>,
) -> Option<BrowserProfile> {
    let history_path = root.join("History");
    if !history_path.exists() {
        return None;
    }

    let profile_name = if definition.key.starts_with("opera") { "Default" } else { "Root" };
    let profile_suffix = if definition.key.starts_with("opera") { "default" } else { "root" };
    let favicons_path = root.join("Favicons");

    Some(BrowserProfile {
        profile_id: format!("{}:{profile_suffix}", definition.key),
        profile_name: profile_name.to_string(),
        browser_family: definition.family.to_string(),
        browser_name: definition.name.to_string(),
        user_name: None,
        profile_path: root.display().to_string(),
        history_path: Some(history_path.display().to_string()),
        favicons_path: favicons_path.exists().then(|| favicons_path.display().to_string()),
        history_exists: true,
        browser_version: browser_version.map(ToString::to_string),
        history_file_name: "History".to_string(),
    })
}

fn discover_firefox_profiles(definition: FirefoxBrowserDefinition) -> Result<Vec<BrowserProfile>> {
    let roots = firefox_root_candidates(definition)?;
    let mut profiles = Vec::new();
    for root in roots {
        if !root.exists() {
            continue;
        }

        let display_names = parse_firefox_profile_names(&root);
        for entry in fs::read_dir(&root).with_context(|| format!("reading {}", root.display()))? {
            let entry = entry?;
            if !entry.file_type()?.is_dir() {
                continue;
            }

            let profile_path = entry.path();
            let history_path = profile_path.join("places.sqlite");
            if !history_path.exists() {
                continue;
            }

            let raw_profile_id = entry.file_name().to_string_lossy().to_string();
            profiles.push(BrowserProfile {
                profile_id: format!("{}:{raw_profile_id}", definition.key),
                profile_name: display_names
                    .get(&raw_profile_id)
                    .cloned()
                    .unwrap_or_else(|| raw_profile_id.clone()),
                browser_family: "firefox".to_string(),
                browser_name: definition.name.to_string(),
                user_name: None,
                profile_path: profile_path.display().to_string(),
                history_path: Some(history_path.display().to_string()),
                favicons_path: None,
                history_exists: true,
                browser_version: None,
                history_file_name: "places.sqlite".to_string(),
            });
        }
    }
    Ok(profiles)
}

fn firefox_root_candidates(definition: FirefoxBrowserDefinition) -> Result<Vec<PathBuf>> {
    if let Some(path) = std::env::var_os(FIREFOX_PROFILES_OVERRIDE_ENV) {
        return Ok(vec![PathBuf::from(path)]);
    }

    let home =
        directories::UserDirs::new().context("resolving home directory")?.home_dir().to_path_buf();
    let relative_paths = if cfg!(target_os = "macos") {
        firefox_relative_paths(definition.key).0
    } else if cfg!(target_os = "windows") {
        firefox_relative_paths(definition.key).2
    } else {
        firefox_relative_paths(definition.key).1
    };

    if cfg!(target_os = "windows") {
        let mut candidates = Vec::new();
        for base in windows_data_dirs()? {
            for relative in &relative_paths {
                candidates.push(base.join(relative));
            }
        }
        return Ok(candidates);
    }

    Ok(relative_paths.into_iter().map(|relative| home.join(relative)).collect())
}

fn firefox_relative_paths(
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

fn parse_firefox_profile_names(root: &Path) -> BTreeMap<String, String> {
    let profiles_ini = root.parent().unwrap_or(root).join("profiles.ini");
    let content = match fs::read_to_string(&profiles_ini) {
        Ok(content) => content,
        Err(_) => return BTreeMap::new(),
    };

    let mut names = BTreeMap::new();
    let mut current_name: Option<String> = None;
    let mut current_path: Option<String> = None;

    let flush = |names: &mut BTreeMap<String, String>,
                 current_name: &mut Option<String>,
                 current_path: &mut Option<String>| {
        if let (Some(name), Some(path)) = (current_name.take(), current_path.take()) {
            let key = PathBuf::from(path)
                .file_name()
                .map(|value| value.to_string_lossy().to_string())
                .unwrap_or_else(|| name.clone());
            names.insert(key, name);
        }
    };

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            flush(&mut names, &mut current_name, &mut current_path);
            continue;
        }
        if let Some(name) = trimmed.strip_prefix("Name=") {
            current_name = Some(name.to_string());
            continue;
        }
        if let Some(path) = trimmed.strip_prefix("Path=") {
            current_path = Some(path.to_string());
        }
    }
    flush(&mut names, &mut current_name, &mut current_path);
    names
}

fn discover_safari_profile() -> Result<Option<BrowserProfile>> {
    if !cfg!(target_os = "macos") && std::env::var_os(SAFARI_ROOT_OVERRIDE_ENV).is_none() {
        return Ok(None);
    }

    let safari_root = if let Some(path) = std::env::var_os(SAFARI_ROOT_OVERRIDE_ENV) {
        PathBuf::from(path)
    } else {
        directories::UserDirs::new()
            .context("resolving home directory")?
            .home_dir()
            .join("Library/Safari")
    };

    let history_path = safari_root.join("History.db");
    if !history_path.exists() {
        return Ok(None);
    }

    Ok(Some(BrowserProfile {
        profile_id: "safari:default".to_string(),
        profile_name: "Default".to_string(),
        browser_family: "safari".to_string(),
        browser_name: "Safari".to_string(),
        user_name: None,
        profile_path: safari_root.display().to_string(),
        history_path: Some(history_path.display().to_string()),
        favicons_path: None,
        history_exists: true,
        browser_version: None,
        history_file_name: "History.db".to_string(),
    }))
}

fn copy_database_with_sidecars(
    source_dir: &Path,
    base_name: &str,
    destination_dir: &Path,
) -> Result<PathBuf> {
    let destination = destination_dir.join(base_name);
    fs::copy(source_dir.join(base_name), &destination).with_context(|| {
        format!("copying {} to {}", source_dir.join(base_name).display(), destination.display())
    })?;

    for suffix in ["-wal", "-shm", "-journal"] {
        let source_sidecar = source_dir.join(format!("{base_name}{suffix}"));
        if source_sidecar.exists() {
            let target_sidecar = destination_dir.join(format!("{base_name}{suffix}"));
            fs::copy(&source_sidecar, &target_sidecar).with_context(|| {
                format!("copying {} to {}", source_sidecar.display(), target_sidecar.display())
            })?;
        }
    }

    Ok(destination)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ProjectPaths;
    use std::io::Write;
    use tempfile::tempdir;

    fn sample_paths(root: &Path) -> ProjectPaths {
        ProjectPaths {
            app_root: root.to_path_buf(),
            config_path: root.join("config.json"),
            archive_database_path: root.join("archive/history-vault.sqlite"),
            audit_repo_path: root.join("audit"),
            manifests_dir: root.join("audit/manifests"),
            exports_dir: root.join("exports"),
            raw_snapshots_dir: root.join("raw-snapshots"),
            staging_dir: root.join("staging"),
            quarantine_dir: root.join("quarantine"),
            schedule_dir: root.join("schedule"),
            stronghold_path: root.join("vault.hold"),
            stronghold_salt_path: root.join("stronghold-salt.txt"),
        }
    }

    #[test]
    fn discover_profiles_reads_local_state_from_override() {
        let dir = tempdir().expect("tempdir");
        let default_dir = dir.path().join("Default");
        fs::create_dir_all(&default_dir).expect("create profile");
        fs::write(default_dir.join("History"), b"sqlite").expect("write history");
        fs::write(dir.path().join("Last Version"), "135.0.0.0").expect("write last version");
        fs::write(
            dir.path().join("Local State"),
            r#"{"profile":{"info_cache":{"Default":{"name":"Personal","user_name":"tim@example.com"}}}}"#,
        )
        .expect("write local state");

        unsafe {
            std::env::set_var(CHROME_USER_DATA_OVERRIDE_ENV, dir.path());
        }
        let profiles = discover_profiles().expect("discover");
        unsafe {
            std::env::remove_var(CHROME_USER_DATA_OVERRIDE_ENV);
        }

        assert!(profiles.iter().any(|profile| {
            profile.profile_name == "Personal"
                && profile.browser_name == "Google Chrome"
                && profile.browser_version.as_deref() == Some("135.0.0.0")
        }));
    }

    #[test]
    fn discover_profiles_supports_firefox_and_safari_overrides() {
        let dir = tempdir().expect("tempdir");
        let firefox_profiles = dir.path().join("firefox");
        let firefox_profile = firefox_profiles.join("abcd.default-release");
        fs::create_dir_all(&firefox_profile).expect("create firefox profile");
        fs::write(firefox_profile.join("places.sqlite"), b"sqlite").expect("write firefox db");
        fs::write(
            dir.path().join("profiles.ini"),
            "[Profile0]\nName=Work Firefox\nPath=abcd.default-release\nIsRelative=1\n",
        )
        .expect("write profiles.ini");

        let safari_root = dir.path().join("Safari");
        fs::create_dir_all(&safari_root).expect("create safari root");
        fs::write(safari_root.join("History.db"), b"safari").expect("write safari db");

        unsafe {
            std::env::set_var(FIREFOX_PROFILES_OVERRIDE_ENV, &firefox_profiles);
            std::env::set_var(SAFARI_ROOT_OVERRIDE_ENV, &safari_root);
        }
        let profiles = discover_profiles().expect("discover all");
        unsafe {
            std::env::remove_var(FIREFOX_PROFILES_OVERRIDE_ENV);
            std::env::remove_var(SAFARI_ROOT_OVERRIDE_ENV);
        }

        assert!(profiles.iter().any(|profile| {
            profile.browser_family == "firefox" && profile.profile_name == "Work Firefox"
        }));
        assert!(
            profiles
                .iter()
                .any(|profile| profile.browser_family == "safari" && profile.history_exists)
        );
    }

    #[test]
    fn firefox_profile_name_parser_handles_relative_and_absolute_paths() {
        let dir = tempdir().expect("tempdir");
        let profiles_root = dir.path().join("Profiles");
        fs::create_dir_all(&profiles_root).expect("create profiles root");
        fs::write(
            dir.path().join("profiles.ini"),
            "[Profile0]\nName=Personal\nPath=abcd.default-release\n\n[Profile1]\nName=Absolute\nPath=/tmp/absolute.profile\n",
        )
        .expect("write profiles.ini");

        let names = parse_firefox_profile_names(&profiles_root);
        assert_eq!(names.get("abcd.default-release"), Some(&"Personal".to_string()));
        assert_eq!(names.get("absolute.profile"), Some(&"Absolute".to_string()));
    }

    #[test]
    fn discover_safari_profile_returns_none_when_history_is_missing() {
        let dir = tempdir().expect("tempdir");
        unsafe {
            std::env::set_var(SAFARI_ROOT_OVERRIDE_ENV, dir.path());
        }
        let profile = discover_safari_profile().expect("discover safari");
        unsafe {
            std::env::remove_var(SAFARI_ROOT_OVERRIDE_ENV);
        }
        assert!(profile.is_none());
    }

    #[test]
    fn stage_profile_snapshot_copies_database_and_sidecars() {
        let dir = tempdir().expect("tempdir");
        let paths = sample_paths(dir.path());
        fs::create_dir_all(&paths.staging_dir).expect("create staging");

        let profile_dir = dir.path().join("Profile 1");
        fs::create_dir_all(&profile_dir).expect("create profile dir");
        fs::write(profile_dir.join("places.sqlite"), b"history-db").expect("write history");
        fs::write(profile_dir.join("places.sqlite-journal"), b"journal").expect("write journal");

        let profile = BrowserProfile {
            profile_id: "firefox:Profile 1".to_string(),
            profile_name: "Work".to_string(),
            browser_family: "firefox".to_string(),
            browser_name: "Firefox".to_string(),
            user_name: None,
            profile_path: profile_dir.display().to_string(),
            history_path: Some(profile_dir.join("places.sqlite").display().to_string()),
            favicons_path: None,
            history_exists: true,
            browser_version: None,
            history_file_name: "places.sqlite".to_string(),
        };

        let snapshot = stage_profile_snapshot(&paths, &profile).expect("snapshot");
        assert!(snapshot.history_path.exists());
        assert!(snapshot.temp_dir.path().join("places.sqlite-journal").exists());
        assert!(snapshot.favicons_path.is_none());
        assert_eq!(snapshot.source_hashes.len(), 1);
    }

    #[test]
    fn copy_database_with_sidecars_copies_known_sidecars_only() {
        let source = tempdir().expect("source");
        let destination = tempdir().expect("dest");
        fs::write(source.path().join("History"), b"history").expect("history");
        fs::write(source.path().join("History-wal"), b"wal").expect("wal");
        fs::write(source.path().join("History-shm"), b"shm").expect("shm");
        let mut ignored = fs::File::create(source.path().join("History-random")).expect("ignored");
        ignored.write_all(b"ignored").expect("write ignored");

        let copied = copy_database_with_sidecars(source.path(), "History", destination.path())
            .expect("copy");

        assert_eq!(copied, destination.path().join("History"));
        assert!(destination.path().join("History").exists());
        assert!(destination.path().join("History-wal").exists());
        assert!(destination.path().join("History-shm").exists());
        assert!(!destination.path().join("History-random").exists());
    }

    #[test]
    fn fallback_chromium_profiles_support_direct_history_layouts() {
        let dir = tempdir().expect("tempdir");
        fs::write(dir.path().join("History"), b"history").expect("write history");
        fs::write(dir.path().join("Favicons"), b"favicons").expect("write favicons");

        let definition = CHROMIUM_BROWSERS
            .iter()
            .copied()
            .find(|definition| definition.key == "opera")
            .expect("opera definition");
        let profiles =
            fallback_chromium_profiles(definition, dir.path(), Some("118.0")).expect("profiles");

        assert_eq!(profiles.len(), 1);
        assert_eq!(profiles[0].profile_id, "opera:default");
        assert_eq!(profiles[0].browser_name, "Opera");
        assert!(profiles[0].history_exists);
        assert_eq!(profiles[0].browser_version.as_deref(), Some("118.0"));
    }

    #[test]
    fn fallback_chromium_profiles_collects_directory_profiles_with_favicons() {
        let dir = tempdir().expect("tempdir");
        let profile_dir = dir.path().join("Profile 1");
        fs::create_dir_all(&profile_dir).expect("create profile dir");
        fs::write(profile_dir.join("History"), b"history").expect("write history");
        fs::write(profile_dir.join("Favicons"), b"favicons").expect("write favicons");

        let definition = CHROMIUM_BROWSERS
            .iter()
            .copied()
            .find(|definition| definition.key == "chrome")
            .expect("chrome definition");
        let profiles =
            fallback_chromium_profiles(definition, dir.path(), Some("146.0")).expect("profiles");

        assert_eq!(profiles.len(), 1);
        assert_eq!(profiles[0].profile_id, "chrome:Profile 1");
        assert!(profiles[0].favicons_path.is_some());
        assert_eq!(profiles[0].browser_version.as_deref(), Some("146.0"));
    }
}
