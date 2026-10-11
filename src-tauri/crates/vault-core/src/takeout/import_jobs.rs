//! Responsibilities: let the app stop an import it started: each running import is registered
//! under the id its caller chose, with the token its writes check between batches.
//! Not responsible for: what a stopped import leaves (`import_flow`, `browser_history`), or
//! progress, which goes out as events.
//! Failure modes: a Stop meant for one import stops another (a stale click after it finished
//! and the next began); two imports share an id; an import that ended, by any path, stays
//! registered and its id can never be used again.

use anyhow::{Result, bail};
use std::{
    collections::HashMap,
    sync::{Mutex, OnceLock},
};
use vault_store::CancelToken;

static RUNNING: OnceLock<Mutex<HashMap<String, CancelToken>>> = OnceLock::new();

fn running() -> &'static Mutex<HashMap<String, CancelToken>> {
    RUNNING.get_or_init(Mutex::default)
}

/// One running import, registered under its id until it is dropped.
pub struct ImportJob {
    id: Option<String>,
    cancel: CancelToken,
}

impl ImportJob {
    /// Registers an import under `id`; without one, it runs but nothing can stop it.
    pub fn start(id: Option<&str>) -> Result<Self> {
        let cancel = CancelToken::new();
        if let Some(id) = id {
            let mut jobs = running().lock().unwrap();
            if jobs.contains_key(id) {
                bail!("import id is already running");
            }
            jobs.insert(id.to_owned(), cancel.clone());
        }
        Ok(Self { id: id.map(str::to_owned), cancel })
    }

    /// The token the import's writes check.
    pub fn cancel_token(&self) -> &CancelToken {
        &self.cancel
    }
}

impl Drop for ImportJob {
    fn drop(&mut self) {
        if let Some(id) = &self.id {
            running().lock().unwrap().remove(id);
        }
    }
}

/// Asks the running import `id` to stop after the batch it is writing. True if one was running
/// and had not been asked already; an import past its last batch finishes anyway.
pub fn stop_import(id: &str) -> bool {
    let jobs = running().lock().unwrap();
    let Some(cancel) = jobs.get(id) else { return false };
    if cancel.check().is_err() {
        return false;
    }
    cancel.cancel();
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_stop_reaches_only_the_running_import_of_its_id() {
        assert!(!stop_import("job-unknown"), "nothing runs under it");
        let first = ImportJob::start(Some("job-a")).unwrap();
        let other = ImportJob::start(Some("job-b")).unwrap();
        assert!(ImportJob::start(Some("job-a")).is_err(), "one import per id");
        assert!(stop_import("job-a"));
        assert!(first.cancel_token().check().is_err());
        assert!(other.cancel_token().check().is_ok(), "another import runs on");
        assert!(!stop_import("job-a"), "asked already");
        drop(first);
        assert!(!stop_import("job-a"), "it ended: a late click stops nothing");
        let next = ImportJob::start(Some("job-a")).unwrap();
        assert!(next.cancel_token().check().is_ok(), "the id is free again");
        let unnamed = ImportJob::start(None).unwrap();
        assert!(unnamed.cancel_token().check().is_ok());
    }
}
