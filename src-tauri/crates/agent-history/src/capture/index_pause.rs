//! A pause applies only to the final lexical index update. Yield outside the index writer so
//! searches continue using its last publication and source files remain committed.
use super::{BackupOptions, notify_phase};
use crate::{Archive, Error, Result};
use std::sync::{
    Condvar, Mutex,
    atomic::{AtomicBool, Ordering},
};
use std::time::Duration;

#[derive(Default, PartialEq, Eq, Clone, Copy)]
enum State {
    #[default]
    Inactive,
    Running,
    Paused,
}
#[derive(Default)]
pub struct IndexPause {
    state: Mutex<State>,
    wake: Condvar,
}
impl IndexPause {
    pub fn paused(&self) -> bool {
        *self.state.lock().unwrap() == State::Paused
    }
    /// Refuse controls delivered after the indexing phase ended.
    pub fn set(&self, pause: bool) -> bool {
        let mut state = self.state.lock().unwrap();
        if *state == State::Inactive {
            return false;
        }
        *state = if pause { State::Paused } else { State::Running };
        self.wake.notify_all();
        true
    }
    pub(super) fn synchronize(&self, a: &Archive, opts: &BackupOptions) -> Result<()> {
        *self.state.lock().unwrap() = State::Running;
        notify_phase(opts, "index", None);
        let result = super::debug_delay::wait(
            super::debug_delay::configured("PATHKEEP_TEST_AGENT_INDEX_DELAY_US"),
            &opts.cancel,
        )
        .and_then(|()| self.run(a, opts));
        *self.state.lock().unwrap() = State::Inactive;
        result
    }
    fn run(&self, a: &Archive, opts: &BackupOptions) -> Result<()> {
        loop {
            let mut state = self.state.lock().unwrap();
            while *state == State::Paused {
                super::check_cancel(opts)?;
                state = self.wake.wait_timeout(state, Duration::from_millis(100)).unwrap().0;
            }
            drop(state);
            let yielded = AtomicBool::new(false);
            let result = a.synchronize_search_with_cancel(&|| {
                if self.paused() {
                    yielded.store(true, Ordering::Relaxed);
                    return true;
                }
                opts.cancel.check().is_err()
            });
            super::check_cancel(opts)?;
            match result {
                Err(Error::Cancelled) if yielded.load(Ordering::Relaxed) => continue,
                other => return other.map(drop),
            }
        }
    }
}
