//! Responsibilities: a statement that runs long is named in the log while it runs.
//! Not responsible for: stopping it, or how long a statement should take.
//! Its own binary, because the logger is process-wide.
//! Failure modes (2026-11-13: one statement ran for hours and the log said nothing; it was
//! found by sampling the process):
//! - a statement runs long and no line names it until it ends, or ever;
//! - only some ways of running a statement are watched: one prepared first, then run or read
//!   row by row, goes unnamed;
//! - a cursor read to its end and kept still counts as running;
//! - the line carries a value written into the statement, which may be the user's data;
//! - statements that end soon fill the log;
//! - nothing says the long statement has ended.
mod support;
use std::{
    sync::{Arc, Mutex, mpsc},
    thread,
    time::{Duration, Instant},
};
use vault_store::{Db, Priority, Store};

struct Lines(Mutex<Vec<String>>);
impl log::Log for Lines {
    fn enabled(&self, _: &log::Metadata<'_>) -> bool {
        true
    }
    fn log(&self, record: &log::Record<'_>) {
        self.0.lock().unwrap().push(format!("{} {}", record.level(), record.args()));
    }
    fn flush(&self) {}
}
static LOG: Lines = Lines(Mutex::new(Vec::new()));

/// The first line from `from` on that has `text` in it. The deadline only keeps a broken build
/// from hanging the suite.
fn line_with(from: usize, text: &str) -> Option<String> {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if let Some(line) = LOG.0.lock().unwrap().iter().skip(from).find(|l| l.contains(text)) {
            return Some(line.clone());
        }
        if Instant::now() > deadline {
            return None;
        }
        thread::sleep(Duration::from_millis(5));
    }
}
fn lines() -> usize {
    LOG.0.lock().unwrap().len()
}

const WALK: &str =
    "SELECT id FROM visits WHERE url <> 'https://private.example/a path' ORDER BY id";
const PREPARED: &str =
    "SELECT title FROM visits WHERE url <> 'https://private.example/b path' ORDER BY id";
/// Counts to `?1` and inserts one row: as slow as the number is large.
const SLOW_INSERT: &str = "WITH RECURSIVE seq(x) AS (VALUES(1) UNION ALL SELECT x+1 FROM seq WHERE x<?1) \
     INSERT INTO visits(id,url,title,visited_at) \
     SELECT 1000000+x,'https://private.example/c path','page',x FROM seq WHERE x=?1";

// One test: the logger is process-wide.
#[test]
fn a_statement_that_runs_long_is_named_while_it_runs_and_when_it_ends() {
    log::set_logger(&LOG).unwrap();
    log::set_max_level(log::LevelFilter::Info);
    let dir = tempfile::tempdir().unwrap();
    let mut config = support::config(dir.path());
    config.long_statement_after = Duration::from_millis(100);
    let store = Arc::new(Store::open(config).unwrap());
    for id in 1..=20 {
        support::insert(&store, id).unwrap();
        assert_eq!(support::count(&store), id);
    }

    // A statement stays open for as long as its rows are being read.
    let (started_tx, started_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel::<()>();
    let worker = store.clone();
    let reader = thread::spawn(move || {
        worker
            .read(Db::Archive, move |c| {
                let mut waited = false;
                c.for_each(WALK, (), |_| {
                    if !waited {
                        waited = true;
                        started_tx.send(()).unwrap();
                        release_rx.recv().unwrap();
                    }
                    Ok(())
                })
            })
            .unwrap()
    });
    started_rx.recv_timeout(Duration::from_secs(10)).unwrap();
    let running = line_with(0, "has been running one statement").expect("no line while it ran");
    assert!(running.starts_with("WARN Archive reader"), "{running}");
    assert!(running.contains("SELECT id FROM visits WHERE url <> ? ORDER BY id"), "{running}");
    assert!(!running.contains("private"), "{running}");
    release_tx.send(()).unwrap();
    reader.join().unwrap();
    let ended = line_with(0, "finished the statement").expect("no line when it ended");
    assert!(ended.starts_with("WARN Archive reader"), "{ended}");

    // Forty short statements came before it: only the long one is in the log.
    let so_far = LOG.0.lock().unwrap().clone();
    assert!(
        so_far
            .iter()
            .all(|line| line.contains("SELECT id FROM visits") || line.contains("finished")),
        "{so_far:#?}"
    );

    // The same for a statement prepared first and read row by row. It has ended once its last
    // row is read, though the cursor is still there.
    let from = lines();
    let (started_tx, started_rx) = mpsc::channel();
    let (read_on_tx, read_on_rx) = mpsc::channel::<()>();
    let (at_end_tx, at_end_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel::<()>();
    let worker = store.clone();
    let reader = thread::spawn(move || {
        worker
            .read(Db::Archive, move |c| {
                let mut statement = c.prepare(PREPARED)?;
                let mut rows = statement.query(())?;
                rows.next()?;
                started_tx.send(()).unwrap();
                read_on_rx.recv().unwrap();
                while rows.next()?.is_some() {}
                at_end_tx.send(()).unwrap();
                release_rx.recv().unwrap();
                Ok(())
            })
            .unwrap()
    });
    started_rx.recv_timeout(Duration::from_secs(10)).unwrap();
    let running = line_with(from, "has been running one statement")
        .expect("no line while a prepared statement's rows were being read");
    assert!(running.starts_with("WARN Archive reader"), "{running}");
    assert!(running.contains("SELECT title FROM visits WHERE url <> ? ORDER BY id"), "{running}");
    read_on_tx.send(()).unwrap();
    at_end_rx.recv_timeout(Duration::from_secs(10)).unwrap();
    let ended = LOG.0.lock().unwrap().iter().skip(from).any(|l| l.contains("finished"));
    release_tx.send(()).unwrap();
    reader.join().unwrap();
    assert!(ended, "the cursor was read to its end and the statement still counted as running");

    // And for a prepared statement run in one go. Nothing can hold one open, so it is given
    // more to do until it has run long enough to be named, however fast the machine.
    let from = lines();
    let mut count_to = 50_000_i64;
    let running = loop {
        let began = Instant::now();
        store
            .write(Db::Archive, Priority::Batch, move |tx| {
                tx.prepare(SLOW_INSERT)?.execute([count_to])?;
                Ok(())
            })
            .unwrap();
        let took = began.elapsed();
        let named = LOG.0.lock().unwrap().iter().skip(from).find(|l| l.contains("seq(x)")).cloned();
        if let Some(line) = named {
            break line;
        }
        assert!(took < Duration::from_secs(5), "ran for {took:?} and was never named");
        count_to *= 4;
    };
    assert!(running.starts_with("WARN Archive writer"), "{running}");
    assert!(running.contains("SELECT 1000000+x,?,?,x FROM seq WHERE x=?1"), "{running}");
    assert!(!running.contains("private"), "{running}");
}
