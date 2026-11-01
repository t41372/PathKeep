//! Responsibilities: select profiles and preserve native cursor units.
//! Not responsible for: database ownership or batch transactions.
//! Failure modes: unreadable profiles run; raw microsecond markers are truncated.
//! Tests: dedup_scenarios* through full ingests.
use super::*;
use browser_history_parser::{ParsedFavicon, ParsedUrl};
pub(super) fn url_marker(family: &str, url: &ParsedUrl) -> i64 {
    // Prefer the parser-provided raw native timestamp so the watermark
    // preserves sub-millisecond precision: chromium's
    // `urls.last_visit_time` is microseconds-since-1601, but the old path
    // round-tripped through `last_visit_ms` (`div_euclid(1_000)` + `* 1_000`)
    // and lost up to 999 µs per URL. The truncated marker meant every
    // incremental ingest's `WHERE last_visit_time >= ?1` predicate
    // re-matched the same URL and forced a redundant upsert.
    //
    // Firefox / Safari parsers don't expose this field yet; they fall
    // through to `last_visit_ms`, which keeps the legacy behaviour and
    // the same sub-ms precision loss documented for those families in
    // BACKLOG.md. Tracking separately so the watermark unit change for
    // those parsers can ship with a one-shot migration of stored cursors.
    if let Some(native) = url.source_last_visit_marker {
        return native;
    }
    if family == "chromium" {
        unix_micros_to_chrome_time(url.last_visit_ms.saturating_mul(1_000))
    } else {
        url.last_visit_ms
    }
}

pub(super) fn favicon_marker(family: &str, favicon: &ParsedFavicon) -> i64 {
    if family == "chromium" {
        unix_micros_to_chrome_time(favicon.last_updated_ms.saturating_mul(1_000))
    } else {
        favicon.last_updated_ms
    }
}
