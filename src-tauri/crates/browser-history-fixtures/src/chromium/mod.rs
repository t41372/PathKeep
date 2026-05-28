//! Real-format Chromium `History` SQLite generator.
//!
//! ## Responsibilities
//! - Emit SQLite files with the `History` and `Favicons` table shapes that
//!   `browser_history_parser::chromium` reads, populated from caller-supplied
//!   record structs.
//! - Keep on-disk column types and value semantics faithful to a real Chrome
//!   `History` file, so scenario tests exercise the same code paths the
//!   production parser hits against a user's actual database.
//!
//! ## Not responsible for
//! - Generating synthetic content (URLs, titles, timestamps) — that belongs
//!   to the scenario layer once it ships. This module is the low-level writer.
//! - Verifying the round-trip parse contract — `tests/chromium_roundtrip.rs`
//!   owns that, since it requires the parser crate as a dev-dependency.
//!
//! ## Performance notes
//! - All rows are written inside a single SQLite transaction; a 1.44M-row
//!   fixture writes in well under the AGENTS.md memory ceiling because we
//!   never materialize the rendered SQL — `rusqlite` prepares once and binds
//!   per row.

use crate::time::unix_ms_to_chrome_time;
use rusqlite::{Connection, params};
use std::path::Path;

/// One row destined for the Chromium `urls` table.
///
/// Fields mirror the columns the production parser reads in
/// `INGEST_URLS_FULL_SQL`. Times are expressed in Unix milliseconds and
/// converted to Chrome epoch on write.
#[derive(Debug, Clone)]
pub struct ChromiumUrlRow {
    /// `urls.id` — Chrome's per-URL primary key. Must be unique within one fixture.
    pub id: i64,
    /// `urls.url` — full URL string, stored exactly as the browser would persist it.
    pub url: String,
    /// `urls.title` — page title, or `None` for pages Chrome never received a title for.
    pub title: Option<String>,
    /// `urls.visit_count` — lifetime visit count Chrome itself tracks.
    pub visit_count: i64,
    /// `urls.typed_count` — how many of those visits were typed into the omnibox.
    pub typed_count: i64,
    /// `urls.last_visit_time` — Unix milliseconds; converted to Chrome epoch at write time.
    pub last_visit_unix_ms: i64,
    /// `urls.hidden` — Chrome's "hidden from suggestions" flag.
    pub hidden: bool,
}

/// One row destined for the Chromium `visits` table.
///
/// Fields mirror the columns the production parser reads in `INGEST_VISITS_SQL`,
/// including the awkwardly-named `visits.url` column which is the foreign key
/// to `urls.id` (not a URL string).
#[derive(Debug, Clone)]
pub struct ChromiumVisitRow {
    /// `visits.id` — visit primary key. Must be unique within one fixture.
    pub id: i64,
    /// `visits.url` — foreign key into the `urls.id` column.
    pub url_id: i64,
    /// `visits.visit_time` — Unix milliseconds; converted to Chrome epoch at write time.
    pub visit_time_unix_ms: i64,
    /// `visits.from_visit` — the visit that linked here, or 0 / `None` for entry points.
    pub from_visit: Option<i64>,
    /// `visits.transition` — Chrome's transition-type bitfield.
    pub transition: Option<i64>,
    /// `visits.visit_duration` — page-engagement duration in microseconds (Chrome's unit).
    pub visit_duration_micros: Option<i64>,
    /// `visits.is_known_to_sync` — whether Chrome Sync has acknowledged this row.
    pub is_known_to_sync: bool,
    /// `visits.visited_link_id` — Chrome's visited-link partition key.
    pub visited_link_id: Option<i64>,
    /// `visits.external_referrer_url` — the off-site referrer header, when Chrome captured one.
    pub external_referrer_url: Option<String>,
    /// `visits.app_id` — Chrome's web-app association string.
    pub app_id: Option<String>,
}

/// One row destined for the Chromium `downloads` table.
///
/// Times are expressed in Unix milliseconds and converted to Chrome epoch on
/// write. Nullable columns stay nullable so scenarios can pin partial-download
/// rows without inventing synthetic values.
#[derive(Debug, Clone)]
pub struct ChromiumDownloadRow {
    /// `downloads.id` — Chrome's per-download primary key.
    pub id: i64,
    /// `downloads.guid` — browser-generated stable download GUID.
    pub guid: Option<String>,
    /// `downloads.current_path` — active or partial file path.
    pub current_path: Option<String>,
    /// `downloads.target_path` — final target path requested by the user.
    pub target_path: Option<String>,
    /// `downloads.start_time` — Unix milliseconds; converted to Chrome epoch.
    pub start_time_unix_ms: Option<i64>,
    /// `downloads.received_bytes` — bytes already written.
    pub received_bytes: Option<i64>,
    /// `downloads.total_bytes` — expected total size when known.
    pub total_bytes: Option<i64>,
    /// `downloads.state` — Chrome download state enum value.
    pub state: Option<i64>,
    /// `downloads.mime_type` — MIME type Chrome observed.
    pub mime_type: Option<String>,
    /// `downloads.original_mime_type` — original server MIME type when present.
    pub original_mime_type: Option<String>,
}

/// One row destined for the Chromium `keyword_search_terms` table.
#[derive(Debug, Clone)]
pub struct ChromiumKeywordSearchTermRow {
    /// `keyword_search_terms.keyword_id` — Chrome's search-engine keyword id.
    pub keyword_id: i64,
    /// `keyword_search_terms.url_id` — foreign key into `urls.id`.
    pub url_id: i64,
    /// `keyword_search_terms.term` — original query text.
    pub term: String,
    /// `keyword_search_terms.normalized_term` — Chrome's normalized query text.
    pub normalized_term: String,
}

/// One icon row plus a single bitmap destined for Chromium's `Favicons` DB.
#[derive(Debug, Clone)]
pub struct ChromiumFaviconRow {
    /// `favicons.id` and `favicon_bitmaps.icon_id`.
    pub id: i64,
    /// `favicons.url` — the icon resource URL.
    pub icon_url: String,
    /// `favicons.icon_type` — Chrome icon type enum value.
    pub icon_type: Option<i64>,
    /// `favicon_bitmaps.width`.
    pub width: i64,
    /// `favicon_bitmaps.height`.
    pub height: i64,
    /// `favicon_bitmaps.last_updated` — Unix milliseconds; converted to Chrome epoch.
    pub last_updated_unix_ms: i64,
    /// `favicon_bitmaps.image_data` — synthetic icon bytes.
    pub image_data: Option<Vec<u8>>,
}

/// One row destined for Chromium's `icon_mapping` table.
#[derive(Debug, Clone)]
pub struct ChromiumIconMappingRow {
    /// `icon_mapping.page_url` — the page URL that should receive an icon.
    pub page_url: String,
    /// `icon_mapping.icon_id` — foreign key into `favicons.id`.
    pub icon_id: i64,
}

/// Builder for one Chromium `History` SQLite fixture.
///
/// Use [`ChromiumHistoryFixture::new`] then the `add_*` methods to compose
/// records. [`Self::write`] materializes the `History` database;
/// [`Self::write_favicons`] materializes the companion `Favicons` database.
#[derive(Debug, Default)]
pub struct ChromiumHistoryFixture {
    urls: Vec<ChromiumUrlRow>,
    visits: Vec<ChromiumVisitRow>,
    downloads: Vec<ChromiumDownloadRow>,
    search_terms: Vec<ChromiumKeywordSearchTermRow>,
    favicons: Vec<ChromiumFaviconRow>,
    icon_mappings: Vec<ChromiumIconMappingRow>,
}

impl ChromiumHistoryFixture {
    /// Creates an empty fixture builder.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds one URL row to the fixture. Returns the builder for chaining.
    pub fn add_url(mut self, url: ChromiumUrlRow) -> Self {
        self.urls.push(url);
        self
    }

    /// Adds one visit row to the fixture. Returns the builder for chaining.
    pub fn add_visit(mut self, visit: ChromiumVisitRow) -> Self {
        self.visits.push(visit);
        self
    }

    /// Adds one download row to the fixture. Returns the builder for chaining.
    pub fn add_download(mut self, download: ChromiumDownloadRow) -> Self {
        self.downloads.push(download);
        self
    }

    /// Adds one keyword-search-term row to the fixture. Returns the builder for chaining.
    pub fn add_search_term(mut self, search_term: ChromiumKeywordSearchTermRow) -> Self {
        self.search_terms.push(search_term);
        self
    }

    /// Adds one favicon row + bitmap to the companion Favicons fixture.
    pub fn add_favicon(mut self, favicon: ChromiumFaviconRow) -> Self {
        self.favicons.push(favicon);
        self
    }

    /// Adds one icon mapping row to the companion Favicons fixture.
    pub fn add_icon_mapping(mut self, icon_mapping: ChromiumIconMappingRow) -> Self {
        self.icon_mappings.push(icon_mapping);
        self
    }

    /// Materializes the fixture as a real-format SQLite file at `path`.
    ///
    /// Overwrites any existing file at the same path. Callers using the
    /// `tempfile` crate get the standard `TempDir::path().join("History")`
    /// pattern; the file name is conventional but not enforced here, since
    /// PathKeep's parser accepts any path it's given.
    pub fn write(&self, path: &Path) -> Result<(), rusqlite::Error> {
        if path.exists() {
            std::fs::remove_file(path)
                .map_err(|err| rusqlite::Error::ToSqlConversionFailure(Box::new(err)))?;
        }

        let mut connection = Connection::open(path)?;
        let transaction = connection.transaction()?;

        transaction.execute_batch(SCHEMA_SQL)?;

        {
            let mut url_stmt = transaction.prepare(
                "INSERT INTO urls (id, url, title, visit_count, typed_count, last_visit_time, hidden)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            )?;
            for url in &self.urls {
                url_stmt.execute(params![
                    url.id,
                    url.url,
                    url.title,
                    url.visit_count,
                    url.typed_count,
                    unix_ms_to_chrome_time(url.last_visit_unix_ms),
                    url.hidden as i64,
                ])?;
            }
        }

        {
            let mut visit_stmt = transaction.prepare(
                "INSERT INTO visits (
                    id, url, visit_time, from_visit, transition, visit_duration,
                    is_known_to_sync, visited_link_id, external_referrer_url, app_id
                 )
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            )?;
            for visit in &self.visits {
                visit_stmt.execute(params![
                    visit.id,
                    visit.url_id,
                    unix_ms_to_chrome_time(visit.visit_time_unix_ms),
                    visit.from_visit,
                    visit.transition,
                    visit.visit_duration_micros,
                    visit.is_known_to_sync as i64,
                    visit.visited_link_id,
                    visit.external_referrer_url,
                    visit.app_id,
                ])?;
            }
        }

        {
            let mut download_stmt = transaction.prepare(
                "INSERT INTO downloads (
                    id, guid, current_path, target_path, start_time, received_bytes,
                    total_bytes, state, mime_type, original_mime_type
                 )
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            )?;
            for download in &self.downloads {
                download_stmt.execute(params![
                    download.id,
                    download.guid,
                    download.current_path,
                    download.target_path,
                    download.start_time_unix_ms.map(unix_ms_to_chrome_time),
                    download.received_bytes,
                    download.total_bytes,
                    download.state,
                    download.mime_type,
                    download.original_mime_type,
                ])?;
            }
        }

        {
            let mut search_stmt = transaction.prepare(
                "INSERT INTO keyword_search_terms (keyword_id, url_id, term, normalized_term)
                 VALUES (?1, ?2, ?3, ?4)",
            )?;
            for search_term in &self.search_terms {
                search_stmt.execute(params![
                    search_term.keyword_id,
                    search_term.url_id,
                    search_term.term,
                    search_term.normalized_term,
                ])?;
            }
        }

        transaction.commit()?;
        Ok(())
    }

    /// Materializes the companion Chromium `Favicons` SQLite file at `path`.
    ///
    /// The parser reads favicons from a separate database, matching real Chrome
    /// profile layout. Callers that do not need favicon coverage can skip this
    /// method and pass `None` as the parser's favicons path.
    pub fn write_favicons(&self, path: &Path) -> Result<(), rusqlite::Error> {
        if path.exists() {
            std::fs::remove_file(path)
                .map_err(|err| rusqlite::Error::ToSqlConversionFailure(Box::new(err)))?;
        }

        let mut connection = Connection::open(path)?;
        let transaction = connection.transaction()?;

        transaction.execute_batch(FAVICONS_SCHEMA_SQL)?;

        {
            let mut favicon_stmt = transaction.prepare(
                "INSERT INTO favicons (id, url, icon_type)
                 VALUES (?1, ?2, ?3)",
            )?;
            let mut bitmap_stmt = transaction.prepare(
                "INSERT INTO favicon_bitmaps (icon_id, width, height, last_updated, image_data)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
            )?;
            for favicon in &self.favicons {
                favicon_stmt.execute(params![favicon.id, favicon.icon_url, favicon.icon_type])?;
                bitmap_stmt.execute(params![
                    favicon.id,
                    favicon.width,
                    favicon.height,
                    unix_ms_to_chrome_time(favicon.last_updated_unix_ms),
                    favicon.image_data,
                ])?;
            }
        }

        {
            let mut mapping_stmt = transaction.prepare(
                "INSERT INTO icon_mapping (page_url, icon_id)
                 VALUES (?1, ?2)",
            )?;
            for mapping in &self.icon_mappings {
                mapping_stmt.execute(params![mapping.page_url, mapping.icon_id])?;
            }
        }

        transaction.commit()?;
        Ok(())
    }
}

/// SQLite schema matching the columns the PathKeep Chromium parser reads.
///
/// Real Chrome `History` files carry many more columns (favicon_id on
/// `urls`; sync metadata, segment_id, opener_visit, originator_* fields on
/// `visits`; dozens of download danger/interrupt fields). Those are
/// intentionally omitted here because the parser does not project them; adding
/// them would invite drift between fixture and parser without buying behavior
/// coverage.
const SCHEMA_SQL: &str = r#"
CREATE TABLE urls (
  id INTEGER PRIMARY KEY,
  url TEXT NOT NULL,
  title TEXT,
  visit_count INTEGER NOT NULL DEFAULT 0,
  typed_count INTEGER NOT NULL DEFAULT 0,
  last_visit_time INTEGER NOT NULL DEFAULT 0,
  hidden INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE visits (
  id INTEGER PRIMARY KEY,
  url INTEGER NOT NULL,
  visit_time INTEGER NOT NULL DEFAULT 0,
  from_visit INTEGER,
  transition INTEGER,
  visit_duration INTEGER,
  is_known_to_sync INTEGER NOT NULL DEFAULT 0,
  visited_link_id INTEGER,
  external_referrer_url TEXT,
  app_id TEXT
);

CREATE TABLE downloads (
  id INTEGER PRIMARY KEY,
  guid TEXT,
  current_path TEXT,
  target_path TEXT,
  start_time INTEGER,
  received_bytes INTEGER,
  total_bytes INTEGER,
  state INTEGER,
  mime_type TEXT,
  original_mime_type TEXT
);

CREATE TABLE keyword_search_terms (
  keyword_id INTEGER,
  url_id INTEGER,
  term TEXT NOT NULL,
  normalized_term TEXT NOT NULL
);

CREATE INDEX urls_url_index ON urls(url);
CREATE INDEX visits_url_index ON visits(url);
CREATE INDEX visits_time_index ON visits(visit_time);
CREATE INDEX downloads_id_index ON downloads(id);
CREATE INDEX keyword_search_terms_url_index ON keyword_search_terms(url_id);
"#;

/// SQLite schema matching the companion Chromium `Favicons` database columns
/// the PathKeep parser reads.
const FAVICONS_SCHEMA_SQL: &str = r#"
CREATE TABLE favicons (
  id INTEGER PRIMARY KEY,
  url TEXT NOT NULL,
  icon_type INTEGER
);

CREATE TABLE icon_mapping (
  page_url TEXT NOT NULL,
  icon_id INTEGER NOT NULL
);

CREATE TABLE favicon_bitmaps (
  icon_id INTEGER NOT NULL,
  width INTEGER,
  height INTEGER,
  last_updated INTEGER,
  image_data BLOB
);

CREATE INDEX icon_mapping_page_url_idx ON icon_mapping(page_url);
CREATE INDEX icon_mapping_icon_id_idx ON icon_mapping(icon_id);
CREATE INDEX favicon_bitmaps_icon_id_idx ON favicon_bitmaps(icon_id);
"#;

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn write_overwrites_existing_file_at_same_path() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("History");
        let fixture = ChromiumHistoryFixture::new()
            .add_url(ChromiumUrlRow {
                id: 1,
                url: "https://a.test".to_string(),
                title: Some("A".to_string()),
                visit_count: 1,
                typed_count: 0,
                last_visit_unix_ms: 1_700_000_000_000,
                hidden: false,
            })
            .add_visit(ChromiumVisitRow {
                id: 1,
                url_id: 1,
                visit_time_unix_ms: 1_700_000_000_000,
                from_visit: None,
                transition: Some(1),
                visit_duration_micros: None,
                is_known_to_sync: false,
                visited_link_id: None,
                external_referrer_url: None,
                app_id: None,
            });
        fixture.write(&path).unwrap();
        assert!(path.exists());
        fixture.write(&path).unwrap();
        assert!(path.exists());
    }

    #[test]
    fn write_favicons_overwrites_existing_file_with_companion_schema() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("Favicons");
        std::fs::write(&path, b"not sqlite").unwrap();

        let icon_bytes = vec![0x89, b'P', b'N', b'G'];
        let fixture = ChromiumHistoryFixture::new()
            .add_favicon(ChromiumFaviconRow {
                id: 9,
                icon_url: "https://a.test/favicon.png".to_string(),
                icon_type: Some(1),
                width: 32,
                height: 32,
                last_updated_unix_ms: 1_700_000_000_000,
                image_data: Some(icon_bytes.clone()),
            })
            .add_icon_mapping(ChromiumIconMappingRow {
                page_url: "https://a.test/page".to_string(),
                icon_id: 9,
            });

        fixture.write_favicons(&path).unwrap();

        {
            let connection = Connection::open(&path).unwrap();
            let favicon_url: String = connection
                .query_row("SELECT url FROM favicons WHERE id = 9", [], |row| row.get(0))
                .unwrap();
            assert_eq!(favicon_url, "https://a.test/favicon.png");

            let mapped_icon_id: i64 = connection
                .query_row(
                    "SELECT icon_id FROM icon_mapping WHERE page_url = 'https://a.test/page'",
                    [],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(mapped_icon_id, 9);

            let stored_bytes: Option<Vec<u8>> = connection
                .query_row("SELECT image_data FROM favicon_bitmaps WHERE icon_id = 9", [], |row| {
                    row.get(0)
                })
                .unwrap();
            assert_eq!(stored_bytes, Some(icon_bytes));
        }

        fixture.write_favicons(&path).unwrap();

        let connection = Connection::open(&path).unwrap();
        let favicon_count: i64 =
            connection.query_row("SELECT COUNT(*) FROM favicons", [], |row| row.get(0)).unwrap();
        assert_eq!(favicon_count, 1);
    }
}
