//! New categories, sorted in place, equal a build from scratch under them.
//!
//! Ways it can go wrong, each covered: a visit whose categories moved keeps the old ones; a day
//! keeps a category that left it (its category row has to go); a trail's compare sets keep pages
//! of the old kind; new visits in the same build get the old categories or are counted twice; a
//! build that only changed categories starts over (it must report an incremental build).
use super::append_aggregates::{Visit, add, build, published, vault, visits};
use super::fixtures::seed_core_intelligence_fixture;
use crate::intelligence::site_dictionary::{
    SiteDictionaryOverrideTargetKind, SiteDictionaryOverrideUpsert, upsert_site_dictionary_override,
};
use crate::test_support::native::TestDatabase;
use vault_store::Db;

const FACTS: &[&str] = &["visit_derived_facts"];
const DAILY: &[&str] = &[
    "domain_daily_rollups",
    "category_daily_rollups",
    "engine_daily_rollups",
    "daily_summary_rollups",
];
const AGGREGATES: &[&str] =
    &["compare_sets", "refind_pages", "path_flows", "query_families", "reopened_investigations"];

/// The categories a user gives a site: GitHub becomes documentation (the issues read back and
/// forth turn from an issue compare set into documentation pages beside docs.rs), then docs.rs
/// becomes articles.
const CHANGES: [(&str, &str, &str); 2] =
    [("github.com", "docs", "docs_page"), ("docs.rs", "developer", "article_page")];

fn recategorize(vault: &crate::Vault, (site, domain, page): (&str, &str, &str)) {
    let change = SiteDictionaryOverrideUpsert {
        target_kind: SiteDictionaryOverrideTargetKind::ExactDomain,
        target_value: site.into(),
        domain_category: Some(domain.into()),
        page_category: Some(page.into()),
        interaction_kind: None,
        display_name: None,
        search_engine: None,
        is_noisy: false,
        note: None,
    };
    TestDatabase::new(vault, Db::Intelligence)
        .write(move |s| upsert_site_dictionary_override(s, &change))
        .unwrap();
}

/// Facts, daily rollups and aggregates of `vault` against one build from scratch of the same
/// visits with the same site categories.
fn assert_as_built_anew(vault: &crate::Vault, visits: &[Visit], changes: &[(&str, &str, &str)]) {
    let anew = super::append_aggregates::vault();
    let archive = TestDatabase::new(&anew, Db::Archive);
    archive
        .write(|s| {
            seed_core_intelligence_fixture(s);
            Ok(())
        })
        .unwrap();
    add(&archive, visits);
    drop(archive);
    for change in changes {
        recategorize(&anew, *change);
    }
    assert_eq!(build(&anew), "fallback-full");
    for (tables, group) in [(FACTS, "facts"), (DAILY, "daily"), (AGGREGATES, "aggregates")] {
        let want = published(&anew, tables, group);
        for (table, rows) in &want {
            assert!(!rows.is_empty(), "the fixture gives {table} something to hold");
        }
        for ((table, got), (_, want)) in published(vault, tables, group).iter().zip(&want) {
            assert_eq!(got, want, "{table} sorted in place differs from a build from scratch");
        }
    }
}

#[test]
fn new_categories_sorted_in_place_equal_a_build_from_scratch() {
    let all = visits();
    // Inside the morning trail of day 18, so the second build continues it.
    let cut = all.len() * 3 / 5 + 1;
    let sorted = vault();
    let archive = TestDatabase::new(&sorted, Db::Archive);
    archive
        .write(|s| {
            seed_core_intelligence_fixture(s);
            Ok(())
        })
        .unwrap();
    add(&archive, &all[..cut]);
    assert_eq!(build(&sorted), "fallback-full");

    recategorize(&sorted, CHANGES[0]);
    assert_eq!(build(&sorted), "incremental", "new categories alone are sorted in place");
    assert_as_built_anew(&sorted, &all[..cut], &CHANGES[..1]);

    // New categories and new visits in one build.
    recategorize(&sorted, CHANGES[1]);
    add(&archive, &all[cut..]);
    assert_eq!(build(&sorted), "incremental", "an append with new categories stays in place");
    assert_as_built_anew(&sorted, &all, &CHANGES);
}
