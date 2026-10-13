//! Responsibilities: preserve the regex scan's failure contracts on real Vault owners.
//! Not responsible for: the removed SQLCipher cursor encoding or frontend rendering.
//! Failure modes: chunk edges repeat or skip pages; lower bounds look exact; reading only the
//! pages with the pattern's fragment finds less than reading every visit.
use super::{
    SearchRequest, SearchSession,
    fixture::{Archive, CHROME, FIREFOX},
};
use std::time::Duration;

fn archive() -> Archive {
    let archive = Archive::with_key(Some([87; 32]));
    for id in 1..=520 {
        archive.url(id, CHROME, &format!("https://filler.test/{id}"), "filler", &[id * 1000]);
    }
    archive
        .url(521, FIREFOX, "https://news.test/story?id=1", "Story one", &[500, 12500, 25500])
        .url(522, CHROME, "https://news.test/story?id=2", "Story two", &[7500])
        .url(523, CHROME, "https://news.test/story?id=1", "Story one", &[18500, 300])
        .url(524, CHROME, "https://news.test/old", "An old STORY", &[100])
        .index();
    archive
}
fn request() -> SearchRequest {
    SearchRequest { query: "story".into(), regex: true, ..Default::default() }
}
#[test]
fn chunk_edges_neither_repeat_nor_skip_a_page() {
    let archive = archive();
    let whole = SearchSession::start(&archive.vault, request()).unwrap().finish_page().unwrap();
    for size in [1, 2, 5, 100] {
        let mut session =
            SearchSession::start(&archive.vault, SearchRequest { page_size: size, ..request() })
                .unwrap();
        let mut ids = Vec::new();
        loop {
            let page = session.finish_page().unwrap();
            ids.extend(page.items.iter().map(|h| h.page_id));
            let Some(cursor) = page.cursor else { break };
            session.next_page(&cursor).unwrap();
        }
        assert_eq!(ids, whole.items.iter().map(|h| h.page_id).collect::<Vec<_>>());
    }
}
#[test]
fn a_count_is_exact_only_once_the_scan_covered_everything() {
    let archive = archive();
    let mut session = SearchSession::start(&archive.vault, request()).unwrap();
    let first = session.step(Duration::ZERO).unwrap();
    assert!(!first.count.exact);
    let last = session.finish_page().unwrap();
    assert!(last.count.exact);
    assert_eq!(last.count.value, 3);
    assert_eq!(last.visits.value, 7);
}

#[test]
fn reading_only_the_pages_with_the_fragment_counts_what_reading_every_visit_counts() {
    let archive = archive();
    // "story" has a literal fragment the index lists pages for; "st.ry" has none, so
    // it reads every visit. Both match the same rows here.
    let count = |query: &str| {
        let step = SearchSession::start(
            &archive.vault,
            SearchRequest { query: query.into(), page_size: 2, ..request() },
        )
        .unwrap()
        .finish_page()
        .unwrap();
        let times = step.items.iter().map(|h| h.visit.visit_time).collect::<Vec<_>>();
        (step.count.value, step.visits.value, times)
    };
    assert_eq!(count("story"), (3, 7, vec![25500, 7500]));
    assert_eq!(count("st.ry"), count("story"));
}
