//! Browser discovery helpers.
//!
//! This module turns host path heuristics into `BrowserProfile` read models.
//! It intentionally keeps discovery separate from staging/parsing so we can be
//! honest about unreadable/missing profiles without mutating or opening them.

use super::paths::{
    chromium_override_active, chromium_root_candidates, default_safari_root,
    discovery_overrides_active_with, firefox_override_active, firefox_root_candidates,
    safari_override_active, should_discover_chromium_definition, should_discover_firefox,
    should_discover_safari,
};
use super::staging::profile_storage_bytes;
use super::*;

/// Discovers supported browser profiles across the current host roots and overrides.
pub fn discover_profiles() -> Result<Vec<BrowserProfile>> {
    let chrome_override_active = chromium_override_active();
    let firefox_override_active = firefox_override_active();
    let safari_override_active = safari_override_active();
    let overrides_active = discovery_overrides_active_with(
        chrome_override_active,
        firefox_override_active,
        safari_override_active,
    );
    let mut profiles = Vec::new();
    for definition in CHROMIUM_BROWSERS {
        if !should_discover_chromium_definition(overrides_active, definition) {
            continue;
        }
        profiles.extend(discover_chromium_profiles(definition)?);
    }
    if should_discover_firefox(overrides_active, firefox_override_active) {
        for definition in FIREFOX_BROWSERS {
            profiles.extend(discover_firefox_profiles(definition)?);
        }
    }
    if should_discover_safari(overrides_active, safari_override_active)
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

/// Discovers Chromium-family profiles for one browser definition.
pub(super) fn discover_chromium_profiles(
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
            let history_exists = history_path.exists();
            let favicons_exists = favicons_path.exists();
            let (history_bytes, favicons_bytes, supporting_bytes) = profile_storage_bytes(
                &history_path,
                favicons_exists.then_some(favicons_path.as_path()),
            );
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
                history_path: history_exists.then(|| history_path.display().to_string()),
                favicons_path: favicons_exists.then(|| favicons_path.display().to_string()),
                history_exists,
                browser_version: chrome_version.clone(),
                history_file_name: "History".to_string(),
                history_bytes,
                favicons_bytes,
                supporting_bytes,
                retention_boundary: retention_boundary_for_browser(definition.family),
            });
        }
    }
    Ok(profiles)
}

/// Reads Chromium's `Local State` profile info cache when present.
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

/// Falls back to directory scanning when Chromium `Local State` is missing or incomplete.
pub(super) fn fallback_chromium_profiles(
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
        let favicons_exists = favicons_path.exists();
        let (history_bytes, favicons_bytes, supporting_bytes) = profile_storage_bytes(
            &history_path,
            favicons_exists.then_some(favicons_path.as_path()),
        );
        profiles.push(BrowserProfile {
            profile_id: format!("{}:{}", definition.key, raw_profile_id),
            profile_name: raw_profile_id.clone(),
            browser_family: definition.family.to_string(),
            browser_name: definition.name.to_string(),
            user_name: None,
            profile_path: profile_path.display().to_string(),
            history_path: Some(history_path.display().to_string()),
            favicons_path: favicons_exists.then(|| favicons_path.display().to_string()),
            history_exists: true,
            browser_version: browser_version.map(ToString::to_string),
            history_file_name: "History".to_string(),
            history_bytes,
            favicons_bytes,
            supporting_bytes,
            retention_boundary: retention_boundary_for_browser(definition.family),
        });
    }
    Ok(profiles)
}

/// Treats a browser root that directly contains `History` as one synthetic profile.
pub(super) fn direct_root_chromium_profile(
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
    let favicons_exists = favicons_path.exists();
    let (history_bytes, favicons_bytes, supporting_bytes) =
        profile_storage_bytes(&history_path, favicons_exists.then_some(favicons_path.as_path()));

    Some(BrowserProfile {
        profile_id: format!("{}:{profile_suffix}", definition.key),
        profile_name: profile_name.to_string(),
        browser_family: definition.family.to_string(),
        browser_name: definition.name.to_string(),
        user_name: None,
        profile_path: root.display().to_string(),
        history_path: Some(history_path.display().to_string()),
        favicons_path: favicons_exists.then(|| favicons_path.display().to_string()),
        history_exists: true,
        browser_version: browser_version.map(ToString::to_string),
        history_file_name: "History".to_string(),
        history_bytes,
        favicons_bytes,
        supporting_bytes,
        retention_boundary: retention_boundary_for_browser(definition.family),
    })
}

/// Discovers Firefox-family profiles for one browser definition.
pub(super) fn discover_firefox_profiles(
    definition: FirefoxBrowserDefinition,
) -> Result<Vec<BrowserProfile>> {
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
            let (history_bytes, favicons_bytes, supporting_bytes) =
                profile_storage_bytes(&history_path, None);
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
                history_bytes,
                favicons_bytes,
                supporting_bytes,
                retention_boundary: retention_boundary_for_browser("firefox"),
            });
        }
    }
    Ok(profiles)
}

/// Parses Firefox `profiles.ini` into `profile dir -> display name` mappings.
pub(super) fn parse_firefox_profile_names(root: &Path) -> BTreeMap<String, String> {
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

/// Returns the singleton Safari profile placeholder/read model when relevant.
pub(super) fn discover_safari_profile() -> Result<Option<BrowserProfile>> {
    #[rustfmt::skip]
    let Some(safari_root) = std::env::var_os(SAFARI_ROOT_OVERRIDE_ENV).map(PathBuf::from).map(Some).unwrap_or(default_safari_root()?) else { return Ok(None); };

    let history_path = safari_root.join("History.db");
    let history_exists = history_path.exists();
    let (history_bytes, favicons_bytes, supporting_bytes) =
        profile_storage_bytes(&history_path, None);
    Ok(Some(BrowserProfile {
        profile_id: "safari:default".to_string(),
        profile_name: "Default".to_string(),
        browser_family: "safari".to_string(),
        browser_name: "Safari".to_string(),
        user_name: None,
        profile_path: safari_root.display().to_string(),
        history_path: history_exists.then(|| history_path.display().to_string()),
        favicons_path: None,
        history_exists,
        browser_version: None,
        history_file_name: "History.db".to_string(),
        history_bytes,
        favicons_bytes,
        supporting_bytes,
        retention_boundary: retention_boundary_for_browser("safari"),
    }))
}
