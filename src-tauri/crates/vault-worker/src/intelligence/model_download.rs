//! Worker-side consent-gated in-app embedding model download (W-AI-4b, §C.5).
//!
//! ## Responsibilities
//! - drive [`vault_core::ensure_model_downloaded`] OFF the UI thread (its own background thread,
//!   mirroring the streaming-chat + import/backup off-thread pattern) so a multi-hundred-MB GGUF
//!   download never blocks the renderer
//! - bridge the vault-core [`vault_core::ModelDownloadProgress`] trait callbacks onto the desktop
//!   emit sink as [`vault_core::ModelDownloadProgressEvent`]s on
//!   [`vault_core::MODEL_DOWNLOAD_PROGRESS_EVENT`], plus a terminal `Done`/`Error`
//! - support cooperative cancellation between files via a shared atomic flag
//!
//! ## Not responsible for
//! - emitting Tauri events directly (the desktop command supplies the `emit` sink closure)
//! - the consent TOGGLE / provider-config UI — that is W-AI-9 scope. This module is the documented
//!   seam: it is the FIRST production caller of `ensure_model_downloaded` (closing the W-AI-4b S4
//!   "no production caller" gap), reachable from the `download_ai_embedding_model` Tauri command,
//!   so the download path is exercised end-to-end rather than shipped as dead code. The W-AI-9 UI
//!   only has to call this command once the user flips the consent toggle.
//!
//! ## Why consent stays honored
//! The command always passes `consented = true` ONLY because reaching this code IS the explicit
//! user action (pressing "Download model"); vault-core still refuses to touch the network unless
//! the model is absent, and never auto-downloads on its own (the selector degrades instead of
//! fetching). The toggle that gates the button is W-AI-9.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use anyhow::Result;
use vault_core::{
    DEFAULT_CANDLE_MODEL_FILES, DEFAULT_CANDLE_MODEL_REPO, ModelDownloadProgress,
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

    fn file_finished(&mut self, file: &str) {
        (self.emit)(ModelDownloadProgressEvent::FileFinished { file: file.to_string() });
    }

    fn cancelled(&self) -> bool {
        self.cancel.load(Ordering::SeqCst)
    }
}

/// Starts the consent-gated default in-app embedding model download on a background thread.
///
/// Returns immediately after spawning; the caller (desktop command) subscribes to
/// [`vault_core::MODEL_DOWNLOAD_PROGRESS_EVENT`] for per-file progress and the terminal
/// `Done`/`Error`. The `emit` sink is the desktop `AppHandle::emit` wrapper; it must be
/// `Send + 'static` so the background thread can own it. Resets the cancel flag on start so a prior
/// cancellation never aborts a fresh run.
///
/// TODO(W-AI-9): expose a per-repo/quant variant + the consent toggle + provider-config UI. For
/// W-AI-4b this fetches the single default model (`DEFAULT_CANDLE_MODEL_REPO` @ `DEFAULT_CANDLE_QUANT`).
#[cfg(not(coverage))]
pub fn download_ai_embedding_model<E>(emit: E) -> Result<()>
where
    E: Fn(ModelDownloadProgressEvent) + Send + 'static,
{
    let cancel = cancel_flag().clone();
    cancel.store(false, Ordering::SeqCst);
    std::thread::Builder::new()
        .name("pathkeep-model-download".to_string())
        .spawn(move || run_download(emit, cancel))
        .map_err(|error| anyhow::anyhow!("spawning model download thread: {error}"))?;
    Ok(())
}

/// Coverage stub: runs the download synchronously (no thread) so the bridge + emit + terminal events
/// are exercised at 100% coverage without a background thread the harness cannot join. Same call
/// graph as the real path (reset cancel → run → terminal event).
#[cfg(coverage)]
pub fn download_ai_embedding_model<E>(emit: E) -> Result<()>
where
    E: Fn(ModelDownloadProgressEvent) + Send + 'static,
{
    let cancel = cancel_flag().clone();
    cancel.store(false, Ordering::SeqCst);
    run_download(emit, cancel);
    Ok(())
}

/// The download body: resolves the project paths + default model, then downloads + emits a terminal.
///
/// Thin wrapper over [`run_download_for`] that supplies the production target (`project_paths()` +
/// the default repo/manifest).
fn run_download<E>(emit: E, cancel: Arc<AtomicBool>)
where
    E: Fn(ModelDownloadProgressEvent) + Send,
{
    run_download_for(
        vault_core::project_paths(),
        DEFAULT_CANDLE_MODEL_REPO,
        DEFAULT_CANDLE_MODEL_FILES,
        emit,
        cancel,
    );
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    fn collect(
        events: &Arc<Mutex<Vec<ModelDownloadProgressEvent>>>,
    ) -> impl Fn(ModelDownloadProgressEvent) + Send + 'static {
        let sink = events.clone();
        move |event| sink.lock().expect("sink lock").push(event)
    }

    #[test]
    fn emit_progress_bridges_callbacks_and_reads_cancel_flag() {
        let events: Arc<Mutex<Vec<ModelDownloadProgressEvent>>> = Arc::new(Mutex::new(Vec::new()));
        let cancel = Arc::new(AtomicBool::new(false));
        let mut bridge = EmitProgress { emit: collect(&events), cancel: cancel.clone() };
        assert!(!bridge.cancelled());
        bridge.file_started("config.json", 727);
        bridge.file_finished("config.json");
        cancel.store(true, Ordering::SeqCst);
        assert!(bridge.cancelled());
        let recorded = events.lock().expect("events");
        assert_eq!(
            recorded[0],
            ModelDownloadProgressEvent::FileStarted {
                file: "config.json".to_string(),
                total_bytes: 727
            }
        );
        assert_eq!(
            recorded[1],
            ModelDownloadProgressEvent::FileFinished { file: "config.json".to_string() }
        );
    }

    #[test]
    fn cancel_model_download_sets_the_shared_flag() {
        // Reset (the flag is process-global), request cancel, observe it set.
        cancel_flag().store(false, Ordering::SeqCst);
        assert!(!cancel_flag().load(Ordering::SeqCst));
        cancel_model_download();
        assert!(cancel_flag().load(Ordering::SeqCst));
        cancel_flag().store(false, Ordering::SeqCst);
    }

    #[test]
    fn run_download_resolves_paths_and_always_emits_a_terminal() {
        // Drives the production `run_download` (real `project_paths()` + default model). The cancel
        // flag is pre-SET so that — IF the default model is absent and a fetch would be needed —
        // `ensure_model_downloaded` bails at the before-first-file cancel check rather than touching
        // the network (keeps the unit test offline + deterministic). The point under test is that
        // `run_download` resolves paths and ALWAYS emits exactly one terminal so a subscriber keyed
        // on the event never hangs; we assert only that the LAST event is terminal.
        let events: Arc<Mutex<Vec<ModelDownloadProgressEvent>>> = Arc::new(Mutex::new(Vec::new()));
        let cancel = Arc::new(AtomicBool::new(true));
        run_download(collect(&events), cancel);
        let recorded = events.lock().expect("events");
        let last = recorded.last().expect("a terminal event");
        assert!(
            matches!(
                last,
                ModelDownloadProgressEvent::Done | ModelDownloadProgressEvent::Error { .. }
            ),
            "the download body must always emit a terminal event, got {last:?}"
        );
    }

    #[test]
    fn run_download_for_emits_error_when_path_resolution_fails() {
        // Drives the path-resolution FAILURE arm of `run_download_for` (which the production
        // `run_download` cannot hit on a normal machine, since `project_paths()` succeeds) by passing
        // an `Err`. Must emit exactly one terminal Error so a subscriber never hangs.
        let events: Arc<Mutex<Vec<ModelDownloadProgressEvent>>> = Arc::new(Mutex::new(Vec::new()));
        let cancel = Arc::new(AtomicBool::new(false));
        run_download_for(
            Err(anyhow::anyhow!("no unlocked project context")),
            DEFAULT_CANDLE_MODEL_REPO,
            DEFAULT_CANDLE_MODEL_FILES,
            collect(&events),
            cancel,
        );
        let recorded = events.lock().expect("events");
        assert!(
            matches!(recorded.last(), Some(ModelDownloadProgressEvent::Error { .. })),
            "path-resolution failure must emit the Error terminal"
        );
    }

    #[test]
    fn run_download_into_emits_done_on_offline_first_success() {
        // Pre-seed a model dir whose files already verify, so `ensure_model_downloaded` takes the
        // offline-first success path WITHOUT any network — driving `run_download_into`'s SUCCESS
        // terminal (Done). Manifest digests are the SHA-256 of the seeded bytes (same impl the
        // manifest verify uses, via the re-exported `sha256_hex`).
        let dir = tempfile::tempdir().expect("tempdir");
        let paths = vault_core::project_paths_with_root(dir.path());
        let repo = "Stub/Repo";
        let model_dir = vault_core::model_dir_for_repo(&paths, repo);
        std::fs::create_dir_all(&model_dir).expect("mkdir");
        let entries: [(&str, &[u8]); 2] = [("config.json", b"cfg"), ("weights.gguf", b"weights")];
        let files: Vec<vault_core::ModelFile> = entries
            .iter()
            .map(|(name, bytes)| {
                std::fs::write(model_dir.join(name), bytes).expect("seed file");
                let digest = vault_core::sha256_hex(bytes);
                vault_core::ModelFile { name, sha256: Box::leak(digest.into_boxed_str()), repo }
            })
            .collect();

        let events: Arc<Mutex<Vec<ModelDownloadProgressEvent>>> = Arc::new(Mutex::new(Vec::new()));
        let cancel = Arc::new(AtomicBool::new(false));
        run_download_into(&paths, repo, &files, collect(&events), cancel);
        let recorded = events.lock().expect("events");
        assert_eq!(
            recorded.last(),
            Some(&ModelDownloadProgressEvent::Done),
            "offline-first success must emit the Done terminal"
        );
    }

    #[test]
    fn run_download_into_emits_error_on_cancel_before_any_fetch() {
        // A manifest whose file is ABSENT would otherwise need a fetch; pre-setting the cancel flag
        // makes `ensure_model_downloaded` bail at the BEFORE-first-file cancel check, so this drives
        // `run_download_into`'s ERROR terminal WITHOUT any network call (deterministic + offline).
        let dir = tempfile::tempdir().expect("tempdir");
        let paths = vault_core::project_paths_with_root(dir.path());
        let files = [vault_core::ModelFile {
            name: "config.json",
            sha256: Box::leak("a".repeat(64).into_boxed_str()),
            repo: "Stub/Repo",
        }];
        let events: Arc<Mutex<Vec<ModelDownloadProgressEvent>>> = Arc::new(Mutex::new(Vec::new()));
        let cancel = Arc::new(AtomicBool::new(true)); // already cancelled → bails before any fetch
        run_download_into(&paths, "Stub/Repo", &files, collect(&events), cancel);
        let recorded = events.lock().expect("events");
        assert!(
            matches!(recorded.last(), Some(ModelDownloadProgressEvent::Error { .. })),
            "a cancelled download must emit the Error terminal"
        );
    }
}
