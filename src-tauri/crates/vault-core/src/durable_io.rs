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
//! - Sweep orphaned `.pk-durable-*` temps left when the process is `SIGKILL`ed
//!   between temp-create and persist (Drop can't run on SIGKILL).
//!
//! ## Not responsible for
//! - SQLite-internal durability (PRAGMA synchronous / WAL checkpointing).
//! - Cross-process serialization (see the archive write-lock). Callers MUST quiesce
//!   other readers/writers before `install_file_durably`: the rename swaps the inode,
//!   and a reader holding the old fd would otherwise read a now-detached file
//!   (split-brain). The cross-process archive lock is what provides that quiescence.
//!
//! ## The barrier
//! `write(temp) -> full_fsync(temp) -> rename(temp, dest) -> full_fsync(parent dir)`.
//! On macOS a plain `fsync`/`sync_all` only reaches the drive's write cache, not the
//! platter, so a power loss can still lose it; `fcntl(fd, F_FULLFSYNC)` is the only
//! real barrier and is what we use for BOTH the data write and the parent-directory
//! metadata write (the rename is a directory-metadata change, so the dir fsync is
//! what makes the rename itself durable). Filesystems that don't support F_FULLFSYNC
//! (exFAT / SMB / NFS / some external/network volumes — and the archive location is
//! user-choosable) degrade to a best-effort `sync_all` instead of hard-failing, the
//! same dance SQLite's `os_unix.c` does.

use anyhow::{Context, Result};
use std::fs::{self, File};
use std::io::{self, BufWriter, Write};
use std::path::Path;
use std::time::{Duration, SystemTime};

// --- test-only fault-injection seam --------------------------------------------------------------
//
// This seam stays in the MEASURED (coverage/CI) build on purpose: the quality gate forbids
// `cfg`-compiling production I/O out of the coverage binary, because that hides the real failure
// modes. So the REAL barrier (`real_full_fsync` -> `F_FULLFSYNC`/`sync_all`) and the REAL recovery
// decision (`handle_fsync_errno`) always run; only a TEST may pre-load a directive that makes
// `full_fsync` take the recovery branch *as if* `fcntl` had returned a given `errno`. The
// thread-local is read on every call (production default = "no fault, run the real barrier") and is
// written only by the `#[cfg(all(unix, test))]` setter below, so test threads are isolated from each
// other and from production with zero behavioural change at runtime.

#[cfg(unix)]
use std::cell::RefCell;
#[cfg(unix)]
use std::collections::VecDeque;

#[cfg(unix)]
thread_local! {
    /// FIFO of per-`full_fsync`-call directives. Each `full_fsync` pops the front: `Some(errno)`
    /// drives the recovery code as if `fcntl(F_FULLFSYNC)` had failed with that errno; `None` (or an
    /// empty queue) runs the real barrier. Always empty in production.
    static FSYNC_FAULTS: RefCell<VecDeque<Option<i32>>> = const { RefCell::new(VecDeque::new()) };
}

/// Pops the next injected `full_fsync` directive. `None` => run the real barrier (always so in
/// production, where the queue is never written).
#[cfg(unix)]
fn next_fsync_fault() -> Option<i32> {
    FSYNC_FAULTS.with(|queue| queue.borrow_mut().pop_front().flatten())
}

/// Queues per-`full_fsync` fault directives for the current test thread (replacing any prior queue).
#[cfg(all(unix, test))]
pub(crate) fn inject_fsync_faults(faults: Vec<Option<i32>>) {
    FSYNC_FAULTS.with(|queue| {
        let mut queue = queue.borrow_mut();
        queue.clear();
        queue.extend(faults);
    });
}

/// `true` when `errno` means the filesystem/handle cannot provide a real platter barrier, so we
/// degrade to a best-effort `sync_all` rather than hard-failing a user-chosen archive location
/// forever. Mirrors SQLite's `os_unix.c` handling of `F_FULLFSYNC`. (Comparisons, not a `matches!`
/// pattern, because `ENOTSUP == EOPNOTSUPP` on Linux — a duplicate match arm is an
/// unreachable-pattern warning under `-D warnings`.)
#[cfg(unix)]
fn is_unsupported_fsync_errno(errno: i32) -> bool {
    errno == libc::ENOTSUP
        || errno == libc::EOPNOTSUPP
        || errno == libc::EINVAL
        || errno == libc::ENOSYS
}

/// Maps a failed barrier `errno` to either a best-effort `sync_all` fallback (unsupported
/// filesystems) or a propagated fatal error. Shared by the real macOS `fcntl` error arm and the test
/// seam so both exercise the identical recovery decision.
#[cfg(unix)]
fn handle_fsync_errno(file: &File, errno: i32) -> Result<()> {
    if is_unsupported_fsync_errno(errno) {
        // Best-effort: `sync_all` still reaches the drive cache, the strongest guarantee these
        // filesystems offer. Durable-enough beats a permanent hard failure on exFAT/SMB/NFS.
        return file.sync_all().context("sync_all fallback after F_FULLFSYNC unsupported");
    }
    Err(io::Error::from_raw_os_error(errno)).context("F_FULLFSYNC failed")
}

/// Performs the real platter barrier for `file`, with no fault injection.
#[cfg(target_os = "macos")]
fn real_full_fsync(file: &File) -> Result<()> {
    use std::os::unix::io::AsRawFd;
    // SAFETY: the fd is owned by `file` and stays valid for this call.
    let rc = unsafe { libc::fcntl(file.as_raw_fd(), libc::F_FULLFSYNC) };
    if rc == -1 {
        let errno = io::Error::last_os_error().raw_os_error().unwrap_or(libc::EIO);
        return handle_fsync_errno(file, errno);
    }
    Ok(())
}

/// Performs the real platter barrier for `file`, with no fault injection.
#[cfg(not(target_os = "macos"))]
fn real_full_fsync(file: &File) -> Result<()> {
    file.sync_all().context("sync_all failed")
}

/// Flushes a file's (or directory's) data + metadata all the way to stable storage.
///
/// macOS `File::sync_all` only reaches the drive cache; `F_FULLFSYNC` is the real platter barrier.
/// Every other platform uses `sync_all`. On macOS, an `F_FULLFSYNC` rejection from a filesystem that
/// doesn't support it degrades to a best-effort `sync_all` (see [`handle_fsync_errno`]); only a
/// genuinely fatal errno propagates.
fn full_fsync(file: &File) -> Result<()> {
    #[cfg(unix)]
    {
        if let Some(errno) = next_fsync_fault() {
            return handle_fsync_errno(file, errno);
        }
    }
    real_full_fsync(file)
}

/// Fsyncs the directory at `dir` itself (its own metadata), routing the handle through the same
/// platter barrier as data. Used to make a freshly-populated directory's new entries durable — e.g.
/// the cross-directory `rename` that moves the canonical DBs into `quarantine/<ts>/` only becomes
/// crash-durable once the destination directory is fsynced (otherwise a power loss can leave the
/// moved file in NEITHER directory). On a filesystem that rejects F_FULLFSYNC on a directory fd it
/// degrades to `sync_all` (see `handle_fsync_errno`). A no-op on non-unix targets.
#[cfg(unix)]
pub(crate) fn fsync_dir(dir: &Path) -> Result<()> {
    let handle =
        File::open(dir).with_context(|| format!("open dir {} for fsync", dir.display()))?;
    full_fsync(&handle).with_context(|| format!("fsync dir {}", dir.display()))
}

#[cfg(not(unix))]
pub(crate) fn fsync_dir(_dir: &Path) -> Result<()> {
    Ok(())
}

/// Fsyncs the directory that holds `path`, so a freshly renamed entry survives a crash. Delegates to
/// [`fsync_dir`] (same platter barrier as data); if the filesystem rejects `F_FULLFSYNC` on a
/// directory fd it degrades to `sync_all`. A no-op on non-unix targets (Windows rename durability is
/// handled by the OS differently and there is no portable directory fsync).
#[cfg(unix)]
fn fsync_parent_dir(path: &Path) -> Result<()> {
    let dir = path.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or_else(|| Path::new("."));
    fsync_dir(dir)
}

#[cfg(not(unix))]
fn fsync_parent_dir(_path: &Path) -> Result<()> {
    Ok(())
}

/// Atomically + durably writes `contents` to `path`.
///
/// Writes to a uniquely-named temp in the SAME directory (so the final rename is atomic on one
/// filesystem), fsyncs the temp to the platter, renames it onto the destination, then fsyncs the
/// directory. A reader either sees the old file or the whole new file; a crash never leaves a
/// half-written `path`. The destination inherits the temp's permissions (0600 from
/// `tempfile::Builder`), which is the intended mode for config/salt files.
pub fn atomic_durable_write(path: &Path, contents: &[u8]) -> Result<()> {
    atomic_durable_write_with(path, |writer| {
        writer.write_all(contents).context("write temp contents")?;
        Ok(())
    })
}

/// Atomically + durably writes a file whose contents `write` produces piece by piece.
///
/// For files too large to build in memory first (a 14.4M-visit history export is gigabytes).
/// `write` gets a buffered writer over the same-directory temp; the barrier afterwards is the one
/// [`atomic_durable_write`] uses. If `write` fails, the temp is deleted and `path` is untouched.
pub fn atomic_durable_write_with(
    path: &Path,
    write: impl FnOnce(&mut dyn Write) -> Result<()>,
) -> Result<()> {
    let parent =
        path.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or_else(|| Path::new("."));
    let tmp = tempfile::Builder::new()
        .prefix(".pk-durable-")
        .tempfile_in(parent)
        .with_context(|| format!("create temp beside {}", path.display()))?;
    {
        let mut writer = BufWriter::new(tmp.as_file());
        write(&mut writer)?;
        writer.flush().context("flush temp")?;
    }
    full_fsync(tmp.as_file())?;
    // persist() = rename onto the destination (atomic on the same filesystem).
    tmp.persist(path)
        .map_err(|error| error.error)
        .with_context(|| format!("rename temp onto {}", path.display()))?;
    fsync_parent_dir(path)
}

/// Durably installs an already-built file (`built`) as `dest`.
///
/// Used when the new contents were produced out-of-band (e.g. a SQLCipher export temp): fsync the
/// built file to the platter, rename it onto `dest`, then fsync the directory.
///
/// Contract:
/// - `built` MUST already live on the same filesystem AND same directory as `dest`: the rename must
///   be atomic (same filesystem) and only `dest`'s parent directory is fsynced, so if `built` lived
///   in a different directory that directory's `unlink` half of the rename would not be made durable.
///   Callers that genuinely need a cross-directory install must fsync both parents themselves.
/// - The rename replaces `dest` with `built`, so `dest` ends up with `built`'s mode (0600 for our
///   build temps) and a fresh inode — fine for config/salt, and the reason callers must quiesce open
///   readers first (see the module-level "Not responsible for" note).
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

/// Flushes the already-built file at `path` to the platter (no rename).
///
/// WHY: `apply_import` swaps the two canonical databases back-to-back behind a
/// durable interrupted-import marker. The slow platter barrier (`F_FULLFSYNC` on
/// macOS) is paid HERE, in a "prepare" pass over the staged copies, so the swap
/// renames themselves issue with that barrier already paid — shrinking the window
/// in which a crash leaves one canonical DB swapped and the other not.
pub(crate) fn fsync_file_durably(path: &Path) -> Result<()> {
    let file = File::open(path).with_context(|| format!("open {} for fsync", path.display()))?;
    full_fsync(&file)
}

/// Durably removes the file at `path`: a no-op when already absent (idempotent),
/// otherwise unlink + fsync the parent directory so the removal survives a crash.
///
/// WHY: `apply_import` clears the interrupted-import marker at its commit point. If
/// that unlink reached only the directory's write cache, a power loss could
/// RESURRECT the marker and crash-recovery would then roll back an import that
/// actually committed. Fsyncing the parent makes "the marker is gone" durable, so
/// recovery is a no-op exactly when the import succeeded.
pub fn remove_file_durably(path: &Path) -> Result<()> {
    if !path.exists() {
        return Ok(());
    }
    fs::remove_file(path).with_context(|| format!("remove {}", path.display()))?;
    fsync_parent_dir(path)
}

/// Removes orphaned `.pk-durable-*` temp files left in `dir` when the process was killed between
/// temp-create and persist (a `SIGKILL` skips `NamedTempFile`'s Drop, so the temp leaks), but only
/// those not written to for `min_age`: another write may be in progress in the same directory (two
/// history exports at once), and a temp that is still being written keeps a fresh modification
/// time. Returns the number removed. A missing `dir` is not an error (nothing to sweep). Nothing
/// sweeps the archive directory at startup yet; history export sweeps its own directory by age.
pub fn sweep_temps_older_than(dir: &Path, min_age: Duration) -> Result<usize> {
    let now = SystemTime::now();
    sweep_temps_matching(dir, |entry| {
        entry
            .metadata()
            .and_then(|metadata| metadata.modified())
            .ok()
            .and_then(|modified| now.duration_since(modified).ok())
            .is_some_and(|age| age >= min_age)
    })
}

fn sweep_temps_matching(dir: &Path, stale: impl Fn(&fs::DirEntry) -> bool) -> Result<usize> {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(0),
        Err(error) => {
            return Err(error).with_context(|| format!("scan {} for stale temps", dir.display()));
        }
    };
    let mut removed = 0usize;
    for entry in entries {
        let entry = entry.with_context(|| format!("read entry in {}", dir.display()))?;
        if entry.file_name().to_string_lossy().starts_with(".pk-durable-") && stale(&entry) {
            let temp = entry.path();
            fs::remove_file(&temp)
                .with_context(|| format!("remove stale temp {}", temp.display()))?;
            removed += 1;
        }
    }
    Ok(removed)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An export that fails halfway (archive locked, disk full) must not leave a truncated file
    /// where the old one was, nor a partial temp.
    #[test]
    fn a_failed_streamed_write_keeps_the_old_file_and_removes_the_temp() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("export.html");
        fs::write(&path, b"old export").expect("seed");
        let error = atomic_durable_write_with(&path, |writer| {
            writer.write_all(b"<html><body>half")?;
            anyhow::bail!("archive went away")
        })
        .expect_err("the writer's error propagates");

        assert!(error.to_string().contains("archive went away"));
        assert_eq!(fs::read(&path).expect("read"), b"old export");
        assert_eq!(fs::read_dir(dir.path()).expect("list").count(), 1, "partial temp left behind");
    }

    #[test]
    fn age_gated_sweep_leaves_temps_that_are_still_being_written() {
        let dir = tempfile::tempdir().expect("tempdir");
        fs::write(dir.path().join(".pk-durable-live"), b"in progress").expect("seed temp");
        fs::write(dir.path().join("export.md"), b"keep").expect("seed export");

        assert_eq!(
            sweep_temps_older_than(dir.path(), Duration::from_secs(3600)).expect("sweep"),
            0
        );
        assert!(dir.path().join(".pk-durable-live").exists());
        assert_eq!(sweep_temps_older_than(dir.path(), Duration::ZERO).expect("sweep"), 1);
        assert!(!dir.path().join(".pk-durable-live").exists());
        assert!(dir.path().join("export.md").exists());
    }

    // --- Barrier fault paths via the seam -----------------
    //
    // These drive the SAME production recovery code (`handle_fsync_errno`, the dir-fsync arm) that
    // the real disk barrier runs, without needing a filesystem that actually rejects F_FULLFSYNC.

    #[cfg(unix)]
    #[test]
    fn full_fsync_propagates_a_genuinely_fatal_errno() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("config.json");
        inject_fsync_faults(vec![Some(libc::EIO)]);
        let error = atomic_durable_write(&path, b"x").expect_err("fatal fsync must propagate");
        assert!(error.to_string().contains("F_FULLFSYNC failed"));
        // A fatal data-barrier failure must abort BEFORE the rename, so no destination appears.
        assert!(!path.exists(), "must not publish a file whose data was never made durable");
    }

    #[cfg(unix)]
    #[test]
    fn fsync_parent_dir_reports_directory_fsync_failure() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("config.json");
        // 1st full_fsync (temp) runs for real; 2nd (the directory) hits a fatal injected errno.
        inject_fsync_faults(vec![None, Some(libc::EIO)]);
        let error = atomic_durable_write(&path, b"x").expect_err("dir fsync failure must surface");
        assert!(error.to_string().contains("fsync dir"));
    }
}
