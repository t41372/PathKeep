//! Responsibilities: make the standalone format marker and key envelope durable.
//! Not responsible for: database WAL durability, encryption or general-purpose file replacement.
use std::{fs::File, io, path::Path};

pub(super) fn sync_file(path: &Path) -> io::Result<()> {
    full_sync(&File::open(path)?)
}

#[cfg(unix)]
pub(super) fn sync_directory(path: &Path) -> io::Result<()> {
    full_sync(&File::open(path)?)
}

#[cfg(not(unix))]
pub(super) fn sync_directory(_path: &Path) -> io::Result<()> {
    // Windows directory handles do not support FlushFileBuffers. File contents
    // still receive the strongest std-supported sync before their references publish.
    Ok(())
}

#[cfg(target_os = "macos")]
fn full_sync(file: &File) -> io::Result<()> {
    use std::os::fd::AsRawFd;
    // SAFETY: the borrowed file owns this live descriptor for the entire call.
    if unsafe { libc::fcntl(file.as_raw_fd(), libc::F_FULLFSYNC) } == 0 {
        return Ok(());
    }
    let error = io::Error::last_os_error();
    match error.raw_os_error() {
        Some(errno) if errno == libc::ENOTSUP || errno == libc::EINVAL || errno == libc::ENOSYS => {
            file.sync_all()
        }
        _ => Err(error),
    }
}

#[cfg(not(target_os = "macos"))]
fn full_sync(file: &File) -> io::Result<()> {
    file.sync_all()
}
