//! Shared Core Intelligence utility helpers.
//!
//! ## Responsibilities
//! - Convert between local-day keys, RFC3339 timestamps, and date-range bounds
//!   for deterministic read-model queries.
//! - Centralize query-token and navigational-noise heuristics so schema
//!   bootstrap, rebuilds, and read models do not drift.
//! - Keep small derived metrics such as KPI trend math and refind counts out of
//!   route-specific modules.
//!
//! ## Not responsible for
//! - Running rebuild stages or persisting deterministic rows.
//! - Loading archive visits or applying the site dictionary.
//! - Shaping route-level `/intelligence` payloads.
//!
//! ## Dependencies
//! - Parent-module `SearchQueryKind` and `DateRange` contracts.
//! - `reqwest::Url` for URL-like query detection.
//! - SQLite read access only for bounded aggregate helpers.
//!
//! ## Performance notes
//! - Tokenization and date helpers are pure, allocation-bounded utilities.
//! - `count_refind_pages_in_range` stays on indexed range filters instead of
//!   materializing refind rows in memory.

use super::SearchQueryKind;
use crate::models::DateRange;
use anyhow::{Context, Result};
use chrono::{Datelike, Duration, Local, LocalResult, NaiveDate, TimeZone, Utc};
use reqwest::Url;
use rusqlite::{Connection, params};
use std::collections::HashSet;

/// Returns the local calendar day key used by deterministic daily rollups.
pub(super) fn local_date_key(visit_time_ms: i64) -> String {
    local_datetime_from_millis(visit_time_ms).format("%Y-%m-%d").to_string()
}

/// Converts a local-epoch millisecond timestamp into a stable UTC RFC3339
/// string for payloads that expose exact event time.
pub(super) fn rfc3339_from_millis(visit_time_ms: i64) -> String {
    local_datetime_from_millis(visit_time_ms).with_timezone(&Utc).to_rfc3339()
}

/// Resolves an archive visit timestamp into the local timezone even when the
/// host timezone rules produce ambiguous edges.
pub(super) fn local_datetime_from_millis(visit_time_ms: i64) -> chrono::DateTime<Local> {
    match Local.timestamp_millis_opt(visit_time_ms) {
        LocalResult::Single(value) => value,
        _ => Local::now(),
    }
}

/// Expands a user-facing inclusive date range into `[start_ms, end_ms)` bounds
/// so SQL filters can stay exact and timezone-safe.
pub(super) fn date_range_bounds(range: &DateRange) -> Result<(i64, i64)> {
    let start = NaiveDate::parse_from_str(&range.start, "%Y-%m-%d")
        .with_context(|| format!("parsing start date {}", range.start))?;
    let end = NaiveDate::parse_from_str(&range.end, "%Y-%m-%d")
        .with_context(|| format!("parsing end date {}", range.end))?;
    let start_dt = resolve_local_date(start)?;
    let end_dt = resolve_local_date(end.succ_opt().unwrap_or(end))?;
    Ok((start_dt.timestamp_millis(), end_dt.timestamp_millis()))
}

/// Tokenizes a normalized query into similarity-ready terms while dropping
/// obvious stop words and URL noise.
pub(super) fn tokenize_query_terms(query: &str) -> Vec<String> {
    let stop_words = [
        "a", "an", "and", "com", "edu", "for", "from", "how", "html", "http", "https", "in", "is",
        "net", "of", "on", "org", "the", "to", "what", "when", "where", "with", "www",
    ];
    query
        .split(|ch: char| !ch.is_alphanumeric() && !is_cjk_like(ch))
        .filter_map(|token| {
            let token = token.trim().to_lowercase();
            if token.is_empty() || stop_words.contains(&token.as_str()) {
                None
            } else {
                Some(token)
            }
        })
        .collect()
}

/// Builds the set-based representation used by query-family similarity and
/// related search-surface dedup heuristics.
pub(super) fn query_token_set(query: &str) -> HashSet<String> {
    tokenize_query_terms(query).into_iter().collect()
}

/// Distinguishes keyword queries from navigational noise without requiring any
/// LLM interpretation or cross-surface special cases.
pub(super) fn classify_search_query_kind(
    raw_query: &str,
    normalized_query: &str,
    landing_domain: Option<&str>,
) -> SearchQueryKind {
    if normalized_query.trim().is_empty() {
        return SearchQueryKind::Navigational;
    }

    if let Some(candidate_domain) =
        query_domain_candidate(raw_query).or_else(|| query_domain_candidate(normalized_query))
    {
        let landing_matches = landing_domain.is_none_or(|domain| {
            domain == candidate_domain
                || landing_domain_matches_candidate(domain, &candidate_domain)
        });
        if landing_matches {
            return SearchQueryKind::Navigational;
        }
    }

    SearchQueryKind::Keyword
}

/// Computes set overlap for the deterministic query-family heuristic.
pub(super) fn jaccard(left: &HashSet<String>, right: &HashSet<String>) -> f32 {
    if left.is_empty() || right.is_empty() {
        return 0.0;
    }
    let intersection = left.intersection(right).count() as f32;
    let union = left.union(right).count() as f32;
    if union == 0.0 { 0.0 } else { intersection / union }
}

/// Shifts a date range backwards by its own inclusive span so KPI comparisons
/// use the immediately preceding window.
pub(super) fn previous_date_range(range: &DateRange) -> Result<DateRange> {
    let start = NaiveDate::parse_from_str(&range.start, "%Y-%m-%d")?;
    let end = NaiveDate::parse_from_str(&range.end, "%Y-%m-%d")?;
    let days = (end - start).num_days().max(0) + 1;
    let prev_end = start - Duration::days(1);
    let prev_start = prev_end - Duration::days(days - 1);
    Ok(DateRange {
        start: prev_start.format("%Y-%m-%d").to_string(),
        end: prev_end.format("%Y-%m-%d").to_string(),
    })
}

/// Counts refind pages that overlap the supplied local date range without
/// materializing page payloads.
pub(super) fn count_refind_pages_in_range(
    connection: &Connection,
    range: &DateRange,
    profile_id: Option<&str>,
) -> Result<i64> {
    let (start_ms, end_ms) = date_range_bounds(range)?;
    connection
        .query_row(
            "SELECT COUNT(*)
             FROM refind_pages
             WHERE (?1 IS NULL OR profile_id = ?1)
               AND last_seen_ms >= ?2
               AND first_seen_ms < ?3",
            params![profile_id, start_ms, end_ms],
            |row| row.get(0),
        )
        .map_err(Into::into)
}

/// Builds the canonical KPI delta object shared by summary surfaces and
/// regression assertions.
pub(super) fn build_kpi(current: i64, previous: i64) -> crate::models::KpiMetric {
    let trend = if current > previous {
        "up"
    } else if current < previous {
        "down"
    } else {
        "flat"
    };
    let change_percent = if previous == 0 {
        None
    } else {
        Some(((current - previous) as f32 / previous as f32) * 100.0)
    };
    crate::models::KpiMetric {
        value: current,
        previous_value: Some(previous),
        change_percent,
        trend: trend.to_string(),
    }
}

/// Collapses exact day keys into the user-requested reporting granularity while
/// preserving existing week/month/year bucket grammar.
pub(super) fn collapse_date_key(date_key: &str, granularity: &str) -> String {
    match granularity {
        "week" => {
            if let Ok(date) = NaiveDate::parse_from_str(date_key, "%Y-%m-%d") {
                let iso = date.iso_week();
                format!("{}-W{:02}", iso.year(), iso.week())
            } else {
                date_key.to_string()
            }
        }
        "month" => date_key.get(0..7).unwrap_or(date_key).to_string(),
        "year" => date_key.get(0..4).unwrap_or(date_key).to_string(),
        _ => date_key.to_string(),
    }
}

fn resolve_local_date(date: NaiveDate) -> Result<chrono::DateTime<Local>> {
    let naive = date.and_hms_opt(0, 0, 0).expect("midnight");
    match Local.from_local_datetime(&naive) {
        LocalResult::Single(value) => Ok(value),
        LocalResult::Ambiguous(first, _) => Ok(first),
        LocalResult::None => anyhow::bail!("could not resolve local midnight for {}", date),
    }
}

fn landing_domain_matches_candidate(landing_domain: &str, candidate_domain: &str) -> bool {
    landing_domain == candidate_domain
        || landing_domain.ends_with(&format!(".{candidate_domain}"))
        || candidate_domain.ends_with(&format!(".{landing_domain}"))
}

fn query_domain_candidate(query: &str) -> Option<String> {
    let trimmed = query.trim().trim_matches('"').trim_matches('\'');
    if trimmed.is_empty() || trimmed.contains(char::is_whitespace) {
        return None;
    }

    let parsed = Url::parse(trimmed).ok().or_else(|| {
        if trimmed.contains('.') { Url::parse(&format!("https://{trimmed}")).ok() } else { None }
    })?;
    let host = parsed.host_str()?.trim_end_matches('.').to_ascii_lowercase();
    if host.is_empty() {
        return None;
    }

    Some(crate::visit_taxonomy::registrable_domain_for_host(&host))
}

fn is_cjk_like(ch: char) -> bool {
    matches!(
        ch as u32,
        0x3400..=0x4DBF | 0x4E00..=0x9FFF | 0x3040..=0x30FF | 0xAC00..=0xD7AF
    )
}
