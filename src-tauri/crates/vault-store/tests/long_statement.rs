//! Responsibilities: a statement that runs long is named in the log while it runs.
//! Not responsible for: stopping it, or how long a statement should take.
//! Its own binary, because the logger is process-wide.
//! Failure modes (2026-11-13: one statement ran for hours and the log said nothing; it was
//! found by sampling the process):
//! - a statement runs long and no line names it until it ends, or ever;
//! - the line carries a value written into the statement, which may be the user's data;
//! - statements that end soon fill the log;
//! - nothing says the long statement has ended.
mod support;
use std::{
    sync::{Arc, Mutex, mpsc},
    thread,
    time::{Duration, Instant},
};
use vault_store::{Db, Store};

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

fn line_with(text: &str) -> Option<String> {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if let Some(line) = LOG.0.lock().unwrap().iter().find(|line| line.contains(text)) {
            return Some(line.clone());
        }
        if Instant::now() > deadline {
            return None;
        }
        thread::sleep(Duration::from_millis(5));
    }
}

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
                c.for_each(
                    "SELECT id FROM visits WHERE url <> 'https://private.example/a path' ORDER BY id",
                    (),
                    |_| {
                        if !waited {
                            waited = true;
                            started_tx.send(()).unwrap();
                            release_rx.recv().unwrap();
                        }
                        Ok(())
                    },
                )
            })
            .unwrap()
    });
    started_rx.recv_timeout(Duration::from_secs(10)).unwrap();
    let running = line_with("has been running one statement").expect("no line while it ran");
    assert!(running.starts_with("WARN Archive reader"), "{running}");
    assert!(running.contains("SELECT id FROM visits WHERE url <> ? ORDER BY id"), "{running}");
    assert!(!running.contains("private"), "{running}");
    release_tx.send(()).unwrap();
    reader.join().unwrap();
    let ended = line_with("finished the statement").expect("no line when it ended");
    assert!(ended.starts_with("WARN Archive reader"), "{ended}");

    // Forty short statements came before it: only the long one is in the log.
    let lines = LOG.0.lock().unwrap().clone();
    assert!(
        lines
            .iter()
            .all(|line| line.contains("SELECT id FROM visits") || line.contains("finished")),
        "{lines:#?}"
    );
}
