//! Responsibilities: reserve in-flight candidates across archive operations and let a wipe close
//! their open files without waiting for an HTTP read. Not responsible for selecting a generation.

use anyhow::{Context, Result};
use std::{
    collections::BTreeMap,
    fs::File,
    path::{Path, PathBuf},
    sync::{Mutex, MutexGuard},
};

// Selection and recovery take the archive gate (if needed) before this mutex. A transfer takes
// only this mutex for local I/O. Never call the network, progress callbacks or acquire the
// archive gate with it held.
static ACTIVE: Mutex<Transfers> = Mutex::new(Transfers { writers: BTreeMap::new() });

pub(in crate::ai) fn active() -> MutexGuard<'static, Transfers> {
    ACTIVE.lock().unwrap_or_else(|e| e.into_inner())
}

pub(in crate::ai) struct Transfers {
    writers: BTreeMap<PathBuf, Option<File>>,
}

impl Transfers {
    pub(super) fn register(&mut self, path: PathBuf) -> Result<Candidate> {
        let key = path.canonicalize()?;
        self.writers.insert(key.clone(), None);
        Ok(Candidate { path, key })
    }

    pub(in crate::ai) fn contains(&self, path: &Path) -> bool {
        self.writers.contains_key(path)
    }

    /// The wipe holds the archive gate. Removing a reservation also drops its file handle.
    pub(crate) fn revoke_under(&mut self, models: &Path) -> Result<()> {
        let root = match models.canonicalize() {
            Ok(root) => root,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(error) => return Err(error).context("locating model downloads before wipe"),
        };
        self.writers.retain(|path, _| !path.starts_with(&root));
        Ok(())
    }
}

/// A random generation name is never reused. Only its creating transfer may write there.
pub(super) struct Candidate {
    pub(super) path: PathBuf,
    key: PathBuf,
}

impl Candidate {
    /// Wipe and local file access exclude each other. A blocked HTTP read owns no guard.
    pub(super) fn with_io<T>(
        &self,
        action: impl FnOnce(&mut Option<File>) -> Result<T>,
    ) -> Result<T> {
        let mut active = active();
        let writer = active.writers.get_mut(&self.key).context("model download was wiped")?;
        action(writer)
    }

    pub(super) fn finish(&self, active: &mut Transfers) -> Result<()> {
        active.writers.remove(&self.key).context("model download was wiped")?;
        Ok(())
    }
}

impl Drop for Candidate {
    fn drop(&mut self) {
        // On unwind the next recovery owns the directory. No cleanup by pathname after a wipe.
        active().writers.remove(&self.key);
    }
}
