//! Responsibilities: validate file authority and stream repeatable native bytes.
//! Not responsible for publication, parsing, SQLite consistency or source mutation.
//! Failure modes are specified first in `tests/capture_files.rs`.
//!
//! The explicitly selected root must not be a symlink. Ambient ancestors may be
//! OS aliases (for example macOS `/var`); they are resolved before establishing
//! authority. Descendant symlinks are rejected. A selected file authorizes only
//! itself, with its parent used as the traversal anchor. Unix opens each relative
//! component through directory descriptors with `O_NOFOLLOW`. Windows validates
//! reparse points before and after opening, but std does not expose a comparable
//! descriptor-relative traversal; hostile concurrent directory replacement is a
//! remaining platform limitation, not a promised security boundary.

use std::{
    fs::{self, File, Metadata, OpenOptions},
    io::{Read, Write},
    path::{Component, Path, PathBuf},
    time::SystemTime,
};

use sha2::{Digest, Sha256};

use crate::{Error, Result};

mod git;
pub(crate) use git::read_git_file;

const BUFFER_BYTES: usize = 64 * 1024;

/// Evidence from a full bounded read, tied to the file which supplied those bytes.
#[derive(Clone, Debug)]
pub struct FileSnapshot {
    pub digest: String,
    pub byte_len: u64,
    path: PathBuf,
    mark: FileMark,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct FileMark {
    len: u64,
    modified: Option<SystemTime>,
    created: Option<SystemTime>,
    #[cfg(unix)]
    identity: (u64, u64),
    #[cfg(unix)]
    changed: (i64, i64),
}

impl FileMark {
    fn from(metadata: &Metadata) -> Self {
        #[cfg(unix)]
        use std::os::unix::fs::MetadataExt;
        Self {
            len: metadata.len(),
            modified: metadata.modified().ok(),
            created: metadata.created().ok(),
            #[cfg(unix)]
            identity: (metadata.dev(), metadata.ino()),
            #[cfg(unix)]
            changed: (metadata.ctime(), metadata.ctime_nsec()),
        }
    }
}

struct AuthorizedPath {
    path: PathBuf,
    root: PathBuf,
    mark: FileMark,
}

/// Checks root access without enumerating it or following a selected symlink.
pub(crate) fn root_readable(path: &Path) -> bool {
    if !fs::symlink_metadata(path).is_ok_and(|metadata| {
        reject_link(&metadata).is_ok() && (metadata.is_dir() || metadata.is_file())
    }) {
        return false;
    }
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC);
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        // Directory handles require BACKUP_SEMANTICS; OPEN_REPARSE_POINT keeps
        // the selected root from becoming a followed junction during this open.
        options.custom_flags(0x0200_0000 | 0x0020_0000);
    }
    options.open(path).is_ok()
}

/// Hashes a regular file without trusting a timestamp/length freshness shortcut.
/// Appends, replacement and truncation during the bounded read reject the result.
pub fn inspect(path: &Path, root: &Path, cancel: &dyn Fn() -> bool) -> Result<FileSnapshot> {
    cancelled(cancel)?;
    let (authorized, mut input) = open_source(path, root)?;
    let digest = read_prefix(&mut input, authorized.mark.len, None, cancel)?;
    validate_after(&input, path, root, &authorized)?;
    cancelled(cancel)?;
    Ok(FileSnapshot {
        digest,
        byte_len: authorized.mark.len,
        path: authorized.path,
        mark: authorized.mark,
    })
}

/// Writes only an inspected version and verifies its digest before returning.
/// On any error the output may contain provisional bytes; callers must discard
/// their unpublished revision. Destination write errors remain ordinary I/O errors.
pub fn copy_verified(
    path: &Path,
    root: &Path,
    expected: &FileSnapshot,
    output: &mut dyn Write,
    cancel: &dyn Fn() -> bool,
) -> Result<()> {
    cancelled(cancel)?;
    let (authorized, mut input) = open_source(path, root).map_err(changed_if_missing)?;
    if authorized.path != expected.path || authorized.mark != expected.mark {
        return Err(changed("file metadata or identity no longer matches inspection"));
    }
    let digest = read_prefix(&mut input, expected.byte_len, Some(output), cancel)?;
    validate_after(&input, path, root, &authorized)?;
    if digest != expected.digest {
        return Err(changed("file contents no longer match inspection"));
    }
    cancelled(cancel)
}

/// Establishes an approved canonical file path without following source symlinks.
/// Capture must still validate the open handle; an inventory path is not a lease.
pub fn ensure_source_path(path: &Path, root: &Path) -> Result<PathBuf> {
    Ok(authorize(path, root)?.path)
}

fn authorize(path: &Path, root: &Path) -> Result<AuthorizedPath> {
    let selected = absolute(root)?;
    let candidate = absolute(path)?;
    let metadata = fs::symlink_metadata(&selected)?;
    reject_link(&metadata)?;
    let selected_file = metadata.is_file();
    if !selected_file && !metadata.is_dir() {
        return Err(Error::InvalidInput("source root is not a regular file or directory".into()));
    }
    let anchor = if selected_file {
        selected.parent().ok_or_else(|| Error::InvalidInput("source has no parent".into()))?
    } else {
        selected.as_path()
    };
    let canonical_root = fs::canonicalize(anchor)?;
    let selected_canonical = if selected_file {
        canonical_root.join(
            selected
                .file_name()
                .ok_or_else(|| Error::InvalidInput("source has no filename".into()))?,
        )
    } else {
        canonical_root.clone()
    };
    if selected_file && candidate != selected && candidate != selected_canonical {
        return Err(Error::InvalidInput("a file source authorizes only the selected file".into()));
    }
    let relative = candidate
        .strip_prefix(anchor)
        .or_else(|_| candidate.strip_prefix(&canonical_root))
        .map_err(|_| Error::InvalidInput("source path is outside the selected root".into()))?;
    if relative.as_os_str().is_empty() {
        return Err(Error::InvalidInput("source path must name a regular file".into()));
    }
    let mut resolved = canonical_root.clone();
    let mut components = relative.components().peekable();
    while let Some(component) = components.next() {
        let Component::Normal(name) = component else {
            return Err(Error::InvalidInput(
                "source path contains a non-relative component".into(),
            ));
        };
        resolved.push(name);
        let metadata = fs::symlink_metadata(&resolved)?;
        reject_link(&metadata)?;
        if components.peek().is_some() && !metadata.is_dir() {
            return Err(Error::InvalidInput("source ancestor is not a directory".into()));
        }
    }
    let file_metadata = fs::symlink_metadata(&resolved)?;
    reject_link(&file_metadata)?;
    if !file_metadata.is_file() {
        return Err(Error::InvalidInput("source is not a regular file".into()));
    }
    let confirmed_root = fs::symlink_metadata(&selected)?;
    reject_link(&confirmed_root)?;
    if !same_identity(&metadata, &confirmed_root) || fs::canonicalize(anchor)? != canonical_root {
        return Err(changed("selected root changed while validating its path"));
    }
    let canonical_path = fs::canonicalize(&resolved)?;
    if !canonical_path.starts_with(&canonical_root) {
        return Err(Error::InvalidInput("source path escaped its approved root".into()));
    }
    Ok(AuthorizedPath {
        path: canonical_path,
        root: canonical_root,
        mark: FileMark::from(&file_metadata),
    })
}

fn absolute(path: &Path) -> Result<PathBuf> {
    let joined =
        if path.is_absolute() { path.to_owned() } else { std::env::current_dir()?.join(path) };
    let mut normalized = PathBuf::new();
    for component in joined.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                if !normalized.pop() {
                    return Err(Error::InvalidInput(
                        "source path escapes the filesystem root".into(),
                    ));
                }
            }
            other => normalized.push(other.as_os_str()),
        }
    }
    Ok(normalized)
}

fn reject_link(metadata: &Metadata) -> Result<()> {
    #[cfg(windows)]
    let reparse = {
        use std::os::windows::fs::MetadataExt;
        metadata.file_attributes() & 0x400 != 0 // FILE_ATTRIBUTE_REPARSE_POINT
    };
    #[cfg(not(windows))]
    let reparse = false;
    if metadata.file_type().is_symlink() || reparse {
        Err(Error::InvalidInput("source symlinks and reparse points are not followed".into()))
    } else {
        Ok(())
    }
}

fn same_identity(left: &Metadata, right: &Metadata) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        left.dev() == right.dev() && left.ino() == right.ino()
    }
    #[cfg(not(unix))]
    {
        left.file_type() == right.file_type() && left.created().ok() == right.created().ok()
    }
}

fn open_source(path: &Path, root: &Path) -> Result<(AuthorizedPath, File)> {
    let authorized = authorize(path, root)?;
    let input = open_regular(&authorized)?;
    let metadata = input.metadata()?;
    if !metadata.is_file() || FileMark::from(&metadata) != authorized.mark {
        return Err(changed("opened file differs from the authorized source"));
    }
    Ok((authorized, input))
}

#[cfg(unix)]
fn open_regular(authorized: &AuthorizedPath) -> Result<File> {
    use std::{
        ffi::CString,
        os::{
            fd::{AsRawFd, FromRawFd},
            unix::{ffi::OsStrExt, fs::OpenOptionsExt},
        },
    };
    let mut directory = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(&authorized.root)?;
    let relative = authorized
        .path
        .strip_prefix(&authorized.root)
        .map_err(|_| Error::InvalidInput("source path escaped its traversal root".into()))?;
    let mut components = relative.components().peekable();
    while let Some(component) = components.next() {
        let Component::Normal(name) = component else {
            return Err(Error::InvalidInput("invalid relative source path".into()));
        };
        let name = CString::new(name.as_bytes())
            .map_err(|_| Error::InvalidInput("source path contains a NUL byte".into()))?;
        let last = components.peek().is_none();
        let flags = libc::O_RDONLY
            | libc::O_NOFOLLOW
            | libc::O_CLOEXEC
            | if last { libc::O_NONBLOCK } else { libc::O_DIRECTORY };
        // SAFETY: directory owns a live directory descriptor; name is a valid
        // NUL-terminated component. The returned descriptor is owned below.
        let descriptor = unsafe { libc::openat(directory.as_raw_fd(), name.as_ptr(), flags) };
        if descriptor < 0 {
            return Err(std::io::Error::last_os_error().into());
        }
        // SAFETY: successful openat returned a new descriptor not owned elsewhere.
        let opened = unsafe { File::from_raw_fd(descriptor) };
        if last {
            return Ok(opened);
        }
        directory = opened;
    }
    Err(Error::InvalidInput("source path has no file component".into()))
}

#[cfg(not(unix))]
fn open_regular(authorized: &AuthorizedPath) -> Result<File> {
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        options.custom_flags(0x00200000); // FILE_FLAG_OPEN_REPARSE_POINT
    }
    let file = options.open(&authorized.path)?;
    reject_link(&file.metadata()?)?;
    Ok(file)
}

fn read_prefix(
    input: &mut File,
    byte_len: u64,
    mut output: Option<&mut dyn Write>,
    cancel: &dyn Fn() -> bool,
) -> Result<String> {
    let mut digest = Sha256::new();
    let mut buffer = [0u8; BUFFER_BYTES];
    let mut remaining = byte_len;
    while remaining > 0 {
        cancelled(cancel)?;
        let amount = remaining.min(BUFFER_BYTES as u64) as usize;
        let read = input.read(&mut buffer[..amount])?;
        if read == 0 {
            return Err(changed("source was truncated during capture"));
        }
        digest.update(&buffer[..read]);
        if let Some(writer) = output.as_deref_mut() {
            writer.write_all(&buffer[..read])?;
        }
        remaining -= read as u64;
    }
    cancelled(cancel)?;
    if input.read(&mut buffer[..1])? != 0 {
        return Err(changed("source grew during capture"));
    }
    Ok(format!("{:x}", digest.finalize()))
}

fn validate_after(input: &File, path: &Path, root: &Path, before: &AuthorizedPath) -> Result<()> {
    let after = authorize(path, root).map_err(changed_if_missing)?;
    if after.path != before.path
        || after.root != before.root
        || after.mark != before.mark
        || FileMark::from(&input.metadata()?) != before.mark
    {
        return Err(changed("source changed while its bytes were read"));
    }
    Ok(())
}

fn changed_if_missing(error: Error) -> Error {
    match error {
        Error::Io(error) if error.kind() == std::io::ErrorKind::NotFound => {
            changed("source disappeared after inspection")
        }
        other => other,
    }
}

fn changed(reason: &str) -> Error {
    Error::SourceChanged(reason.into())
}

fn cancelled(cancel: &dyn Fn() -> bool) -> Result<()> {
    if cancel() { Err(Error::Cancelled) } else { Ok(()) }
}
