//! Responsibilities: prove the worker exposes the same real streaming search contract.
//! Not responsible for: Tauri serialization or frontend rendering.
//! Failure modes: workers open a second owner, stop after an empty provisional step,
//! lose continuation, or keep sessions alive after close.
use std::{sync::mpsc, time::Duration};
use vault_core::{
    Vault,
    search::{SearchEvent, SearchRequest},
};
#[test]
fn worker_streams_count_cursor_and_indexer_state_and_cancels() {
    for key in [None, Some([93; 32])] {
        let vault = Vault::open_temp_with_key(key).unwrap();
        let (tx, rx) = mpsc::channel();
        let start = crate::start_search(&vault, SearchRequest::default(), move |event| {
            tx.send(event).is_ok()
        })
        .unwrap();
        assert!(matches!(
            rx.recv_timeout(Duration::from_secs(10)).unwrap(),
            SearchEvent::Indexer(_)
        ));
        loop {
            match rx.recv_timeout(Duration::from_secs(10)).unwrap() {
                SearchEvent::Step(step) if !step.provisional && step.count.exact => {
                    assert!(step.items.is_empty());
                    assert!(step.cursor.is_none());
                    break;
                }
                SearchEvent::Error(error) => panic!("{error}"),
                _ => {}
            }
        }
        crate::cancel_search(&vault, &start.session_id).unwrap();
        assert!(crate::next_search_page(&vault, &start.session_id, "invalid".into()).is_err());
    }
}

#[test]
fn worker_continuation_reaches_every_page_and_close_revokes_the_session() {
    for key in [None, Some([94; 32])] {
        let vault = Vault::open_temp_with_key(key).unwrap();
        vault_core::test_support::search::seed_pages(&vault, 1107);
        let (tx, rx) = mpsc::channel();
        let started = crate::start_search(
            &vault,
            SearchRequest { query: "common".into(), page_size: 101, ..Default::default() },
            move |event| tx.send(event).is_ok(),
        )
        .unwrap();
        let mut ids = Vec::new();
        let mut page_number = 1;
        loop {
            match rx.recv_timeout(Duration::from_secs(30)).unwrap() {
                SearchEvent::Step(step) if !step.provisional && step.count.exact => {
                    assert_eq!(step.count.value, 1107);
                    assert_eq!(step.page, page_number);
                    page_number += 1;
                    ids.extend(step.items.into_iter().map(|hit| hit.page_id));
                    match step.cursor {
                        Some(cursor) => {
                            crate::next_search_page(&vault, &started.session_id, cursor).unwrap()
                        }
                        None => break,
                    }
                }
                SearchEvent::Error(error) => panic!("{error}"),
                _ => {}
            }
        }
        ids.sort_unstable();
        assert_eq!(ids, (1..=1107).collect::<Vec<_>>());
        vault.close().unwrap();
        assert!(crate::next_search_page(&vault, &started.session_id, "closed".into()).is_err());
    }
}
