//! "Pages reopened" in a range: the refind pages opened on one of its days, the pages the day
//! page lists.
//!
//! Failure modes: a page whose first and last visit span the range counts though nothing opened
//! it then (a day with no visits at all showed 58); a page that qualifies by typed revisits on
//! one day is left out because it was not opened on two days; a page opened once counts; the
//! count and the day page's list disagree.

use super::append_aggregates::build;
use super::fixtures::{append_fixture_visit, seed_core_intelligence_fixture, sync_archive_totals};
use crate::models::{
    AppConfig, ArchiveMode, DateRange, DayInsightsRequest, ScopedDateRangeRequest,
};
use crate::test_support::native::TestDatabase;
use vault_store::Db;

const DAY_MS: i64 = 86_400_000;
/// Noon UTC on 2024-04-10, after the seed fixture's visits.
const DAY_10: i64 = 1_712_750_400_000;

fn day(n: i64) -> String {
    chrono::DateTime::from_timestamp_millis(DAY_10 + (n - 10) * DAY_MS)
        .unwrap()
        .format("%Y-%m-%d")
        .to_string()
}

fn reopened(vault: &crate::Vault, from: i64, to: i64) -> i64 {
    super::super::get_digest_summary(
        vault,
        &ScopedDateRangeRequest {
            date_range: DateRange { start: day(from), end: day(to) },
            profile_id: None,
        },
    )
    .unwrap()
    .refind_pages
    .value
}

fn listed(vault: &crate::Vault, on: i64) -> Vec<String> {
    let mut urls = super::super::get_day_insights(
        vault,
        &DayInsightsRequest { date: day(on), profile_id: None },
    )
    .unwrap()
    .refind_pages
    .items
    .into_iter()
    .map(|page| page.url)
    .collect::<Vec<_>>();
    urls.sort();
    urls
}

#[test]
fn reopened_pages_are_the_refind_pages_opened_in_the_range() {
    let vault = crate::Vault::open_temp().unwrap().with_config(AppConfig {
        initialized: true,
        archive_mode: ArchiveMode::Plaintext,
        insights_time_zone: Some("UTC".into()),
        ..AppConfig::default()
    });
    let archive = TestDatabase::new(&vault, Db::Archive);
    archive
        .write(|s| {
            seed_core_intelligence_fixture(s);
            let at = |n: i64, minute: i64| DAY_10 + (n - 10) * DAY_MS + minute * 60_000;
            // Opened on day 10 and day 14, nothing on days 11 to 13.
            append_fixture_visit(s, 5_001, "https://docs.example/a", "A", at(10, 0), Some(1), None);
            append_fixture_visit(s, 5_002, "https://docs.example/a", "A", at(14, 0), Some(1), None);
            // Typed in twice on day 14 (nothing opened it): a refind page on one day.
            append_fixture_visit(
                s,
                5_003,
                "https://typed.example/b",
                "B",
                at(14, 5),
                Some(0),
                None,
            );
            append_fixture_visit(
                s,
                5_004,
                "https://typed.example/b",
                "B",
                at(14, 9),
                Some(0),
                None,
            );
            // Opened once.
            append_fixture_visit(s, 5_005, "https://once.example/c", "C", at(14, 7), Some(1), None);
            Ok(())
        })
        .unwrap();
    sync_archive_totals(&archive);
    drop(archive);
    build(&vault);

    assert_eq!(reopened(&vault, 12, 12), 0, "a day with no visits, inside A's span");
    assert_eq!(reopened(&vault, 11, 13), 0, "A spans these days but was not opened on them");
    assert_eq!(reopened(&vault, 10, 10), 1, "A");
    assert_eq!(listed(&vault, 10), vec!["https://docs.example/a"]);
    assert_eq!(reopened(&vault, 14, 14), 2, "A, and B by its typed revisits; not C");
    assert_eq!(listed(&vault, 14), vec!["https://docs.example/a", "https://typed.example/b"]);
    assert_eq!(reopened(&vault, 10, 14), 2, "A counts once over the days it was opened on");
}
