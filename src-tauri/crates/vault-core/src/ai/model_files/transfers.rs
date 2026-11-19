//! Responsibilities: reserve in-flight candidates across archive operations and let a wipe close
//! their open files without waiting for an HTTP read. Not responsible for selecting a generation.

use anyhow::{Context, Result};
use std::{
    collections::BTreeMap,
    fs::File,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, MutexGuard},
};

// Selection and recovery take the archive gate (if needed) before this mutex. A transfer takes
// its own writer lock for local I/O. Neither lock spans network reads.
static ACTIVE: Mutex<Transfers> = Mutex::new(Transfers { writers: BTreeMap::new() });

pub(in crate::ai) fn active() -> MutexGuard<'static, Transfers> {
    ACTIVE.lock().unwrap_or_else(|e| e.into_inner())
}

pub(in crate::ai) struct Transfers {
    writers: BTreeMap<PathBuf, Arc<Mutex<Writer>>>,
}

impl Transfers {
    /// Reserve the name in memory before verification. A wipe can revoke even a request
    /// that has not yet needed to create a directory.
    pub(in crate::ai) fn reserve(&mut self, repo: &Path) -> Result<Candidate> {
        let mut id = [0; 16];
        getrandom::fill(&mut id)?;
        let name = format!("gen-{}", hex::encode(id));
        let path = repo.join(&name);
        let key = repo.canonicalize()?.join(name);
        let writer = Arc::new(Mutex::new(Writer::Active(None)));
        self.writers.insert(key.clone(), writer.clone());
        Ok(Candidate { path, key, writer })
    }

    pub(in crate::ai) fn contains(&self, path: &Path) -> bool {
        self.writers.contains_key(path)
    }
}

/// The wipe owns this archive's gate. Remove its reservations before waiting for any local
/// file I/O to finish, so hashing cannot hold up the registry for another archive.
pub(in crate::ai) fn revoke_under(models: &Path) -> Result<()> {
    let root = match models.canonicalize() {
        Ok(root) => root,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error).context("locating model downloads before wipe"),
    };
    let mut revoked = Vec::new();
    active().writers.retain(|path, writer| {
        if path.starts_with(&root) {
            revoked.push(Arc::clone(writer));
            return false;
        }
        true
    });
    for writer in revoked {
        *writer.lock().unwrap_or_else(|e| e.into_inner()) = Writer::Wiped;
    }
    Ok(())
}

/// A random generation name is never reused. Only its creating transfer may write there.
pub(in crate::ai) struct Candidate {
    pub(in crate::ai) path: PathBuf,
    key: PathBuf,
    writer: Arc<Mutex<Writer>>,
}

enum Writer {
    Active(Option<File>),
    Wiped,
}

impl Candidate {
    /// Wipe and local file access exclude each other. A blocked HTTP read owns no guard.
    pub(in crate::ai) fn with_io<T>(
        &self,
        action: impl FnOnce(&mut Option<File>) -> Result<T>,
    ) -> Result<T> {
        match &mut *self.writer.lock().unwrap_or_else(|e| e.into_inner()) {
            Writer::Active(file) => action(file),
            Writer::Wiped => anyhow::bail!("model download was wiped"),
        }
    }

    pub(in crate::ai) fn check(&self, active: &Transfers) -> Result<()> {
        anyhow::ensure!(active.contains(&self.key), "model download was wiped");
        Ok(())
    }

    pub(super) fn finish(&self, active: &mut Transfers) -> Result<()> {
        self.check(active)?;
        active.writers.remove(&self.key);
        // An error mid-stream may have left a file open. Close it before candidate cleanup.
        *self.writer.lock().unwrap_or_else(|e| e.into_inner()) = Writer::Wiped;
        Ok(())
    }
}

impl Drop for Candidate {
    fn drop(&mut self) {
        // On unwind the next recovery owns the directory. No cleanup by pathname after a wipe.
        active().writers.remove(&self.key);
    }
}
