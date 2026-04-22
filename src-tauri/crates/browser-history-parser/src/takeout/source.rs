//! Takeout source discovery helpers.
//!
//! ## Responsibilities
//! - Enumerate candidate payload files from directories or zip archives.
//! - Recognize supported payload families by path.
//! - Build the lightweight inspection surface used before deeper parsing.
//!
//! ## Not responsible for
//! - Parsing payload JSON into canonical or source-native rows.
//! - Merging multiple payload reports into one parser history.
//! - Archive quarantine or import-batch semantics.

use super::{KIND_BROWSER_JSON, KIND_INDEX, KIND_JSONL, KIND_SESSION_JSON, KIND_TYPED_URL_JSON};
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
        if let Some(kind) = recognize_payload(&file.path)
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

/// Recognizes the supported Takeout payload family for one file path.
pub fn recognize_payload(path: &str) -> Option<&'static str> {
    let lower = path.to_ascii_lowercase();
    if lower.ends_with(".jsonl") {
        Some(KIND_JSONL)
    } else if lower.ends_with(".json")
        && ((lower.contains("typed") && lower.contains("url")) || lower.contains("typedurl"))
    {
        Some(KIND_TYPED_URL_JSON)
    } else if lower.ends_with(".json") && lower.contains("session") {
        Some(KIND_SESSION_JSON)
    } else if lower.ends_with(".json") && (lower.contains("browser") || lower.contains("history")) {
        Some(KIND_BROWSER_JSON)
    } else if lower.ends_with("archive_browser.html") {
        Some(KIND_INDEX)
    } else {
        None
    }
}

/// Enumerates every file candidate contained in a Takeout directory or zip archive.
pub(super) fn gather_takeout_files(source: &Path) -> Result<Vec<TakeoutFile>, ParseError> {
    if source.is_dir() {
        return Ok(WalkDir::new(source)
            .into_iter()
            .filter_map(Result::ok)
            .filter(|entry| entry.file_type().is_file())
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
        if entry.is_file() {
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
