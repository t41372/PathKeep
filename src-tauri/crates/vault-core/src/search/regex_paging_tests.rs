//! Responsibilities: prove that a regex search's later pages follow from what its first page
//! read, in every order and from both sources, without reading every visit again.
//! Not responsible for: the first page's ranking (`most_visited_tests`, `ported_regex`) or the
//! History screen.
//! Failure modes, written down before the code:
//! - a later page reads every visit again, one more walk of the archive per page;
//! - a walk in time order that goes on from the cursor skips a page that has the cursor's time
//!   and ranks after it (equal times rank by page ID, a walk meets pages by visit ID), or ends
//!   its page before reading every visit of the page's last time;
//! - a page already returned comes back on a later page;
//! - a page says another follows when none does, or ends short before the last;
//! - the keys kept for later pages run out and the next page skips or repeats a page, or the
//!   scan never reads its source again for the rest;
//! - a walk ranking by visits holds on to the memory it took for every matching page after it
//!   ends, though it keeps only the next pages' keys;
//! - the page and visit totals start over, or change, on a later page;
//! - reading only the pages with the fragment and walking every visit page differently.
use super::{
    SearchRequest, SearchSession, SearchStep,
    fixture::{Archive, CHROME, FIREFOX},
    regex::KEPT_PAGES,
};

/// One row as the list shows it: the URL, the visit shown and its visit count.
type Row = (String, i64, u64);

/// "story" reads only the pages with its fragment; "st.ry" has none, so it walks every visit.
const SOURCES: [&str; 2] = ["story", "st.ry"];
const ORDERS: [&str; 3] = ["newest", "oldest", "visits"];

fn archive() -> Archive {
    let archive = Archive::with_key(Some([131; 32]));
    // Three hundred stories ten apart, the 7th one visited three times, and 2,100 visits that
    // never match spread between them: a walk crosses several 512-visit chunks, and a page of
    // twenty stories takes one.
    for id in 1..=300 {
        let times = if id == 7 { vec![70, 1500, 2990] } else { vec![id * 10] };
        archive.url(id, CHROME, &format!("https://s.test/{id}"), &format!("story {id}"), &times);
    }
    let filler = (0..2100).map(|i| 2 + i * 5 / 3).collect::<Vec<_>>();
    archive.url(400, CHROME, "https://filler.test/", "filler", &filler);
    // The newest time, shared by three pages whose visit IDs run against their page IDs: A is
    // created first but visited after B, so a newest-first walk meets C, A, B while the list
    // shows C, B, A.
    archive
        .url(401, CHROME, "https://tie.test/a", "story tie a", &[])
        .url(402, CHROME, "https://tie.test/b", "story tie b", &[5000])
        .url(403, FIREFOX, "https://tie.test/a", "story tie a", &[5000])
        .url(404, CHROME, "https://tie.test/c", "story tie c", &[5000]);
    // The oldest time, shared by three pages in creation order: an oldest-first walk meets D,
    // E, F while the list shows F, E, D.
    archive
        .url(405, CHROME, "https://tie.test/d", "story tie d", &[1])
        .url(406, CHROME, "https://tie.test/e", "story tie e", &[1])
        .url(407, CHROME, "https://tie.test/f", "story tie f", &[1])
        .index();
    archive
}

fn request(query: &str, sort: &str, page_size: usize) -> SearchRequest {
    SearchRequest {
        query: query.into(),
        regex: true,
        sort: Some(sort.into()),
        page_size,
        ..Default::default()
    }
}

fn rows(step: &SearchStep) -> Vec<Row> {
    step.items
        .iter()
        .map(|hit| {
            (hit.visit.url.clone(), hit.visit.visit_time, hit.visit.visit_count.unwrap_or(0))
        })
        .collect()
}

/// The rows each page of `total` rows holds: every page is full but the last.
fn full_pages(total: usize, size: usize) -> Vec<usize> {
    let mut rows = vec![size; total / size];
    if total % size > 0 {
        rows.push(total % size);
    }
    rows
}

/// The rows on each page.
fn sizes(pages: &[SearchStep]) -> Vec<usize> {
    pages.iter().map(|page| page.items.len()).collect()
}

/// Every page of one session, and each page's last step.
fn pages(archive: &Archive, request: SearchRequest) -> Vec<SearchStep> {
    let mut session = SearchSession::start(&archive.vault, request).unwrap();
    let mut pages = Vec::new();
    loop {
        let page = session.finish_page().unwrap();
        let cursor = page.cursor.clone();
        pages.push(page);
        let Some(cursor) = cursor else { break };
        session.next_page(&cursor).unwrap();
    }
    pages
}

#[test]
fn every_page_of_a_regex_search_continues_its_first_page_in_every_order() {
    let archive = archive();
    for order in ORDERS {
        let whole = pages(&archive, request("story", order, 10_000)).remove(0);
        assert_eq!(whole.count.value, 306, "{order}");
        assert_eq!(whole.visits.value, 308, "{order}");
        for query in SOURCES {
            for size in [1, 2, 3, 7, 50] {
                let pages = pages(&archive, request(query, order, size));
                assert_eq!(
                    sizes(&pages),
                    full_pages(rows(&whole).len(), size),
                    "{query} {order} {size}: rows a page"
                );
                for page in &pages {
                    assert!(page.count.exact, "{query} {order} {size}");
                    assert_eq!(
                        (page.count.value, page.visits.value),
                        (whole.count.value, whole.visits.value),
                        "{query} {order} {size}: totals on page {}",
                        page.page
                    );
                }
                assert_eq!(
                    pages.iter().flat_map(rows).collect::<Vec<_>>(),
                    rows(&whole),
                    "{query} {order} {size}"
                );
            }
        }
    }
}

#[test]
fn a_later_page_of_a_regex_search_does_not_read_every_visit_again() {
    let archive = archive();
    for query in SOURCES {
        for order in ORDERS {
            let walks_on = query == "st.ry" && order != "visits";
            let twenty = pages(&archive, request(query, order, 20));
            assert!(twenty.len() > 2, "{query} {order}");
            let first = twenty[0].progress.documents_processed;
            for page in &twenty[1..] {
                let read = page.progress.documents_processed;
                if walks_on {
                    // A walk in time order goes on from the cursor's time.
                    assert!(read < first / 2, "{query} {order}: {read} of {first}");
                } else {
                    // The keys kept cover every page after the first.
                    assert_eq!(read, 0, "{query} {order}: page {}", page.page);
                }
            }
            if !walks_on {
                // One row a page keeps 200 keys, fewer than the 306 matches, so the scan reads
                // its source again once, for the rest.
                let single = pages(&archive, request(query, order, 1));
                let reads =
                    single[1..].iter().filter(|page| page.progress.documents_processed > 0).count();
                assert_eq!(reads, 1, "{query} {order}");
            }
        }
    }
}

#[test]
fn the_keys_kept_run_out_with_a_page_left_and_the_pages_after_them_stay_whole() {
    // Four rows a page: the scan reads five a page to know if another follows, and keeps the
    // keys of a hundred such pages. They run out with a whole page left to show, so the page
    // after them is the first to come from reading the source again.
    let size = 4;
    let kept = (size + 1) * KEPT_PAGES;
    assert_eq!(kept % size, 0, "the keys kept run out with a whole page left");
    let matches = kept + 30;
    let archive = Archive::with_key(Some([134; 32]));
    // Each page has one to three visits, so most visited does not follow the time order.
    for id in 1..=matches as i64 {
        let times = (0..=id % 3).map(|n| id * 10 + n).collect::<Vec<_>>();
        archive.url(id, CHROME, &format!("https://k.test/{id}"), "story", &times);
    }
    archive.index();
    for (query, order) in
        [("story", "newest"), ("story", "oldest"), ("story", "visits"), ("st.ry", "visits")]
    {
        let whole = pages(&archive, request(query, order, 10_000)).remove(0);
        assert_eq!(whole.count.value, matches as u64, "{query} {order}");
        let pages = pages(&archive, request(query, order, size));
        assert_eq!(sizes(&pages), full_pages(matches, size), "{query} {order}: rows a page");
        assert_eq!(
            pages.iter().flat_map(rows).collect::<Vec<_>>(),
            rows(&whole),
            "{query} {order}"
        );
        let read_again = pages[1..].iter().filter(|page| page.progress.documents_processed > 0);
        assert_eq!(read_again.count(), 1, "{query} {order}");
    }
}

#[test]
fn a_later_walk_page_reads_every_visit_of_its_last_time_before_it_ends() {
    let archive = Archive::with_key(Some([132; 32]));
    // 520 pages share the newest time and 520 the oldest, more than one 512-visit chunk each,
    // so a later page's chunk ends inside them. A walk meets each group against the order its
    // pages are listed in: the newest group's visits run against its page IDs (the pages are
    // created first and visited in reverse), and an oldest-first walk meets the oldest group
    // by rising visit ID while the list goes by falling page ID.
    for id in 1..=520 {
        archive.url(id, CHROME, &format!("https://new.test/{id}"), "story new", &[]);
    }
    for id in (1..=520).rev() {
        archive.url(1000 + id, FIREFOX, &format!("https://new.test/{id}"), "story new", &[9000]);
    }
    for id in 1..=520 {
        archive.url(2000 + id, CHROME, &format!("https://old.test/{id}"), "story old", &[1]);
    }
    for id in 1..=40 {
        archive.url(3000 + id, CHROME, &format!("https://mid.test/{id}"), "story", &[id * 100]);
    }
    archive.index();
    for order in ["newest", "oldest"] {
        let whole = pages(&archive, request("st.ry", order, 10_000)).remove(0);
        assert_eq!(whole.count.value, 1080, "{order}");
        for size in [7, 50] {
            let pages = pages(&archive, request("st.ry", order, size));
            assert_eq!(
                pages.iter().flat_map(rows).collect::<Vec<_>>(),
                rows(&whole),
                "{order} {size}"
            );
        }
    }
}

#[test]
fn a_walk_ranking_by_visits_keeps_memory_only_for_the_pages_it_keeps() {
    // Four thousand matching pages and no fragment to read them by: the walk counts the visits
    // of every one, then keeps the keys of 200, with one row a page.
    let archive = Archive::with_key(Some([133; 32]));
    for id in 1..=4000 {
        archive.url(id, CHROME, &format!("https://w.test/{id}"), "story", &[id * 10]);
    }
    archive.index();
    let mut session = SearchSession::start(&archive.vault, request("st.ry", "visits", 1)).unwrap();
    let mut page = session.finish_page().unwrap();
    assert_eq!(page.count.value, 4000);
    let room = session.regex_room();
    assert!(room < 1000, "first page: memory for {room} pages");
    // Page on until the keys kept run out and the scan walks again.
    loop {
        session.next_page(&page.cursor.unwrap()).unwrap();
        page = session.finish_page().unwrap();
        if page.progress.documents_processed > 0 {
            break;
        }
    }
    let room = session.regex_room();
    assert!(room < 1000, "page {}: memory for {room} pages", page.page);
}
