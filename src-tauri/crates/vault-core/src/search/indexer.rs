//! Responsibilities: one owner indexer, ordered outbox replay, rebuild and shutdown.
//! Not responsible for: archive writes, normalization policy or query sessions.
//! Failure modes: acknowledging before commit loses pages; replay duplicates pages;
//! failed rebuild replaces good readers; missing index silently looks empty;
//! close leaves the writer or encryption cache alive; unbounded queues retain the corpus.
//! Tests: search::tests, vault-search canonical/crash/recovery contracts.

use super::{documents, generations::Generations};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::{
    path::Path,
    sync::{Arc, Mutex, mpsc},
    thread::JoinHandle,
    time::Duration,
};
use vault_search::SearchIndex;
use vault_store::{CancelToken, Db, Priority, Store};

/// Coverage accompanies every search response, including an empty partial first build.
#[derive(Clone, Debug, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct IndexerState {
    pub phase: IndexerPhase,
    pub documents_processed: u64,
    pub documents_total: u64,
    pub applied_seq: u64,
    pub target_seq: u64,
    pub error: Option<String>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum IndexerPhase {
    Building,
    Updating,
    Ready,
    Rebuilding,
    Cancelled,
    Failed,
    Closed,
}

type ProgressSink = Box<dyn FnMut(vault_search::RebuildProgress) + Send>;
enum Command {
    Cycle(mpsc::SyncSender<Result<()>>),
    Rebuild(mpsc::SyncSender<Result<()>>, ProgressSink, CancelToken),
    Stop,
    Repair,
    #[cfg(test)]
    Pause(mpsc::SyncSender<()>),
    #[cfg(test)]
    Resume,
}

/// Shared by Vault clones; dropping the final owner joins its only indexer thread.
pub struct Indexer {
    index: Arc<Mutex<Option<Arc<SearchIndex>>>>,
    state: Arc<Mutex<IndexerState>>,
    stop: CancelToken,
    rebuild_cancel: Arc<Mutex<CancelToken>>,
    sender: mpsc::Sender<Command>,
    sessions: Mutex<Vec<super::session::RetainedSession>>,
    repair_requested: Arc<std::sync::atomic::AtomicBool>,
    thread: Mutex<Option<JoinHandle<()>>>,
}
impl Indexer {
    pub(crate) fn open(
        store: Arc<Store>,
        path: &Path,
        key: Option<&vault_crypto::DataKey>,
    ) -> Result<Self> {
        let (generations, index, initial) = Generations::open(path, key)?;
        let applied_seq = index.last_applied_seq()?;
        let index = Arc::new(Mutex::new(Some(Arc::new(index))));
        let state = Arc::new(Mutex::new(IndexerState {
            phase: if initial { IndexerPhase::Building } else { IndexerPhase::Updating },
            documents_processed: 0,
            documents_total: 0,
            applied_seq,
            target_seq: applied_seq,
            error: None,
        }));
        let stop = CancelToken::new();
        let rebuild_cancel = Arc::new(Mutex::new(CancelToken::new()));
        let (sender, receiver) = mpsc::channel();
        let repair_requested = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let worker_repair = repair_requested.clone();
        let worker_index = index.clone();
        let worker_state = state.clone();
        let worker_stop = stop.clone();
        let worker_cancel = rebuild_cancel.clone();
        let thread =
            std::thread::Builder::new().name("search-indexer".into()).spawn(move || {
                let engine = worker_index.lock().unwrap().as_ref().unwrap().clone();
                let mut worker = Worker {
                    store,
                    index: engine,
                    state: worker_state.clone(),
                    stop: worker_stop,
                    cancel: worker_cancel,
                    generations,
                    published: worker_index.clone(),
                };
                let mut result =
                    if initial { worker.rebuild(true, &mut |_| {}) } else { worker.cycle() };
                #[cfg(test)]
                let mut paused = false;
                loop {
                    if let Err(error) = &result {
                        worker.fail(error);
                        if error.downcast_ref::<vault_search::Error>().is_some_and(|error| {
                            matches!(error, vault_search::Error::RebuildNeeded { .. })
                        }) {
                            result = worker.repair();
                            worker_repair.store(false, std::sync::atomic::Ordering::Release);
                        }
                    }
                    if worker.stop.check().is_err() || !worker.store.is_open() {
                        break;
                    }
                    let command = match receiver.recv_timeout(Duration::from_millis(100)) {
                        Ok(command) => Some(command),
                        Err(mpsc::RecvTimeoutError::Timeout) => None,
                        Err(mpsc::RecvTimeoutError::Disconnected) => break,
                    };
                    result = match command {
                        Some(Command::Stop) => break,
                        Some(Command::Repair) => {
                            let result = worker.repair();
                            worker_repair.store(false, std::sync::atomic::Ordering::Release);
                            result
                        }
                        #[cfg(test)]
                        Some(Command::Pause(reply)) => {
                            paused = true;
                            let _ = reply.send(());
                            Ok(())
                        }
                        #[cfg(test)]
                        Some(Command::Resume) => {
                            paused = false;
                            Ok(())
                        }
                        #[cfg(test)]
                        None if paused => continue,
                        Some(Command::Cycle(reply)) => {
                            let result = worker.cycle();
                            if let Err(error) = &result {
                                worker.fail(error);
                            }
                            let _ = reply.send(result);
                            Ok(())
                        }
                        Some(Command::Rebuild(reply, mut progress, cancel)) => {
                            *worker.cancel.lock().unwrap() = cancel;
                            let result = worker.rebuild(false, &mut progress);
                            if let Err(error) = &result {
                                worker.fail(error);
                            }
                            let _ = reply.send(result);
                            Ok(())
                        }
                        None if result.is_ok() => worker.cycle(),
                        None => continue,
                    };
                }
                worker_state.lock().unwrap().phase = IndexerPhase::Closed;
                worker_index.lock().unwrap().take();
            })?;
        Ok(Self {
            index,
            state,
            stop,
            rebuild_cancel,
            sender,
            repair_requested,
            sessions: Mutex::new(Vec::new()),
            thread: Mutex::new(Some(thread)),
        })
    }
    pub(super) fn request_repair(&self) {
        if !self.repair_requested.swap(true, std::sync::atomic::Ordering::AcqRel)
            && self.sender.send(Command::Repair).is_err()
        {
            self.repair_requested.store(false, std::sync::atomic::Ordering::Release);
        }
    }
    pub(super) fn index(&self) -> Result<Arc<SearchIndex>> {
        self.index.lock().unwrap().clone().context("search owner is closed")
    }
    #[cfg(test)]
    pub(crate) fn indexed_ids(&self) -> Result<Vec<u64>> {
        let mut ids = self
            .index()?
            .search(vault_search::Ast::And(Vec::new()), vault_search::Filters::default())?
            .page(usize::MAX, None)?
            .hits
            .into_iter()
            .map(|hit| hit.page_id)
            .collect::<Vec<_>>();
        ids.sort_unstable();
        Ok(ids)
    }
    #[cfg(test)]
    pub(crate) fn pause(&self) -> Pause<'_> {
        let (tx, rx) = mpsc::sync_channel(1);
        self.sender.send(Command::Pause(tx)).unwrap();
        rx.recv().unwrap();
        Pause(self)
    }
    pub fn state(&self) -> IndexerState {
        self.state.lock().unwrap().clone()
    }
    /// Wait for one complete cycle; background work uses the same implementation.
    pub fn cycle(&self) -> Result<()> {
        let (tx, rx) = mpsc::sync_channel(1);
        self.sender
            .send(Command::Cycle(tx))
            .map_err(|_| anyhow::anyhow!("search indexer closed"))?;
        rx.recv()?
    }
    /// Build beside the published commit; searches continue on its reader until publication.
    pub fn rebuild(&self) -> Result<()> {
        self.rebuild_with_progress(|_| {})
    }
    /// Reports source progress without making query callers wait on a retained SQL reader.
    pub fn rebuild_with_progress(
        &self,
        progress: impl FnMut(vault_search::RebuildProgress) + Send + 'static,
    ) -> Result<()> {
        let cancel = CancelToken::new();
        *self.rebuild_cancel.lock().unwrap() = cancel.clone();
        let (tx, rx) = mpsc::sync_channel(1);
        self.sender
            .send(Command::Rebuild(tx, Box::new(progress), cancel))
            .map_err(|_| anyhow::anyhow!("search indexer closed"))?;
        rx.recv()?
    }
    pub fn cancel_rebuild(&self) {
        self.rebuild_cancel.lock().unwrap().cancel();
    }
    pub(super) fn track_session(
        &self,
        slot: std::sync::Weak<Mutex<Option<super::session::RunningSession>>>,
        cancel: CancelToken,
    ) -> Result<()> {
        let mut sessions = self.sessions.lock().unwrap();
        self.stop.check()?;
        sessions.retain(|session| session.slot.strong_count() > 0);
        sessions.push(super::session::RetainedSession { slot, cancel });
        Ok(())
    }
    pub(crate) fn close(&self) {
        self.stop.cancel();
        let sessions = std::mem::take(&mut *self.sessions.lock().unwrap());
        for session in &sessions {
            session.cancel.cancel();
        }
        for session in sessions {
            if let Some(slot) = session.slot.upgrade() {
                if let Ok(mut active) = slot.try_lock() {
                    active.take();
                }
            }
        }
        self.rebuild_cancel.lock().unwrap().cancel();
        let _ = self.sender.send(Command::Stop);
        if let Some(thread) = self.thread.lock().unwrap().take() {
            let _ = thread.join();
        }
    }
}
impl Drop for Indexer {
    fn drop(&mut self) {
        self.close();
    }
}

struct Worker {
    generations: Generations,
    published: Arc<Mutex<Option<Arc<SearchIndex>>>>,
    store: Arc<Store>,
    index: Arc<SearchIndex>,
    state: Arc<Mutex<IndexerState>>,
    stop: CancelToken,
    cancel: Arc<Mutex<CancelToken>>,
}
impl Worker {
    fn fail(&self, error: &anyhow::Error) {
        let mut state = self.state.lock().unwrap();
        state.phase = if error
            .downcast_ref::<vault_search::Error>()
            .is_some_and(|error| matches!(error, vault_search::Error::Cancelled))
            || error
                .downcast_ref::<vault_store::Error>()
                .is_some_and(|error| matches!(error, vault_store::Error::Cancelled { .. }))
        {
            IndexerPhase::Cancelled
        } else {
            IndexerPhase::Failed
        };
        state.error = Some(format!("{error:#}"));
    }
    fn repair(&mut self) -> Result<()> {
        *self.cancel.lock().unwrap() = CancelToken::new();
        let (name, index) = self.generations.create()?;
        self.generations.publish(name, &self.index)?;
        self.index = Arc::new(index);
        *self.published.lock().unwrap() = Some(self.index.clone());
        self.rebuild(true, &mut |_| {})
    }
    fn cycle(&mut self) -> Result<()> {
        self.generations.collect()?;
        let target = self.store.read(Db::Archive, documents::watermark)?;
        self.state.lock().unwrap().target_seq = target;
        let mut after = self.index.last_applied_seq()?;
        if after > target {
            return self.rebuild(false, &mut |_| {});
        }
        while after < target && self.stop.check().is_ok() {
            self.state.lock().unwrap().phase = IndexerPhase::Updating;
            let Some(batch) = self.store.read(Db::Archive, move |s| documents::batch(s, after))?
            else {
                return self.rebuild(false, &mut |_| {});
            };
            let last = batch.last_seq;
            let documents = batch.upserts.len() as u64;
            self.index.apply(batch)?;
            self.acknowledge(last)?;
            after = last;
            let mut state = self.state.lock().unwrap();
            state.applied_seq = last;
            state.documents_processed += documents;
            state.documents_total = state.documents_total.max(state.documents_processed);
        }
        // Includes a crash after publication but before pruning.
        self.acknowledge(after)?;
        let mut state = self.state.lock().unwrap();
        state.applied_seq = after;
        state.phase = if after >= target { IndexerPhase::Ready } else { IndexerPhase::Updating };
        state.error = None;
        Ok(())
    }
    fn acknowledge(&self, seq: u64) -> Result<()> {
        self.store.write(Db::Archive, Priority::Batch, move |s| {
            s.execute("DELETE FROM search_outbox WHERE seq<=?1", [seq as i64])?;
            Ok(())
        })?;
        Ok(())
    }
    fn rebuild(
        &mut self,
        initial: bool,
        report: &mut dyn FnMut(vault_search::RebuildProgress),
    ) -> Result<()> {
        self.state.lock().unwrap().phase =
            if initial { IndexerPhase::Building } else { IndexerPhase::Rebuilding };
        let (seq,total,through)=self.store.read(Db::Archive,|s| {
            let seq=documents::watermark(s)?;
            let (total,through)=s.query_one("SELECT COUNT(CASE WHEN newest_visit_ms IS NOT NULL THEN 1 END),COALESCE(MAX(page_id),0) FROM search_documents",(),|r|Ok((r.get::<i64>(0)? as u64,r.get::<i64>(1)?)))?;
            Ok((seq,total,through))
        })?;
        {
            let mut state = self.state.lock().unwrap();
            state.documents_total = total;
            state.documents_processed = 0;
            state.target_seq = seq;
        }
        let cancel = self.cancel.lock().unwrap().clone();
        let source =
            documents::Source::new(self.store.clone(), cancel.clone(), self.stop.clone(), through);
        if initial {
            let state = self.state.clone();
            self.index.build_initial(
                source,
                seq,
                |progress| {
                    state.lock().unwrap().documents_processed = progress.documents;
                    if !progress.complete {
                        report(progress)
                    }
                },
                || cancel.check().is_err() || self.stop.check().is_err(),
            )?;
            self.acknowledge(seq)?;
        } else {
            self.index.release_writer();
            let (name, new_index) = self.generations.create()?;
            let state = self.state.clone();
            let result = new_index.rebuild(
                source,
                seq,
                |progress| {
                    state.lock().unwrap().documents_processed = progress.documents;
                    if !progress.complete {
                        report(progress)
                    }
                },
                || cancel.check().is_err() || self.stop.check().is_err(),
            );
            if let Err(error) = result {
                drop(new_index);
                self.generations.discard(&name)?;
                return Err(error.into());
            }
            // Short source reads may observe later commits. Replay their complete outbox
            // prefix before publication, keeping all events durable until the pointer swaps.
            let target = self.store.read(Db::Archive, documents::watermark)?;
            let mut applied = seq;
            while applied < target {
                cancel.check()?;
                self.stop.check()?;
                let Some(batch) =
                    self.store.read(Db::Archive, move |s| documents::batch(s, applied))?
                else {
                    anyhow::bail!("search rebuild lost its canonical outbox prefix")
                };
                applied = batch.last_seq;
                new_index.apply(batch)?;
            }
            cancel.check()?;
            self.stop.check()?;
            self.generations.publish(name, &self.index)?;
            self.index = Arc::new(new_index);
            *self.published.lock().unwrap() = Some(self.index.clone());
            self.acknowledge(applied)?;
        }
        self.generations.collect()?;
        self.cycle()?;
        let documents = self.index.num_documents();
        {
            let mut state = self.state.lock().unwrap();
            state.documents_processed = documents;
            state.documents_total = documents;
        }
        report(vault_search::RebuildProgress { documents, complete: true });
        Ok(())
    }
}

#[cfg(test)]
pub(crate) struct Pause<'a>(&'a Indexer);
#[cfg(test)]
impl Drop for Pause<'_> {
    fn drop(&mut self) {
        let _ = self.0.sender.send(Command::Resume);
    }
}
