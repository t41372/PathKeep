//! Responsibilities: hold the OS ownership lock for the complete Store lifetime.
//! Not responsible for: database locking, IPC or stale lockfile deletion.
//! Failure modes: a second process opens database files; drop retains ownership.
//! Tests: ownership subprocess tests.
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
// Closing the file releases the OS lock. Never unlink it: another process may
// already hold an open descriptor for this inode while waiting to acquire it.
