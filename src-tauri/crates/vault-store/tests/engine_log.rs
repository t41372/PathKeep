//! Responsibilities: prove the storage engine's diagnostics reach `log` only as warnings and errors.
//! Not responsible for: the app's log files.
//! Failure modes: Turso's spans or debug events ask the app logger on every B-tree step;
//! engine warnings are dropped. Its own binary, because the logger and dispatcher are process-wide.
use std::sync::{
    Mutex,
    atomic::{AtomicUsize, Ordering},
};

/// Counts what reaches `log`, as the app's logger at Debug would see it.
struct Counting {
    asked: AtomicUsize,
    lines: Mutex<Vec<String>>,
}
impl log::Log for Counting {
    fn enabled(&self, _: &log::Metadata<'_>) -> bool {
        self.asked.fetch_add(1, Ordering::Relaxed);
        true
    }
    fn log(&self, record: &log::Record<'_>) {
        self.lines.lock().unwrap().push(format!("{} {}", record.level(), record.args()));
    }
    fn flush(&self) {}
}

// One test: the logger and the dispatcher are process-wide.
#[test]
fn engine_spans_never_reach_the_logger_and_warnings_do() {
    static LOGGER: Counting =
        Counting { asked: AtomicUsize::new(0), lines: Mutex::new(Vec::new()) };
    log::set_logger(&LOGGER).unwrap();
    log::set_max_level(log::LevelFilter::Trace);
    vault_store::route_engine_diagnostics_to_log();

    for page in 0..1_000 {
        let span = tracing::debug_span!("seek", page);
        let _entered = span.enter();
        tracing::trace!(page, "visited");
        tracing::debug!("descended");
    }
    tracing::warn!(pages = 3, "checkpoint was busy");
    tracing::error!("page checksum failed");

    let lines = LOGGER.lines.lock().unwrap().clone();
    assert_eq!(lines, ["WARN checkpoint was busy pages=3", "ERROR page checksum failed"]);
    assert_eq!(LOGGER.asked.load(Ordering::Relaxed), 0, "spans or debug events asked the logger");

    // Turso's own spans too: a store's work never reaches the logger.
    let dir = tempfile::tempdir().unwrap();
    let store = vault_store::Store::open(vault_store::Config::new(dir.path())).unwrap();
    store
        .write(vault_store::Db::Archive, vault_store::Priority::Interactive, |s| {
            s.execute("CREATE TABLE t(x INTEGER PRIMARY KEY, y TEXT)", ())?;
            s.execute(
                "WITH RECURSIVE n(i) AS (VALUES(1) UNION ALL SELECT i+1 FROM n WHERE i<2000)
                 INSERT INTO t SELECT i, 'row '||i FROM n",
                (),
            )?;
            Ok(())
        })
        .unwrap();
    store.close().unwrap();
    assert_eq!(LOGGER.asked.load(Ordering::Relaxed), 0, "the engine's spans asked the logger");
}
