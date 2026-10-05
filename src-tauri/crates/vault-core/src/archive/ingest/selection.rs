//! Responsibilities: select profiles and preserve native cursor units.
//! Not responsible for: database ownership or batch transactions.
//! Failure modes: unreadable profiles run; raw microsecond markers are truncated.
//! Tests: core_tests and dedup_scenarios_edge_cases.
use super::*;
use browser_history_parser::{ParsedFavicon, ParsedUrl};
/// Keeps only the selected profiles that are currently readable on this host.
pub(crate) fn select_supported_profiles<'a>(
    discovered: &'a [crate::models::BrowserProfile],
    selected_profile_ids: &[String],
) -> Vec<&'a crate::models::BrowserProfile> {
    discovered
        .iter()
        .filter(|profile| profile.history_exists)
        .filter(|profile| profile.history_readable)
        .filter(|profile| {
            selected_profile_ids.iter().any(|selected| selected == &profile.profile_id)
        })
        .collect()
}

/// Explains why selected profiles were skipped before staging or ingest began.
///
/// Each skip is a coded [`crate::models::BackupWarning`] so Audit localizes it;
/// the English message stays the ledger/manifest fallback. The Safari sentence
/// is FROZEN (see `backup::staging_access_skip_warning`): the shell's
/// `isFullDiskAccessIssueMessage` keys on its marker text.
pub(crate) fn collect_skipped_profiles(
    discovered: &[crate::models::BrowserProfile],
    selected_profile_ids: &[String],
) -> Vec<crate::models::BackupWarning> {
    let mut warnings = discovered
        .iter()
        .filter(|profile| !profile.history_exists || !profile.history_readable)
        .filter(|profile| {
            selected_profile_ids
                .iter()
                .any(|selected| selected == &profile.profile_id)
        })
        .map(|profile| {
            if profile.browser_family == "safari" {
                crate::models::BackupWarning::new(
                    "safari-full-disk-access-skip",
                    format!(
                        "Skipped `{}` because Safari History.db is not readable yet. On macOS, grant Full Disk Access before the next backup.",
                        profile.profile_id
                    ),
                )
                .with_profile_id(profile.profile_id.clone())
            } else {
                crate::models::BackupWarning::new(
                    "profile-history-unreadable-skip",
                    format!(
                        "Skipped `{}` because {} is missing or unreadable at {}.",
                        profile.profile_id, profile.history_file_name, profile.profile_path
                    ),
                )
                .with_profile_id(profile.profile_id.clone())
                .with_diagnostic(format!(
                    "{} is missing or unreadable at {}",
                    profile.history_file_name, profile.profile_path
                ))
            }
        })
        .collect::<Vec<_>>();

    for selected_profile_id in selected_profile_ids {
        if !discovered.iter().any(|profile| profile.profile_id == *selected_profile_id) {
            warnings.push(
                crate::models::BackupWarning::new(
                    "profile-not-detected-skip",
                    format!(
                        "Skipped `{selected_profile_id}` because it is no longer detected on this device."
                    ),
                )
                .with_profile_id(selected_profile_id.clone()),
            );
        }
    }

    warnings
}

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
