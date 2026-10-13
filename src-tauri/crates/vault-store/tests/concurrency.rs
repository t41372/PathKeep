//! Responsibilities: verify real queue ordering, snapshot visibility and panic recovery.
//! Not responsible for: replacing the separate throughput benchmark.
//! Failure modes: queued batches overtake interactive jobs; readers see dirty data
//! or block behind a writer; panics poison owners.
mod support;
use std::{
    sync::{Arc, mpsc},
    thread,
    time::{Duration, Instant},
};
use vault_store::{Db, Error, Priority};
#[test]
fn interactive_during_batch_stream_waits_for_only_current_batch() {
    let (_dir, store) = support::open();
    let store = Arc::new(store);
    let (started_tx, started_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let worker = store.clone();
    let batch = thread::spawn(move || {
        worker
            .write(Db::Archive, Priority::Batch, move |tx| {
                tx.execute("INSERT INTO visits(id) VALUES (1)", ())?;
                started_tx.send(()).unwrap();
                release_rx.recv().unwrap();
                Ok(())
            })
            .unwrap();
        for id in 2..=100 {
            support::insert(&worker, id).unwrap();
        }
    });
    started_rx.recv_timeout(Duration::from_secs(10)).unwrap();
    let worker = store.clone();
    let arrival = Instant::now();
    let interactive = thread::spawn(move || {
        worker
            .write(Db::Archive, Priority::Interactive, move |tx| {
                let elapsed = arrival.elapsed();
                let count =
                    tx.query_one("SELECT count(*) FROM visits", (), |r| Ok(r.get::<i64>(0)?))?;
                Ok((count, elapsed))
            })
            .unwrap()
    });
    // Observe acceptance, not scheduling luck, before releasing the current batch.
    let deadline = Instant::now() + Duration::from_secs(10);
    while store.metrics(Db::Archive).interactive_pending == 0 {
        assert!(Instant::now() < deadline);
        thread::yield_now();
    }
    release_tx.send(()).unwrap();
    let (count, wait) = interactive.join().unwrap();
    assert_eq!(count, 1);
    println!("interactive wait_ms={:.3}", wait.as_secs_f64() * 1000.0);
    batch.join().unwrap();
    assert_eq!(support::count(&store), 100);
}
#[test]
fn reader_sees_only_commits_and_finishes_while_batch_is_held() {
    let (_dir, store) = support::open();
    let store = Arc::new(store);
    support::insert(&store, 1).unwrap();
    let (start_tx, start_rx) = mpsc::channel();
    let (end_tx, end_rx) = mpsc::channel();
    let worker = store.clone();
    let batch = thread::spawn(move || {
        worker
            .write(Db::Archive, Priority::Batch, move |tx| {
                tx.execute("INSERT INTO visits(id) VALUES (2)", ())?;
                start_tx.send(()).unwrap();
                end_rx.recv().unwrap();
                Ok(())
            })
            .unwrap()
    });
    start_rx.recv_timeout(Duration::from_secs(10)).unwrap();
    let worker = store.clone();
    let (read_tx, read_rx) = mpsc::channel();
    let reader = thread::spawn(move || {
        let start = Instant::now();
        read_tx.send((support::count(&worker), start.elapsed())).unwrap();
    });
    let result = read_rx.recv_timeout(Duration::from_secs(5));
    end_tx.send(()).unwrap();
    batch.join().unwrap();
    reader.join().unwrap();
    let (count, latency) = result.expect("reader blocked behind uncommitted writer");
    assert_eq!(count, 1);
    println!("reader_during_batch_ms={:.3}", latency.as_secs_f64() * 1000.0);
    assert_eq!(support::count(&store), 2);
}
#[test]
fn panics_and_errors_roll_back_and_owners_remain_usable() {
    let (_dir, store) = support::open();
    let result = store.write::<()>(Db::Archive, Priority::Interactive, |tx| {
        tx.execute("INSERT INTO visits(id) VALUES (1)", ())?;
        panic!("expected fixture panic")
    });
    assert!(matches!(result, Err(Error::JobPanicked { .. })));
    assert_eq!(support::count(&store), 0);
    assert!(matches!(
        store.read::<()>(Db::Archive, |_| panic!("read fixture")),
        Err(Error::JobPanicked { .. })
    ));
    let result = store.write::<()>(Db::Archive, Priority::Batch, |tx| {
        tx.execute("INSERT INTO visits(id) VALUES (2)", ())?;
        Err(Error::cancelled())
    });
    assert!(matches!(result, Err(Error::Cancelled { .. })));
    let result = store.write::<()>(Db::Archive, Priority::Interactive, |tx| {
        tx.query_one("INSERT INTO visits(id) VALUES (4) RETURNING id", (), |_| {
            panic!("row decoder fixture")
        })
    });
    assert!(matches!(result, Err(Error::JobPanicked { .. })));
    assert_eq!(support::count(&store), 0);
    support::insert(&store, 3).unwrap();
    assert_eq!(support::count(&store), 1);
}

#[test]
fn a_held_archive_batch_does_not_serialize_other_database_writers() {
    let (_dir, store) = support::open();
    let store = Arc::new(store);
    let (start_tx, start_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let worker = store.clone();
    let held = thread::spawn(move || {
        worker.write(Db::Archive, Priority::Batch, move |tx| {
            tx.execute("INSERT INTO visits(id) VALUES (1)", ())?;
            start_tx.send(()).unwrap();
            release_rx.recv().unwrap();
            Ok(())
        })
    });
    start_rx.recv_timeout(Duration::from_secs(10)).unwrap();
    let worker = store.clone();
    let (done_tx, done_rx) = mpsc::channel();
    let other = thread::spawn(move || {
        let result: vault_store::Result<()> = (|| {
            for db in [Db::Agent, Db::Scratch] {
                worker.write(db, Priority::Interactive, |tx| {
                    tx.execute("CREATE TABLE conversations(id INTEGER)", ())?;
                    Ok(())
                })?;
            }
            assert_eq!(support::count(&worker), 0);
            Ok(())
        })();
        done_tx.send(result).unwrap();
    });
    let result = done_rx.recv_timeout(Duration::from_secs(5));
    release_tx.send(()).unwrap();
    held.join().unwrap().unwrap();
    other.join().unwrap();
    result.expect("unrelated writer blocked").unwrap();
}

#[test]
fn committed_write_reply_does_not_wait_for_an_existing_reader_snapshot() {
    let (_dir, store) = support::open();
    let store = Arc::new(store);
    support::insert(&store, 1).unwrap();
    let (started_tx, started_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let worker = store.clone();
    let reader = thread::spawn(move || {
        worker
            .read(Db::Archive, move |c| {
                let first =
                    c.query_one("SELECT count(*) FROM visits", (), |r| Ok(r.get::<i64>(0)?))?;
                started_tx.send(()).unwrap();
                release_rx.recv().unwrap();
                let second =
                    c.query_one("SELECT count(*) FROM visits", (), |r| Ok(r.get::<i64>(0)?))?;
                Ok((first, second))
            })
            .unwrap()
    });
    started_rx.recv_timeout(Duration::from_secs(10)).unwrap();
    let worker = store.clone();
    let (done_tx, done_rx) = mpsc::channel();
    let writer = thread::spawn(move || done_tx.send(support::insert(&worker, 2)).unwrap());
    let result = done_rx.recv_timeout(Duration::from_secs(5));
    release_tx.send(()).unwrap();
    assert_eq!(reader.join().unwrap(), (1, 1));
    writer.join().unwrap();
    result.expect("commit acknowledgement waited for maintenance").unwrap();
    assert_eq!(support::count(&store), 2);
}
