//! Coordinates work that can outlive a single store transaction, including cached raw reads.
//! The storage owner decides when to pause or retire a generation and closes the store itself.

use super::Archive;
use crate::Result;
use std::sync::{Arc, Condvar, Mutex};
use vault_store::CancelToken;

#[derive(Default)]
struct State {
    closed: bool,
    pauses: usize,
    active: usize,
    // Backup and reparse share one producer slot.
    capture: Option<CancelToken>,
}

#[derive(Default)]
pub(crate) struct Jobs {
    state: Mutex<State>,
    idle: Condvar,
}

impl Jobs {
    pub(crate) fn enter(self: &Arc<Self>, capture: Option<&CancelToken>) -> Result<Job> {
        let mut state = self.state.lock().unwrap();
        if state.closed {
            return Err(vault_store::Error::Closed.into());
        }
        if state.pauses > 0 {
            return Err(crate::Error::Cancelled);
        }
        if let Some(token) = capture {
            if state.capture.is_some() {
                return Err(crate::Error::InvalidInput(
                    "another backup already owns this archive".into(),
                ));
            }
            state.capture = Some(token.clone());
        }
        state.active += 1;
        Ok(Job { jobs: self.clone(), capture: capture.is_some() })
    }

    pub(crate) fn stopping(&self) -> bool {
        let state = self.state.lock().unwrap();
        state.closed || state.pauses > 0
    }

    fn drain(&self, permanent: bool) {
        let mut state = self.state.lock().unwrap();
        if permanent {
            state.closed = true;
        } else {
            state.pauses += 1;
        }
        if let Some(token) = &state.capture {
            token.cancel();
        }
        drop(self.idle.wait_while(state, |state| state.active > 0).unwrap());
    }
}

pub(crate) struct Job {
    jobs: Arc<Jobs>,
    capture: bool,
}

impl Drop for Job {
    fn drop(&mut self) {
        let mut state = self.jobs.state.lock().unwrap();
        state.active -= 1;
        if self.capture {
            state.capture = None;
        }
        if state.active == 0 {
            self.jobs.idle.notify_all();
        }
    }
}

/// Holds the committed capture boundary across a copy or a generation replacement.
pub struct ArchivePause(Arc<Jobs>);

impl Drop for ArchivePause {
    fn drop(&mut self) {
        self.0.state.lock().unwrap().pauses -= 1;
    }
}

impl Archive {
    /// Cancels and drains producers before the owner freezes databases. Dropping the
    /// guard resumes admission after maintenance, including a failed replacement.
    pub fn pause_jobs(&self) -> ArchivePause {
        self.jobs.drain(false);
        ArchivePause(self.jobs.clone())
    }

    /// Permanently drains file users and releases the search subkey before store shutdown.
    pub fn stop_jobs(&self) {
        self.jobs.drain(true);
        self.search_key.lock().unwrap().take();
    }
}
