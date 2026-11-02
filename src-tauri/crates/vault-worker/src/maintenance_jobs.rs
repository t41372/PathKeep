//! Responsibilities: share maintenance progress/cancellation across worker transports, and list
//! each operation as foreground work while it runs, so quitting asks first.
//! Not responsible for: database work, thread creation or frontend presentation.
//! Failure modes: a second operation overwrites active progress; cancellation waits
//! behind SQL; terminal failures look successful. Tests: maintenance_jobs::tests.
use crate::ForegroundWork;
use anyhow::Result;
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock},
};
use vault_core::maintenance::Progress;
use vault_store::CancelToken;

struct Job {
    cancel: CancelToken,
    progress: Progress,
    active: bool,
}
static JOBS: OnceLock<Mutex<HashMap<PathBuf, Job>>> = OnceLock::new();
fn jobs() -> &'static Mutex<HashMap<PathBuf, Job>> {
    JOBS.get_or_init(Default::default)
}

/// Runs one maintenance operation: `kind` is the phase its progress starts with, `foreground`
/// what the quit prompt calls it while it runs.
pub(crate) fn run<T>(
    root: &Path,
    kind: &str,
    foreground: ForegroundWork,
    work: impl FnOnce(CancelToken, Box<dyn FnMut(Progress) + Send>) -> Result<T>,
) -> Result<T> {
    let token = CancelToken::new();
    {
        let mut jobs = jobs().lock().unwrap();
        anyhow::ensure!(
            !jobs.get(root).is_some_and(|j| j.active),
            "archive-busy: maintenance is running"
        );
        jobs.insert(
            root.into(),
            Job {
                cancel: token.clone(),
                active: true,
                progress: Progress {
                    phase: kind.into(),
                    database: None,
                    table: None,
                    processed: 0,
                    total: 0,
                },
            },
        );
    }
    let _running = crate::foreground_work::begin(foreground);
    let path = root.to_owned();
    let sink = Box::new(move |progress| {
        if let Some(job) = jobs().lock().unwrap().get_mut(&path) {
            job.progress = progress;
        }
    });
    let result = work(token.clone(), sink);
    let mut jobs = jobs().lock().unwrap();
    let job = jobs.get_mut(root).unwrap();
    job.active = false;
    job.progress.phase = if result.is_ok() {
        "completed"
    } else if token.check().is_err() {
        "cancelled"
    } else {
        "failed"
    }
    .into();
    result
}

/// A small read model remains responsive while the database owner is paused.
pub fn maintenance_progress() -> Result<Option<Progress>> {
    let paths = vault_core::project_paths()?;
    crate::context::load_unlocked_config(&paths)?;
    Ok(jobs().lock().unwrap().get(&paths.app_root).map(|j| j.progress.clone()))
}
fn cancel_at(root: &Path) -> bool {
    if let Some(job) = jobs().lock().unwrap().get(root).filter(|j| j.active) {
        job.cancel.cancel();
        true
    } else {
        false
    }
}
/// Signals cooperative cancellation without entering any database queue.
pub fn cancel_maintenance() -> Result<bool> {
    let paths = vault_core::project_paths()?;
    crate::context::load_unlocked_config(&paths)?;
    Ok(cancel_at(&paths.app_root))
}
