//! Crash-durable file writes: atomic temp + rename behind a real disk barrier.
//!
//! ## Responsibilities
//! - Persist a file so that after a crash / kill / power-loss the on-disk file is
//!   EITHER the complete old contents OR the complete new contents — never
//!   truncated, partially written, or zeroed.
//! - Provide the single durability barrier the archive write paths were missing.
//!   The 2026-06-30 incident (a rekey that left `config.json` stale and the new
//!   encrypted DB's 16-byte salt zeroed after a power loss) is exactly what
//!   happens when a rename is persisted while the data write behind it is not.
//!
//! ## Not responsible for
//! - SQLite-internal durability (PRAGMA synchronous / WAL checkpointing).
//! - Cross-process serialization (see the archive write-lock).
//!
//! ## The barrier
//! `write(temp) -> full_fsync(temp) -> rename(temp, dest) -> fsync(parent dir)`.
//! On macOS a plain `fsync`/`sync_all` only reaches the drive's write cache, not
//! the platter, so a power loss can still lose it; `fcntl(fd, F_FULLFSYNC)` is the
//! only real barrier and is what we use there. The parent-dir fsync is what makes
//! the rename (a directory-metadata change) itself durable.

use anyhow::{Context, Result};
use std::fs::{self, File};
use std::io::Write;
use std::path::Path;

/// Flushes a file's data + metadata all the way to stable storage.
///
/// macOS `File::sync_all` only reaches the drive cache; `F_FULLFSYNC` is the real
/// platter barrier. Every other platform uses `sync_all`.
fn full_fsync(file: &File) -> Result<()> {
    #[cfg(target_os = "macos")]
    {
        use std::os::unix::io::AsRawFd;
        // SAFETY: the fd is owned by `file` and stays valid for this call.
        let rc = unsafe { libc::fcntl(file.as_raw_fd(), libc::F_FULLFSYNC) };
        if rc == -1 {
            return Err(std::io::Error::last_os_error()).context("F_FULLFSYNC failed");
        }
        Ok(())
    }
    #[cfg(not(target_os = "macos"))]
    {
        file.sync_all().context("sync_all failed")
    }
}

/// Fsyncs the directory that holds `path`, so a freshly renamed entry survives a
/// crash. A no-op on non-unix targets (Windows rename durability is handled by
/// the OS differently and there is no portable directory fsync).
#[cfg(unix)]
fn fsync_parent_dir(path: &Path) -> Result<()> {
    let dir = path.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or_else(|| Path::new("."));
    let handle =
        File::open(dir).with_context(|| format!("open dir {} for fsync", dir.display()))?;
    handle.sync_all().with_context(|| format!("fsync dir {}", dir.display()))
}

#[cfg(not(unix))]
fn fsync_parent_dir(_path: &Path) -> Result<()> {
    Ok(())
}

/// Atomically + durably writes `contents` to `path`.
///
/// Writes to a uniquely-named temp in the SAME directory (so the final rename is
/// atomic on one filesystem), fsyncs the temp to the platter, renames it onto the
/// destination, then fsyncs the directory. A reader either sees the old file or
/// the whole new file; a crash never leaves a half-written `path`.
pub fn atomic_durable_write(path: &Path, contents: &[u8]) -> Result<()> {
    let parent =
        path.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or_else(|| Path::new("."));
    let mut tmp = tempfile::Builder::new()
        .prefix(".pk-durable-")
        .tempfile_in(parent)
        .with_context(|| format!("create temp beside {}", path.display()))?;
    tmp.write_all(contents).context("write temp contents")?;
    tmp.flush().context("flush temp")?;
    full_fsync(tmp.as_file())?;
    // persist() = rename onto the destination (atomic on the same filesystem).
    tmp.persist(path)
        .map_err(|error| error.error)
        .with_context(|| format!("rename temp onto {}", path.display()))?;
    fsync_parent_dir(path)
}

/// Durably installs an already-built file (`built`) as `dest`.
///
/// Used when the new contents were produced out-of-band (e.g. a SQLCipher export
/// temp): fsync the built file to the platter, rename it onto `dest`, then fsync
/// the directory. `built` must already live on the same filesystem as `dest`.
pub fn install_file_durably(built: &Path, dest: &Path) -> Result<()> {
    {
        let file =
            File::open(built).with_context(|| format!("open built file {}", built.display()))?;
        full_fsync(&file)?;
    }
    fs::rename(built, dest)
        .with_context(|| format!("rename {} -> {}", built.display(), dest.display()))?;
    fsync_parent_dir(dest)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn atomic_durable_write_persists_contents_and_leaves_no_temp() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("config.json");

        atomic_durable_write(&path, b"{\"v\":1}").expect("write");
        assert_eq!(fs::read(&path).expect("read"), b"{\"v\":1}");

        // Overwrite durably (the save_config case) — old content fully replaced.
        atomic_durable_write(&path, b"{\"v\":2,\"more\":true}").expect("overwrite");
        assert_eq!(fs::read(&path).expect("read"), b"{\"v\":2,\"more\":true}");

        // No leftover temp files in the directory — only the destination remains.
        let leftovers: Vec<_> = fs::read_dir(dir.path())
            .expect("read_dir")
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|name| name.starts_with(".pk-durable-"))
            .collect();
        assert!(leftovers.is_empty(), "temp not cleaned up: {leftovers:?}");
    }

    #[test]
    fn atomic_durable_write_errors_when_parent_dir_is_missing() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("does-not-exist").join("config.json");
        let error = atomic_durable_write(&path, b"x").expect_err("missing parent must error");
        assert!(error.to_string().contains("create temp beside"));
    }

    #[test]
    fn install_file_durably_renames_built_file_onto_dest() {
        let dir = tempfile::tempdir().expect("tempdir");
        let built = dir.path().join("archive.rekey.tmp");
        let dest = dir.path().join("history-vault.sqlite");
        fs::write(&built, b"SQLite format 3\0...new...").expect("seed built");

        install_file_durably(&built, &dest).expect("install");
        assert_eq!(fs::read(&dest).expect("read dest"), b"SQLite format 3\0...new...");
        assert!(!built.exists(), "built temp should be consumed by the rename");
    }

    #[test]
    fn install_file_durably_errors_when_built_file_is_missing() {
        let dir = tempfile::tempdir().expect("tempdir");
        let built = dir.path().join("nope.tmp");
        let dest = dir.path().join("dest.sqlite");
        let error = install_file_durably(&built, &dest).expect_err("missing built file must error");
        assert!(error.to_string().contains("open built file"));
    }
}
