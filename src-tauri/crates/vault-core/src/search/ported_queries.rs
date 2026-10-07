//! Responsibilities: preserve the legacy query-form and enrichment contracts on Vault.
//! Not responsible for: FTS5 cursors, SQLCipher projections or timeline reads.
//! Failure modes: aliases, compact/CJK folding, phrase/exclusion/field/date forms,
//! annotation-only matches and enrichment excerpts drift while changing engines.
use super::{
    fixture::{Archive, CHROME},
    history as list_history,
};
use crate::HistoryQuery;
fn lexical() -> Archive {
    let archive = Archive::with_key(Some([105; 32]));
    for (id, url, title, time) in [
        (1, "https://example.test/preferences", "瀏覽器設定中心", 1000),
        (2, "https://example.test/simplified", "浏览器设定说明", 2000),
        (3, "https://example.test/github-actions", "GitHub Actions manual", 3000),
        (4, "https://github.com/releases", "Release notes", 4000),
        (5, "https://example.test/git-hub-guide", "Git Hub spacing guide", 5000),
        (6, "https://www.youtube.com/watch?v=recall", "YouTube watch queue", 6000),
        (7, "https://example.test/reviews/pull-request", "Pull Request review checklist", 7000),
    ] {
        archive.url(id, CHROME, url, title, &[time]);
    }
    archive.index();
    archive
}
fn insert_lexical_history_row(
    archive: &Archive,
    id: i64,
    url: &str,
    title: &str,
    time: i64,
    _iso: &str,
) {
    archive.url(id, CHROME, url, title, &[time]).index();
}
fn store_lexical_enrichment(archive: &Archive, id: i64, summary: &str) {
    let extraction=serde_json::json!({"fullName":"o/r","description":summary,"topics":["enrichkw"],"language":"Rust"}).to_string();
    let text = crate::archive::enrichment_text_for_index(Some(summary), Some(&extraction));
    crate::archive::set_enrichment_text(&archive.vault, id, &text).unwrap();
    archive.index();
}
#[test]
fn lexical_search_surfaces_capped_enrichment_excerpt_only_for_enriched_results() {
    let archive = lexical();
    // Enrich ONE of the seeded rows (url/visit id 3 = the GitHub Actions page) with a summary the
    // search query will hit, and a `topics` keyword ("enrichkw") that only lives in enrichment_text.
    store_lexical_enrichment(&archive, 3, "Reusable workflow runner for CI pipelines");

    // A keyword that matches the enriched page surfaces a capped excerpt on THAT result.
    let enriched = list_history(
        &archive.vault,
        HistoryQuery { q: Some("github".to_string()), limit: Some(10), ..HistoryQuery::default() },
    )
    .expect("enriched query");
    let actions_entry = enriched
        .items
        .iter()
        .find(|entry| entry.title.as_deref() == Some("GitHub Actions manual"))
        .expect("the enriched GitHub Actions row is recalled");
    // The excerpt mirrors the projected enrichment_text: the summary plus the searchable metadata
    // (topics/desc/language) the index carries — NOT just the bare summary.
    let actions_excerpt =
        actions_entry.enrichment_excerpt.as_deref().expect("the enriched row carries an excerpt");
    assert!(
        actions_excerpt.contains("Reusable workflow runner for CI pipelines"),
        "an enriched search result must surface its enrichment summary: {actions_excerpt}"
    );
    assert!(
        actions_excerpt.contains("enrichkw"),
        "the excerpt carries the searchable enrichment metadata too: {actions_excerpt}"
    );
    // A different recalled result with NO stored enrichment leaves the excerpt None.
    let plain_entry = enriched
        .items
        .iter()
        .find(|entry| entry.title.as_deref() == Some("Git Hub spacing guide"))
        .expect("the non-enriched Git Hub row is recalled");
    assert_eq!(
        plain_entry.enrichment_excerpt, None,
        "a result without stored enrichment must leave the excerpt None"
    );

    // The enrichment text itself is keyword-searchable, and that match also carries the excerpt.
    let by_enrichment_keyword = list_history(
        &archive.vault,
        HistoryQuery {
            q: Some("enrichkw".to_string()),
            limit: Some(10),
            ..HistoryQuery::default()
        },
    )
    .expect("enrichment-keyword query");
    assert_eq!(
        by_enrichment_keyword.total, 1,
        "a token that lives only in enrichment_text must recall exactly the enriched page"
    );
    assert!(
        by_enrichment_keyword.items[0]
            .enrichment_excerpt
            .as_deref()
            .is_some_and(|excerpt| excerpt.contains("Reusable workflow runner for CI pipelines")),
        "the enrichment-keyword match also carries the excerpt"
    );
}

#[test]
fn lexical_search_caps_long_enrichment_excerpt_on_a_cjk_char_boundary() {
    let archive = lexical();
    // A long CJK summary (> the 180-char cap) on the GitHub Actions row: the excerpt must be capped on
    // a CHAR boundary (never panicking by splitting a multi-byte codepoint) and gain a trailing "…".
    let long_summary = "工作流程".repeat(80); // 320 CJK chars, well over the cap.
    store_lexical_enrichment(&archive, 3, &long_summary);

    let response = list_history(
        &archive.vault,
        HistoryQuery { q: Some("github".to_string()), limit: Some(10), ..HistoryQuery::default() },
    )
    .expect("capped query");
    let excerpt = response
        .items
        .iter()
        .find(|entry| entry.title.as_deref() == Some("GitHub Actions manual"))
        .and_then(|entry| entry.enrichment_excerpt.clone())
        .expect("the enriched row carries a capped excerpt");
    // 180 capped chars + the trailing ellipsis = 181 chars; the long input was truncated, not panicked.
    assert_eq!(excerpt.chars().count(), 181, "the excerpt is capped on a char boundary");
    assert!(excerpt.ends_with('…'), "a truncated excerpt gains a trailing ellipsis");
    assert!(excerpt.starts_with("工作流程"), "CJK codepoints survive the char-boundary cap");
}

#[test]
fn lexical_recall_matches_cjk_script_folding_and_compact_substrings() {
    let archive = lexical();

    let simplified_query = list_history(
        &archive.vault,
        HistoryQuery { q: Some("设定".to_string()), limit: Some(10), ..HistoryQuery::default() },
    )
    .expect("simplified query");
    assert_eq!(simplified_query.total, 2);
    assert!(
        simplified_query.items.iter().any(|entry| entry.title.as_deref() == Some("浏览器设定说明"))
    );
    assert!(
        simplified_query.items.iter().any(|entry| entry.title.as_deref() == Some("瀏覽器設定中心")),
        "simplified query should recall the traditional indexed title"
    );

    let traditional_query = list_history(
        &archive.vault,
        HistoryQuery { q: Some("設定".to_string()), limit: Some(10), ..HistoryQuery::default() },
    )
    .expect("traditional query");
    assert_eq!(traditional_query.total, 2);
    assert!(
        traditional_query
            .items
            .iter()
            .any(|entry| entry.title.as_deref() == Some("瀏覽器設定中心"))
    );
    assert!(
        traditional_query
            .items
            .iter()
            .any(|entry| entry.title.as_deref() == Some("浏览器设定说明")),
        "traditional query should recall the simplified indexed title"
    );

    let cjk_substring = list_history(
        &archive.vault,
        HistoryQuery { q: Some("器设".to_string()), limit: Some(10), ..HistoryQuery::default() },
    )
    .expect("cjk substring query");
    assert_eq!(cjk_substring.total, 2);

    let compact_latin = list_history(
        &archive.vault,
        HistoryQuery { q: Some("github".to_string()), limit: Some(10), ..HistoryQuery::default() },
    )
    .expect("compact latin query");
    assert!(
        compact_latin
            .items
            .iter()
            .any(|entry| entry.title.as_deref() == Some("Git Hub spacing guide")),
        "compact trigram recall should match Git Hub when the query is github"
    );

    let full_width_latin = list_history(
        &archive.vault,
        HistoryQuery {
            q: Some("ＧｉｔＨｕｂ".to_string()),
            limit: Some(10),
            ..HistoryQuery::default()
        },
    )
    .expect("full-width latin query");
    assert!(
        full_width_latin
            .items
            .iter()
            .any(|entry| entry.title.as_deref() == Some("GitHub Actions manual")),
        "NFKC recall should fold full-width GitHub into the indexed latin form"
    );
}

#[test]
fn lexical_recall_expands_aliases_and_pages_every_fuzzy_match() {
    let archive = lexical();

    let github_alias = list_history(
        &archive.vault,
        HistoryQuery { q: Some("gh".to_string()), limit: Some(10), ..HistoryQuery::default() },
    )
    .expect("github alias query");
    assert!(
        github_alias
            .items
            .iter()
            .any(|entry| entry.title.as_deref() == Some("GitHub Actions manual")),
        "gh should expand to github before lexical recall"
    );

    let youtube_alias = list_history(
        &archive.vault,
        HistoryQuery { q: Some("yt".to_string()), limit: Some(10), ..HistoryQuery::default() },
    )
    .expect("youtube alias query");
    assert!(
        youtube_alias
            .items
            .iter()
            .any(|entry| entry.title.as_deref() == Some("YouTube watch queue")),
        "yt should expand to youtube before lexical recall"
    );

    let pull_request_alias = list_history(
        &archive.vault,
        HistoryQuery { q: Some("pr".to_string()), limit: Some(10), ..HistoryQuery::default() },
    )
    .expect("pull request alias query");
    assert!(
        pull_request_alias
            .items
            .iter()
            .any(|entry| entry.title.as_deref() == Some("Pull Request review checklist")),
        "pr should expand to pull request before lexical recall"
    );

    let fuzzy_first_page = list_history(
        &archive.vault,
        HistoryQuery { q: Some("gihub".to_string()), limit: Some(1), ..HistoryQuery::default() },
    )
    .expect("fuzzy typo query");
    assert_eq!(fuzzy_first_page.total, 3);
    // Trigram BM25 now supplies the fuzzy score; recency and page ID break ties.
    assert_eq!(fuzzy_first_page.items[0].title.as_deref(), Some("Git Hub spacing guide"));
    assert!(fuzzy_first_page.next_cursor.as_deref().is_some_and(|cursor| cursor.contains(':')));

    let fuzzy_second_page = list_history(
        &archive.vault,
        HistoryQuery {
            q: Some("gihub".to_string()),
            limit: Some(1),
            cursor: fuzzy_first_page.next_cursor.clone(),
            ..HistoryQuery::default()
        },
    )
    .expect("fuzzy typo second page");
    assert_eq!(fuzzy_second_page.items.len(), 1);
    assert!(fuzzy_second_page.has_previous);
    let fuzzy_second_page_by_number = list_history(
        &archive.vault,
        HistoryQuery {
            q: Some("gihub".to_string()),
            limit: Some(1),
            page: Some(2),
            ..HistoryQuery::default()
        },
    )
    .expect("fuzzy typo explicit second page");
    assert_eq!(fuzzy_second_page_by_number.items.len(), 1);
    assert_eq!(fuzzy_second_page_by_number.page, 2);

    let fuzzy_newest = list_history(
        &archive.vault,
        HistoryQuery {
            q: Some("gihub".to_string()),
            sort: Some("newest".to_string()),
            limit: Some(1),
            ..HistoryQuery::default()
        },
    )
    .expect("fuzzy newest query");
    assert_eq!(fuzzy_newest.items[0].title.as_deref(), Some("Git Hub spacing guide"));
    let fuzzy_newest_second = list_history(
        &archive.vault,
        HistoryQuery {
            q: Some("gihub".to_string()),
            sort: Some("newest".to_string()),
            limit: Some(1),
            cursor: fuzzy_newest.next_cursor.clone(),
            ..HistoryQuery::default()
        },
    )
    .expect("fuzzy newest second page");
    assert_eq!(fuzzy_newest_second.items[0].title.as_deref(), Some("Release notes"));

    let fuzzy_oldest = list_history(
        &archive.vault,
        HistoryQuery {
            q: Some("gihub".to_string()),
            sort: Some("oldest".to_string()),
            limit: Some(1),
            ..HistoryQuery::default()
        },
    )
    .expect("fuzzy oldest query");
    assert_eq!(fuzzy_oldest.items[0].title.as_deref(), Some("GitHub Actions manual"));
    let fuzzy_oldest_second = list_history(
        &archive.vault,
        HistoryQuery {
            q: Some("gihub".to_string()),
            sort: Some("oldest".to_string()),
            limit: Some(1),
            cursor: fuzzy_oldest.next_cursor.clone(),
            ..HistoryQuery::default()
        },
    )
    .expect("fuzzy oldest second page");
    assert_eq!(fuzzy_oldest_second.items[0].title.as_deref(), Some("Release notes"));
}

#[test]
fn lexical_recall_defaults_to_relevance_and_accepts_time_sort_override() {
    let archive = lexical();

    let relevance = list_history(
        &archive.vault,
        HistoryQuery { q: Some("github".to_string()), limit: Some(1), ..HistoryQuery::default() },
    )
    .expect("relevance query");
    assert_eq!(relevance.items[0].title.as_deref(), Some("GitHub Actions manual"));
    assert!(relevance.next_cursor.as_deref().is_some_and(|cursor| cursor.contains(':')));

    let explicit_relevance = list_history(
        &archive.vault,
        HistoryQuery {
            q: Some("github".to_string()),
            sort: Some("relevance".to_string()),
            limit: Some(1),
            ..HistoryQuery::default()
        },
    )
    .expect("explicit relevance query");
    assert_eq!(explicit_relevance.items[0].title.as_deref(), Some("GitHub Actions manual"));

    let second_relevance_page = list_history(
        &archive.vault,
        HistoryQuery {
            q: Some("github".to_string()),
            limit: Some(1),
            cursor: relevance.next_cursor.clone(),
            ..HistoryQuery::default()
        },
    )
    .expect("second relevance page");
    assert_eq!(second_relevance_page.items.len(), 1);
    assert!(second_relevance_page.has_previous);

    let legacy_cursor_relevance_page = list_history(
        &archive.vault,
        HistoryQuery {
            q: Some("github".to_string()),
            sort: Some("relevance".to_string()),
            limit: Some(1),
            cursor: Some("4000|4".to_string()),
            ..HistoryQuery::default()
        },
    )
    .expect_err("a legacy chronological cursor cannot address a pinned search session");
    assert!(legacy_cursor_relevance_page.to_string().contains("cursor"));

    let newest = list_history(
        &archive.vault,
        HistoryQuery {
            q: Some("github".to_string()),
            sort: Some("newest".to_string()),
            limit: Some(1),
            ..HistoryQuery::default()
        },
    )
    .expect("newest query");
    assert_eq!(newest.items[0].title.as_deref(), Some("Git Hub spacing guide"));
    assert!(newest.next_cursor.as_deref().is_some_and(|cursor| cursor.contains(':')));
}

#[test]
fn history_keyword_query_supports_google_like_local_operators() {
    let archive = lexical();
    insert_lexical_history_row(
        &archive,
        8,
        "https://github.com/pathkeep/pathkeep/issues",
        "PathKeep issue tracker",
        8_000,
        "2026-05-01T00:00:08+00:00",
    );
    insert_lexical_history_row(
        &archive,
        9,
        "https://github.com/pathkeep/spec.pdf",
        "PathKeep PDF spec",
        9_000,
        "2026-05-01T00:00:09+00:00",
    );

    let site_without_keyword = list_history(
        &archive.vault,
        HistoryQuery {
            q: Some("site:github.com -pathkeep".to_string()),
            ..HistoryQuery::default()
        },
    )
    .expect("site exclusion query");
    assert_eq!(site_without_keyword.total, 1);
    assert_eq!(site_without_keyword.items[0].title.as_deref(), Some("Release notes"));

    let domain_without_keyword = list_history(
        &archive.vault,
        HistoryQuery {
            q: Some("-pathkeep".to_string()),
            domain: Some("github.com".to_string()),
            ..HistoryQuery::default()
        },
    )
    .expect("domain exclusion query");
    assert_eq!(domain_without_keyword.total, 1);
    assert_eq!(domain_without_keyword.items[0].title.as_deref(), Some("Release notes"));

    let exact_phrase = list_history(
        &archive.vault,
        HistoryQuery { q: Some("\"release notes\"".to_string()), ..HistoryQuery::default() },
    )
    .expect("exact phrase query");
    assert_eq!(exact_phrase.total, 1);
    assert_eq!(exact_phrase.items[0].title.as_deref(), Some("Release notes"));

    let intersected_date_filters = list_history(
        &archive.vault,
        HistoryQuery {
            q: Some("site:github.com after:1970-01-01 before:1970-01-01".to_string()),
            start_time_ms: Some(3_500),
            end_time_ms: Some(4_500),
            ..HistoryQuery::default()
        },
    )
    .expect("intersected date filters");
    assert_eq!(intersected_date_filters.total, 1);
    assert_eq!(intersected_date_filters.items[0].title.as_deref(), Some("Release notes"));

    let ui_time_only_filters = list_history(
        &archive.vault,
        HistoryQuery {
            start_time_ms: Some(3_500),
            end_time_ms: Some(4_500),
            ..HistoryQuery::default()
        },
    )
    .expect("ui time-only filters");
    assert_eq!(ui_time_only_filters.total, 1);
    assert_eq!(ui_time_only_filters.items[0].title.as_deref(), Some("Release notes"));

    let title_operator = list_history(
        &archive.vault,
        HistoryQuery { q: Some("intitle:manual".to_string()), ..HistoryQuery::default() },
    )
    .expect("title operator query");
    assert_eq!(title_operator.total, 1);
    assert_eq!(title_operator.items[0].title.as_deref(), Some("GitHub Actions manual"));

    let url_operator = list_history(
        &archive.vault,
        HistoryQuery { q: Some("inurl:pull-request".to_string()), ..HistoryQuery::default() },
    )
    .expect("url operator query");
    assert_eq!(url_operator.total, 1);
    assert_eq!(url_operator.items[0].title.as_deref(), Some("Pull Request review checklist"));

    let filetype_operator = list_history(
        &archive.vault,
        HistoryQuery {
            q: Some("site:github.com filetype:pdf".to_string()),
            ..HistoryQuery::default()
        },
    )
    .expect("filetype operator query");
    assert_eq!(filetype_operator.total, 1);
    assert_eq!(filetype_operator.items[0].title.as_deref(), Some("PathKeep PDF spec"));

    let any_of_these_words = list_history(
        &archive.vault,
        HistoryQuery { q: Some("manual OR youtube".to_string()), ..HistoryQuery::default() },
    )
    .expect("or query");
    assert_eq!(any_of_these_words.total, 2);
    assert!(
        any_of_these_words
            .items
            .iter()
            .any(|entry| entry.title.as_deref() == Some("GitHub Actions manual"))
    );
    assert!(
        any_of_these_words
            .items
            .iter()
            .any(|entry| entry.title.as_deref() == Some("YouTube watch queue"))
    );
}

#[test]
fn history_keyword_query_supports_tag_and_note_operators_against_annotations() {
    // feedback-2026-05-25 §3.3 A — annotations existed but the search
    // surface only saw title / URL. This pins the new `tag:` /
    // `note:` operators end-to-end: parser → SQL → archive results.
    let archive = lexical();

    // Two extra URLs that only differ by their annotations — neither
    // mentions "rust" or "design" in title or URL, so any match must
    // come from the new tag/note filters.
    insert_lexical_history_row(
        &archive,
        20,
        "https://example.test/page-with-rust-tag",
        "Generic page A",
        20_000,
        "2026-05-01T00:00:20+00:00",
    );
    insert_lexical_history_row(
        &archive,
        21,
        "https://example.test/page-with-design-note",
        "Generic page B",
        21_000,
        "2026-05-01T00:00:21+00:00",
    );
    insert_lexical_history_row(
        &archive,
        22,
        "https://example.test/page-with-no-annotations",
        "Generic page C",
        22_000,
        "2026-05-01T00:00:22+00:00",
    );

    // Seed annotations via the public surface so we exercise the same
    // schema the detail panel writes to.
    // Independent annotation facts exercise the unported query parser, not the Turso writer.
    archive.tag("https://example.test/page-with-rust-tag", "Rust");
    crate::set_notes(
        &archive.vault,
        crate::SetNotesRequest {
            url: "https://example.test/page-with-design-note".into(),
            notes: "Initial design doc for the cache layer".into(),
            source_profile: None,
        },
    )
    .unwrap();
    archive.index();

    // `tag:rust` — only the page tagged Rust matches (and the case is
    // folded so the user can type either `tag:Rust` or `tag:rust`).
    let tag_hit = list_history(
        &archive.vault,
        HistoryQuery { q: Some("tag:rust".to_string()), ..HistoryQuery::default() },
    )
    .expect("tag query");
    assert_eq!(tag_hit.total, 1);
    assert_eq!(tag_hit.items[0].url, "https://example.test/page-with-rust-tag",);

    // `note:design` — substring match on `url_annotations.notes`,
    // case-insensitive.
    let note_hit = list_history(
        &archive.vault,
        HistoryQuery { q: Some("note:\"design doc\"".to_string()), ..HistoryQuery::default() },
    )
    .expect("note query");
    assert_eq!(note_hit.total, 1);
    assert_eq!(note_hit.items[0].url, "https://example.test/page-with-design-note",);

    // `-tag:rust` — exclude any URL tagged Rust. Both Generic page B
    // and Generic page C survive (along with every other seed row).
    let tag_negated = list_history(
        &archive.vault,
        HistoryQuery { q: Some("-tag:rust".to_string()), ..HistoryQuery::default() },
    )
    .expect("negated tag query");
    assert!(
        !tag_negated
            .items
            .iter()
            .any(|entry| entry.url == "https://example.test/page-with-rust-tag"),
        "expected -tag:rust to exclude the Rust-tagged row",
    );
    // And the un-tagged Generic page C is still in the result set,
    // pinning that the exclusion is a soft filter (not "only show
    // rows that have any tag").
    assert!(
        tag_negated
            .items
            .iter()
            .any(|entry| { entry.url == "https://example.test/page-with-no-annotations" }),
        "expected -tag:rust to keep un-tagged rows",
    );
}

#[test]
fn filetype_normalization_does_not_create_sql_wildcards() {
    let archive = Archive::with_key(Some([113; 32]));
    archive
        .url(1, CHROME, "https://files.test/one.pf", "ordinary", &[1])
        .url(2, CHROME, "https://files.test/two.pdf", "ordinary", &[2])
        .url(3, CHROME, "https://files.test/three.pxf", "ordinary", &[3])
        .index();
    assert_eq!(
        archive
            .words("filetype:p%f")
            .items
            .iter()
            .map(|item| item.url.as_str())
            .collect::<Vec<_>>(),
        ["https://files.test/one.pf"]
    );
    assert_eq!(
        archive.words("ext:p_f").items.iter().map(|item| item.url.as_str()).collect::<Vec<_>>(),
        ["https://files.test/one.pf"]
    );
}

#[test]
fn unsupported_negative_date_operators_keep_the_parsers_literal_exclusion() {
    let archive = Archive::new();
    archive
        .url(1, CHROME, "https://date.test/1", "after 2024", &[1])
        .url(2, CHROME, "https://date.test/2", "ordinary", &[2])
        .index();
    assert_eq!(
        archive.words("-after:2024").items.iter().map(|item| item.url.as_str()).collect::<Vec<_>>(),
        ["https://date.test/2"]
    );
}

#[test]
fn advanced_constraints_apply_to_every_keyword_or_group() {
    let archive = Archive::new();
    for (id, title) in [(1, "alpha secret"), (2, "beta"), (3, "alpha guide"), (4, "beta guide")] {
        archive.url(id, CHROME, &format!("https://operators.test/{id}"), title, &[id]);
    }
    archive.index();
    for (query, fuzzy, expected) in [
        ("alpha OR beta -secret", false, vec![2, 3, 4]),
        ("alpha OR beta intitle:guide", false, vec![3, 4]),
        ("alpha OR beta \"guide\"", false, vec![3, 4]),
        ("alxha OR betta -secret", true, vec![2, 3, 4]),
    ] {
        let page = super::SearchSession::start(
            &archive.vault,
            super::SearchRequest { query: query.into(), fuzzy, ..Default::default() },
        )
        .unwrap()
        .finish_page()
        .unwrap();
        let mut ids = page.items.into_iter().map(|hit| hit.page_id).collect::<Vec<_>>();
        ids.sort_unstable();
        assert_eq!(ids, expected, "{query}");
    }
}

#[test]
fn field_fragments_keep_the_existing_substring_and_literal_punctuation_semantics() {
    let archive = Archive::new();
    archive
        .url(1, CHROME, "https://github.com/project", "GitHub manual", &[1])
        .url(2, CHROME, "https://gitlab.test/github/company", "Other", &[2])
        .url(3, CHROME, "https://example.test/3", "Hubcaps manual", &[3])
        .url(4, CHROME, "https://foo.test/4", "bar", &[4])
        .url(5, CHROME, "https://example.test/5", "foo bar", &[5])
        .index();
    for (query, expected) in [
        ("intitle:hub", vec![1, 3]),
        ("inurl:github.com", vec![1]),
        ("-intitle:\"hub manual\"", vec![2, 3, 4, 5]),
        ("-foo-bar", vec![1, 2, 3, 4]),
    ] {
        let mut ids = archive.words(query).items.into_iter().map(|hit| hit.id).collect::<Vec<_>>();
        ids.sort_unstable();
        assert_eq!(ids, expected, "{query}");
    }
}

#[test]
fn phrases_do_not_match_synthetic_gram_adjacency_or_expand_short_aliases() {
    let archive = Archive::with_key(Some([116; 32]));
    archive
        .url(1, CHROME, "https://phrase.test/1", "上 x 海", &[1])
        .url(2, CHROME, "https://phrase.test/2", "上海", &[2])
        .url(3, CHROME, "https://phrase.test/上", "海", &[3])
        .url(4, CHROME, "https://phrase.test/4", "GitHub", &[4])
        .url(5, CHROME, "https://phrase.test/5", "gh", &[5])
        .index();
    assert_eq!(archive.words("上海").items.iter().map(|hit| hit.id).collect::<Vec<_>>(), vec![2]);
    assert_eq!(archive.words("\"gh\"").items.iter().map(|hit| hit.id).collect::<Vec<_>>(), vec![5]);
}
