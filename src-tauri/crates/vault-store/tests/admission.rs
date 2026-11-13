//! Responsibilities: a read that runs long delays the idle checkpoint, and nothing else.
//! Not responsible for: how long a read may run, or WAL size while one is still running.
//! Failure modes (2026-11-13: one statement ran for hours on an Intelligence reader and every
//! other read of Archive and Intelligence waited for it, because a writer that had just
//! committed kept new readers out until the readers already running had left):
//! - after a write, a new read waits for an unrelated read that is still running;
//! - the same across databases: an Intelligence read of `archive.*` holds up Archive reads;
//! - the writer stays parked behind that read, so later writes are never served;
//! - the checkpoint put off for a long read is forgotten once that read ends;
//! - the writer keeps readers out for the same long read again: at every later write, or when
//!   it hears of some other reader that left before.
//!
//! None of these measures how long anything took. `PROMPT` and the deadlines only keep a
//! broken build from hanging the suite; what a writer did is read from `truncates_put_off`.
mod support;
use std::{
    sync::{Arc, mpsc},
    thread,
    time::{Duration, Instant},
};
use vault_store::{Db, Priority, Store};

/// Long enough that a wait for the held reader cannot be mistaken for a slow machine.
const PROMPT: Duration = Duration::from_secs(5);

/// A read of `sql` on `db` that stays open until the returned sender is used or dropped.
fn hold_reader(
    store: &Arc<Store>,
    db: Db,
    sql: &'static str,
) -> (mpsc::Sender<()>, thread::JoinHandle<()>) {
    let (started_tx, started_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel::<()>();
    let worker = store.clone();
    let reader = thread::spawn(move || {
        worker
            .read(db, move |c| {
                c.query_one(sql, (), |r| Ok(r.get::<i64>(0)?))?;
                started_tx.send(()).unwrap();
                let _ = release_rx.recv();
                Ok(())
            })
            .unwrap()
    });
    started_rx.recv_timeout(Duration::from_secs(10)).unwrap();
    (release_tx, reader)
}

/// Runs `work` on its own thread and returns its result if it finished within [`PROMPT`].
fn promptly<T: Send + 'static>(work: impl FnOnce() -> T + Send + 'static) -> Option<T> {
    let (done_tx, done_rx) = mpsc::channel();
    thread::spawn(move || {
        let _ = done_tx.send(work());
    });
    done_rx.recv_timeout(PROMPT).ok()
}

fn open() -> (tempfile::TempDir, Arc<Store>) {
    let (dir, store) = support::open();
    let store = Arc::new(store);
    support::insert(&store, 1).unwrap();
    (dir, store)
}

/// Returns once the Archive writer has put a truncate off `times` times.
fn wait_until_put_off(store: &Store, times: u64) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while store.metrics(Db::Archive).truncates_put_off < times {
        assert!(Instant::now() < deadline, "the writer never gave up on the reader");
        thread::sleep(Duration::from_millis(1));
    }
}

#[test]
fn a_read_after_a_write_does_not_wait_for_a_read_that_is_still_running() {
    let (_dir, store) = open();
    let (release, held) = hold_reader(&store, Db::Archive, "SELECT count(*) FROM visits");
    support::insert(&store, 2).unwrap();
    let worker = store.clone();
    let seen = promptly(move || support::count(&worker));
    release.send(()).unwrap();
    held.join().unwrap();
    assert_eq!(seen, Some(2), "the read waited for the reader that was still running");
}

#[test]
fn an_archive_read_does_not_wait_for_an_intelligence_read_of_the_archive() {
    let (_dir, store) = open();
    let (release, held) =
        hold_reader(&store, Db::Intelligence, "SELECT count(*) FROM archive.visits");
    support::insert(&store, 2).unwrap();
    let worker = store.clone();
    let seen = promptly(move || support::count(&worker));
    release.send(()).unwrap();
    held.join().unwrap();
    assert_eq!(seen, Some(2), "the Archive read waited for the Intelligence reader");
}

#[test]
fn writes_are_served_while_a_read_is_still_running() {
    let (_dir, store) = open();
    let (release, held) = hold_reader(&store, Db::Archive, "SELECT count(*) FROM visits");
    support::insert(&store, 2).unwrap();
    let worker = store.clone();
    let second = promptly(move || support::insert(&worker, 3).is_ok());
    release.send(()).unwrap();
    held.join().unwrap();
    assert_eq!(second, Some(true), "the second write waited for the reader that was still running");
    assert_eq!(support::count(&store), 3);
}

#[test]
fn the_checkpoint_put_off_for_a_long_read_runs_once_that_read_ends() {
    let (dir, store) = open();
    let wal = dir.path().join("archive/history-vault.db-wal");
    let (release, held) = hold_reader(&store, Db::Archive, "SELECT count(*) FROM visits");
    store
        .write(Db::Archive, Priority::Batch, |tx| {
            tx.execute(
                "WITH RECURSIVE seq(x) AS (VALUES(1) UNION ALL SELECT x+1 FROM seq WHERE x<200) INSERT INTO visits(id,url,title,visited_at) SELECT 1000+x,'https://example.test',hex(zeroblob(400)),x FROM seq",
                (),
            )?;
            Ok(())
        })
        .unwrap();
    wait_until_put_off(&store, 1);
    let before = store.metrics(Db::Archive).truncate_checkpoints;
    assert!(std::fs::metadata(&wal).unwrap().len() > 32, "the write left nothing to checkpoint");
    release.send(()).unwrap();
    held.join().unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    while std::fs::metadata(&wal).unwrap().len() > 32 {
        assert!(Instant::now() < deadline, "the WAL was not truncated after the long read ended");
        thread::sleep(Duration::from_millis(5));
    }
    assert!(store.metrics(Db::Archive).truncate_checkpoints > before);
    assert_eq!(support::count(&store), 201);
}

#[test]
fn later_writes_do_not_keep_readers_out_again_for_the_same_long_read() {
    let (_dir, store) = open();
    let (release, held) = hold_reader(&store, Db::Archive, "SELECT count(*) FROM visits");
    // A write is served only once the writer is done with the one before it, waiting for
    // readers included. So by the last of these, every earlier one that waited has been counted.
    for id in 2..23 {
        support::insert(&store, id).unwrap();
    }
    let put_off = store.metrics(Db::Archive).truncates_put_off;
    release.send(()).unwrap();
    held.join().unwrap();
    assert_eq!(put_off, 1, "the writer waited for the same read more than once");
}

#[test]
fn word_of_a_reader_that_left_earlier_does_not_bring_a_second_wait() {
    let (_dir, store) = open();
    // A write that stays open: until it ends, the writer hears nothing.
    let (writing_tx, writing_rx) = mpsc::channel();
    let (finish_tx, finish_rx) = mpsc::channel::<()>();
    let worker = store.clone();
    let write = thread::spawn(move || {
        worker
            .write(Db::Archive, Priority::Batch, move |tx| {
                tx.execute(
                    "INSERT INTO visits(id,url,title,visited_at) VALUES (2,'https://example.test','page',2)",
                    (),
                )?;
                writing_tx.send(()).unwrap();
                let _ = finish_rx.recv();
                Ok(())
            })
            .unwrap()
    });
    writing_rx.recv_timeout(Duration::from_secs(10)).unwrap();
    // A short read ends with nobody else inside, which the writer is told and hears later.
    assert_eq!(support::count(&store), 1);
    // By then this read is inside, and stays.
    let (release, held) = hold_reader(&store, Db::Archive, "SELECT count(*) FROM visits");
    finish_tx.send(()).unwrap();
    write.join().unwrap();
    wait_until_put_off(&store, 1);
    // With nothing queued the writer now hears of the short read. Each of these is served only
    // after it is done with what came before, so whatever that news made it do has been counted.
    for id in 3..6 {
        support::insert(&store, id).unwrap();
    }
    let put_off = store.metrics(Db::Archive).truncates_put_off;
    release.send(()).unwrap();
    held.join().unwrap();
    assert_eq!(put_off, 1, "the writer waited again for the read it had given up on");
}

#[test]
fn a_second_long_read_inside_at_the_wait_does_not_bring_another_once_the_first_leaves() {
    let (_dir, store) = open();
    let (release_first, first) = hold_reader(&store, Db::Archive, "SELECT count(*) FROM visits");
    let (release_second, second) = hold_reader(&store, Db::Archive, "SELECT count(*) FROM visits");
    support::insert(&store, 2).unwrap();
    wait_until_put_off(&store, 1);
    release_first.send(()).unwrap();
    first.join().unwrap();
    for id in 3..6 {
        support::insert(&store, id).unwrap();
    }
    let put_off = store.metrics(Db::Archive).truncates_put_off;
    release_second.send(()).unwrap();
    second.join().unwrap();
    assert_eq!(put_off, 1, "the writer waited again for a read that was inside the first time");
}
