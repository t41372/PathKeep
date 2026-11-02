//! Responsibilities: decide how much of a browser file one backup reads, and finish what the
//! read leaves open: openers it did not reach.
//! Not responsible for: matching rows to visits (`identity`) or writing them (`consumer`).
//! Failure modes: a renumbered file read only after an id it no longer reaches, so every new visit
//! is lost while the run says it succeeded; a value the browser wrote after a visit never read
//! again; an opener outside the read left unnamed.
//!
//! A backup reads the visits after the last row it read, and the last day's visits again, while
//! that row still holds the visit it held: the browser has added rows since, and not numbered
//! them again. Otherwise, and once a week, it reads every row; a read of every row that stopped
//! part way is carried on from its cursor the same way.
use browser_history_parser::native_visits::{self, VisitMark, VisitTable};
use chrono::{DateTime, Duration, Utc};
use std::{collections::HashMap, path::Path};

use super::parser::Watermark;

/// How long after a visit the browser may still change it in ways a backup should keep: Chrome
/// writes a visit's duration when its tab closes. A tab open longer is caught by the weekly read.
const RECENT: Duration = Duration::hours(24);
/// How often every row is read again: durations of tabs left open for days, openers Sync moved,
/// rows marked known to Sync later. Its cost is the browser file's, not the archive's.
const EVERYTHING_EVERY: Duration = Duration::days(7);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ReadPlan {
    /// The rows after the last one read, and the visits since this time (Unix ms) again.
    Recent { since_ms: i64 },
    /// Every row, from the first.
    Everything(Why),
    /// The rest of a read of every row that stopped part way. Its URL phase carries on from its
    /// cursor; its visits are read from the first again, because the openers they were waiting
    /// for were only in memory. Visits already saved are matched, not written. Once it reaches
    /// the end it is a full read.
    Continue,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Why {
    /// The first backup of this profile, the first one after the update that added browser
    /// times, or a replay of a saved copy.
    First,
    /// A week since the last full read.
    Periodic,
    /// The row the last backup stopped at holds another visit now, or is gone in a way only a
    /// renumbering explains.
    Renumbered,
}

impl ReadPlan {
    pub(super) fn recent_since_ms(self) -> Option<i64> {
        match self {
            Self::Recent { since_ms } => Some(since_ms),
            Self::Everything(_) | Self::Continue => None,
        }
    }
    /// The read starts from the first row: the cursors go back to it.
    pub(super) fn starts_full_read(self) -> bool {
        matches!(self, Self::Everything(_))
    }
    /// Reaching the end of this read means every row was read.
    pub(super) fn completes_full_read(self) -> bool {
        matches!(self, Self::Everything(_) | Self::Continue)
    }
}

/// The browser whose visit table a family's files hold.
pub(super) fn table(family: &str) -> Option<VisitTable> {
    match family {
        "chromium" => Some(VisitTable::Chromium),
        "firefox" => Some(VisitTable::Firefox),
        "safari" => Some(VisitTable::Safari),
        _ => None,
    }
}

pub(super) fn plan(
    path: &Path,
    table: VisitTable,
    watermark: &Watermark,
    now: DateTime<Utc>,
) -> anyhow::Result<ReadPlan> {
    let since_ms = (now - RECENT).timestamp_millis();
    if watermark.full_scan_started_at.is_some() {
        return Ok(match still_holds(path, table, watermark)? {
            true => ReadPlan::Continue,
            false => ReadPlan::Everything(Why::Renumbered),
        });
    }
    let last_full = watermark
        .last_full_scan_at
        .as_deref()
        .and_then(|at| DateTime::parse_from_rfc3339(at).ok())
        .map(|at| at.with_timezone(&Utc));
    let (Some(_), Some(_), Some(last_full)) =
        (&watermark.last_visit_key, &watermark.last_visit_url, last_full)
    else {
        return Ok(ReadPlan::Everything(Why::First));
    };
    if watermark.last_visit_id == 0 {
        return Ok(ReadPlan::Everything(Why::First));
    }
    if now - last_full >= EVERYTHING_EVERY {
        return Ok(ReadPlan::Everything(Why::Periodic));
    }
    Ok(match still_holds(path, table, watermark)? {
        true => ReadPlan::Recent { since_ms },
        false => ReadPlan::Everything(Why::Renumbered),
    })
}

/// Whether the rows after the watermark's are still the rows the browser added since: the row
/// it stopped at holds the visit it held then, and no row at or below it is a visit newer than
/// any a backup read (a new visit given an old id). A read that has not reached a visit row yet
/// has nothing to check.
fn still_holds(path: &Path, table: VisitTable, watermark: &Watermark) -> anyhow::Result<bool> {
    let id = watermark.last_visit_id;
    let (Some(key), Some(url)) = (&watermark.last_visit_key, &watermark.last_visit_url) else {
        return Ok(id == 0);
    };
    let same_row = match native_visits::visit_marks(path, table, &[id])?.pop() {
        Some((_, mark)) => &mark.key == key && &mark.url == url,
        // The row was deleted. An AUTOINCREMENT table never gives an id up to its sequence
        // again; without one (Firefox, or a Chrome table created again) the next visit may.
        None => native_visits::visit_sequence(path, table)?.is_some_and(|sequence| sequence >= id),
    };
    Ok(same_row
        && !native_visits::newer_rows_at_or_below(path, table, id, watermark.newest_visit_ms)?)
}

/// The browser's own record of the openers in `unresolved` (archive id of a visit, browser row id
/// of the visit that opened it), for the ones this read did not reach. Rows the file no longer
/// has are left out.
pub(super) fn openers(
    path: &Path,
    table: VisitTable,
    unresolved: &[(i64, i64)],
) -> anyhow::Result<HashMap<i64, VisitMark>> {
    let mut ids = unresolved.iter().map(|(_, opener)| *opener).collect::<Vec<_>>();
    ids.sort_unstable();
    ids.dedup();
    Ok(native_visits::visit_marks(path, table, &ids)?.into_iter().collect())
}

#[cfg(test)]
mod tests {
    //! Failure modes: a renumbered file read from an id it no longer reaches; a deleted newest
    //! row in an AUTOINCREMENT table read as a renumbering (a full read every hour); Firefox's
    //! deleted newest row read as safe; the weekly read never comes; a read of every row that
    //! stopped part way starts over each time (a first backup too large to finish at once never
    //! does), or carries on from a cursor the browser has since renumbered; new visits written
    //! under old ids below the sequence are never read.
    use super::*;
    use rusqlite::Connection;

    fn file(dir: &Path, autoincrement: bool) -> std::path::PathBuf {
        let path = dir.join("History");
        let key =
            if autoincrement { "INTEGER PRIMARY KEY AUTOINCREMENT" } else { "INTEGER PRIMARY KEY" };
        Connection::open(&path)
            .unwrap()
            .execute_batch(&format!(
                "CREATE TABLE urls(id {key},url TEXT);
                 CREATE TABLE visits(id {key},url INTEGER,visit_time INTEGER);
                 INSERT INTO urls(id,url) VALUES(1,'https://a.example/');
                 INSERT INTO visits(id,url,visit_time) VALUES(1,1,13300000000000001),(2,1,13300000000000002);"
            ))
            .unwrap();
        path
    }

    fn after(id: i64, time: i64) -> Watermark {
        Watermark {
            newest_visit_ms: browser_history_parser::chromium::chrome_time_to_unix_ms(time),
            last_visit_id: id,
            last_visit_key: Some(native_visits::chromium_key(time)),
            last_visit_url: Some("https://a.example/".into()),
            last_full_scan_at: Some(Utc::now().to_rfc3339()),
            ..Watermark::default()
        }
    }

    #[test]
    fn the_same_row_reads_recent_and_another_visit_in_it_reads_everything() {
        let dir = tempfile::tempdir().unwrap();
        let path = file(dir.path(), true);
        let now = Utc::now();
        assert!(matches!(
            plan(&path, VisitTable::Chromium, &after(2, 13_300_000_000_000_002), now).unwrap(),
            ReadPlan::Recent { .. }
        ));
        assert_eq!(
            plan(&path, VisitTable::Chromium, &after(2, 13_300_000_000_000_999), now).unwrap(),
            ReadPlan::Everything(Why::Renumbered)
        );
    }

    #[test]
    fn a_deleted_newest_row_is_safe_only_with_a_sequence() {
        let dir = tempfile::tempdir().unwrap();
        let now = Utc::now();
        for (autoincrement, expected) in [(true, true), (false, false)] {
            let folder = dir.path().join(autoincrement.to_string());
            std::fs::create_dir_all(&folder).unwrap();
            let path = file(&folder, autoincrement);
            Connection::open(&path).unwrap().execute("DELETE FROM visits WHERE id=2", []).unwrap();
            let recent = matches!(
                plan(&path, VisitTable::Chromium, &after(2, 13_300_000_000_000_002), now).unwrap(),
                ReadPlan::Recent { .. }
            );
            assert_eq!(recent, expected, "autoincrement {autoincrement}");
        }
    }

    #[test]
    fn a_new_visit_written_under_an_old_id_reads_everything() {
        // Every visit deleted and new ones written from id 1 while the AUTOINCREMENT sequence
        // stays past the old cursor: row 2 is gone and the sequence vouches for it, but row 1 is
        // a visit newer than any read.
        let dir = tempfile::tempdir().unwrap();
        let path = file(dir.path(), true);
        Connection::open(&path)
            .unwrap()
            .execute_batch(
                "DELETE FROM visits; INSERT INTO visits(id,url,visit_time) VALUES(1,1,13300000000009000);",
            )
            .unwrap();
        assert_eq!(
            plan(&path, VisitTable::Chromium, &after(2, 13_300_000_000_000_002), Utc::now())
                .unwrap(),
            ReadPlan::Everything(Why::Renumbered)
        );
    }

    #[test]
    fn a_stopped_full_read_carries_on_unless_its_row_changed() {
        let dir = tempfile::tempdir().unwrap();
        let path = file(dir.path(), true);
        let now = Utc::now();
        let mut stopped = after(2, 13_300_000_000_000_002);
        stopped.last_full_scan_at = None;
        stopped.full_scan_started_at = Some(now.to_rfc3339());
        assert_eq!(plan(&path, VisitTable::Chromium, &stopped, now).unwrap(), ReadPlan::Continue);
        stopped.last_visit_key = Some(native_visits::chromium_key(13_300_000_000_000_999));
        assert_eq!(
            plan(&path, VisitTable::Chromium, &stopped, now).unwrap(),
            ReadPlan::Everything(Why::Renumbered)
        );
        let before_any_visit =
            Watermark { full_scan_started_at: Some(now.to_rfc3339()), ..Watermark::default() };
        assert_eq!(
            plan(&path, VisitTable::Chromium, &before_any_visit, now).unwrap(),
            ReadPlan::Continue
        );
    }

    #[test]
    fn every_row_is_read_once_a_week_and_on_the_first_backup() {
        let dir = tempfile::tempdir().unwrap();
        let path = file(dir.path(), true);
        let now = Utc::now();
        let mut old = after(2, 13_300_000_000_000_002);
        old.last_full_scan_at = Some((now - Duration::days(8)).to_rfc3339());
        assert_eq!(
            plan(&path, VisitTable::Chromium, &old, now).unwrap(),
            ReadPlan::Everything(Why::Periodic)
        );
        assert_eq!(
            plan(&path, VisitTable::Chromium, &Watermark::default(), now).unwrap(),
            ReadPlan::Everything(Why::First)
        );
    }
}
