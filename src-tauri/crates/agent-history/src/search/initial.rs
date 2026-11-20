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
        use vault_store::{Db, Priority};
        let missing = !self.search_path.try_exists()?;
        let pending = self.store.read(Db::AgentHistoryReadModel, |s| {
            s.query_one(
                "SELECT EXISTS(SELECT 1 FROM ah_meta WHERE key='initial_search_pending')",
                (),
                |r| Ok(r.get::<i64>(0)? != 0),
            )
        })?;
        if !missing && !resume && !pending {
            match super::index::open(self) {
                Ok(index)
                    if index.watermark < self.projection_generation()?
                        || self.search_cards_dirty()? => {}
                Err(crate::Error::IndexBehind)
                    if super::index::previous_schema(self).unwrap_or(false) => {}
                _ => return Ok(()),
            }
        }
        if missing && !pending {
            self.store.write(Db::AgentHistoryReadModel, Priority::Batch, |s| {
                s.execute(
                    "INSERT OR IGNORE INTO ah_meta(key,value) VALUES('initial_search_pending','1')",
                    (),
                )?;
                Ok(())
            })?;
        }
        self.synchronize_search_with_cancel(&|| token.check().is_err())?;
        if missing || pending {
            self.store.write(Db::AgentHistoryReadModel, Priority::Batch, |s| {
                s.execute("DELETE FROM ah_meta WHERE key='initial_search_pending'", ())?;
                Ok(())
            })?;
        }
        Ok(())
    }
    /// Reader repair binds both database files. Initial search publishes its selectors
    /// into the same read model, so its last write must precede the reviewed checkpoint.
    pub fn wait_for_search_preparation(&self, cancel: &CancelToken) -> Result<()> {
        let pending = &self.initial_search;
        let mut state = pending.state.lock().unwrap();
        while state.active.is_some() {
            cancel.check()?;
            state =
                pending.ready.wait_timeout(state, std::time::Duration::from_millis(100)).unwrap().0;
        }
        cancel.check()?;
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Harness, SearchQuery, SourceRoot, capture::BackupOptions};
    #[test]
    fn owner_loss_before_the_first_watermark_resumes_from_retained_bytes() {
        const CHILD: &str = "PATHKEEP_INITIAL_INDEX_CRASH_FIXTURE";
        if let Some(root) = std::env::var_os(CHILD) {
            let archive = Arc::new(Archive::open(std::path::Path::new(&root), b"").unwrap());
            archive.prepare_visible_search().unwrap();
            archive.initial_search.wait();
            panic!("did not stop at initial index creation");
        }
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("archive");
        let native = tmp.path().join("pi");
        std::fs::create_dir_all(native.join("sessions")).unwrap();
        std::fs::write(
            native.join("sessions/one.jsonl"),
            br#"{"type":"session","version":3,"id":"saved","cwd":"/fixture"}
{"type":"message","id":"m","message":{"role":"user","content":"indexcrashneedle"}}
"#,
        )
        .unwrap();
        {
            let archive = Archive::create_unencrypted(&root).unwrap();
            archive
                .backup(
                    &[SourceRoot { root: native.clone(), harness: Harness::Pi }],
                    &BackupOptions::default(),
                )
                .unwrap();
            std::fs::remove_dir_all(&archive.search_path).unwrap();
        }
        std::fs::remove_dir_all(native).unwrap();
        let name = std::thread::current().name().unwrap().to_owned();
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", &name])
            .env(CHILD, &root)
            .status()
            .unwrap();
        assert_eq!(status.code(), Some(76));
        let archive = Arc::new(Archive::open(&root, b"").unwrap());
        archive.prepare_visible_search().unwrap();
        assert_eq!(
            archive
                .search(&SearchQuery {
                    text: "indexcrashneedle".into(),
                    harness: None,
                    project_path: None,
                    limit: 5
                })
                .unwrap()
                .total,
            1
        );
    }
}
