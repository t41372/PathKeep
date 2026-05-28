//! Real-format Safari `History.db` generator.
//!
//! ## Responsibilities
//! - Emit a SQLite file with the `history_items` / `history_visits` shape
//!   `browser_history_parser::safari` reads.
//! - Support both the minimal historical schema (just `visit_time`) and the
//!   current macOS Safari schema with `load_successful`, `synthesized`,
//!   `redirect_*`, `origin`, `score`, etc. — selectable per fixture.
//! - Convert fixture-author Unix milliseconds into Safari's CFAbsoluteTime
//!   `f64` (seconds since 2001-01-01).
//!
//! ## Not responsible for
//! - The `history_tombstones` table; scenarios that exercise sync-deletion
//!   semantics will extend this writer.
//! - Synthesizing realistic content; scenario builders compose records.

use rusqlite::{Connection, params};
use std::path::Path;

const SAFARI_UNIX_EPOCH_OFFSET_SECONDS: f64 = 978_307_200.0;

/// Which Safari schema variant the writer should produce.
///
/// Real macOS Safari ships the `Current` schema today; the `Minimal` variant
/// covers older OS versions and the legacy parser-test fixture path.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SafariSchemaVariant {
    /// Minimal `history_visits` columns: only `id`, `history_item`, `title`, `visit_time`.
    Minimal,
    /// Current macOS Safari schema: adds `load_successful`, `synthesized`,
    /// `redirect_*`, `origin`, `generation`, `attributes`, `score`.
    #[default]
    Current,
}

/// One row destined for the Safari `history_items` table.
#[derive(Debug, Clone)]
pub struct SafariHistoryItemRow {
    /// `history_items.id` — Safari's per-URL primary key.
    pub id: i64,
    /// `history_items.url` — full URL.
    pub url: String,
}

/// One row destined for the Safari `history_visits` table.
#[derive(Debug, Clone)]
pub struct SafariHistoryVisitRow {
    /// `history_visits.id` — visit primary key.
    pub id: i64,
    /// `history_visits.history_item` — foreign key to `history_items.id`.
    pub history_item: i64,
    /// `history_visits.title` — Safari attaches title at the visit level, not the URL.
    pub title: Option<String>,
    /// `history_visits.visit_time` — Unix milliseconds; converted to CFAbsoluteTime at write.
    pub visit_time_unix_ms: i64,
    /// `history_visits.load_successful` — whether the page loaded without error.
    pub load_successful: Option<bool>,
    /// `history_visits.http_non_get` — whether the request used a non-GET method.
    pub http_non_get: Option<bool>,
    /// `history_visits.synthesized` — whether Safari generated this row as a side-effect of a redirect or similar.
    pub synthesized: Option<bool>,
    /// `history_visits.redirect_source` — the visit id that redirected here.
    pub redirect_source: Option<i64>,
    /// `history_visits.redirect_destination` — the visit id this redirected to.
    pub redirect_destination: Option<i64>,
    /// `history_visits.origin` — Safari's load-origin enum.
    pub origin: Option<i64>,
    /// `history_visits.generation` — Safari's content-generation counter.
    pub generation: Option<i64>,
    /// `history_visits.attributes` — Safari's per-visit attribute bitfield.
    pub attributes: Option<i64>,
    /// `history_visits.score` — Safari's relevance score.
    pub score: Option<f64>,
}

/// Builder for one Safari `History.db` fixture.
#[derive(Debug, Default)]
pub struct SafariHistoryFixture {
    variant: SafariSchemaVariant,
    items: Vec<SafariHistoryItemRow>,
    visits: Vec<SafariHistoryVisitRow>,
}

impl SafariHistoryFixture {
    /// Creates an empty builder using the current macOS Safari schema variant.
    pub fn new() -> Self {
        Self::default()
    }

    /// Switches the writer to the minimal historical schema (for legacy testing).
    pub fn with_variant(mut self, variant: SafariSchemaVariant) -> Self {
        self.variant = variant;
        self
    }

    /// Adds one history item row.
    pub fn add_item(mut self, item: SafariHistoryItemRow) -> Self {
        self.items.push(item);
        self
    }

    /// Adds one history visit row.
    pub fn add_visit(mut self, visit: SafariHistoryVisitRow) -> Self {
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

        transaction.execute_batch(match self.variant {
            SafariSchemaVariant::Minimal => SCHEMA_MINIMAL_SQL,
            SafariSchemaVariant::Current => SCHEMA_CURRENT_SQL,
        })?;

        {
            let mut item_stmt =
                transaction.prepare("INSERT INTO history_items (id, url) VALUES (?1, ?2)")?;
            for item in &self.items {
                item_stmt.execute(params![item.id, item.url])?;
            }
        }

        match self.variant {
            SafariSchemaVariant::Minimal => {
                let mut visit_stmt = transaction.prepare(
                    "INSERT INTO history_visits (id, history_item, title, visit_time)
                     VALUES (?1, ?2, ?3, ?4)",
                )?;
                for visit in &self.visits {
                    visit_stmt.execute(params![
                        visit.id,
                        visit.history_item,
                        visit.title,
                        unix_ms_to_safari_time(visit.visit_time_unix_ms),
                    ])?;
                }
            }
            SafariSchemaVariant::Current => {
                let mut visit_stmt = transaction.prepare(
                    "INSERT INTO history_visits (
                        id, history_item, title, visit_time, load_successful,
                        http_non_get, synthesized, redirect_source, redirect_destination,
                        origin, generation, attributes, score
                     )
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
                )?;
                for visit in &self.visits {
                    visit_stmt.execute(params![
                        visit.id,
                        visit.history_item,
                        visit.title,
                        unix_ms_to_safari_time(visit.visit_time_unix_ms),
                        visit.load_successful.map(|flag| flag as i64),
                        visit.http_non_get.map(|flag| flag as i64),
                        visit.synthesized.map(|flag| flag as i64),
                        visit.redirect_source,
                        visit.redirect_destination,
                        visit.origin,
                        visit.generation,
                        visit.attributes,
                        visit.score,
                    ])?;
                }
            }
        }

        transaction.commit()?;
        Ok(())
    }
}

/// Converts Unix milliseconds into Safari's CFAbsoluteTime (seconds since 2001-01-01).
pub fn unix_ms_to_safari_time(unix_ms: i64) -> f64 {
    (unix_ms.max(0) as f64 / 1_000.0) - SAFARI_UNIX_EPOCH_OFFSET_SECONDS
}

/// Inverse of [`unix_ms_to_safari_time`], rounding to the nearest millisecond.
pub fn safari_time_to_unix_ms(safari_seconds: f64) -> i64 {
    (((safari_seconds + SAFARI_UNIX_EPOCH_OFFSET_SECONDS) * 1_000.0).round() as i64).max(0)
}

const SCHEMA_MINIMAL_SQL: &str = r#"
CREATE TABLE history_items (
  id INTEGER PRIMARY KEY,
  url TEXT NOT NULL
);

CREATE TABLE history_visits (
  id INTEGER PRIMARY KEY,
  history_item INTEGER NOT NULL,
  title TEXT,
  visit_time REAL NOT NULL
);

CREATE INDEX history_visits_item_index ON history_visits(history_item);
CREATE INDEX history_visits_time_index ON history_visits(visit_time);
"#;

const SCHEMA_CURRENT_SQL: &str = r#"
CREATE TABLE history_items (
  id INTEGER PRIMARY KEY,
  url TEXT NOT NULL
);

CREATE TABLE history_visits (
  id INTEGER PRIMARY KEY,
  history_item INTEGER NOT NULL,
  title TEXT,
  visit_time REAL NOT NULL,
  load_successful INTEGER,
  http_non_get INTEGER,
  synthesized INTEGER,
  redirect_source INTEGER,
  redirect_destination INTEGER,
  origin INTEGER,
  generation INTEGER,
  attributes INTEGER,
  score REAL
);

CREATE INDEX history_visits_item_index ON history_visits(history_item);
CREATE INDEX history_visits_time_index ON history_visits(visit_time);
"#;

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn write_overwrites_existing_file_at_same_path() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("History.db");
        let fixture = SafariHistoryFixture::new()
            .add_item(SafariHistoryItemRow { id: 1, url: "https://a.test".to_string() })
            .add_visit(SafariHistoryVisitRow {
                id: 1,
                history_item: 1,
                title: Some("A".to_string()),
                visit_time_unix_ms: 1_700_000_000_000,
                load_successful: None,
                http_non_get: None,
                synthesized: None,
                redirect_source: None,
                redirect_destination: None,
                origin: None,
                generation: None,
                attributes: None,
                score: None,
            });
        fixture.write(&path).unwrap();
        assert!(path.exists());
        fixture.write(&path).unwrap();
        assert!(path.exists());
    }
}
