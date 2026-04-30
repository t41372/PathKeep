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
        std::env::remove_var(SAFARI_ROOT_OVERRIDE_ENV);
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

    let snapshot = stage_profile_snapshot(&paths, &profile).expect("snapshot");
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
