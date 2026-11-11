//! Responsibilities: publish explicit plaintext exports without replacing existing
//! files or writing into an archive or an agent's source tree.
//! Not responsible for: choosing an export format, unlocking archives, or
//! authorizing the operation. The CLI requires `--execute` before calling here.

use agent_history::{Error, Result};
use std::{
    ffi::{OsStr, OsString},
    fs::{self, File},
    io::{self, BufWriter, Write},
    path::{Path, PathBuf},
};

/// Keeps a partial export private until encoding and the data barrier succeed.
/// Publication refuses a destination that another process created meanwhile.
pub(super) fn write_export<T>(
    output: &Path,
    archive_root: &Path,
    source_roots: &[PathBuf],
    write: impl FnOnce(&mut dyn Write) -> Result<T>,
) -> Result<T> {
    let destination = validate_destination(output, archive_root, source_roots)?;
    let parent = destination
        .parent()
        .ok_or_else(|| Error::InvalidInput("export destination has no parent".into()))?;
    reject_missing_name_aliases(&destination, archive_root, source_roots)?;
    let temporary = tempfile::Builder::new()
        .prefix(".pathkeep-export-")
        .suffix(".partial")
        .tempfile_in(parent)?;
    let result = {
        let mut output = BufWriter::with_capacity(64 * 1024, temporary.as_file());
        let result = write(&mut output)?;
        output.flush()?;
        result
    };
    full_sync(temporary.as_file())?;
    temporary.persist_noclobber(&destination).map_err(|error| {
        if error.error.kind() == io::ErrorKind::AlreadyExists {
            Error::AlreadyExists("export destination".into())
        } else {
            Error::Io(error.error)
        }
    })?;
    sync_directory(parent)?;
    Ok(result)
}

/// Performs read-only path checks and resolves the publishing parent. Execution
/// also checks missing reserved names using the filesystem's own naming rules.
pub(super) fn validate_destination(
    output: &Path,
    archive_root: &Path,
    source_roots: &[PathBuf],
) -> Result<PathBuf> {
    match fs::symlink_metadata(output) {
        Ok(_) => return Err(Error::AlreadyExists("export destination".into())),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }
    let name = output
        .file_name()
        .filter(|name| !name.is_empty())
        .ok_or_else(|| Error::InvalidInput("export destination needs a file name".into()))?;
    let parent =
        output.parent().filter(|path| !path.as_os_str().is_empty()).unwrap_or(Path::new("."));
    let parent = fs::canonicalize(parent)?;
    if !parent.is_dir() {
        return Err(Error::InvalidInput("export parent is not a directory".into()));
    }
    let destination = parent.join(name);
    if destination.starts_with(canonical_location(archive_root)?) {
        return Err(Error::InvalidInput("export destination overlaps the archive".into()));
    }
    for source in source_roots {
        if destination.starts_with(canonical_location(source)?) {
            return Err(Error::InvalidInput("export destination overlaps an agent source".into()));
        }
    }
    Ok(destination)
}

/// Canonicalization cannot tell whether two absent spellings name the same file.
/// A missing root can alias an output only at its final component: the output's
/// parent must already exist. Probe that component in the actual parent directory
/// during explicit execution, without creating or opening the reserved source.
fn reject_missing_name_aliases(
    destination: &Path,
    archive_root: &Path,
    source_roots: &[PathBuf],
) -> Result<()> {
    let parent = destination.parent().expect("resolved destination has a parent");
    let output_name = destination.file_name().expect("validated destination has a name");
    for root in std::iter::once(archive_root).chain(source_roots.iter().map(PathBuf::as_path)) {
        let reserved = canonical_location(root)?;
        if reserved.try_exists()? {
            continue;
        }
        let (Some(reserved_parent), Some(reserved_name)) =
            (reserved.parent(), reserved.file_name())
        else {
            continue;
        };
        if same_directory(parent, reserved_parent)?
            && names_alias(parent, reserved_name, output_name)?
        {
            return Err(Error::InvalidInput("export destination aliases a reserved root".into()));
        }
    }
    Ok(())
}

fn same_directory(left: &Path, right: &Path) -> io::Result<bool> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let left = fs::metadata(left)?;
        match fs::metadata(right) {
            Ok(right) => Ok(left.dev() == right.dev() && left.ino() == right.ino()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
            Err(error) => Err(error),
        }
    }
    #[cfg(not(unix))]
    {
        match fs::canonicalize(right) {
            Ok(right) => Ok(fs::canonicalize(left)? == right),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
            Err(error) => Err(error),
        }
    }
}

/// The common random ASCII prefix leaves case, Unicode normalization and trailing
/// character comparisons to this directory's filesystem. No guessed case-fold
/// table or volume-wide assumption can release a reserved source location.
/// An overlong probe name or unsupported lookup fails closed before encoding.
fn names_alias(parent: &Path, reserved: &OsStr, output: &OsStr) -> io::Result<bool> {
    let probe =
        tempfile::Builder::new().prefix(".pathkeep-name-").suffix(reserved).tempfile_in(parent)?;
    let name = probe.path().file_name().expect("temporary file has a name").as_encoded_bytes();
    let suffix = reserved.as_encoded_bytes();
    let prefix = name
        .strip_suffix(suffix)
        .and_then(|bytes| std::str::from_utf8(bytes).ok())
        .ok_or_else(|| io::Error::other("temporary name prefix is not ASCII"))?;
    let mut alternate = OsString::from(prefix);
    alternate.push(output);
    let alias = match fs::symlink_metadata(parent.join(alternate)) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error),
    };
    probe.close()?;
    alias
}

/// A deleted source root is still reserved. Resolve its closest surviving parent
/// and append the absent components, rather than forgetting it after NotFound.
fn canonical_location(path: &Path) -> io::Result<PathBuf> {
    let absolute =
        if path.is_absolute() { path.to_path_buf() } else { std::env::current_dir()?.join(path) };
    // Resolve before removing components: `symlink/..` refers to the target's
    // parent, which is not necessarily the symlink's lexical parent.
    let mut ancestor = absolute.as_path();
    let mut absent = Vec::new();
    loop {
        match fs::canonicalize(ancestor) {
            Ok(mut resolved) => {
                for component in absent.iter().rev() {
                    resolved.push(component);
                }
                return Ok(resolved);
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                let name = ancestor.file_name().ok_or(error)?;
                absent.push(name.to_os_string());
                ancestor = ancestor
                    .parent()
                    .ok_or_else(|| io::Error::other("path has no existing ancestor"))?;
            }
            Err(error) => return Err(error),
        }
    }
}

/// Match the archive durability policy without depending on the application core.
#[cfg(target_os = "macos")]
fn full_sync(file: &File) -> io::Result<()> {
    use std::os::fd::AsRawFd;
    // SAFETY: this call borrows a live file descriptor and does not retain it.
    if unsafe { libc::fcntl(file.as_raw_fd(), libc::F_FULLFSYNC) } == 0 {
        return Ok(());
    }
    let error = io::Error::last_os_error();
    let code = error.raw_os_error().unwrap_or(libc::EIO);
    if code == libc::ENOTSUP
        || code == libc::EOPNOTSUPP
        || code == libc::EINVAL
        || code == libc::ENOSYS
    {
        // Removable/network filesystems may offer only a best-effort barrier.
        file.sync_all()
    } else {
        Err(error)
    }
}

#[cfg(not(target_os = "macos"))]
fn full_sync(file: &File) -> io::Result<()> {
    file.sync_all()
}

#[cfg(unix)]
fn sync_directory(path: &Path) -> io::Result<()> {
    full_sync(&File::open(path)?)
}

#[cfg(not(unix))]
fn sync_directory(_path: &Path) -> io::Result<()> {
    // Rust has no portable directory-flush API on Windows. File contents are
    // flushed before the OS publishes the destination without replacement.
    Ok(())
}

#[cfg(test)]
#[path = "output_tests.rs"]
mod tests;
