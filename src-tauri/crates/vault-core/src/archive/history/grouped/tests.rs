//! Tests for grouped history search, one per failure mode in the module header.

use super::*;
use crate::config::project_paths_with_root;

pub(in crate::archive::history) const CHROME: i64 = 1;
pub(in crate::archive::history) const FIREFOX: i64 = 2;

/// A test archive: Chrome and Firefox profiles, then the URLs and visits each test adds. Shared with
/// the regex scan tests.
pub(in crate::archive::history) struct Archive {
    _dir: tempfile::TempDir,
    pub(in crate::archive::history) paths: ProjectPaths,
    pub(in crate::archive::history) config: AppConfig,
}

impl Archive {
    pub(in crate::archive::history) fn new() -> Self {
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
    pub(in crate::archive::history) fn url(
        &self,
        url_id: i64,
        profile: i64,
        url: &str,
        title: &str,
        times: &[i64],
    ) -> &Self {
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

    pub(in crate::archive::history) fn index(&self) {
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
    connection
        .execute_batch(
            "CREATE TEMP TABLE history_ranked_urls (url_id INTEGER PRIMARY KEY, score REAL NOT NULL);
             CREATE TEMP TABLE history_window_scope (url_id INTEGER PRIMARY KEY);",
        )
        .expect("ranked urls table");
    for sql in [
        RANKED_PAGES_SQL,
        RANKED_PAGE_TOTALS_SQL,
        window::TERMS_NEWEST_SQL,
        LIST_HISTORY_LEXICAL_SQL,
        COUNT_HISTORY_LEXICAL_SQL,
    ] {
        let mut statement =
            connection.prepare(&format!("EXPLAIN QUERY PLAN {sql}")).expect("plan statement");
        bind(&mut statement, ":ftsQuery", &"tokio").expect("bind");
        bind(&mut statement, ":lowUrlId", &i64::MIN).expect("bind");
        bind(&mut statement, ":highUrlId", &i64::MAX).expect("bind");
        bind(&mut statement, ":scanLimit", &50_001).expect("bind");
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
        if sql == window::TERMS_NEWEST_SQL {
            // The index hands out matches in `url_id` order; a sort would rank every match again.
            assert!(!plan.contains("TEMP B-TREE"), "the window scan does not sort:\n{plan}");
        }
    }
}

#[test]
fn url_filters_match_the_visit_list() {
    for sql in [LIST_HISTORY_LEXICAL_SQL, list_history_sql("newest"), list_history_sql("oldest")] {
        assert!(sql.contains(page_url_filters!()), "the URL filters drifted from the visit list");
    }
}

// The keyword window (`window.rs`): one test per failure mode in its header. The window cap is a
// parameter of `list_history_with_window`, so a cap of 3 stands in for the production 50,000.

impl Archive {
    fn capped(&self, query: HistoryQuery, cap: usize) -> HistoryQueryResponse {
        list_history_with_window(&self.paths, &self.config, None, query, cap)
            .expect("windowed search")
    }

    /// Every row, `limit` at a time, calling `between` after each page.
    fn walk_capped(
        &self,
        query: HistoryQuery,
        limit: u32,
        cap: usize,
        mut between: impl FnMut(&Self),
    ) -> Vec<HistoryEntry> {
        let mut rows = Vec::new();
        let mut cursor = None;
        loop {
            let page = self.capped(
                HistoryQuery {
                    include_total: Some(false),
                    limit: Some(limit),
                    cursor: cursor.clone(),
                    ..query.clone()
                },
                cap,
            );
            rows.extend(page.items);
            match page.next_cursor {
                Some(next) => cursor = Some(next),
                None => break rows,
            }
            between(self);
            assert!(rows.len() < 1_000, "the cursor walk does not end");
        }
    }
}

/// "tokio page" archived as url 1 to 5 (one visit each at `1_000 * id`, url 1 in Firefox), and a
/// page without the word.
fn five_tokio_pages() -> Archive {
    let archive = Archive::new();
    for id in 1..=5 {
        let profile = if id == 1 { FIREFOX } else { CHROME };
        archive.url(id, profile, &format!("https://p{id}.test/"), "tokio page", &[1_000 * id]);
    }
    archive.url(6, CHROME, "https://other.test/", "unrelated", &[6_000]);
    archive.index();
    archive
}

fn tokio(sort: &str, grouped: bool) -> HistoryQuery {
    HistoryQuery {
        q: Some("tokio".into()),
        sort: Some(sort.into()),
        group_by_url: Some(grouped),
        include_total: Some(true),
        limit: Some(100),
        ..Default::default()
    }
}

#[test]
fn a_term_under_the_cap_returns_exactly_what_it_did_before() {
    let archive = five_tokio_pages();
    for grouped in [true, false] {
        for sort in ["relevance", "newest", "oldest"] {
            let roomy = archive.capped(tokio(sort, grouped), 1_000);
            let tight = archive.capped(tokio(sort, grouped), 5);
            assert_eq!(urls(&tight.items), urls(&roomy.items), "{sort} grouped={grouped}");
            assert_eq!(roomy.items.len(), 5, "{sort} grouped={grouped}");
            for response in [&roomy, &tight] {
                assert!(!response.windowed, "{sort} grouped={grouped}");
                assert!(response.total_exact, "{sort} grouped={grouped}");
                assert_eq!(response.total, 5, "{sort} grouped={grouped}");
            }
        }
    }
    let rare = archive.capped(HistoryQuery { q: Some("unrelated".into()), ..tokio("", true) }, 1);
    assert_eq!(urls(&rare.items), ["https://other.test/"]);
    assert!(!rare.windowed && rare.total_exact, "one match fits a cap of one");
}

#[test]
fn a_term_over_the_cap_ranks_only_the_most_recently_archived_matches() {
    let archive = five_tokio_pages();
    let newest = archive.capped(tokio("newest", true), 3);
    assert_eq!(urls(&newest.items), ["https://p5.test/", "https://p4.test/", "https://p3.test/"]);
    assert!(newest.windowed);

    let visits = archive.capped(tokio("relevance", false), 3);
    assert!(visits.windowed, "the visit list is windowed too");
    let mut listed = urls(&visits.items);
    listed.sort_unstable();
    assert_eq!(listed, ["https://p3.test/", "https://p4.test/", "https://p5.test/"]);

    let oldest = archive.capped(tokio("oldest", true), 3);
    assert_eq!(
        urls(&oldest.items),
        ["https://p1.test/", "https://p2.test/", "https://p3.test/"],
        "oldest order windows the least recently archived matches"
    );

    let operators =
        archive.capped(HistoryQuery { q: Some("site:test".into()), ..tokio("newest", true) }, 2);
    assert!(operators.windowed, "operator-only searches are windowed the same way");
    assert_eq!(urls(&operators.items), ["https://other.test/", "https://p5.test/"]);
}

#[test]
fn the_best_match_survives_when_the_term_fits_the_window() {
    let archive = Archive::new();
    archive
        .url(1, CHROME, "https://best.test/", "tokio tokio tokio", &[1_000])
        .url(2, CHROME, "https://p2.test/", "tokio and many other words in a long title", &[2_000])
        .url(3, CHROME, "https://p3.test/", "tokio and many other words in a long title", &[3_000])
        .index();
    let fits = archive.capped(tokio("relevance", true), 3);
    assert!(!fits.windowed);
    assert_eq!(fits.items[0].url, "https://best.test/");

    // One URL past the cap, the oldest archived falls out, and the response says so.
    let over = archive.capped(tokio("relevance", true), 2);
    assert!(over.windowed && !over.total_exact);
    assert!(!urls(&over.items).contains(&"https://best.test/"));
}

#[test]
fn a_windowed_count_never_claims_exactness_and_matches_the_rows() {
    let archive = five_tokio_pages();
    for grouped in [true, false] {
        let counted = archive.capped(tokio("relevance", grouped), 3);
        assert!(counted.windowed, "grouped={grouped}");
        assert!(!counted.total_exact, "grouped={grouped}: a windowed total is a lower bound");
        assert_eq!(counted.total, 3, "grouped={grouped}");
        let walked = archive.walk_capped(tokio("relevance", grouped), 1, 3, |_| {});
        assert_eq!(walked.len(), counted.total, "grouped={grouped}");
        if grouped {
            let visits: u64 = walked.iter().map(|row| row.visit_count.unwrap_or(0)).sum();
            assert_eq!(Some(visits as usize), counted.total_visits);
        }
    }
}

#[test]
fn cursor_pages_keep_the_window_when_a_backup_lands_between_them() {
    let archive = five_tokio_pages();
    for sort in ["relevance", "newest", "oldest"] {
        for grouped in [true, false] {
            let all = archive.capped(tokio(sort, grouped), 3);
            let walked = archive.walk_capped(tokio(sort, grouped), 1, 3, |_| {});
            assert_eq!(urls(&walked), urls(&all.items), "{sort} grouped={grouped}");
        }
    }

    // A newer matching page archived after the first page would move an unpinned window past
    // url 3; the pinned window keeps url 3 and also shows the new page, visited long ago.
    let archive = five_tokio_pages();
    let mut added = false;
    let walked = archive.walk_capped(tokio("newest", true), 1, 3, |archive| {
        if !added {
            archive.url(7, CHROME, "https://p7.test/", "tokio page", &[500]);
            archive.index();
            added = true;
        }
    });
    assert_eq!(
        urls(&walked),
        ["https://p5.test/", "https://p4.test/", "https://p3.test/", "https://p7.test/"]
    );
}

#[test]
fn filters_choose_the_window_instead_of_trimming_it() {
    let archive = five_tokio_pages();
    // Both ways of testing a date range: the URLs visited in it, and a probe per match.
    for (scope, grouped) in [(i64::MAX, true), (i64::MAX, false), (0, true), (0, false)] {
        window::SCOPE_MAX_VISITS_FOR_TEST.with(|max| max.set(scope));
        let early = archive
            .capped(HistoryQuery { end_time_ms: Some(1_500), ..tokio("relevance", grouped) }, 3);
        assert_eq!(urls(&early.items), ["https://p1.test/"], "grouped={grouped}");
        assert!(!early.windowed && early.total_exact, "grouped={grouped}");

        let firefox = archive.capped(
            HistoryQuery { browser_kind: Some("firefox".into()), ..tokio("relevance", grouped) },
            3,
        );
        assert_eq!(urls(&firefox.items), ["https://p1.test/"], "grouped={grouped}");

        let domain = archive.capped(
            HistoryQuery { domain: Some("p2.test".into()), ..tokio("relevance", grouped) },
            1,
        );
        assert_eq!(urls(&domain.items), ["https://p2.test/"], "grouped={grouped}");
        assert!(!domain.windowed, "grouped={grouped}");

        // Four pages visited by 4,500 and a cap of two: the window is the newest two of those
        // four, never pages visited later.
        let before = archive
            .capped(HistoryQuery { end_time_ms: Some(4_500), ..tokio("newest", grouped) }, 2);
        assert!(before.windowed, "scope={scope} grouped={grouped}");
        assert_eq!(
            urls(&before.items),
            ["https://p4.test/", "https://p3.test/"],
            "scope={scope} grouped={grouped}"
        );
    }
    window::SCOPE_MAX_VISITS_FOR_TEST.with(|max| max.set(i64::MAX));
}

impl Archive {
    fn tag(&self, url: &str, tag: &str) -> &Self {
        let connection = open_archive_connection(&self.paths, &self.config, None).expect("open");
        connection
            .execute(
                "INSERT INTO url_tags (url, tag, created_at) VALUES (?1, ?2, '2026-05-01T00:00:00Z')",
                params![url, tag],
            )
            .expect("tag");
        self
    }
}

#[test]
fn tag_searches_list_only_pages_carrying_every_tag() {
    let archive = Archive::new();
    archive
        .url(1, CHROME, "https://tokio.rs/blog", "Tokio blog", &[1_000, 2_000])
        .url(2, FIREFOX, "https://tokio.rs/blog", "Tokio blog", &[3_000])
        .url(3, CHROME, "https://docs.rs/smol", "smol", &[4_000])
        .url(4, CHROME, "https://example.test/", "Example", &[5_000])
        .index();
    archive
        .tag("https://tokio.rs/blog", "Rust")
        .tag("https://tokio.rs/blog", "async runtime")
        .tag("https://docs.rs/smol", "rust");

    // Case-insensitive, one row per page, every browser's visits counted.
    let rust = archive.words("tag:rust");
    assert_eq!(urls(&rust.items), ["https://docs.rs/smol", "https://tokio.rs/blog"]);
    assert_eq!(rust.total, 2);
    assert_eq!(rust.total_visits, Some(4));
    // Two tags mean both; a quoted tag keeps its space.
    let both = archive.words("tag:rust tag:\"async runtime\"");
    assert_eq!(urls(&both.items), ["https://tokio.rs/blog"]);
    assert_eq!(both.items[0].visit_count, Some(3));
    // Other operators still apply on top of the tag.
    assert_eq!(urls(&archive.words("tag:rust site:docs.rs").items), ["https://docs.rs/smol"]);
    assert_eq!(
        urls(&archive.words("tag:rust -tag:\"async runtime\"").items),
        ["https://docs.rs/smol"]
    );
    assert!(archive.words("tag:missing").items.is_empty());
}

#[test]
fn tag_pages_start_from_the_tagged_urls_not_every_url() {
    let archive = Archive::new();
    let connection =
        open_archive_connection(&archive.paths, &archive.config, None).expect("open archive");
    prepare_advanced_search_filters(&connection, &ParsedHistorySearchQuery::default())
        .expect("filter tables");
    for sql in [TAGGED_PAGES_SQL, TAGGED_PAGE_TOTALS_SQL] {
        let mut statement =
            connection.prepare(&format!("EXPLAIN QUERY PLAN {sql}")).expect("plan statement");
        for name in [":profileId", ":browserKind", ":domainPattern", ":cursorScore", ":cursorUrl"] {
            bind(&mut statement, name, &Option::<String>::None).expect("bind");
        }
        bind(&mut statement, ":cursorTime", &Option::<i64>::None).expect("bind");
        bind(&mut statement, ":startTimeMs", &i64::MIN).expect("bind");
        bind(&mut statement, ":endTimeMs", &i64::MAX).expect("bind");
        bind(&mut statement, ":sort", &"newest").expect("bind");
        bind(&mut statement, ":pageLimit", &101).expect("bind");
        let mut rows = statement.raw_query();
        let mut plan = Vec::new();
        while let Some(row) = rows.next().expect("plan row") {
            plan.push(row.get::<_, String>(3).expect("plan detail"));
        }
        let plan = plan.join("\n");
        assert!(plan.contains("idx_urls_url"), "urls by address:\n{plan}");
        assert!(!plan.contains("SCAN urls"), "no scan of every url:\n{plan}");
        assert!(!plan.contains("SCAN visits"), "no scan of every visit:\n{plan}");
    }
}

#[test]
fn three_copies_of_one_page_stay_one_row_on_every_page_of_the_walk() {
    // The E2E fixture's "Why I left tokio" page lives in three profiles, with the newest visits
    // minutes apart; the list showed it twice. The query never did: its copies fold into one row
    // however the walk is cut (the duplicate was the timeline shown as search results while the
    // search loaded, fixed in `src/features/history/queries.ts`).
    let archive = Archive::new();
    let connection = open_archive_connection(&archive.paths, &archive.config, None).expect("open");
    connection
        .execute(
            "INSERT INTO source_profiles (id, browser_kind, profile_name, profile_path,
               discovered_at, enabled, profile_key)
             VALUES (3, 'chrome', 'work', '/tmp/work', '2026-05-01T00:00:00Z', 1, 'chrome:Work')",
            [],
        )
        .expect("third profile");
    let page = "https://news.ycombinator.com/item?id=41502281";
    let title = "Hacker News · Why I left tokio";
    archive
        .url(1, FIREFOX, page, title, &[9_000, 9_480, 9_530])
        .url(2, CHROME, page, title, &[5_000, 9_500])
        .url(3, 3, page, title, &[7_000])
        .url(4, CHROME, "https://tokio.rs/", "tokio", &[9_490])
        .url(5, FIREFOX, "https://example.test/left", "why I left my job", &[9_520])
        .index();

    for query in ["why I left tokio", "tokio"] {
        for sort in ["relevance", "newest", "oldest"] {
            let search = HistoryQuery {
                q: Some(query.into()),
                sort: Some(sort.into()),
                ..Default::default()
            };
            for limit in [1, 2, 100] {
                let walked = archive.walk(search.clone(), limit);
                let copies = walked.iter().filter(|row| row.url == page).collect::<Vec<_>>();
                assert_eq!(copies.len(), 1, "{query} {sort} limit {limit}: {:?}", urls(&walked));
                assert_eq!(copies[0].visit_count, Some(6), "{query} {sort} limit {limit}");
                assert_eq!(copies[0].visit_time, 9_530, "{query} {sort} limit {limit}");
            }
        }
    }
}
