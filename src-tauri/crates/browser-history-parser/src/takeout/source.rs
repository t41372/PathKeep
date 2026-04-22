//! Takeout source discovery helpers.
//!
//! ## Responsibilities
//! - Enumerate candidate payload files from directories or zip archives.
//! - Recognize supported payload families by locale-aware path dispatch.
//! - Build the lightweight inspection surface used before deeper parsing.
//!
//! ## Not responsible for
//! - Parsing payload JSON into canonical or source-native rows.
//! - Merging multiple payload reports into one parser history.
//! - Archive quarantine or import-batch semantics.

use super::{
    KIND_BROWSER_JSON, KIND_INDEX, KIND_JSONL, KIND_SESSION_JSON, KIND_TYPED_URL_JSON,
    TakeoutPathDisposition, TakeoutPathMatch,
};
use crate::{
    ParseError,
    types::{DatabaseInspection, ParserWarning},
};
use std::{
    collections::BTreeSet,
    fs,
    io::Read,
    path::{Path, PathBuf},
};
use walkdir::WalkDir;
use zip::ZipArchive;

/// Describes one discovered file inside a Takeout directory or zip archive.
#[derive(Debug, Clone)]
pub(super) struct TakeoutFile {
    pub(super) path: String,
    pub(super) from_zip: bool,
}

/// Builds the lightweight inspection result for a whole Takeout source.
pub fn inspect_history(path: &Path) -> Result<DatabaseInspection, ParseError> {
    let files = gather_takeout_files(path)?;
    let mut table_names = BTreeSet::new();
    let mut warnings = Vec::new();
    for file in files {
        let path_match = classify_payload_path(&file.path);
        if let Some(kind) = path_match.recognized_kind
            && kind != KIND_INDEX
        {
            table_names.insert(kind.to_string());
        }
    }
    if table_names.is_empty() {
        warnings.push(ParserWarning {
            code: "no-recognized-payload".to_string(),
            message: "No importable Takeout payloads were recognized in the provided source."
                .to_string(),
        });
    }
    Ok(DatabaseInspection { table_names: table_names.into_iter().collect(), warnings })
}

/// Classifies one Takeout file path according to the current Chrome-first import scope.
pub fn classify_payload_path(path: &str) -> TakeoutPathMatch {
    let normalized = normalize_takeout_path(path);
    let file_name = normalized.rsplit('/').next().unwrap_or(normalized.as_str());
    let first_segment = normalized.split('/').next().unwrap_or(normalized.as_str());
    let has_chrome_segment = normalized.split('/').any(|segment| segment == "chrome");

    if file_name.ends_with(".jsonl") {
        return TakeoutPathMatch {
            family: KIND_JSONL,
            recognized_kind: Some(KIND_JSONL),
            locale: None,
            disposition: TakeoutPathDisposition::WillImport,
            reason_code: "jsonl-history-fixture",
        };
    }

    if is_importable_chrome_history_path(file_name, has_chrome_segment) {
        return TakeoutPathMatch {
            family: KIND_BROWSER_JSON,
            recognized_kind: Some(KIND_BROWSER_JSON),
            locale: chrome_history_locale(file_name),
            disposition: TakeoutPathDisposition::WillImport,
            reason_code: "chrome-history-json",
        };
    }

    if is_typed_url_path(file_name) {
        return TakeoutPathMatch {
            family: KIND_TYPED_URL_JSON,
            recognized_kind: Some(KIND_TYPED_URL_JSON),
            locale: None,
            disposition: TakeoutPathDisposition::KnownIgnored,
            reason_code: "source-evidence-only",
        };
    }

    if is_session_path(file_name) {
        return TakeoutPathMatch {
            family: KIND_SESSION_JSON,
            recognized_kind: Some(KIND_SESSION_JSON),
            locale: None,
            disposition: TakeoutPathDisposition::KnownIgnored,
            reason_code: "source-evidence-only",
        };
    }

    if file_name == "archive_browser.html" || file_name == "archiv_übersicht.html" {
        return TakeoutPathMatch {
            family: KIND_INDEX,
            recognized_kind: Some(KIND_INDEX),
            locale: if file_name == "archiv_übersicht.html" { Some("de") } else { Some("en") },
            disposition: TakeoutPathDisposition::KnownIgnored,
            reason_code: "takeout-index",
        };
    }

    if is_chrome_activity_json_path(&normalized) {
        return TakeoutPathMatch {
            family: "chrome-activity",
            recognized_kind: None,
            locale: chrome_activity_locale(&normalized),
            disposition: TakeoutPathDisposition::NeedsReview,
            reason_code: "chrome-my-activity-json",
        };
    }

    if is_chrome_activity_html_path(&normalized) {
        return TakeoutPathMatch {
            family: "chrome-activity",
            recognized_kind: None,
            locale: chrome_activity_locale(&normalized),
            disposition: TakeoutPathDisposition::NeedsReview,
            reason_code: "chrome-my-activity-html",
        };
    }

    if has_chrome_segment {
        return TakeoutPathMatch {
            family: "chrome-supporting-file",
            recognized_kind: None,
            locale: None,
            disposition: chrome_supporting_file_disposition(file_name),
            reason_code: chrome_supporting_file_reason(file_name),
        };
    }

    if first_segment == "my activity" || first_segment == "meine aktivitäten" {
        return TakeoutPathMatch {
            family: "google-activity",
            recognized_kind: None,
            locale: if first_segment == "meine aktivitäten" { Some("de") } else { Some("en") },
            disposition: TakeoutPathDisposition::KnownIgnored,
            reason_code: "activity-outside-scope",
        };
    }

    if KNOWN_IGNORED_TOP_LEVEL_EN.contains(&first_segment) {
        return TakeoutPathMatch {
            family: "outside-scope",
            recognized_kind: None,
            locale: Some("en"),
            disposition: TakeoutPathDisposition::KnownIgnored,
            reason_code: "outside-chrome-scope",
        };
    }

    if KNOWN_IGNORED_TOP_LEVEL_DE.contains(&first_segment) {
        return TakeoutPathMatch {
            family: "outside-scope",
            recognized_kind: None,
            locale: Some("de"),
            disposition: TakeoutPathDisposition::KnownIgnored,
            reason_code: "outside-chrome-scope",
        };
    }

    if looks_history_like(file_name) {
        return TakeoutPathMatch {
            family: "unknown-history-like",
            recognized_kind: None,
            locale: None,
            disposition: TakeoutPathDisposition::NeedsReview,
            reason_code: "unrecognized-history-file",
        };
    }

    TakeoutPathMatch {
        family: "outside-scope",
        recognized_kind: None,
        locale: None,
        disposition: TakeoutPathDisposition::KnownIgnored,
        reason_code: "outside-chrome-scope",
    }
}

/// Recognizes the supported Takeout payload family for one file path.
pub fn recognize_payload(path: &str) -> Option<&'static str> {
    classify_payload_path(path).recognized_kind
}

const KNOWN_IGNORED_TOP_LEVEL_EN: &[&str] = &[
    "access log activity",
    "android device configuration service",
    "assistant notes and lists",
    "blogger",
    "calendar",
    "classroom",
    "cloud print",
    "contacts",
    "discover",
    "drive",
    "fit",
    "gmail",
    "google account",
    "google business profile",
    "google chat",
    "google developers",
    "google finanzas",
    "google fit",
    "google my business",
    "google pay",
    "google photos",
    "google play books",
    "google play games services",
    "google play movies _ tv",
    "google play store",
    "google shopping",
    "google store",
    "google translator toolkit",
    "google workspace marketplace",
    "groups",
    "home app",
    "keep",
    "location history",
    "mail",
    "maps",
    "maps (your places)",
    "my maps",
    "news",
    "profile",
    "saved",
    "search contributions",
    "shopping lists",
    "tasks",
    "youtube and youtube music",
];

const KNOWN_IGNORED_TOP_LEVEL_DE: &[&str] = &[
    "android-gerätekonfigurationsdienst",
    "aufgaben",
    "business messages",
    "classroom",
    "discover",
    "drive",
    "gespeichert",
    "gmail",
    "google chat",
    "google developers",
    "google fit",
    "google finanzen",
    "google fotos",
    "google kontakte",
    "google news",
    "google pay",
    "google play filme _ serien",
    "google play store",
    "google play-spieldienste",
    "google shopping",
    "google unternehmensprofil",
    "google workspace marketplace",
    "google-hilfe-communities",
    "google-konto",
    "groups",
    "home app",
    "kalender",
    "location history (timeline)",
    "maps",
    "maps (meine orte)",
    "profil",
    "search contributions",
    "tasks",
    "youtube und youtube music",
    "zugriffsprotokollaktivitäten",
];

fn normalize_takeout_path(path: &str) -> String {
    let mut normalized = String::with_capacity(path.len());
    let mut previous_space = false;
    for char in path.chars() {
        let next = match char {
            '\\' => '/',
            _ if char.is_whitespace() => ' ',
            _ => char,
        };
        if next == ' ' {
            if previous_space {
                continue;
            }
            previous_space = true;
        } else {
            previous_space = false;
        }
        normalized.push(next);
    }
    normalized.trim_matches([' ', '/']).to_lowercase()
}

fn is_importable_chrome_history_path(file_name: &str, has_chrome_segment: bool) -> bool {
    matches!(file_name, "browserhistory.json" | "history.json" | "verlauf.json")
        && (has_chrome_segment
            || file_name == "browserhistory.json"
            || file_name == "history.json"
            || file_name == "verlauf.json")
}

fn chrome_history_locale(file_name: &str) -> Option<&'static str> {
    match file_name {
        "verlauf.json" => Some("de"),
        "browserhistory.json" | "history.json" => Some("en"),
        _ => None,
    }
}

fn is_typed_url_path(file_name: &str) -> bool {
    matches!(file_name, "typedurl.json" | "typed url.json")
}

fn is_session_path(file_name: &str) -> bool {
    file_name == "session.json" || file_name == "sessions.json"
}

fn is_chrome_activity_json_path(normalized: &str) -> bool {
    chrome_activity_suffixes(".json").iter().any(|suffix| normalized.ends_with(suffix))
}

fn is_chrome_activity_html_path(normalized: &str) -> bool {
    chrome_activity_suffixes(".html").iter().any(|suffix| normalized.ends_with(suffix))
}

fn chrome_activity_locale(normalized: &str) -> Option<&'static str> {
    for (segment, locale) in [
        ("my activity", "en"),
        ("meine aktivitäten", "de"),
        ("我的活动", "zh-cn"),
        ("我的活動", "zh-tw"),
    ] {
        if normalized.contains(&normalize_takeout_path(segment)) {
            return Some(locale);
        }
    }
    None
}

fn chrome_supporting_file_disposition(file_name: &str) -> TakeoutPathDisposition {
    if looks_history_like(file_name) {
        TakeoutPathDisposition::NeedsReview
    } else {
        TakeoutPathDisposition::KnownIgnored
    }
}

fn chrome_supporting_file_reason(file_name: &str) -> &'static str {
    if looks_history_like(file_name) {
        "unrecognized-history-file"
    } else {
        "chrome-supporting-file"
    }
}

fn looks_history_like(file_name: &str) -> bool {
    file_name.contains("history")
        || file_name.contains("verlauf")
        || file_name.contains("myactivity")
        || file_name.contains("browser")
}

fn chrome_activity_suffixes(extension: &str) -> [&'static str; 4] {
    [
        match extension {
            ".json" => "my activity/chrome/myactivity.json",
            ".html" => "my activity/chrome/myactivity.html",
            _ => unreachable!("unsupported chrome activity extension"),
        },
        match extension {
            ".json" => "meine aktivitäten/chrome/meine aktivitäten.json",
            ".html" => "meine aktivitäten/chrome/meine aktivitäten.html",
            _ => unreachable!("unsupported chrome activity extension"),
        },
        match extension {
            ".json" => "我的活动/chrome/我的活动.json",
            ".html" => "我的活动/chrome/我的活动.html",
            _ => unreachable!("unsupported chrome activity extension"),
        },
        match extension {
            ".json" => "我的活動/chrome/我的活動.json",
            ".html" => "我的活動/chrome/我的活動.html",
            _ => unreachable!("unsupported chrome activity extension"),
        },
    ]
}

/// Enumerates every file candidate contained in a Takeout directory or zip archive.
pub(super) fn gather_takeout_files(source: &Path) -> Result<Vec<TakeoutFile>, ParseError> {
    if source.is_dir() {
        return Ok(WalkDir::new(source)
            .into_iter()
            .filter_map(Result::ok)
            .filter(|entry| entry.file_type().is_file())
            .filter(|entry| !should_skip_takeout_file(entry.path().to_string_lossy().as_ref()))
            .map(|entry| TakeoutFile { path: entry.path().display().to_string(), from_zip: false })
            .collect());
    }

    let file = fs::File::open(source).map_err(|source_error| ParseError::ReadSource {
        path: source.to_path_buf(),
        source: source_error,
    })?;
    let mut archive = ZipArchive::new(file)?;
    let mut files = Vec::new();
    for index in 0..archive.len() {
        let entry = archive.by_index(index)?;
        if entry.is_file() && !should_skip_takeout_file(entry.name()) {
            files.push(TakeoutFile { path: entry.name().to_string(), from_zip: true });
        }
    }
    Ok(files)
}

/// Reads one Takeout file or zip entry into memory for parser-side processing.
pub(super) fn read_takeout_file(
    source_root: &Path,
    file: &TakeoutFile,
) -> Result<Vec<u8>, ParseError> {
    if file.from_zip {
        return read_zip_entry(source_root, &file.path);
    }
    fs::read(&file.path)
        .map_err(|source| ParseError::ReadSource { path: PathBuf::from(&file.path), source })
}

fn read_zip_entry(source: &Path, entry_name: &str) -> Result<Vec<u8>, ParseError> {
    let file = fs::File::open(source).map_err(|source_error| ParseError::ReadSource {
        path: source.to_path_buf(),
        source: source_error,
    })?;
    let mut archive = ZipArchive::new(file)?;
    let mut entry = archive.by_name(entry_name)?;
    let mut bytes = Vec::new();
    entry.read_to_end(&mut bytes).map_err(|source_error| ParseError::ReadSource {
        path: PathBuf::from(format!("{}::{entry_name}", source.display())),
        source: source_error,
    })?;
    Ok(bytes)
}

fn should_skip_takeout_file(path: &str) -> bool {
    let normalized = normalize_takeout_path(path);
    let file_name = normalized.rsplit('/').next().unwrap_or(normalized.as_str());
    file_name.starts_with('.') || normalized.split('/').any(|segment| segment == "__macosx")
}
