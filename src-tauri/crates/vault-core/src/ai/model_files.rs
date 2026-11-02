//! Responsibilities: the files of an in-app model on disk: the manifest entry (name, pinned
//! SHA-256, source repo), one directory per repo under `models_dir`, verifying the files and
//! remembering that they passed until they change, and the consent-gated download that fetches
//! only what is missing, verifies each file, reports byte progress and can be cancelled between
//! files and mid-file.
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
/// This full re-hash is the AUTHORITATIVE check the download runs. Loading the engine and the
/// status read go through [`model_files_verified`], which runs it only when the files changed.
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
/// It holds a [`FileStamp`] per file: the digest the file matched, and its length and modification
/// time at that moment. While those still match, the files are the ones that were hashed, so
/// [`model_files_verified`] trusts the marker instead of reading hundreds of MB again. The hash
/// guards against a broken or half-written download, not against someone rewriting a file and
/// putting its old length and time back. A marker written before the stamps (`ok`) or a missing
/// one matches nothing, so the next check hashes once and writes the stamps.
const VERIFIED_MARKER_FILE: &str = ".pathkeep-verified";

/// One manifest file as it was when it last matched its pinned digest.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
struct FileStamp {
    name: String,
    /// The digest it matched: a manifest that pins another one must not trust this stamp.
    sha256: String,
    len: u64,
    modified_secs: u64,
    modified_nanos: u32,
}

/// What the verified marker holds.
#[derive(serde::Serialize, serde::Deserialize)]
struct VerifiedMarker {
    files: Vec<FileStamp>,
}

/// Every manifest file's stamp as it is now, or `None` when one is missing, is not a regular file
/// or has no modification time after 1970 (such a model is hashed on every check).
fn file_stamps(model_dir: &std::path::Path, files: &[ModelFile]) -> Option<Vec<FileStamp>> {
    files
        .iter()
        .map(|file| {
            let metadata = std::fs::metadata(model_dir.join(file.name)).ok()?;
            let modified = metadata.modified().ok()?.duration_since(std::time::UNIX_EPOCH).ok()?;
            metadata.is_file().then(|| FileStamp {
                name: file.name.to_string(),
                sha256: file.sha256.to_string(),
                len: metadata.len(),
                modified_secs: modified.as_secs(),
                modified_nanos: modified.subsec_nanos(),
            })
        })
        .collect()
}

/// Records that the files, as `stamps` describe them, passed full SHA-256 verification.
fn write_verified_marker(model_dir: &std::path::Path, stamps: Vec<FileStamp>) -> Result<()> {
    let marker = serde_json::to_vec(&VerifiedMarker { files: stamps })?;
    std::fs::write(model_dir.join(VERIFIED_MARKER_FILE), marker)
        .with_context(|| format!("writing verified marker in {}", model_dir.display()))
}

/// Whether the marker holds every one of `stamps`.
fn marker_holds(model_dir: &std::path::Path, stamps: &[FileStamp]) -> bool {
    let Ok(bytes) = std::fs::read(model_dir.join(VERIFIED_MARKER_FILE)) else {
        return false;
    };
    serde_json::from_slice::<VerifiedMarker>(&bytes)
        .is_ok_and(|marker| stamps.iter().all(|stamp| marker.files.contains(stamp)))
}

/// Whether every manifest file is present and matches its pinned digest, hashing only when needed.
///
/// Loading the engine and every app snapshot (the Settings status) ask this, and the full check
/// reads the whole model. When each file's length and modification time still match the marker,
/// nothing is read. Otherwise it runs [`model_is_present_and_verified`] and, if that passes,
/// records the new stamps; a failure leaves the marker as it was. The stamps are taken before the
/// hash, so a file that changes while it is read no longer matches them afterwards.
pub fn model_files_verified(model_dir: &std::path::Path, files: &[ModelFile]) -> bool {
    let stamps = file_stamps(model_dir, files);
    if stamps.as_deref().is_some_and(|stamps| marker_holds(model_dir, stamps)) {
        return true;
    }
    if !model_is_present_and_verified(model_dir, files) {
        return false;
    }
    if let Some(stamps) = stamps {
        // A marker that cannot be written costs the next check another full hash, nothing more.
        let _ = write_verified_marker(model_dir, stamps);
    }
    true
}

/// Whether every manifest file is present and non-empty, without reading any of them.
///
/// The engine load asks this first, so a model that is not downloaded is an unavailable service
/// the worker retries later, while files that are there but fail verification are an error.
pub fn model_files_present(model_dir: &std::path::Path, files: &[ModelFile]) -> bool {
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
///   the verified marker records each file's stamp, so later loads and status reads skip the hash.
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
    let stamps = file_stamps(&model_dir, files);
    if model_is_present_and_verified(&model_dir, files) {
        if let Some(stamps) = stamps {
            write_verified_marker(&model_dir, stamps)?;
        }
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
    // Every file is downloaded + verified once: record their stamps so later checks skip the hash.
    if let Some(stamps) = file_stamps(&model_dir, files) {
        write_verified_marker(&model_dir, stamps)?;
    }
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

    // How `model_files_verified` could go wrong, written down before it was:
    // - an unchanged model is read again on every check (the hundreds of MB it exists to save);
    // - a file whose length or modification time changed is trusted without a hash;
    // - a marker from before the stamps, or none at all, keeps a good model unavailable or keeps
    //   it hashed on every check;
    // - a manifest that pins another digest trusts files that matched the old one;
    // - a missing file passes.

    /// Writes `bytes` over `path`, then gives it `modified` as its modification time.
    fn write_with_mtime(path: &std::path::Path, bytes: &[u8], modified: std::time::SystemTime) {
        std::fs::write(path, bytes).expect("write file");
        std::fs::File::options()
            .write(true)
            .open(path)
            .and_then(|file| file.set_modified(modified))
            .expect("set modification time");
    }

    fn mtime(path: &std::path::Path) -> std::time::SystemTime {
        std::fs::metadata(path).and_then(|meta| meta.modified()).expect("modification time")
    }

    /// Two files that verify, and a time unlike any the test writes them with.
    fn verified_pair() -> (tempfile::TempDir, Vec<ModelFile>, std::time::SystemTime) {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::write(dir.path().join("a.txt"), b"alpha").expect("write a");
        std::fs::write(dir.path().join("b.txt"), b"beta").expect("write b");
        let files = leak_manifest(&[("a.txt", b"alpha"), ("b.txt", b"beta")]);
        let other_time = std::time::UNIX_EPOCH + std::time::Duration::from_secs(86_400);
        (dir, files, other_time)
    }

    // Swapping bytes while keeping a file's length and time stands in for "was it read?": a check
    // that hashed would see the swap, one that trusted the stamp does not.
    #[test]
    fn model_files_verified_trusts_unchanged_stamps_and_hashes_changed_files() {
        let (dir, files, other_time) = verified_pair();
        let b = dir.path().join("b.txt");
        assert!(model_files_verified(dir.path(), &files), "first check hashes and passes");

        write_with_mtime(&b, b"BETA", mtime(&b));
        assert!(model_files_verified(dir.path(), &files), "same length and time: not read again");

        write_with_mtime(&b, b"BETA", other_time);
        assert!(!model_files_verified(dir.path(), &files), "another time: hashed, and wrong");
        write_with_mtime(&b, b"beta", other_time);
        assert!(model_files_verified(dir.path(), &files), "hashed again and right");
        write_with_mtime(&b, b"BETA", other_time);
        assert!(model_files_verified(dir.path(), &files), "the new stamp was recorded");

        write_with_mtime(&b, b"beta!", other_time);
        assert!(!model_files_verified(dir.path(), &files), "another length: hashed, and wrong");
        std::fs::remove_file(&b).expect("remove b");
        assert!(!model_files_verified(dir.path(), &files), "a missing file never passes");
    }

    #[test]
    fn model_files_verified_replaces_an_old_or_missing_marker_after_one_hash() {
        for old_marker in [Some(&b"ok"[..]), None] {
            let (dir, files, _) = verified_pair();
            if let Some(contents) = old_marker {
                std::fs::write(dir.path().join(VERIFIED_MARKER_FILE), contents).expect("marker");
            }
            assert!(model_files_verified(dir.path(), &files), "a good model stays available");
            let b = dir.path().join("b.txt");
            write_with_mtime(&b, b"BETA", mtime(&b));
            assert!(model_files_verified(dir.path(), &files), "the stamps replaced {old_marker:?}");
        }
    }

    #[test]
    fn model_files_verified_does_not_carry_stamps_to_another_pinned_digest() {
        let (dir, files, _) = verified_pair();
        assert!(model_files_verified(dir.path(), &files));
        let repinned = leak_manifest(&[("a.txt", b"alpha"), ("b.txt", b"BETA")]);
        assert!(!model_files_verified(dir.path(), &repinned), "b.txt never matched the new pin");
        assert!(model_files_verified(dir.path(), &files), "the old pin still holds");
    }

    #[test]
    fn model_files_present_requires_every_file_non_empty_and_no_marker() {
        let dir = tempfile::tempdir().expect("tempdir");
        let model_dir = dir.path();
        let files = leak_manifest(&[("model.safetensors", b"weights"), ("tokenizer.json", b"tok")]);
        assert!(!model_files_present(model_dir, &files));
        std::fs::write(model_dir.join("model.safetensors"), b"weights").expect("write w");
        assert!(!model_files_present(model_dir, &files), "one file is still missing");
        std::fs::write(model_dir.join("tokenizer.json"), b"tok").expect("write tok");
        assert!(model_files_present(model_dir, &files), "verification is the load's job");
        std::fs::write(model_dir.join("model.safetensors"), b"").expect("truncate");
        assert!(!model_files_present(model_dir, &files), "an empty file is not a download");
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
        // The download's stamps are ones loads and status reads trust: bytes swapped under the
        // same length and time are not read again.
        let config = model_dir_for_repo(&paths, "Stub/Repo").join("config.json");
        let (original, modified) = (std::fs::read(&config).expect("config"), mtime(&config));
        write_with_mtime(&config, &vec![b'x'; original.len()], modified);
        assert!(model_files_verified(config.parent().expect("model dir"), &manifest));
        write_with_mtime(&config, &original, modified);
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
