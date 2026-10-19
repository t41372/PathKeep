//! Shared worker-pool and cooperative-stop helpers for background jobs.
//!
//! The worker crate owns the runtime glue around durable queue rows. This
//! module keeps two policies reusable across AI and intelligence queues:
//!
//! - configured concurrency must translate into real parallel workers
//! - long-running jobs must observe stop requests at safe checkpoints
//! - "Delete all data" stops every worker and job already started, even while one waits between
//!   jobs or its database is already closed (`stop_all_background_work`)

use anyhow::Result;
use std::{
    cell::Cell,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
        mpsc,
    },
    thread,
    time::{Duration, Instant},
};
use vault_core::{AiRunCancelled, AiRunControl};

/// One queue's workers: how many run, and how many times work was asked for.
pub(crate) struct WorkerPool {
    active: AtomicUsize,
    wakes: AtomicUsize,
}

impl WorkerPool {
    pub(crate) const fn new() -> Self {
        Self { active: AtomicUsize::new(0), wakes: AtomicUsize::new(0) }
    }

    pub(crate) fn active(&self) -> usize {
        self.active.load(Ordering::Acquire)
    }
}

/// Moves on when everything running has to stop. A stop request written to a job's queue row is
/// not enough for "Delete all data": a worker waiting for a retry reads no row, and once the
/// databases are closed a job cannot read its row and would carry on writing files.
static STOP_EPOCH: AtomicU64 = AtomicU64::new(0);

thread_local! {
    /// The epoch a worker thread started in; `None` outside the worker pools.
    static WORKER_EPOCH: Cell<Option<u64>> = const { Cell::new(None) };
}

/// Stops every worker and job started before this call; ones started after it run normally.
pub(crate) fn stop_all_background_work() {
    STOP_EPOCH.fetch_add(1, Ordering::AcqRel);
}

/// Whether this worker thread was told to stop. Threads outside the pools never are.
pub(crate) fn worker_stopped() -> bool {
    WORKER_EPOCH.get().is_some_and(|epoch| epoch != STOP_EPOCH.load(Ordering::Acquire))
}

/// Waits between jobs, ending early when this worker is told to stop. False when it was.
pub(crate) fn wait_unless_stopped(duration: Duration) -> bool {
    let deadline = Instant::now() + duration;
    loop {
        if worker_stopped() {
            return false;
        }
        let left = deadline.saturating_duration_since(Instant::now());
        if left.is_zero() {
            return true;
        }
        thread::sleep(left.min(Duration::from_millis(100)));
    }
}

/// The run control of work on a pool thread that has no queue row: it stops with the epoch.
pub(crate) struct WorkerStopControl;

impl AiRunControl for WorkerStopControl {
    fn checkpoint(&self, detail: &str) -> Result<()> {
        if worker_stopped() {
            return Err(AiRunCancelled::new(detail).into());
        }
        Ok(())
    }

    fn cancelled(&self) -> bool {
        worker_stopped()
    }
}

/// Spawns workers until the active count reaches the requested pool size. Each worker drains
/// its queue until it finds nothing (see `run_worker` for what happens then).
pub(crate) fn maybe_spawn_worker_pool<F>(
    base_name: &str,
    pool: &'static WorkerPool,
    desired_workers: usize,
    worker: F,
) where
    F: Fn() + Send + Sync + 'static,
{
    // Counted before looking for a free slot, so a worker on its way out sees it.
    pool.wakes.fetch_add(1, Ordering::AcqRel);
    let desired_workers = desired_workers.max(1);
    let worker = Arc::new(worker);
    while let Some(active) = claim_free_slot(pool, desired_workers) {
        let worker = Arc::clone(&worker);
        let thread_name = format!("{base_name}-{}", active + 1);
        // The guard releases the claimed slot on EVERY exit path: normal
        // return, a panic inside `worker()` (unwind drops the guard), and
        // even spawn failure (the OS refusing the thread drops the closure,
        // and with it the guard). The previous `worker(); fetch_sub(...)`
        // tail leaked the slot on panic / spawn error, and once the leaked
        // count reached `desired_workers` the pool never spawned again —
        // wedging the AI / intelligence queue drain until process restart.
        let guard = WorkerSlotGuard(&pool.active);
        let _ = thread::Builder::new()
            .name(thread_name)
            .spawn(move || run_worker(pool, desired_workers, guard, &*worker));
    }
}

/// Runs `worker` until a pass ends with no one having asked for work during it. A job queued
/// while this worker was finding its queue empty found the pool full and started no worker; on a
/// real archive that left two Insights jobs queued for over an hour. So after each pass the
/// worker gives up its slot, and takes one back to look again if work was asked for meanwhile.
fn run_worker(
    pool: &'static WorkerPool,
    desired_workers: usize,
    mut slot: WorkerSlotGuard,
    worker: &dyn Fn(),
) {
    WORKER_EPOCH.set(Some(STOP_EPOCH.load(Ordering::Acquire)));
    loop {
        let asked = pool.wakes.load(Ordering::Acquire);
        if !worker_stopped() {
            worker();
        }
        drop(slot);
        if worker_stopped()
            || pool.wakes.load(Ordering::Acquire) == asked
            || claim_free_slot(pool, desired_workers).is_none()
        {
            return;
        }
        slot = WorkerSlotGuard(&pool.active);
    }
}

/// Takes a slot when fewer than `desired_workers` run, returning how many ran before.
fn claim_free_slot(pool: &WorkerPool, desired_workers: usize) -> Option<usize> {
    loop {
        let active = pool.active.load(Ordering::Acquire);
        if active >= desired_workers {
            return None;
        }
        if claim_worker_slot(&pool.active, active) {
            return Some(active);
        }
    }
}

/// Releases one worker-pool slot when dropped.
struct WorkerSlotGuard(&'static AtomicUsize);

impl Drop for WorkerSlotGuard {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::AcqRel);
    }
}

fn claim_worker_slot(active_workers: &AtomicUsize, active: usize) -> bool {
    active_workers.compare_exchange(active, active + 1, Ordering::AcqRel, Ordering::Acquire).is_ok()
}

/// Background cooperative-stop monitor for one long-running queue job.
#[derive(Clone)]
pub(crate) struct BackgroundJobControl {
    cancelled: Arc<AtomicBool>,
    /// The stop epoch the job started in: once its database is closed a job cannot read its stop
    /// request, and the epoch still stops it (`stop_all_background_work`).
    epoch: u64,
    stop_tx: mpsc::Sender<()>,
}

impl BackgroundJobControl {
    pub(crate) fn spawn<FHeartbeat, FStopRequested>(
        poll_interval: Duration,
        heartbeat_every: Duration,
        heartbeat: FHeartbeat,
        stop_requested: FStopRequested,
    ) -> Self
    where
        FHeartbeat: Fn() -> Result<()> + Send + 'static,
        FStopRequested: Fn() -> Result<bool> + Send + 'static,
    {
        let cancelled = Arc::new(AtomicBool::new(false));
        let epoch = STOP_EPOCH.load(Ordering::Acquire);
        let monitor_cancelled = Arc::clone(&cancelled);
        let (stop_tx, stop_rx) = mpsc::channel::<()>();
        thread::spawn(move || {
            let mut last_heartbeat = Instant::now();
            loop {
                match stop_rx.recv_timeout(poll_interval) {
                    Ok(_) | Err(mpsc::RecvTimeoutError::Disconnected) => break,
                    Err(mpsc::RecvTimeoutError::Timeout) => {
                        if let Ok(stop_requested) = stop_requested() {
                            monitor_cancelled.store(stop_requested, Ordering::Release);
                        }
                        if last_heartbeat.elapsed() >= heartbeat_every {
                            let _ = heartbeat();
                            last_heartbeat = Instant::now();
                        }
                    }
                }
            }
        });
        Self { cancelled, epoch, stop_tx }
    }

    pub(crate) fn shutdown(&self) {
        let _ = self.stop_tx.send(());
    }
}

impl AiRunControl for BackgroundJobControl {
    fn checkpoint(&self, detail: &str) -> Result<()> {
        if self.cancelled() {
            return Err(AiRunCancelled::new(detail).into());
        }
        Ok(())
    }

    fn cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire) || STOP_EPOCH.load(Ordering::Acquire) != self.epoch
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Barrier;

    #[test]
    fn worker_pool_spawns_real_parallel_workers() {
        static POOL: WorkerPool = WorkerPool::new();
        let concurrent = Arc::new(AtomicUsize::new(0));
        let peak = Arc::new(AtomicUsize::new(0));
        let barrier = Arc::new(Barrier::new(3));

        maybe_spawn_worker_pool("pathkeep-test-pool", &POOL, 2, {
            let concurrent = Arc::clone(&concurrent);
            let peak = Arc::clone(&peak);
            let barrier = Arc::clone(&barrier);
            move || {
                let now = concurrent.fetch_add(1, Ordering::AcqRel) + 1;
                peak.fetch_max(now, Ordering::AcqRel);
                barrier.wait();
                concurrent.fetch_sub(1, Ordering::AcqRel);
            }
        });

        barrier.wait();
        for _ in 0..100 {
            if POOL.active() == 0 {
                break;
            }
            thread::sleep(Duration::from_millis(10));
        }

        assert_eq!(peak.load(Ordering::Acquire), 2);
        assert_eq!(POOL.active(), 0);

        let claimed = AtomicUsize::new(0);
        assert!(claim_worker_slot(&claimed, 0));
        assert!(!claim_worker_slot(&claimed, 0));
    }

    #[test]
    fn worker_pool_releases_slot_when_worker_panics() {
        static POOL: WorkerPool = WorkerPool::new();
        let release = Arc::new(Barrier::new(2));

        maybe_spawn_worker_pool("pathkeep-panic-pool", &POOL, 1, {
            let release = Arc::clone(&release);
            move || {
                release.wait();
                panic!("worker boom");
            }
        });

        // Let the worker reach its panic, then confirm the slot was released so
        // the pool is not permanently wedged at its desired-worker ceiling.
        release.wait();
        for _ in 0..200 {
            if POOL.active() == 0 {
                break;
            }
            thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(POOL.active(), 0, "a panicking worker must still release its pool slot");
    }

    /// A job queued while the only worker is finding its queue empty: the request finds the pool
    /// full and starts no worker, so the leaving worker has to look again.
    #[test]
    fn work_asked_for_while_the_only_worker_is_leaving_is_still_done() {
        static POOL: WorkerPool = WorkerPool::new();
        let queued = Arc::new(AtomicUsize::new(0));
        let done = Arc::new(AtomicUsize::new(0));
        let found_empty = Arc::new(Barrier::new(2));
        let job_queued = Arc::new(Barrier::new(2));
        let first_look = Arc::new(AtomicBool::new(true));
        let drain = || {
            let (queued, done) = (Arc::clone(&queued), Arc::clone(&done));
            let (found_empty, job_queued) = (Arc::clone(&found_empty), Arc::clone(&job_queued));
            let first_look = Arc::clone(&first_look);
            move || {
                while queued.load(Ordering::Acquire) > 0 {
                    queued.fetch_sub(1, Ordering::AcqRel);
                    done.fetch_add(1, Ordering::AcqRel);
                }
                // The queue looked empty: the first time, a job is queued right now.
                if first_look.swap(false, Ordering::AcqRel) {
                    found_empty.wait();
                    job_queued.wait();
                }
            }
        };

        maybe_spawn_worker_pool("pathkeep-wake-pool", &POOL, 1, drain());
        found_empty.wait();
        queued.fetch_add(1, Ordering::AcqRel);
        maybe_spawn_worker_pool("pathkeep-wake-pool", &POOL, 1, drain());
        assert_eq!(POOL.active(), 1, "the request found the only slot taken");
        job_queued.wait();

        for _ in 0..200 {
            if done.load(Ordering::Acquire) == 1 && POOL.active() == 0 {
                break;
            }
            thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(done.load(Ordering::Acquire), 1, "the job queued meanwhile was run");
        assert_eq!(POOL.active(), 0);
    }
}
