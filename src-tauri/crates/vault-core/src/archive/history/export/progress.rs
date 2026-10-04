//! Pollable export jobs: bounded process-local state, cancellation and terminal outcomes.
//!
//! Responsibilities: isolate callers' jobs and retain a short-lived result for polling.
//! Not responsible for archive reads, rendering, disk publication or transport wiring.

// Failure modes (recorded before implementation):
// (a) cancellation publishes a partial destination; publication must follow a cancellation barrier.
// (b) a large SQL/write batch ignores cancellation; check SQL opcodes, rows and write chunks.
// (c) errors strand progress in running/finishing; every exit must record a terminal outcome.
// (d) different ids share counters/stop flags or filenames; every job owns its state and artifact.
// (e) output changes; retain the renderer and compare against the previous page walk.
// (f) memory follows archive size; use fixed buffers and cap both retained and active jobs.

use crate::{
    models::{ExportProgress, ExportState},
    utils::now_rfc3339,
};
use anyhow::{Result, bail};
use std::{
    collections::HashMap,
    sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

const RETENTION: Duration = Duration::from_secs(5 * 60);
const MAX_JOBS: usize = 64;

/// Identifies an accepted cancellation through anyhow context and the command error envelope.
#[derive(Debug, thiserror::Error)]
#[error("export-cancelled")]
pub struct ExportCancelled;

struct JobData {
    progress: ExportProgress,
    finished: Option<Instant>,
}

pub(super) struct ExportJob {
    data: Mutex<JobData>,
    cancelled: AtomicBool,
}

type Registry = HashMap<String, Arc<ExportJob>>;
static JOBS: OnceLock<Mutex<Registry>> = OnceLock::new();

fn registry() -> &'static Mutex<Registry> {
    JOBS.get_or_init(Mutex::default)
}

fn prune(jobs: &mut Registry) {
    jobs.retain(|_, job| {
        job.data.lock().unwrap().finished.is_none_or(|at| at.elapsed() < RETENTION)
    });
}

/// Lets either IPC transport poll without opening an archive or waiting for the export thread.
pub fn get_export_progress(export_id: &str) -> Option<ExportProgress> {
    let mut jobs = registry().lock().unwrap();
    prune(&mut jobs);
    jobs.get(export_id).map(|job| job.data.lock().unwrap().progress.clone())
}

/// Accepts a stop only while writing; finishing is the non-cancellable durable publication phase.
pub fn cancel_export(export_id: &str) -> bool {
    let mut jobs = registry().lock().unwrap();
    prune(&mut jobs);
    let Some(job) = jobs.get(export_id) else {
        return false;
    };
    let data = job.data.lock().unwrap();
    if data.progress.state != ExportState::Running || job.cancelled.load(Ordering::Relaxed) {
        return false;
    }
    job.cancelled.store(true, Ordering::Relaxed);
    true
}

impl ExportJob {
    pub(super) fn start(export_id: Option<&str>) -> Result<Option<Arc<Self>>> {
        let Some(export_id) = export_id else {
            return Ok(None);
        };
        let mut jobs = registry().lock().unwrap();
        Self::start_in(&mut jobs, export_id).map(Some)
    }

    fn start_in(jobs: &mut Registry, export_id: &str) -> Result<Arc<Self>> {
        prune(jobs);
        if jobs.get(export_id).is_some_and(|job| job.data.lock().unwrap().finished.is_none()) {
            bail!("export id is already running");
        }
        jobs.remove(export_id);
        if jobs.len() >= MAX_JOBS {
            let oldest = jobs
                .iter()
                .filter_map(|(id, job)| {
                    job.data.lock().unwrap().finished.map(|at| (id.clone(), at))
                })
                .min_by_key(|(_, at)| *at);
            if let Some((id, _)) = oldest {
                jobs.remove(&id);
            } else {
                bail!("too many running exports");
            }
        }
        let job = Arc::new(Self {
            cancelled: AtomicBool::new(false),
            data: Mutex::new(JobData {
                progress: ExportProgress {
                    export_id: export_id.to_owned(),
                    state: ExportState::Running,
                    rows_written: 0,
                    total_rows: None,
                    bytes_written: 0,
                    started_at: now_rfc3339(),
                    error: None,
                },
                finished: None,
            }),
        });
        jobs.insert(export_id.to_owned(), job.clone());
        Ok(job)
    }

    pub(super) fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Relaxed)
    }

    pub(super) fn check(&self) -> Result<()> {
        if self.is_cancelled() {
            return Err(ExportCancelled.into());
        }
        Ok(())
    }

    pub(super) fn total(&self, total: Option<u64>) {
        self.data.lock().unwrap().progress.total_rows = total;
    }

    pub(super) fn bytes(&self, bytes: u64) {
        self.data.lock().unwrap().progress.bytes_written = bytes;
    }

    pub(super) fn update(&self, rows: u64, bytes: u64) {
        let mut data = self.data.lock().unwrap();
        data.progress.rows_written = rows;
        data.progress.bytes_written = bytes;
    }

    pub(super) fn finishing(&self) -> Result<()> {
        // Shares the cancel lock: a true cancellation can never race past this barrier.
        let mut data = self.data.lock().unwrap();
        self.check()?;
        data.progress.state = ExportState::Finishing;
        Ok(())
    }

    pub(super) fn complete<T>(&self, result: &Result<T>) {
        let mut data = self.data.lock().unwrap();
        data.progress.state = match result {
            Ok(_) => ExportState::Done,
            Err(error) if error.downcast_ref::<ExportCancelled>().is_some() => {
                ExportState::Cancelled
            }
            Err(_) => ExportState::Failed,
        };
        data.progress.error = result.as_ref().err().map(|error| format!("{error:#}"));
        data.finished = Some(Instant::now());
    }
}

/// Includes worker preflight failures (config/lock/path errors) in the same polling contract.
pub fn record_export_failure(export_id: Option<&str>, error: &anyhow::Error) {
    if let Ok(Some(job)) = ExportJob::start(export_id) {
        job.complete::<()>(&Err(anyhow::anyhow!("{error:#}")));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_is_bounded_expires_terminal_jobs_and_allows_id_reuse() {
        let mut jobs = Registry::new();
        for index in 0..MAX_JOBS {
            ExportJob::start_in(&mut jobs, &format!("bounded-{index}")).unwrap();
        }
        assert!(ExportJob::start_in(&mut jobs, "too-many").is_err());
        assert!(ExportJob::start_in(&mut jobs, "bounded-0").is_err());
        jobs["bounded-0"].complete(&Ok(()));
        ExportJob::start_in(&mut jobs, "replacement").unwrap();
        assert_eq!(jobs.len(), MAX_JOBS);
        assert!(!jobs.contains_key("bounded-0"));
        jobs.clear();
        let job = ExportJob::start(Some("registry-expiry")).unwrap().unwrap();
        job.complete(&Ok(()));
        job.data.lock().unwrap().finished = Some(Instant::now() - RETENTION);
        jobs.insert("expired".into(), job);
        prune(&mut jobs);
        assert!(jobs.is_empty());
        let previous = ExportJob::start(Some("registry-reuse")).unwrap().unwrap();
        previous.complete(&Ok(()));
        let replacement = ExportJob::start(Some("registry-reuse")).unwrap().unwrap();
        assert!(!Arc::ptr_eq(&previous, &replacement));
        replacement.complete(&Ok(()));
    }
}

#[cfg(test)]
mod barrier_tests {
    use super::*;

    #[test]
    fn accepted_cancel_cannot_cross_finishing_and_publication_errors_become_failed() {
        let cancelled = ExportJob::start(Some("barrier-cancelled")).unwrap().unwrap();
        assert!(cancel_export("barrier-cancelled"));
        assert!(cancelled.finishing().unwrap_err().downcast_ref::<ExportCancelled>().is_some());
        cancelled.complete::<()>(&Err(ExportCancelled.into()));
        let finishing = ExportJob::start(Some("barrier-failed")).unwrap().unwrap();
        finishing.finishing().unwrap();
        assert_eq!(get_export_progress("barrier-failed").unwrap().state, ExportState::Finishing);
        assert!(!cancel_export("barrier-failed"));
        finishing.complete::<()>(&Err(anyhow::anyhow!("flush failed")));
        assert_eq!(get_export_progress("barrier-failed").unwrap().state, ExportState::Failed);
    }
}
