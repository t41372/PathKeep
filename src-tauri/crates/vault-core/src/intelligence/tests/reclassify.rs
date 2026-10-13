//! New categories, sorted in place, equal a build from scratch under them.
//!
//! Ways it can go wrong, each covered: a visit whose categories moved keeps the old ones; a day
//! keeps a category that left it (its category row has to go); a trail's compare sets keep pages
//! of the old kind; new visits in the same build get the old categories or are counted twice; a
//! build that only changed categories starts over (it must report an incremental build).
//!
//! A site's category set by the user (the site page): the build walks every URL of the profile
//! instead of that site's (3.19M pages at 14.4M visits, for one site); taking the override away
//! leaves the site in the user's category; a category changed while a build runs is left out of
//! the Insights it publishes, or starts the build over; the site's page says Insights are being
//! updated before they are, after they are, or for a site no one changed.
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

/// A site's category and its pages' kind; an empty kind leaves the pages' kinds to the rules.
fn recategorize(vault: &crate::Vault, (site, domain, page): (&str, &str, &str)) {
    let change = SiteDictionaryOverrideUpsert {
        target_kind: SiteDictionaryOverrideTargetKind::ExactDomain,
        target_value: site.into(),
        domain_category: Some(domain.into()),
        page_category: (!page.is_empty()).then(|| page.into()),
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

/// A site's category alone, as the site page sets it, or none: the site goes back to its rules.
fn set_site_category(vault: &crate::Vault, site: &str, category: Option<&str>) {
    super::super::set_site_category(vault, site, category).unwrap();
}

/// The sites the last build of the fixture's profile sorted again; `None` for every URL.
fn reclassified_sites(vault: &crate::Vault) -> Option<Vec<String>> {
    vault
        .read(Db::Intelligence, |s| super::super::build_state::load(s, "chrome:Default"))
        .unwrap()
        .and_then(|state| state.reclassify)
        .expect("the build sorted visits again")
        .sites
}

/// The site's page says its Insights are being updated, in the scope of every profile.
fn pending(vault: &crate::Vault, site: &str) -> bool {
    let site = site.to_owned();
    vault
        .read(Db::Intelligence, move |s| {
            super::super::build_pipeline::site_category_pending(s, &site, None)
        })
        .unwrap()
}

#[test]
fn a_site_category_from_the_user_sorts_only_that_sites_pages_again() {
    let all = visits();
    let sorted = vault();
    let archive = TestDatabase::new(&sorted, Db::Archive);
    archive
        .write(|s| {
            seed_core_intelligence_fixture(s);
            Ok(())
        })
        .unwrap();
    add(&archive, &all);
    drop(archive);
    assert_eq!(build(&sorted), "fallback-full");
    assert!(!pending(&sorted, "github.com"));

    set_site_category(&sorted, "github.com", Some("docs"));
    assert!(pending(&sorted, "github.com"));
    assert!(!pending(&sorted, "docs.rs"), "only the site changed");
    assert_eq!(build(&sorted), "incremental");
    assert!(!pending(&sorted, "github.com"));
    assert_eq!(reclassified_sites(&sorted), Some(vec!["github.com".to_owned()]));
    assert_as_built_anew(&sorted, &all, &[("github.com", "docs", "")]);

    // Taken away, the site goes back to what its rules say.
    set_site_category(&sorted, "github.com", None);
    assert!(pending(&sorted, "github.com"));
    assert_eq!(build(&sorted), "incremental");
    assert!(!pending(&sorted, "github.com"));
    assert_eq!(reclassified_sites(&sorted), Some(vec!["github.com".to_owned()]));
    assert_as_built_anew(&sorted, &all, &[]);
}

#[test]
fn a_site_category_changed_while_a_build_runs_is_in_what_it_publishes() {
    let all = visits();
    let sorted = vault();
    let archive = TestDatabase::new(&sorted, Db::Archive);
    archive
        .write(|s| {
            seed_core_intelligence_fixture(s);
            Ok(())
        })
        .unwrap();
    add(&archive, &all);
    drop(archive);
    assert_eq!(build(&sorted), "fallback-full");

    set_site_category(&sorted, "github.com", Some("docs"));
    let mut changed = false;
    let report = super::super::run_core_intelligence_with_progress(
        &sorted,
        &crate::models::CoreIntelligenceRebuildRequest::default(),
        |_| {
            // The user changes another site while the first change is being sorted in.
            if !changed {
                changed = true;
                set_site_category(&sorted, "docs.rs", Some("news"));
                assert!(pending(&sorted, "github.com"), "not published yet");
                assert!(pending(&sorted, "docs.rs"));
            }
            Ok(())
        },
    )
    .unwrap();
    assert!(changed);
    assert!(!pending(&sorted, "github.com") && !pending(&sorted, "docs.rs"));
    assert_ne!(report.execution_mode.as_deref(), Some("fallback-full"), "not built anew");
    assert_as_built_anew(&sorted, &all, &[("github.com", "docs", ""), ("docs.rs", "news", "")]);
}
