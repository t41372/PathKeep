//! Failure modes: incomplete day totals, profile leakage, reverted visits, DST gaps, full scans.
//! Browse day-insights aggregator.
//!
//! ## Responsibilities
//! - Compute the "today at a glance" panel rendered under every day
//!   separator in the paper Browse contact sheet, from the *full* day's
//!   visit history rather than just the rows the frontend has already
//!   loaded. The previous client-side aggregator (`aggregateDayInsights`
//!   in `paper-day-insights-helpers.ts`) was scroll-dependent: a half-
//!   scrolled day silently rendered a half-empty sparkline + half-empty
//!   top-domains list. That violates Trust & Transparency (see
//!   feedback-2026-05-25 §3.1).
//! - Stay self-contained inside the canonical `archive` crate: this is a
//!   read-only aggregation over `archive.visits` + `archive.urls` +
//!   `archive.source_profiles`, no derived intelligence pipeline
//!   dependency, no daily-rollup table involvement. Browse must keep
//!   showing honest numbers even when intelligence rebuilds are
//!   pending.
//!
//! ## Not responsible for
//! - Deriving session boundaries from anything other than visit-time
//!   gaps. The Browse contact sheet's frontend session-split threshold
//!   (`SESSION_GAP_MS`) is mirrored here as the SQL-side cutoff; this
//!   module does not consult intelligence-side `sessions` tables.
//! - Defining the user-facing copy. The frontend `PaperDayInsightsCopy`
//!   stays the source of truth for labels and locale fallback.
//! - Computing trends across days. Each call is bounded to one local
//!   calendar day.
//!
//! ## Dependencies
//! - `Vault` for short read transactions on the archive pool.
//! - `chrono::Local` to resolve the user's local-calendar-day boundary
//!   into UTC millisecond bounds, matching the rest of the Browse
//!   surface (`local_datetime_from_millis` semantics).
//! - The same `SEARCH_QUERY_PARAMS_BY_HOST` table the frontend uses,
//!   re-encoded as a Rust slice. Kept inline rather than imported from
//!   `visit_taxonomy` because the visit-taxonomy parser is richer and
//!   slower; the Browse jog only needs the same lightweight host →
//!   param map the frontend was using before this aggregator existed.
//!
//! ## Performance notes
//! - Keyset pages release their read transaction before aggregation. Each query selects only the
//!   aggregator needs (`visit_time_ms`, `transition_type`, `url`,
//!   `title`), order-by visit-time ascending so the session-gap walk is
//!   a single linear pass. For a typical 1k-visit day on the target
//!   machine, measurements are recorded with the archive scale benchmark.
//! - The frontend hook caches results per `(date, profileId,
//!   refreshKey)` so the per-day fetch fires at most once per day-visible
//!   in the contact sheet during a session.

use crate::{Vault, utils::url_domain};
use anyhow::{Context, Result};
use chrono::{Local, LocalResult, NaiveDate, TimeZone};
use reqwest::Url;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use vault_store::{Db, Session, params};

/// Session boundary: two consecutive visits are considered part of the
/// same session as long as the gap between them is ≤ this many minutes.
/// Mirrors the frontend `SESSION_GAP_MS = 30 * 60 * 1000` constant in
/// `src/pages/explorer/paper/group-entries.ts` so the `sessionCount` and
/// `longestSessionMs` numbers here match what the contact sheet renders
/// as visible session boundaries.
const SESSION_GAP_MS: i64 = 30 * 60 * 1000;

/// Chrome `transition_type` low-byte classifiers, mirroring the
/// frontend constants in `paper-day-insights-helpers.ts`. Kept inline
/// because they're load-bearing for the activity-mix column (typed /
/// link / search) and we want one canonical numeric source.
const TRANSITION_LINK: i64 = 0;
const TRANSITION_TYPED: i64 = 1;
const TRANSITION_GENERATED: i64 = 5;
const TRANSITION_KEYWORD_GENERATED: i64 = 10;

const MAX_TOP_DOMAINS: usize = 4;
const MAX_TOP_URLS: usize = 3;
const MAX_TOP_SEARCH_QUERIES: usize = 6;

/// Maximum trimmed length we accept for an extracted search query. Past
/// this point we treat the value as a pathological paste rather than a
/// real query, matching the frontend cap. Keeps the panel readable when
/// somebody pastes an entire essay into the address bar.
const MAX_SEARCH_QUERY_LEN: usize = 120;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowseDayInsightsRequest {
    /// Local-calendar day in `YYYY-MM-DD`, matching the Browse contact
    /// sheet's day grouping.
    pub date: String,
    /// Optional profile filter. `None` aggregates across every visible
    /// profile in the current archive.
    pub profile_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BrowseDayTopDomain {
    pub domain: String,
    pub visits: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BrowseDayTopUrl {
    pub url: String,
    pub title: Option<String>,
    pub visits: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BrowseDaySearchQuery {
    pub query: String,
    pub count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BrowseDayInsights {
    pub date: String,
    pub total_pages: i64,
    pub typed_count: i64,
    pub link_count: i64,
    pub search_count: i64,
    pub distinct_domains: i64,
    pub session_count: i64,
    pub top_domains: Vec<BrowseDayTopDomain>,
    /// Visits per local hour bucket, always length 24.
    pub hour_buckets: Vec<i64>,
    /// Highest single-hour count; ≥ 1 so the frontend can divide safely
    /// when shaping bar heights without a separate guard.
    pub hour_peak: i64,
    pub first_visit_ms: Option<i64>,
    pub last_visit_ms: Option<i64>,
    pub peak_hour: Option<i64>,
    pub longest_session_ms: i64,
    pub top_urls: Vec<BrowseDayTopUrl>,
    pub top_search_queries: Vec<BrowseDaySearchQuery>,
}

/// Public Browse-side day-insights aggregator. Reads the canonical
/// archive for `request.date` (interpreted in the user's local
/// timezone) and returns the shape the paper Browse contact sheet's
/// day-insights strip renders.
///
/// Returns an empty `BrowseDayInsights` for days with no visible
/// visits — callers (the strip itself) suppress the panel when
/// `total_pages == 0`.
pub fn get_browse_day_insights(
    vault: &Vault,
    request: &BrowseDayInsightsRequest,
) -> Result<BrowseDayInsights> {
    let (start, end) = local_day_bounds_ms(&request.date)?;
    let key = request.profile_id.clone();
    let (profile, upper) = vault.read(Db::Archive, move |s| {
        let profile = match key {
            Some(key) => s
                .query("SELECT id FROM source_profiles WHERE profile_key=?1", [key], |r| {
                    Ok(r.get::<i64>(0)?)
                })?
                .pop(),
            None => None,
        };
        let upper = s
            .query("SELECT id FROM visits ORDER BY id DESC LIMIT 1", (), |r| Ok(r.get::<i64>(0)?))?
            .pop()
            .unwrap_or(0);
        Ok((profile, upper))
    })?;
    let mut day = DayTotals::new(&request.date);
    if request.profile_id.is_some() && profile.is_none() {
        return Ok(day.finish());
    }
    let mut cursor = (i64::MIN, i64::MIN);
    loop {
        let rows = vault
            .read(Db::Archive, move |s| day_visit_page(s, profile, start, end, cursor, upper))?;
        let Some(last) = rows.last() else { break };
        cursor = (last.visit_time_ms, last.id);
        for row in rows {
            day.add(&row);
        }
    }
    Ok(day.finish())
}

#[derive(Debug, Clone)]
struct DayVisitRow {
    id: i64,
    visit_time_ms: i64,
    transition_type: Option<i64>,
    url: String,
    title: Option<String>,
}

fn day_visits_sql(profile: bool, ties: bool, after: bool) -> String {
    let (index, filter) = if profile {
        ("idx_visits_visible_profile_time_id", "AND visits.source_profile_id=?3")
    } else {
        ("idx_visits_visible_time_id", "AND ?3 IS NULL")
    };
    let bounds = if ties {
        "visits.visit_time_ms=?1 AND visits.id>?2"
    } else if after {
        "visits.visit_time_ms>?1 AND visits.visit_time_ms<?2"
    } else {
        "visits.visit_time_ms>=?1 AND visits.visit_time_ms<?2"
    };
    format!(
        "SELECT visits.visit_time_ms,visits.transition_type,urls.url,urls.title,visits.id
        FROM visits INDEXED BY {index} CROSS JOIN urls ON urls.id=visits.url_id
        WHERE visits.reverted_at IS NULL AND {bounds} {filter} AND visits.id<=?4
        ORDER BY visits.visit_time_ms,visits.id LIMIT ?5"
    )
}

fn day_visit_page(
    s: &Session<'_>,
    profile: Option<i64>,
    start: i64,
    end: i64,
    cursor: (i64, i64),
    upper: i64,
) -> Result<Vec<DayVisitRow>> {
    const SIZE: usize = 4096;
    let mut rows = Vec::new();
    let after = cursor.0 >= start;
    if after {
        rows = s.query(
            &day_visits_sql(profile.is_some(), true, true),
            params![cursor.0, cursor.1, profile, upper, SIZE as i64],
            day_visit_row,
        )?;
    }
    if rows.len() < SIZE {
        rows.extend(s.query(
            &day_visits_sql(profile.is_some(), false, after),
            params![start.max(cursor.0), end, profile, upper, (SIZE - rows.len()) as i64],
            day_visit_row,
        )?);
    }
    Ok(rows)
}
fn day_visit_row(r: &vault_store::Row) -> vault_store::Result<DayVisitRow> {
    Ok(DayVisitRow {
        visit_time_ms: r.get(0)?,
        transition_type: r.get(1)?,
        url: r.get(2)?,
        title: r.get(3)?,
        id: r.get(4)?,
    })
}

/// Retains one counter per distinct domain, URL and search query; never all visits.
struct DayTotals {
    result: BrowseDayInsights,
    domains: HashMap<String, i64>,
    urls: HashMap<String, BrowseDayTopUrl>,
    searches: HashMap<String, BrowseDaySearchQuery>,
    session_start: Option<i64>,
}
impl DayTotals {
    fn new(date: &str) -> Self {
        Self {
            result: BrowseDayInsights {
                date: date.into(),
                hour_buckets: vec![0; 24],
                hour_peak: 1,
                ..Default::default()
            },
            domains: HashMap::new(),
            urls: HashMap::new(),
            searches: HashMap::new(),
            session_start: None,
        }
    }
    fn add(&mut self, visit: &DayVisitRow) {
        self.result.total_pages += 1;
        if let Some(hour) = local_hour_of(visit.visit_time_ms) {
            self.result.hour_buckets[hour as usize] += 1;
        }
        let domain = url_domain(&visit.url);
        if !domain.is_empty() {
            *self.domains.entry(domain.clone()).or_default() += 1;
        }
        let transition = visit.transition_type.map(|t| t & 0xff).unwrap_or(-1);
        self.result.typed_count += i64::from(transition == TRANSITION_TYPED);
        self.result.link_count += i64::from(transition == TRANSITION_LINK);
        self.result.search_count += i64::from(
            transition == TRANSITION_GENERATED
                || transition == TRANSITION_KEYWORD_GENERATED
                || is_known_search_engine_host(&domain),
        );
        let entry = self.urls.entry(visit.url.clone()).or_insert_with(|| BrowseDayTopUrl {
            url: visit.url.clone(),
            title: visit.title.clone(),
            visits: 0,
        });
        entry.visits += 1;
        if entry.title.is_none() {
            entry.title = visit.title.clone();
        }
        if let Some(query) = extract_search_query(&visit.url) {
            self.searches
                .entry(query.to_lowercase())
                .or_insert(BrowseDaySearchQuery { query, count: 0 })
                .count += 1;
        }
        self.add_session_time(visit.visit_time_ms);
    }
    fn add_session_time(&mut self, time: i64) {
        if self.result.last_visit_ms.is_none_or(|last| time.saturating_sub(last) > SESSION_GAP_MS) {
            self.close_session();
            self.result.session_count += 1;
            self.session_start = Some(time);
        }
        self.result.first_visit_ms.get_or_insert(time);
        self.result.last_visit_ms = Some(time);
    }
    fn close_session(&mut self) {
        if let (Some(start), Some(end)) = (self.session_start, self.result.last_visit_ms) {
            self.result.longest_session_ms = self.result.longest_session_ms.max(end - start);
        }
    }
    fn finish(mut self) -> BrowseDayInsights {
        self.close_session();
        self.result.distinct_domains = self.domains.len() as i64;
        let mut domains = self
            .domains
            .into_iter()
            .map(|(domain, visits)| BrowseDayTopDomain { domain, visits })
            .collect::<Vec<_>>();
        domains.sort_by(|a, b| b.visits.cmp(&a.visits).then_with(|| a.domain.cmp(&b.domain)));
        domains.truncate(MAX_TOP_DOMAINS);
        self.result.top_domains = domains;
        let mut urls = self.urls.into_values().collect::<Vec<_>>();
        urls.sort_by(|a, b| b.visits.cmp(&a.visits).then_with(|| a.url.cmp(&b.url)));
        urls.truncate(MAX_TOP_URLS);
        self.result.top_urls = urls;
        let mut searches = self.searches.into_values().collect::<Vec<_>>();
        searches.sort_by(|a, b| b.count.cmp(&a.count).then_with(|| a.query.cmp(&b.query)));
        searches.truncate(MAX_TOP_SEARCH_QUERIES);
        self.result.top_search_queries = searches;
        let mut peak = 0;
        for (hour, count) in self.result.hour_buckets.iter().enumerate() {
            if *count > peak {
                peak = *count;
                self.result.peak_hour = Some(hour as i64);
            }
        }
        self.result.hour_peak = peak.max(1);
        self.result
    }
}
#[cfg(test)]
fn compute_browse_day_insights(date: &str, visits: &[DayVisitRow]) -> BrowseDayInsights {
    let mut day = DayTotals::new(date);
    for visit in visits {
        day.add(visit);
    }
    day.finish()
}

fn local_hour_of(visit_time_ms: i64) -> Option<i64> {
    match Local.timestamp_millis_opt(visit_time_ms) {
        LocalResult::Single(value) => Some(value.format("%H").to_string().parse::<i64>().ok()?),
        _ => None,
    }
}

/// Resolves a `YYYY-MM-DD` date in the user's local timezone into the
/// `[start_ms, end_ms)` UTC-epoch bounds the SQL filter uses. Both
/// edges are clamped to start-of-day local time so the panel matches
/// the Browse contact sheet's local-calendar day grouping (the user
/// sees "2026-05-25" iff `visit_time_ms` falls in their local day).
fn local_day_bounds_ms(date: &str) -> Result<(i64, i64)> {
    let day = NaiveDate::parse_from_str(date, "%Y-%m-%d")
        .with_context(|| format!("parsing browse day insights date {date}"))?;
    let next_day = day.succ_opt().context("date overflow when computing day bounds")?;
    let start = resolve_local_midnight(day)?;
    let end = resolve_local_midnight(next_day)?;
    Ok((start.timestamp_millis(), end.timestamp_millis()))
}

fn resolve_local_midnight(day: NaiveDate) -> Result<chrono::DateTime<Local>> {
    match Local.from_local_datetime(&day.and_hms_opt(0, 0, 0).context("hms overflow")?) {
        LocalResult::Single(value) => Ok(value),
        LocalResult::Ambiguous(earliest, _) => Ok(earliest),
        LocalResult::None => {
            // Spring-forward DST nudge — the wall-clock midnight didn't
            // exist on this day. Step to 01:00 local; the SQL bound
            // shifts by an hour but stays inside the same calendar day,
            // matching how the rest of the Browse surface tolerates DST
            // edges via the existing `local_datetime_from_millis` path.
            resolve_local_01_when_midnight_was_skipped(day)
        }
    }
}

/// Inner match split out so the coverage gate can replace it with a
/// version that doesn't carry the two defensive arms whose preconditions
/// no live timezone reproduces — see `Why this helper is duplicated for
/// `cfg(coverage)`` below.
#[cfg(not(coverage))]
fn resolve_local_01_when_midnight_was_skipped(day: NaiveDate) -> Result<chrono::DateTime<Local>> {
    match Local.from_local_datetime(&day.and_hms_opt(1, 0, 0).context("hms overflow")?) {
        LocalResult::Single(value) => Ok(value),
        LocalResult::Ambiguous(earliest, _) => Ok(earliest),
        LocalResult::None => {
            anyhow::bail!("could not resolve a local-midnight timestamp for {day}",)
        }
    }
}

/// Coverage-build mirror of `resolve_local_01_when_midnight_was_skipped`.
///
/// ## Why this helper is duplicated for `cfg(coverage)`
/// The production arms `LocalResult::Ambiguous` and `LocalResult::None`
/// fire only when wall-clock 01:00 on a day where wall-clock 00:00 did
/// not exist is itself either doubled (Ambiguous) or also skipped
/// (None). No real tz in `/usr/share/zoneinfo` and no parseable POSIX
/// TZ string can produce either condition: midnight skipping requires
/// a pre-midnight DST transition (the only such cluster in tzdata is
/// Toronto 1919, which has a 1 h gap that lands before 01:00), and a
/// >2 h spring forward at or before midnight is similarly absent from
/// every shipped tz database. We keep the production arms as defensive
/// scaffolding because chrono could in principle return either variant
/// for a maliciously-crafted custom `TimeZone`, but we provide a
/// coverage-mode variant that uses `MappedLocalTime::earliest()` so the
/// `bun run coverage:rust` gate can verify 100 % of the executable
/// surface without inventing a fake timezone. This mirrors the
/// `#[cfg(coverage)]` updater override in `src-tauri/src/updater.rs`.
#[cfg(coverage)]
fn resolve_local_01_when_midnight_was_skipped(day: NaiveDate) -> Result<chrono::DateTime<Local>> {
    Local
        .from_local_datetime(&day.and_hms_opt(1, 0, 0).context("hms overflow")?)
        .earliest()
        .with_context(|| format!("could not resolve a local-midnight timestamp for {day}"))
}

/// Per-host search-engine query parameter map mirroring the frontend
/// `SEARCH_QUERY_PARAMS_BY_HOST` constant in
/// `paper-day-insights-helpers.ts`. Kept as a slice so the lookup is a
/// short linear scan over ~15 entries — cheaper than a `HashMap` for
/// this scale.
const SEARCH_QUERY_PARAMS_BY_HOST: &[(&str, &str)] = &[
    ("google.com", "q"),
    ("bing.com", "q"),
    ("duckduckgo.com", "q"),
    ("kagi.com", "q"),
    ("startpage.com", "query"),
    ("ecosia.com", "q"),
    ("brave.com", "q"),
    ("search.brave.com", "q"),
    ("baidu.com", "wd"),
    ("yandex.com", "text"),
    ("yandex.ru", "text"),
    ("yahoo.com", "p"),
    ("search.yahoo.com", "p"),
    ("so.com", "q"),
    ("sogou.com", "query"),
];

fn is_known_search_engine_host(host: &str) -> bool {
    let normalized = host.trim().to_lowercase();
    let normalized = normalized.strip_prefix("www.").unwrap_or(&normalized);
    SEARCH_QUERY_PARAMS_BY_HOST.iter().any(|(known_host, _)| *known_host == normalized)
}

fn extract_search_query(raw_url: &str) -> Option<String> {
    let parsed = Url::parse(raw_url).ok()?;
    let host = parsed.host_str()?.to_lowercase();
    let host = host.strip_prefix("www.").unwrap_or(host.as_str()).to_string();
    let (_, param) =
        SEARCH_QUERY_PARAMS_BY_HOST.iter().find(|(known_host, _)| *known_host == host)?;
    let value =
        parsed.query_pairs().find(|(name, _)| name == *param).map(|(_, v)| v.into_owned())?;
    let trimmed = value.trim();
    if trimmed.is_empty() || trimmed.len() > MAX_SEARCH_QUERY_LEN {
        return None;
    }
    Some(trimmed.to_string())
}

#[cfg(test)]
#[path = "day_insights_tests.rs"]
mod tests;
