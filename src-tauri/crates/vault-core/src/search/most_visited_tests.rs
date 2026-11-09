//! Responsibilities: prove the most-visited order on real Vault owners, through the index
//! (posting batches, and date-scoped membership) and through both regex sources.
//! Not responsible for: the engine's brute-force contract (vault-search contracts) or the
//! History screen.
//! Failure modes, written down before the code:
//! - pages that tie on visits come in no fixed order (ties: newest visit, then page ID);
//! - a cursor equal to the next page's key repeats or skips it;
//! - visits outside the date scope, or revoked ones, count toward the order or the row;
//! - a page with a URL in two profiles shows twice or counts only one of them;
//! - a regex reading only the pages with its fragment ranks differently from one reading
//!   every visit;
//! - a regex walk ranks a page by the visits it has read so far, so the most visited page,
//!   met last and across chunk edges, drops out of a small page;
//! - a step before the page is ranked and counted hands out a cursor;
//! - the fuzzy fallback forgets the order.
use super::{
    SearchRequest, SearchSession,
    fixture::{Archive, CHROME, FIREFOX},
};
use crate::HistoryQuery;
use std::time::Duration;
use vault_store::{Db, Priority};

/// One row as the list shows it: the URL, the visit shown and its visit count.
type Row = (String, i64, u64);

/// The same request three ways: the index, a regex reading only the pages with its
/// fragment ("story"), and a regex with no fragment, which reads every visit.
const PATHS: [(&str, bool); 3] = [("story", false), ("story", true), ("st.ry", true)];

fn archive() -> Archive {
    let archive = Archive::with_key(Some([121; 32]));
    archive
        // One page, a URL in two profiles: five visits.
        .url(1, CHROME, "https://a.test/story", "story alpha", &[100, 200, 300])
        .url(2, FIREFOX, "https://a.test/story", "story alpha", &[150, 250])
        // B and C tie on four visits; C's newest is newer.
        .url(3, CHROME, "https://b.test/story", "story beta", &[400, 500, 600, 700])
        .url(4, CHROME, "https://c.test/story", "story gamma", &[800, 900, 1000, 1100])
        // D and E tie on two visits and on their newest; E has the higher page ID.
        .url(5, CHROME, "https://d.test/story", "story delta", &[1200, 1300])
        .url(6, FIREFOX, "https://e.test/story", "story epsilon", &[1300, 50])
        .url(7, CHROME, "https://f.test/story", "story zeta", &[5000])
        // Most visited, but all except one visit before 1,000.
        .url(
            8,
            CHROME,
            "https://g.test/story",
            "story eta",
            &[10, 20, 30, 40, 50, 60, 70, 80, 90, 100, 2000],
        )
        .url(9, CHROME, "https://h.test/other", "unrelated", &[6000])
        .index();
    archive
}
/// Rows of `https://<host>/story` pages.
fn rows(rows: &[(&str, i64, u64)]) -> Vec<Row> {
    rows.iter()
        .map(|(host, time, visits)| (format!("https://{host}/story"), *time, *visits))
        .collect()
}
fn everything() -> Vec<Row> {
    rows(&[
        ("g.test", 2000, 11),
        ("a.test", 300, 5),
        ("c.test", 1100, 4),
        ("b.test", 700, 4),
        ("e.test", 1300, 2),
        ("d.test", 1300, 2),
        ("f.test", 5000, 1),
    ])
}
fn request(query: &str, regex: bool, page_size: usize) -> SearchRequest {
    SearchRequest {
        query: query.into(),
        regex,
        sort: Some("visits".into()),
        page_size,
        ..Default::default()
    }
}
/// Every page of one session, a page at a time.
fn walk(archive: &Archive, request: SearchRequest) -> Vec<Row> {
    let mut session = SearchSession::start(&archive.vault, request).unwrap();
    let mut rows = Vec::new();
    loop {
        let page = session.finish_page().unwrap();
        rows.extend(
            page.items
                .into_iter()
                .map(|hit| (hit.visit.url, hit.visit.visit_time, hit.visit.visit_count.unwrap())),
        );
        let Some(cursor) = page.cursor else { break };
        session.next_page(&cursor).unwrap();
    }
    rows
}

#[test]
fn most_visited_goes_by_visits_in_scope_then_newest_visit_then_page_on_every_path() {
    let archive = archive();
    // From 1,000 on only the visits since then count: G keeps one of its eleven, C two of
    // its four, and A and B have none left.
    let scoped = rows(&[
        ("d.test", 1300, 2),
        ("c.test", 1100, 2),
        ("f.test", 5000, 1),
        ("g.test", 2000, 1),
        ("e.test", 1300, 1),
    ]);
    for (query, regex) in PATHS {
        for size in [1, 2, 3, 100] {
            let request = request(query, regex, size);
            assert_eq!(walk(&archive, request.clone()), everything(), "{query} {regex} {size}");
            let request = SearchRequest { start_time_ms: Some(1000), ..request };
            assert_eq!(walk(&archive, request), scoped, "{query} {regex} {size} from 1000");
        }
    }
    let history = archive.walk(
        HistoryQuery { q: Some("story".into()), sort: Some("visits".into()), ..Default::default() },
        2,
    );
    let history =
        history.into_iter().map(|row| (row.url, row.visit_time, row.visit_count.unwrap()));
    assert_eq!(history.collect::<Vec<_>>(), everything(), "the paged history API");
}

#[test]
fn revoked_visits_count_toward_neither_the_order_nor_the_row() {
    let archive = archive();
    // Before the search starts, A loses its Chrome URL's three visits and G its ten oldest.
    archive
        .vault
        .store()
        .write(Db::Archive, Priority::Batch, |s| {
            s.execute(
                "UPDATE visits SET reverted_at='now' WHERE url_id=1 OR (url_id=8 AND visit_time_ms<2000)",
                (),
            )?;
            for url in ["https://a.test/story", "https://g.test/story"] {
                let page =
                    s.query_one("SELECT id FROM pages WHERE url=?1", [url], |r| Ok(r.get::<i64>(0)?))?;
                crate::archive::search_documents::refresh(s, page)?;
            }
            Ok(())
        })
        .unwrap();
    archive.index();
    let expected = rows(&[
        ("c.test", 1100, 4),
        ("b.test", 700, 4),
        ("e.test", 1300, 2),
        ("d.test", 1300, 2),
        ("a.test", 250, 2),
        ("f.test", 5000, 1),
        ("g.test", 2000, 1),
    ]);
    for (query, regex) in PATHS {
        for size in [1, 3] {
            assert_eq!(
                walk(&archive, request(query, regex, size)),
                expected,
                "{query} {regex} {size}"
            );
        }
    }
}

#[test]
fn no_step_hands_out_a_cursor_before_its_page_is_ranked_and_counted() {
    let archive = archive();
    for (query, regex) in PATHS {
        let mut session = SearchSession::start(&archive.vault, request(query, regex, 2)).unwrap();
        loop {
            let step = session.step(Duration::ZERO).unwrap();
            if step.provisional || !step.count.exact {
                assert_eq!(step.cursor, None, "{query} {regex}");
                continue;
            }
            let urls = step.items.iter().map(|hit| hit.visit.url.as_str()).collect::<Vec<_>>();
            assert_eq!(urls, ["https://g.test/story", "https://a.test/story"], "{query} {regex}");
            assert!(step.cursor.is_some(), "{query} {regex}");
            break;
        }
    }
}

#[test]
fn a_regex_walk_ranks_the_page_it_meets_last_by_every_visit() {
    let archive = Archive::with_key(Some([122; 32]));
    // A walk reads visits newest first, 512 at a time. G's 30 visits are its oldest matches,
    // spread among 600 visits that don't match, so it is met last and across a chunk edge.
    let filler = (1..=600).map(|i| i * 10).collect::<Vec<_>>();
    let g = (0..30).map(|i| 5 + i * 200).collect::<Vec<_>>();
    archive
        .url(1, CHROME, "https://filler.test/", "filler", &filler)
        .url(2, CHROME, "https://g.test/story", "story g", &g)
        .url(3, CHROME, "https://a.test/story", "story a", &[7000, 7100, 7200])
        .url(4, CHROME, "https://b.test/story", "story b", &[8000, 8100])
        .url(5, FIREFOX, "https://c.test/story", "story c", &[9000])
        .index();
    let expected = rows(&[
        ("g.test", 5805, 30),
        ("a.test", 7200, 3),
        ("b.test", 8100, 2),
        ("c.test", 9000, 1),
    ]);
    for (query, regex) in PATHS {
        for size in [1, 2, 4] {
            assert_eq!(
                walk(&archive, request(query, regex, size)),
                expected,
                "{query} {regex} {size}"
            );
        }
    }
}

#[test]
fn the_fuzzy_fallback_keeps_the_order() {
    let archive = archive();
    // No page has "storx"; the fallback finds "story" with one edit.
    assert_eq!(walk(&archive, request("storx", false, 2)), everything());
}
