//! Failure modes: profile loss, reverted rows counted, wrong week bounds, unknown URL errors, full scans.
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
//!   first/last seek and each weekly range. Lifetime counts come from the page/profile summaries.

use super::source_stats::rfc3339_from_ms;
use crate::{
    Vault,
    chrome::browser_display_name,
    models::{UrlDetail, UrlWeeklyVisits},
    utils::url_domain,
};
use anyhow::{Context, Result};
use chrono::{Datelike, Duration, Local, NaiveDate, TimeZone};
use std::collections::{HashMap, HashSet};
use vault_store::{Db, Session, params};

const WEEKS: i64 = 12;

/// Loads the detail panel data for `url` from the canonical archive.
/// Whether the user's history holds a visit to `url`, as recorded or as its page's canonical URL,
/// that they have not removed. Opening a `file://` URL can launch an application, so PathKeep
/// opens one only when it is a page the user visited, never because a link said so.
pub fn is_visited_url(vault: &Vault, url: &str) -> Result<bool> {
    let url = url.trim().to_owned();
    vault.read(Db::Archive, move |s| {
        let visible = "EXISTS(SELECT 1 FROM visits INDEXED BY idx_visits_visible_url_time WHERE url_id=urls.id AND reverted_at IS NULL)";
        let recorded = s.query_one(
            &format!("SELECT EXISTS(SELECT 1 FROM urls INDEXED BY idx_urls_url WHERE urls.url=?1 AND {visible})"),
            params![url.as_str()],
            |r| Ok(r.get::<i64>(0)? != 0),
        )?;
        Ok(recorded
            || s.query_one(
                &format!("SELECT EXISTS(SELECT 1 FROM pages INDEXED BY idx_pages_canonical_url CROSS JOIN urls INDEXED BY idx_urls_page_id ON urls.page_id=pages.id WHERE pages.canonical_url=?1 AND {visible})"),
                params![url.as_str()],
                |r| Ok(r.get::<i64>(0)? != 0),
            )?)
    })
}

pub fn get_url_detail(vault: &Vault, url: &str) -> Result<UrlDetail> {
    let url = url.to_owned();
    vault.read(Db::Archive, move |s| url_detail_for_session(s, &url, Local::now().date_naive()))
}

const URL_ROWS_SQL: &str = "SELECT urls.id,urls.title,urls.last_visit_ms,source_profiles.browser_kind,source_profiles.id FROM urls CROSS JOIN source_profiles ON source_profiles.id=urls.source_profile_id WHERE urls.url=?1 AND EXISTS(SELECT 1 FROM visits WHERE url_id=urls.id AND reverted_at IS NULL)";
const URL_COUNTS_SQL: &str = "SELECT source_profile_id,visible_visit_count FROM search_document_profiles WHERE page_id=(SELECT id FROM pages WHERE url=?1)";
const URL_FIRST_SQL: &str = "SELECT visit_time_ms FROM visits INDEXED BY idx_visits_visible_url_time WHERE url_id=?1 AND reverted_at IS NULL ORDER BY visit_time_ms,id LIMIT 1";
const URL_LAST_SQL: &str = "SELECT visit_time_ms FROM visits INDEXED BY idx_visits_visible_url_time WHERE url_id=?1 AND reverted_at IS NULL ORDER BY visit_time_ms DESC,id DESC LIMIT 1";
const URL_RANGE_SQL: &str = "SELECT COUNT(*) FROM visits INDEXED BY idx_visits_visible_url_time WHERE url_id=?1 AND reverted_at IS NULL AND visit_time_ms>=?2 AND visit_time_ms<?3";
const URL_DOMAIN_SQL: &str = "SELECT registrable_domain FROM pages WHERE url=?1";

pub(crate) fn url_detail_for_session(
    s: &Session<'_>,
    url: &str,
    today: NaiveDate,
) -> Result<UrlDetail> {
    let weeks = week_bounds(today)?;
    let rows = s.query(URL_ROWS_SQL, [url], |r| {
        Ok((
            r.get::<i64>(0)?,
            r.get::<Option<String>>(1)?,
            r.get::<i64>(2)?,
            r.get::<String>(3)?,
            r.get::<i64>(4)?,
        ))
    })?;
    let counts = s
        .query(URL_COUNTS_SQL, [url], |r| Ok((r.get::<i64>(0)?, r.get::<i64>(1)?)))?
        .into_iter()
        .collect::<HashMap<_, _>>();
    let mut counted_profiles = HashSet::new();
    let mut total = 0_i64;
    let mut first_ms: Option<i64> = None;
    let mut last_ms: Option<i64> = None;
    let mut by_browser: HashMap<String, i64> = HashMap::new();
    let mut weekly = vec![0_i64; WEEKS as usize];
    let mut title: Option<(i64, Option<String>)> = None;

    for (url_id, row_title, row_last_visit, browser_kind, profile) in &rows {
        if counted_profiles.insert(*profile) {
            let visits = counts[profile];
            total += visits;
            *by_browser.entry(browser_display_name(browser_kind)).or_default() += visits;
        }
        let row_first: Option<i64> = s.query(URL_FIRST_SQL, [*url_id], |r| Ok(r.get(0)?))?.pop();
        let row_last: Option<i64> = s.query(URL_LAST_SQL, [*url_id], |r| Ok(r.get(0)?))?.pop();
        first_ms = min_option(first_ms, row_first);
        last_ms = max_option(last_ms, row_last);
        for (index, (start, end)) in weeks.iter().enumerate() {
            let in_week: i64 =
                s.query_one(URL_RANGE_SQL, params![*url_id, *start, *end], |r| Ok(r.get(0)?))?;
            weekly[index] += in_week;
        }
        if title.as_ref().is_none_or(|(seen, _)| row_last_visit > seen) {
            title = Some((*row_last_visit, row_title.clone()));
        }
    }

    let mut browsers: Vec<(String, i64)> = by_browser.into_iter().collect();
    browsers.sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(&right.0)));

    let domain = s
        .query(URL_DOMAIN_SQL, [url], |r| Ok(r.get::<Option<String>>(0)?))?
        .pop()
        .flatten()
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
#[path = "url_detail_tests.rs"]
mod tests;
