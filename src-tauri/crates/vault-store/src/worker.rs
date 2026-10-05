//! Responsibilities: start, service and join connection-owning reader/writer threads.
//! Not responsible for: application work or async APIs outside this crate.
//! Failure modes: connections cross threads; panic kills an owner; startup failure
//! leaks threads; interactive jobs wait behind queued batches. Tests: concurrency, opening.
use crate::{
    Db, Error, Metrics, Migration, Priority, Result, Session,
    checkpoint::Gate,
    engine::{Engine, Settings},
    queue::{Next, Queue},
};
use std::{
    cell::Cell,
    sync::{Arc, Mutex, mpsc},
    thread::{self, JoinHandle},
};

type Reply = Box<dyn FnOnce() + Send>;
type Job = Box<dyn FnOnce(&Engine, Option<&Gate>) -> Reply + Send>;
thread_local! {static IN_OWNER:Cell<bool>=const{Cell::new(false)};}
pub(crate) fn check_caller() -> Result<()> {
    if IN_OWNER.with(Cell::get) { Err(Error::Reentrant) } else { Ok(()) }
}

pub(crate) struct Workers {
    writer: Arc<Queue<Job>>,
    readers: Arc<Queue<Job>>,
    threads: Vec<JoinHandle<()>>,
    pub gate: Arc<Gate>,
    metrics: Arc<Mutex<Metrics>>,
}
impl Workers {
    pub fn start(
        db: Db,
        settings: Settings,
        steps: Vec<Migration>,
        reader_count: usize,
        capacity: usize,
        scratch_gate: Option<Arc<Gate>>,
    ) -> Result<Self> {
        let writer = Arc::new(Queue::<Job>::new(capacity));
        let readers = Arc::new(Queue::<Job>::new(capacity));
        let wake = Arc::downgrade(&writer);
        let gate = Arc::new(Gate::new(move || {
            if let Some(q) = wake.upgrade() {
                q.wake();
            }
        }));
        let metrics = Arc::new(Mutex::new(Metrics::default()));
        let mut owners = Self {
            writer: writer.clone(),
            readers: readers.clone(),
            threads: Vec::new(),
            gate: gate.clone(),
            metrics: metrics.clone(),
        };
        let (ready_tx, ready_rx) = mpsc::sync_channel(1);
        let write_settings = settings.clone();
        let write_scratch = scratch_gate.clone();
        owners.threads.push(thread::Builder::new().name(format!("store-{db:?}-writer")).spawn(
            move || {
                IN_OWNER.with(|v| v.set(true));
                let opened = crate::session::catch_job(|| {
                    let engine = Engine::open(&write_settings)?;
                    crate::migration::apply(
                        &engine.session(false).with_scratch(write_scratch.as_deref()),
                        db,
                        &steps,
                    )?;
                    Ok(engine)
                });
                let engine = match opened {
                    Ok(engine) => engine,
                    Err(error) => {
                        let _ = ready_tx.send(Err(error));
                        return;
                    }
                };
                if ready_tx.send(Ok(engine.database.clone())).is_err() {
                    return;
                }
                let mut dirty = true;
                loop {
                    let mut completion = None;
                    match writer.next() {
                        Next::Job(priority, job) => {
                            let reply = job(&engine, write_scratch.as_deref());
                            dirty = true;
                            if priority == Priority::Batch {
                                crate::checkpoint::run(&engine, false, &metrics);
                            }
                            completion = Some(reply);
                        }
                        Next::Maintenance => {}
                        Next::Closed => {
                            if let Some(_guard) = gate.try_truncate() {
                                crate::checkpoint::run(&engine, true, &metrics);
                            }
                            break;
                        }
                    }
                    if dirty && writer.pending() == (0, 0) {
                        let guard = gate.reserve_truncate();
                        // Acknowledgement belongs to COMMIT, not to maintenance.
                        // In particular, an active reader may depend on this reply.
                        if let Some(reply) = completion.take() {
                            reply();
                        }
                        guard.wait_for_readers();
                        dirty = !crate::checkpoint::run(&engine, true, &metrics);
                    }
                    if let Some(reply) = completion {
                        reply();
                    }
                }
            },
        )?);
        let database = ready_rx.recv().map_err(|_| Error::Closed)??;
        for index in 0..reader_count {
            let database = database.clone();
            let settings = settings.clone();
            let queue = readers.clone();
            let gate = owners.gate.clone();
            let scratch = scratch_gate.clone();
            let (ready_tx, ready_rx) = mpsc::sync_channel(1);
            owners.threads.push(
                thread::Builder::new().name(format!("store-{db:?}-reader-{index}")).spawn(
                    move || {
                        IN_OWNER.with(|v| v.set(true));
                        let engine =
                            match crate::session::catch_job(|| Engine::reader(database, &settings))
                            {
                                Ok(e) => e,
                                Err(error) => {
                                    let _ = ready_tx.send(Err(error));
                                    return;
                                }
                            };
                        if ready_tx.send(Ok(())).is_err() {
                            return;
                        }
                        loop {
                            match queue.next() {
                                Next::Job(_, job) => {
                                    let guard = gate.enter();
                                    let reply = job(&engine, scratch.as_deref());
                                    drop(guard);
                                    reply();
                                }
                                Next::Maintenance => {}
                                Next::Closed => break,
                            }
                        }
                    },
                )?,
            );
            ready_rx.recv().map_err(|_| Error::Closed)??;
        }
        Ok(owners)
    }
    pub fn request<T: Send + 'static>(
        &self,
        read: bool,
        priority: Priority,
        work: impl FnOnce(&Session<'_>) -> Result<T> + Send + 'static,
    ) -> Result<T> {
        check_caller()?;
        let (tx, rx) = mpsc::sync_channel(1);
        let job: Job = Box::new(move |engine, scratch| {
            let result =
                crate::session::transaction(&engine.session(read).with_scratch(scratch), work);
            Box::new(move || {
                let _ = tx.send(result);
            })
        });
        if read {
            self.readers.push(Priority::Interactive, job)?
        } else {
            self.writer.push(priority, job)?
        }
        rx.recv().map_err(|_| Error::Closed)?
    }
    pub fn metrics(&self) -> Metrics {
        let mut m = self.metrics.lock().unwrap().clone();
        (m.interactive_pending, m.batch_pending) = self.writer.pending();
        m
    }
}
impl Drop for Workers {
    fn drop(&mut self) {
        self.readers.close();
        self.writer.close();
        // Reader shutdown precedes the final writer checkpoint. The writer may
        // already have exited; all handles are joined before Store drops its lock.
        while let Some(thread) = self.threads.pop() {
            let _ = thread.join();
        }
    }
}
