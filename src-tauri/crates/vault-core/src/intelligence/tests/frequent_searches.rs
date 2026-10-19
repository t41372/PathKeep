//! The read plan of range-scoped frequent searches (`get_frequent_searches`).
//!
//! ## Responsibilities
//! - Check that the range read seeks the search-events kind and time index.
//!
//! Failure modes: the range query scans every search ever made.
//!
//! ## Not responsible for
//! - Speed at scale; `archive_scale_bench` measures that.

use super::super::{get_frequent_searches, intelligence_rebuild::run_core_intelligence};
use super::fixtures::{append_fixture_visit, seed_core_intelligence_fixture};
use crate::models::{
    AppConfig, ArchiveMode, CoreIntelligenceRebuildRequest, DateRange, FrequentSearch,
    FrequentSearchesRequest,
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

#[test]
fn the_range_read_walks_the_kind_and_time_index() {
    let paths = searched_archive();
    paths.store().capture_query_plans(vault_store::Db::Intelligence, true).unwrap();
    frequent(&paths, None, 10);
    let plans = paths.store().take_query_plans(vault_store::Db::Intelligence).unwrap();
    let plan = format!("{plans:?}");
    assert!(plan.contains("idx_search_events_kind_time"), "range must seek its index: {plan}");
}
