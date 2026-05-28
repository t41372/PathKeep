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
/// pinned to their behavior; the round-trip test catches any divergence.
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
/// The inverse of [`unix_ms_to_chrome_time`] for positive Unix timestamps;
/// used by round-trip tests to assert the fixture writer and the production
/// parser agree on the epoch.
///
/// Mirrors the production parser's `.max(0)` clamp at
/// `browser-history-parser/src/chromium/mod.rs:290` so any pre-1970 chrome
/// timestamp (negative-after-offset-subtraction) lands as 0 — keeping
/// fixture-side verification helpers aligned with how production stores
/// the value, even though the inverse is no longer total across i64.
pub fn chrome_time_to_unix_ms(chrome_micros: i64) -> i64 {
    chrome_micros.saturating_sub(CHROME_UNIX_EPOCH_OFFSET_MICROS).div_euclid(1_000).max(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unix_to_chrome_and_back_round_trips() {
        let unix_ms = 1_700_000_000_000_i64; // 2023-11-14T22:13:20Z
        let chrome = unix_ms_to_chrome_time(unix_ms);
        assert_eq!(chrome_time_to_unix_ms(chrome), unix_ms);
    }

    #[test]
    fn unix_epoch_zero_maps_to_offset_only() {
        assert_eq!(unix_ms_to_chrome_time(0), CHROME_UNIX_EPOCH_OFFSET_MICROS);
        assert_eq!(chrome_time_to_unix_ms(CHROME_UNIX_EPOCH_OFFSET_MICROS), 0);
    }

    #[test]
    fn far_future_unix_saturates_rather_than_wraps() {
        let absurd = i64::MAX / 1_000;
        let chrome = unix_ms_to_chrome_time(absurd);
        assert_eq!(chrome, i64::MAX);
    }

    #[test]
    fn pre_unix_epoch_chrome_time_clamps_to_zero() {
        // chrome_micros = 0 represents the Windows NT epoch (1601-01-01),
        // which is well before the Unix epoch. Production parser clamps
        // such values to 0; the fixture-side inverse helper must do the
        // same so verification helpers agree with archived state.
        assert_eq!(chrome_time_to_unix_ms(0), 0);
        assert_eq!(chrome_time_to_unix_ms(CHROME_UNIX_EPOCH_OFFSET_MICROS - 1), 0);
    }
}
