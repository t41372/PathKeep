//! The cards that rank or count sites share one read of a range's site-day rows, kept until the
//! Insights data changes (`site_days`).
//!
//! Ways this can go wrong: a kept range is served after a build added visits to it; the cards
//! reading it disagree with each other.
use super::fixtures::{append_fixture_visit, seed_core_intelligence_fixture, sync_archive_totals};
use crate::intelligence::intelligence_rebuild::run_core_intelligence;
use crate::intelligence::{get_breadth_index, get_digest_summary, get_top_sites};
use crate::models::{
    CoreIntelligenceRebuildRequest, DateRange, ScopedDateRangeRequest, TopSitesRequest,
};
use crate::test_support::native::TestDatabase;
use chrono::{Local, NaiveDate};

fn noon(day: u32) -> i64 {
    NaiveDate::from_ymd_opt(2025, 6, day)
        .and_then(|date| date.and_hms_opt(12, 0, 0))
        .and_then(|time| time.and_local_timezone(Local).earliest())
        .expect("local time")
        .timestamp_millis()
}

/// `count` visits to `site` an hour apart from noon on `day`, ids from `first_id`.
fn visit(archive: &TestDatabase, site: &str, day: u32, count: i64, first_id: i64) {
    let site = site.to_owned();
    archive
        .write(move |s| {
            for offset in 0..count {
                let id = first_id + offset;
                let url = format!("https://{site}/page/{id}");
                append_fixture_visit(
                    s,
                    id,
                    &url,
                    "Page",
                    noon(day) + offset * 3_600_000,
                    None,
                    None,
                );
            }
            Ok(())
        })
        .unwrap();
    sync_archive_totals(archive);
}

fn visits_to(vault: &crate::Vault, range: &DateRange) -> Vec<(String, i64)> {
    get_top_sites(
        vault,
        &TopSitesRequest {
            cursor: None,
            date_range: range.clone(),
            profile_id: None,
            profile_ids: Vec::new(),
            sort_by: None,
            limit: Some(100),
        },
    )
    .unwrap()
    .items
    .into_iter()
    .map(|site| (site.registrable_domain, site.visit_count))
    .collect()
}

#[test]
fn a_kept_range_is_read_again_after_a_build_changes_it() {
    let vault = crate::Vault::open_temp().unwrap();
    let archive = TestDatabase::new(&vault, vault_store::Db::Archive);
    archive
        .write(|s| {
            seed_core_intelligence_fixture(s);
            Ok(())
        })
        .unwrap();
    visit(&archive, "alpha.dev", 3, 4, 100);
    visit(&archive, "beta.dev", 4, 2, 200);
    run_core_intelligence(&vault, &CoreIntelligenceRebuildRequest::default()).unwrap();
    let june = DateRange { start: "2025-06-01".into(), end: "2025-06-30".into() };
    assert_eq!(visits_to(&vault, &june), [("alpha.dev".into(), 4), ("beta.dev".into(), 2)]);

    // An append: more visits in the same range, and a site the range did not have.
    visit(&archive, "beta.dev", 5, 5, 300);
    visit(&archive, "gamma.dev", 6, 1, 400);
    run_core_intelligence(&vault, &CoreIntelligenceRebuildRequest::default()).unwrap();
    assert_eq!(
        visits_to(&vault, &june),
        [("beta.dev".into(), 7), ("alpha.dev".into(), 4), ("gamma.dev".into(), 1)]
    );

    // The other readers of the same rows agree with the list.
    let scope = ScopedDateRangeRequest { date_range: june, profile_id: None };
    assert_eq!(get_digest_summary(&vault, &scope).unwrap().distinct_domains.value, 3);
    assert_eq!(get_breadth_index(&vault, &scope).unwrap().concentration_domain_count, 1);
}
