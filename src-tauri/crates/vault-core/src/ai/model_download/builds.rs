//! Responsibilities: let both transports consume a successful model setup exactly once in
//! the owner process. Not responsible for provider selection, job execution or persisted state.

use anyhow::{Context, Result};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

enum Build {
    Downloading,
    Ready,
    Queued(i64),
}

struct Receipt {
    repository: PathBuf,
    build: Mutex<Build>,
}

static RECEIPTS: Mutex<BTreeMap<String, Arc<Receipt>>> = Mutex::new(BTreeMap::new());

pub(super) fn issued(id: &str, repository: &Path) {
    RECEIPTS.lock().unwrap_or_else(|e| e.into_inner()).insert(
        id.to_owned(),
        Arc::new(Receipt {
            repository: repository.to_owned(),
            build: Mutex::new(Build::Downloading),
        }),
    );
}

pub(super) fn finished(id: &str, success: bool) {
    let mut receipts = RECEIPTS.lock().unwrap_or_else(|e| e.into_inner());
    if !success {
        receipts.remove(id);
    } else if let Some(receipt) = receipts.get(id) {
        *receipt.build.lock().unwrap_or_else(|e| e.into_inner()) = Build::Ready;
    }
}

pub(crate) fn forget_under(models: &Path) {
    RECEIPTS
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .retain(|_, receipt| !receipt.repository.starts_with(models));
}

/// A failed enqueue is retryable. Only the first successful queue transaction consumes the
/// receipt. Keep its job ID even after the job finishes, without retaining the vault or its lock.
pub(crate) fn enqueue_once(
    repository: &Path,
    id: &str,
    enqueue: impl FnOnce() -> Result<i64>,
) -> Result<(i64, bool)> {
    let receipt = RECEIPTS
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .get(id)
        .filter(|r| r.repository == repository)
        .cloned()
        .context("unknown model transfer for this archive")?;
    let mut build = receipt.build.lock().unwrap_or_else(|e| e.into_inner());
    match *build {
        Build::Downloading => anyhow::bail!("model transfer has not finished"),
        Build::Queued(job) => Ok((job, false)),
        Build::Ready => {
            let job = enqueue()?;
            *build = Build::Queued(job);
            Ok((job, true))
        }
    }
}
