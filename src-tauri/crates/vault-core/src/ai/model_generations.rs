//! Responsibilities: publish complete model bundles and keep retired files until readers release them.
//! Not responsible for choosing a manifest, downloading files or loading an embedding engine.

use super::model_files::transfers;
use super::model_files::{ModelFile, VERIFIED_MARKER_FILE, model_is_present_and_verified};
use crate::durable_io;
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
    match fs::symlink_metadata(&pointer) {
        Ok(meta) => ensure!(
            meta.is_file() && meta.len() == 36,
            "invalid model pointer in {}",
            repo.display()
        ),
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(repo.to_owned()),
        Err(e) => return Err(e.into()),
    }
    let name = fs::read_to_string(pointer)?;
    ensure!(generation_name(&name), "invalid model generation pointer in {}", repo.display());
    let directory = repo.join(name);
    real_directory_if_present(&directory)?;
    Ok(directory)
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

pub(super) fn create(repo: &Path) -> Result<PathBuf> {
    fs::create_dir_all(repo)?;
    durable_io::fsync_dir(repo.parent().context("missing models directory")?)?;
    let mut id = [0; 16];
    getrandom::fill(&mut id)?;
    let path = repo.join(format!("gen-{}", hex::encode(id)));
    fs::create_dir(&path)?;
    Ok(path)
}

pub(super) fn publish(repo: &Path, candidate: &Path) -> Result<()> {
    let name = candidate.file_name().and_then(|s| s.to_str()).context("missing generation name")?;
    ensure!(generation_name(name), "invalid model generation name");
    {
        let mut readers = readers();
        if let Some(entry) = readers.get_mut(&candidate.canonicalize()?) {
            entry.retired = None;
        }
        durable_io::atomic_durable_write(&repo.join(POINTER), name.as_bytes())?;
    }
    #[cfg(not(test))]
    super::embedding_static::forget_replaced_model(repo, candidate);
    crate::fault_inject::checkpoint("model-generation-published")
}

pub(super) fn collect_previous(repo: &Path, previous: &Path) {
    if previous == repo {
        Removal::FlatFiles.retire(repo);
    } else {
        Removal::Generation.retire(previous);
    }
}

/// Only a complete, freshly rehashed candidate can finish an interrupted switch. A valid
/// current selection wins. All other owned directories are partial downloads or retired copies.
pub(super) fn recover(
    repo: &Path,
    files: &[ModelFile],
    transfers: &transfers::Transfers,
) -> Result<PathBuf> {
    let mut current = selected(repo)?;
    if !repo.try_exists()? {
        return Ok(current);
    }
    let canonical_repo = repo.canonicalize()?;
    let mut candidates = Vec::new();
    for entry in fs::read_dir(repo)? {
        let entry = entry?;
        if entry.file_type()?.is_dir()
            && entry.file_name().to_str().is_some_and(generation_name)
            && entry.path() != current
            && !transfers.contains(&canonical_repo.join(entry.file_name()))
        {
            candidates.push(entry.path());
        }
    }
    if !candidates.is_empty() && !super::model_files::model_files_verified(&current, files) {
        for candidate in &candidates {
            // A former selection can match the pins again. Its last engine could release it
            // during this hash. Hold it through publication, which cancels pending retirement.
            let _pin = hold(candidate.clone(), &mut readers());
            if candidate.join(VERIFIED_MARKER_FILE).is_file()
                && model_is_present_and_verified(candidate, files)
            {
                publish(repo, candidate)?;
                if current != repo {
                    Removal::Generation.retire(&current);
                }
                current.clone_from(candidate);
                break;
            }
        }
    }
    for path in candidates {
        if path != current {
            Removal::Generation.retire(&path);
        }
    }
    #[cfg(not(test))]
    super::embedding_static::forget_replaced_model(repo, &current);
    if current != repo {
        Removal::FlatFiles.retire(repo);
    }
    // Pointer and verification-marker writes use durable_io. A killed process can leave a
    // sibling temp. Exclude concurrent status-marker writes while sweeping the live directories.
    let _metadata = readers();
    durable_io::sweep_temps_older_than(repo, std::time::Duration::ZERO)?;
    if current != repo {
        durable_io::sweep_temps_older_than(&current, std::time::Duration::ZERO)?;
    }
    Ok(current)
}

/// Replace metadata through a fresh inode, even when someone sideloaded a linked marker.
pub(super) fn write_marker(directory: &Path, bytes: &[u8]) -> Result<()> {
    let _metadata = readers();
    durable_io::atomic_durable_write(&directory.join(VERIFIED_MARKER_FILE), bytes)
}

/// The live owner already passed root admission. Staged archives never need a model.
pub(crate) fn recover_builtin(paths: &crate::ProjectPaths) {
    let repo = super::model_dir_for_repo(paths, super::DEFAULT_STATIC_MODEL_REPO);
    if let Err(error) = recover(&repo, super::DEFAULT_STATIC_MODEL_FILES, &transfers::active()) {
        eprintln!("pathkeep: recovering built-in model in {}: {error:#}", repo.display());
    }
}
