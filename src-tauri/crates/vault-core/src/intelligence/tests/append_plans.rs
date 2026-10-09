//! An append updates the aggregates in place by key: it looks up, deletes and writes one page,
//! family, compare set or flow at a time, so every one of those must seek its key.
//!
//! At 14.4M visits a 10,000-visit append looked up and replaced 5,378 pages' refind rows; a
//! lookup planned without the page's key reads every refind row of the profile, and the whole
//! step took hours instead of seconds. Failure modes: a statement on a large table planned as a
//! scan, or as a search on the generation and profile alone; either reads the whole table once
//! per page. Deletes and updates are checked too (`vault_store` captures their plans).
use super::append_aggregates::{add, build, vault, visits};
use super::fixtures::seed_core_intelligence_fixture;
use crate::test_support::native::TestDatabase;
use vault_store::Db;

/// Tables with a row per visit, page, session, trail, family or flow.
const LARGE: &[&str] = &[
    "visit_derived_facts_rows",
    "visit_assignments",
    "sessions_rows",
    "session_paths",
    "search_trails_rows",
    "search_trail_members_rows",
    "search_events_rows",
    "refind_pages_rows",
    "reopened_investigations_rows",
    "query_families_rows",
    "query_family_queries_rows",
    "compare_sets_rows",
    "path_flows_rows",
    "site_flows_rows",
];

/// Columns that only narrow a table to one build or one profile, never to one row.
const BROAD: &[&str] = &["generation", "profile_id"];

/// A plan step that reads a large table without a key: a scan, or a search constrained only
/// by the build and the profile. A read of the top rows in an index's order (the overview the
/// build warms afterwards) stops at its `LIMIT`, so it does not count.
fn reads_whole(step: &str, sql: &str) -> bool {
    let mut words = step.split_whitespace();
    let (Some(verb), Some(table)) = (words.next(), words.next()) else { return false };
    if !LARGE.contains(&table) {
        return false;
    }
    let top_rows =
        step.contains(" USING INDEX ") && sql.contains("ORDER BY") && sql.contains("LIMIT");
    match verb {
        "SCAN" => !top_rows,
        "SEARCH" => step
            .rsplit_once('(')
            .and_then(|(_, constraints)| constraints.strip_suffix(')'))
            .is_none_or(|constraints| {
                constraints.split(" AND ").all(|constraint| {
                    BROAD.iter().any(|column| constraint.trim().starts_with(column))
                })
            }),
        _ => false,
    }
}

#[test]
fn an_append_reads_and_writes_the_aggregates_by_key() {
    let all = visits();
    let cut = all.len() * 3 / 5 + 1;
    let appended = vault();
    let archive = TestDatabase::new(&appended, Db::Archive);
    archive
        .write(|s| {
            seed_core_intelligence_fixture(s);
            Ok(())
        })
        .unwrap();
    add(&archive, &all[..cut]);
    assert_eq!(build(&appended), "fallback-full");
    add(&archive, &all[cut..]);

    let store = appended.store();
    store.capture_query_plans(Db::Intelligence, true).unwrap();
    assert_eq!(build(&appended), "incremental");
    store.capture_query_plans(Db::Intelligence, false).unwrap();
    let plans = store.take_query_plans(Db::Intelligence).unwrap();
    assert!(!plans.is_empty());
    let whole = plans
        .iter()
        .flat_map(|plan| {
            plan.steps
                .iter()
                .filter(|step| reads_whole(step, &plan.sql))
                .map(move |step| format!("{step}\n    in {}", plan.sql))
        })
        .collect::<Vec<_>>();
    assert!(whole.is_empty(), "large tables read without a key:\n{}", whole.join("\n"));
}
