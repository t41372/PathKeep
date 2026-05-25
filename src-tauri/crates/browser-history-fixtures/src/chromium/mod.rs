//! Real-format Chromium `History` SQLite generator.
//!
//! ## Responsibilities
//! - Emit a SQLite file with the `urls` and `visits` table shapes that
//!   `browser_history_parser::chromium` reads, populated from caller-supplied
//!   record structs.
//! - Keep on-disk column types and value semantics faithful to a real Chrome
//!   `History` file, so scenario tests exercise the same code paths the
//!   production parser hits against a user's actual database.
//!
//! ## Not responsible for
//! - Generating synthetic content (URLs, titles, timestamps) — that belongs
//!   to the scenario layer once it ships. This module is the low-level writer.
//! - Downloads / favicons / keyword search terms — separate writers will be
//!   added when scenarios that exercise those tables come online.
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

/// Builder for one Chromium `History` SQLite fixture.
///
/// Use [`ChromiumHistoryFixture::new`] then [`Self::add_url`] / [`Self::add_visit`]
/// to compose records, and [`Self::write`] to materialize the SQLite file.
#[derive(Debug, Default)]
pub struct ChromiumHistoryFixture {
    urls: Vec<ChromiumUrlRow>,
    visits: Vec<ChromiumVisitRow>,
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

        transaction.commit()?;
        Ok(())
    }
}

/// SQLite schema matching the columns the PathKeep Chromium parser reads.
///
/// Real Chrome `History` files carry many more columns (favicon_id on
/// `urls`; sync metadata, segment_id, opener_visit, originator_* fields on
/// `visits`). Those are intentionally omitted here because the parser does
/// not project them; adding them would invite drift between fixture and
/// reality without buying any extra coverage. Slices that need favicon or
/// sync coverage will extend this schema in their own writer.
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

CREATE INDEX urls_url_index ON urls(url);
CREATE INDEX visits_url_index ON visits(url);
CREATE INDEX visits_time_index ON visits(visit_time);
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
}
