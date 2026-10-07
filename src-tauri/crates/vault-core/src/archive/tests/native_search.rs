//! Responsibilities: preserve history syntax and pagination through the native search owner.
//! Not responsible for: timeline reads or legacy FTS query plans.
//! Failure modes: search terms disappear, URL fragments fail, cursors repeat, regex dialect changes.
use super::*;
#[test]
fn history_search_terms_fragments_regex_and_paging_use_native_facts() {
    let vault = crate::Vault::open_temp().unwrap();
    let dir = tempdir().unwrap();
    let paths = sample_paths(dir.path());
    let config = AppConfig::default();
    legacy_queries::seed_reader_archive(&paths, &config);
    crate::test_support::legacy_fixtures::copy_to_vault(&vault, &paths, &config);
    vault.store().write(vault_store::Db::Archive, vault_store::Priority::Interactive, |s| {
        s.execute("INSERT INTO pages(id,url,canonical_url,registrable_domain) VALUES(2,'https://example.com/archive-two','https://example.com/archive-two','example.com')", ())?;
        s.execute("INSERT INTO urls(id,url,title,source_profile_id,page_id,source_url_id,first_visit_ms,first_visit_iso,last_visit_ms,last_visit_iso,created_by_run_id) VALUES(2,'https://example.com/archive-two','Archive docs',1,2,2,1775387000000,'then',1775387000000,'then',1)", ())?;
        s.execute("INSERT INTO visits(id,url_id,source_profile_id,visit_time_ms,visit_time_iso,created_by_run_id,source_visit_id) VALUES(3,2,1,1775387000000,'2026-04-05',1,'3')", ())?;
        s.execute("INSERT INTO search_terms(url_id,term,normalized_term,source_profile_id,created_by_run_id) VALUES(2,'deep recall token','deep recall token',1,1)", ())?;
        crate::archive::search_documents::refresh(s,2)
    }).unwrap();
    vault.search().cycle().unwrap();
    let search_term_history = list_history(
        &vault,
        HistoryQuery { q: Some("deep recall".to_string()), ..HistoryQuery::default() },
    )
    .expect("list search term history");
    assert_eq!(search_term_history.total, 2);
    let search_term_first_page = list_history(
        &vault,
        HistoryQuery {
            q: Some("deep recall".to_string()),
            limit: Some(1),
            ..HistoryQuery::default()
        },
    )
    .expect("search term first page");
    assert_eq!(search_term_first_page.items.len(), 1);
    assert!(search_term_first_page.next_cursor.is_some());
    let search_term_second_cursor_page = list_history(
        &vault,
        HistoryQuery {
            q: Some("deep recall".to_string()),
            limit: Some(1),
            cursor: search_term_first_page.next_cursor.clone(),
            ..HistoryQuery::default()
        },
    )
    .expect("search term second cursor page");
    assert_eq!(search_term_second_cursor_page.items.len(), 1);
    assert!(search_term_second_cursor_page.has_previous);
    let search_term_explicit_second_page = list_history(
        &vault,
        HistoryQuery {
            q: Some("deep recall".to_string()),
            limit: Some(1),
            page: Some(2),
            ..HistoryQuery::default()
        },
    )
    .expect("search term explicit second page");
    assert_eq!(search_term_explicit_second_page.page, 2);
    assert_eq!(search_term_explicit_second_page.items.len(), 1);

    let url_fragment_history = list_history(
        &vault,
        HistoryQuery { q: Some("example.com/archive".to_string()), ..HistoryQuery::default() },
    )
    .expect("list url fragment history");
    assert_eq!(url_fragment_history.total, 2);

    let punctuation_only_history = list_history(
        &vault,
        HistoryQuery { q: Some("!!!".to_string()), ..HistoryQuery::default() },
    )
    .expect("list punctuation-only history");
    assert_eq!(punctuation_only_history.total, 0);

    let regex_history = list_history(
        &vault,
        HistoryQuery {
            q: Some("archive\\sdocs".to_string()),
            regex_mode: Some(true),
            ..HistoryQuery::default()
        },
    )
    .expect("regex history");
    assert_eq!(regex_history.total, 2);
    let regex_oldest_first_page = list_history(
        &vault,
        HistoryQuery {
            q: Some("archive\\sdocs".to_string()),
            regex_mode: Some(true),
            sort: Some("oldest".to_string()),
            limit: Some(1),
            ..HistoryQuery::default()
        },
    )
    .expect("regex oldest first page");
    assert!(regex_oldest_first_page.next_cursor.is_some());
    let regex_oldest_second_page = list_history(
        &vault,
        HistoryQuery {
            q: Some("archive\\sdocs".to_string()),
            regex_mode: Some(true),
            sort: Some("oldest".to_string()),
            limit: Some(1),
            cursor: regex_oldest_first_page.next_cursor.clone(),
            ..HistoryQuery::default()
        },
    )
    .expect("regex oldest second page");
    assert_eq!(regex_oldest_second_page.items.len(), 1);
    assert!(regex_oldest_second_page.has_previous);
    let regex_newest_first_page = list_history(
        &vault,
        HistoryQuery {
            q: Some("archive\\sdocs".to_string()),
            regex_mode: Some(true),
            limit: Some(1),
            ..HistoryQuery::default()
        },
    )
    .expect("regex newest first page");
    let regex_newest_second_page = list_history(
        &vault,
        HistoryQuery {
            q: Some("archive\\sdocs".to_string()),
            regex_mode: Some(true),
            limit: Some(1),
            cursor: regex_newest_first_page.next_cursor.clone(),
            ..HistoryQuery::default()
        },
    )
    .expect("regex newest second page");
    assert_eq!(regex_newest_second_page.items.len(), 1);
    let regex_explicit_second_page = list_history(
        &vault,
        HistoryQuery {
            q: Some("archive\\sdocs".to_string()),
            regex_mode: Some(true),
            limit: Some(1),
            page: Some(2),
            ..HistoryQuery::default()
        },
    )
    .expect("regex explicit second page");
    assert_eq!(regex_explicit_second_page.page, 2);
    assert_eq!(regex_explicit_second_page.items.len(), 1);

    let invalid_regex = list_history(
        &vault,
        HistoryQuery {
            q: Some("archive(".to_string()),
            regex_mode: Some(true),
            ..HistoryQuery::default()
        },
    )
    .expect_err("invalid regex");
    assert!(
        format!("{invalid_regex:#}").contains("invalid regex pattern"),
        "unexpected error: {invalid_regex:#}"
    );
    let unsupported_regex = list_history(
        &vault,
        HistoryQuery {
            q: Some("^((?!pathkeep).)*$".to_string()),
            regex_mode: Some(true),
            ..HistoryQuery::default()
        },
    )
    .expect_err("unsupported regex");
    assert!(
        format!("{unsupported_regex:#}").contains("invalid regex pattern"),
        "Rust regex should reject look-around patterns before searching"
    );
}
