//! Responsibilities: publish complete model bundles and keep retired files until readers release them.
//! Not responsible for choosing a manifest, downloading files or loading an embedding engine.

use super::model_files::transfers;
use super::model_files::{ModelFile, VERIFIED_MARKER_FILE, model_is_present_and_verified};
use crate::durable_io;
use crate::{ProjectPaths, archive::ArchiveOpGate};
use anyhow::{Context, Result, ensure};
use std::{
    collections::BTreeMap,
    fs, io,
    path::{Path, PathBuf},
    sync::{Mutex, MutexGuard},
};

const POINTER: &str = "current";
const LEGACY_FILES: &[&str] =
    &["config.json", "tokenizer.json", "model.safetensors", VERIFIED_MARKER_FILE];

fn generation_name(name: &str) -> bool {
    name.strip_prefix("gen-").is_some_and(|id| {
        id.len() == 32 && id.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    })
}

fn real_directory_if_present(path: &Path) -> Result<()> {
    match fs::symlink_metadata(path) {
        Ok(meta) => ensure!(meta.is_dir(), "expected a model directory at {}", path.display()),
        Err(e) if e.kind() == io::ErrorKind::NotFound => (),
        Err(e) => return Err(e.into()),
    }
    Ok(())
}

// With no pointer the flat files are the original generation, including sideloaded bundles.
pub(super) fn selected(repo: &Path) -> Result<PathBuf> {
    real_directory_if_present(repo.parent().context("missing models directory")?)?;
    real_directory_if_present(repo)?;
    let pointer = repo.join(POINTER);
    let Some(name) = pointer_name(&pointer)? else { return Ok(repo.to_owned()) };
    let directory = repo.join(name);
    real_directory_if_present(&directory)?;
    Ok(directory)
}

// Malformed regular contents have no authority to name a directory. Preserve errors for
// inaccessible files and links; recovery must never follow an untrusted pointer path.
fn pointer_name(pointer: &Path) -> Result<Option<String>> {
    match fs::symlink_metadata(pointer) {
        Ok(meta) => {
            ensure!(
                meta.is_file(),
                "invalid model pointer in {}",
                pointer.parent().unwrap().display()
            );
            if meta.len() != 36 {
                return Ok(None);
            }
        }
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e.into()),
    }
    Ok(String::from_utf8(fs::read(pointer)?).ok().filter(|name| generation_name(name)))
}

#[derive(Default)]
struct Readers {
    count: usize,
    retired: Option<Removal>,
}

static READERS: Mutex<BTreeMap<PathBuf, Readers>> = Mutex::new(BTreeMap::new());

fn readers() -> MutexGuard<'static, BTreeMap<PathBuf, Readers>> {
    READERS.lock().unwrap_or_else(|e| e.into_inner())
}

/// Keeps a single selection valid from verification through the last use of its mapped matrix.
#[derive(Debug)]
pub struct ModelDirectory(PathBuf);

impl ModelDirectory {
    /// Replacement keeps this bundle available while the caller holds the directory.
    pub fn path(&self) -> &Path {
        &self.0
    }
}

/// Resolve the built-in bundle once for a reader that opens several related files.
pub fn open_model_directory(paths: &crate::ProjectPaths, repo: &str) -> Result<ModelDirectory> {
    pin(&super::model_dir_for_repo(paths, repo))
}

pub(super) fn pin(repo: &Path) -> Result<ModelDirectory> {
    let mut readers = readers();
    let directory = selected(repo)?;
    Ok(hold(directory, &mut readers))
}

fn hold(directory: PathBuf, readers: &mut BTreeMap<PathBuf, Readers>) -> ModelDirectory {
    let key = directory.canonicalize().unwrap_or(directory);
    readers.entry(key.clone()).or_default().count += 1;
    ModelDirectory(key)
}

impl Drop for ModelDirectory {
    fn drop(&mut self) {
        let mut readers = readers();
        let entry = readers.get_mut(&self.0).expect("model directory has a reader");
        entry.count -= 1;
        if entry.count == 0 {
            if let Some(action) = readers.remove(&self.0).and_then(|entry| entry.retired) {
                action.remove(&self.0);
            }
        }
    }
}

#[derive(Clone, Copy)]
enum Removal {
    Generation,
    FlatFiles,
}

impl Removal {
    fn retire(self, path: &Path) {
        let mut readers = readers();
        let key = path.canonicalize().unwrap_or_else(|_| path.to_owned());
        if let Some(entry) = readers.get_mut(&key) {
            entry.retired = Some(self);
        } else {
            self.remove(path);
        }
    }

    fn remove(self, path: &Path) {
        // A restored/recreated folder can occupy this name when the last reader lets go.
        let repo = match self {
            Self::Generation => path.parent().expect("generation is inside a repository"),
            Self::FlatFiles => path,
        };
        if !selected(repo).is_ok_and(|current| current != path) {
            return;
        }
        // An unmanaged reader (or antivirus) can still hold a file on Windows. Retry on open
        // and install, without holding up publication. Never edit the contents of a retired file.
        match self {
            Self::Generation => {
                let _ = fs::remove_dir_all(path);
            }
            Self::FlatFiles => {
                for name in LEGACY_FILES {
                    let _ = fs::remove_file(path.join(name));
                }
            }
        }
    }
}

pub(super) fn publish(repo: &Path, candidate: &Path) -> Result<()> {
    let name = candidate.file_name().and_then(|s| s.to_str()).context("missing generation name")?;
    ensure!(candidate == repo || generation_name(name), "invalid model generation name");
    {
        let mut readers = readers();
        if let Some(entry) = readers.get_mut(&candidate.canonicalize()?) {
            entry.retired = None;
        }
        if candidate == repo {
            durable_io::remove_file_durably(&repo.join(POINTER))?;
        } else {
            durable_io::atomic_durable_write(&repo.join(POINTER), name.as_bytes())?;
        }
    }
    crate::fault_inject::checkpoint("model-generation-published")
}

pub(super) fn collect_previous(repo: &Path, previous: &Path) {
    if previous == repo {
        Removal::FlatFiles.retire(repo);
    } else {
        Removal::Generation.retire(previous);
    }
}

/// Each path observed under the transfer registry stays pinned until we recheck the pointer.
struct Bundles {
    current: PathBuf,
    damaged_pointer: bool,
    candidates: Vec<PathBuf>,
    _pins: Vec<ModelDirectory>,
}

impl Bundles {
    fn inspect(repo: &Path, transfers: &transfers::Transfers) -> Result<Self> {
        let current = selected(repo)?;
        let canonical = repo.canonicalize()?;
        let mut candidates = Vec::new();
        for entry in fs::read_dir(repo)? {
            let entry = entry?;
            if entry.file_type()?.is_dir()
                && entry.file_name().to_str().is_some_and(generation_name)
                && entry.path() != current
                && !transfers.contains(&canonical.join(entry.file_name()))
            {
                candidates.push(entry.path());
            }
        }
        let mut readers = readers();
        let pins = candidates
            .iter()
            .chain([&current])
            .cloned()
            .chain((current != repo).then(|| repo.to_owned()))
            .map(|path| hold(path, &mut readers))
            .collect();
        let damaged_pointer = if current == repo {
            repo.join(POINTER).try_exists()?
        } else {
            !current.try_exists()?
        };
        Ok(Self { current, damaged_pointer, candidates, _pins: pins })
    }

    fn verified(
        &self,
        repo: &Path,
        files: &[ModelFile],
        install: bool,
        recover_candidate: bool,
    ) -> Result<Option<PathBuf>> {
        // An ordinary open with nothing to promote or retire needs no model bytes.
        if !install
            && !self.damaged_pointer
            && self.candidates.is_empty()
            && (self.current == repo || !LEGACY_FILES.iter().any(|name| repo.join(name).is_file()))
        {
            return Ok(None);
        }
        let current_ok = if install {
            super::model_files::verify_for_install(&self.current, files)?
        } else {
            super::model_files::model_files_verified(&self.current, files)
        };
        if current_ok {
            return Ok(Some(self.current.clone()));
        }
        if recover_candidate
            && let Some(candidate) = self.candidates.iter().find(|path| {
                path.join(VERIFIED_MARKER_FILE).is_file()
                    && model_is_present_and_verified(path, files)
            })
        {
            return Ok(Some(candidate.clone()));
        }
        if self.current != repo && model_is_present_and_verified(repo, files) {
            return Ok(Some(repo.to_owned()));
        }
        Ok(None)
    }

    fn apply(&self, repo: &Path, verified: Option<&Path>) -> Result<()> {
        if let Some(next) = verified
            && next != self.current
        {
            publish(repo, next)?;
            collect_previous(repo, &self.current);
        }
        // No replacement has been verified. Preserve every possible former selection.
        if self.damaged_pointer && verified.is_none() {
            return Ok(());
        }
        if self.damaged_pointer && self.current == repo && verified == Some(repo) {
            durable_io::remove_file_durably(&repo.join(POINTER))?;
        }
        for path in &self.candidates {
            if Some(path.as_path()) != verified {
                Removal::Generation.retire(path);
            }
        }
        if verified.is_some_and(|path| path != repo) {
            Removal::FlatFiles.retire(repo);
        }
        let _metadata = readers();
        durable_io::sweep_temps_older_than(repo, std::time::Duration::ZERO)?;
        let current = verified.unwrap_or(&self.current);
        if current != repo {
            durable_io::sweep_temps_older_than(current, std::time::Duration::ZERO)?;
        }
        Ok(())
    }
}

#[derive(Clone, Copy)]
pub(super) enum Request<'a> {
    OnOpen,
    Setup { consented: bool, progress: &'a dyn super::ModelDownloadProgress },
}

impl Request<'_> {
    fn gate(self, paths: &ProjectPaths) -> Option<ArchiveOpGate> {
        match self {
            Self::OnOpen => ArchiveOpGate::try_acquire(paths),
            Self::Setup { progress, .. } => {
                Some(super::model_files::acquire_download_gate(paths, progress))
            }
        }
    }
}

pub(super) enum Resolution {
    Busy,
    Available(PathBuf),
    Missing,
    Download { previous: PathBuf, source: ModelDirectory, candidate: transfers::Candidate },
}

/// Startup recovers interrupted work. An explicit install also fully rehashes the selected
/// model, and can reserve disk space for a download only when consented.
pub(super) fn resolve(
    paths: &ProjectPaths,
    repo: &Path,
    files: &[ModelFile],
    request: Request<'_>,
) -> Result<Resolution> {
    let can_download = matches!(request, Request::Setup { consented: true, .. });
    let reservation = {
        let Some(_archive) = request.gate(paths) else { return Ok(Resolution::Busy) };
        crate::admission::guard(&paths.app_root)?;
        if let Request::Setup { progress, .. } = request {
            anyhow::ensure!(!progress.cancelled(), "model download was cancelled");
        }
        selected(repo)?;
        if !repo.try_exists()? {
            if !can_download {
                return Ok(Resolution::Missing);
            }
            fs::create_dir_all(repo)?;
            durable_io::fsync_dir(repo.parent().context("missing models directory")?)?;
        }
        transfers::active().reserve(repo)?
    };
    loop {
        let bundles = {
            let active = transfers::active();
            reservation.check(&active)?;
            Bundles::inspect(repo, &active)?
        };
        let setup = matches!(request, Request::Setup { .. });
        let may_promote = setup || crate::load_config(paths)?.ai.wants_builtin_model();
        // A damaged pointer cannot distinguish a withdrawn candidate from the previously
        // installed generation. With consent off, defer that decision to the next setup.
        if bundles.damaged_pointer && !may_promote {
            return Ok(Resolution::Missing);
        }
        let verified =
            reservation.with_io(|_| bundles.verified(repo, files, setup, may_promote))?;
        let Some(archive) = request.gate(paths) else { return Ok(Resolution::Busy) };
        let control = match request {
            Request::Setup { .. } => crate::ControlGate::acquire(paths),
            Request::OnOpen => {
                let Some(control) = crate::ControlGate::try_acquire(paths) else {
                    return Ok(Resolution::Busy);
                };
                control
            }
        };
        if !setup && may_promote != crate::load_config(paths)?.ai.wants_builtin_model() {
            continue;
        }
        if let Request::Setup { progress, .. } = request {
            anyhow::ensure!(!progress.cancelled(), "model download was cancelled");
        }
        let active = transfers::active();
        reservation.check(&active)?;
        crate::admission::guard(&paths.app_root)?;
        if selected(repo)? != bundles.current {
            continue;
        }
        if let Request::Setup { progress, .. } = request
            && (verified.is_some() || !can_download)
        {
            anyhow::ensure!(progress.decide(), "model download was cancelled");
        }
        bundles.apply(repo, verified.as_deref())?;
        if let Some(current) = verified {
            drop(active);
            drop(control);
            drop(archive);
            #[cfg(not(test))]
            super::embedding_static::forget_replaced_model(repo, &current);
            return Ok(Resolution::Available(current));
        }
        if !can_download {
            return Ok(Resolution::Missing);
        }
        fs::create_dir(&reservation.path)?;
        let source = pin(repo)?;
        return Ok(Resolution::Download {
            previous: bundles.current,
            source,
            candidate: reservation,
        });
    }
}

/// Replace metadata through a fresh inode, even when someone sideloaded a linked marker.
pub(super) fn write_marker(directory: &Path, bytes: &[u8]) -> Result<()> {
    let _metadata = readers();
    durable_io::atomic_durable_write(&directory.join(VERIFIED_MARKER_FILE), bytes)
}

/// The live owner already passed root admission. Staged archives never need a model.
pub(crate) fn recover_builtin(paths: &ProjectPaths) -> Result<()> {
    let repo = super::model_dir_for_repo(paths, super::DEFAULT_STATIC_MODEL_REPO);
    if repo.try_exists()? {
        // A maintenance reopen already owns the gate. Leave the model tree for the next
        // ordinary open or explicit setup. Nothing may act on this owner's paths after close.
        resolve(paths, &repo, super::builtin_model_files()?, Request::OnOpen)?;
    }
    Ok(())
}
