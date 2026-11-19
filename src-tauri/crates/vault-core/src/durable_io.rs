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
//! - Wait, briefly, for a file another program holds open on Windows before a move, a delete or
//!   the open that flushes a file gives up (`vault_fs`). One held longer makes the operation fail:
//!   before its commit it is undone, after it the next start finishes it.
//! - Flush a file written earlier through a handle Windows flushes: one that may write
//!   (`open_to_flush`).
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
#[cfg(all(unix, any(test, feature = "test-fixtures")))]
pub(crate) fn inject_fsync_faults(faults: Vec<Option<i32>>) {
    FSYNC_FAULTS.with(|queue| {
        let mut queue = queue.borrow_mut();
        queue.clear();
        queue.extend(faults);
    });
}

/// Exercise the complete save-command result on Unix without replacing its actual rename.
#[cfg(all(unix, feature = "test-fixtures"))]
pub fn fail_next_atomic_parent_sync() {
    inject_fsync_faults(vec![None, Some(libc::EIO)]);
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

/// The rename succeeded. Callers publishing settings must apply the visible value even
/// though its survival across power loss is uncertain.
#[derive(Debug, thiserror::Error)]
#[error("replacement is visible but syncing its parent directory failed: {0:#}")]
pub(crate) struct PublishedButUnsynced(#[source] anyhow::Error);

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

/// The live config save distinguishes a failed write from an installed config whose
/// directory flush failed. Inject its E2E fault at that exact boundary.
pub(crate) fn write_live_config(path: &Path, contents: &[u8]) -> Result<()> {
    atomic_write_with_sync(
        path,
        |writer| writer.write_all(contents).context("write temp contents"),
        |path| {
            #[cfg(debug_assertions)]
            crate::fault_inject::checkpoint("config-save-parent-sync")?;
            fsync_parent_dir(path)
        },
    )
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
    atomic_write_with_sync(path, write, fsync_parent_dir)
}

fn atomic_write_with_sync(
    path: &Path,
    write: impl FnOnce(&mut dyn Write) -> Result<()>,
    sync_parent: impl FnOnce(&Path) -> Result<()>,
) -> Result<()> {
    let parent =
        path.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or_else(|| Path::new("."));
    let tmp = tempfile::Builder::new()
        .prefix(vault_layout::family_prefix(vault_layout::ArtifactId::AtomicTemps))
        .tempfile_in(parent)
        .with_context(|| format!("create temp beside {}", path.display()))?;
    {
        let mut writer = BufWriter::new(tmp.as_file());
        write(&mut writer)?;
        writer.flush().context("flush temp")?;
    }
    full_fsync(tmp.as_file())?;
    // persist() = rename onto the destination (atomic on the same filesystem).
    vault_fs::persist(tmp, path).with_context(|| format!("rename temp onto {}", path.display()))?;
    sync_parent(path).map_err(|error| PublishedButUnsynced(error).into())
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
            open_to_flush(built).with_context(|| format!("open built file {}", built.display()))?;
        full_fsync(&file)?;
    }
    vault_fs::rename(built, dest)
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
    let file = open_to_flush(path).with_context(|| format!("open {} for fsync", path.display()))?;
    full_fsync(&file)
}

/// Opens a file written earlier so it can be flushed. Windows flushes only through a handle that
/// may write: `FlushFileBuffers` requires `GENERIC_WRITE` and refuses a read-only handle ("access
/// denied"). Elsewhere a read-only handle flushes the same, and the file is opened as before. A
/// program that holds the file for a moment, as one does right after it is written, is waited for.
fn open_to_flush(path: &Path) -> io::Result<File> {
    vault_fs::retry_while_held(|| fs::OpenOptions::new().read(true).write(cfg!(windows)).open(path))
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
    match vault_fs::remove_file(path) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        removed => removed,
    }
    .with_context(|| format!("remove {}", path.display()))?;
    fsync_parent_dir(path)
}

/// Renames `from` to `to` only if nothing is at `to`. Recovery puts a file back, or moves one in,
/// only where nothing is; a plain rename replaces what is there, so a file that appeared after
/// recovery looked (a sync client, a file browser, a person) would be deleted.
///
/// In one step where the system offers it: `renamex_np` with `RENAME_EXCL` on macOS,
/// `renameat2` with `RENAME_NOREPLACE` on Linux, `MoveFileExW` without
/// `MOVEFILE_REPLACE_EXISTING` on Windows. A file system that rejects the flag (some network and
/// FUSE ones, Linux before 3.15) gets a look first and then the rename: only a program writing into
/// the data folder in that instant is not caught there.
pub(crate) fn rename_no_replace(from: &Path, to: &Path) -> io::Result<()> {
    vault_fs::retry_while_held(|| rename_no_replace_once(from, to))
}

fn rename_no_replace_once(from: &Path, to: &Path) -> io::Result<()> {
    if let Some(result) = exclusive_rename(from, to) {
        return result;
    }
    match fs::symlink_metadata(to) {
        Ok(_) => Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            format!("{} is already there", to.display()),
        )),
        Err(error) if error.kind() == io::ErrorKind::NotFound => fs::rename(from, to),
        Err(error) => Err(error),
    }
}

#[cfg(unix)]
fn c_path(path: &Path) -> io::Result<std::ffi::CString> {
    use std::os::unix::ffi::OsStrExt;
    std::ffi::CString::new(path.as_os_str().as_bytes())
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "a path with a NUL byte"))
}

/// The system's own no-replace rename; `None` when the file system or kernel does not offer it.
#[cfg(target_os = "macos")]
fn exclusive_rename(from: &Path, to: &Path) -> Option<io::Result<()>> {
    let (from, to) = match (c_path(from), c_path(to)) {
        (Ok(from), Ok(to)) => (from, to),
        (Err(error), _) | (_, Err(error)) => return Some(Err(error)),
    };
    // SAFETY: two NUL-terminated paths that outlive the call.
    if unsafe { libc::renamex_np(from.as_ptr(), to.as_ptr(), libc::RENAME_EXCL) } == 0 {
        return Some(Ok(()));
    }
    let error = io::Error::last_os_error();
    match error.raw_os_error() {
        Some(libc::ENOTSUP) => None,
        _ => Some(Err(error)),
    }
}

#[cfg(target_os = "linux")]
fn exclusive_rename(from: &Path, to: &Path) -> Option<io::Result<()>> {
    let (from, to) = match (c_path(from), c_path(to)) {
        (Ok(from), Ok(to)) => (from, to),
        (Err(error), _) | (_, Err(error)) => return Some(Err(error)),
    };
    // The system call itself: glibc wraps it only from 2.28, the kernel has it from 3.15.
    // SAFETY: two NUL-terminated paths that outlive the call; the flag is the documented one.
    let renamed = unsafe {
        libc::syscall(
            libc::SYS_renameat2,
            libc::AT_FDCWD,
            from.as_ptr(),
            libc::AT_FDCWD,
            to.as_ptr(),
            libc::RENAME_NOREPLACE,
        )
    };
    if renamed == 0 {
        return Some(Ok(()));
    }
    let error = io::Error::last_os_error();
    match error.raw_os_error() {
        Some(libc::EINVAL | libc::ENOSYS) => None,
        _ => Some(Err(error)),
    }
}

#[cfg(windows)]
fn exclusive_rename(from: &Path, to: &Path) -> Option<io::Result<()>> {
    use windows_sys::Win32::Storage::FileSystem::MoveFileExW;
    let (from, to) = match (verbatim(from), verbatim(to)) {
        (Ok(from), Ok(to)) => (from, to),
        (Err(error), _) | (_, Err(error)) => return Some(Err(error)),
    };
    // No flags: without MOVEFILE_REPLACE_EXISTING it fails when `to` exists, file or folder.
    // SAFETY: two NUL-terminated wide paths that outlive the call.
    Some(if unsafe { MoveFileExW(from.as_ptr(), to.as_ptr(), 0) } != 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    })
}

/// A path in the `\\?\` form, NUL-terminated, as the standard library passes its own: a data
/// folder deep enough to pass 260 characters is not refused.
#[cfg(windows)]
fn verbatim(path: &Path) -> io::Result<Vec<u16>> {
    use std::os::windows::ffi::OsStrExt;
    let wide: Vec<u16> = std::path::absolute(path)?.as_os_str().encode_wide().collect();
    let utf16 = |text: &str| text.encode_utf16().collect::<Vec<u16>>();
    let mut verbatim = if wide.starts_with(&utf16(r"\\?\")) || wide.starts_with(&utf16(r"\\.\")) {
        wide
    } else if let Some(share) = wide.strip_prefix(&utf16(r"\\")[..]) {
        [utf16(r"\\?\UNC\"), share.to_vec()].concat()
    } else {
        [utf16(r"\\?\"), wide].concat()
    };
    verbatim.push(0);
    Ok(verbatim)
}

#[cfg(not(any(target_os = "macos", target_os = "linux", windows)))]
fn exclusive_rename(_from: &Path, _to: &Path) -> Option<io::Result<()>> {
    None
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
        if vault_layout::family_pattern(vault_layout::ArtifactId::AtomicTemps)
            .matches(&entry.file_name().to_string_lossy())
            && stale(&entry)
        {
            let temp = entry.path();
            vault_fs::remove_file(&temp)
                .with_context(|| format!("remove stale temp {}", temp.display()))?;
            removed += 1;
        }
    }
    Ok(removed)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Windows refuses to move, replace or delete a file another program has open without delete
    /// sharing, as an antivirus scan holds a file it reads right after it was written. Each waits
    /// for the program to let go; one that never does makes it fail, not hang.
    #[cfg(windows)]
    #[test]
    fn a_file_another_program_holds_for_a_moment_is_still_moved_replaced_and_deleted() {
        use std::os::windows::fs::OpenOptionsExt;
        const FILE_SHARE_READ: u32 = 1;
        let dir = tempfile::tempdir().expect("tempdir");
        let open = |path: &Path| {
            fs::OpenOptions::new().read(true).share_mode(FILE_SHARE_READ).open(path).expect("held")
        };
        let hold = |path: &Path| {
            let file = open(path);
            std::thread::spawn(move || {
                std::thread::sleep(Duration::from_millis(300));
                drop(file);
            })
        };
        let (a, b) = (dir.path().join("a"), dir.path().join("b"));
        fs::write(&a, b"a").expect("seed");
        let held = hold(&a);
        rename_no_replace(&a, &b).expect("moved once let go");
        held.join().expect("let go");
        let held = hold(&b);
        atomic_durable_write(&b, b"replaced").expect("replaced once let go");
        held.join().expect("let go");
        assert_eq!(fs::read(&b).expect("read"), b"replaced");
        let held = hold(&b);
        remove_file_durably(&b).expect("deleted once let go");
        held.join().expect("let go");
        assert!(!a.exists() && !b.exists());

        let c = dir.path().join("c");
        fs::write(&c, b"c").expect("seed");
        let kept = open(&c);
        let started = std::time::Instant::now();
        remove_file_durably(&c).expect_err("held throughout");
        assert!(started.elapsed() < Duration::from_secs(5), "{:?}", started.elapsed());
        drop(kept);
        assert!(c.exists());
    }

    /// What a plain rename would do, and this one must not: replace a file, or an empty folder (a
    /// rename onto an empty folder replaces it on Unix). Onto nothing it moves, folders included.
    #[test]
    fn a_rename_that_never_replaces_leaves_what_is_there() {
        let dir = tempfile::tempdir().expect("tempdir");
        let (from, to) = (dir.path().join("from"), dir.path().join("to"));
        fs::write(&from, b"moving").expect("seed");
        fs::write(&to, b"there").expect("seed");
        let error = rename_no_replace(&from, &to).expect_err("a file was replaced");
        assert_eq!(error.kind(), io::ErrorKind::AlreadyExists);
        assert_eq!(fs::read(&from).expect("read"), b"moving");
        assert_eq!(fs::read(&to).expect("read"), b"there");

        let (folder, empty) = (dir.path().join("folder"), dir.path().join("empty"));
        fs::create_dir(&folder).expect("seed");
        fs::write(folder.join("inside"), b"inside").expect("seed");
        fs::create_dir(&empty).expect("seed");
        let error = rename_no_replace(&folder, &empty).expect_err("an empty folder was replaced");
        assert_eq!(error.kind(), io::ErrorKind::AlreadyExists);
        assert!(folder.join("inside").exists() && empty.is_dir());

        let moved = dir.path().join("moved");
        rename_no_replace(&folder, &moved).expect("moved");
        assert_eq!(fs::read(moved.join("inside")).expect("read"), b"inside");
        assert!(!folder.exists());
    }

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
