//! Responsibilities: the files of an in-app model on disk: the manifest entry (name, pinned
//! SHA-256, source repo), one directory per repo under `models_dir`, verifying the files and
//! remembering that they passed until they change, and the consent-gated download that fetches
//! only what is missing, verifies each file, reports byte progress and can be cancelled between
//! files and mid-file.
//! Not responsible for: any engine that reads the files (`embedding_static`), choosing which model
//! to use, or running the download off the UI thread (the worker does).

pub(super) mod transfers;

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
    /// Basename within the selected bundle.
    pub name: &'static str,
    /// Lowercase hex SHA-256 the file must match.
    pub sha256: &'static str,
    /// Hugging Face repo the file is fetched from.
    pub repo: &'static str,
}

/// The repository root for sideloading or installation. Readers select a bundle with
/// [`super::open_model_directory`].
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
    #[cfg(test)]
    tests::observe_hash(model_dir);
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
pub(super) const VERIFIED_MARKER_FILE: &str = ".pathkeep-verified";

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
    super::model_generations::write_marker(model_dir, &marker)
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

/// Installer verification always reads the bytes, including when a previous stamp still matches.
pub(super) fn verify_for_install(directory: &std::path::Path, files: &[ModelFile]) -> Result<bool> {
    let stamps = file_stamps(directory, files);
    if !model_is_present_and_verified(directory, files) {
        return Ok(false);
    }
    if let Some(stamps) = stamps {
        write_verified_marker(directory, stamps)?;
    }
    Ok(true)
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

/// The worker forwards file and byte progress to the UI. Cancellation is cooperative:
/// the installer checks between files and before each network read. An outstanding HTTP
/// read must return before this thread can observe a cancellation.
pub trait ModelDownloadProgress: Send {
    /// Signal gate contention while the transfer still owns its progress stream.
    fn waiting(&self, _reason: crate::ModelDownloadWait) {}
    /// Arbitrate cancellation against a final decision before touching the pointer.
    fn decide(&self) -> bool {
        !self.cancelled()
    }

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
/// - **cancelable**: the installer polls before each file and each chunk read. An observed
///   cancellation discards the candidate.
/// - **progress**: `progress.file_started/finished` bracket each downloaded file.
///
/// Runs synchronously (blocking `ureq`); the worker calls it on a blocking/off-thread task
/// so the UI never blocks (the off-thread hop is the worker's job, mirroring the import/backup
/// pattern). Returns the directory of the verified bundle on success.
pub fn ensure_model_downloaded(
    paths: &ProjectPaths,
    repo: &str,
    files: &[ModelFile],
    consented: bool,
    progress: &mut dyn ModelDownloadProgress,
) -> Result<std::path::PathBuf> {
    // Every failure, including admission and filesystem errors, is final before it
    // crosses back to the coordinator. Publication claims its decision earlier, at the pointer.
    install_model(paths, repo, files, consented, progress).inspect_err(|_| {
        progress.decide();
    })
}

fn install_model(
    paths: &ProjectPaths,
    repo: &str,
    files: &[ModelFile],
    consented: bool,
    progress: &mut dyn ModelDownloadProgress,
) -> Result<std::path::PathBuf> {
    use super::model_generations as generations;
    // Manifest names are compiled-in basenames. A caller cannot designate the pointer, a
    // different folder or another generation as a download destination.
    let mut names = std::collections::BTreeSet::new();
    for file in files {
        anyhow::ensure!(
            crate::recorded_path::is_plain(file.name)
                && !file.name.starts_with('.')
                && !file.name.ends_with([' ', '.'])
                && !file.name.eq_ignore_ascii_case("current")
                && names.insert(file.name.to_ascii_lowercase()),
            "invalid or repeated model filename {}",
            file.name
        );
    }
    let root = model_dir_for_repo(paths, repo);
    let (previous, source, candidate) = match generations::resolve(
        paths,
        &root,
        files,
        generations::Request::Setup { consented, progress },
    )? {
        generations::Resolution::Busy => unreachable!("explicit setup waits for the archive gate"),
        generations::Resolution::Available(path) => return Ok(path),
        generations::Resolution::Missing => anyhow::bail!(
            "model {repo} is not present and downloading it requires explicit consent; enable the in-app model download to fetch it"
        ),
        generations::Resolution::Download { previous, source, candidate } => {
            (previous, source, candidate)
        }
    };

    let mut prepared = prepare_generation(&candidate, source.path(), files, progress);
    if prepared.is_err() {
        // Cleanup may wait for maintenance. The failure is already final.
        progress.decide();
        #[cfg(test)]
        super::model_download::tests::decision_probe();
    }
    drop(source);
    let mut checked = previous.clone();
    loop {
        let archive = crate::archive::ArchiveOpGate::acquire_reporting(paths, || {
            progress.waiting(crate::ModelDownloadWait::ArchiveBusy)
        });
        // Order publication against a concurrent control save. Only local metadata and
        // publication belong in this hold; a competing winner is hashed after releasing it.
        let control = crate::ControlGate::acquire(paths);
        let cancelled = progress.cancelled();
        let mut active = transfers::active();
        candidate.check(&active)?;
        crate::admission::guard(&paths.app_root)?;
        if cancelled {
            prepared = Err(anyhow::anyhow!("model download was cancelled"));
        }
        if let Err(error) = prepared {
            candidate.discard(&mut active)?;
            return Err(error);
        }
        let selected = generations::selected(&root)?;
        if selected != checked {
            let winner = generations::pin(&root)?;
            drop(active);
            drop(control);
            drop(archive);
            prepared = candidate.with_io(|_| {
                anyhow::ensure!(
                    model_files_verified(winner.path(), files),
                    "model selection changed during download"
                );
                Ok(())
            });
            if prepared.is_err() {
                progress.decide();
            }
            checked = selected;
            continue;
        }
        if !progress.decide() {
            candidate.discard(&mut active)?;
            anyhow::bail!("model download was cancelled");
        }
        if selected != previous {
            candidate.discard(&mut active)?;
            return Ok(selected);
        }
        candidate.finish(&mut active)?;
        #[cfg(test)]
        super::model_download::tests::decision_probe();
        crate::fault_inject::checkpoint("model-generation-verified")?;
        generations::publish(&root, &candidate.path)?;
        generations::collect_previous(&root, &previous);
        drop(active);
        drop(control);
        drop(archive);
        #[cfg(not(test))]
        super::embedding_static::forget_replaced_model(&root, &candidate.path);
        return Ok(candidate.path.clone());
    }
}

fn prepare_generation(
    candidate: &transfers::Candidate,
    current: &std::path::Path,
    files: &[ModelFile],
    progress: &mut dyn ModelDownloadProgress,
) -> Result<()> {
    for file in files {
        anyhow::ensure!(!progress.cancelled(), "model download was cancelled");
        let dest = candidate.path.join(file.name);
        let fetch = candidate.with_io(|_| {
            if download_needed(current, file) {
                return Ok(true);
            }
            // A released version can still write the flat layout, so never hard-link it.
            let mut source = std::fs::File::open(current.join(file.name))?;
            let mut copy = std::fs::File::create_new(&dest)?;
            std::io::copy(&mut source, &mut copy)?;
            Ok(false)
        })?;
        if fetch {
            fetch_one_file(file, candidate, progress)?;
        }
        candidate.with_io(|_| {
            verify_file_sha256(&dest, file.sha256)?;
            crate::durable_io::fsync_file_durably(&dest)
        })?;
        if fetch {
            progress.file_finished(file.name);
        }
    }
    anyhow::ensure!(!progress.cancelled(), "model download was cancelled");
    candidate.with_io(|_| {
        let directory = &candidate.path;
        if let Some(stamps) = file_stamps(directory, files) {
            write_verified_marker(directory, stamps)?;
        } else {
            // Filesystems without usable modification times still get a completion marker.
            super::model_generations::write_marker(directory, b"ok")?;
        }
        crate::durable_io::fsync_dir(directory)?;
        crate::durable_io::fsync_dir(directory.parent().context("missing model repository")?)
    })
}

/// Read-buffer size for the streaming download (one `read` syscall fills at most this many bytes).
const DOWNLOAD_CHUNK_BYTES: usize = 64 * 1024;

/// Emit a `file_progress` event at most ~once per this many bytes (plus a guaranteed final 100%).
///
/// Throttling by bytes (not by chunk) bounds the event rate so a 90 MB file emits ~90 events, not
/// thousands: a per-read event would flood the IPC channel and the renderer for no visible benefit.
const DOWNLOAD_PROGRESS_STRIDE_BYTES: u64 = 1024 * 1024;

/// The worker reads and reports progress without any archive or file-I/O lock. Between
/// chunks the candidate owns the file handle, so a wipe can revoke and close it immediately.
fn stream_to_file(
    mut reader: impl std::io::Read,
    total_bytes: u64,
    candidate: &transfers::Candidate,
    file_name: &str,
    progress: &mut dyn ModelDownloadProgress,
) -> Result<()> {
    use std::io::Write;
    let dest = candidate.path.join(file_name);
    candidate.with_io(|writer| {
        *writer = Some(
            std::fs::File::create_new(&dest)
                .with_context(|| format!("creating model file {}", dest.display()))?,
        );
        Ok(())
    })?;
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
        anyhow::ensure!(!progress.cancelled(), "model file download for {file_name} was cancelled");
        if read == 0 {
            break;
        }
        candidate.with_io(|writer| {
            writer
                .as_mut()
                .context("missing model download writer")?
                .write_all(&buffer[..read])
                .with_context(|| format!("writing model file {}", dest.display()))
        })?;
        downloaded += read as u64;
        if downloaded - last_emitted >= DOWNLOAD_PROGRESS_STRIDE_BYTES {
            progress.file_progress(file_name, downloaded, total_bytes);
            last_emitted = downloaded;
        }
    }
    progress.file_progress(file_name, downloaded, total_bytes);
    candidate.with_io(|writer| {
        writer
            .take()
            .context("missing model download writer")?
            .flush()
            .with_context(|| format!("flushing model file {}", dest.display()))
    })
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
    candidate: &transfers::Candidate,
    progress: &mut dyn ModelDownloadProgress,
) -> Result<()> {
    let (reader, total_bytes) = open_download_stream(file)?;
    progress.file_started(file.name, total_bytes);
    stream_to_file(reader, total_bytes, candidate, file.name, progress)
}

/// Opens a streaming HTTP reader for one model file + its content-length (the network seam).
///
/// The ONE irreducible socket seam (Change 3 methodology): GETs the HF resolve URL
/// `https://huggingface.co/{repo}/resolve/main/{file}` over `ureq` (rustls) which follows the redirect to the CDN, and returns the body reader plus the content-length
/// (0 when the server omits it). Everything ELSE — the read/write/throttle/cancel loop — lives in the
/// pure [`stream_to_file`] so it runs in tests without a network. Compiled out under test.
#[cfg(not(test))]
fn open_download_stream(file: &ModelFile) -> Result<(impl std::io::Read, u64)> {
    use super::model_source::ModelSource;
    let source = ModelSource::configured().map_err(anyhow::Error::msg)?;
    let request = ureq::get(source.url(file.repo, file.name));
    #[cfg(debug_assertions)]
    let request = match source {
        ModelSource::Fixture { .. } => request.config().proxy(None).max_redirects(0).build(),
        ModelSource::Hub => request,
    };
    let response = request.call().map_err(|error| {
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
    candidate: &transfers::Candidate,
    progress: &mut dyn ModelDownloadProgress,
) -> Result<()> {
    let bytes = stub_file_bytes(file.name);
    let total_bytes = bytes.len() as u64;
    progress.file_started(file.name, total_bytes);
    stream_to_file(std::io::Cursor::new(bytes), total_bytes, candidate, file.name, progress)
}

/// The synthetic bytes the download stub writes for a given file name.
#[cfg(test)]
fn stub_file_bytes(name: &str) -> Vec<u8> {
    format!("stub-model-file::{name}").into_bytes()
}

#[cfg(test)]
mod tests;
