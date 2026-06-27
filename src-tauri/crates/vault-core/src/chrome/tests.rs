//! Regression tests for browser discovery and staging helpers.
use super::*;
use crate::{
    config::{ProjectPaths, project_paths_with_root},
    utils::{restore_test_env_var, test_env_lock},
};
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
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
        profiles.iter().any(|profile| profile.browser_family == "safari" && profile.history_exists)
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

#[cfg(unix)]
#[test]
fn discover_safari_profile_marks_unreadable_history_as_full_disk_access_issue() {
    let _guard = lock_env();
    let dir = tempdir().expect("tempdir");
    let history_path = dir.path().join("History.db");
    fs::write(&history_path, b"safari").expect("write safari history");
    let mut permissions = fs::metadata(&history_path).expect("metadata").permissions();
    permissions.set_mode(0o000);
    fs::set_permissions(&history_path, permissions).expect("deny read");

    unsafe {
        std::env::set_var(SAFARI_ROOT_OVERRIDE_ENV, dir.path());
    }
    let profile = discover_safari_profile().expect("discover safari").expect("safari profile");
    unsafe {
        std::env::remove_var(SAFARI_ROOT_OVERRIDE_ENV);
    }

    let mut restore_permissions =
        fs::metadata(&history_path).expect("metadata for restore").permissions();
    restore_permissions.set_mode(0o600);
    fs::set_permissions(&history_path, restore_permissions).expect("restore read");

    assert!(profile.history_exists);
    assert!(!profile.history_readable);
    assert_eq!(profile.access_issue.as_deref(), Some("macos-full-disk-access"));
}

#[cfg(unix)]
#[test]
fn history_access_state_reports_generic_unreadable_browser_files() {
    let dir = tempdir().expect("tempdir");
    let history_path = dir.path().join("History");
    fs::write(&history_path, b"history").expect("write history");
    let mut permissions = fs::metadata(&history_path).expect("metadata").permissions();
    permissions.set_mode(0o000);
    fs::set_permissions(&history_path, permissions).expect("deny read");

    let (readable, issue) = history_access_state(&history_path, true, "chromium");

    let mut restore_permissions =
        fs::metadata(&history_path).expect("metadata for restore").permissions();
    restore_permissions.set_mode(0o600);
    fs::set_permissions(&history_path, restore_permissions).expect("restore read");

    assert!(!readable);
    assert_eq!(issue.as_deref(), Some("history-file-not-readable"));
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
        std::env::set_var(SAFARI_ROOT_OVERRIDE_ENV, dir.path().join("missing-safari"));
    }
    let default_candidates =
        firefox_root_candidates(firefox_definition).expect("default candidates");
    assert!(!default_candidates.is_empty());
    assert!(default_candidates.iter().all(|path| path.starts_with(dir.path())));
    assert!(discover_firefox_profiles(firefox_definition).expect("discover firefox").is_empty());
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
        history_readable: true,
        access_issue: None,
        browser_version: None,
        history_file_name: "places.sqlite".to_string(),
        history_bytes: 10,
        favicons_bytes: 0,
        supporting_bytes: 7,
        retention_boundary: retention_boundary_for_browser("firefox"),
    };

    let snapshot = stage_profile_snapshot(&paths, &profile).expect("snapshot").snapshot;
    let temp_name =
        snapshot.temp_dir.path().file_name().and_then(|name| name.to_str()).expect("temp dir name");
    assert!(temp_name.starts_with("firefox%3AProfile%201-"));
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

    let copied =
        copy_database_with_sidecars(source.path(), "History", destination.path()).expect("copy");

    assert_eq!(copied.path, destination.path().join("History"));
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
#[cfg(target_os = "macos")]
fn atlas_discovery_uses_chromium_parser_family_and_host_profile_root() {
    let _guard = lock_env();
    let dir = tempdir().expect("tempdir");
    let original_home = std::env::var_os("HOME");
    let host_root =
        dir.path().join("Library/Application Support/com.openai.atlas/browser-data/host");
    let profile_dir = host_root.join("user-test__profile");
    fs::create_dir_all(&profile_dir).expect("create atlas profile");
    fs::write(profile_dir.join("History"), b"history").expect("write atlas history");
    fs::write(profile_dir.join("Favicons"), b"favicons").expect("write atlas favicons");
    fs::write(host_root.join("Last Version"), "145.0.7584.0").expect("write atlas version");
    fs::write(
        host_root.join("Local State"),
        r#"{"profile":{"info_cache":{"user-test__profile":{"name":"Work Atlas","user_name":"atlas@example.test"}}}}"#,
    )
    .expect("write atlas local state");

    let definition = CHROMIUM_BROWSERS
        .iter()
        .copied()
        .find(|definition| definition.key == "atlas")
        .expect("atlas definition");
    unsafe {
        std::env::set_var("HOME", dir.path());
    }
    let profiles = discover_chromium_profiles(definition).expect("discover atlas");
    restore_test_env_var("HOME", original_home.as_deref());

    assert_eq!(profiles.len(), 1);
    assert_eq!(profiles[0].profile_id, "atlas:user-test__profile");
    assert_eq!(profiles[0].profile_name, "Work Atlas");
    assert_eq!(profiles[0].browser_family, "chromium");
    assert_eq!(profiles[0].browser_name, "ChatGPT Atlas");
    assert_eq!(profiles[0].browser_version.as_deref(), Some("145.0.7584.0"));
    assert_eq!(profiles[0].history_file_name, "History");
    assert!(profiles[0].favicons_path.is_some());
}

#[test]
#[cfg(target_os = "macos")]
fn comet_discovery_uses_chromium_parser_family_and_app_support_profile_root() {
    let _guard = lock_env();
    let dir = tempdir().expect("tempdir");
    let original_home = std::env::var_os("HOME");
    let comet_root = dir.path().join("Library/Application Support/Comet");
    let profile_dir = comet_root.join("Default");
    fs::create_dir_all(&profile_dir).expect("create comet profile");
    fs::write(profile_dir.join("History"), b"history").expect("write comet history");
    fs::write(profile_dir.join("Favicons"), b"favicons").expect("write comet favicons");
    fs::write(comet_root.join("Last Version"), "145.2.7632.5934").expect("write comet version");
    fs::write(
        comet_root.join("Local State"),
        r#"{"profile":{"info_cache":{"Default":{"name":"Comet Default","user_name":"comet@example.test"}}}}"#,
    )
    .expect("write comet local state");

    let definition = CHROMIUM_BROWSERS
        .iter()
        .copied()
        .find(|definition| definition.key == "comet")
        .expect("comet definition");
    unsafe {
        std::env::set_var("HOME", dir.path());
    }
    let profiles = discover_chromium_profiles(definition).expect("discover comet");
    restore_test_env_var("HOME", original_home.as_deref());

    assert_eq!(profiles.len(), 1);
    assert_eq!(profiles[0].profile_id, "comet:Default");
    assert_eq!(profiles[0].profile_name, "Comet Default");
    assert_eq!(profiles[0].browser_family, "chromium");
    assert_eq!(profiles[0].browser_name, "Perplexity Comet");
    assert_eq!(profiles[0].browser_version.as_deref(), Some("145.2.7632.5934"));
    assert_eq!(profiles[0].history_file_name, "History");
    assert!(profiles[0].favicons_path.is_some());
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
        if !matches!(definition.key, "atlas" | "comet") {
            assert!(!linux.is_empty());
        }
        if !matches!(definition.key, "arc" | "atlas" | "comet") {
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

/// Reopens a cleanly written SQLite database, starts an uncommitted write, and
/// flushes its dirty pages to disk so a populated rollback journal lands beside
/// it, then copies that hot `(database, journal)` pair into `dest_dir` under
/// `base_name`.
///
/// The result reproduces a browser caught mid-write whose writer then vanished —
/// the exact state that fails a read-only open with `SQLITE_READONLY_ROLLBACK`,
/// which is what broke backups when the staging fallback handed it to the parser.
fn capture_hot_journal_pair(clean_db: &Path, dest_dir: &Path, base_name: &str, dirty_sql: &str) {
    use rusqlite::Connection;

    let source_journal = PathBuf::from(format!("{}-journal", clean_db.display()));
    let writer = Connection::open(clean_db).expect("reopen clean database");
    writer
        .execute_batch(&format!("BEGIN IMMEDIATE;\n{dirty_sql}"))
        .expect("start uncommitted write");
    writer.cache_flush().expect("flush dirty pages into the rollback journal");
    assert!(source_journal.exists(), "an uncommitted flush must leave a rollback journal");

    fs::copy(clean_db, dest_dir.join(base_name)).expect("copy database into staging source");
    fs::copy(&source_journal, dest_dir.join(format!("{base_name}-journal")))
        .expect("copy hot journal into staging source");

    writer.execute_batch("ROLLBACK").ok();
}

#[test]
fn staging_recovers_a_hot_rollback_journal_that_blocks_the_read_only_parser() {
    use rusqlite::{Connection, ErrorCode, OpenFlags};

    // A cleanly written baseline with one committed row; the connection is
    // dropped so no journal lingers next to it.
    let clean = tempdir().expect("clean db dir");
    let clean_db = clean.path().join("History");
    {
        let seed = Connection::open(&clean_db).expect("open seed db");
        seed.execute_batch(
            "PRAGMA journal_mode=DELETE;\n\
             CREATE TABLE t (id INTEGER PRIMARY KEY, payload TEXT);\n\
             INSERT INTO t (id, payload) VALUES (1, 'committed');",
        )
        .expect("seed committed row");
    }

    // Capture it mid-write: a hot journal whose writer has gone, exactly like a
    // browser interrupted while saving history.
    let source = tempdir().expect("staging source");
    capture_hot_journal_pair(
        &clean_db,
        source.path(),
        "History",
        "UPDATE t SET payload = 'in-flight' WHERE id = 1;",
    );

    let read_payload = |database: &Path| -> rusqlite::Result<String> {
        Connection::open_with_flags(
            database,
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .and_then(|connection| {
            connection.query_row("SELECT payload FROM t WHERE id = 1", [], |row| row.get(0))
        })
    };

    // The hazard is real: opening the un-recovered copy read-only — exactly what
    // the parser does — fails with SQLITE_READONLY_ROLLBACK (extended code 776).
    let hazard = read_payload(&source.path().join("History")).expect_err("hot journal must block");
    match hazard {
        rusqlite::Error::SqliteFailure(error, _) => {
            assert_eq!(error.code, ErrorCode::ReadOnly);
            assert_eq!(error.extended_code, 776, "want SQLITE_READONLY_ROLLBACK");
        }
        other => panic!("expected a readonly-rollback failure, got {other:?}"),
    }

    // Staging recovers the copy: the read-only parser now opens it and sees the
    // last committed state, with the in-flight write rolled back.
    let staged_dir = tempdir().expect("staged dir");
    let staged = copy_database_with_sidecars(source.path(), "History", staged_dir.path())
        .expect("stage hot-journal copy");
    assert_eq!(read_payload(&staged.path).expect("recovered copy opens read-only"), "committed");
    // The fallback ran (the online snapshot could not read the hot-journal
    // source), and it recorded a reason for the run to surface.
    assert!(staged.fallback_reason.is_some(), "fallback must report why it was taken");
}

#[test]
fn copy_database_with_sidecars_prefers_a_journal_free_online_snapshot() {
    use rusqlite::{Connection, OpenFlags};

    let source = tempdir().expect("source");
    {
        let db = Connection::open(source.path().join("History")).expect("open source db");
        db.execute_batch(
            "CREATE TABLE t (id INTEGER PRIMARY KEY);\nINSERT INTO t (id) VALUES (1), (2), (3);",
        )
        .expect("seed rows");
    }
    // A harmless leftover sidecar the online snapshot should make irrelevant.
    fs::write(source.path().join("History-journal"), b"").expect("write empty journal");

    let dest = tempdir().expect("dest");
    let staged =
        copy_database_with_sidecars(source.path(), "History", dest.path()).expect("stage clean db");

    // The online backup produced one self-contained file: it is the clean path,
    // no sidecar followed it into staging, and the read-only parser opens it.
    assert!(staged.fallback_reason.is_none(), "a clean db must take the online snapshot path");
    assert!(!dest.path().join("History-journal").exists(), "online snapshot copies no journal");
    assert!(!dest.path().join("History-wal").exists());
    let reader = Connection::open_with_flags(
        &staged.path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .expect("read-only open");
    let count: i64 =
        reader.query_row("SELECT count(*) FROM t", [], |row| row.get(0)).expect("count rows");
    assert_eq!(count, 3);
}

#[test]
fn stage_profile_snapshot_recovers_a_hot_journal_for_the_chromium_parser() {
    use browser_history_fixtures::chromium::{
        ChromiumHistoryFixture, ChromiumUrlRow, ChromiumVisitRow,
    };
    use browser_history_parser::{
        chromium,
        types::{ChromiumReadCursor, HistoryDatabaseSet},
    };

    let visit_time = 1_777_000_000_000_i64;

    // A real-format Chrome History database, written cleanly...
    let clean = tempdir().expect("clean db dir");
    let clean_db = clean.path().join("History");
    ChromiumHistoryFixture::new()
        .add_url(ChromiumUrlRow {
            id: 1,
            url: "https://example.com/a".to_string(),
            title: Some("Committed Title".to_string()),
            visit_count: 1,
            typed_count: 0,
            last_visit_unix_ms: visit_time,
            hidden: false,
        })
        .add_visit(ChromiumVisitRow {
            id: 10,
            url_id: 1,
            visit_time_unix_ms: visit_time,
            from_visit: None,
            transition: Some(0),
            visit_duration_micros: None,
            is_known_to_sync: false,
            visited_link_id: None,
            external_referrer_url: None,
            app_id: None,
        })
        .write(&clean_db)
        .expect("write chrome fixture");

    // ...then captured mid-write with a hot rollback journal in the profile dir.
    let profile_dir = tempdir().expect("profile dir");
    capture_hot_journal_pair(
        &clean_db,
        profile_dir.path(),
        "History",
        "UPDATE urls SET title = 'in-flight' WHERE id = 1;",
    );

    let staging_root = tempdir().expect("staging root");
    let paths = sample_paths(staging_root.path());
    fs::create_dir_all(&paths.staging_dir).expect("create staging dir");

    let profile = BrowserProfile {
        profile_id: "chrome:Default".to_string(),
        profile_name: "Default".to_string(),
        browser_family: "chromium".to_string(),
        browser_name: "Google Chrome".to_string(),
        user_name: None,
        profile_path: profile_dir.path().display().to_string(),
        history_path: Some(profile_dir.path().join("History").display().to_string()),
        favicons_path: None,
        history_exists: true,
        history_readable: true,
        access_issue: None,
        browser_version: None,
        history_file_name: "History".to_string(),
        history_bytes: 0,
        favicons_bytes: 0,
        supporting_bytes: 0,
        retention_boundary: retention_boundary_for_browser("chromium"),
    };

    let staged = stage_profile_snapshot(&paths, &profile).expect("stage hot-journal profile");

    // The degraded staging path was recorded as a run warning, not swallowed.
    assert_eq!(staged.warnings.len(), 1, "the recovered-copy fallback must be reported");
    assert!(staged.warnings[0].contains("chrome:Default"), "the warning names the profile");

    // The production read path opens the recovered copy and parses the last
    // committed state — no SQLITE_READONLY_ROLLBACK, in-flight write rolled back.
    let parsed = chromium::parse_history(
        &HistoryDatabaseSet {
            history_path: staged.snapshot.history_path.clone(),
            favicons_path: None,
        },
        ChromiumReadCursor::default(),
    )
    .expect("parse recovered chrome copy");
    assert_eq!(parsed.urls.len(), 1, "the committed url survives recovery");
    assert_eq!(parsed.visits.len(), 1, "the committed visit survives recovery");
}

#[test]
fn recover_staged_database_folds_a_captured_wal_into_the_main_file() {
    use rusqlite::{Connection, OpenFlags};

    // A WAL database whose committed rows live only in the -wal: autocheckpoint
    // is off and the writer is kept open so nothing checkpoints them into the
    // main file. Copy the whole WAL set out, the way the raw fallback would.
    let live = tempdir().expect("live");
    let live_db = live.path().join("History");
    let writer = Connection::open(&live_db).expect("open wal db");
    writer
        .execute_batch(
            "PRAGMA journal_mode=WAL;\n\
             PRAGMA wal_autocheckpoint=0;\n\
             CREATE TABLE t (id INTEGER PRIMARY KEY);\n\
             INSERT INTO t (id) VALUES (1), (2), (3), (4);",
        )
        .expect("seed wal rows");
    assert!(live.path().join("History-wal").exists(), "writer must leave a WAL");

    let staged = tempdir().expect("staged");
    let staged_db = staged.path().join("History");
    for suffix in ["", "-wal", "-shm"] {
        let from = PathBuf::from(format!("{}{suffix}", live_db.display()));
        if from.exists() {
            fs::copy(&from, staged.path().join(format!("History{suffix}")))
                .expect("copy wal member");
        }
    }
    drop(writer);

    recover_staged_database(&staged_db).expect("recover captured wal");

    // The WAL is checkpointed away: a read-only open with no sidecar present sees
    // every committed row.
    assert!(!staged.path().join("History-wal").exists(), "recovery clears the WAL");
    let reader = Connection::open_with_flags(
        &staged_db,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .expect("read-only open of recovered wal copy");
    let count: i64 =
        reader.query_row("SELECT count(*) FROM t", [], |row| row.get(0)).expect("count rows");
    assert_eq!(count, 4, "all committed WAL rows survive recovery");
}

#[test]
fn staging_backs_up_a_live_wal_source_to_a_complete_journal_free_copy() {
    use rusqlite::{Connection, OpenFlags};

    // A WAL database with committed rows still living in the -wal (autocheckpoint
    // off, writer kept open), captured the way a live Firefox `places.sqlite`
    // would be. Whichever path staging takes — the online snapshot reads WAL
    // frames directly, the fallback recovers them — the result must be complete.
    let live = tempdir().expect("live");
    let live_db = live.path().join("History");
    let writer = Connection::open(&live_db).expect("open wal db");
    writer
        .execute_batch(
            "PRAGMA journal_mode=WAL;\n\
             PRAGMA wal_autocheckpoint=0;\n\
             CREATE TABLE t (id INTEGER PRIMARY KEY, payload TEXT);\n\
             INSERT INTO t (id, payload) VALUES (1, 'a'), (2, 'b'), (3, 'c');",
        )
        .expect("seed wal rows");
    assert!(live.path().join("History-wal").exists(), "writer must leave a WAL");

    let source = tempdir().expect("source");
    for suffix in ["", "-wal", "-shm"] {
        let from = PathBuf::from(format!("{}{suffix}", live_db.display()));
        if from.exists() {
            fs::copy(&from, source.path().join(format!("History{suffix}")))
                .expect("copy wal member");
        }
    }
    drop(writer);

    let dest = tempdir().expect("dest");
    let staged =
        copy_database_with_sidecars(source.path(), "History", dest.path()).expect("stage wal copy");

    // The copy the parser receives is self-contained: every committed row is
    // present and a read-only open needs no WAL sidecar.
    assert!(!dest.path().join("History-wal").exists(), "staged copy carries no live WAL");
    let reader = Connection::open_with_flags(
        &staged.path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .expect("read-only open of staged wal copy");
    let count: i64 =
        reader.query_row("SELECT count(*) FROM t", [], |row| row.get(0)).expect("count rows");
    assert_eq!(count, 3, "all committed WAL rows survive staging");
}
