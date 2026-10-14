//! Responsibilities: preserve grouped search behavior on the owner engine.
//! Not responsible for: FTS5 SQL or archive timeline reads.
//! Failure modes: tie cursor loss, one page shown as several rows across a long walk, and
//! tag-only searches that read every page. Ported from history/grouped.
use super::fixture::{Archive, CHROME, FIREFOX};
use crate::{HistoryEntry, HistoryQuery};
fn urls(rows: &[HistoryEntry]) -> Vec<&str> {
    rows.iter().map(|row| row.url.as_str()).collect()
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
                // The row shows the visit the order goes by: the first for oldest first.
                let shown = if sort == "oldest" { 5_000 } else { 9_530 };
                assert_eq!(copies[0].visit_time, shown, "{query} {sort} limit {limit}");
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
