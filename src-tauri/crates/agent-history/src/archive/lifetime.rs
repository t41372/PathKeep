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
    readers: Vec<Arc<CancelToken>>,
}

#[derive(Default)]
pub(crate) struct Jobs {
    state: Mutex<State>,
    idle: Condvar,
}

impl State {
    fn check_admission(&self) -> Result<()> {
        if self.closed {
            return Err(vault_store::Error::Closed.into());
        }
        if self.pauses > 0 {
            return Err(crate::Error::Cancelled);
        }
        Ok(())
    }
}

impl Jobs {
    pub(crate) fn enter_reader(&self, cancel: &CancelToken) -> Result<Job<'_>> {
        let mut state = self.state.lock().unwrap();
        state.check_admission()?;
        let token = Arc::new(cancel.clone());
        state.readers.push(token.clone());
        state.active += 1;
        Ok(Job { jobs: self, capture: false, reader: Some(token) })
    }

    pub(crate) fn enter(&self, capture: Option<&CancelToken>) -> Result<Job<'_>> {
        let mut state = self.state.lock().unwrap();
        state.check_admission()?;
        if let Some(token) = capture {
            if state.capture.is_some() {
                return Err(crate::Error::InvalidInput(
                    "another backup already owns this archive".into(),
                ));
            }
            state.capture = Some(token.clone());
        }
        state.active += 1;
        Ok(Job { jobs: self, capture: capture.is_some(), reader: None })
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
        for reader in &state.readers {
            reader.cancel()
        }
        drop(self.idle.wait_while(state, |state| state.active > 0).unwrap());
    }
}

pub(crate) struct Job<'a> {
    jobs: &'a Jobs,
    capture: bool,
    reader: Option<Arc<CancelToken>>,
}

impl Drop for Job<'_> {
    fn drop(&mut self) {
        let mut state = self.jobs.state.lock().unwrap();
        state.active -= 1;
        if let Some(reader) = &self.reader {
            state.readers.retain(|other| !Arc::ptr_eq(other, reader));
        }
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
