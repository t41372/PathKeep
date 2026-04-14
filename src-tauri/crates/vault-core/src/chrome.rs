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
/// Stable checksum record for one staged browser source file.
pub struct FileFingerprint {
    pub path: String,
    pub sha256: String,
}

#[derive(Debug)]
/// One staged browser profile snapshot ready for parser/archive ingest.
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
mod tests;
