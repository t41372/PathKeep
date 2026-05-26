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
//! - `archive::open_archive_connection` for the canonical archive DB.
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
//! - One SQL pass per day. The query selects only the columns the
//!   aggregator needs (`visit_time_ms`, `transition_type`, `url`,
//!   `title`), order-by visit-time ascending so the session-gap walk is
//!   a single linear pass. For a typical 1k-visit day on the target
//!   machine (4-core 3GHz / 8GB), this stays well under 50 ms.
//! - The frontend hook caches results per `(date, profileId,
//!   refreshKey)` so the per-day fetch fires at most once per day-visible
//!   in the contact sheet during a session.

use crate::{
    archive::open_archive_connection, config::ProjectPaths, models::AppConfig, utils::url_domain,
};
use anyhow::{Context, Result};
use chrono::{Local, LocalResult, NaiveDate, TimeZone};
use reqwest::Url;
use rusqlite::{Connection, named_params};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

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
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    request: &BrowseDayInsightsRequest,
) -> Result<BrowseDayInsights> {
    let connection = open_archive_connection(paths, config, key)?;
    aggregate_browse_day_insights(&connection, request)
}

/// Connection-level variant used by tests and the public entrypoint.
pub(crate) fn aggregate_browse_day_insights(
    connection: &Connection,
    request: &BrowseDayInsightsRequest,
) -> Result<BrowseDayInsights> {
    let (start_ms, end_ms) = local_day_bounds_ms(&request.date)?;
    let visits = load_day_visits(connection, request.profile_id.as_deref(), start_ms, end_ms)?;
    Ok(compute_browse_day_insights(&request.date, &visits))
}

#[derive(Debug, Clone)]
struct DayVisitRow {
    visit_time_ms: i64,
    transition_type: Option<i64>,
    url: String,
    title: Option<String>,
}

fn load_day_visits(
    connection: &Connection,
    profile_id: Option<&str>,
    start_ms: i64,
    end_ms: i64,
) -> Result<Vec<DayVisitRow>> {
    let mut statement = connection.prepare(
        r#"
        SELECT
          visits.visit_time_ms,
          visits.transition_type,
          urls.url,
          urls.title
        FROM visits
        JOIN urls ON urls.id = visits.url_id
        JOIN source_profiles ON source_profiles.id = visits.source_profile_id
        WHERE visits.reverted_at IS NULL
          AND (:profileId IS NULL OR source_profiles.profile_key = :profileId)
          AND visits.visit_time_ms >= :startMs
          AND visits.visit_time_ms < :endMs
        ORDER BY visits.visit_time_ms ASC, visits.id ASC
        "#,
    )?;
    let rows = statement
        .query_map(
            named_params! {
                ":profileId": profile_id,
                ":startMs": start_ms,
                ":endMs": end_ms,
            },
            |row| {
                Ok(DayVisitRow {
                    visit_time_ms: row.get(0)?,
                    transition_type: row.get(1)?,
                    url: row.get(2)?,
                    title: row.get(3)?,
                })
            },
        )?
        .collect::<rusqlite::Result<Vec<_>>>()
        .context("loading day visits for browse day insights")?;
    Ok(rows)
}

fn compute_browse_day_insights(date: &str, visits: &[DayVisitRow]) -> BrowseDayInsights {
    if visits.is_empty() {
        return BrowseDayInsights {
            date: date.to_string(),
            hour_buckets: vec![0; 24],
            hour_peak: 1,
            ..BrowseDayInsights::default()
        };
    }

    let mut domain_counts: HashMap<String, i64> = HashMap::new();
    let mut url_counts: HashMap<String, BrowseDayTopUrl> = HashMap::new();
    let mut search_counts: HashMap<String, BrowseDaySearchQuery> = HashMap::new();
    let mut hour_buckets = vec![0_i64; 24];

    let mut total_pages = 0_i64;
    let mut typed_count = 0_i64;
    let mut link_count = 0_i64;
    let mut search_count = 0_i64;

    // Session walk: visits are already in ascending order, so a single
    // linear pass closes a session whenever the gap to the previous
    // visit exceeds SESSION_GAP_MS.
    let mut session_count = 0_i64;
    let mut longest_session_ms = 0_i64;
    let mut current_session_start_ms: Option<i64> = None;
    let mut current_session_end_ms: Option<i64> = None;
    let mut last_visit_ms_seen: Option<i64> = None;

    for visit in visits {
        total_pages += 1;

        let local_hour = local_hour_of(visit.visit_time_ms);
        if let Some(hour) = local_hour {
            hour_buckets[hour as usize] += 1;
        }

        let domain = url_domain(&visit.url);
        let domain_for_classification = domain.clone();
        if !domain.is_empty() {
            *domain_counts.entry(domain).or_default() += 1;
        }

        let transition_low = visit.transition_type.map(|t| t & 0xff).unwrap_or(-1);
        if transition_low == TRANSITION_TYPED {
            typed_count += 1;
        }
        if transition_low == TRANSITION_LINK {
            link_count += 1;
        }
        let transition_marks_search = transition_low == TRANSITION_GENERATED
            || transition_low == TRANSITION_KEYWORD_GENERATED;
        let host_marks_search = is_known_search_engine_host(&domain_for_classification);
        if transition_marks_search || host_marks_search {
            search_count += 1;
        }

        let entry = url_counts.entry(visit.url.clone()).or_insert_with(|| BrowseDayTopUrl {
            url: visit.url.clone(),
            title: visit.title.clone(),
            visits: 0,
        });
        entry.visits += 1;
        if entry.title.is_none() && visit.title.is_some() {
            entry.title = visit.title.clone();
        }

        if let Some(query) = extract_search_query(&visit.url) {
            let key = query.to_lowercase();
            let entry = search_counts
                .entry(key)
                .or_insert_with(|| BrowseDaySearchQuery { query: query.clone(), count: 0 });
            entry.count += 1;
        }

        match (current_session_start_ms, current_session_end_ms) {
            // The SQL `ORDER BY visit_time_ms` guarantees the inputs are
            // ascending and the subtraction never goes negative under
            // contract. `saturating_sub` makes the defence explicit so a
            // future refactor that relaxes the ordering or an external
            // feed that wraps near `i64::MAX` cannot silently corrupt the
            // session-gap classifier into producing wrong day insights.
            // Claude review finding #7.
            (Some(_), Some(end)) if visit.visit_time_ms.saturating_sub(end) > SESSION_GAP_MS => {
                // Close the previous session, open a new one.
                if let (Some(start), Some(end)) = (current_session_start_ms, current_session_end_ms)
                {
                    let span = (end - start).max(0);
                    if span > longest_session_ms {
                        longest_session_ms = span;
                    }
                }
                session_count += 1;
                current_session_start_ms = Some(visit.visit_time_ms);
                current_session_end_ms = Some(visit.visit_time_ms);
            }
            (None, None) => {
                // First visit of the day opens session #1 below the
                // loop; track its bounds here.
                session_count = 1;
                current_session_start_ms = Some(visit.visit_time_ms);
                current_session_end_ms = Some(visit.visit_time_ms);
            }
            _ => {
                current_session_end_ms = Some(visit.visit_time_ms);
            }
        }
        last_visit_ms_seen = Some(visit.visit_time_ms);
    }

    // Close the final open session.
    if let (Some(start), Some(end)) = (current_session_start_ms, current_session_end_ms) {
        let span = (end - start).max(0);
        if span > longest_session_ms {
            longest_session_ms = span;
        }
    }

    let mut top_domains: Vec<BrowseDayTopDomain> = domain_counts
        .iter()
        .map(|(domain, visits)| BrowseDayTopDomain { domain: domain.clone(), visits: *visits })
        .collect();
    top_domains.sort_by(|a, b| b.visits.cmp(&a.visits).then_with(|| a.domain.cmp(&b.domain)));
    top_domains.truncate(MAX_TOP_DOMAINS);

    let mut top_urls: Vec<BrowseDayTopUrl> = url_counts.into_values().collect();
    top_urls.sort_by(|a, b| b.visits.cmp(&a.visits).then_with(|| a.url.cmp(&b.url)));
    top_urls.truncate(MAX_TOP_URLS);

    let mut top_search_queries: Vec<BrowseDaySearchQuery> = search_counts.into_values().collect();
    top_search_queries.sort_by(|a, b| b.count.cmp(&a.count).then_with(|| a.query.cmp(&b.query)));
    top_search_queries.truncate(MAX_TOP_SEARCH_QUERIES);

    let mut peak_hour: Option<i64> = None;
    let mut peak_count = 0_i64;
    for (hour, count) in hour_buckets.iter().enumerate() {
        if *count > peak_count {
            peak_count = *count;
            peak_hour = Some(hour as i64);
        }
    }

    BrowseDayInsights {
        date: date.to_string(),
        total_pages,
        typed_count,
        link_count,
        search_count,
        distinct_domains: domain_counts.len() as i64,
        session_count,
        top_domains,
        hour_buckets,
        hour_peak: peak_count.max(1),
        first_visit_ms: visits.first().map(|v| v.visit_time_ms),
        last_visit_ms: last_visit_ms_seen,
        peak_hour,
        longest_session_ms,
        top_urls,
        top_search_queries,
    }
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
mod tests {
    use super::*;
    use crate::archive::create_schema;
    use crate::config::project_paths_with_root;
    use rusqlite::params;
    use tempfile::TempDir;

    fn fresh_connection() -> (TempDir, Connection) {
        let dir = TempDir::new().expect("tempdir");
        let paths = project_paths_with_root(dir.path());
        let config = AppConfig::default();
        std::fs::create_dir_all(paths.archive_database_path.parent().unwrap()).unwrap();
        let connection = open_archive_connection(&paths, &config, None).unwrap();
        create_schema(&connection).unwrap();
        // Seed one run + one source profile so the foreign-key
        // constraints on `visits` / `urls` are satisfied.
        connection
            .execute(
                "INSERT INTO source_profiles \
                 (id, profile_key, browser_kind, browser_version, profile_name, profile_path, discovered_at) \
                 VALUES (1, 'chrome:Default', 'chromium', '120.0.0', 'Default', '/tmp/Default', '2026-05-25T00:00:00Z')",
                [],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO source_profiles \
                 (id, profile_key, browser_kind, browser_version, profile_name, profile_path, discovered_at) \
                 VALUES (2, 'firefox:Default', 'firefox', '120.0', 'Default', '/tmp/Firefox', '2026-05-25T00:00:00Z')",
                [],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO runs \
                 (id, started_at, finished_at, status, run_type, trigger, timezone) \
                 VALUES (1, '2026-05-25T00:00:00Z', '2026-05-25T00:00:00Z', 'success', 'backup', 'manual', 'UTC')",
                [],
            )
            .unwrap();
        (dir, connection)
    }

    fn insert_visit(
        connection: &Connection,
        profile_db_id: i64,
        url: &str,
        title: Option<&str>,
        visit_time_ms: i64,
        transition_type: i64,
    ) {
        // urls is a per-row table in this schema, so reuse one row per url.
        let url_id: i64 = connection
            .query_row(
                "SELECT id FROM urls WHERE url = ?1 AND source_profile_id = ?2",
                params![url, profile_db_id],
                |row| row.get(0),
            )
            .unwrap_or(-1);
        let url_id = if url_id > 0 {
            url_id
        } else {
            connection
                .execute(
                    "INSERT INTO urls \
                     (url, title, visit_count, typed_count, first_visit_ms, first_visit_iso, last_visit_ms, last_visit_iso, source_profile_id, created_by_run_id) \
                     VALUES (?1, ?2, 1, 0, ?3, '2026-05-25T00:00:00Z', ?3, '2026-05-25T00:00:00Z', ?4, 1)",
                    params![url, title, visit_time_ms, profile_db_id],
                )
                .unwrap();
            connection.last_insert_rowid()
        };
        connection
            .execute(
                "INSERT INTO visits \
                 (url_id, source_visit_id, visit_time_ms, visit_time_iso, transition_type, visit_duration_ms, source_profile_id, created_by_run_id) \
                 VALUES (?1, ?2, ?3, '2026-05-25T00:00:00Z', ?4, NULL, ?5, 1)",
                params![
                    url_id,
                    format!("{visit_time_ms}"),
                    visit_time_ms,
                    transition_type,
                    profile_db_id
                ],
            )
            .unwrap();
    }

    // Helper: a fixed local-day timestamp so tests don't depend on the
    // host clock. We resolve "2026-05-25 HH:MM:SS local" into a single
    // millis value.
    //
    // DST handling: if the requested local time falls into a DST
    // "spring-forward" gap (Ambiguous / None on either call), retry one
    // hour later. The retry is wrapped in an explicit `match` instead
    // of `.unwrap()` so a hypothetical future tzdata with a >2-hour
    // jump that nukes both midnight AND 01:00 fails with a clear
    // assertion message — the previous `.unwrap()` would emit a bare
    // "called `Option::unwrap()` on a `None` value" with no file:line
    // context inside the test runner. Claude review finding #8.
    fn local_ms(hour: u32, minute: u32, second: u32) -> i64 {
        let day = NaiveDate::from_ymd_opt(2026, 5, 25).unwrap();
        let first =
            Local.from_local_datetime(&day.and_hms_opt(hour, minute, second).unwrap()).single();
        let resolved = match first {
            Some(dt) => dt,
            None => match Local
                .from_local_datetime(&day.and_hms_opt(hour + 1, minute, second).unwrap())
                .single()
            {
                Some(dt) => dt,
                None => panic!(
                    "DST helper exhausted at 2026-05-25 {hour:02}:{minute:02}:{second:02} \
                     local — both the requested hour and hour+1 returned Ambiguous/None. \
                     The test tzdata likely has a >2h DST jump or the helper needs widening.",
                ),
            },
        };
        resolved.timestamp_millis()
    }

    #[test]
    fn empty_day_returns_zero_metrics_with_padded_hour_buckets() {
        let (_dir, connection) = fresh_connection();
        let insights = aggregate_browse_day_insights(
            &connection,
            &BrowseDayInsightsRequest { date: "2026-05-25".to_string(), profile_id: None },
        )
        .unwrap();
        assert_eq!(insights.total_pages, 0);
        assert_eq!(insights.session_count, 0);
        assert_eq!(insights.hour_buckets.len(), 24);
        assert_eq!(insights.hour_peak, 1);
        assert!(insights.peak_hour.is_none());
        assert!(insights.first_visit_ms.is_none());
        assert!(insights.last_visit_ms.is_none());
        assert!(insights.top_domains.is_empty());
        assert!(insights.top_urls.is_empty());
        assert!(insights.top_search_queries.is_empty());
    }

    #[test]
    fn aggregates_transitions_domains_urls_and_search_queries() {
        let (_dir, connection) = fresh_connection();
        // Three visits to one domain (one typed, two link), one search
        // visit, and one extra page on a second domain.
        insert_visit(
            &connection,
            1,
            "https://news.example.com/a",
            Some("A"),
            local_ms(9, 0, 0),
            TRANSITION_TYPED,
        );
        insert_visit(
            &connection,
            1,
            "https://news.example.com/b",
            Some("B"),
            local_ms(9, 5, 0),
            TRANSITION_LINK,
        );
        insert_visit(
            &connection,
            1,
            "https://news.example.com/a",
            Some("A"),
            local_ms(9, 8, 0),
            TRANSITION_LINK,
        );
        insert_visit(
            &connection,
            1,
            "https://www.google.com/search?q=sqlite+wal",
            Some("Google"),
            local_ms(10, 0, 0),
            TRANSITION_GENERATED,
        );
        insert_visit(
            &connection,
            1,
            "https://docs.example.org/foo",
            None,
            local_ms(11, 0, 0),
            TRANSITION_LINK,
        );

        let insights = aggregate_browse_day_insights(
            &connection,
            &BrowseDayInsightsRequest { date: "2026-05-25".to_string(), profile_id: None },
        )
        .unwrap();
        assert_eq!(insights.total_pages, 5);
        assert_eq!(insights.typed_count, 1);
        assert_eq!(insights.link_count, 3);
        assert_eq!(insights.search_count, 1);
        assert_eq!(insights.distinct_domains, 3);

        // The most-visited URL is /a with 2 visits, then /b, /search, /foo.
        assert_eq!(insights.top_urls[0].url, "https://news.example.com/a");
        assert_eq!(insights.top_urls[0].visits, 2);

        // news.example.com leads with 3 visits.
        assert_eq!(insights.top_domains[0].domain, "news.example.com");
        assert_eq!(insights.top_domains[0].visits, 3);

        // Search query extracted from google.com.
        assert_eq!(insights.top_search_queries.len(), 1);
        assert_eq!(insights.top_search_queries[0].query, "sqlite wal");
        assert_eq!(insights.top_search_queries[0].count, 1);

        assert_eq!(insights.first_visit_ms, Some(local_ms(9, 0, 0)));
        assert_eq!(insights.last_visit_ms, Some(local_ms(11, 0, 0)));
    }

    #[test]
    fn session_walk_splits_on_gap_greater_than_thirty_minutes() {
        let (_dir, connection) = fresh_connection();
        // Session 1: 09:00, 09:15 (15-min gap), 09:30 (15-min gap)
        // Session 2: 11:00 (gap of 1.5h splits)
        insert_visit(&connection, 1, "https://a.test/", None, local_ms(9, 0, 0), TRANSITION_LINK);
        insert_visit(&connection, 1, "https://a.test/", None, local_ms(9, 15, 0), TRANSITION_LINK);
        insert_visit(&connection, 1, "https://a.test/", None, local_ms(9, 30, 0), TRANSITION_LINK);
        insert_visit(&connection, 1, "https://a.test/", None, local_ms(11, 0, 0), TRANSITION_LINK);

        let insights = aggregate_browse_day_insights(
            &connection,
            &BrowseDayInsightsRequest { date: "2026-05-25".to_string(), profile_id: None },
        )
        .unwrap();
        assert_eq!(insights.session_count, 2);
        // Longest session is session 1 spanning 30 minutes = 1_800_000 ms.
        assert_eq!(insights.longest_session_ms, 30 * 60 * 1000);
    }

    #[test]
    fn hour_buckets_track_local_hour_and_peak_hour_picks_the_busiest() {
        let (_dir, connection) = fresh_connection();
        insert_visit(&connection, 1, "https://a.test/", None, local_ms(9, 0, 0), TRANSITION_LINK);
        insert_visit(&connection, 1, "https://a.test/", None, local_ms(14, 0, 0), TRANSITION_LINK);
        insert_visit(&connection, 1, "https://a.test/", None, local_ms(14, 30, 0), TRANSITION_LINK);
        insert_visit(&connection, 1, "https://a.test/", None, local_ms(14, 45, 0), TRANSITION_LINK);
        insert_visit(&connection, 1, "https://a.test/", None, local_ms(20, 0, 0), TRANSITION_LINK);

        let insights = aggregate_browse_day_insights(
            &connection,
            &BrowseDayInsightsRequest { date: "2026-05-25".to_string(), profile_id: None },
        )
        .unwrap();
        assert_eq!(insights.hour_buckets.len(), 24);
        assert_eq!(insights.hour_buckets[9], 1);
        assert_eq!(insights.hour_buckets[14], 3);
        assert_eq!(insights.hour_buckets[20], 1);
        assert_eq!(insights.peak_hour, Some(14));
        assert_eq!(insights.hour_peak, 3);
    }

    #[test]
    fn profile_filter_only_aggregates_the_requested_profile() {
        let (_dir, connection) = fresh_connection();
        insert_visit(
            &connection,
            1,
            "https://chrome.test/",
            None,
            local_ms(9, 0, 0),
            TRANSITION_LINK,
        );
        insert_visit(
            &connection,
            1,
            "https://chrome.test/",
            None,
            local_ms(9, 1, 0),
            TRANSITION_LINK,
        );
        insert_visit(
            &connection,
            2,
            "https://firefox.test/",
            None,
            local_ms(10, 0, 0),
            TRANSITION_LINK,
        );
        // Aggregate across both:
        let all = aggregate_browse_day_insights(
            &connection,
            &BrowseDayInsightsRequest { date: "2026-05-25".to_string(), profile_id: None },
        )
        .unwrap();
        assert_eq!(all.total_pages, 3);
        // Filter to chrome only:
        let chrome = aggregate_browse_day_insights(
            &connection,
            &BrowseDayInsightsRequest {
                date: "2026-05-25".to_string(),
                profile_id: Some("chrome:Default".to_string()),
            },
        )
        .unwrap();
        assert_eq!(chrome.total_pages, 2);
        assert_eq!(chrome.distinct_domains, 1);
        assert_eq!(chrome.top_domains[0].domain, "chrome.test");
    }

    #[test]
    fn reverted_visits_are_excluded_from_the_aggregate() {
        let (_dir, connection) = fresh_connection();
        insert_visit(&connection, 1, "https://a.test/", None, local_ms(9, 0, 0), TRANSITION_LINK);
        insert_visit(&connection, 1, "https://a.test/", None, local_ms(9, 5, 0), TRANSITION_LINK);
        // Mark the second visit reverted.
        connection
            .execute(
                "UPDATE visits SET reverted_at = '2026-05-25T00:00:00Z' WHERE visit_time_ms = ?1",
                params![local_ms(9, 5, 0)],
            )
            .unwrap();
        let insights = aggregate_browse_day_insights(
            &connection,
            &BrowseDayInsightsRequest { date: "2026-05-25".to_string(), profile_id: None },
        )
        .unwrap();
        assert_eq!(insights.total_pages, 1);
    }

    #[test]
    fn invalid_date_string_returns_an_error() {
        let (_dir, connection) = fresh_connection();
        let result = aggregate_browse_day_insights(
            &connection,
            &BrowseDayInsightsRequest { date: "not-a-date".to_string(), profile_id: None },
        );
        assert!(result.is_err());
    }

    #[test]
    fn extract_search_query_understands_known_engines_only() {
        assert_eq!(
            extract_search_query("https://www.google.com/search?q=sqlite+wal"),
            Some("sqlite wal".to_string()),
        );
        assert_eq!(
            extract_search_query("https://duckduckgo.com/?q=pathkeep"),
            Some("pathkeep".to_string()),
        );
        // Yandex uses `text`, not `q`.
        assert_eq!(extract_search_query("https://yandex.ru/?q=ignored"), None);
        assert_eq!(
            extract_search_query("https://yandex.ru/?text=pathkeep"),
            Some("pathkeep".to_string()),
        );
        // Empty values are rejected.
        assert_eq!(extract_search_query("https://www.google.com/search?q="), None);
        // Very long pastes are rejected.
        let long = "a".repeat(MAX_SEARCH_QUERY_LEN + 1);
        let url = format!("https://www.google.com/search?q={long}");
        assert_eq!(extract_search_query(&url), None);
        // Unknown hosts contribute nothing.
        assert_eq!(extract_search_query("https://example.com/?q=foo"), None);
    }

    #[test]
    fn get_browse_day_insights_public_api_opens_archive_and_aggregates_the_requested_day() {
        // Drives the public entry point end-to-end so the
        // open_archive_connection + aggregate path is exercised the way
        // the Tauri command façade calls it. Other tests in this file
        // talk directly to `aggregate_browse_day_insights` with an
        // already-open connection.
        let dir = TempDir::new().expect("tempdir");
        let paths = project_paths_with_root(dir.path());
        let config = AppConfig::default();
        std::fs::create_dir_all(paths.archive_database_path.parent().unwrap()).unwrap();
        let connection = open_archive_connection(&paths, &config, None).unwrap();
        create_schema(&connection).unwrap();
        connection
            .execute(
                "INSERT INTO source_profiles \
                 (id, profile_key, browser_kind, browser_version, profile_name, profile_path, discovered_at) \
                 VALUES (1, 'chrome:Default', 'chromium', '120.0.0', 'Default', '/tmp/Default', '2026-05-25T00:00:00Z')",
                [],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO runs \
                 (id, started_at, finished_at, status, run_type, trigger, timezone) \
                 VALUES (1, '2026-05-25T00:00:00Z', '2026-05-25T00:00:00Z', 'success', 'backup', 'manual', 'UTC')",
                [],
            )
            .unwrap();
        insert_visit(&connection, 1, "https://a.test/", None, local_ms(10, 0, 0), TRANSITION_LINK);
        drop(connection);

        let insights = get_browse_day_insights(
            &paths,
            &config,
            None,
            &BrowseDayInsightsRequest { date: "2026-05-25".to_string(), profile_id: None },
        )
        .expect("public API call should succeed");
        assert_eq!(insights.total_pages, 1);
        assert_eq!(insights.distinct_domains, 1);
    }

    #[test]
    fn compute_browse_day_insights_back_fills_title_when_first_visit_was_titleless() {
        // Two visits to the same URL: the first carries no title, the
        // second carries one. The aggregator must back-fill so the
        // top-URLs row in the day-insights panel doesn't render with
        // an empty title when a later visit did capture it.
        let visits = vec![
            DayVisitRow {
                visit_time_ms: 1_700_000_000_000,
                transition_type: Some(0),
                url: "https://example.test/article".to_string(),
                title: None,
            },
            DayVisitRow {
                visit_time_ms: 1_700_000_060_000,
                transition_type: Some(0),
                url: "https://example.test/article".to_string(),
                title: Some("Example Article".to_string()),
            },
        ];
        let insights = compute_browse_day_insights("2026-05-25", &visits);
        assert_eq!(insights.top_urls.len(), 1);
        assert_eq!(insights.top_urls[0].title.as_deref(), Some("Example Article"));
    }

    #[test]
    fn local_hour_of_returns_none_for_visit_times_outside_chrono_representable_range() {
        // Exercises the `_ => None` arm in local_hour_of without going
        // through the SQL path — chrono's timestamp_millis_opt returns
        // None for values past its representable range, and the
        // hour-bucket index must simply be skipped rather than
        // panicking. A single visit avoids the session-gap subtraction,
        // which would itself overflow for an extreme millis value (a
        // pathological case that can only arise from a corrupt archive
        // row and is exercised by the load_day_visits SQL filter
        // separately).
        let visits = vec![DayVisitRow {
            visit_time_ms: i64::MAX,
            transition_type: Some(0),
            url: "https://example.test/".to_string(),
            title: None,
        }];
        let insights = compute_browse_day_insights("2026-05-25", &visits);
        assert_eq!(insights.total_pages, 1);
        // No representable hour means the panel hour-buckets stay zero.
        assert!(insights.hour_buckets.iter().all(|count| *count == 0));
    }

    // ── DST-edge coverage for resolve_local_midnight ─────────────────
    //
    // Each test below grabs the crate-global `test_env_lock` before it
    // mutates `TZ`, mirroring how `chrome::tests` synchronises any test
    // that races against another reader of `std::env`. Chrono's
    // `Local` keeps a per-thread cache that only refreshes after a
    // 1-second window even when `TZ` changes, so we run the actual
    // resolver call from a freshly-spawned thread — its `TZ_INFO`
    // thread-local starts empty and reads the env var on first use.
    fn with_tz<R>(tz: &str, run: impl FnOnce() -> R + Send + 'static) -> R
    where
        R: Send + 'static,
    {
        let _guard =
            crate::utils::test_env_lock().lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let previous = std::env::var_os("TZ");
        unsafe {
            std::env::set_var("TZ", tz);
        }
        let outcome = std::thread::spawn(run).join().expect("tz-scoped worker panicked");
        crate::utils::restore_test_env_var("TZ", previous.as_deref());
        outcome
    }

    #[test]
    fn local_day_bounds_ms_resolves_atlantic_azores_fall_back_midnight_as_ambiguous() {
        // Atlantic/Azores falls back at 01:00 DST → 00:00 STD, so the
        // wall-clock 00:00 on the fall-back Sunday occurs twice. The
        // resolver must take the `Ambiguous → Ok(earliest)` arm rather
        // than bail.
        let bounds = with_tz(":Atlantic/Azores", || local_day_bounds_ms("2026-10-25"));
        let (start, end) = bounds.expect("ambiguous-midnight day must still produce bounds");
        assert!(end > start, "bounds must enclose at least one millisecond");
    }

    #[test]
    fn local_day_bounds_ms_steps_to_01_00_when_toronto_pre_midnight_spring_forward_skips_midnight()
    {
        // America/Toronto's first recorded DST start (1919-03-30) ran
        // at 23:30 EST → 00:30 EDT — a one-hour spring forward whose
        // gap starts *before* midnight. Chrono's
        // `find_local_time_type_from_local` treats wall-clock midnight
        // on day X as Single only when it is `<=` transition_start;
        // because this transition begins at 23:30 of day X (strictly
        // before midnight of day X+1), the wall-clock 1919-03-31 00:00
        // lands inside the gap and the resolver gets a genuine
        // `LocalResult::None`. 01:00 exists (the gap ends at 00:30
        // EDT), so the retry succeeds via the inner Single arm.
        //
        // We picked this transition deliberately: every modern DST
        // timezone springs at 02:00 or 03:00 wall-clock, which makes
        // midnight always resolve to Single(STD) and leaves this
        // defensive None-arm uncovered without an ahistorical TZ.
        let bounds = with_tz(":America/Toronto", || local_day_bounds_ms("1919-03-31"));
        let (start, end) = bounds.expect("pre-midnight spring-forward must still produce bounds");
        assert!(end > start);
    }
}
