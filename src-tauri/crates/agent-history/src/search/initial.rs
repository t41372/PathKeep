//! A visible archive can arrive without its disposable index. The owner starts
//! this work; only searches wait for it. Browsing and inventory remain available.
use crate::{Archive, Result};
use std::sync::{Arc, Condvar, Mutex};
use vault_store::CancelToken;

#[derive(Default)]
pub(crate) struct InitialSearch {
    state: Mutex<State>,
    ready: Condvar,
}
#[derive(Default)]
struct State {
    active: Option<Arc<CancelToken>>,
    retry: bool,
}

struct Attempt {
    owner: Arc<Archive>,
    token: Arc<CancelToken>,
    complete: bool,
}
impl Drop for Attempt {
    fn drop(&mut self) {
        let preparation = &self.owner.initial_search;
        let mut state = preparation.state.lock().unwrap();
        if state.active.as_ref().is_some_and(|t| Arc::ptr_eq(t, &self.token)) {
            state.active = None;
            state.retry = !self.complete;
            preparation.ready.notify_all();
        }
    }
}

impl Archive {
    /// Prepare an omitted index at the owner's visibility transition. A present
    /// but unreadable index stays on the explicit repair path.
    pub fn prepare_visible_search(self: &Arc<Self>) -> Result<()> {
        let mut state = self.initial_search.state.lock().unwrap();
        if state.active.as_ref().is_some_and(|t| t.check().is_ok()) {
            return Ok(());
        }
        // Hold the generation through the watermark check too. Maintenance may
        // retire the index and its key as soon as the last lease is dropped.
        let lease = self.jobs.enter(None)?;
        let resume = state.retry || state.active.is_some();
        let token = Arc::new(CancelToken::new());
        state.active = Some(token.clone());
        drop(state);
        let mut attempt = Attempt { owner: self.clone(), token, complete: false };
        std::thread::Builder::new().name("retained-conversation-search".into()).spawn(
            move || {
                let _lease = lease;
                let result = attempt.owner.prepare_retained_index(resume, &attempt.token);
                attempt.complete = result.is_ok();
                if let Err(error) = result
                    && !matches!(error, crate::Error::Cancelled)
                {
                    eprintln!("PathKeep could not prepare retained conversation search: {error}");
                }
                drop(attempt);
            },
        )?;
        Ok(())
    }
    fn prepare_retained_index(&self, resume: bool, token: &CancelToken) -> Result<()> {
        if !resume && self.search_path.try_exists()? {
            match super::index::open(self) {
                Ok(index) if index.watermark < self.projection_generation()? => {}
                _ => return Ok(()),
            }
        }
        self.synchronize_search_with_cancel(&|| token.check().is_err())?;
        Ok(())
    }
    /// Revoking visibility stops the owner-started build at a bounded batch.
    pub fn cancel_visible_search(&self) {
        if let Some(token) = &self.initial_search.state.lock().unwrap().active {
            token.cancel();
        }
    }
}
impl InitialSearch {
    pub(super) fn wait(&self) {
        drop(self.ready.wait_while(self.state.lock().unwrap(), |s| s.active.is_some()).unwrap());
    }
}
