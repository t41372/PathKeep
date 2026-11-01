//! Responsibilities: answer what a staged browser file says about its visit rows by id: which
//! event a row is now, how far the table's ids have gone, and which rows lost their page.
//! Not responsible for: streaming rows or deciding how a backup reads them.
//!
//! A visit row's id does not identify a visit. Chrome numbers its visits again from 1 when it
//! clears all history or rebuilds a damaged file; Firefox gives a deleted row's id to the next
//! visit. A visit is its page's address and the browser's own time, kept without loss: the time
//! is written once and never changes.
use crate::ParseError;
use rusqlite::{Connection, OpenFlags, OptionalExtension, params_from_iter, types::Value};
use std::path::Path;

/// The browser whose visit table a staged file holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VisitTable {
    Chromium,
    Firefox,
    Safari,
}

/// One visit as a backup identifies it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VisitMark {
    pub url: String,
    /// The browser's time, without loss (`chromium-us:`, `firefox-us:`, `safari-f64:`).
    pub key: String,
    /// The same time in Unix milliseconds, as the archive keeps it.
    pub time_ms: i64,
}

/// Chromium writes microseconds since 1601.
pub fn chromium_key(visit_time: i64) -> String {
    format!("chromium-us:{visit_time}")
}
/// Firefox writes microseconds since 1970.
pub fn firefox_key(visit_date: i64) -> String {
    format!("firefox-us:{visit_date}")
}
/// Safari writes seconds since 2001 as a double; its bits keep times less than a microsecond
/// apart distinct.
pub fn safari_key(visit_time: f64) -> String {
    format!("safari-f64:{:016x}", visit_time.to_bits())
}

impl VisitTable {
    /// Visit table, its page column, the page table, the time column.
    fn names(self) -> (&'static str, &'static str, &'static str, &'static str) {
        match self {
            Self::Chromium => ("visits", "url", "urls", "visit_time"),
            Self::Firefox => ("moz_historyvisits", "place_id", "moz_places", "visit_date"),
            Self::Safari => ("history_visits", "history_item", "history_items", "visit_time"),
        }
    }

    fn mark(
        self,
        url: String,
        row: &rusqlite::Row<'_>,
        column: usize,
    ) -> rusqlite::Result<VisitMark> {
        Ok(match self {
            Self::Chromium => {
                let time = row.get(column)?;
                VisitMark {
                    url,
                    key: chromium_key(time),
                    time_ms: crate::chromium::chrome_time_to_unix_ms(time),
                }
            }
            Self::Firefox => {
                let time = row.get(column)?;
                VisitMark {
                    url,
                    key: firefox_key(time),
                    time_ms: crate::firefox::firefox_time_to_unix_ms(time),
                }
            }
            Self::Safari => {
                let time = row.get(column)?;
                VisitMark {
                    url,
                    key: safari_key(time),
                    time_ms: crate::safari::safari_time_to_unix_ms(time),
                }
            }
        })
    }

    /// The first time, in the browser's unit, of a visit at or after `ms` (Unix milliseconds).
    /// `None` is a time no visit reaches.
    pub(crate) fn time_floor(self, ms: Option<i64>) -> Value {
        match (self, ms) {
            (Self::Chromium, Some(ms)) => {
                Value::Integer(ms.saturating_mul(1_000).saturating_add(CHROME_EPOCH_OFFSET_MICROS))
            }
            (Self::Firefox, Some(ms)) => Value::Integer(ms.saturating_mul(1_000)),
            // The parser rounds Safari's seconds to the nearest millisecond, so a time half a
            // millisecond before one already reads as it.
            (Self::Safari, Some(ms)) => {
                Value::Real((ms as f64 - 0.5) / 1_000.0 - SAFARI_EPOCH_SECONDS)
            }
            (Self::Safari, None) => Value::Real(f64::MAX),
            (_, None) => Value::Integer(i64::MAX),
        }
    }
}

const CHROME_EPOCH_OFFSET_MICROS: i64 = 11_644_473_600_000_000;
const SAFARI_EPOCH_SECONDS: f64 = 978_307_200.0;

/// The visits `ids` name now, in id order; an id the file no longer has is left out.
pub fn visit_marks(
    path: &Path,
    table: VisitTable,
    ids: &[i64],
) -> Result<Vec<(i64, VisitMark)>, ParseError> {
    let connection = open(path)?;
    let (visits, page, pages, time) = table.names();
    let mut marks = Vec::with_capacity(ids.len());
    for chunk in ids.chunks(500) {
        let list = vec!["?"; chunk.len()].join(",");
        let mut statement = connection.prepare(&format!(
            "SELECT {visits}.id,{pages}.url,{visits}.{time} FROM {visits}
             JOIN {pages} ON {pages}.id={visits}.{page}
             WHERE {visits}.id IN ({list}) ORDER BY {visits}.id"
        ))?;
        let rows = statement.query_map(params_from_iter(chunk.iter()), |row| {
            Ok((row.get(0)?, table.mark(row.get(1)?, row, 2)?))
        })?;
        for row in rows {
            marks.push(row?);
        }
    }
    Ok(marks)
}

/// How far the visit table's ids have gone. Chromium and Safari number their visits with
/// AUTOINCREMENT, which never gives an id up to this number again, even after the row with it is
/// deleted. `None` when the file keeps no such record: Firefox's table never does, and Chrome
/// loses it when it creates the table again (clearing all history, rebuilding the file).
pub fn visit_sequence(path: &Path, table: VisitTable) -> Result<Option<i64>, ParseError> {
    let connection = open(path)?;
    let has_sequence = connection
        .query_row(
            "SELECT 1 FROM sqlite_master WHERE type='table' AND name='sqlite_sequence'",
            [],
            |_| Ok(()),
        )
        .optional()?
        .is_some();
    if !has_sequence {
        return Ok(None);
    }
    Ok(connection
        .query_row("SELECT seq FROM sqlite_sequence WHERE name=?1", [table.names().0], |row| {
            row.get::<_, i64>(0)
        })
        .optional()?)
}

/// Whether a row at or below `id` is a visit newer than `newest_ms` (Unix ms). Every row at or
/// below the id a backup stopped at was read by some backup, and browser times never change, so
/// such a row is a new visit given an old id: something wrote it with an explicit id below the
/// table's sequence. Read through the browser's own time index: only rows newer than the newest
/// visit read are looked at. Rows at [`past_latest_time`] are left out, and `newest_ms` must
/// leave them out too.
pub fn newer_rows_at_or_below(
    path: &Path,
    table: VisitTable,
    id: i64,
    newest_ms: i64,
) -> Result<bool, ParseError> {
    let connection = open(path)?;
    let (visits, page, pages, time) = table.names();
    // Only rows a read sees: one whose page row is gone never reaches a backup, so it never
    // moves the newest time a backup read either.
    let found = connection.query_row(
        &format!(
            "SELECT EXISTS(SELECT 1 FROM {visits}
               WHERE {visits}.{time} >= ?1 AND {visits}.{time} < ?3 AND {visits}.id <= ?2
               AND EXISTS (SELECT 1 FROM {pages} WHERE {pages}.id={visits}.{page}))"
        ),
        rusqlite::params![
            table.time_floor(Some(newest_ms.saturating_add(1))),
            id,
            table.time_floor(Some(crate::types::MAX_VALID_UNIX_MS)),
        ],
        |row| row.get::<_, i64>(0),
    )?;
    Ok(found != 0)
}

/// Whether a visit's time (Unix ms, as read) is the latest the parser keeps, year 9999: the
/// browser's own time is that or later, so no real visit's. The archive holds it as that bound,
/// so a row with it would read as newer than the newest visit read, itself included, and every
/// backup would read every row.
pub fn past_latest_time(time_ms: i64) -> bool {
    time_ms >= crate::types::MAX_VALID_UNIX_MS
}

/// Visits a read of `after_id` and `recent_since_ms` covers whose page row is gone. The streams
/// join each visit to its page for the address, so these never reach a backup; it says how many.
pub fn orphan_visits(
    path: &Path,
    table: VisitTable,
    after_id: i64,
    recent_since_ms: Option<i64>,
) -> Result<usize, ParseError> {
    let connection = open(path)?;
    let (visits, page, pages, time) = table.names();
    let count = connection.query_row(
        &format!(
            "SELECT COUNT(*) FROM {visits}
             WHERE ({visits}.id > ?1 OR {visits}.{time} >= ?2)
               AND NOT EXISTS (SELECT 1 FROM {pages} WHERE {pages}.id={visits}.{page})"
        ),
        rusqlite::params![after_id, table.time_floor(recent_since_ms)],
        |row| row.get::<_, i64>(0),
    )?;
    Ok(count.max(0) as usize)
}

fn open(path: &Path) -> Result<Connection, ParseError> {
    Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(|source| ParseError::OpenDatabase { path: path.to_path_buf(), source })
}

#[cfg(test)]
mod tests {
    //! Failure modes: a reused id reads as the visit it held before; a deleted newest row in an
    //! AUTOINCREMENT table reads as a renumbered one; a table created again keeps its old
    //! sequence; a new visit written under an old id goes unseen; Safari times less than a
    //! microsecond apart get one key; an orphan is counted outside the window, or a visit with
    //! its page is.
    use super::*;

    fn chromium(dir: &Path) -> std::path::PathBuf {
        let path = dir.join("History");
        let connection = Connection::open(&path).unwrap();
        connection
            .execute_batch(
                "CREATE TABLE urls(id INTEGER PRIMARY KEY AUTOINCREMENT,url TEXT);
                 CREATE TABLE visits(id INTEGER PRIMARY KEY AUTOINCREMENT,url INTEGER,visit_time INTEGER);
                 INSERT INTO urls(id,url) VALUES(1,'https://a.example/'),(2,'https://b.example/');
                 INSERT INTO visits(id,url,visit_time) VALUES(1,1,13300000000000001),(2,2,13300000000000002),(3,9,13300000000000003);",
            )
            .unwrap();
        path
    }

    #[test]
    fn marks_name_the_event_a_row_holds_now() {
        let dir = tempfile::tempdir().unwrap();
        let path = chromium(dir.path());
        let marks = visit_marks(&path, VisitTable::Chromium, &[2, 1, 7]).unwrap();
        assert_eq!(
            marks,
            vec![
                (
                    1,
                    VisitMark {
                        url: "https://a.example/".into(),
                        key: "chromium-us:13300000000000001".into(),
                        time_ms: crate::chromium::chrome_time_to_unix_ms(13_300_000_000_000_001),
                    }
                ),
                (
                    2,
                    VisitMark {
                        url: "https://b.example/".into(),
                        key: "chromium-us:13300000000000002".into(),
                        time_ms: crate::chromium::chrome_time_to_unix_ms(13_300_000_000_000_002),
                    }
                ),
            ]
        );
    }

    #[test]
    fn a_deleted_newest_row_keeps_its_sequence_and_a_new_table_loses_it() {
        let dir = tempfile::tempdir().unwrap();
        let path = chromium(dir.path());
        Connection::open(&path).unwrap().execute("DELETE FROM visits WHERE id=3", []).unwrap();
        assert_eq!(visit_sequence(&path, VisitTable::Chromium).unwrap(), Some(3));
        Connection::open(&path)
            .unwrap()
            .execute_batch(
                "DROP TABLE visits;
                 CREATE TABLE visits(id INTEGER PRIMARY KEY AUTOINCREMENT,url INTEGER,visit_time INTEGER);",
            )
            .unwrap();
        assert_eq!(visit_sequence(&path, VisitTable::Chromium).unwrap(), None);
    }

    #[test]
    fn orphans_are_counted_only_inside_the_window() {
        let dir = tempfile::tempdir().unwrap();
        let path = chromium(dir.path());
        assert_eq!(orphan_visits(&path, VisitTable::Chromium, 0, None).unwrap(), 1);
        assert_eq!(orphan_visits(&path, VisitTable::Chromium, 3, None).unwrap(), 0);
        // Visit 3 is at 13300000000000003 µs since 1601: inside a window that starts before it.
        let ms = (13_300_000_000_000_003 - CHROME_EPOCH_OFFSET_MICROS) / 1_000;
        assert_eq!(orphan_visits(&path, VisitTable::Chromium, 3, Some(ms)).unwrap(), 1);
    }

    #[test]
    fn a_newer_visit_under_an_old_id_is_found_and_older_ones_are_not() {
        let dir = tempfile::tempdir().unwrap();
        let path = chromium(dir.path());
        let newest = crate::chromium::chrome_time_to_unix_ms(13_300_000_000_000_003);
        assert!(!newer_rows_at_or_below(&path, VisitTable::Chromium, 3, newest).unwrap());
        Connection::open(&path)
            .unwrap()
            .execute("UPDATE visits SET visit_time=13300000000009000 WHERE id=2", [])
            .unwrap();
        assert!(newer_rows_at_or_below(&path, VisitTable::Chromium, 3, newest).unwrap());
        assert!(!newer_rows_at_or_below(&path, VisitTable::Chromium, 1, newest).unwrap());
    }

    #[test]
    fn a_time_past_year_9999_is_not_a_newer_visit() {
        let dir = tempfile::tempdir().unwrap();
        let path = chromium(dir.path());
        let newest = crate::chromium::chrome_time_to_unix_ms(13_300_000_000_000_003);
        // A damaged row: its time is far past what the parser keeps, which reads it as 9999.
        Connection::open(&path)
            .unwrap()
            .execute("UPDATE visits SET visit_time=?1 WHERE id=2", [i64::MAX / 2])
            .unwrap();
        assert!(past_latest_time(crate::chromium::chrome_time_to_unix_ms(i64::MAX / 2)));
        assert!(!past_latest_time(newest));
        assert!(!newer_rows_at_or_below(&path, VisitTable::Chromium, 3, newest).unwrap());
        // A new visit under an old id is still found beside it.
        Connection::open(&path)
            .unwrap()
            .execute("UPDATE visits SET visit_time=13300000000009000 WHERE id=1", [])
            .unwrap();
        assert!(newer_rows_at_or_below(&path, VisitTable::Chromium, 3, newest).unwrap());
    }

    #[test]
    fn a_floor_is_the_first_browser_time_the_parser_reads_as_that_millisecond() {
        // Otherwise a check bounded by it disagrees with the times a backup read: a Safari time
        // in the last half millisecond before year 9999 reads as that bound, is left out of the
        // newest time read, and is still looked at, so every backup read every row.
        for ms in [1_777_680_000_000, crate::types::MAX_VALID_UNIX_MS] {
            let Value::Real(floor) = VisitTable::Safari.time_floor(Some(ms)) else {
                unreachable!("Safari times are seconds")
            };
            assert_eq!(crate::safari::safari_time_to_unix_ms(floor), ms);
            assert_eq!(crate::safari::safari_time_to_unix_ms(floor - 1e-4), ms - 1);
            let Value::Integer(floor) = VisitTable::Chromium.time_floor(Some(ms)) else {
                unreachable!("Chromium times are microseconds")
            };
            assert_eq!(crate::chromium::chrome_time_to_unix_ms(floor), ms);
            assert_eq!(crate::chromium::chrome_time_to_unix_ms(floor - 1), ms - 1);
        }
    }

    #[test]
    fn safari_times_less_than_a_microsecond_apart_keep_two_keys() {
        let a = 800_000_000.000_000_1_f64;
        let b = 800_000_000.000_000_2_f64;
        assert_ne!(a, b);
        assert_ne!(safari_key(a), safari_key(b));
    }
}
