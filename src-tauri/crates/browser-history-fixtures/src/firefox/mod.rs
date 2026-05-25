//! Real-format Firefox `places.sqlite` generator.
//!
//! ## Responsibilities
//! - Emit a SQLite file with the `moz_places` / `moz_historyvisits` shape
//!   that `browser_history_parser::firefox` reads, populated from caller-
//!   supplied record structs.
//! - Convert fixture-author-friendly Unix milliseconds into Firefox's native
//!   `i64` microseconds-since-Unix-epoch on write.
//!
//! ## Not responsible for
//! - The optional `moz_inputhistory` / `moz_places_metadata*` sidecar tables;
//!   those are added when scenarios exercise typed-evidence extraction.
//! - Synthesizing realistic content. Scenario builders compose these records.
//!
//! ## Performance notes
//! - Single-transaction write. Bound by SQLite throughput, not Rust overhead.

use rusqlite::{Connection, params};
use std::path::Path;

/// One row destined for the Firefox `moz_places` table.
#[derive(Debug, Clone)]
pub struct FirefoxPlaceRow {
    /// `moz_places.id` — Firefox's per-URL primary key (`place_id`).
    pub id: i64,
    /// `moz_places.url` — full URL.
    pub url: String,
    /// `moz_places.title` — page title, or `None` for pages without one.
    pub title: Option<String>,
    /// `moz_places.visit_count` — Firefox's lifetime visit count.
    pub visit_count: i64,
    /// `moz_places.hidden` — whether the URL is hidden from suggestion lists.
    pub hidden: bool,
    /// `moz_places.last_visit_date` — Unix milliseconds; converted to μs at write time.
    pub last_visit_unix_ms: i64,
}

/// One row destined for the Firefox `moz_historyvisits` table.
#[derive(Debug, Clone)]
pub struct FirefoxVisitRow {
    /// `moz_historyvisits.id` — visit primary key.
    pub id: i64,
    /// `moz_historyvisits.place_id` — foreign key into `moz_places.id`.
    pub place_id: i64,
    /// `moz_historyvisits.visit_date` — Unix milliseconds; converted to μs at write time.
    pub visit_time_unix_ms: i64,
    /// `moz_historyvisits.from_visit` — the visit that linked here, or `None`.
    pub from_visit: Option<i64>,
    /// `moz_historyvisits.visit_type` — Firefox's transition-type enum.
    pub visit_type: Option<i64>,
}

/// Builder for one Firefox `places.sqlite` fixture.
#[derive(Debug, Default)]
pub struct FirefoxPlacesFixture {
    places: Vec<FirefoxPlaceRow>,
    visits: Vec<FirefoxVisitRow>,
}

impl FirefoxPlacesFixture {
    /// Creates an empty fixture builder.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds one place row to the fixture.
    pub fn add_place(mut self, place: FirefoxPlaceRow) -> Self {
        self.places.push(place);
        self
    }

    /// Adds one visit row to the fixture.
    pub fn add_visit(mut self, visit: FirefoxVisitRow) -> Self {
        self.visits.push(visit);
        self
    }

    /// Materializes the fixture as a real-format SQLite file at `path`.
    pub fn write(&self, path: &Path) -> Result<(), rusqlite::Error> {
        if path.exists() {
            std::fs::remove_file(path)
                .map_err(|err| rusqlite::Error::ToSqlConversionFailure(Box::new(err)))?;
        }

        let mut connection = Connection::open(path)?;
        let transaction = connection.transaction()?;

        transaction.execute_batch(SCHEMA_SQL)?;

        {
            let mut place_stmt = transaction.prepare(
                "INSERT INTO moz_places (id, url, title, visit_count, hidden, last_visit_date)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            )?;
            for place in &self.places {
                place_stmt.execute(params![
                    place.id,
                    place.url,
                    place.title,
                    place.visit_count,
                    place.hidden as i64,
                    unix_ms_to_firefox_time(place.last_visit_unix_ms),
                ])?;
            }
        }

        {
            let mut visit_stmt = transaction.prepare(
                "INSERT INTO moz_historyvisits (id, place_id, visit_date, from_visit, visit_type)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
            )?;
            for visit in &self.visits {
                visit_stmt.execute(params![
                    visit.id,
                    visit.place_id,
                    unix_ms_to_firefox_time(visit.visit_time_unix_ms),
                    visit.from_visit,
                    visit.visit_type,
                ])?;
            }
        }

        transaction.commit()?;
        Ok(())
    }
}

/// Converts Unix milliseconds into Firefox's microseconds-since-Unix-epoch.
///
/// Mirrors `browser_history_parser::firefox::unix_ms_to_firefox_time`. Keeping
/// a local copy here avoids a runtime dependency on the parser crate.
pub fn unix_ms_to_firefox_time(unix_ms: i64) -> i64 {
    unix_ms.max(0).saturating_mul(1_000)
}

/// Inverse of [`unix_ms_to_firefox_time`].
pub fn firefox_time_to_unix_ms(firefox_micros: i64) -> i64 {
    firefox_micros.div_euclid(1_000).max(0)
}

/// Minimum schema the production Firefox parser reads.
///
/// Real Firefox `places.sqlite` files carry many more tables (bookmarks,
/// keywords, metadata, input history, search queries). Scenarios that need
/// those tables will extend the schema in a dedicated writer slice.
const SCHEMA_SQL: &str = r#"
CREATE TABLE moz_places (
  id INTEGER PRIMARY KEY,
  url TEXT NOT NULL,
  title TEXT,
  visit_count INTEGER,
  hidden INTEGER,
  last_visit_date INTEGER
);

CREATE TABLE moz_historyvisits (
  id INTEGER PRIMARY KEY,
  place_id INTEGER NOT NULL,
  visit_date INTEGER NOT NULL,
  from_visit INTEGER,
  visit_type INTEGER
);

CREATE INDEX moz_places_url_index ON moz_places(url);
CREATE INDEX moz_historyvisits_place_index ON moz_historyvisits(place_id);
CREATE INDEX moz_historyvisits_date_index ON moz_historyvisits(visit_date);
"#;

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn write_overwrites_existing_file_at_same_path() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("places.sqlite");
        let fixture = FirefoxPlacesFixture::new()
            .add_place(FirefoxPlaceRow {
                id: 1,
                url: "https://a.test".to_string(),
                title: Some("A".to_string()),
                visit_count: 1,
                hidden: false,
                last_visit_unix_ms: 1_700_000_000_000,
            })
            .add_visit(FirefoxVisitRow {
                id: 1,
                place_id: 1,
                visit_time_unix_ms: 1_700_000_000_000,
                from_visit: None,
                visit_type: Some(1),
            });
        fixture.write(&path).unwrap();
        assert!(path.exists());
        fixture.write(&path).unwrap();
        assert!(path.exists());
    }
}
