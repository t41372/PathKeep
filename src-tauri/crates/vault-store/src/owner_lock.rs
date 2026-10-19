//! Responsibilities: hold the OS ownership lock for the complete Store lifetime.
//! Not responsible for: database locking, IPC or stale lockfile deletion.
//! Failure modes: a second process opens database files; drop retains ownership (also through a
//! descriptor copy a spawning child holds). Tests: ownership subprocess tests, and below.
use crate::{Error, Result};
use std::{
    fs::{File, OpenOptions, TryLockError},
    io::Write,
    path::Path,
};

/// Reserves database ownership before the desktop starts any command listeners.
pub struct OwnerLock {
    _file: File,
    pub(crate) root: std::path::PathBuf,
}
impl OwnerLock {
    /// The app holds this guard until all database work has stopped.
    #[cfg(any(unix, windows))]
    pub fn acquire(root: &Path) -> Result<Self> {
        let directory = root.join("archive");
        std::fs::create_dir_all(&directory)?;
        let path = directory.join(".pk-owner.lock");
        let mut file =
            OpenOptions::new().read(true).write(true).create(true).truncate(false).open(&path)?;
        // Rust's File::try_lock uses flock(LOCK_EX|LOCK_NB) on Unix and
        // LockFileEx(LOCKFILE_EXCLUSIVE_LOCK|LOCKFILE_FAIL_IMMEDIATELY) on Windows.
        match file.try_lock() {
            Ok(()) => {
                file.set_len(0)?;
                writeln!(file, "{}", std::process::id())?;
                file.sync_data()?;
                Ok(Self { _file: file, root: root.to_path_buf() })
            }
            Err(source @ TryLockError::WouldBlock) => Err(Error::OwnedElsewhere { path, source }),
            Err(TryLockError::Error(error)) => Err(Error::Io(error)),
        }
    }
}
// Never unlink the file: another process may already hold an open descriptor for this inode
// while waiting to acquire it.
impl Drop for OwnerLock {
    /// Unlocks before the file closes. Closing alone is not enough: the lock belongs to the open
    /// file, and a child process another thread is spawning holds a copy of every descriptor
    /// until it starts its program. Until then ownership would outlive this guard, and the next
    /// acquire (a reopen after a failed open, a wipe or a move) would fail as "owned elsewhere".
    fn drop(&mut self) {
        let _ = self._file.unlock();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dropping_releases_ownership_while_a_copy_of_the_descriptor_lives() {
        let dir = tempfile::tempdir().unwrap();
        let lock = OwnerLock::acquire(dir.path()).unwrap();
        // What a child spawned by another thread holds until it starts its program.
        let inherited = lock._file.try_clone().unwrap();
        drop(lock);
        OwnerLock::acquire(dir.path()).expect("ownership outlived its guard");
        drop(inherited);
    }
}
