//! Responsibilities: prove that the Insights reads with a fixed cost per range seek their
//! derived tables instead of reading them whole.
//! Not responsible for: results (the read model tests cover those) or paging (keyset plans).
//! Failure modes: a read through a generation view, or a filter written `?1 IS NULL OR ...`
//! in front of the indexed column, makes Turso scan the whole table. At 14.4M visits the KPI
//! row (`get_digest_summary`) took 126 s for 30 days that way.
use vault_core::{DateRange, ScopedDateRangeRequest, Vault};
use vault_store::{Db, QueryPlan};

/// Derived tables with a row per visit, page, session, session-day or site-day, and the
/// generation views over them: never read whole. (The daily summary and category rollups hold a row per day,
/// and category, per profile, so reading them whole is cheap.)
const LARGE: &[&str] = &[
    "sessions_rows",
    "sessions",
    "session_paths",
    "refind_pages_rows",
    "refind_pages",
    "domain_daily_rollups_rows",
    "domain_daily_rollups",
    "search_trails_rows",
    "search_trails",
    "visit_derived_facts_rows",
    "visit_derived_facts",
];

/// Rollups with a row per day (and category or engine) per profile: cheap to read whole.
const PER_DAY: &[&str] =
    &["daily_summary_rollups_rows", "category_daily_rollups_rows", "engine_daily_rollups_rows"];

/// Partial indexes over a large table that hold few rows, read whole on purpose.
const SMALL_INDEXES: &[&str] = &[
    // One row per site and profile: each site's first day (`new_sites_by_day`).
    "idx_domain_daily_first_day",
];

fn plans(vault: &Vault, read: impl FnOnce()) -> Vec<QueryPlan> {
    vault.store().capture_query_plans(Db::Intelligence, true).unwrap();
    read();
    vault.store().capture_query_plans(Db::Intelligence, false).unwrap();
    vault.store().take_query_plans(Db::Intelligence).unwrap()
}

/// No large table is scanned, and none is searched on its generation alone: that reads every
/// row a profile's build wrote, which is what a read through a generation view does.
fn assert_no_scans(plans: &[QueryPlan]) {
    let scans: Vec<_> = plans
        .iter()
        .flat_map(|plan| {
            plan.steps
                .iter()
                .filter(|step| {
                    let scanned = step.starts_with("SCAN ")
                        && LARGE.iter().any(|table| step.split_whitespace().nth(1) == Some(table))
                        && !SMALL_INDEXES.iter().any(|index| step.contains(index));
                    let whole_generation = step.ends_with("(generation=?)")
                        && !PER_DAY.iter().any(|table| step.contains(table));
                    scanned || whole_generation
                })
                .map(move |step| format!("{step}\n    in {}", plan.sql))
        })
        .collect();
    assert!(scans.is_empty(), "large tables read whole:\n{}", scans.join("\n"));
}

#[test]
fn the_kpi_row_seeks_its_tables_for_every_profile_and_one() {
    let vault = Vault::open_temp().unwrap();
    for profile in [None, Some("profile")] {
        let request = ScopedDateRangeRequest {
            date_range: DateRange { start: "2026-01-01".into(), end: "2026-01-30".into() },
            profile_id: profile.map(Into::into),
        };
        let captured = plans(&vault, || {
            vault_core::get_digest_summary(&vault, &request).unwrap();
        });
        assert!(!captured.is_empty());
        assert_no_scans(&captured);
    }
}

/// The other range reads that took a minute or more for 30 days at 14.4M visits: top sites,
/// explore or focus, habits, browsers compared, common paths, and one site's page.
#[test]
fn the_slow_range_reads_seek_their_tables() {
    let vault = Vault::open_temp().unwrap();
    for profile in [None, Some("profile")] {
        let range = DateRange { start: "2026-01-01".into(), end: "2026-01-30".into() };
        let scope = ScopedDateRangeRequest {
            date_range: range.clone(),
            profile_id: profile.map(Into::into),
        };
        let captured = plans(&vault, || {
            vault_core::get_top_sites(
                &vault,
                &vault_core::TopSitesRequest {
                    date_range: range.clone(),
                    profile_id: profile.map(Into::into),
                    limit: Some(40),
                    ..Default::default()
                },
            )
            .unwrap();
            vault_core::get_breadth_index(&vault, &scope).unwrap();
            vault_core::get_habit_patterns(&vault, &scope).unwrap();
            vault_core::get_multi_browser_diff(&vault, &scope).unwrap();
            vault_core::get_path_flows(
                &vault,
                &vault_core::PathFlowRequest {
                    date_range: range.clone(),
                    profile_id: profile.map(Into::into),
                    step_count: 3,
                    limit: Some(15),
                    ..Default::default()
                },
            )
            .unwrap();
            vault_core::get_domain_deep_dive(
                &vault,
                &vault_core::DomainDeepDiveRequest {
                    date_range: range.clone(),
                    profile_id: profile.map(Into::into),
                    registrable_domain: "example.com".into(),
                    pages_cursor: None,
                    searches_cursor: None,
                    page_size: None,
                },
            )
            .unwrap();
        });
        assert_no_scans(&captured);
        // The shared site-day read walks its index in date order; a sort would hold every row.
        let sorted: Vec<_> = captured
            .iter()
            .filter(|plan| plan.sql.contains("idx_domain_daily_range"))
            .flat_map(|plan| plan.steps.iter().filter(|step| step.contains("TEMP B-TREE")))
            .collect();
        assert!(sorted.is_empty(), "site-day rows sorted: {sorted:?}");
    }
}
