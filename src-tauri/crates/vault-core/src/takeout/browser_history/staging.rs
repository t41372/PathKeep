//! Browser-direct database staging and source-family detection.
//!
//! ## Responsibilities
//! - Resolve a user-selected browser database or profile directory to the
//!   concrete SQLite history file the parser can read.
//! - Snapshot or copy live browser SQLite databases into the import staging
//!   directory before parser access.
//! - Detect whether the staged database uses the Chromium or Safari history
//!   schema and attach profile metadata for the canonical import path.
//!
//! ## Not responsible for
//! - Writing canonical archive rows.
//! - Persisting source evidence or import batches.
//! - Discovering installed browser profiles before the user selects one.
//!
//! ## Dependencies
//! - `browser-history-parser` streaming contracts for Chromium and Safari.
//! - SQLite backup APIs for coherent local snapshots.
//! - The surrounding Takeout import module for shared path, hash, and model
//!   types.
//!
//! ## Performance notes
//! - The import path streams parser rows in bounded chunks. Staging copies only
//!   the selected SQLite files and sidecars, not the whole browser profile.

use super::*;
use browser_history_parser::{
    ChromiumReadCursor, HistoryBatchConsumer, HistoryDatabaseSet, StreamHistoryError,
    StreamedHistory, chromium, safari,
};
use rusqlite::{MAIN_DB, OpenFlags};
use tempfile::TempDir;

const BROWSER_IMPORT_CHUNK_SIZE: usize = 10_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum BrowserHistoryFamily {
    Chromium,
    Safari,
}

impl BrowserHistoryFamily {
    pub(super) fn as_str(self) -> &'static str {
        match self {
            Self::Chromium => "chromium",
            Self::Safari => "safari",
        }
    }

    pub(super) fn source_kind(self) -> &'static str {
        match self {
            Self::Chromium => "chromium-history",
            Self::Safari => "safari-history",
        }
    }

    fn file_kind(self) -> &'static str {
        match self {
            Self::Chromium => "chromium-history-db",
            Self::Safari => "safari-history-db",
        }
    }

    fn default_browser_name(self) -> &'static str {
        match self {
            Self::Chromium => "Google Chrome",
            Self::Safari => "Safari",
        }
    }
}

#[derive(Debug)]
pub(super) struct StagedBrowserHistorySource {
    pub(super) _temp_dir: TempDir,
    pub(super) requested_path: PathBuf,
    pub(super) history_path: PathBuf,
    pub(super) favicons_path: Option<PathBuf>,
    pub(super) family: BrowserHistoryFamily,
    pub(super) profile_id: String,
    pub(super) browser_name: String,
    pub(super) profile_name: String,
}

/// Stages one Browser Direct source before parser or archive access.
///
/// The preview and execute flows both call this boundary so live browser
/// databases are validated consistently before any canonical write occurs.
pub(super) fn stage_browser_history_source(
    paths: &ProjectPaths,
    request: &BrowserHistoryImportRequest,
) -> Result<StagedBrowserHistorySource> {
    let requested_path = resolve_requested_history_path(Path::new(&request.source_path))?;
    let temp_dir = tempfile::Builder::new()
        .prefix("browser-direct-")
        .tempdir_in(&paths.staging_dir)
        .with_context(|| format!("creating temp dir in {}", paths.staging_dir.display()))?;
    let history_file_name =
        requested_path.file_name().and_then(|name| name.to_str()).unwrap_or("History");
    let history_path = temp_dir.path().join(history_file_name);
    snapshot_or_copy_sqlite_database(&requested_path, &history_path, request)?;
    quick_check_sqlite(&history_path)?;
    let detected_family = detect_browser_history_family(&history_path)?;
    let family =
        normalize_requested_family(request.browser_family.as_deref()).unwrap_or(detected_family);
    if family != detected_family {
        anyhow::bail!(
            "selected file looks like {}, but the request identified it as {}",
            detected_family.as_str(),
            family.as_str()
        );
    }
    let favicons_path = if family == BrowserHistoryFamily::Chromium {
        stage_chromium_favicons(&requested_path, temp_dir.path()).ok()
    } else {
        None
    };
    let profile_id = request.profile_id.clone().unwrap_or_else(|| {
        let fingerprint = sha256_hex(requested_path.display().to_string().as_bytes());
        format!("browser-direct::{}::{}", family.as_str(), &fingerprint[..12])
    });
    Ok(StagedBrowserHistorySource {
        _temp_dir: temp_dir,
        requested_path,
        history_path,
        favicons_path,
        family,
        profile_id,
        browser_name: request
            .browser_name
            .clone()
            .unwrap_or_else(|| family.default_browser_name().to_string()),
        profile_name: request
            .profile_name
            .clone()
            .unwrap_or_else(|| "Imported browser profile".to_string()),
    })
}

/// Streams one staged Browser Direct database through the matching parser.
///
/// Keeping parser dispatch here makes the archive-writing module independent of
/// source-specific sidecar details such as Chromium Favicons.
pub(super) fn stream_browser_history<C>(
    staged: &StagedBrowserHistorySource,
    consumer: &mut C,
) -> Result<StreamedHistory>
where
    C: HistoryBatchConsumer<Error = anyhow::Error>,
{
    match staged.family {
        BrowserHistoryFamily::Chromium => {
            let source = HistoryDatabaseSet {
                history_path: staged.history_path.clone(),
                favicons_path: staged.favicons_path.clone(),
            };
            chromium::stream_history(
                &source,
                ChromiumReadCursor::default(),
                BROWSER_IMPORT_CHUNK_SIZE,
                consumer,
            )
        }
        BrowserHistoryFamily::Safari => {
            safari::stream_history(&staged.history_path, 0, 0, BROWSER_IMPORT_CHUNK_SIZE, consumer)
        }
    }
    .map_err(|error| match error {
        StreamHistoryError::Parse(error) => anyhow::Error::new(error),
        StreamHistoryError::Consumer(error) => error,
    })
}

/// Builds the review file row used by preview, execute, and import-batch detail.
pub(super) fn browser_file_report(
    staged: &StagedBrowserHistorySource,
    status: &str,
    records: usize,
) -> TakeoutFileReport {
    TakeoutFileReport {
        path: staged.requested_path.display().to_string(),
        kind: staged.family.file_kind().to_string(),
        status: status.to_string(),
        records,
        classification: "will-import".to_string(),
        reason_code: Some(format!("{}-history-sqlite", staged.family.as_str())),
        reason_detail: None,
        detected_locale: None,
    }
}

fn resolve_requested_history_path(path: &Path) -> Result<PathBuf> {
    if path.is_dir() {
        for file_name in ["History.db", "History"] {
            let candidate = path.join(file_name);
            if candidate.exists() {
                return Ok(candidate);
            }
        }
        anyhow::bail!(
            "browser directory {} does not contain History.db or History",
            path.display()
        );
    }
    Ok(path.to_path_buf())
}

fn snapshot_or_copy_sqlite_database(
    source: &Path,
    destination: &Path,
    request: &BrowserHistoryImportRequest,
) -> Result<()> {
    if let Err(snapshot_error) = snapshot_sqlite_database(source, destination) {
        copy_sqlite_database_with_sidecars(source, destination).with_context(|| {
            safari_access_hint(request, source)
                .unwrap_or_else(|| format!("copying browser database {}", source.display()))
        })?;
        if let Err(check_error) = quick_check_sqlite(destination) {
            return Err(check_error).with_context(|| {
                format!(
                    "staged copy from {} failed SQLite quick_check after online backup failed: {snapshot_error:#}",
                    source.display()
                )
            });
        }
    }
    Ok(())
}

fn snapshot_sqlite_database(source: &Path, destination: &Path) -> Result<()> {
    let source_connection = Connection::open_with_flags(
        source,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .with_context(|| format!("opening source database {}", source.display()))?;
    source_connection
        .backup(MAIN_DB, destination, None)
        .with_context(|| format!("creating SQLite snapshot from {}", source.display()))?;
    Ok(())
}

fn copy_sqlite_database_with_sidecars(source: &Path, destination: &Path) -> Result<()> {
    fs::copy(source, destination)
        .with_context(|| format!("copying {} to {}", source.display(), destination.display()))?;
    for suffix in ["-wal", "-shm", "-journal"] {
        let source_sidecar = PathBuf::from(format!("{}{}", source.display(), suffix));
        if source_sidecar.exists() {
            let destination_sidecar = PathBuf::from(format!("{}{}", destination.display(), suffix));
            fs::copy(&source_sidecar, &destination_sidecar).with_context(|| {
                format!("copying {} to {}", source_sidecar.display(), destination_sidecar.display())
            })?;
        }
    }
    Ok(())
}

fn quick_check_sqlite(path: &Path) -> Result<()> {
    let connection = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .with_context(|| format!("opening staged database {}", path.display()))?;
    let result: String = connection
        .query_row("PRAGMA quick_check", [], |row| row.get(0))
        .with_context(|| format!("running PRAGMA quick_check on {}", path.display()))?;
    if result != "ok" {
        anyhow::bail!("SQLite quick_check failed for {}: {}", path.display(), result);
    }
    Ok(())
}

fn detect_browser_history_family(path: &Path) -> Result<BrowserHistoryFamily> {
    let connection = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .with_context(|| format!("opening staged database {}", path.display()))?;
    let tables = connection
        .prepare("SELECT name FROM sqlite_master WHERE type = 'table'")?
        .query_map([], |row| row.get::<_, String>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    if has_tables(&tables, &["history_items", "history_visits"]) {
        return Ok(BrowserHistoryFamily::Safari);
    }
    if has_tables(&tables, &["urls", "visits"]) {
        return Ok(BrowserHistoryFamily::Chromium);
    }
    anyhow::bail!(
        "unsupported browser history database {}; expected Safari History.db or Chromium History",
        path.display()
    )
}

fn has_tables(tables: &[String], required: &[&str]) -> bool {
    required.iter().all(|name| tables.iter().any(|table| table == name))
}

fn normalize_requested_family(value: Option<&str>) -> Option<BrowserHistoryFamily> {
    match value.map(|item| item.to_ascii_lowercase()) {
        Some(value) if value == "safari" => Some(BrowserHistoryFamily::Safari),
        Some(value) if value == "chromium" || value == "chrome" => {
            Some(BrowserHistoryFamily::Chromium)
        }
        _ => None,
    }
}

fn stage_chromium_favicons(history_path: &Path, destination_dir: &Path) -> Result<PathBuf> {
    let Some(source_dir) = history_path.parent() else {
        anyhow::bail!("Chromium History path has no parent directory");
    };
    let source = source_dir.join("Favicons");
    if !source.exists() {
        anyhow::bail!("Chromium Favicons database is not present");
    }
    let destination = destination_dir.join("Favicons");
    snapshot_or_copy_sqlite_database(
        &source,
        &destination,
        &BrowserHistoryImportRequest {
            source_path: source.display().to_string(),
            dry_run: true,
            browser_family: Some("chromium".to_string()),
            profile_id: None,
            browser_name: None,
            profile_name: None,
        },
    )?;
    Ok(destination)
}

fn safari_access_hint(request: &BrowserHistoryImportRequest, source: &Path) -> Option<String> {
    let request_family = request.browser_family.as_deref().unwrap_or_default();
    let source_text = source.display().to_string();
    (request_family.eq_ignore_ascii_case("safari")
        || source_text.contains("/Library/Safari/")
        || source.file_name().and_then(|name| name.to_str()) == Some("History.db"))
    .then(|| {
        "Safari History.db is not readable yet. Grant Full Disk Access to PathKeep or the running development process, then retry Browser Direct import.".to_string()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;
    use std::path::PathBuf;

    #[test]
    fn copy_sqlite_database_with_sidecars_preserves_wal_backed_chromium_rows() {
        let source_dir = tempfile::tempdir().expect("source dir");
        let destination_dir = tempfile::tempdir().expect("destination dir");
        let source = source_dir.path().join("History");
        let destination = destination_dir.path().join("History");
        let connection = Connection::open(&source).expect("open source");
        connection.pragma_update(None, "journal_mode", "WAL").expect("enable wal");
        connection.pragma_update(None, "wal_autocheckpoint", 0).expect("disable autocheckpoint");
        connection
            .execute_batch(
                "CREATE TABLE urls (
                   id INTEGER PRIMARY KEY,
                   url TEXT NOT NULL,
                   title TEXT,
                   visit_count INTEGER NOT NULL,
                   typed_count INTEGER NOT NULL,
                   last_visit_time INTEGER NOT NULL,
                   hidden INTEGER NOT NULL
                 );
                 CREATE TABLE visits (
                   id INTEGER PRIMARY KEY,
                   url INTEGER NOT NULL,
                   visit_time INTEGER NOT NULL,
                   from_visit INTEGER,
                   transition INTEGER,
                   visit_duration INTEGER,
                   is_known_to_sync INTEGER,
                   visited_link_id INTEGER,
                   external_referrer_url TEXT,
                   app_id TEXT
                 );
                 INSERT INTO urls (
                   id, url, title, visit_count, typed_count, last_visit_time, hidden
                 )
                 VALUES (1, 'https://example.test/atlas', 'Atlas', 1, 0, 13358534400000000, 0);
                 INSERT INTO visits (
                   id, url, visit_time, from_visit, transition, visit_duration,
                   is_known_to_sync, visited_link_id, external_referrer_url, app_id
                 )
                 VALUES (7, 1, 13358534400000000, NULL, 1, 1000, 0, NULL, NULL, 'atlas');",
            )
            .expect("write wal-backed chromium rows");

        assert!(PathBuf::from(format!("{}-wal", source.display())).exists());

        copy_sqlite_database_with_sidecars(&source, &destination).expect("copy with sidecars");
        quick_check_sqlite(&destination).expect("quick check copied db");
        let copied = Connection::open_with_flags(
            &destination,
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .expect("open copied db");
        let visit_count: i64 = copied
            .query_row("SELECT COUNT(*) FROM visits", [], |row| row.get(0))
            .expect("count copied visits");

        assert_eq!(visit_count, 1);
    }
}
