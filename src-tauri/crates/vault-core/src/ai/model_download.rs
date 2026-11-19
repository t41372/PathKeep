//! Responsibilities: share one consented transfer per local model, attribute its progress
//! and cancellation to its ID, and report the installer's terminal outcome off the UI thread.
//! Not responsible for model verification, archive admission or IPC transport.

pub(crate) mod builds;
mod state;

use crate::{
    DEFAULT_STATIC_MODEL_REPO, ModelDownloadProgress, ModelDownloadProgressEvent,
    ModelDownloadTransfer, ModelDownloadUpdate, ModelFile, ProjectPaths, ensure_model_downloaded,
    model_dir_for_repo,
};
use anyhow::{Context, Result};
use std::{
    collections::BTreeMap,
    path::PathBuf,
    sync::{Arc, Condvar, Mutex, MutexGuard},
};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Phase {
    Running,
    Cancelled,
    Decided,
    Released,
}

struct Transfer {
    identity: ModelDownloadTransfer,
    phase: Mutex<Phase>,
    released: Condvar,
    progress: Mutex<state::Progress>,
}

impl Transfer {
    fn new(identity: ModelDownloadTransfer) -> Self {
        Self {
            identity,
            phase: Mutex::new(Phase::Running),
            released: Condvar::new(),
            progress: Mutex::default(),
        }
    }
    fn progress(&self) -> MutexGuard<'_, state::Progress> {
        self.progress.lock().unwrap_or_else(|e| e.into_inner())
    }
    fn phase(&self) -> MutexGuard<'_, Phase> {
        self.phase.lock().unwrap_or_else(|e| e.into_inner())
    }
    fn request_cancel(&self) -> bool {
        let mut phase = self.phase();
        if *phase != Phase::Running {
            return false;
        }
        *phase = Phase::Cancelled;
        true
    }
    fn decide(&self) -> bool {
        let mut phase = self.phase();
        if *phase == Phase::Cancelled {
            return false;
        }
        *phase = Phase::Decided;
        true
    }
    fn wait_until_released(&self, progress: &dyn ModelDownloadProgress) {
        let mut phase = self.phase();
        while *phase != Phase::Released {
            drop(phase);
            progress.waiting(crate::ModelDownloadWait::PreviousTransfer);
            phase = self
                .released
                .wait_timeout_while(self.phase(), std::time::Duration::from_secs(20), |phase| {
                    *phase != Phase::Released
                })
                .unwrap_or_else(|e| e.into_inner())
                .0;
        }
    }
    fn release(&self) {
        *self.phase() = Phase::Released;
        self.released.notify_all();
    }
}

// The key is the on-disk model repository: two different archives have independent installs.
struct Registry {
    last_seq: u64,
    by_repo: BTreeMap<PathBuf, Arc<Transfer>>,
}

static RUNNING: Mutex<Registry> = Mutex::new(Registry { last_seq: 0, by_repo: BTreeMap::new() });

fn running() -> MutexGuard<'static, Registry> {
    RUNNING.lock().unwrap_or_else(|e| e.into_inner())
}

/// Look up the latest transfer for the active archive without joining or starting one.
pub fn get_model_download_state() -> Result<Option<crate::ModelDownloadState>> {
    Ok(state_for(&crate::project_paths()?, DEFAULT_STATIC_MODEL_REPO))
}

fn state_for(paths: &ProjectPaths, repo: &str) -> Option<crate::ModelDownloadState> {
    running()
        .by_repo
        .get(&model_dir_for_repo(paths, repo))
        .map(|transfer| transfer.progress().snapshot(&transfer.identity))
}

/// Returns true only for the first stop request for this live transfer.
pub fn cancel_model_download(id: &str) -> bool {
    running()
        .by_repo
        .values()
        .find(|transfer| transfer.identity.transfer_id == id)
        .is_some_and(|transfer| transfer.request_cancel())
}

/// A durable loss of consent or a wipe revokes the archive's transfers with the same flag
/// that the ID-based Cancel command uses.
pub fn cancel_archive_model_downloads(paths: &ProjectPaths) {
    for (repo, transfer) in running().by_repo.iter() {
        if repo.starts_with(&paths.models_dir) {
            transfer.request_cancel();
        }
    }
}

/// Reconcile a journaled install or rollback that durably replaces the live control by
/// rename or by restoring its bytes. Copies and staging controls do not change live consent.
pub fn reconcile_model_download_after_control_replacement(gate: &crate::ControlGate) {
    let paths = gate.paths();
    let has_transfer = running().by_repo.keys().any(|repo| repo.starts_with(&paths.models_dir));
    if has_transfer
        && !crate::load_config(paths).is_ok_and(|config| config.ai.wants_builtin_model())
    {
        cancel_archive_model_downloads(paths);
    }
}

struct EmitProgress<E> {
    emit: E,
    transfer: Arc<Transfer>,
}

impl<E: Fn(ModelDownloadProgressEvent)> EmitProgress<E> {
    fn send(&self, update: ModelDownloadUpdate) {
        self.transfer.progress().record(&update);
        (self.emit)(ModelDownloadProgressEvent {
            transfer_id: self.transfer.identity.transfer_id.clone(),
            seq: self.transfer.identity.seq,
            update,
        });
    }
}

impl<E: Fn(ModelDownloadProgressEvent) + Send> ModelDownloadProgress for EmitProgress<E> {
    fn file_started(&mut self, file: &str, total_bytes: u64) {
        self.send(ModelDownloadUpdate::FileStarted { file: file.into(), total_bytes });
    }

    fn file_progress(&mut self, file: &str, downloaded_bytes: u64, total_bytes: u64) {
        self.send(ModelDownloadUpdate::FileProgress {
            file: file.into(),
            downloaded_bytes,
            total_bytes,
        });
    }

    fn file_finished(&mut self, file: &str) {
        self.send(ModelDownloadUpdate::FileFinished { file: file.into() });
    }

    fn waiting(&self, reason: crate::ModelDownloadWait) {
        self.send(ModelDownloadUpdate::Waiting { reason });
    }
    fn resumed(&self) {
        self.transfer.progress().phase = crate::ModelDownloadPhase::Downloading;
    }
    fn decide(&self) -> bool {
        self.transfer.decide()
    }
    fn cancelled(&self) -> bool {
        *self.transfer.phase() == Phase::Cancelled
    }
}

/// Starts the user-approved install or attaches to its broadcast. The transport subscribes
/// before invoking start: even an already-installed model can finish before the reply arrives.
pub fn download_static_embedding_model<E>(emit: E) -> Result<ModelDownloadTransfer>
where
    E: Fn(ModelDownloadProgressEvent) + Send + 'static,
{
    start_model_download(
        crate::project_paths()?,
        DEFAULT_STATIC_MODEL_REPO,
        super::builtin_model_files()?,
        emit,
    )
}

fn start_model_download<E>(
    paths: ProjectPaths,
    repo: &'static str,
    files: &'static [ModelFile],
    emit: E,
) -> Result<ModelDownloadTransfer>
where
    E: Fn(ModelDownloadProgressEvent) + Send + 'static,
{
    // Read the committed selection and register in one control-gate hold: a late start
    // cannot slip between a save and cancellation of the formerly wanted transfer.
    let _control = crate::ControlGate::acquire(&paths);
    let config = crate::load_config(&paths)?;
    anyhow::ensure!(
        config.ai.wants_builtin_model(),
        "model download requires AI and semantic search enabled with the built-in model selected"
    );
    let key = model_dir_for_repo(&paths, repo);
    let mut active = running();
    if let Some(transfer) = active.by_repo.get(&key)
        && *transfer.phase() == Phase::Running
    {
        return Ok(transfer.identity.clone());
    }

    // JavaScript compares these JSON numbers. Stop at its largest exact integer.
    let seq = active.last_seq + 1;
    anyhow::ensure!(seq < 1 << 53, "model download sequence exhausted");
    let identity = ModelDownloadTransfer::fresh(seq)?;
    let predecessor = active.by_repo.get(&key).cloned();
    let transfer = Arc::new(Transfer::new(identity.clone()));
    if predecessor.as_ref().is_some_and(|previous| *previous.phase() != Phase::Released) {
        transfer.progress().phase = crate::ModelDownloadPhase::Waiting {
            reason: crate::ModelDownloadWait::PreviousTransfer,
        };
    }
    let mut progress = EmitProgress { emit, transfer: transfer.clone() };
    let worker_key = key.clone();
    std::thread::Builder::new()
        .name("pathkeep-static-model-download".into())
        .spawn(move || {
            // A worker that unwinds must still release its ID and settle its listeners.
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                if let Some(previous) = &predecessor {
                    previous.wait_until_released(&progress);
                    progress.resumed();
                }
                anyhow::ensure!(!progress.cancelled(), "model download was cancelled");
                ensure_model_downloaded(&paths, repo, files, true, &mut progress)
            }))
            .unwrap_or_else(|_| {
                progress.decide();
                if let Some(previous) = &predecessor {
                    previous.wait_until_released(&crate::NoopDownloadProgress);
                }
                Err(anyhow::anyhow!("model download worker stopped unexpectedly"))
            });
            let accepted = progress.decide();
            progress.transfer.release();
            // Close admission before broadcasting the terminal event. A caller can immediately
            // restart from that event. A cancelled predecessor cannot unregister the replacement.
            let terminal = {
                let mut active = running();
                if active
                    .by_repo
                    .get(&worker_key)
                    .is_some_and(|current| Arc::ptr_eq(current, &progress.transfer))
                {
                    active.by_repo.remove(&worker_key);
                }
                builds::finished(
                    &progress.transfer.identity.transfer_id,
                    accepted && result.is_ok(),
                );
                match result {
                    _ if !accepted => ModelDownloadUpdate::Cancelled,
                    Ok(_) => ModelDownloadUpdate::Done,
                    Err(error) => ModelDownloadUpdate::Error { message: format!("{error:#}") },
                }
            };
            progress.send(terminal);
        })
        .context("spawning static model download thread")?;
    builds::issued(&identity.transfer_id, &key);
    active.last_seq = seq;
    active.by_repo.insert(key, transfer);
    Ok(identity)
}

#[cfg(test)]
pub(super) mod tests;
