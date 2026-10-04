//! Tests for grouped history search, one per failure mode in the module header.

use super::*;
use crate::config::project_paths_with_root;

const CHROME: i64 = 1;
const FIREFOX: i64 = 2;

/// A test archive: Chrome and Firefox profiles, then the URLs and visits each test adds.
struct Archive {
    _dir: tempfile::TempDir,
    paths: ProjectPaths,
    config: AppConfig,
}

impl Archive {
    fn new() -> Self {
        let dir = tempfile::tempdir().expect("tempdir");
        let paths = project_paths_with_root(dir.path());
        let config = AppConfig::default();
        let connection = open_archive_connection(&paths, &config, None).expect("open archive");
        for (id, kind, key) in
            [(CHROME, "chrome", "chrome:Default"), (FIREFOX, "firefox", "firefox:main")]
        {
            connection
                .execute(
                    "INSERT INTO source_profiles (id, browser_kind, profile_name, profile_path,
                       discovered_at, enabled, profile_key)
                     VALUES (?1, ?2, 'profile', '/tmp/profile', '2026-05-01T00:00:00Z', 1, ?3)",
                    params![id, kind, key],
                )
                .expect("profile");
        }
        Self { _dir: dir, paths, config }
    }

    /// Adds one `urls` row (one profile's copy of an address) and its visits at `times`.
    fn url(&self, url_id: i64, profile: i64, url: &str, title: &str, times: &[i64]) -> &Self {
        let connection = open_archive_connection(&self.paths, &self.config, None).expect("open");
        connection
            .execute(
                "INSERT INTO urls (id, url, title, visit_count, typed_count, first_visit_ms,
                   first_visit_iso, last_visit_ms, last_visit_iso, source_profile_id,
                   created_by_run_id, source_url_id, hidden)
                 VALUES (?1, ?2, ?3, 0, 0, 0, '', 0, '', ?4, 0, ?1, 0)",
                params![url_id, url, title, profile],
            )
            .expect("url");
        for time in times {
            connection
                .execute(
                    "INSERT INTO visits (url_id, source_visit_id, visit_time_ms, visit_time_iso,
                       source_profile_id, created_by_run_id)
                     VALUES (?1, ?2, ?3, '', ?4, 0)",
                    params![url_id, format!("{url_id}-{time}"), time, profile],
                )
                .expect("visit");
        }
        self
    }

    fn index(&self) {
        rebuild_search_projection(&self.paths, &self.config, None).expect("search projection");
    }

    fn search(&self, query: HistoryQuery) -> HistoryQueryResponse {
        list_history(
            &self.paths,
            &self.config,
            None,
            HistoryQuery { group_by_url: Some(true), include_total: Some(true), ..query },
        )
        .expect("grouped search")
    }

    fn words(&self, q: &str) -> HistoryQueryResponse {
        self.search(HistoryQuery { q: Some(q.to_string()), ..HistoryQuery::default() })
    }

    /// Every row of a search, read `limit` at a time through the cursor.
    fn walk(&self, query: HistoryQuery, limit: u32) -> Vec<HistoryEntry> {
        let mut rows = Vec::new();
        let mut cursor = None;
        loop {
            let page = list_history(
                &self.paths,
                &self.config,
                None,
                HistoryQuery {
                    group_by_url: Some(true),
                    include_total: Some(false),
                    limit: Some(limit),
                    cursor: cursor.clone(),
                    ..query.clone()
                },
            )
            .expect("page");
            assert_eq!(page.has_previous, cursor.is_some());
            rows.extend(page.items);
            match page.next_cursor {
                Some(next) => cursor = Some(next),
                None => break rows,
            }
            assert!(rows.len() < 1_000, "the cursor walk does not end");
        }
    }
}

fn urls(rows: &[HistoryEntry]) -> Vec<&str> {
    rows.iter().map(|row| row.url.as_str()).collect()
}

#[test]
fn one_page_visited_from_two_browsers_is_one_row_with_every_visit_counted() {
    let archive = Archive::new();
    archive
        .url(1, CHROME, "https://tokio.rs/blog", "Tokio blog", &[1_000, 2_000, 3_000])
        .url(2, FIREFOX, "https://tokio.rs/blog", "Tokio blog", &[4_000, 5_000])
        .url(3, CHROME, "https://example.test/other", "Unrelated page", &[6_000])
        .index();

    let response = archive.words("tokio");

    assert_eq!(urls(&response.items), ["https://tokio.rs/blog"]);
    let row = &response.items[0];
    assert_eq!(row.visit_count, Some(5));
    assert_eq!(row.visit_time, 5_000, "the row is the newest visit");
    assert_eq!(row.profile_id, "firefox:main");
    assert_eq!((response.total, response.total_visits, response.total_exact), (1, Some(5), true));
}

#[test]
fn the_row_is_the_newest_visit_even_when_another_copy_ranks_higher() {
    // Chrome's copy repeats the word in its title and ranks better; Firefox has the newest visit.
    let archive = Archive::new();
    archive
        .url(1, CHROME, "https://example.test/runtime", "tokio tokio tokio guide", &[1_000, 9_000])
        .url(2, FIREFOX, "https://example.test/runtime", "runtime notes tokio", &[9_500])
        .index();

    for sort in ["relevance", "newest", "oldest"] {
        let response = archive.search(HistoryQuery {
            q: Some("tokio".into()),
            sort: Some(sort.into()),
            ..Default::default()
        });
        assert_eq!(response.items.len(), 1, "{sort}");
        let row = &response.items[0];
        assert_eq!(row.visit_time, 9_500, "{sort}");
        assert_eq!(row.profile_id, "firefox:main", "{sort}");
        assert_eq!(row.title.as_deref(), Some("runtime notes tokio"), "{sort}");
        assert_eq!(row.visit_count, Some(3), "{sort}");
    }
}

#[test]
fn browser_and_date_filters_apply_to_the_count_and_the_row() {
    let archive = Archive::new();
    archive
        .url(1, CHROME, "https://tokio.rs/", "Tokio", &[1_000, 2_000, 8_000])
        .url(2, FIREFOX, "https://tokio.rs/", "Tokio", &[3_000, 4_000])
        .index();

    let firefox = archive.search(HistoryQuery {
        q: Some("tokio".into()),
        browser_kind: Some("firefox".into()),
        ..Default::default()
    });
    assert_eq!(firefox.items.len(), 1);
    assert_eq!(firefox.items[0].visit_count, Some(2));
    assert_eq!(firefox.items[0].visit_time, 4_000);
    assert_eq!(firefox.total_visits, Some(2));

    let early = archive.search(HistoryQuery {
        q: Some("tokio".into()),
        start_time_ms: Some(1_500),
        end_time_ms: Some(3_500),
        ..Default::default()
    });
    assert_eq!(early.items[0].visit_count, Some(2), "the visits at 2,000 and 3,000");
    assert_eq!(early.items[0].visit_time, 3_000);
    assert_eq!(early.items[0].profile_id, "firefox:main");

    let profile = archive.search(HistoryQuery {
        q: Some("tokio".into()),
        profile_id: Some("chrome:Default".into()),
        ..Default::default()
    });
    assert_eq!(profile.items[0].visit_count, Some(3));

    let none = archive.search(HistoryQuery {
        q: Some("tokio".into()),
        start_time_ms: Some(10_000),
        ..Default::default()
    });
    assert!(none.items.is_empty());
    assert_eq!((none.total, none.total_visits), (0, Some(0)));
}

#[test]
fn the_cursor_reaches_every_page_once_even_when_pages_tie() {
    // Five pages, two of them with their newest visit at the same moment, and equal titles so
    // their relevance ties too.
    let archive = Archive::new();
    archive
        .url(1, CHROME, "https://a.test/tokio", "tokio page", &[1_000, 5_000])
        .url(2, CHROME, "https://b.test/tokio", "tokio page", &[5_000])
        .url(3, FIREFOX, "https://c.test/tokio", "tokio page", &[3_000])
        .url(4, FIREFOX, "https://a.test/tokio", "tokio page", &[2_000])
        .url(5, CHROME, "https://d.test/tokio", "tokio page", &[4_000, 4_500])
        .url(6, FIREFOX, "https://e.test/tokio", "tokio page", &[700])
        .index();

    for sort in ["relevance", "newest", "oldest"] {
        let query =
            HistoryQuery { q: Some("tokio".into()), sort: Some(sort.into()), ..Default::default() };
        let all = archive.search(HistoryQuery { limit: Some(100), ..query.clone() });
        let walked = archive.walk(query.clone(), 1);
        assert_eq!(urls(&walked), urls(&all.items), "{sort}: one at a time equals one big page");
        assert_eq!(all.total, 5, "{sort}");
        assert_eq!(walked.len(), all.total, "{sort}: the page total matches the rows");
        let visits: u64 = walked.iter().map(|row| row.visit_count.unwrap_or(0)).sum();
        assert_eq!(Some(visits as usize), all.total_visits, "{sort}: the visit total too");
        assert_eq!(all.total_visits, Some(8), "{sort}");
    }

    let newest = archive.walk(
        HistoryQuery { q: Some("tokio".into()), sort: Some("newest".into()), ..Default::default() },
        2,
    );
    assert_eq!(
        urls(&newest),
        [
            "https://a.test/tokio",
            "https://b.test/tokio",
            "https://d.test/tokio",
            "https://c.test/tokio",
            "https://e.test/tokio"
        ],
        "newest visit first, then the URL"
    );
}

#[test]
fn regex_groups_the_same_way() {
    let archive = Archive::new();
    archive
        .url(1, CHROME, "https://news.test/item?id=42", "Story", &[1_000, 2_000])
        .url(2, FIREFOX, "https://news.test/item?id=42", "Story", &[3_000])
        .url(3, CHROME, "https://news.test/item?id=43", "Other story", &[4_000])
        .index();
    let regex = |pattern: &str, browser: Option<&str>| HistoryQuery {
        q: Some(pattern.to_string()),
        regex_mode: Some(true),
        browser_kind: browser.map(str::to_string),
        ..Default::default()
    };

    let response = archive.search(regex(r"item\?id=4\d$", None));
    assert_eq!(
        urls(&response.items),
        ["https://news.test/item?id=43", "https://news.test/item?id=42"]
    );
    assert_eq!(response.items[1].visit_count, Some(3));
    assert_eq!(response.items[1].visit_time, 3_000);
    assert_eq!((response.total, response.total_visits), (2, Some(4)));

    let firefox = archive.search(regex(r"id=42$", Some("firefox")));
    assert_eq!(firefox.items[0].visit_count, Some(1));
    assert_eq!(urls(&archive.walk(regex(r"item", None), 1)), urls(&response.items));
}

#[test]
fn a_typo_falls_back_to_fuzzy_matching_and_still_groups() {
    let archive = Archive::new();
    archive
        .url(1, CHROME, "https://example.test/kubernetes", "Kubernetes handbook", &[1_000])
        .url(2, FIREFOX, "https://example.test/kubernetes", "Kubernetes handbook", &[2_000])
        .index();

    for include_total in [true, false] {
        let response = list_history(
            &archive.paths,
            &archive.config,
            None,
            HistoryQuery {
                q: Some("kubernetis".into()),
                group_by_url: Some(true),
                include_total: Some(include_total),
                ..Default::default()
            },
        )
        .expect("fuzzy");
        assert_eq!(urls(&response.items), ["https://example.test/kubernetes"], "{include_total}");
        assert_eq!(response.items[0].visit_count, Some(2));
        assert_eq!(response.total_visits, Some(2));
    }
}

#[test]
fn operator_only_searches_group_every_url_the_operators_allow() {
    let archive = Archive::new();
    archive
        .url(1, CHROME, "https://github.com/tokio-rs/tokio", "tokio", &[1_000, 2_000])
        .url(2, FIREFOX, "https://github.com/tokio-rs/tokio", "tokio", &[3_000])
        .url(3, CHROME, "https://example.test/", "Example", &[4_000])
        .index();

    let response = archive.words("site:github.com");
    assert_eq!(urls(&response.items), ["https://github.com/tokio-rs/tokio"]);
    assert_eq!(response.items[0].visit_count, Some(3));
    assert_eq!(response.total_visits, Some(3));
}

#[test]
fn browsing_without_text_still_lists_every_visit() {
    let archive = Archive::new();
    archive.url(1, CHROME, "https://tokio.rs/", "Tokio", &[1_000, 2_000]).index();
    let response = archive.search(HistoryQuery::default());
    assert_eq!(response.items.len(), 2);
    assert!(response.items.iter().all(|row| row.visit_count.is_none()));
    assert_eq!(response.total_visits, None);
}

#[test]
fn keyword_pages_read_visits_through_the_url_time_index() {
    let archive = Archive::new();
    let connection =
        open_archive_connection(&archive.paths, &archive.config, None).expect("open archive");
    prepare_advanced_search_filters(&connection, &ParsedHistorySearchQuery::default())
        .expect("filter tables");
    for sql in [KEYWORD_PAGES_SQL, KEYWORD_PAGE_TOTALS_SQL] {
        let mut statement =
            connection.prepare(&format!("EXPLAIN QUERY PLAN {sql}")).expect("plan statement");
        for name in [":termsFtsQuery", ":trigramFtsQuery"] {
            bind(&mut statement, name, &"tokio").expect("bind");
        }
        for name in [":profileId", ":browserKind", ":domainPattern", ":cursorScore", ":cursorUrl"] {
            bind(&mut statement, name, &Option::<String>::None).expect("bind");
        }
        bind(&mut statement, ":cursorTime", &Option::<i64>::None).expect("bind");
        bind(&mut statement, ":startTimeMs", &i64::MIN).expect("bind");
        bind(&mut statement, ":endTimeMs", &i64::MAX).expect("bind");
        bind(&mut statement, ":sort", &"relevance").expect("bind");
        bind(&mut statement, ":pageLimit", &101).expect("bind");
        let mut rows = statement.raw_query();
        let mut plan = Vec::new();
        while let Some(row) = rows.next().expect("plan row") {
            plan.push(row.get::<_, String>(3).expect("plan detail"));
        }
        let plan = plan.join("\n");
        assert!(plan.contains("idx_visits_visible_url_time"), "visits by url and time:\n{plan}");
        assert!(!plan.contains("SCAN visits"), "no scan of every visit:\n{plan}");
    }
}

#[test]
fn url_filters_match_the_visit_list() {
    for sql in [LIST_HISTORY_LEXICAL_SQL, list_history_sql("newest"), list_history_sql("oldest")] {
        assert!(sql.contains(page_url_filters!()), "the URL filters drifted from the visit list");
    }
}
