//! Responsibilities: preserve grouped search behavior on the owner engine.
//! Not responsible for: FTS5 SQL or archive timeline reads.
//! Failure modes: duplicate profiles, wrong representative visit, wrong filtered counts,
//! tie cursor loss, regex/fuzzy drift and annotation filters. Ported from history/grouped.
use super::fixture::{Archive, CHROME, FIREFOX};
use crate::{HistoryEntry, HistoryQuery};
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
            "https://b.test/tokio",
            "https://a.test/tokio",
            "https://d.test/tokio",
            "https://c.test/tokio",
            "https://e.test/tokio"
        ],
        "newest visit first, then page ID descending"
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
        let response = super::history(
            &archive.vault,
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
fn three_copies_of_one_page_stay_one_row_on_every_page_of_the_walk() {
    // The E2E fixture's "Why I left tokio" page lives in three profiles, with the newest visits
    // minutes apart; the list showed it twice. The query never did: its copies fold into one row
    // however the walk is cut (the duplicate was the timeline shown as search results while the
    // search loaded, fixed in `src/features/history/queries.ts`).
    let archive = Archive::new();
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

#[test]
fn tag_pages_start_from_tag_postings_not_every_page() {
    let archive = Archive::with_key(Some([108; 32]));
    archive
        .url(1, CHROME, "https://tags.test/1", "ordinary", &[1])
        .url(2, CHROME, "https://tags.test/2", "ordinary", &[2])
        .index();
    archive.tag("https://tags.test/1", "chosen");
    let query = super::query::build("tag:chosen", false);
    let candidates = archive
        .vault
        .search()
        .index()
        .unwrap()
        .search(query.ast, Default::default())
        .unwrap()
        .count()
        .unwrap();
    assert_eq!(candidates, 1);
    assert_eq!(archive.words("tag:chosen").total, 1);
}

#[test]
fn a_tag_filter_does_not_become_an_extra_or_branch() {
    let archive = Archive::new();
    archive
        .url(1, CHROME, "https://tags.test/1", "alpha", &[1])
        .url(2, CHROME, "https://tags.test/2", "beta", &[2])
        .index();
    archive.tag("https://tags.test/1", "chosen").tag("https://tags.test/2", "chosen");
    assert_eq!(urls(&archive.words("alpha OR tag:chosen").items), ["https://tags.test/1"]);
    assert_eq!(archive.words("alpha OR beta tag:chosen").total, 2);
}
