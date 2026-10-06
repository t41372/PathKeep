//! Range-scoped frequent searches (`get_frequent_searches`).
//!
//! ## Responsibilities
//! - Check counts against a hand-built archive: searches on the first and last millisecond of
//!   the range count, searches one millisecond outside do not.
//! - Check grouping (spellings that normalize alike are one query, shown with the newest
//!   spelling), the keyword-only rule, the profile filter and the limit.
//! - Check that counts never exceed the digest's search total for the same range, which is the
//!   bug this read replaced (all-time query-family counts shown against a 30-day total).
//!
//! Failure modes: boundary searches leak, normalized spellings split counts, profiles mix,
//! or the range query scans all historical events.
//!
//! ## Not responsible for
//! - Speed at scale; `archive_scale_bench` measures that.

use super::super::{
    get_digest_summary, get_frequent_searches, intelligence_rebuild::run_core_intelligence,
};
use super::fixtures::{append_fixture_visit, seed_core_intelligence_fixture};
use crate::models::{
    AppConfig, ArchiveMode, CoreIntelligenceRebuildRequest, DateRange, FrequentSearch,
    FrequentSearchesRequest, ScopedDateRangeRequest,
};
use chrono::{Local, NaiveDate, TimeZone};

const HOUR_MS: i64 = 3_600_000;

fn local_midnight_ms(date: &str) -> i64 {
    let naive = NaiveDate::parse_from_str(date, "%Y-%m-%d")
        .expect("date")
        .and_hms_opt(0, 0, 0)
        .expect("midnight");
    Local.from_local_datetime(&naive).earliest().expect("local midnight").timestamp_millis()
}

fn range() -> DateRange {
    DateRange { start: "2024-04-10".to_string(), end: "2024-04-12".to_string() }
}

fn config() -> AppConfig {
    AppConfig { initialized: true, archive_mode: ArchiveMode::Plaintext, ..AppConfig::default() }
}

/// Builds an archive whose searches sit on and just outside the edges of [`range`], then runs
/// the real intelligence rebuild over it.
fn searched_archive() -> crate::Vault {
    let vault = crate::Vault::open_temp().unwrap().with_config(config());
    vault
        .write(vault_store::Db::Archive, vault_store::Priority::Batch, |archive| {
            seed_core_intelligence_fixture(archive);
            let start = local_midnight_ms("2024-04-10");
            let end = local_midnight_ms("2024-04-13");
            let searches: [(i64, &str); 9] = [
                (start - 1, "rust ownership"),
                (start, "rust ownership"),
                (start + HOUR_MS, "Rust  Ownership"),
                (start + 2 * HOUR_MS, "sqlite wal"),
                (start + 3 * HOUR_MS, "tokio select"),
                (start + 4 * HOUR_MS, "https://asu.edu"),
                (end - HOUR_MS, "sqlite wal"),
                (end - 1, "SQLite WAL"),
                (end, "sqlite wal"),
            ];
            for (index, (time, query)) in searches.iter().enumerate() {
                let visit_id = 100 + index as i64;
                append_fixture_visit(
                    archive,
                    visit_id,
                    &format!("https://www.google.com/search?q={}", query.replace(' ', "+")),
                    &format!("{query} - Google Search"),
                    *time,
                    None,
                    Some(query),
                );
            }
            Ok(())
        })
        .unwrap();
    super::fixtures::sync_archive_totals(&crate::test_support::native::TestDatabase::new(
        &vault,
        vault_store::Db::Archive,
    ));
    run_core_intelligence(&vault, &CoreIntelligenceRebuildRequest::default())
        .expect("rebuild intelligence");
    vault
}

fn frequent(
    paths: &crate::Vault,
    profile_id: Option<&str>,
    limit: u32,
) -> crate::models::IntelligencePage<FrequentSearch> {
    get_frequent_searches(
        paths,
        &FrequentSearchesRequest {
            cursor: None,
            date_range: range(),
            profile_id: profile_id.map(str::to_string),
            limit: Some(limit),
        },
    )
    .expect("frequent searches")
}

fn counts(rows: &[FrequentSearch]) -> Vec<(&str, i64)> {
    rows.iter().map(|row| (row.normalized_query.as_str(), row.search_count)).collect()
}

#[test]
fn counts_only_the_keyword_searches_inside_the_range() {
    let paths = searched_archive();

    let rows = frequent(&paths, None, 10);
    assert_eq!(
        counts(&rows),
        vec![
            // 10 Apr 02:00, 12 Apr 23:00 and 23:59:59.999 count; 13 Apr 00:00 does not.
            ("sqlite wal", 3),
            // 10 Apr 00:00:00.000 and 01:00 (spelled differently) count; 9 Apr 23:59:59.999
            // does not.
            ("rust ownership", 2),
            ("tokio select", 1),
        ],
        "the navigational https://asu.edu search must not appear"
    );

    let digest = get_digest_summary(
        &paths,
        &ScopedDateRangeRequest { date_range: range(), profile_id: None },
    )
    .expect("digest");
    let total: i64 = rows.iter().map(|row| row.search_count).sum();
    assert!(
        total <= digest.total_searches.value,
        "frequent searches add up to {total}, more than the {} searches in the same range",
        digest.total_searches.value
    );
}

#[test]
fn honours_the_limit_and_the_profile_filter() {
    let paths = searched_archive();

    let top = frequent(&paths, None, 1);
    assert_eq!(counts(&top), vec![("sqlite wal", 3)]);
    assert_eq!(frequent(&paths, Some("chrome:Default"), 10).len(), 3);
    assert!(frequent(&paths, Some("firefox:other"), 10).is_empty());
}

/// The rebuild stores queries lower-cased today, but `raw_query` is what the user typed, and the
/// card shows it. When spellings differ, the newest one is shown.
#[test]
fn shows_the_newest_spelling_of_each_query() {
    let paths = searched_archive();
    let start = local_midnight_ms("2024-04-10");
    let intelligence =
        crate::test_support::native::TestDatabase::new(&paths, vault_store::Db::Intelligence);
    intelligence
        .execute_batch(&format!(
            "UPDATE intelligence_generations SET navigation=0; DELETE FROM search_events_rows;
             INSERT INTO search_events_rows
               (generation, visit_id, profile_id, search_engine, raw_query, normalized_query, query_kind,
                computed_at, visit_time_ms)
             VALUES
               (0, 1, 'chrome:Default', 'google', 'Tokio Select', 'tokio select', 'keyword', '', {}),
               (0, 2, 'chrome:Default', 'bing', 'TOKIO select', 'tokio select', 'keyword', '', {}),
               (0, 3, 'chrome:Default', 'google', 'tokio  SELECT', 'tokio select', 'keyword', '', {});",
            start + HOUR_MS,
            start + 3 * HOUR_MS,
            start + 2 * HOUR_MS,
        ))
        .expect("search events");
    drop(intelligence);

    let rows = frequent(&paths, None, 10);
    assert_eq!(rows.len(), 1, "one query across engines and spellings: {rows:?}");
    assert_eq!(rows[0].query, "TOKIO select");
    assert_eq!(rows[0].search_count, 3);
}

#[test]
fn the_range_read_walks_the_kind_and_time_index() {
    let paths = searched_archive();
    paths.store().capture_query_plans(vault_store::Db::Intelligence, true).unwrap();
    frequent(&paths, None, 10);
    let plans = paths.store().take_query_plans(vault_store::Db::Intelligence).unwrap();
    let plan = format!("{plans:?}");
    assert!(plan.contains("idx_search_events_kind_time"), "range must seek its index: {plan}");
}
