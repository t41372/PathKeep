//! Responsibilities: the files of an in-app model on disk: the manifest entry (name, pinned
//! SHA-256, source repo), one directory per repo under `models_dir`, verifying the files, and the
//! consent-gated download that fetches only what is missing, verifies each file, reports byte
//! progress and can be cancelled between files and mid-file.
//! Not responsible for: any engine that reads the files (`embedding_static`), choosing which model
//! to use, or running the download off the UI thread (the worker does).

use crate::config::ProjectPaths;
use anyhow::{Context, Result};

/// One required file in the model bundle, with the exact SHA-256 it must hash to and its source repo.
///
/// Pinning the digest (not just the name) is the §C.5 integrity contract: a downloaded or
/// sideloaded file that does not match is rejected before it is ever loaded into the engine, so a
/// corrupted or substituted weight file fails loudly rather than producing silent garbage vectors.
/// `repo` lets one manifest pull files from more than one Hugging Face repo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModelFile {
    /// File name within `models_dir/<repo-dir>/` (the local layout flattens everything into one dir).
    pub name: &'static str,
    /// Lowercase hex SHA-256 the file must match.
    pub sha256: &'static str,
    /// Hugging Face repo the file is fetched from.
    pub repo: &'static str,
}

/// Resolves the on-disk directory for one model repo under `models_dir`.
///
/// One directory per repo id, with the repo's `org/name` flattened to a single safe segment so a
/// slash in the id never escapes `models_dir`. Pure (path math only).
pub fn model_dir_for_repo(paths: &ProjectPaths, repo: &str) -> std::path::PathBuf {
    paths.models_dir.join(crate::utils::filesystem_safe_path_segment(repo))
}

/// Returns `true` only when every manifest file is present AND matches its pinned SHA-256.
///
/// This is the offline-first + integrity gate (§C.5): a model is "available" without any network
/// iff its files are already on disk and verified. A present-but-corrupt file makes the model
/// UNAVAILABLE (returns `false`), so the engine degrades rather than loading bad weights. Streams
/// each file through [`crate::utils::file_sha256_hex`] so a multi-hundred-MB weight file is never
/// read into memory whole.
///
/// This full re-hash is the AUTHORITATIVE check used at download time and by the selector's
/// cache miss. The per-load hot path uses the cheaper [`model_is_loadable`] presence/marker
/// check (S5) so the full SHA-256 is not recomputed on every engine load.
pub fn model_is_present_and_verified(model_dir: &std::path::Path, files: &[ModelFile]) -> bool {
    files.iter().all(|file| {
        let path = model_dir.join(file.name);
        match crate::utils::file_sha256_hex(&path) {
            Ok(digest) => digest == file.sha256,
            Err(_) => false,
        }
    })
}

/// Name of the marker file written next to the weights once they pass full SHA-256 verification.
///
/// Its presence means "every manifest file in this dir verified at download time"; the per-load
/// path trusts it (plus a cheap presence/size check) instead of re-hashing hundreds of MB on every
/// engine load (S5). Removing the weights (or the marker) forces a re-verify/re-download.
pub(super) const VERIFIED_MARKER_FILE: &str = ".pathkeep-verified";

/// Records that a model directory passed full SHA-256 verification (writes the verified marker).
///
/// Called once after a download verifies every file, so subsequent loads can use the cheap
/// presence check instead of re-hashing the weights (S5).
fn write_verified_marker(model_dir: &std::path::Path) -> Result<()> {
    std::fs::write(model_dir.join(VERIFIED_MARKER_FILE), b"ok")
        .with_context(|| format!("writing verified marker in {}", model_dir.display()))
}

/// Cheap per-load readiness check: every manifest file is present + non-empty AND the dir is marked
/// verified (S5 — avoids re-hashing the weights on each load).
///
/// Pure apart from `stat`-ing each file + the marker. Used by the engine load path so a model that
/// was verified at download time loads without recomputing its SHA-256. The authoritative full
/// re-hash ([`model_is_present_and_verified`]) still runs at download time and on engine-cache misses, so a model is never LOADED unless it was verified at least once.
pub fn model_is_loadable(model_dir: &std::path::Path, files: &[ModelFile]) -> bool {
    if !model_dir.join(VERIFIED_MARKER_FILE).exists() {
        return false;
    }
    files.iter().all(|file| match std::fs::metadata(model_dir.join(file.name)) {
        Ok(meta) => meta.is_file() && meta.len() > 0,
        Err(_) => false,
    })
}

/// Verifies one already-downloaded file against its expected digest, erroring on mismatch.
///
/// Used right after a download writes a file: a digest mismatch is a hard error (the file is
/// removed by the caller) so a truncated or tampered download is never accepted. Pure apart from
/// reading the file it is asked to verify.
pub fn verify_file_sha256(path: &std::path::Path, expected_sha256: &str) -> Result<()> {
    let digest = crate::utils::file_sha256_hex(path)
        .with_context(|| format!("hashing downloaded model file {}", path.display()))?;
    if digest != expected_sha256 {
        anyhow::bail!(
            "model file {} failed SHA-256 verification (expected {expected_sha256}, got {digest})",
            path.display()
        );
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Consent-gated, SHA-256-verified, off-thread, cancelable, progress-reporting download (§C.5).
// ---------------------------------------------------------------------------

/// Progress sink for a model download .
///
/// The worker implements this to forward per-file byte progress to the UI's `pathkeep://`-style
/// event channel; a no-op default lets the foreground/offline path skip reporting. `cancelled`
/// lets the caller abort BETWEEN files at a consistent boundary (a half-written file is discarded,
/// never placed into `models_dir`), satisfying the "cancelable" requirement without tearing a
/// partially-verified blob into the model directory.
pub trait ModelDownloadProgress: Send {
    /// Announces the start of one file's download with its total size in bytes (0 if unknown).
    fn file_started(&mut self, _file: &str, _total_bytes: u64) {}
    /// Reports throttled byte progress for the in-flight file as bytes arrive.
    ///
    /// DEFAULT no-op so existing impls (and the offline/foreground path) need no change — that
    /// back-compat is why this is additive on the trait. The streaming download
    /// ([`stream_to_file`]) calls it ~every 1 MB plus a final 100%, so the UI renders a real moving
    /// bar instead of a per-file indicator that sits near-empty while the 90 MB safetensors streams.
    fn file_progress(&mut self, _file: &str, _downloaded_bytes: u64, _total_bytes: u64) {}
    /// Announces that one file finished downloading + verifying.
    fn file_finished(&mut self, _file: &str) {}
    /// Returns whether the caller has asked to cancel; checked between files AND mid-stream.
    fn cancelled(&self) -> bool {
        false
    }
}

/// A do-nothing progress sink for the offline/foreground path.
#[derive(Debug, Default)]
pub struct NoopDownloadProgress;
impl ModelDownloadProgress for NoopDownloadProgress {}

/// Decides, for one manifest file, whether it must be (re)downloaded or is already satisfied.
///
/// Pure (one stat + hash of the existing file): returns `false` when the file is already present
/// AND matches its pinned digest (offline-first reuse — no network when the bytes are already
/// correct), `true` otherwise (missing or corrupt → must fetch). Factored out so the offline-first
/// decision is unit-tested without a network.
pub fn download_needed(model_dir: &std::path::Path, file: &ModelFile) -> bool {
    let path = model_dir.join(file.name);
    match crate::utils::file_sha256_hex(&path) {
        Ok(digest) => digest != file.sha256,
        Err(_) => true,
    }
}

/// Ensures the model is present + verified in `models_dir`, downloading only what is missing.
///
/// The §C.5 contract:
/// - **consent-gated**: with `consented == false` it NEVER touches the network; it returns `Ok` iff
///   the model is ALREADY present + verified (offline/sideloaded), else an error telling the caller
///   consent is required. No auto-download ever happens without explicit consent.
/// - **offline-first**: a file already on disk that matches its digest is reused untouched; with
///   every file satisfied the function returns without any network call even when `consented`.
/// - **SHA-256 verified**: every freshly downloaded file is hashed and a mismatch is a hard error
///   (the bad blob is removed), so a truncated/tampered download is never accepted. On full success
///   a verified marker is written so later loads skip the full re-hash (S5).
/// - **cancelable**: `progress.cancelled()` is checked before each file; a cancel returns an error
///   without leaving a partial file in `models_dir`.
/// - **progress**: `progress.file_started/finished` bracket each downloaded file.
///
/// Runs synchronously (blocking `ureq`); the worker calls it on a blocking/off-thread task
/// so the UI never blocks (the off-thread hop is the worker's job, mirroring the import/backup
/// pattern). Returns the resolved `models_dir/<repo-segment>` directory on success.
pub fn ensure_model_downloaded(
    paths: &ProjectPaths,
    repo: &str,
    files: &[ModelFile],
    consented: bool,
    progress: &mut dyn ModelDownloadProgress,
) -> Result<std::path::PathBuf> {
    let model_dir = model_dir_for_repo(paths, repo);

    // Offline-first: if everything is already present + verified, we are done with NO network and
    // regardless of consent (the user already has the bytes). Refresh the verified marker so a
    // sideloaded (manually-placed) model is also fast to load thereafter.
    if model_is_present_and_verified(&model_dir, files) {
        write_verified_marker(&model_dir)?;
        return Ok(model_dir);
    }

    if !consented {
        anyhow::bail!(
            "model {repo} is not present and downloading it requires explicit consent; enable the in-app model download to fetch it"
        );
    }

    std::fs::create_dir_all(&model_dir)
        .with_context(|| format!("creating model directory {}", model_dir.display()))?;

    for file in files {
        if progress.cancelled() {
            anyhow::bail!("model download for {repo} was cancelled");
        }
        if !download_needed(&model_dir, file) {
            continue; // Already present + verified (offline-first per-file reuse).
        }
        let dest = model_dir.join(file.name);
        // Keep the destination inode immutable: an active embedding engine may still map it.
        // Download and verify a sibling file, then atomically replace the directory entry.
        let downloaded = tempfile::NamedTempFile::new_in(&model_dir)
            .with_context(|| format!("creating temporary download for {}", dest.display()))?;
        // `fetch_one_file` now owns the per-file lifecycle: it emits `file_started` with the REAL
        // content-length (not a hardcoded 0) and streams the bytes to disk emitting throttled
        // `file_progress`. It also honors `cancelled()` MID-stream, so a cancel during the 90 MB
        // safetensors aborts without writing a partial blob, not only between files.
        fetch_one_file(file, downloaded.path(), progress)?;
        // A failed download or hash check removes only the temporary file, never an active model.
        verify_file_sha256(downloaded.path(), file.sha256)?;
        downloaded
            .persist(&dest)
            .with_context(|| format!("installing verified model file {}", dest.display()))?;
        progress.file_finished(file.name);
    }
    // Every file is downloaded + verified once: record the marker so subsequent loads are cheap (S5).
    write_verified_marker(&model_dir)?;
    Ok(model_dir)
}

/// Read-buffer size for the streaming download (one `read` syscall fills at most this many bytes).
const DOWNLOAD_CHUNK_BYTES: usize = 64 * 1024;

/// Emit a `file_progress` event at most ~once per this many bytes (plus a guaranteed final 100%).
///
/// Throttling by bytes (not by chunk) bounds the event rate so a 90 MB file emits ~90 events, not
/// thousands: a per-read event would flood the IPC channel and the renderer for no visible benefit.
const DOWNLOAD_PROGRESS_STRIDE_BYTES: u64 = 1024 * 1024;

/// Streams `reader` into `dest`, emitting throttled byte progress and honoring mid-stream cancel.
///
/// The PURE, ALWAYS-COMPILED heart of the streaming download (Change 3): it reads in
/// [`DOWNLOAD_CHUNK_BYTES`] chunks, writes each to `dest`, and emits a `file_progress` event at most
/// once per [`DOWNLOAD_PROGRESS_STRIDE_BYTES`] plus a guaranteed FINAL event at 100% — so the read /
/// write / emit loop runs in tests over a [`std::io::Cursor`] rather than being `cfg`-compiled out
/// (the only network seam is [`open_download_stream`]). `cancelled()` is
/// checked at the TOP of every chunk, so a cancel mid-stream aborts and REMOVES the partial `dest`
/// rather than leaving a truncated blob behind. `total_bytes` is the content-length (0 = unknown);
/// it is only echoed into the events, never trusted as the read length.
fn stream_to_file(
    mut reader: impl std::io::Read,
    total_bytes: u64,
    dest: &std::path::Path,
    file_name: &str,
    progress: &mut dyn ModelDownloadProgress,
) -> Result<()> {
    use std::io::Write;
    let mut file = std::fs::File::create(dest)
        .with_context(|| format!("creating model file {}", dest.display()))?;
    // Removes the half-written blob on EVERY error path (cancel, network read error, disk write
    // error, flush error) so a failed download never leaves a truncated file in `models_dir` — the
    // success path disarms it. A leftover partial is already harmless (the SHA verify +
    // `.pathkeep-verified` marker gate loading, and the next attempt truncates), but RAII keeps the
    // cleanup symmetric across all exits without per-branch plumbing.
    let mut partial = PartialFileGuard { dest, armed: true };
    let mut buffer = vec![0_u8; DOWNLOAD_CHUNK_BYTES];
    let mut downloaded: u64 = 0;
    let mut last_emitted: u64 = 0;
    loop {
        if progress.cancelled() {
            anyhow::bail!("model file download for {file_name} was cancelled");
        }
        let read = reader
            .read(&mut buffer)
            .with_context(|| format!("reading download stream for {file_name}"))?;
        if read == 0 {
            break;
        }
        file.write_all(&buffer[..read])
            .with_context(|| format!("writing model file {}", dest.display()))?;
        downloaded += read as u64;
        if downloaded - last_emitted >= DOWNLOAD_PROGRESS_STRIDE_BYTES {
            progress.file_progress(file_name, downloaded, total_bytes);
            last_emitted = downloaded;
        }
    }
    file.flush().with_context(|| format!("flushing model file {}", dest.display()))?;
    // Completed cleanly: keep the file (disarm) and emit a final event so the bar reaches the end
    // even for a tiny/last sub-stride chunk.
    partial.armed = false;
    progress.file_progress(file_name, downloaded, total_bytes);
    Ok(())
}

/// Removes a half-written model file on drop unless it was disarmed by a clean completion.
///
/// RAII so EVERY error exit of [`stream_to_file`] (cancel, read, write, flush) scrubs the partial
/// blob without per-branch cleanup; the success path sets `armed = false`. Removing a still-open file
/// is fine on the unix targets PathKeep ships (the directory entry goes immediately, the inode frees
/// when the handle drops right after).
struct PartialFileGuard<'a> {
    dest: &'a std::path::Path,
    armed: bool,
}

impl Drop for PartialFileGuard<'_> {
    fn drop(&mut self) {
        if self.armed {
            let _ = std::fs::remove_file(self.dest);
        }
    }
}

/// Downloads ONE file from its HF repo into `dest` (real network path), with byte progress.
///
/// Opens a streaming HTTP reader via [`open_download_stream`] (the ONLY `cfg`-gated network seam),
/// emits `file_started` with the REAL content-length, then delegates to the pure [`stream_to_file`]
/// for the read/write/throttled-emit loop. Each file declares its own source repo, so the
/// files of a manifest that span repos are fetched from the right place.
#[cfg(not(test))]
fn fetch_one_file(
    file: &ModelFile,
    dest: &std::path::Path,
    progress: &mut dyn ModelDownloadProgress,
) -> Result<()> {
    let (reader, total_bytes) = open_download_stream(file)?;
    progress.file_started(file.name, total_bytes);
    stream_to_file(reader, total_bytes, dest, file.name, progress)
}

/// Opens a streaming HTTP reader for one model file + its content-length (the network seam).
///
/// The ONE irreducible socket seam (Change 3 methodology): GETs the HF resolve URL
/// `https://huggingface.co/{repo}/resolve/main/{file}` over `ureq` (rustls) which follows the redirect to the CDN, and returns the body reader plus the content-length
/// (0 when the server omits it). Everything ELSE — the read/write/throttle/cancel loop — lives in the
/// pure [`stream_to_file`] so it runs in tests without a network. Compiled out under test.
#[cfg(not(test))]
fn open_download_stream(file: &ModelFile) -> Result<(impl std::io::Read, u64)> {
    let url = format!("https://huggingface.co/{}/resolve/main/{}", file.repo, file.name);
    let response = ureq::get(&url).call().map_err(|error| {
        anyhow::anyhow!("downloading {} from {}: {error}", file.name, file.repo)
    })?;
    let total_bytes = response
        .headers()
        .get("content-length")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(0);
    Ok((response.into_body().into_reader(), total_bytes))
}

/// Deterministic offline stub for the download fetch (tests): streams the expected bytes.
///
/// Mirrors the real path's call graph — emit `file_started`, then run the SAME pure
/// [`stream_to_file`] over an in-memory [`std::io::Cursor`] — so the consent/offline/cancel/verify/
/// progress logic AND the streaming loop run end-to-end with no network. The manifest digests match
/// these synthetic bytes; the real `ureq` path is compiled out under test.
#[cfg(test)]
fn fetch_one_file(
    file: &ModelFile,
    dest: &std::path::Path,
    progress: &mut dyn ModelDownloadProgress,
) -> Result<()> {
    let bytes = stub_file_bytes(file.name);
    let total_bytes = bytes.len() as u64;
    progress.file_started(file.name, total_bytes);
    stream_to_file(std::io::Cursor::new(bytes), total_bytes, dest, file.name, progress)
}

/// The synthetic bytes the download stub writes for a given file name.
#[cfg(test)]
fn stub_file_bytes(name: &str) -> Vec<u8> {
    format!("stub-model-file::{name}").into_bytes()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn leak_manifest(entries: &[(&'static str, &[u8])]) -> Vec<ModelFile> {
        entries
            .iter()
            .map(|(name, bytes)| {
                let digest = crate::utils::sha256_hex(bytes);
                ModelFile { name, sha256: Box::leak(digest.into_boxed_str()), repo: "Stub/Repo" }
            })
            .collect()
    }

    #[test]
    fn model_is_present_and_verified_requires_all_files_and_matching_digests() {
        let dir = tempfile::tempdir().expect("tempdir");
        let model_dir = dir.path();
        let files = leak_manifest(&[("a.txt", b"alpha"), ("b.txt", b"beta")]);

        // Missing files → not present.
        assert!(!model_is_present_and_verified(model_dir, &files));

        std::fs::write(model_dir.join("a.txt"), b"alpha").expect("write a");
        // Still missing b → not present.
        assert!(!model_is_present_and_verified(model_dir, &files));

        std::fs::write(model_dir.join("b.txt"), b"WRONG").expect("write b wrong");
        // b present but wrong digest → not verified.
        assert!(!model_is_present_and_verified(model_dir, &files));

        std::fs::write(model_dir.join("b.txt"), b"beta").expect("write b right");
        // All present + verified.
        assert!(model_is_present_and_verified(model_dir, &files));
    }

    #[test]
    fn model_is_loadable_requires_marker_and_present_nonempty_files() {
        let dir = tempfile::tempdir().expect("tempdir");
        let model_dir = dir.path();
        let files = leak_manifest(&[("model.safetensors", b"weights"), ("tokenizer.json", b"tok")]);
        // No marker, no files → not loadable.
        assert!(!model_is_loadable(model_dir, &files));
        std::fs::write(model_dir.join("model.safetensors"), b"weights").expect("write w");
        std::fs::write(model_dir.join("tokenizer.json"), b"tok").expect("write tok");
        // Files present but no verified marker → not loadable (never load an unverified model).
        assert!(!model_is_loadable(model_dir, &files));
        write_verified_marker(model_dir).expect("marker");
        // Marker + present non-empty files → loadable.
        assert!(model_is_loadable(model_dir, &files));
        // Marker present but a file MISSING (metadata errors) → not loadable (covers the Err arm).
        std::fs::remove_file(model_dir.join("model.safetensors")).expect("remove w");
        assert!(!model_is_loadable(model_dir, &files));
        std::fs::write(model_dir.join("model.safetensors"), b"weights").expect("rewrite w");
        // An empty file is not loadable even with the marker.
        std::fs::write(model_dir.join("model.safetensors"), b"").expect("truncate");
        assert!(!model_is_loadable(model_dir, &files));
    }

    // ---- download path (consent / offline-first / cancel / verify / progress) ----

    /// A test manifest whose pinned digests match the download stub's synthetic bytes.
    fn stub_manifest() -> Vec<ModelFile> {
        // The digests must equal the SHA-256 of `stub_file_bytes(name)` so a stub download verifies.
        ["config.json", "tokenizer.json", "model.safetensors"]
            .into_iter()
            .map(|name| {
                let digest = crate::utils::sha256_hex(&stub_file_bytes(name));
                ModelFile { name, sha256: Box::leak(digest.into_boxed_str()), repo: "Stub/Repo" }
            })
            .collect()
    }

    #[derive(Default)]
    struct RecordingProgress {
        started: Vec<String>,
        finished: Vec<String>,
        cancel_after: Option<usize>,
    }

    impl ModelDownloadProgress for RecordingProgress {
        fn file_started(&mut self, file: &str, _total_bytes: u64) {
            self.started.push(file.to_string());
        }
        fn file_finished(&mut self, file: &str) {
            self.finished.push(file.to_string());
        }
        fn cancelled(&self) -> bool {
            self.cancel_after.is_some_and(|threshold| self.started.len() >= threshold)
        }
    }

    #[test]
    fn ensure_download_refuses_without_consent_when_absent() {
        let dir = tempfile::tempdir().expect("tempdir");
        let paths = crate::config::project_paths_with_root(dir.path());
        let manifest = stub_manifest();
        let mut progress = NoopDownloadProgress;
        let error = ensure_model_downloaded(&paths, "Stub/Repo", &manifest, false, &mut progress)
            .expect_err("no consent");
        assert!(error.to_string().contains("requires explicit consent"));
    }

    #[test]
    fn ensure_download_is_offline_first_when_already_verified() {
        let dir = tempfile::tempdir().expect("tempdir");
        let paths = crate::config::project_paths_with_root(dir.path());
        let manifest = stub_manifest();
        // First download places + verifies the files.
        let mut first = RecordingProgress::default();
        ensure_model_downloaded(&paths, "Stub/Repo", &manifest, true, &mut first).expect("first");
        // A second call WITHOUT consent succeeds (already present) and reports no new progress —
        // proving the offline-first reuse path runs without touching the fetch.
        let mut second = RecordingProgress::default();
        let model_dir = ensure_model_downloaded(&paths, "Stub/Repo", &manifest, false, &mut second)
            .expect("offline reuse");
        assert!(model_is_present_and_verified(&model_dir, &manifest));
        assert!(second.started.is_empty(), "no file should be re-fetched");
    }

    #[test]
    fn ensure_download_rejects_digest_mismatch_and_removes_blob() {
        let dir = tempfile::tempdir().expect("tempdir");
        let paths = crate::config::project_paths_with_root(dir.path());
        // A manifest whose digest does NOT match the stub bytes → verification fails, blob removed.
        let wrong: &'static str = Box::leak("a".repeat(64).into_boxed_str());
        let manifest = vec![ModelFile { name: "config.json", sha256: wrong, repo: "Stub/Repo" }];
        let mut progress = NoopDownloadProgress;
        let error = ensure_model_downloaded(&paths, "Stub/Repo", &manifest, true, &mut progress)
            .expect_err("digest mismatch");
        assert!(error.to_string().contains("failed SHA-256 verification"));
        let model_dir = model_dir_for_repo(&paths, "Stub/Repo");
        assert!(!model_dir.join("config.json").exists(), "bad blob must be removed");
    }

    // A repair must not truncate an inode that an active embedding engine still maps. Failed
    // verification must leave that inode alone too, and temporary downloads must be removed.
    #[test]
    fn ensure_download_replaces_files_without_changing_open_readers() {
        use std::io::Read;

        let dir = tempfile::tempdir().expect("tempdir");
        let paths = crate::config::project_paths_with_root(dir.path());
        let model_dir = model_dir_for_repo(&paths, "Stub/Repo");
        std::fs::create_dir_all(&model_dir).expect("create model dir");
        let path = model_dir.join("model.safetensors");
        std::fs::write(&path, b"old model bytes").expect("write old file");
        let mut reader = std::fs::File::open(&path).expect("open old file");
        ensure_model_downloaded(
            &paths,
            "Stub/Repo",
            &stub_manifest(),
            true,
            &mut NoopDownloadProgress,
        )
        .expect("repair model");
        let mut old_bytes = Vec::new();
        reader.read_to_end(&mut old_bytes).expect("read old file");
        assert_eq!(old_bytes, b"old model bytes");
        assert_eq!(std::fs::read(path).expect("new file"), stub_file_bytes("model.safetensors"));
        assert_eq!(std::fs::read_dir(model_dir).expect("directory").count(), 4);
    }

    #[test]
    fn ensure_download_failed_verification_preserves_existing_file() {
        let dir = tempfile::tempdir().expect("tempdir");
        let paths = crate::config::project_paths_with_root(dir.path());
        let model_dir = model_dir_for_repo(&paths, "Stub/Repo");
        std::fs::create_dir_all(&model_dir).expect("create model dir");
        let path = model_dir.join("model.safetensors");
        std::fs::write(&path, b"old model bytes").expect("write old file");
        let files = [ModelFile { name: "model.safetensors", sha256: "wrong", repo: "Stub/Repo" }];
        ensure_model_downloaded(&paths, "Stub/Repo", &files, true, &mut NoopDownloadProgress)
            .expect_err("reject bad download");
        assert_eq!(std::fs::read(path).expect("old file"), b"old model bytes");
        assert_eq!(std::fs::read_dir(model_dir).expect("directory").count(), 1);
    }
}
