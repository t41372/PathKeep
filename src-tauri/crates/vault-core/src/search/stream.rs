//! Responsibilities: own streaming searches, continuations and cancellation until Vault close.
//! Not responsible for: transport encoding or collecting an archive-sized result list.
//! Failure modes: start blocks on SQL, page requests overlap, abandoned jobs retain readers,
//! cancel waits behind search, close leaves idle sessions alive, failed sinks leak jobs.
//! Tests: the History E2E specs search through this streaming path.
use super::{IndexerState, SearchRequest, SearchSession, SearchStep};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex, mpsc},
    thread::JoinHandle,
    time::Duration,
};
use vault_store::CancelToken;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchStarted {
    pub session_id: String,
    pub indexer: IndexerState,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", content = "data", rename_all = "camelCase")]
pub enum SearchEvent {
    Indexer(IndexerState),
    Step(Box<SearchStep>),
    Error(String),
    Cancelled,
}

type Sink = Box<dyn FnMut(SearchEvent) -> bool + Send>;
enum Command {
    Next(String, Option<Sink>),
    Stop,
}
struct Job {
    request: String,
    cancel: CancelToken,
    commands: mpsc::Sender<Command>,
    thread: JoinHandle<()>,
}
/// One registry belongs to all clones of a Vault; App Lock leaves it intact.
#[derive(Default)]
pub struct SearchSessions {
    jobs: Mutex<HashMap<String, Job>>,
    closed: std::sync::atomic::AtomicBool,
}
impl SearchSessions {
    pub fn start(
        &self,
        vault: &crate::Vault,
        request: SearchRequest,
        sink: impl FnMut(SearchEvent) -> bool + Send + 'static,
    ) -> Result<SearchStarted> {
        self.start_owned(vault.store().clone(), vault.search().clone(), request, Box::new(sink))
    }
    pub(super) fn start_owned(
        &self,
        store: Arc<vault_store::Store>,
        indexer: Arc<super::Indexer>,
        request: SearchRequest,
        sink: Sink,
    ) -> Result<SearchStarted> {
        self.spawn(store, indexer, request, None, sink)
    }
    pub(super) fn start_prepared(
        &self,
        store: Arc<vault_store::Store>,
        indexer: Arc<super::Indexer>,
        request: SearchRequest,
        session: SearchSession,
        sink: Sink,
    ) -> Result<SearchStarted> {
        self.spawn(store, indexer, request, Some(session), sink)
    }
    fn spawn(
        &self,
        store: Arc<vault_store::Store>,
        indexer: Arc<super::Indexer>,
        request: SearchRequest,
        prepared: Option<SearchSession>,
        mut sink: Sink,
    ) -> Result<SearchStarted> {
        let fingerprint = serde_json::to_string(&(&request, &request.ranking_key))?;
        let mut jobs = self.jobs.lock().unwrap();
        jobs.retain(|_, job| !job.thread.is_finished());
        anyhow::ensure!(
            !self.closed.load(std::sync::atomic::Ordering::Acquire),
            "search owner closed"
        );
        let mut bytes = [0u8; 16];
        getrandom::fill(&mut bytes)?;
        let session_id = hex::encode(bytes);
        let state = indexer
            .state_against(store.read(vault_store::Db::Archive, super::documents::watermark)?);
        let cancel =
            prepared.as_ref().map(SearchSession::cancel_token).transpose()?.unwrap_or_default();
        let token = cancel.clone();
        let (commands, rx) = mpsc::channel();
        let started_state = state.clone();
        let thread =
            std::thread::Builder::new().name("search-session".into()).spawn(move || {
                if !sink(SearchEvent::Indexer(started_state)) {
                    return;
                }
                let result = (|| -> Result<()> {
                    let mut session = match prepared {
                        Some(session) => session,
                        None => {
                            SearchSession::start_owned(&store, &indexer, request, token.clone())?
                        }
                    };
                    loop {
                        token.check()?;
                        let step = session.step(Duration::from_millis(8))?;
                        let complete = !step.provisional && step.count.exact;
                        if !sink(SearchEvent::Step(Box::new(step))) {
                            break;
                        }
                        if complete {
                            match rx.recv() {
                                Ok(Command::Next(cursor, new_sink)) => {
                                    if let Some(new_sink) = new_sink {
                                        sink = new_sink;
                                    }
                                    session.next_page(&cursor)?;
                                }
                                _ => break,
                            }
                        }
                    }
                    Ok(())
                })();
                if token.check().is_err() {
                    sink(SearchEvent::Cancelled);
                } else if let Err(error) = result {
                    sink(SearchEvent::Error(format!("{error:#}")));
                }
            })?;
        jobs.insert(session_id.clone(), Job { request: fingerprint, cancel, commands, thread });
        Ok(SearchStarted { session_id, indexer: state })
    }
    pub fn next_page(&self, id: &str, cursor: String) -> Result<()> {
        self.jobs
            .lock()
            .unwrap()
            .get(id)
            .context("search session not found")?
            .commands
            .send(Command::Next(cursor, None))
            .map_err(|_| anyhow::anyhow!("search session closed"))?;
        Ok(())
    }
    pub(super) fn next_with_sink(
        &self,
        id: &str,
        cursor: String,
        sink: Sink,
        request: &SearchRequest,
    ) -> Result<()> {
        let fingerprint = serde_json::to_string(&(request, &request.ranking_key))?;
        let jobs = self.jobs.lock().unwrap();
        let job = jobs.get(id).context("search session not found")?;
        anyhow::ensure!(
            job.request == fingerprint,
            "this cursor belongs to a search with other arguments: send it with that search's query, filters, sort and limit, or leave the cursor out to start from the first page"
        );
        job.commands
            .send(Command::Next(cursor, Some(sink)))
            .map_err(|_| anyhow::anyhow!("search session closed"))?;
        Ok(())
    }
    pub fn cancel(&self, id: &str) -> Result<()> {
        let job = self.jobs.lock().unwrap().remove(id).context("search session not found")?;
        stop(job);
        Ok(())
    }
    pub(crate) fn close(&self) {
        self.closed.store(true, std::sync::atomic::Ordering::Release);
        let jobs = std::mem::take(&mut *self.jobs.lock().unwrap());
        for (_, job) in jobs {
            stop(job);
        }
    }
}
fn stop(job: Job) {
    job.cancel.cancel();
    let _ = job.commands.send(Command::Stop);
    let _ = job.thread.join();
}
impl Drop for SearchSessions {
    fn drop(&mut self) {
        self.close();
    }
}
