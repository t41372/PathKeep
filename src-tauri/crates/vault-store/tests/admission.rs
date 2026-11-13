//! Responsibilities: a read that runs long delays the idle checkpoint, and nothing else.
//! Not responsible for: how long a read may run, or WAL size while one is still running.
//! Failure modes (2026-11-13: one statement ran for hours on an Intelligence reader and every
//! other read of Archive and Intelligence waited for it, because a writer that had just
//! committed kept new readers out until the readers already running had left):
//! - after a write, a new read waits for an unrelated read that is still running;
//! - the same across databases: an Intelligence read of `archive.*` holds up Archive reads;
//! - the writer stays parked behind that read, so later writes are never served;
//! - the checkpoint put off for a long read is forgotten once that read ends.
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
    // Longer than the writer waits for readers, so its checkpoint has been put off by now.
    thread::sleep(Duration::from_millis(500));
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
    // The first write after the read began waits for it once, then gives up on it.
    support::insert(&store, 2).unwrap();
    assert_eq!(support::count(&store), 2);
    // Were each of these writes to wait for that read again, every read after one would wait
    // with it: twenty of them, a second in all.
    let started = Instant::now();
    for id in 3..23 {
        support::insert(&store, id).unwrap();
        assert_eq!(support::count(&store), id);
    }
    let took = started.elapsed();
    release.send(()).unwrap();
    held.join().unwrap();
    assert!(took < Duration::from_millis(500), "twenty writes and reads took {took:?}");
}
