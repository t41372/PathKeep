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
    decoders: usize,
    queue: std::collections::VecDeque<Arc<CancelToken>>,
    gc_requested: bool,
    gc_owner: std::sync::Weak<Archive>,

    scanning: bool,
    // Backup and reparse share one producer slot.
    capture: Option<CancelToken>,
    readers: Vec<Arc<CancelToken>>,
}

pub(crate) struct Jobs {
    store: Arc<vault_store::Store>,
    epochs: Mutex<std::collections::BTreeMap<i64, usize>>,
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

/// Two decoder workers share the archive. One background scan leaves a slot for
/// an interactive read. The outer operation keeps its permit through row assembly and
/// delivery. Nested evidence and page reads use the same worker.
const DECODER_SLOTS: usize = 2;
impl Jobs {
    pub fn new(store: Arc<vault_store::Store>) -> Self {
        Self {
            store,
            epochs: Default::default(),
            state: Default::default(),
            idle: Default::default(),
        }
    }
    pub(crate) fn request_collection(self: &Arc<Self>) {
        let work = {
            let mut state = self.state.lock().unwrap();
            state.gc_requested = true;
            self.take_idle_job(&mut state)
        };
        Self::run_idle_job(work);
    }
    fn take_idle_job(self: &Arc<Self>, state: &mut State) -> Option<(Arc<Archive>, Job)> {
        if state.active != 0 || state.pauses != 0 || state.closed || !state.gc_requested {
            return None;
        }
        let archive = state.gc_owner.upgrade()?;
        state.gc_requested = false;
        state.active += 1;
        Some((archive, Job { jobs: self.clone(), capture: false, reader: None }))
    }
    fn run_idle_job(work: Option<(Arc<Archive>, Job)>) {
        let Some((archive, lease)) = work else { return };
        let started =
            std::thread::Builder::new().name("conversation-reader-gc".into()).spawn(move || {
                let result = archive.collect_reader_garbage(&CancelToken::new());
                match result {
                    Err(crate::Error::Cancelled) => archive.jobs.request_collection(),
                    Err(error) => eprintln!("Conversation reader cleanup: {error}"),
                    Ok(()) => {}
                }
                drop(lease)
            });
        if let Err(error) = started {
            eprintln!("Starting conversation reader cleanup: {error}");
        }
    }
    fn retirement_epoch(&self) -> Result<i64> {
        Ok(self.store.read(vault_store::Db::AgentHistoryReadModel,|tx|tx.query_one("SELECT coalesce((SELECT seq FROM sqlite_sequence WHERE name='ah_reader_retired'),0)",(),|r|Ok(r.get(0)?)))?)
    }
    pub(crate) fn collection_cutoff(&self) -> Result<i64> {
        let pins = self.epochs.lock().unwrap();
        match pins.first_key_value() {
            Some((&epoch, _)) => Ok(epoch),
            None => self.retirement_epoch(),
        }
    }
    pub(crate) fn enter_window(self: &Arc<Self>, cancel: &CancelToken) -> Result<DecoderPermit> {
        self.decoder(cancel, false)
    }
    pub(crate) fn pin_publication(&self) -> Result<PublicationPin<'_>> {
        let mut pins = self.epochs.lock().unwrap();
        let epoch = self.retirement_epoch()?;
        *pins.entry(epoch).or_default() += 1;
        Ok(PublicationPin { jobs: self, epoch })
    }
    pub(crate) fn enter_scan(self: &Arc<Self>, cancel: &CancelToken) -> Result<DecoderPermit> {
        self.decoder(cancel, true)
    }
    fn decoder(self: &Arc<Self>, cancel: &CancelToken, scan: bool) -> Result<DecoderPermit> {
        let mut state = self.state.lock().unwrap();
        state.check_admission()?;
        let token = Arc::new(cancel.clone());
        state.queue.push_back(token.clone());
        state.readers.push(token.clone());
        state.active += 1;
        // Lay the job guard below the mutex guard so an error releases the lock before Drop.
        drop(state);
        let job = Job { jobs: self.clone(), capture: false, reader: Some(token.clone()) };
        let mut state = self.state.lock().unwrap();
        loop {
            state.check_admission()?;
            cancel.check().map_err(|_| crate::Error::Cancelled)?;
            if state.queue.front().is_some_and(|front| Arc::ptr_eq(front, &token))
                && state.decoders < DECODER_SLOTS
                && !(scan && state.scanning)
            {
                state.queue.pop_front();
                self.idle.notify_all();
                state.decoders += 1;
                state.scanning |= scan;
                return Ok(DecoderPermit { job, scan });
            }
            // External tokens are atomic and have no condition-variable notification.
            state = self.idle.wait_timeout(state, std::time::Duration::from_millis(10)).unwrap().0;
        }
    }

    /// Keep a cancellable publication alive across its ready probe and pointer switch.
    pub(crate) fn track(self: &Arc<Self>, cancel: &CancelToken) -> Result<Job> {
        let mut state = self.state.lock().unwrap();
        state.check_admission()?;
        cancel.check().map_err(|_| crate::Error::Cancelled)?;
        let token = Arc::new(cancel.clone());
        state.readers.push(token.clone());
        state.active += 1;
        Ok(Job { jobs: self.clone(), capture: false, reader: Some(token) })
    }
    pub(crate) fn enter(self: &Arc<Self>, capture: Option<&CancelToken>) -> Result<Job> {
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
        Ok(Job { jobs: self.clone(), capture: capture.is_some(), reader: None })
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
        self.idle.notify_all();
        drop(self.idle.wait_while(state, |state| state.active > 0).unwrap());
    }
}

pub(crate) struct PublicationPin<'a> {
    jobs: &'a Jobs,
    epoch: i64,
}
impl Drop for PublicationPin<'_> {
    fn drop(&mut self) {
        let mut pins = self.jobs.epochs.lock().unwrap();
        let count = pins.get_mut(&self.epoch).expect("registered publication epoch");
        *count -= 1;
        if *count == 0 {
            pins.remove(&self.epoch);
        }
    }
}

pub(crate) struct DecoderPermit {
    job: Job,
    scan: bool,
}
impl Drop for DecoderPermit {
    fn drop(&mut self) {
        let mut state = self.job.jobs.state.lock().unwrap();
        state.decoders -= 1;
        if self.scan {
            state.scanning = false;
        }
        self.job.jobs.idle.notify_all();
    }
}

pub(crate) struct Job {
    jobs: Arc<Jobs>,
    capture: bool,
    reader: Option<Arc<CancelToken>>,
}

impl Drop for Job {
    fn drop(&mut self) {
        let mut state = self.jobs.state.lock().unwrap();
        state.active -= 1;
        if let Some(reader) = &self.reader {
            state.queue.retain(|other| !Arc::ptr_eq(other, reader));
            self.jobs.idle.notify_all();
            state.readers.retain(|other| !Arc::ptr_eq(other, reader));
        }
        if self.capture {
            state.capture = None;
        }
        if state.active == 0 {
            self.jobs.idle.notify_all();
            let work = self.jobs.take_idle_job(&mut state);
            drop(state);
            Jobs::run_idle_job(work);
        }
    }
}

/// Holds the committed capture boundary across a copy or a generation replacement.
pub struct ArchivePause(Arc<Jobs>);

impl Drop for ArchivePause {
    fn drop(&mut self) {
        let mut state = self.0.state.lock().unwrap();
        state.pauses -= 1;
        let work = self.0.take_idle_job(&mut state);
        drop(state);
        Jobs::run_idle_job(work);
    }
}

impl Archive {
    /// Attach low-priority reader reclamation to this owner's generation and maintenance barrier.
    pub fn enable_idle_reader_collection(self: &Arc<Self>) {
        self.jobs.state.lock().unwrap().gc_owner = Arc::downgrade(self);
    }
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

#[cfg(test)]
mod admission_tests {
    use super::*;
    use std::{
        sync::mpsc,
        time::{Duration, Instant},
    };
    #[test]
    fn queued_scan_is_not_overtaken_by_a_new_window() {
        let t = tempfile::tempdir().unwrap();
        let a = Archive::create_unencrypted(t.path()).unwrap();
        let busy = a.jobs.enter_scan(&CancelToken::new()).unwrap();
        std::thread::scope(|s| {
            s.spawn(|| drop(a.jobs.enter_scan(&CancelToken::new()).unwrap()));
            let deadline = Instant::now() + Duration::from_secs(2);
            while a.jobs.state.lock().unwrap().readers.len() < 2 {
                assert!(Instant::now() < deadline);
                std::thread::yield_now();
            }
            let (sent, received) = mpsc::channel();
            let jobs = a.jobs.clone();
            s.spawn(move || {
                let _permit = jobs.enter_window(&CancelToken::new()).unwrap();
                sent.send(()).unwrap();
            });
            let early = received.recv_timeout(Duration::from_millis(100)).is_ok();
            drop(busy);
            if !early {
                received.recv_timeout(Duration::from_secs(2)).unwrap();
            }
            assert!(!early, "window bypassed the queued scan");
        });
    }
}
