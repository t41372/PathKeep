//! Responsibilities: preserve the regex scan's failure contracts on real Vault owners.
//! Not responsible for: the removed SQLCipher cursor encoding or frontend rendering.
//! Failure modes: empty chunks truncate coverage; ties duplicate pages; lower bounds look
//! exact; cancellation retains work; scope and dialect drift; scans sort all visits.
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
fn a_match_older_than_the_first_chunk_is_found_by_continuing() {
    let archive = archive();
    let mut session = SearchSession::start(
        &archive.vault,
        SearchRequest { query: "old STORY".into(), ..request() },
    )
    .unwrap();
    let first = session.step(Duration::ZERO).unwrap();
    assert!(first.items.is_empty());
    assert!(first.provisional);
    assert!(!first.count.exact);
    let final_step = session.finish_page().unwrap();
    assert_eq!(final_step.items.len(), 1);
    assert_eq!(final_step.items[0].visit.url, "https://news.test/old");
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
fn an_abandoned_scan_leaves_nothing_behind() {
    let archive = archive();
    let mut session = SearchSession::start(&archive.vault, request()).unwrap();
    session.step(Duration::ZERO).unwrap();
    session.cancel();
    assert!(session.step(Duration::ZERO).is_err());
    assert_eq!(
        SearchSession::start(&archive.vault, request()).unwrap().finish_page().unwrap().count.value,
        3
    );
}
#[test]
fn filters_bound_the_scan_before_the_regex() {
    let archive = archive();
    let ranged = SearchSession::start(
        &archive.vault,
        SearchRequest { start_time_ms: Some(200), end_time_ms: Some(10000), ..request() },
    )
    .unwrap()
    .finish_page()
    .unwrap();
    assert_eq!(ranged.items.iter().map(|h| h.visit.visit_time).collect::<Vec<_>>(), [7500, 500]);
    assert_eq!(ranged.visits.value, 3);
    let firefox = SearchSession::start(
        &archive.vault,
        SearchRequest { browser_kind: Some("firefox".into()), ..request() },
    )
    .unwrap()
    .finish_page()
    .unwrap();
    assert_eq!(firefox.count.value, 1);
    assert_eq!(firefox.visits.value, 3);
    assert_eq!(firefox.items[0].visit.profile_id, "firefox:main");
}
#[test]
fn the_dialect_is_the_rust_regex_crate() {
    let archive = archive();
    for pattern in ["story(?=one)", r"(s)\1"] {
        assert!(
            SearchSession::start(
                &archive.vault,
                SearchRequest { query: pattern.into(), ..request() }
            )
            .is_err()
        );
    }
    let result = SearchSession::start(
        &archive.vault,
        SearchRequest { query: r"story\?id=\d$".into(), ..request() },
    )
    .unwrap()
    .finish_page()
    .unwrap();
    assert_eq!(result.count.value, 2);
    assert_eq!(result.visits.value, 6);
}

#[test]
fn regex_keeps_operator_words_and_slashes_literal_and_requires_its_mode_flag() {
    use super::{SearchRequest, SearchSession};
    let archive = Archive::with_key(Some([115; 32]));
    archive
        .url(1, CHROME, "https://regex.test/1", "tag:chosen", &[1])
        .url(2, CHROME, "https://regex.test/chosen", "ordinary", &[2])
        .url(3, CHROME, "https://regex.test/prefix/chosen/suffix", "ordinary", &[3])
        .url(4, CHROME, "https://regex.test/4", "ordinary", &[4])
        .index();
    crate::set_notes(
        &archive.vault,
        crate::SetNotesRequest {
            url: "https://regex.test/4".into(),
            notes: "/chosen/".into(),
            source_profile: None,
        },
    )
    .unwrap();
    archive.index();
    for (query, regex, expected) in [
        ("tag:chosen", true, vec![1]),
        ("/chosen/", true, vec![3]),
        ("/chosen/", false, vec![1, 2, 3, 4]),
    ] {
        let page = SearchSession::start(
            &archive.vault,
            SearchRequest { query: query.into(), regex, ..Default::default() },
        )
        .unwrap()
        .finish_page()
        .unwrap();
        let mut ids = page.items.into_iter().map(|hit| hit.page_id).collect::<Vec<_>>();
        ids.sort_unstable();
        assert_eq!(ids, expected, "{query} regex={regex}");
    }
}
