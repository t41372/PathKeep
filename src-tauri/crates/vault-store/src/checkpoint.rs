//! Responsibilities: coordinate reader admission with idle WAL truncation and record results.
//! Not responsible for: automatic checkpoint tuning or interrupting statements.
//! Failure modes: TRUNCATE races new readers; reader completion fails to wake an idle
//! writer; busy checkpoint is reported as success; WAL grows across short reads; a read that
//! runs long keeps every other reader out for as long as it runs, or once more each time the
//! writer thinks of truncating. Tests: checkpoint tests, wal tests and admission tests.
use crate::{Error, Result, engine::Engine};
use std::{
    sync::{Arc, Condvar, Mutex},
    time::{Duration, Instant},
};

/// Operational measurements distinguish a completed commit from later maintenance.
#[derive(Clone, Debug, Default)]
pub struct Metrics {
    pub interactive_pending: usize,
    pub batch_pending: usize,
    pub passive_checkpoints: u64,
    pub truncate_checkpoints: u64,
    /// Idle truncates the writer put off because a read was still running when it had waited
    /// long enough. The reads inside at one wait cost one, however many writes come while
    /// they run.
    pub truncates_put_off: u64,
    pub checkpoint_busy: u64,
    pub last_checkpoint_frames: Option<i64>,
    pub last_checkpoint_backfilled: Option<i64>,
    pub last_checkpoint_error: Option<Arc<Error>>,
    /// Statements compiled on this database's connections. Reused statements
    /// don't count, so a batch that repeats one statement compiles it once.
    pub statements_compiled: u64,
}
#[derive(Default)]
struct Admission {
    /// The readers inside, by entry number. Numbers only grow, so a lower one entered earlier.
    inside: Vec<u64>,
    entered: u64,
    truncating: bool,
    /// The newest reader there was when the writer last gave up waiting. Every reader that
    /// was inside then has this number or a lower one.
    given_up: u64,
}
pub(crate) struct Gate {
    state: Mutex<Admission>,
    changed: Condvar,
    wake: Box<dyn Fn() + Send + Sync>,
}
pub(crate) struct ReadGuard<'a>(&'a Gate, u64);
pub(crate) struct TruncateGuard<'a>(&'a Gate);

impl Gate {
    pub fn new(wake: impl Fn() + Send + Sync + 'static) -> Self {
        Self {
            state: Mutex::new(Admission::default()),
            changed: Condvar::new(),
            wake: Box::new(wake),
        }
    }
    pub fn enter(&self) -> ReadGuard<'_> {
        let mut s = self.state.lock().unwrap();
        while s.truncating {
            s = self.changed.wait(s).unwrap()
        }
        s.entered += 1;
        let entry = s.entered;
        s.inside.push(entry);
        ReadGuard(self, entry)
    }
    // Reserve admission only while the writer is idle. Existing short readers
    // finish normally; new readers wait for this checkpoint, never for a batch, and
    // never for a reader that is still running (`TruncateGuard::wait_for_readers`).
    /// `None` while a reader the writer has already given up waiting for is still inside.
    /// Keeping new readers out for it again would gain nothing, whatever made the writer
    /// ask: a later write, or word that some other reader has left.
    pub fn reserve_truncate(&self) -> Option<TruncateGuard<'_>> {
        let mut s = self.state.lock().unwrap();
        let given_up = s.given_up;
        if s.inside.iter().any(|entry| *entry <= given_up) {
            return None;
        }
        s.truncating = true;
        Some(TruncateGuard(self))
    }
    pub fn try_truncate(&self) -> Option<TruncateGuard<'_>> {
        let mut s = self.state.lock().unwrap();
        if !s.inside.is_empty() || s.truncating {
            return None;
        }
        s.truncating = true;
        Some(TruncateGuard(self))
    }
}
impl TruncateGuard<'_> {
    /// Waits, for at most `limit`, until the readers already inside have left, and says
    /// whether they have. If one is still running, the caller drops this guard, so readers are
    /// let in again, and truncates later: [`Gate::reserve_truncate`] gives no other guard
    /// until the readers given up on here are gone. Waiting without a limit made every new
    /// read wait for the slowest one running (2026-11-13: for hours).
    pub fn wait_for_readers(&self, limit: Duration) -> bool {
        let deadline = Instant::now() + limit;
        let mut state = self.0.state.lock().unwrap();
        while !state.inside.is_empty() {
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() {
                state.given_up = state.entered;
                return false;
            }
            state = self.0.changed.wait_timeout(state, left).unwrap().0;
        }
        true
    }
}
impl Drop for ReadGuard<'_> {
    fn drop(&mut self) {
        let mut s = self.0.state.lock().unwrap();
        s.inside.retain(|entry| *entry != self.1);
        let last = s.inside.is_empty();
        self.0.changed.notify_all();
        drop(s);
        if last {
            (self.0.wake)();
        }
    }
}
impl Drop for TruncateGuard<'_> {
    fn drop(&mut self) {
        self.0.state.lock().unwrap().truncating = false;
        self.0.changed.notify_all();
    }
}

pub(crate) fn run(engine: &Engine, truncate: bool, metrics: &Mutex<Metrics>) -> bool {
    let result: Result<(i64, Option<i64>, Option<i64>)> = engine.rt.block_on(async {
        let mut rows = engine
            .conn
            .query(
                if truncate {
                    "PRAGMA wal_checkpoint(TRUNCATE)"
                } else {
                    "PRAGMA wal_checkpoint(PASSIVE)"
                },
                (),
            )
            .await?;
        let row = rows.next().await?.ok_or(turso::Error::QueryReturnedNoRows)?;
        let result = (row.get(0)?, row.get(1)?, row.get(2)?);
        while rows.next().await?.is_some() {}
        Ok(result)
    });
    let mut m = metrics.lock().unwrap();
    match result {
        Ok((busy, frames, backfilled)) => {
            if truncate {
                m.truncate_checkpoints += 1
            } else {
                m.passive_checkpoints += 1
            }
            m.last_checkpoint_frames = frames;
            m.last_checkpoint_backfilled = backfilled;
            if busy != 0 {
                m.checkpoint_busy += 1;
            }
            m.last_checkpoint_error = None;
            busy == 0
        }
        Err(error) => {
            m.last_checkpoint_error = Some(Arc::new(error));
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    #[test]
    fn active_readers_exclude_truncate_and_last_exit_notifies() {
        let calls = Arc::new(AtomicUsize::new(0));
        let count = calls.clone();
        let gate = Gate::new(move || {
            count.fetch_add(1, Ordering::SeqCst);
        });
        let reader = gate.enter();
        assert!(gate.try_truncate().is_none());
        drop(reader);
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        let truncate = gate.try_truncate().unwrap();
        assert!(gate.try_truncate().is_none());
        drop(truncate);
        assert!(gate.try_truncate().is_some());
    }

    /// What the writer does when a reader outlasts its wait. No time passes: the limit is zero.
    fn give_up(gate: &Gate) {
        let guard = gate.reserve_truncate().expect("nothing kept the writer from waiting");
        assert!(!guard.wait_for_readers(Duration::ZERO));
    }
    #[test]
    fn readers_are_kept_out_once_for_a_reader_that_stays() {
        let gate = Gate::new(|| {});
        let long = gate.enter();
        give_up(&gate);
        // Asked again, whatever the reason: a later write, word of a reader that left before.
        assert!(gate.reserve_truncate().is_none());
        let newer = gate.enter();
        assert!(gate.reserve_truncate().is_none());
        drop(long);
        // `newer` came in after that wait, so the writer waits for it once too.
        give_up(&gate);
        assert!(gate.reserve_truncate().is_none());
        drop(newer);
        assert!(gate.reserve_truncate().unwrap().wait_for_readers(Duration::ZERO));
    }
    #[test]
    fn every_reader_inside_at_the_wait_has_been_given_up_on() {
        let gate = Gate::new(|| {});
        let first = gate.enter();
        let second = gate.enter();
        give_up(&gate);
        drop(first);
        assert!(gate.reserve_truncate().is_none(), "the second reader was waited for as well");
        drop(second);
        assert!(gate.reserve_truncate().is_some());
    }
}

#[cfg(test)]
mod engine_measurements {
    use super::*;
    use crate::{Config, Db, engine::Settings};
    #[test]
    fn autocheckpoint_pragma_is_ignored_and_native_threshold_is_measured() {
        let dir = tempfile::tempdir().unwrap();
        let config = Config::new(dir.path());
        crate::engine::prepare_paths(&config).unwrap();
        let mut settings = Settings::new(&config, Db::Archive);
        settings.scratch = None;
        let engine = Engine::open(&settings, Default::default()).unwrap();
        engine
            .rt
            .block_on(
                engine.conn.execute("CREATE TABLE rows(id INTEGER PRIMARY KEY,payload BLOB)", ()),
            )
            .unwrap();
        let metrics = Mutex::new(Metrics::default());
        assert!(run(&engine, true, &metrics));
        let before = std::fs::metadata(&settings.path).unwrap().len();
        engine.rt.block_on(engine.conn.execute("PRAGMA wal_autocheckpoint=0", ())).unwrap();
        let pragma_rows = engine.rt.block_on(async {
            let mut rows = engine.conn.query("PRAGMA wal_autocheckpoint", ()).await.unwrap();
            let mut count = 0;
            while rows.next().await.unwrap().is_some() {
                count += 1
            }
            count
        });
        assert_eq!(pragma_rows, 0, "a Turso upgrade changed the no-op pragma contract");
        crate::session::transaction(&engine.session(false),|tx|{tx.execute("WITH RECURSIVE seq(x) AS (VALUES(1) UNION ALL SELECT x+1 FROM seq WHERE x<1000) INSERT INTO rows SELECT x,zeroblob(1000) FROM seq",())?;Ok(())}).unwrap();
        let small_commit = std::fs::metadata(&settings.path).unwrap().len();
        assert_eq!(small_commit, before, "small commit unexpectedly triggered auto-checkpoint");
        crate::session::transaction(&engine.session(false),|tx|{tx.execute("WITH RECURSIVE seq(x) AS (VALUES(1001) UNION ALL SELECT x+1 FROM seq WHERE x<3000) INSERT INTO rows SELECT x,zeroblob(4000) FROM seq",())?;Ok(())}).unwrap();
        let after = std::fs::metadata(&settings.path).unwrap().len();
        assert!(after > before, "commit did not backfill before our explicit checkpoint");
        println!(
            "autocheckpoint_pragma_rows={pragma_rows} main_bytes_before={before} main_bytes_after_small_commit={small_commit} main_bytes_after_large_commit={after}"
        );
        assert!(run(&engine, false, &metrics));
        let metrics = metrics.lock().unwrap();
        assert_eq!(metrics.last_checkpoint_frames, metrics.last_checkpoint_backfilled);
    }
}
