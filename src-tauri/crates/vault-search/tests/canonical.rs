//! Responsibilities: verify owner-supplied recency and first-build publication.
//! Not responsible for: canonical SQL or worker scheduling.
//! Failure modes: partial first builds look complete; cancelled first builds cannot resume;
//! ordinary rebuilds publish partial results; caller-supplied order or scores lose pages.

mod support;
use std::sync::Arc;
use support::*;
use vault_search::*;

#[test]
fn initial_build_publishes_progress_without_acknowledging_an_incomplete_snapshot() {
    let root = tempfile::tempdir().unwrap();
    let index = SearchIndex::create(root.path(), Some(&key()), config()).unwrap();
    index
        .build_initial(
            (1..=2050).map(|id| Ok(page(id, "same"))),
            42,
            |p| {
                if p.documents >= 1024 && !p.complete {
                    assert!(index.initial_build_incomplete().unwrap());
                    assert_eq!(index.last_applied_seq().unwrap(), 0);
                    assert!(session(&index, term("same")).count().unwrap() >= 1024);
                }
            },
            || false,
        )
        .unwrap();
    assert!(!index.initial_build_incomplete().unwrap());
    assert_eq!(index.last_applied_seq().unwrap(), 42);
    assert_eq!(session(&index, term("same")).count().unwrap(), 2050);
}

#[test]
fn interrupted_initial_build_is_detected_after_reopen() {
    let root = tempfile::tempdir().unwrap();
    let index = SearchIndex::create(root.path(), Some(&key()), config()).unwrap();
    let cancel = std::sync::atomic::AtomicBool::new(false);
    assert!(matches!(
        index.build_initial(
            (1..=2050).map(|id| Ok(page(id, "same"))),
            42,
            |p| {
                if p.documents == 1024 {
                    cancel.store(true, std::sync::atomic::Ordering::Release);
                }
            },
            || cancel.load(std::sync::atomic::Ordering::Acquire)
        ),
        Err(Error::Cancelled)
    ));
    drop(index);
    let index = SearchIndex::open(root.path(), Some(&key()), config()).unwrap();
    assert!(index.initial_build_incomplete().unwrap());
    assert_eq!(index.last_applied_seq().unwrap(), 0);
    assert_eq!(session(&index, term("same")).count().unwrap(), 1024);
    index.build_initial((1..=2050).map(|id| Ok(page(id, "same"))), 42, |_| {}, || false).unwrap();
    assert_eq!(session(&index, term("same")).count().unwrap(), 2050);
}

#[test]
fn explicit_chronology_and_count_visitors_preserve_complete_paging() {
    let root = tempfile::tempdir().unwrap();
    let index = create(
        root.path(),
        (1..=50).map(|id| page(id, if id % 2 == 0 { "same same" } else { "same" })).collect(),
    );
    let session = index
        .search(term("same"), Filters { order: SortOrder::Oldest, ..Default::default() })
        .unwrap();
    let mut counter = session.start_count().unwrap();
    let mut ids = Vec::new();
    loop {
        let step = counter.step_with(std::time::Duration::ZERO, |id| ids.push(id)).unwrap();
        if !step.provisional {
            break;
        }
    }
    ids.sort_unstable();
    assert_eq!(ids, (1..=50).collect::<Vec<_>>());
    let ranked = session.page(50, None).unwrap();
    let mut expected = (1..=50).collect::<Vec<_>>();
    expected.sort_by_key(|id| (id % 7, std::cmp::Reverse(*id)));
    assert_eq!(ranked.hits.iter().map(|h| h.page_id).collect::<Vec<_>>(), expected);
}

#[test]
fn match_batches_keep_all_scores_and_stop_at_the_requested_batch_size() {
    let root = tempfile::tempdir().unwrap();
    let index = create(root.path(), (1..=2050).map(|id| page(id, "same")).collect());
    let session = session(&index, term("same"));
    let expected = session.page(3000, None).unwrap().hits;
    let mut scan = session.start_matches(true).unwrap();
    let mut hits = Vec::new();
    loop {
        let step = scan.step(std::time::Duration::MAX, 17).unwrap();
        assert!(step.hits.len() <= 17);
        hits.extend(step.hits);
        if !step.provisional {
            break;
        }
    }
    hits.sort_unstable_by(|a, b| b.cmp(a));
    assert_eq!(hits, expected);
}

#[test]
fn a_changed_owner_normalization_revision_requires_a_rebuild() {
    let root = tempfile::tempdir().unwrap();
    drop(create(root.path(), vec![page(1, "same")]));
    let config = Config { projection_version: 2, ..config() };
    assert!(matches!(
        SearchIndex::open(root.path(), Some(&key()), config),
        Err(Error::RebuildNeeded { .. })
    ));
}

#[test]
fn complete_caller_scores_rank_without_an_oversampled_candidate_pool() {
    let root = tempfile::tempdir().unwrap();
    let index = create(root.path(), (1..=2001).map(|id| page(id, "same")).collect());
    let original = session(&index, term("same"));
    let ranked = original
        .requery(
            Ast::And(vec![]),
            Filters {
                score: Some(Arc::new(|id, _| if id == 1 { 10.0 } else { 1.0 })),
                ..Default::default()
            },
        )
        .unwrap();
    index.apply(Batch { last_seq: 2, upserts: vec![page(2002, "same")], deletes: vec![] }).unwrap();
    assert_eq!(ranked.page(10, None).unwrap().hits[0].page_id, 1);
    assert_eq!(ranked.count().unwrap(), 2001);
    assert_eq!(ranked.last_applied_seq(), 1);
}
