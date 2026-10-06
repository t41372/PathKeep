//! Responsibilities: prove compiled statements are reused safely (src/statement_cache.rs).
//! Not responsible for: SQL validation or checkpoint scheduling.
//! Failure modes covered: every call recompiles; the cache grows without bound;
//! a half-read statement blocks the TRUNCATE checkpoint; a schema change on the
//! same connection leaves a stale program.
mod support;
use std::{
    thread,
    time::{Duration, Instant},
};
use vault_store::{Db, Error, Priority, Store};

fn compiled(store: &Store) -> u64 {
    store.metrics(Db::Archive).statements_compiled
}

#[test]
fn a_statement_repeated_in_and_across_batches_compiles_once() {
    let (_dir, store) = support::open();
    let before = compiled(&store);
    for batch in 0..2_i64 {
        store
            .write(Db::Archive, Priority::Batch, move |tx| {
                for row in 0..5_000_i64 {
                    tx.execute(
                        "INSERT INTO visits(id,url,title,visited_at) VALUES (?1,'https://example.test','page',?1)",
                        [batch * 5_000 + row],
                    )?;
                }
                Ok(())
            })
            .unwrap();
    }
    assert_eq!(compiled(&store) - before, 1, "10,000 executions of one statement");
    assert_eq!(support::count(&store), 10_000);
}

#[test]
fn the_cache_is_bounded_and_evicts_the_least_recently_used() {
    let (_dir, store) = support::open();
    let sql = |i: usize| format!("SELECT {i} AS value");
    store
        .write(Db::Archive, Priority::Batch, move |tx| {
            for i in 0..300 {
                tx.query_one(&sql(i), (), |row| Ok(row.get::<i64>(0)?))?;
            }
            Ok(())
        })
        .unwrap();
    let rerun = |i: usize| {
        let before = compiled(&store);
        store
            .write(Db::Archive, Priority::Batch, move |tx| {
                tx.query_one(&sql(i), (), |row| Ok(row.get::<i64>(0)?))
            })
            .unwrap();
        compiled(&store) - before
    };
    assert_eq!(rerun(299), 0, "a recent statement is still cached");
    assert_eq!(rerun(0), 1, "the oldest statement was evicted beyond the bound");
}

#[test]
fn a_statement_stopped_mid_read_does_not_block_the_truncate_checkpoint() {
    let (dir, store) = support::open();
    for id in 1..=50 {
        support::insert(&store, id).unwrap();
    }
    // Every reader stops after its first row, leaving its statement mid-scan.
    for _ in 0..8 {
        let stopped = store.read(Db::Archive, |conn| {
            conn.for_each("SELECT id FROM visits ORDER BY id", (), |_| {
                Err(Error::RestrictedSql("stop after one row"))
            })
        });
        assert!(stopped.is_err());
    }
    support::insert(&store, 51).unwrap();
    let wal = dir.path().join("archive/history-vault.db-wal");
    let deadline = Instant::now() + Duration::from_secs(10);
    while std::fs::metadata(&wal).unwrap().len() != 0 {
        assert!(Instant::now() < deadline, "a cached reader statement pinned the WAL");
        thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(support::count(&store), 51);
}

#[test]
fn a_cached_statement_sees_a_later_schema_change() {
    let (_dir, store) = support::open();
    support::insert(&store, 1).unwrap();
    let columns = |store: &Store| {
        store
            .write(Db::Archive, Priority::Batch, |tx| {
                tx.query_one("SELECT * FROM visits", (), |row| Ok(row.column_count()))
            })
            .unwrap()
    };
    assert_eq!(columns(&store), 5);
    store
        .write(Db::Archive, Priority::Batch, |tx| {
            tx.execute("ALTER TABLE visits ADD COLUMN note TEXT", ())?;
            Ok(())
        })
        .unwrap();
    assert_eq!(columns(&store), 6);
}
