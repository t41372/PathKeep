//! Worker-side consent-gated in-app embedding model download (§C.5).
//!
//! ## Responsibilities
//! - drive [`vault_core::ensure_model_downloaded`] OFF the UI thread (its own background thread,
//!   mirroring the streaming-chat + import/backup off-thread pattern) so a multi-hundred-MB
//!   download never blocks the renderer
//! - bridge the vault-core [`vault_core::ModelDownloadProgress`] trait callbacks onto the desktop
//!   emit sink as [`vault_core::ModelDownloadProgressEvent`]s on
//!   [`vault_core::MODEL_DOWNLOAD_PROGRESS_EVENT`], plus a terminal `Done`/`Error`
//! - support cooperative cancellation between files via a shared atomic flag
//!
//! ## Not responsible for
//! - emitting Tauri events directly (the desktop command supplies the `emit` sink closure)
//! - the consent UI. The `download_static_embedding_model` command is the only caller.
//!
//! ## Why consent stays honored
//! The command always passes `consented = true` ONLY because reaching this code IS the explicit
//! user action (pressing "Download model"); vault-core still refuses to touch the network unless
//! the model is absent, and never auto-downloads on its own (the selector degrades instead of
//! fetching).

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use anyhow::Result;
use vault_core::{
    DEFAULT_STATIC_MODEL_FILES, DEFAULT_STATIC_MODEL_REPO, ModelDownloadProgress,
    ModelDownloadProgressEvent, ensure_model_downloaded,
};

/// A single live download's cancellation flag, so a second "Download" press or a navigate-away can
/// stop the current run between files (the §C.5 cancelable contract). One at a time is enough: the
/// app only ever fetches the one default in-app model.
static DOWNLOAD_CANCEL: std::sync::OnceLock<Arc<AtomicBool>> = std::sync::OnceLock::new();

fn cancel_flag() -> &'static Arc<AtomicBool> {
    DOWNLOAD_CANCEL.get_or_init(|| Arc::new(AtomicBool::new(false)))
}

/// Requests cancellation of any in-flight in-app model download.
///
/// Idempotent and cheap: sets the shared flag the running download checks between files. A download
/// that already finished simply leaves the flag set; the next download resets it on start.
pub fn cancel_model_download() {
    cancel_flag().store(true, Ordering::SeqCst);
}

/// Bridges the vault-core progress trait onto a desktop emit sink + the shared cancel flag.
struct EmitProgress<E> {
    emit: E,
    cancel: Arc<AtomicBool>,
}

impl<E> ModelDownloadProgress for EmitProgress<E>
where
    E: Fn(ModelDownloadProgressEvent) + Send,
{
    fn file_started(&mut self, file: &str, total_bytes: u64) {
        (self.emit)(ModelDownloadProgressEvent::FileStarted {
            file: file.to_string(),
            total_bytes,
        });
    }

    fn file_progress(&mut self, file: &str, downloaded_bytes: u64, total_bytes: u64) {
        (self.emit)(ModelDownloadProgressEvent::FileProgress {
            file: file.to_string(),
            downloaded_bytes,
            total_bytes,
        });
    }

    fn file_finished(&mut self, file: &str) {
        (self.emit)(ModelDownloadProgressEvent::FileFinished { file: file.to_string() });
    }

    fn cancelled(&self) -> bool {
        self.cancel.load(Ordering::SeqCst)
    }
}

/// Starts the consent-gated in-app STATIC (Tier-0) embedding model download on a background thread (F1).
///
/// Targets the always-on static model ([`DEFAULT_STATIC_MODEL_REPO`]) the built-in static provider
/// loads, through the SHA-256-verified [`ensure_model_downloaded`] path. This is the FE's "Download
/// local model" action; reaching it IS the explicit consent. Returns immediately after spawning;
/// progress + the terminal `Done`/`Error` reach the UI on [`vault_core::MODEL_DOWNLOAD_PROGRESS_EVENT`].
/// The `emit` sink must be `Send + 'static` so the background thread can own it. Resets the cancel
/// flag on start so a prior cancellation never aborts a fresh run.
pub fn download_static_embedding_model<E>(emit: E) -> Result<()>
where
    E: Fn(ModelDownloadProgressEvent) + Send + 'static,
{
    let cancel = cancel_flag().clone();
    cancel.store(false, Ordering::SeqCst);
    std::thread::Builder::new()
        .name("pathkeep-static-model-download".to_string())
        .spawn(move || {
            run_download_for(
                vault_core::project_paths(),
                DEFAULT_STATIC_MODEL_REPO,
                DEFAULT_STATIC_MODEL_FILES,
                emit,
                cancel,
            )
        })
        .map_err(|error| anyhow::anyhow!("spawning static model download thread: {error}"))?;
    Ok(())
}

/// Downloads using a `Result<ProjectPaths>` so BOTH the path-resolution failure AND success arms are
/// testable (a test passes an `Err` to drive the failure terminal without a real broken context).
///
/// A path-resolution failure emits the terminal `Error` (the subscriber never hangs); a success
/// delegates to [`run_download_into`].
fn run_download_for<E>(
    paths: Result<vault_core::ProjectPaths>,
    repo: &str,
    files: &[vault_core::ModelFile],
    emit: E,
    cancel: Arc<AtomicBool>,
) where
    E: Fn(ModelDownloadProgressEvent) + Send,
{
    match paths {
        Ok(paths) => run_download_into(&paths, repo, files, emit, cancel),
        Err(error) => emit(ModelDownloadProgressEvent::Error { message: error.to_string() }),
    }
}

/// Downloads one model into `paths` and emits exactly one terminal `Done`/`Error`.
///
/// Factored out (paths/repo/files as params) so the success AND failure terminal arms are testable
/// without the real default model. Always emits exactly one terminal event so a subscriber keyed on
/// the event never hangs. `emit` is MOVED into the progress bridge (the `ModelDownloadProgress`
/// trait is `Send`, and an owned `E: Send` satisfies it without requiring `E: Sync`), then taken
/// back out for the terminal event.
fn run_download_into<E>(
    paths: &vault_core::ProjectPaths,
    repo: &str,
    files: &[vault_core::ModelFile],
    emit: E,
    cancel: Arc<AtomicBool>,
) where
    E: Fn(ModelDownloadProgressEvent) + Send,
{
    let mut progress = EmitProgress { emit, cancel };
    let result = ensure_model_downloaded(paths, repo, files, true, &mut progress);
    let emit = progress.emit;
    match result {
        Ok(_) => emit(ModelDownloadProgressEvent::Done),
        Err(error) => emit(ModelDownloadProgressEvent::Error { message: error.to_string() }),
    }
}
