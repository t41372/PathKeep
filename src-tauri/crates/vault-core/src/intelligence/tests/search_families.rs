//! The search page's lookup of every family holding one search (`get_search_families`).
//!
//! Failure modes: a search whose families are older than the newest page of families is
//! missed (the page read only the newest 200 and filtered them in the browser), case or
//! surrounding spaces split one search, a family is found only by its anchor and not by
//! another wording, or a search no family holds returns something.

use super::super::{
    get_query_families, get_search_families, intelligence_rebuild::run_core_intelligence,
};
use super::fixtures::{append_fixture_visit, seed_core_intelligence_fixture};
use crate::models::{
    AppConfig, ArchiveMode, CoreIntelligenceRebuildRequest, DateRange, PagedDateRangeRequest,
    SearchFamiliesRequest,
};

const DAY_MS: i64 = 86_400_000;
const START_MS: i64 = 1_712_000_000_000;

fn archive() -> crate::Vault {
    let config = AppConfig {
        initialized: true,
        archive_mode: ArchiveMode::Plaintext,
        ..AppConfig::default()
    };
    let vault = crate::Vault::open_temp().unwrap().with_config(config);
    vault
        .write(vault_store::Db::Archive, vault_store::Priority::Batch, |archive| {
            seed_core_intelligence_fixture(archive);
            // An old search, three times and in two spellings, then 210 newer unrelated
            // searches, one a day, so its family is far past the newest 200.
            let mut searches = vec![
                (START_MS, "rust ownership".to_string()),
                (START_MS + 60_000, "Rust Ownership".to_string()),
                (START_MS + DAY_MS, "rust ownership".to_string()),
            ];
            for day in 0..210 {
                // Words of their own, so each search is its own family.
                searches.push((START_MS + (day + 30) * DAY_MS, format!("w{day}alpha w{day}beta")));
            }
            for (index, (time, query)) in searches.iter().enumerate() {
                append_fixture_visit(
                    archive,
                    1_000 + index as i64,
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
    run_core_intelligence(&vault, &CoreIntelligenceRebuildRequest::default()).unwrap();
    vault
}

#[test]
fn every_family_of_a_search_is_found_however_old_it_is() {
    let vault = archive();
    let newest = get_query_families(
        &vault,
        &PagedDateRangeRequest {
            cursor: None,
            date_range: DateRange { start: "2000-01-01".into(), end: "2100-01-01".into() },
            profile_id: None,
            page_size: 200,
        },
    )
    .unwrap();
    assert!(
        newest.families.iter().all(|family| family.anchor_query != "rust ownership"),
        "the fixture must put the search past the newest page"
    );

    for typed in ["rust ownership", "  RUST OWNERSHIP ", "Rust Ownership"] {
        let found = get_search_families(
            &vault,
            &SearchFamiliesRequest { query: typed.into(), profile_id: None },
        )
        .unwrap();
        assert_eq!(
            found.families.iter().map(|family| family.member_count).sum::<i64>(),
            3,
            "{typed:?}: {:?}",
            found.families
        );
    }
    let none = get_search_families(
        &vault,
        &SearchFamiliesRequest { query: "never searched".into(), profile_id: None },
    )
    .unwrap();
    assert!(none.families.is_empty());
}
