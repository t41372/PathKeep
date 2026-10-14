//! Responsibilities: prove that the Insights reads with a fixed cost per range seek their
//! derived tables instead of reading them whole.
//! Not responsible for: results (the read model tests cover those) or paging (keyset plans).
//! Failure modes: a read through a generation view, or a filter written `?1 IS NULL OR ...`
//! in front of the indexed column, makes Turso scan the whole table. At 14.4M visits the KPI
//! row (`get_digest_summary`) took 126 s for 30 days that way. Searches were not in the
//! benchmark until 2026-10-09; with them What you search for took 2.5 s for 30 days, and
//! trails, friction and families as long for any range: they read more than the range.
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
    "search_events_rows",
    "search_events",
    "search_event_terms_rows",
    "search_event_terms",
    "search_trail_members_rows",
    "search_trail_members",
    "query_families_rows",
    "query_families",
    "query_family_queries_rows",
    "query_family_queries",
    "reopened_investigations_rows",
    "reopened_investigations",
    "path_flows_rows",
    "path_flows",
    "compare_sets_rows",
    "compare_sets",
    "site_flows_rows",
    "visit_assignments",
    "session_domains",
];

/// Columns that only narrow a table to one build, one profile or one of a few kinds: a search
/// on these alone reads every row they hold.
const BROAD: &[&str] = &[
    "generation",
    "profile_id",
    "query_kind",
    "search_engine",
    "kind",
    "step_count",
    "page_category",
];

/// Rollups with a row per day (and category or engine) per profile: cheap to read whole.
const PER_DAY: &[&str] =
    &["daily_summary_rollups_rows", "category_daily_rollups_rows", "engine_daily_rollups_rows"];

/// Indexes a list walks in its own order and stops at a page, on purpose (`store-schema.md`):
/// pages by score, since the pages reopened most are the ones in use (about 1 ms for 30 days or a
/// year at 14.4M visits); sorting a year's pages instead would read hundreds of thousands.
const RANK_WALKS: &[&str] = &["idx_refind_pages_score"];

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

/// No large table is scanned, and none is searched on its generation, profile and kinds alone:
/// that reads every row a profile's build wrote, which is what a read through a generation view
/// does.
fn assert_no_scans(plans: &[QueryPlan]) {
    let scans = whole_reads(plans);
    assert!(scans.is_empty(), "large tables read whole:\n{}", scans.join("\n"));
}

/// The plan steps that read a large table whole, each with its statement.
fn whole_reads(plans: &[QueryPlan]) -> Vec<String> {
    plans
        .iter()
        .flat_map(|plan| {
            plan.steps
                .iter()
                .filter(|step| {
                    let large =
                        LARGE.iter().any(|table| step.split_whitespace().nth(1) == Some(table));
                    let scanned = step.starts_with("SCAN ")
                        && large
                        && !SMALL_INDEXES.iter().any(|index| step.contains(index))
                        && !RANK_WALKS.iter().any(|index| step.contains(index));
                    let whole_generation = step.ends_with("(generation=?)")
                        && !PER_DAY.iter().any(|table| step.contains(table));
                    let broad = step.starts_with("SEARCH ")
                        && large
                        && step
                            .rsplit_once('(')
                            .and_then(|(_, constraints)| constraints.strip_suffix(')'))
                            .is_none_or(|constraints| {
                                constraints.split(" AND ").all(|constraint| {
                                    BROAD.iter().any(|column| constraint.trim().starts_with(column))
                                })
                            });
                    scanned || whole_generation || broad
                })
                .map(move |step| format!("{step}\n    in {}", plan.sql))
        })
        .collect()
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

/// A month of mornings in one profile: a search, a rewording, the pages they opened.
fn archive_with_searches() -> Vault {
    let vault = Vault::open_temp().unwrap();
    vault
        .store()
        .write(Db::Archive, vault_store::Priority::Batch, |s| {
            s.execute("INSERT INTO source_profiles(id,browser_kind,profile_name,profile_path,profile_key,discovered_at) VALUES(1,'chrome','Default','/fixture','chrome:Default','')", ())?;
            let urls = [
                "https://www.google.com/search?q=rust+tokio+runtime",
                "https://www.google.com/search?q=rust+tokio+runtime+v2",
                "https://docs.rs/tokio/latest/tokio/runtime/",
                "https://github.com/tokio-rs/tokio/issues/1",
                "https://github.com/tokio-rs/tokio/issues/2",
            ];
            for (index, url) in urls.iter().enumerate() {
                let id = index as i64 + 1;
                let domain = url.split('/').nth(2).unwrap().trim_start_matches("www.");
                s.execute("INSERT INTO pages(id,url,canonical_url,registrable_domain) VALUES(?1,?2,?2,?3)", vault_store::params![id, *url, domain])?;
                s.execute("INSERT INTO urls(id,page_id,url,source_profile_id,created_by_run_id,first_visit_ms,first_visit_iso,last_visit_ms,last_visit_iso) VALUES(?1,?1,?2,1,0,0,'',0,'')", vault_store::params![id, *url])?;
            }
            let mut id = 0_i64;
            for day in 0..30_i64 {
                let morning = 1_767_254_400_000 + day * 86_400_000 + 9 * 3_600_000;
                for (step, url) in [1, 2, 3, 4, 5, 4].into_iter().enumerate() {
                    id += 1;
                    let from = if step < 2 { 0 } else { id - 1 };
                    s.execute("INSERT INTO visits(id,url_id,source_profile_id,created_by_run_id,visit_time_ms,visit_time_iso,from_visit,source_visit_id) VALUES(?1,?2,1,0,?3,'',?4,CAST(?1 AS TEXT))", vault_store::params![id, url, morning + step as i64 * 60_000, from])?;
                }
            }
            s.execute("UPDATE archive_totals SET value=?1 WHERE name='visits'", [id])?;
            s.execute("INSERT INTO archive_totals(name,value) VALUES('sourceVisits:1',?1)", [id])?;
            Ok(())
        })
        .unwrap();
    vault_core::run_core_intelligence(
        &vault,
        &vault_core::CoreIntelligenceRebuildRequest::default(),
    )
    .unwrap();
    vault
}

/// Every Insights read, for 30 days and for all time, across profiles and in one: none reads a
/// large table past its range.
#[test]
fn every_insights_read_seeks_its_tables() {
    let vault = archive_with_searches();
    let mut whole = Vec::new();
    for (start, end) in [("2026-01-01", "2026-01-30"), ("1900-01-01", "2026-01-30")] {
        for profile in [None, Some("chrome:Default")] {
            let request = serde_json::json!({
                "dateRange": { "start": start, "end": end },
                "profileId": profile,
                "granularity": "day",
                "pageSize": 20,
                "stepCount": 3,
                "limit": 20,
                "sortBy": "visits",
            });
            macro_rules! read {
                ($($call:path),* $(,)?) => {
                    $($call(&vault, &serde_json::from_value(request.clone()).unwrap()).unwrap();)*
                };
            }
            let captured = plans(&vault, || {
                read!(
                    vault_core::intelligence::get_intelligence_primary_overview,
                    vault_core::intelligence::get_intelligence_secondary_overview,
                    vault_core::get_digest_summary,
                    vault_core::get_discovery_trend,
                    vault_core::get_activity_mix_trend,
                    vault_core::get_top_sites,
                    vault_core::get_browsing_rhythm,
                    vault_core::get_frequent_searches,
                    vault_core::get_refind_pages,
                    vault_core::get_reopened_investigations,
                    vault_core::get_breadth_index,
                    vault_core::get_habit_patterns,
                    vault_core::get_multi_browser_diff,
                    vault_core::get_activity_mix,
                    vault_core::get_stable_sources,
                    vault_core::get_search_engine_ranking,
                    vault_core::get_top_search_concepts,
                    vault_core::get_query_families,
                    vault_core::get_search_effectiveness,
                    vault_core::get_friction_signals,
                    vault_core::get_interrupted_habits,
                    vault_core::get_path_flows,
                    vault_core::get_compare_sets,
                    vault_core::get_observed_interactions,
                    vault_core::get_search_trails,
                );
            });
            assert!(
                captured.iter().any(|plan| plan.sql.contains("search_event_terms")),
                "the search reads ran"
            );
            for read in whole_reads(&captured) {
                let read = format!("{start}..{end} {profile:?}: {read}");
                if !whole.contains(&read) {
                    whole.push(read);
                }
            }
        }
    }
    assert!(whole.is_empty(), "large tables read whole:\n{}", whole.join("\n"));
}
