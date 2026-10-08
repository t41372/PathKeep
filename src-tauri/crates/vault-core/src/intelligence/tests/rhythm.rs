//! The rhythm map (`get_browsing_rhythm`) from the daily rollup's hour counts.
//!
//! Failure modes: hours counted in UTC where the visit-by-visit count uses local time, a
//! weekday taken from the wrong day, a profile filter or a range edge lost, visits counted
//! twice across profiles. The visit-by-visit count is the reference.

use super::super::{get_browsing_rhythm, intelligence_rebuild::run_core_intelligence};
use super::fixtures::{append_fixture_visit, seed_core_intelligence_fixture, sync_archive_totals};
use crate::models::{
    AppConfig, ArchiveMode, CategoryFilteredDateRangeRequest, CoreIntelligenceRebuildRequest,
    DateRange, RhythmHeatmap,
};

const HOUR_MS: i64 = 3_600_000;

fn cells(heatmap: RhythmHeatmap) -> Vec<(i64, i64, i64)> {
    let mut cells: Vec<_> =
        heatmap.cells.into_iter().map(|cell| (cell.dow, cell.hour, cell.visit_count)).collect();
    cells.sort();
    cells
}

/// Visits at different hours over two weeks, in two profiles: the map read from the daily
/// hours matches counting every visit, for every profile, one profile, and a range.
#[test]
fn the_rhythm_from_daily_hours_matches_counting_every_visit() {
    let vault = crate::Vault::open_temp().unwrap().with_config(AppConfig {
        initialized: true,
        archive_mode: ArchiveMode::Plaintext,
        ..AppConfig::default()
    });
    vault
        .write(vault_store::Db::Archive, vault_store::Priority::Batch, |archive| {
            seed_core_intelligence_fixture(archive);
            let start = 1_711_929_600_000; // 2024-04-01T00:00:00Z
            for index in 0..60_i64 {
                // Every 5 h 17 min: all hours of the day, both sides of midnight.
                let time = start + index * (5 * HOUR_MS + 17 * 60_000);
                append_fixture_visit(
                    archive,
                    200 + index,
                    &format!("https://docs.example/{}", index % 7),
                    "Docs",
                    time,
                    None,
                    None,
                );
            }
            Ok(())
        })
        .unwrap();
    sync_archive_totals(&crate::test_support::native::TestDatabase::new(
        &vault,
        vault_store::Db::Archive,
    ));
    run_core_intelligence(&vault, &CoreIntelligenceRebuildRequest::default()).expect("rebuild");

    for (profile, start, end) in [
        (None, "2024-01-01", "2024-12-31"),
        (Some("chrome:Default"), "2024-01-01", "2024-12-31"),
        (None, "2024-04-03", "2024-04-09"),
    ] {
        let request = CategoryFilteredDateRangeRequest {
            date_range: DateRange { start: start.into(), end: end.into() },
            profile_id: profile.map(Into::into),
            category: None,
        };
        let from_hours = get_browsing_rhythm(&vault, &request).unwrap();
        // A category filter takes the visit-by-visit count; no category leaves it unfiltered.
        let by_visit = vault
            .read(vault_store::Db::Intelligence, move |s| {
                super::super::intelligence_domain::rhythm_from_visits(s, &request)
            })
            .unwrap();
        assert!(!from_hours.cells.is_empty());
        assert_eq!(cells(from_hours), cells(by_visit), "{profile:?} {start}..{end}");
    }
}
