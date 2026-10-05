//! Responsibilities: verify cancellation and page-scoped read transactions.
//! Not responsible for: interrupting an executing SQL statement.
//! Failure modes: cancellation is missed; page walk buffers whole results or pins
//! one snapshot; zero batch size cannot progress. NonZeroUsize excludes the latter.
mod support;
use std::{
    num::NonZeroUsize,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};
use vault_store::{CancelToken, Db, Error, Priority};
#[test]
fn cancellation_stops_walk_between_bounded_pages_and_releases_reader() {
    let (_dir, store) = support::open();
    store.write(Db::Archive,Priority::Batch,|tx|{tx.execute("WITH RECURSIVE seq(x) AS (VALUES(1) UNION ALL SELECT x+1 FROM seq WHERE x<10000) INSERT INTO visits(id) SELECT x FROM seq",())?;Ok(())}).unwrap();
    let token = CancelToken::new();
    let seen = Arc::new(AtomicUsize::new(0));
    let count = seen.clone();
    let cancel = token.clone();
    let result = store.walk(
        Db::Archive,
        &token,
        NonZeroUsize::new(128).unwrap(),
        0_i64,
        |c, after, limit| {
            c.query(
                "SELECT id FROM visits WHERE id>?1 ORDER BY id LIMIT ?2",
                (after, limit as i64),
                |r| Ok(r.get::<i64>(0)?),
            )
        },
        |page| {
            assert!(page.len() <= 128);
            count.fetch_add(page.len(), Ordering::SeqCst);
            cancel.cancel();
            Ok(*page.last().unwrap())
        },
    );
    assert!(matches!(result, Err(Error::Cancelled { .. })));
    assert_eq!(seen.load(Ordering::SeqCst), 128);
    support::insert(&store, 10001).unwrap();
    assert_eq!(support::count(&store), 10001);
}
#[test]
fn complete_walk_uses_new_snapshot_each_page() {
    let (_dir, store) = support::open();
    for id in 1..=3 {
        support::insert(&store, id).unwrap();
    }
    let mut seen = Vec::new();
    store
        .walk(
            Db::Archive,
            &CancelToken::new(),
            NonZeroUsize::new(2).unwrap(),
            0_i64,
            |c, after, limit| {
                c.query(
                    "SELECT id FROM visits WHERE id>?1 ORDER BY id LIMIT ?2",
                    (after, limit as i64),
                    |r| Ok(r.get::<i64>(0)?),
                )
            },
            |page| {
                if seen.is_empty() {
                    support::insert(&store, 4)?;
                }
                seen.extend_from_slice(page);
                Ok(*page.last().unwrap())
            },
        )
        .unwrap();
    assert_eq!(seen, vec![1, 2, 3, 4]);
}
