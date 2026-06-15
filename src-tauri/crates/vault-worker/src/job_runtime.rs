//! Shared worker-pool and cooperative-stop helpers for background jobs.
//!
//! The worker crate owns the runtime glue around durable queue rows. This
//! module keeps two policies reusable across AI and intelligence queues:
//!
//! - configured concurrency must translate into real parallel workers
//! - long-running jobs must observe stop requests at safe checkpoints

use anyhow::Result;
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
        mpsc,
    },
    thread,
    time::{Duration, Instant},
};
use vault_core::{AiRunCancelled, AiRunControl};

/// Spawns workers until the active count reaches the requested pool size.
pub(crate) fn maybe_spawn_worker_pool<F>(
    base_name: &str,
    active_workers: &'static AtomicUsize,
    desired_workers: usize,
    worker: F,
) where
    F: Fn() + Send + Sync + 'static,
{
    let desired_workers = desired_workers.max(1);
    let worker = Arc::new(worker);

    loop {
        let active = active_workers.load(Ordering::Acquire);
        if active >= desired_workers {
            break;
        }
        if claim_worker_slot(active_workers, active) {
            let worker = Arc::clone(&worker);
            let thread_name = format!("{base_name}-{}", active + 1);
            // The guard releases the claimed slot on EVERY exit path: normal
            // return, a panic inside `worker()` (unwind drops the guard), and
            // even spawn failure (the OS refusing the thread drops the closure,
            // and with it the guard). The previous `worker(); fetch_sub(...)`
            // tail leaked the slot on panic / spawn error, and once the leaked
            // count reached `desired_workers` the pool never spawned again —
            // wedging the AI / intelligence queue drain until process restart.
            let guard = WorkerSlotGuard(active_workers);
            let _ = thread::Builder::new().name(thread_name).spawn(move || {
                let _slot = guard;
                worker();
            });
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
        Self { cancelled, stop_tx }
    }

    pub(crate) fn shutdown(&self) {
        let _ = self.stop_tx.send(());
    }
}

impl AiRunControl for BackgroundJobControl {
    fn checkpoint(&self, detail: &str) -> Result<()> {
        if self.cancelled.load(Ordering::Acquire) {
            return Err(AiRunCancelled::new(detail).into());
        }
        Ok(())
    }

    fn cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Barrier;

    #[test]
    fn worker_pool_spawns_real_parallel_workers() {
        static ACTIVE_WORKERS: AtomicUsize = AtomicUsize::new(0);

        ACTIVE_WORKERS.store(0, Ordering::Release);
        let concurrent = Arc::new(AtomicUsize::new(0));
        let peak = Arc::new(AtomicUsize::new(0));
        let barrier = Arc::new(Barrier::new(3));

        maybe_spawn_worker_pool("pathkeep-test-pool", &ACTIVE_WORKERS, 2, {
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
            if ACTIVE_WORKERS.load(Ordering::Acquire) == 0 {
                break;
            }
            thread::sleep(Duration::from_millis(10));
        }

        assert_eq!(peak.load(Ordering::Acquire), 2);
        assert_eq!(ACTIVE_WORKERS.load(Ordering::Acquire), 0);

        let claimed = AtomicUsize::new(0);
        assert!(claim_worker_slot(&claimed, 0));
        assert!(!claim_worker_slot(&claimed, 0));
    }

    #[test]
    fn worker_pool_releases_slot_when_worker_panics() {
        static ACTIVE_WORKERS: AtomicUsize = AtomicUsize::new(0);
        ACTIVE_WORKERS.store(0, Ordering::Release);
        let release = Arc::new(Barrier::new(2));

        maybe_spawn_worker_pool("pathkeep-panic-pool", &ACTIVE_WORKERS, 1, {
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
            if ACTIVE_WORKERS.load(Ordering::Acquire) == 0 {
                break;
            }
            thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(
            ACTIVE_WORKERS.load(Ordering::Acquire),
            0,
            "a panicking worker must still release its pool slot"
        );
    }

    #[test]
    fn background_job_control_reports_requested_stop_at_checkpoints() {
        let control = BackgroundJobControl::spawn(
            Duration::from_millis(5),
            Duration::from_secs(60),
            || Ok(()),
            || Ok(true),
        );

        for _ in 0..100 {
            if control.cancelled() {
                break;
            }
            thread::sleep(Duration::from_millis(5));
        }

        assert!(control.cancelled());
        let cancelled = control
            .checkpoint("background job noticed a stop request")
            .expect_err("checkpoint should report cancellation");
        assert!(cancelled.to_string().contains("stop request"));
        control.shutdown();
    }
}
