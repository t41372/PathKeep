//! Replacing a small file so that a crash leaves either the old contents or the new ones.
//!
//! Responsibilities: write the new contents to a temporary file next to the target, make them
//! durable, rename the temporary file over the target, and make the rename durable. The key
//! envelope and the search index's `meta.json` and `.managed.json` use this.
//!
//! Not responsible for: encrypting anything (callers pass bytes that are already ciphertext),
//! or serializing concurrent writers to the same target. Each target has one writer: the vault
//! owner for the envelope, and Tantivy's own locks for its files.
//!
//! How it can fail:
//! - A crash after the target was truncated but before the new bytes were written leaves an
//!   empty or partial file. The target is never opened for writing; only `rename` touches it,
//!   and POSIX `rename` replaces the directory entry atomically.
//! - The rename reaches disk before the new file's data, so after a power cut the target names
//!   an empty file. The temporary file is fsynced before the rename.
//! - The rename itself is lost on a power cut. The directory is fsynced after it. (On Windows,
//!   directories cannot be fsynced; `MoveFileEx`'s write-through behaviour is what `std` uses.)
//! - Two writers share one temporary file and interleave. The temporary name is fixed per
//!   target (`.<name>.pk-tmp`), which is safe only because each target has one writer; this
//!   also means a crash leaves at most one stale temporary file per target, and the next write
//!   truncates it.
//! - A stale temporary file leaks the previous plaintext. Callers only ever pass ciphertext.
//!
//! `tests/envelope_crash.rs` kills a child process at each step (temporary file written,
//! temporary file synced, renamed, directory synced) and checks that the envelope opens.

use std::ffi::OsString;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use crate::crash_point::crash_point;

/// Atomically replaces `path` with `contents`.
pub(crate) fn replace(path: &Path, contents: &[u8]) -> io::Result<()> {
    let temp = temp_path(path);
    let mut file = OpenOptions::new().write(true).create(true).truncate(true).open(&temp)?;
    file.write_all(contents)?;
    crash_point("atomic-replace/temp-written", path);
    file.sync_all()?;
    drop(file);
    crash_point("atomic-replace/temp-synced", path);
    fs::rename(&temp, path)?;
    crash_point("atomic-replace/renamed", path);
    sync_parent_directory(path)?;
    crash_point("atomic-replace/directory-synced", path);
    Ok(())
}

/// The temporary file `replace` uses for `path`: `.<file name>.pk-tmp` in the same directory,
/// so the rename never crosses file systems.
pub(crate) fn temp_path(path: &Path) -> PathBuf {
    let mut name = OsString::from(".");
    name.push(path.file_name().expect("atomic replacement target has a file name"));
    name.push(".pk-tmp");
    path.with_file_name(name)
}

/// Makes a rename or a new directory entry durable.
pub(crate) fn sync_parent_directory(path: &Path) -> io::Result<()> {
    match path.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => sync_directory(parent),
        _ => sync_directory(Path::new(".")),
    }
}

/// Fsyncs a directory, so the entries created or renamed in it survive a power cut.
#[cfg(unix)]
pub(crate) fn sync_directory(dir: &Path) -> io::Result<()> {
    File::open(dir)?.sync_all()
}

#[cfg(not(unix))]
pub(crate) fn sync_directory(_dir: &Path) -> io::Result<()> {
    Ok(())
}
