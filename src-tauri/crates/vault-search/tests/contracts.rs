//! Responsibilities: prove ranking, complete pagination, filtering, counting and progress.
//! Not responsible for: canonical visit SQL or the application's presentation.
//! Failure modes: weights drift; score ties lose older pages; deep paging repeats/skips;
//! filters change scores or disagree with counts; tombstones leak; new commits/merges
//! change an existing session; progressive traversal loses its place within a segment.
mod support;
use std::sync::Arc;
use std::time::Duration;
use support::*;
use tantivy::query::{BooleanQuery, BoostQuery, Query, TermQuery};
use tantivy::schema::IndexRecordOption;
use tantivy::{Term, collector::TopDocs};
use vault_search::*;

fn corpus() -> Vec<PageDocument> {
    (1..=1703)
        .map(|i| {
            let mut p = page(
                i,
                if i % 11 == 0 {
                    "different"
                } else if i % 5 == 0 {
                    "needle needle archive"
                } else {
                    "needle archive"
                },
            );
            if i % 13 == 0 {
                p.text[TextField::Notes as usize] = "needle".into();
            }
            p
        })
        .collect()
}

fn brute(
    path: &std::path::Path,
    docs: &[PageDocument],
    accept: impl Fn(&PageDocument) -> bool,
) -> Vec<Hit> {
    let index = raw(path);
    let reader = index.reader().unwrap();
    let searcher = reader.searcher();
    let names = [
        "url",
        "title",
        "search_terms",
        "normalized_url",
        "normalized_title",
        "normalized_search_terms",
        "cjk_grams",
        "enrichment_text",
        "notes_text",
        "tags_text",
    ];
    let weights = [6., 12., 4., 5., 10., 4., 2., 7., 9., 6.];
    let queries: Vec<Box<dyn Query>> = names
        .into_iter()
        .zip(weights)
        .map(|(name, boost)| {
            Box::new(BoostQuery::new(
                Box::new(TermQuery::new(
                    Term::from_field_text(index.schema().get_field(name).unwrap(), "=needle"),
                    IndexRecordOption::WithFreqsAndPositions,
                )),
                boost,
            )) as Box<dyn Query>
        })
        .collect();
    let query = BooleanQuery::union(queries);
    let mut out: Vec<_> = searcher
        .search(&query, &TopDocs::with_limit(docs.len()).order_by_score())
        .unwrap()
        .into_iter()
        .filter_map(|(score, address)| {
            let seg = searcher.segment_reader(address.segment_ord);
            let id = seg.fast_fields().u64("page_id").unwrap().first(address.doc_id).unwrap();
            let doc = docs.iter().find(|p| p.page_id == id).unwrap();
            accept(doc).then_some(Hit { page_id: id, newest_visit_ms: doc.newest_visit_ms, score })
        })
        .collect();
    out.sort_by(|a, b| {
        b.score
            .total_cmp(&a.score)
            .then(b.newest_visit_ms.cmp(&a.newest_visit_ms))
            .then(b.page_id.cmp(&a.page_id))
    });
    out
}

#[test]
fn ranking_and_every_page_equal_a_brute_force_sort() {
    let tmp = tempfile::tempdir().unwrap();
    let docs = corpus();
    let index = create(tmp.path(), docs.clone());
    let expected = brute(tmp.path(), &docs, |_| true);
    let search = session(&index, term("needle"));
    let mut cursor = None;
    let mut all = Vec::new();
    loop {
        let result = search.page(37, cursor.as_ref()).unwrap();
        if result.hits.is_empty() {
            break;
        }
        all.extend(result.hits);
        cursor = result.next;
    }
    assert_eq!(all, expected);
    assert_eq!(search.count().unwrap(), expected.len() as u64);
    assert!(search.page(0, None).is_err());
    let foreign = session(&index, term("archive"));
    assert!(foreign.page(10, cursor.as_ref()).is_err());
}

#[test]
fn all_ten_fields_keep_the_projection_weights() {
    let tmp = tempfile::tempdir().unwrap();
    let docs = (0..10)
        .map(|i| {
            let mut p = PageDocument::new(i);
            p.text[i as usize] = "needle".into();
            p
        })
        .collect();
    let index = create(tmp.path(), docs);
    let hits = session(&index, term("needle")).page(20, None).unwrap().hits;
    assert_eq!(
        hits.iter().map(|h| h.page_id).collect::<Vec<_>>(),
        vec![1, 4, 8, 7, 9, 0, 3, 5, 2, 6]
    );
}

#[test]
fn pushed_and_collector_filters_and_same_visit_eligibility_match_brute_force() {
    let tmp = tempfile::tempdir().unwrap();
    let docs = corpus();
    let index = create(tmp.path(), docs.clone());
    for (domain, profile) in [
        (None, None),
        (Some("other.test"), None),
        (Some("docs.test"), Some(9)),
        (None, Some(1)),
        (None, Some(999)),
        (Some("absent"), None),
    ] {
        let filters = Filters {
            domain: domain.map(str::to_owned),
            profile_id: profile,
            eligibility: Some(Arc::new(|id| id % 50 == 0)),
        };
        let expected = brute(tmp.path(), &docs, |p| {
            domain.is_none_or(|d| p.domain == d)
                && profile.is_none_or(|id| p.profile_ids.contains(&id))
                && p.page_id % 50 == 0
        });
        let search = index.search(term("needle"), filters).unwrap();
        assert_eq!(search.page(100, None).unwrap().hits, expected);
        assert_eq!(search.count().unwrap(), expected.len() as u64);
    }
    assert!(
        index
            .search(
                term("needle"),
                Filters { domain: Some("other.test".into()), ..Filters::default() }
            )
            .unwrap()
            .plan()
            .domain_pushed
    );
    assert!(
        !index
            .search(
                term("needle"),
                Filters { domain: Some("docs.test".into()), ..Filters::default() }
            )
            .unwrap()
            .plan()
            .domain_pushed
    );
}

#[test]
fn progressive_steps_equal_one_shot_across_segments_and_tombstones() {
    let tmp = tempfile::tempdir().unwrap();
    let index = create(tmp.path(), corpus());
    index
        .apply(Batch {
            last_seq: 2,
            upserts: (2000..2300).map(|i| page(i, "needle")).collect(),
            deletes: (1..40).collect(),
        })
        .unwrap();
    let search = session(&index, term("needle"));
    let expected = search.page(73, None).unwrap();
    let count = search.count().unwrap();
    let mut ranking = search.start_page(73, None).unwrap();
    let mut counting = search.start_count().unwrap();
    let mut previous = 0;
    let mut steps = 0;
    loop {
        let step = ranking.step(Duration::ZERO).unwrap();
        steps += 1;
        assert!(step.progress.documents_processed >= previous);
        previous = step.progress.documents_processed;
        if !step.provisional {
            assert_eq!(step.hits, expected.hits);
            break;
        }
    }
    assert!(steps > 2);
    let mut previous = 0;
    loop {
        let step = counting.step(Duration::ZERO).unwrap();
        assert!(step.count >= previous);
        previous = step.count;
        if !step.provisional {
            assert_eq!(step.count, count);
            break;
        }
    }
    assert_eq!(ranking.step(Duration::ZERO).unwrap().hits, expected.hits);
    let next = search.page(73, expected.next.as_ref()).unwrap();
    let mut progressive_next = search.start_page(73, expected.next.as_ref()).unwrap();
    assert_eq!(progressive_next.step(Duration::MAX).unwrap().hits, next.hits);
}

#[test]
fn pagination_and_count_keep_their_snapshot_during_writes_and_rebuild() {
    let tmp = tempfile::tempdir().unwrap();
    let index = create(tmp.path(), corpus());
    let search = session(&index, term("needle"));
    let all = search.page(5000, None).unwrap().hits;
    let first = search.page(30, None).unwrap();
    index
        .apply(Batch {
            last_seq: 2,
            upserts: vec![page(5000, "needle")],
            deletes: (1..1704).collect(),
        })
        .unwrap();
    index.rebuild((6000..6009).map(|i| Ok(page(i, "replacement"))), 3, |_| {}, || false).unwrap();
    let mut got = first.hits;
    let mut cursor = first.next;
    loop {
        let page = search.page(30, cursor.as_ref()).unwrap();
        if page.hits.is_empty() {
            break;
        }
        got.extend(page.hits);
        cursor = page.next;
    }
    assert_eq!(got, all);
    assert_eq!(search.count().unwrap(), all.len() as u64);
    assert_eq!(session(&index, term("replacement")).count().unwrap(), 9);
}

#[test]
fn progressive_handles_can_move_between_blocking_workers() {
    let tmp = tempfile::tempdir().unwrap();
    let index = create(tmp.path(), corpus());
    let session = session(&index, term("needle"));
    let mut page = session.start_page(20, None).unwrap();
    page.step(Duration::ZERO).unwrap();
    let result = std::thread::spawn(move || page.step(Duration::MAX).unwrap()).join().unwrap();
    assert_eq!(result.hits, session.page(20, None).unwrap().hits);
    let mut count = session.start_count().unwrap();
    count.step(Duration::ZERO).unwrap();
    let result = std::thread::spawn(move || count.step(Duration::MAX).unwrap()).join().unwrap();
    assert_eq!(result.count, session.count().unwrap());
}

#[test]
fn exclusions_do_not_contribute_to_bm25_scores() {
    let tmp = tempfile::tempdir().unwrap();
    let index = create(tmp.path(), corpus());
    let original = session(&index, term("needle")).page(100, None).unwrap().hits;
    let with_exclusion =
        session(&index, Ast::And(vec![term("needle"), Ast::Not(Box::new(term("absent")))]));
    assert_eq!(with_exclusion.page(100, None).unwrap().hits, original);
}

#[test]
fn pagination_crosses_the_old_25000_page_window_without_losing_ties() {
    let tmp = tempfile::tempdir().unwrap();
    let index = create(tmp.path(), (0..30_017).map(|id| page(id, "needle")).collect());
    let search = session(&index, term("needle"));
    let mut expected: Vec<_> = (0..30_017).collect();
    expected.sort_unstable_by(|a, b| (b % 7).cmp(&(a % 7)).then(b.cmp(a)));
    let mut actual = vec![];
    let mut cursor = None;
    loop {
        let step = search.page(4096, cursor.as_ref()).unwrap();
        if step.hits.is_empty() {
            break;
        }
        actual.extend(step.hits.into_iter().map(|hit| hit.page_id));
        cursor = step.next;
    }
    assert_eq!(actual, expected);
    assert_eq!(search.count().unwrap(), 30_017);
}

#[test]
fn cold_bm25_metadata_yields_before_ranking_and_handles_do_no_io() {
    let tmp = tempfile::tempdir().unwrap();
    drop(create(tmp.path(), corpus()));
    let index = SearchIndex::open(tmp.path(), Some(&key()), config()).unwrap();
    let search = session(&index, term("needle"));
    let before = index.cache_stats().unwrap().misses;
    let mut handle = search.start_page(37, None).unwrap();
    assert_eq!(
        index.cache_stats().unwrap().misses,
        before,
        "creating a handle must not read statistics"
    );
    let first = handle.step(Duration::ZERO).unwrap();
    assert!(first.provisional);
    assert_eq!(first.progress.statistics_fields_processed, 1);
    assert!(first.progress.statistics_fields_total >= 10);
    assert_eq!(first.progress.documents_processed, 0);
    let mut previous = 1;
    loop {
        let step = handle.step(Duration::ZERO).unwrap();
        assert!(step.progress.statistics_fields_processed >= previous);
        previous = step.progress.statistics_fields_processed;
        if !step.provisional {
            assert_eq!(
                step.progress.statistics_fields_processed,
                step.progress.statistics_fields_total
            );
            assert_eq!(step.hits, brute(tmp.path(), &corpus(), |_| true)[..37]);
            break;
        }
    }
}
