//! Epoch conversions between Unix and Chrome time.
//!
//! Chrome stores `last_visit_time` and `visit_time` as microseconds since
//! `1601-01-01T00:00:00Z` (the Windows NT epoch). PathKeep canonicalizes to
//! Unix milliseconds. Fixture authors think in Unix ms; this module bridges
//! the two without leaking raw offset arithmetic into call sites.

/// Microseconds between the Windows NT epoch (1601-01-01) and the Unix epoch.
///
/// This matches `vault_core::utils::CHROME_UNIX_EPOCH_OFFSET_MICROS` and the
/// constant inside `browser_history_parser::chromium`. Keeping a local copy
/// avoids a runtime dependency on either crate while staying numerically
/// pinned to their behavior; tests that read fixture files through the parser catch any
/// divergence.
const CHROME_UNIX_EPOCH_OFFSET_MICROS: i64 = 11_644_473_600_000_000;

/// Converts Unix milliseconds into Chrome's microseconds-since-1601 format.
///
/// Saturating arithmetic mirrors the production helper so absurd far-future
/// inputs do not silently wrap negative.
pub fn unix_ms_to_chrome_time(unix_ms: i64) -> i64 {
    unix_ms.saturating_mul(1_000).saturating_add(CHROME_UNIX_EPOCH_OFFSET_MICROS)
}

/// Converts Chrome microseconds-since-1601 back into Unix milliseconds.
///
/// The inverse of [`unix_ms_to_chrome_time`] for positive Unix timestamps, for tests that
/// compare a fixture's times with what the parser read.
///
/// Mirrors the production parser's `.max(0)` clamp at
/// `browser-history-parser/src/chromium/mod.rs:290` so any pre-1970 chrome
/// timestamp (negative-after-offset-subtraction) lands as 0 — keeping
/// fixture-side verification helpers aligned with how production stores
/// the value, even though the inverse is no longer total across i64.
pub fn chrome_time_to_unix_ms(chrome_micros: i64) -> i64 {
    chrome_micros.saturating_sub(CHROME_UNIX_EPOCH_OFFSET_MICROS).div_euclid(1_000).max(0)
}
