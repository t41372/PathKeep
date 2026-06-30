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
use std::io::{self, Write};
use std::path::Path;

// --- test-only fault-injection seam --------------------------------------------------------------
//
// This seam stays in the MEASURED (coverage/CI) build on purpose: the quality gate forbids
// `cfg`-compiling production I/O out of the coverage binary, because that hides the real failure
// modes. So the REAL barrier (`real_full_fsync` -> `F_FULLFSYNC`/`sync_all`) and the REAL recovery
// decision (`handle_fsync_errno`) always run; only a TEST may pre-load a directive that makes
// `full_fsync` take the recovery branch *as if* `fcntl` had returned a given `errno`, or make a
// directory open fail. The thread-locals are read on every call (production default = "no fault, run
// the real barrier") and are written only by the `#[cfg(all(unix, test))]` setters below, so test
// threads are isolated from each other and from production with zero behavioural change at runtime.

#[cfg(unix)]
use std::cell::{Cell, RefCell};
#[cfg(unix)]
use std::collections::VecDeque;

#[cfg(unix)]
thread_local! {
    /// FIFO of per-`full_fsync`-call directives. Each `full_fsync` pops the front: `Some(errno)`
    /// drives the recovery code as if `fcntl(F_FULLFSYNC)` had failed with that errno; `None` (or an
    /// empty queue) runs the real barrier. Always empty in production.
    static FSYNC_FAULTS: RefCell<VecDeque<Option<i32>>> = const { RefCell::new(VecDeque::new()) };
    /// When set, `fsync_parent_dir` fails its directory open with this errno before touching the
    /// filesystem. `None` (production default) opens the directory for real.
    static DIR_OPEN_FAULT: Cell<Option<i32>> = const { Cell::new(None) };
}

/// Pops the next injected `full_fsync` directive. `None` => run the real barrier (always so in
/// production, where the queue is never written).
#[cfg(unix)]
fn next_fsync_fault() -> Option<i32> {
    FSYNC_FAULTS.with(|queue| queue.borrow_mut().pop_front().flatten())
}

/// Reads the injected directory-open directive. `None` => open the directory for real (always so in
/// production).
#[cfg(unix)]
fn dir_open_fault() -> Option<i32> {
    DIR_OPEN_FAULT.with(Cell::get)
}

/// Queues per-`full_fsync` fault directives for the current test thread (replacing any prior queue).
#[cfg(all(unix, test))]
fn inject_fsync_faults(faults: Vec<Option<i32>>) {
    FSYNC_FAULTS.with(|queue| {
        let mut queue = queue.borrow_mut();
        queue.clear();
        queue.extend(faults);
    });
}

/// Forces the next `fsync_parent_dir` directory open to fail with `errno` on the current test thread.
#[cfg(all(unix, test))]
fn inject_dir_open_fault(errno: i32) {
    DIR_OPEN_FAULT.with(|cell| cell.set(Some(errno)));
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

/// Fsyncs the directory that holds `path`, so a freshly renamed entry survives a crash. Routes the
/// directory handle through [`full_fsync`] (same platter barrier as data); if the filesystem rejects
/// `F_FULLFSYNC` on a directory fd it degrades to `sync_all`. A no-op on non-unix targets (Windows
/// rename durability is handled by the OS differently and there is no portable directory fsync).
#[cfg(unix)]
fn fsync_parent_dir(path: &Path) -> Result<()> {
    let dir = path.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or_else(|| Path::new("."));
    if let Some(errno) = dir_open_fault() {
        return Err(io::Error::from_raw_os_error(errno))
            .with_context(|| format!("open dir {} for fsync", dir.display()));
    }
    let handle =
        File::open(dir).with_context(|| format!("open dir {} for fsync", dir.display()))?;
    full_fsync(&handle).with_context(|| format!("fsync dir {}", dir.display()))
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

/// Removes orphaned `.pk-durable-*` temp files left in `dir` when the process was killed between
/// temp-create and persist (a `SIGKILL` skips `NamedTempFile`'s Drop, so the temp leaks). Returns
/// the number removed. A missing `dir` is not an error (nothing to sweep). Startup / archive-open
/// paths call this so leaked temps don't accumulate in the user's archive directory.
pub fn sweep_stale_temps(dir: &Path) -> Result<usize> {
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
        if entry.file_name().to_string_lossy().starts_with(".pk-durable-") {
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

    // --- MEDIUM-4: orphaned temp sweep -----------------------------------------------------------

    #[test]
    fn sweep_stale_temps_removes_only_durable_temps_and_counts_them() {
        let dir = tempfile::tempdir().expect("tempdir");
        fs::write(dir.path().join(".pk-durable-abc"), b"leak").expect("seed temp a");
        fs::write(dir.path().join(".pk-durable-xyz"), b"leak").expect("seed temp b");
        fs::write(dir.path().join("config.json"), b"keep").expect("seed config");

        let removed = sweep_stale_temps(dir.path()).expect("sweep");
        assert_eq!(removed, 2, "both leaked temps should be removed");
        assert!(!dir.path().join(".pk-durable-abc").exists());
        assert!(!dir.path().join(".pk-durable-xyz").exists());
        assert!(dir.path().join("config.json").exists(), "real files must be left alone");
    }

    #[test]
    fn sweep_stale_temps_treats_missing_dir_as_nothing_to_do() {
        let dir = tempfile::tempdir().expect("tempdir");
        let missing = dir.path().join("not-an-archive-dir");
        assert_eq!(sweep_stale_temps(&missing).expect("sweep missing"), 0);
    }

    #[test]
    fn sweep_stale_temps_errors_when_path_is_not_a_directory() {
        let dir = tempfile::tempdir().expect("tempdir");
        let file = dir.path().join("a-file");
        fs::write(&file, b"x").expect("seed file");
        let error = sweep_stale_temps(&file).expect_err("scanning a file must error");
        assert!(error.to_string().contains("scan"));
    }

    // --- HIGH-1 / MEDIUM-3 / HIGH-2 / MEDIUM-5: barrier fault paths via the seam -----------------
    //
    // These drive the SAME production recovery code (`handle_fsync_errno`, the rename arms, the
    // dir-open/dir-fsync arms) that the real disk barrier runs, without needing a filesystem that
    // actually rejects F_FULLFSYNC. The real happy-path fsync still runs (see the test above).

    #[cfg(unix)]
    #[test]
    fn full_fsync_degrades_to_best_effort_on_unsupported_filesystem() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("config.json");
        // First full_fsync call (the temp) reports the fs cannot do F_FULLFSYNC -> sync_all fallback;
        // the write must still succeed with the correct contents.
        inject_fsync_faults(vec![Some(libc::ENOTSUP)]);
        atomic_durable_write(&path, b"degraded-ok").expect("write degrades, not fails");
        assert_eq!(fs::read(&path).expect("read"), b"degraded-ok");
    }

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
    fn atomic_durable_write_reports_rename_failure() {
        let dir = tempfile::tempdir().expect("tempdir");
        // A non-empty directory at the destination path makes the rename (persist) fail reliably,
        // regardless of uid (unlike a read-only dir, which root ignores).
        let dest = dir.path().join("occupied");
        fs::create_dir(&dest).expect("mkdir dest");
        fs::write(dest.join("inner"), b"present").expect("seed dest child");

        let error = atomic_durable_write(&dest, b"data").expect_err("rename onto a dir must fail");
        assert!(error.to_string().contains("rename temp onto"));

        // The failed persist must not leak the temp.
        let leftovers: Vec<_> = fs::read_dir(dir.path())
            .expect("read_dir")
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().starts_with(".pk-durable-"))
            .collect();
        assert!(leftovers.is_empty(), "failed rename leaked a temp");
    }

    #[cfg(unix)]
    #[test]
    fn install_file_durably_reports_rename_failure() {
        let dir = tempfile::tempdir().expect("tempdir");
        let built = dir.path().join("built.tmp");
        fs::write(&built, b"new").expect("seed built");
        let dest = dir.path().join("occupied");
        fs::create_dir(&dest).expect("mkdir dest");
        fs::write(dest.join("inner"), b"present").expect("seed dest child");

        let error = install_file_durably(&built, &dest).expect_err("rename onto a dir must fail");
        assert!(error.to_string().contains("rename"));
    }

    #[cfg(unix)]
    #[test]
    fn fsync_parent_dir_reports_directory_open_failure() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("config.json");
        inject_dir_open_fault(libc::EACCES);
        let error = atomic_durable_write(&path, b"x").expect_err("dir open failure must surface");
        assert!(error.to_string().contains("open dir"));
        assert!(error.to_string().contains("for fsync"));
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
