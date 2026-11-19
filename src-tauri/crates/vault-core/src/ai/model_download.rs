//! Responsibilities: share one consented transfer per local model, attribute its progress
//! and cancellation to its ID, and report the installer's terminal outcome off the UI thread.
//! Not responsible for model verification, archive admission or IPC transport.

use crate::{
    DEFAULT_STATIC_MODEL_FILES, DEFAULT_STATIC_MODEL_REPO, ModelDownloadProgress,
    ModelDownloadProgressEvent, ModelDownloadTransfer, ModelDownloadUpdate, ModelFile,
    ProjectPaths, ensure_model_downloaded, model_dir_for_repo,
};
use anyhow::{Context, Result};
use std::{
    collections::BTreeMap,
    path::PathBuf,
    sync::{
        Arc, Mutex, MutexGuard,
        atomic::{AtomicBool, Ordering},
    },
};

struct Transfer {
    identity: ModelDownloadTransfer,
    cancelled: AtomicBool,
}

impl Transfer {
    fn request_cancel(&self) -> bool {
        !self.cancelled.swap(true, Ordering::SeqCst)
    }
}

// The key is the on-disk model repository: two different archives have independent installs.
static RUNNING: Mutex<BTreeMap<PathBuf, Arc<Transfer>>> = Mutex::new(BTreeMap::new());

fn running() -> MutexGuard<'static, BTreeMap<PathBuf, Arc<Transfer>>> {
    RUNNING.lock().unwrap_or_else(|e| e.into_inner())
}

/// Returns true only for the first stop request for this live transfer.
pub fn cancel_model_download(id: &str) -> bool {
    running()
        .values()
        .find(|transfer| transfer.identity.transfer_id == id)
        .is_some_and(|transfer| transfer.request_cancel())
}

/// A durable loss of consent or a wipe revokes the archive's transfers with the same flag
/// that the ID-based Cancel command uses.
pub fn cancel_archive_model_downloads(paths: &ProjectPaths) {
    for (repo, transfer) in running().iter() {
        if repo.starts_with(&paths.models_dir) {
            transfer.request_cancel();
        }
    }
}

/// Reconcile a journaled install or rollback that durably replaces the live control by
/// rename or by restoring its bytes. Copies and staging controls do not change live consent.
pub fn reconcile_model_download_after_control_replacement(gate: &crate::ControlGate) {
    let paths = gate.paths();
    let has_transfer = running().keys().any(|repo| repo.starts_with(&paths.models_dir));
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
        (self.emit)(ModelDownloadProgressEvent {
            transfer_id: self.transfer.identity.transfer_id.clone(),
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

    fn cancelled(&self) -> bool {
        self.transfer.cancelled.load(Ordering::SeqCst)
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
        DEFAULT_STATIC_MODEL_FILES,
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
    if let Some(transfer) = active.get(&key)
        && !transfer.cancelled.load(Ordering::SeqCst)
    {
        return Ok(transfer.identity.clone());
    }

    let identity = ModelDownloadTransfer::fresh()?;
    let transfer =
        Arc::new(Transfer { identity: identity.clone(), cancelled: AtomicBool::new(false) });
    let mut progress = EmitProgress { emit, transfer: transfer.clone() };
    let worker_key = key.clone();
    std::thread::Builder::new()
        .name("pathkeep-static-model-download".into())
        .spawn(move || {
            // A worker that unwinds must still release its ID and settle its listeners.
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                ensure_model_downloaded(&paths, repo, files, true, &mut progress)
            }))
            .unwrap_or_else(|_| Err(anyhow::anyhow!("model download worker stopped unexpectedly")));
            // Close admission before broadcasting the terminal event. A caller can immediately
            // restart from that event. A cancelled predecessor cannot unregister the replacement.
            let terminal = {
                let _control = crate::ControlGate::acquire(&paths);
                let mut active = running();
                if active
                    .get(&worker_key)
                    .is_some_and(|current| Arc::ptr_eq(current, &progress.transfer))
                {
                    active.remove(&worker_key);
                }
                match result {
                    _ if progress.cancelled() => ModelDownloadUpdate::Cancelled,
                    Ok(_) => ModelDownloadUpdate::Done,
                    Err(error) => ModelDownloadUpdate::Error { message: error.to_string() },
                }
            };
            progress.send(terminal);
        })
        .context("spawning static model download thread")?;
    active.insert(key, transfer);
    Ok(identity)
}

#[cfg(test)]
mod tests;
