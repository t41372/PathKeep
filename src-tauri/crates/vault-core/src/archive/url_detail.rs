//! Per-URL detail for the History panel.
//!
//! ## Responsibilities
//! - Totals, first/last visit, recording browsers and a 12-week visit histogram for one exact URL.
//!
//! ## Not responsible for
//! - Fuzzy URL matching or normalization; the URL must match a stored `urls.url` exactly.
//!
//! ## Performance notes
//! - `idx_urls_url` finds the URL rows (one per source profile). Every visit query then runs on
//!   `idx_visits_visible_url_time (url_id, visit_time_ms DESC, id DESC)`, which covers the count,
//!   first/last seek and each weekly range, so no visit row is read even for a URL with 100k visits.

use super::source_stats::rfc3339_from_ms;
use crate::{
    chrome::browser_display_name,
    config::ProjectPaths,
    models::{AppConfig, UrlDetail, UrlWeeklyVisits},
    utils::url_domain,
};
use anyhow::{Context, Result};
use chrono::{Datelike, Duration, Local, NaiveDate, TimeZone};
use rusqlite::{Connection, OptionalExtension, params};
use std::collections::HashMap;

const WEEKS: i64 = 12;

/// Loads the detail panel data for `url` from the canonical archive.
pub fn get_url_detail(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    url: &str,
) -> Result<UrlDetail> {
    let connection = super::open_archive_connection(paths, config, key)?;
    url_detail_for_connection(&connection, url, Local::now().date_naive())
}

pub(crate) fn url_detail_for_connection(
    connection: &Connection,
    url: &str,
    today: NaiveDate,
) -> Result<UrlDetail> {
    let weeks = week_bounds(today)?;

    let mut statement = connection.prepare(
        "SELECT urls.id, urls.title, urls.last_visit_ms, source_profiles.browser_kind
         FROM urls JOIN source_profiles ON source_profiles.id = urls.source_profile_id
         WHERE urls.url = ?1",
    )?;
    let rows = statement
        .query_map([url], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, Option<String>>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, String>(3)?,
            ))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    let mut count = connection
        .prepare("SELECT COUNT(*) FROM visits WHERE url_id = ?1 AND reverted_at IS NULL")?;
    let mut first = connection.prepare(
        "SELECT MIN(visit_time_ms) FROM visits WHERE url_id = ?1 AND reverted_at IS NULL",
    )?;
    let mut last = connection.prepare(
        "SELECT MAX(visit_time_ms) FROM visits WHERE url_id = ?1 AND reverted_at IS NULL",
    )?;
    let mut in_range = connection.prepare(
        "SELECT COUNT(*) FROM visits
         WHERE url_id = ?1 AND reverted_at IS NULL AND visit_time_ms >= ?2 AND visit_time_ms < ?3",
    )?;

    let mut total = 0_i64;
    let mut first_ms: Option<i64> = None;
    let mut last_ms: Option<i64> = None;
    let mut by_browser: HashMap<String, i64> = HashMap::new();
    let mut weekly = vec![0_i64; WEEKS as usize];
    let mut title: Option<(i64, Option<String>)> = None;

    for (url_id, row_title, row_last_visit, browser_kind) in &rows {
        let visits: i64 = count.query_row([url_id], |row| row.get(0))?;
        if visits == 0 {
            continue;
        }
        total += visits;
        *by_browser.entry(browser_display_name(browser_kind)).or_default() += visits;
        let row_first: Option<i64> = first.query_row([url_id], |row| row.get(0))?;
        let row_last: Option<i64> = last.query_row([url_id], |row| row.get(0))?;
        first_ms = min_option(first_ms, row_first);
        last_ms = max_option(last_ms, row_last);
        for (index, (start, end)) in weeks.iter().enumerate() {
            let in_week: i64 = in_range.query_row(params![url_id, start, end], |row| row.get(0))?;
            weekly[index] += in_week;
        }
        if title.as_ref().is_none_or(|(seen, _)| row_last_visit > seen) {
            title = Some((*row_last_visit, row_title.clone()));
        }
    }

    let mut browsers: Vec<(String, i64)> = by_browser.into_iter().collect();
    browsers.sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(&right.0)));

    let domain = connection
        .query_row(
            "SELECT registrable_domain FROM urls WHERE url = ?1 AND registrable_domain IS NOT NULL LIMIT 1",
            [url],
            |row| row.get::<_, String>(0),
        )
        .optional()?
        .unwrap_or_else(|| url_domain(url));

    Ok(UrlDetail {
        url: url.to_string(),
        title: title.and_then(|(_, title)| title).filter(|value| !value.trim().is_empty()),
        domain,
        total_visits: total,
        first_visit_at: first_ms.and_then(rfc3339_from_ms),
        last_visit_at: last_ms.and_then(rfc3339_from_ms),
        browsers: browsers.into_iter().map(|(name, _)| name).collect(),
        weekly_visits: weeks
            .iter()
            .zip(weekly)
            .enumerate()
            .map(|(index, (_, visits))| UrlWeeklyVisits {
                week_start: week_start_date(today, index).format("%Y-%m-%d").to_string(),
                visits,
            })
            .collect(),
    })
}

fn min_option(left: Option<i64>, right: Option<i64>) -> Option<i64> {
    match (left, right) {
        (Some(a), Some(b)) => Some(a.min(b)),
        (a, b) => a.or(b),
    }
}

fn max_option(left: Option<i64>, right: Option<i64>) -> Option<i64> {
    match (left, right) {
        (Some(a), Some(b)) => Some(a.max(b)),
        (a, b) => a.or(b),
    }
}

/// Local Monday of the week that is `WEEKS - 1 - index` weeks before the current one.
fn week_start_date(today: NaiveDate, index: usize) -> NaiveDate {
    let monday = today - Duration::days(i64::from(today.weekday().num_days_from_monday()));
    monday - Duration::weeks(WEEKS - 1 - index as i64)
}

/// `[start_ms, end_ms)` of each of the 12 weeks, oldest first, as local-midnight instants.
fn week_bounds(today: NaiveDate) -> Result<Vec<(i64, i64)>> {
    (0..WEEKS as usize)
        .map(|index| {
            let start = week_start_date(today, index);
            Ok((local_midnight_ms(start)?, local_midnight_ms(start + Duration::weeks(1))?))
        })
        .collect()
}

fn local_midnight_ms(day: NaiveDate) -> Result<i64> {
    let midnight = day.and_hms_opt(0, 0, 0).context("midnight overflow")?;
    // Midnight can be skipped by a DST jump; the first existing hour is close enough for a bucket edge.
    let resolved = Local
        .from_local_datetime(&midnight)
        .earliest()
        .or_else(|| Local.from_local_datetime(&(midnight + Duration::hours(1))).earliest())
        .context("resolving local midnight")?;
    Ok(resolved.timestamp_millis())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::archive::source_stats::tests::seeded_archive;

    fn ms(day: NaiveDate, hour: u32) -> i64 {
        let local = day.and_hms_opt(hour, 0, 0).expect("time");
        Local.from_local_datetime(&local).earliest().expect("local").timestamp_millis()
    }

    #[test]
    fn buckets_are_twelve_monday_weeks_ending_this_week() {
        let (_root, paths, config) = seeded_archive();
        let connection =
            crate::archive::open_archive_connection(&paths, &config, None).expect("open");
        let today = NaiveDate::from_ymd_opt(2026, 10, 3).expect("date"); // Saturday
        let this_monday = NaiveDate::from_ymd_opt(2026, 9, 28).expect("monday");
        connection
            .execute_batch(&format!(
                "DELETE FROM visits;
                 INSERT INTO visits (url_id, visit_time_ms, visit_time_iso, source_profile_id, created_by_run_id, reverted_at) VALUES
                   (1, {a}, '', 1, 1, NULL),
                   (1, {b}, '', 1, 1, NULL),
                   (2, {c}, '', 2, 1, NULL),
                   (2, {d}, '', 2, 1, NULL),
                   (1, {e}, '', 1, 1, '2026-01-01T00:00:00Z');",
                a = ms(this_monday, 0),
                b = ms(today, 12),
                c = ms(this_monday - Duration::days(1), 23),
                d = ms(this_monday - Duration::weeks(11), 9),
                e = ms(today, 13),
            ))
            .expect("visits");

        let detail =
            url_detail_for_connection(&connection, "https://example.com/a", today).expect("detail");
        assert_eq!(detail.total_visits, 4);
        assert_eq!(detail.weekly_visits.len(), 12);
        assert_eq!(detail.weekly_visits[11].week_start, "2026-09-28");
        assert_eq!(detail.weekly_visits[0].week_start, "2026-07-13");
        let counts: Vec<i64> = detail.weekly_visits.iter().map(|week| week.visits).collect();
        assert_eq!(counts, vec![1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 2]);
        assert_eq!(detail.browsers, vec!["Google Chrome", "Firefox"]);
        assert_eq!(detail.title.as_deref(), Some("A"));
        assert_eq!(detail.domain, "example.com");
        assert!(detail.first_visit_at < detail.last_visit_at);
    }

    #[test]
    fn unknown_url_yields_an_empty_detail() {
        let (_root, paths, config) = seeded_archive();
        let connection =
            crate::archive::open_archive_connection(&paths, &config, None).expect("open");
        let today = NaiveDate::from_ymd_opt(2026, 10, 3).expect("date");
        let detail = url_detail_for_connection(&connection, "https://nowhere.test/x", today)
            .expect("detail");
        assert_eq!(detail.total_visits, 0);
        assert!(detail.browsers.is_empty() && detail.first_visit_at.is_none());
        assert_eq!(detail.weekly_visits.len(), 12);
        assert_eq!(detail.domain, "nowhere.test");
    }
}
