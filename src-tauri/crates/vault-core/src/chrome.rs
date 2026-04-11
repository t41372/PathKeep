//! Browser discovery and staging boundary for locally installed profiles.
//!
//! `vault-core::chrome` is legacy-named, but its responsibility is broader than
//! Chromium alone: it turns host browser locations into `BrowserProfile` read
//! models and creates staging copies that the parser/archive pipeline can read
//! safely.
//!
//! The important boundary from the accepted docs is that parser code only sees
//! provided file paths. Discovery and staging live here, while
//! `browser-history-parser` stays ignorant of installed-browser heuristics and
//! live-file copying.

mod discovery;
mod paths;
mod staging;

use crate::{
    browser_retention::retention_boundary_for_browser,
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

pub use self::{
    discovery::discover_profiles, paths::chrome_user_data_dir, staging::stage_profile_snapshot,
};

#[cfg(test)]
use self::{
    discovery::{
        direct_root_chromium_profile, discover_chromium_profiles, discover_firefox_profiles,
        discover_safari_profile, fallback_chromium_profiles, parse_firefox_profile_names,
    },
    paths::{
        chromium_relative_paths, current_chromium_relative_paths, current_firefox_relative_paths,
        default_chrome_user_data_dir, default_safari_root, discovery_overrides_active,
        discovery_overrides_active_with, firefox_relative_paths, firefox_root_candidates,
        should_discover_chromium_definition, should_discover_firefox, should_discover_safari,
        windows_data_dirs,
    },
    staging::copy_database_with_sidecars,
};

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        config::{ProjectPaths, project_paths_with_root},
        utils::{restore_test_env_var, test_env_lock},
    };
    use std::{io::Write, sync::MutexGuard};
    use tempfile::tempdir;

    fn lock_env() -> MutexGuard<'static, ()> {
        test_env_lock().lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn sample_paths(root: &Path) -> ProjectPaths {
        project_paths_with_root(root)
    }

    #[test]
    fn discover_profiles_reads_local_state_from_override() {
        let _guard = lock_env();
        let dir = tempdir().expect("tempdir");
        let default_dir = dir.path().join("Default");
        fs::create_dir_all(&default_dir).expect("create profile");
        fs::write(default_dir.join("History"), b"sqlite").expect("write history");
        fs::write(dir.path().join("Last Version"), "135.0.0.0").expect("write last version");
        fs::write(
            dir.path().join("Local State"),
            r#"{"profile":{"info_cache":{"Default":{"name":"Primary","user_name":"primary@example.test"}}}}"#,
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
            profile.profile_name == "Primary"
                && profile.browser_name == "Google Chrome"
                && profile.browser_version.as_deref() == Some("135.0.0.0")
        }));
    }

    #[test]
    fn discover_profiles_supports_firefox_and_safari_overrides() {
        let _guard = lock_env();
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
    fn firefox_profile_name_parser_accepts_values_that_end_with_a_bracket() {
        let dir = tempdir().expect("tempdir");
        let profiles_root = dir.path().join("Profiles");
        fs::create_dir_all(&profiles_root).expect("create profiles root");
        fs::write(
            dir.path().join("profiles.ini"),
            "[Profile0]\nName=Trailing Bracket\nPath=abcd.default-release]\n",
        )
        .expect("write profiles.ini");

        let names = parse_firefox_profile_names(&profiles_root);
        assert_eq!(names.len(), 1);
        assert_eq!(names.get("abcd.default-release]"), Some(&"Trailing Bracket".to_string()));
    }

    #[test]
    fn discover_safari_profile_marks_missing_history_without_hiding_the_profile() {
        let _guard = lock_env();
        let dir = tempdir().expect("tempdir");
        unsafe {
            std::env::set_var(SAFARI_ROOT_OVERRIDE_ENV, dir.path());
        }
        let profile = discover_safari_profile().expect("discover safari");
        unsafe {
            std::env::remove_var(SAFARI_ROOT_OVERRIDE_ENV);
        }
        let profile = profile.expect("safari profile");
        assert_eq!(profile.profile_id, "safari:default");
        assert!(!profile.history_exists);
        assert!(profile.history_path.is_none());
    }

    #[test]
    fn firefox_root_candidate_helpers_cover_override_default_and_missing_roots() {
        let _guard = lock_env();
        let dir = tempdir().expect("tempdir");
        let original_firefox = std::env::var_os(FIREFOX_PROFILES_OVERRIDE_ENV);
        let original_home = std::env::var_os("HOME");
        let original_safari = std::env::var_os(SAFARI_ROOT_OVERRIDE_ENV);

        let override_root = dir.path().join("override-firefox");
        unsafe {
            std::env::set_var(FIREFOX_PROFILES_OVERRIDE_ENV, &override_root);
        }
        let firefox_definition = FIREFOX_BROWSERS[0];
        let override_candidates =
            firefox_root_candidates(firefox_definition).expect("override candidates");
        assert_eq!(override_candidates, vec![override_root.clone()]);

        unsafe {
            std::env::remove_var(FIREFOX_PROFILES_OVERRIDE_ENV);
            std::env::set_var("HOME", dir.path());
            std::env::remove_var(SAFARI_ROOT_OVERRIDE_ENV);
        }
        let default_candidates =
            firefox_root_candidates(firefox_definition).expect("default candidates");
        assert!(!default_candidates.is_empty());
        assert!(default_candidates.iter().all(|path| path.starts_with(dir.path())));
        assert!(
            discover_firefox_profiles(firefox_definition).expect("discover firefox").is_empty()
        );
        let safari_profile = discover_safari_profile()
            .expect("discover safari without root")
            .expect("safari profile placeholder");
        assert!(!safari_profile.history_exists);

        restore_test_env_var("HOME", original_home.as_deref());
        restore_test_env_var(FIREFOX_PROFILES_OVERRIDE_ENV, original_firefox.as_deref());
        restore_test_env_var(SAFARI_ROOT_OVERRIDE_ENV, original_safari.as_deref());
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
            history_bytes: 10,
            favicons_bytes: 0,
            supporting_bytes: 7,
            retention_boundary: retention_boundary_for_browser("firefox"),
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

    #[test]
    fn browser_location_helpers_cover_supported_browser_matrices_and_overrides() {
        let _guard = lock_env();
        let dir = tempdir().expect("tempdir");
        let original_localappdata = std::env::var_os("LOCALAPPDATA");
        let original_appdata = std::env::var_os("APPDATA");

        for definition in CHROMIUM_BROWSERS {
            let (mac, linux, windows) = chromium_relative_paths(definition.key);
            assert!(!mac.is_empty());
            assert!(!linux.is_empty());
            if definition.key != "arc" {
                assert!(!windows.is_empty());
            }
        }
        for definition in FIREFOX_BROWSERS {
            let (mac, linux, windows) = firefox_relative_paths(definition.key);
            assert!(!mac.is_empty());
            assert!(!linux.is_empty());
            if definition.key != "waterfox" {
                assert!(!windows.is_empty());
            }
        }

        unsafe {
            std::env::set_var(CHROME_USER_DATA_OVERRIDE_ENV, dir.path().join("chrome"));
            std::env::set_var(FIREFOX_PROFILES_OVERRIDE_ENV, dir.path().join("firefox"));
            std::env::set_var(SAFARI_ROOT_OVERRIDE_ENV, dir.path().join("safari"));
            std::env::set_var("LOCALAPPDATA", dir.path().join("local"));
            std::env::set_var("APPDATA", dir.path().join("roaming"));
        }

        assert!(discovery_overrides_active());
        assert_eq!(chrome_user_data_dir().expect("chrome override"), dir.path().join("chrome"));
        let windows_dirs = windows_data_dirs().expect("windows dirs");
        assert_eq!(windows_dirs.len(), 2);

        unsafe {
            std::env::remove_var(CHROME_USER_DATA_OVERRIDE_ENV);
            std::env::remove_var(FIREFOX_PROFILES_OVERRIDE_ENV);
            std::env::remove_var(SAFARI_ROOT_OVERRIDE_ENV);
        }
        restore_test_env_var("LOCALAPPDATA", original_localappdata.as_deref());
        restore_test_env_var("APPDATA", original_appdata.as_deref());
        assert!(!discovery_overrides_active());
    }

    #[test]
    fn discovery_selection_helpers_cover_single_override_states() {
        let chrome_definition = CHROMIUM_BROWSERS
            .iter()
            .copied()
            .find(|definition| definition.key == "chrome")
            .expect("chrome definition");
        let edge_definition = CHROMIUM_BROWSERS
            .iter()
            .copied()
            .find(|definition| definition.key == "edge")
            .expect("edge definition");

        assert!(!discovery_overrides_active_with(false, false, false));
        assert!(discovery_overrides_active_with(true, false, false));
        assert!(discovery_overrides_active_with(false, true, false));
        assert!(discovery_overrides_active_with(false, false, true));

        assert!(should_discover_chromium_definition(false, chrome_definition));
        assert!(should_discover_chromium_definition(false, edge_definition));
        assert!(should_discover_chromium_definition(true, chrome_definition));
        assert!(!should_discover_chromium_definition(true, edge_definition));

        assert!(should_discover_firefox(false, false));
        assert!(!should_discover_firefox(true, false));
        assert!(should_discover_firefox(true, true));

        assert!(should_discover_safari(false, false));
        assert!(!should_discover_safari(true, false));
        assert!(should_discover_safari(true, true));
    }

    #[test]
    fn host_platform_path_helpers_return_expected_values() {
        let home = Path::new("/tmp/chb-home");

        #[cfg(target_os = "macos")]
        {
            assert_eq!(
                default_chrome_user_data_dir(home).expect("default chrome dir"),
                home.join("Library/Application Support/Google/Chrome")
            );
            assert_eq!(
                current_chromium_relative_paths("chrome"),
                vec!["Library/Application Support/Google/Chrome"]
            );
            assert_eq!(
                current_firefox_relative_paths("firefox"),
                vec!["Library/Application Support/Firefox/Profiles"]
            );
        }

        #[cfg(target_os = "windows")]
        {
            let local = Path::new(r"C:\Users\tester\AppData\Local");
            restore_test_env_var("LOCALAPPDATA", Some(local.as_os_str()));
            assert_eq!(
                default_chrome_user_data_dir(home).expect("default chrome dir"),
                local.join("Google/Chrome/User Data")
            );
            assert_eq!(current_chromium_relative_paths("chrome"), vec!["Google/Chrome/User Data"]);
            assert_eq!(current_firefox_relative_paths("firefox"), vec!["Mozilla/Firefox/Profiles"]);
            restore_test_env_var("LOCALAPPDATA", None);
        }

        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        {
            assert_eq!(
                default_chrome_user_data_dir(home).expect("default chrome dir"),
                home.join(".config/google-chrome")
            );
            assert_eq!(
                current_chromium_relative_paths("chrome"),
                vec![".config/google-chrome", ".var/app/com.google.Chrome/config/google-chrome"]
            );
            assert_eq!(
                current_firefox_relative_paths("firefox"),
                vec![
                    ".mozilla/firefox",
                    ".var/app/org.mozilla.firefox/.mozilla/firefox",
                    "snap/firefox/common/.mozilla/firefox",
                ]
            );
        }
    }

    #[test]
    fn browser_profile_helpers_cover_direct_roots_and_default_locations() {
        let _guard = lock_env();
        let dir = tempdir().expect("tempdir");
        let original_chrome = std::env::var_os(CHROME_USER_DATA_OVERRIDE_ENV);
        let original_safari = std::env::var_os(SAFARI_ROOT_OVERRIDE_ENV);
        restore_test_env_var(CHROME_USER_DATA_OVERRIDE_ENV, None);
        restore_test_env_var(SAFARI_ROOT_OVERRIDE_ENV, None);
        let chrome_default = chrome_user_data_dir().expect("default chrome dir");
        #[cfg(target_os = "macos")]
        assert!(chrome_default.ends_with("Library/Application Support/Google/Chrome"));
        #[cfg(target_os = "windows")]
        assert!(chrome_default.ends_with("Google/Chrome/User Data"));
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        assert!(chrome_default.ends_with(".config/google-chrome"));

        let root_profile = direct_root_chromium_profile(
            CHROMIUM_BROWSERS
                .iter()
                .copied()
                .find(|definition| definition.key == "chrome")
                .expect("chrome definition"),
            dir.path(),
            Some("123.0"),
        );
        assert!(root_profile.is_none());

        fs::write(dir.path().join("History"), b"history").expect("write history");
        let root_profile = direct_root_chromium_profile(
            CHROMIUM_BROWSERS
                .iter()
                .copied()
                .find(|definition| definition.key == "chrome")
                .expect("chrome definition"),
            dir.path(),
            Some("123.0"),
        )
        .expect("root profile");
        assert_eq!(root_profile.profile_id, "chrome:root");
        assert_eq!(root_profile.profile_name, "Root");

        let safari_root = dir.path().join("Safari");
        fs::create_dir_all(&safari_root).expect("create safari");
        fs::write(safari_root.join("History.db"), b"safari").expect("write safari");
        unsafe {
            std::env::set_var(SAFARI_ROOT_OVERRIDE_ENV, &safari_root);
        }
        let safari = discover_safari_profile().expect("discover safari").expect("safari profile");
        restore_test_env_var(CHROME_USER_DATA_OVERRIDE_ENV, original_chrome.as_deref());
        restore_test_env_var(SAFARI_ROOT_OVERRIDE_ENV, original_safari.as_deref());
        assert_eq!(safari.browser_name, "Safari");
        assert_eq!(safari.history_file_name, "History.db");
    }

    #[test]
    fn discovery_helpers_cover_unknown_keys_empty_roots_and_default_paths() {
        let _guard = lock_env();
        let dir = tempdir().expect("tempdir");
        let original_chrome = std::env::var_os(CHROME_USER_DATA_OVERRIDE_ENV);
        let original_firefox = std::env::var_os(FIREFOX_PROFILES_OVERRIDE_ENV);
        let original_localappdata = std::env::var_os("LOCALAPPDATA");
        let original_appdata = std::env::var_os("APPDATA");

        assert_eq!(chromium_relative_paths("unknown"), (Vec::new(), Vec::new(), Vec::new()));
        assert_eq!(firefox_relative_paths("unknown"), (Vec::new(), Vec::new(), Vec::new()));
        assert!(current_chromium_relative_paths("unknown").is_empty());
        assert!(current_firefox_relative_paths("unknown").is_empty());
        assert!(parse_firefox_profile_names(dir.path()).is_empty());
        let safari_root = default_safari_root().expect("default safari root");
        #[cfg(target_os = "macos")]
        assert!(safari_root.expect("macOS safari root").ends_with("Library/Safari"));
        #[cfg(not(target_os = "macos"))]
        assert!(safari_root.is_none());

        let chrome_definition = CHROMIUM_BROWSERS
            .iter()
            .copied()
            .find(|definition| definition.key == "chrome")
            .expect("chrome definition");
        unsafe {
            std::env::set_var(CHROME_USER_DATA_OVERRIDE_ENV, dir.path().join("missing-chrome"));
        }
        assert!(discover_chromium_profiles(chrome_definition).expect("missing chrome").is_empty());

        let fallback_root = dir.path().join("fallback");
        fs::create_dir_all(fallback_root.join("Profile 1")).expect("create empty chromium profile");
        assert!(
            fallback_chromium_profiles(chrome_definition, &fallback_root, Some("1.0"))
                .expect("fallback profiles")
                .is_empty()
        );

        let firefox_definition = FIREFOX_BROWSERS
            .iter()
            .copied()
            .find(|definition| definition.key == "firefox")
            .expect("firefox definition");
        let firefox_root = dir.path().join("firefox");
        fs::create_dir_all(firefox_root.join("empty.default-release"))
            .expect("create empty firefox profile");
        fs::write(firefox_root.join("not-a-dir"), "skip").expect("write firefox file");
        unsafe {
            std::env::set_var(FIREFOX_PROFILES_OVERRIDE_ENV, &firefox_root);
        }
        assert!(discover_firefox_profiles(firefox_definition).expect("empty firefox").is_empty());

        restore_test_env_var("LOCALAPPDATA", None);
        restore_test_env_var("APPDATA", None);
        let windows_error = windows_data_dirs().expect_err("windows dirs should require env");
        assert!(windows_error.to_string().contains("LOCALAPPDATA or APPDATA"));

        restore_test_env_var(CHROME_USER_DATA_OVERRIDE_ENV, original_chrome.as_deref());
        restore_test_env_var(FIREFOX_PROFILES_OVERRIDE_ENV, original_firefox.as_deref());
        restore_test_env_var("LOCALAPPDATA", original_localappdata.as_deref());
        restore_test_env_var("APPDATA", original_appdata.as_deref());
    }

    #[test]
    fn restore_env_var_sets_and_clears_values() {
        let _guard = lock_env();
        let value = dir_path_os_string(tempdir().expect("tempdir").path());
        restore_test_env_var("LOCALAPPDATA", Some(value.as_os_str()));
        assert_eq!(std::env::var_os("LOCALAPPDATA"), Some(value.clone()));

        restore_test_env_var("LOCALAPPDATA", None);
        assert!(std::env::var_os("LOCALAPPDATA").is_none());
    }

    fn dir_path_os_string(path: &Path) -> std::ffi::OsString {
        path.as_os_str().to_os_string()
    }
}
