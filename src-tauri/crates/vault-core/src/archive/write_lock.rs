//! Cross-process EXCLUSIVE advisory lock that serializes destructive archive ops.
//!
//! ## Responsibilities
//! - Provide one CROSS-PROCESS mutual-exclusion primitive for the canonical
//!   archive: while a guard is held by this process, no OTHER OS process may hold
//!   it. This is the single chokepoint that keeps the separate scheduled-backup
//!   process from racing a GUI rekey / mode-toggle.
//! - Be PROCESS-REENTRANT: nested or concurrent acquisitions WITHIN one process
//!   all share a single underlying `flock`. A backup that takes the lock and then
//!   calls reconcile (which also takes it) must NOT deadlock against itself.
//! - Expose an RAII guard whose lifetime IS the lock hold: the LAST live guard in
//!   this process releases the OS lock when it drops.
//! - Offer a blocking acquire (callers that MUST run), a non-blocking try-acquire
//!   (the scheduled backup uses this to DEFER instead of racing), and an
//!   interruptible/timeout acquire (UI-facing waits that show "waiting for an
//!   in-flight backup" and can be cancelled).
//!
//! ## Not responsible for
//! - Deciding WHICH operations take the lock or wiring it into call sites. This
//!   module only delivers the primitive; the rekey/backup/import/etc. call sites
//!   are wired in a later block.
//! - In-PROCESS serialization between unrelated threads. By design every
//!   same-process acquirer SHARES the one lock (process-reentrant), so this lock
//!   does NOT serialize two concurrent destructive ops inside one process — the
//!   app's own command pipeline does that. The threat this primitive removes is
//!   the OUT-OF-PROCESS scheduler racing the GUI.
//! - SQLite-internal locking (per-connection `BEGIN IMMEDIATE` / WAL). Those do
//!   not cross the `fs::rename` swap and do not coordinate a separate process, so
//!   they cannot prevent the corruption this lock exists to stop.
//! - Crash durability of the bytes a held operation writes (see `durable_io`).
//! - Windows: `flock(2)` is a POSIX advisory lock. Non-unix targets compile a
//!   stub that fails loudly rather than silently pretending to serialize.
//!
//! ## Why this exists (the write-serialization contract)
//! The 2026-06-30 data-integrity audit confirmed two CRITICAL data-corruption
//! findings that share one root cause — there is no lock anywhere that a backup
//! and a rekey both respect:
//!   * "Scheduled-backup races GUI rekey with no lock", and
//!   * "No lock between rekey and an in-flight backup -> committed rows land in
//!     the renamed-away inode".
//!
//! A backup holds one long write transaction (minutes on a 14.4M-row archive)
//! while, in another process, a rekey `fs::rename`s `history-vault.sqlite` out
//! from under it; on macOS the rename succeeds on the open file, the backup
//! commits into the now-unlinked inode, reports success, and the rows vanish —
//! and the orphaned plaintext WAL left beside the swapped-in encrypted DB bricks
//! the next open with NOTADB. SQLite's own locks cannot prevent this because the
//! rename is a filesystem op and the scheduler runs out-of-process.
//!
//! Every operation that renames/replaces or transactionally rewrites a canonical
//! archive database file MUST hold this lock for its whole duration:
//! **rekey, backup, import, retention-prune, reconcile, and snapshot-restore.**
//!
//! ## How process-reentrancy is achieved
//! `flock` is associated with the OPEN FILE DESCRIPTION, not the path and not the
//! PID. Distinct open descriptions conflict EVEN WITHIN ONE PROCESS, so naively
//! opening a fresh fd per acquire would make a second same-process `LOCK_EX`
//! block forever against the process's own first fd — a self-deadlock. To avoid
//! that while keeping cross-process exclusion, a process-global manager keyed by
//! lock path holds exactly ONE `flock`'d fd per (process, archive). The first
//! acquirer opens the fd and `flock`s it; nested/concurrent same-process
//! acquirers clone the shared owner (no new `flock`). The LAST guard to drop
//! releases (`LOCK_UN`), closes the fd, AND removes the registry entry — all
//! WHILE HOLDING the manager mutex, and it removes the entry BEFORE the unlock.
//! That ordering ("entry absent" happens-before "fd unlocked", both under the
//! one mutex a concurrent acquire must also take) is what stops a fresh acquire
//! from seeing {no live entry, fd still locked} and spuriously deferring on this
//! process's own dying fd. Each OS process has its OWN manager, so the single
//! real fd per process still conflicts across processes — which is exactly the
//! cross-process behaviour we want.
//!
//! ## Blocking acquire is poll-based on purpose
//! Acquiring blocks by polling [`ArchiveWriteLock::try_acquire`] with a short
//! sleep rather than parking inside a kernel `LOCK_EX`. This keeps the manager
//! mutex out of any multi-minute kernel wait (a parked `LOCK_EX` under the mutex
//! would stall every other in-process acquirer, including UI status polls and
//! cancellable waits) and makes timeout/cancellation fall out for free. Polling
//! a lock held by a minutes-long backup wakes a few times per second — negligible
//! cost for a hard "never freeze the UI" requirement.
//!
//! ## LIMITATION — network filesystems (MEDIUM severity)
//! The archive location is user-choosable. On SMB/CIFS (macOS `smbfs`), some NFS
//! mounts, AFP, and WebDAV, `flock` is frequently EMULATED or LOCAL-ONLY: each
//! machine/process believes it won the lock, so two of them can run a destructive
//! op at once — the exact corruption this lock prevents. PathKeep does NOT hard-
//! refuse a network archive (that is a product decision), but it makes the risk
//! loud: on every successful acquisition it best-effort detects a non-local
//! volume (macOS/iOS `statfs.f_fstypename`) and logs a clear warning. Keep the
//! archive on a LOCAL disk for the lock to be trustworthy.
//!
//! ## LIMITATION — the sentinel lock file must never be deleted
//! The lock is anchored to a single stable sentinel path (see
//! [`ARCHIVE_WRITE_LOCK_FILE`]). Deleting and recreating it opens a race where
//! two processes hold `flock`s on two different inodes of "the same" path and
//! both win. Any retention / cleanup / export-sanitize pass MUST skip `.pk-*`
//! dotfiles in the archive directory. The bytes are irrelevant; the file is a
//! pure rendezvous and is intentionally never truncated and never removed.

use crate::config::ProjectPaths;
use anyhow::{Context, Result};
use std::collections::HashMap;
use std::fs::{self, File, OpenOptions};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock, Weak};
use std::thread;
use std::time::{Duration, Instant};

#[cfg(unix)]
use std::cell::RefCell;
#[cfg(unix)]
use std::collections::VecDeque;
#[cfg(unix)]
use std::os::unix::io::{AsRawFd, RawFd};

/// Sentinel lock file placed beside `history-vault.sqlite` inside the archive dir
/// so that every process touching the archive contends on one stable path.
///
/// Retention / cleanup MUST skip `.pk-*` dotfiles: deleting and recreating this
/// file defeats the lock (two inodes, two winners). See the module-level
/// "sentinel lock file must never be deleted" note.
const ARCHIVE_WRITE_LOCK_FILE: &str = ".pk-archive-write.lock";

/// How long the blocking / interruptible acquire sleeps between non-blocking
/// attempts. Short enough that cancellation and "lock just freed" feel instant,
/// long enough that polling a minutes-long backup costs effectively nothing.
const ACQUIRE_POLL_INTERVAL: Duration = Duration::from_millis(50);

// --- process-reentrant lock manager --------------------------------------------------------------

/// Owns the single `flock`'d open file description for ONE (process, archive).
///
/// Created by the first acquirer when the process holds no live lock for the
/// path; shared (via `Arc`) by every nested/concurrent same-process acquirer so
/// they never open a second conflicting fd. Its `Drop` releases the OS lock; it
/// is invoked by the LAST [`ArchiveWriteLock`]'s `Drop` while that guard holds
/// the manager mutex (see [`ArchiveWriteLock`]'s `Drop`), so the lock is held
/// exactly while at least one guard keeps this value alive.
#[derive(Debug)]
struct LockInner {
    /// The open file description that carries the advisory lock. Read in `Drop`
    /// to release; never used for its byte contents.
    file: File,
    /// The on-disk lock path this owner holds, kept for diagnostics / logging and
    /// as the manager map key identity.
    path: PathBuf,
}

impl Drop for LockInner {
    fn drop(&mut self) {
        // Release explicitly before the fd closes. Closing the fd (the `File`'s
        // own `Drop`, which runs immediately after this body) ALSO releases the
        // `flock`, so this is belt-and-suspenders that documents the contract.
        // A failing unlock on a dying owner has nowhere meaningful to surface.
        #[cfg(unix)]
        {
            // SAFETY: the fd is owned by `self.file`, which is still open for the
            // duration of this `Drop` body; `LOCK_UN` is a valid flock operation.
            unsafe {
                libc::flock(self.file.as_raw_fd(), libc::LOCK_UN);
            }
        }
    }
}

/// Process-global registry of live locks, keyed by the resolved sentinel path.
///
/// Keying by path (rather than assuming a single archive per process) keeps the
/// reentrancy correct even if a process ever juggles two archive roots, and is
/// what makes the unit tests — which each use a distinct temp root — isolated.
/// The map holds `Weak`s only, so it never keeps a lock alive; the LAST guard's
/// `Drop` removes its entry while holding this mutex (see [`ArchiveWriteLock`]'s
/// `Drop`), so the map only ever holds entries whose `Weak` is still live — no
/// lazy pruning is needed and no dead `Weak` can linger.
fn manager() -> &'static Mutex<HashMap<PathBuf, Weak<LockInner>>> {
    static MANAGER: OnceLock<Mutex<HashMap<PathBuf, Weak<LockInner>>>> = OnceLock::new();
    MANAGER.get_or_init(|| Mutex::new(HashMap::new()))
}

/// RAII guard for the exclusive archive write lock.
///
/// The guard holds a shared `Arc` to this process's single lock owner. The OS
/// lock is held for as long as ANY guard in this process lives; when the last
/// one drops, its `Drop` (below) removes the registry entry and runs the
/// [`LockInner`] release while holding the manager mutex. There is intentionally
/// no `unlock()` method — the lifetime of the value is the lifetime of (this
/// process's share of) the lock.
///
/// The owner is an `Option` so `Drop` can `take()` it out and decide, under the
/// manager mutex, whether this guard held the last strong reference.
#[derive(Debug)]
pub struct ArchiveWriteLock {
    inner: Option<Arc<LockInner>>,
}

impl Drop for ArchiveWriteLock {
    /// Releases this guard's share of the lock. When it is the LAST holder in
    /// this process, it removes the registry entry AND releases the OS lock — in
    /// that order, both WHILE HOLDING the manager mutex.
    ///
    /// MEDIUM-1: doing release under the same mutex that [`ArchiveWriteLock::try_acquire`]
    /// takes, and removing the map entry BEFORE the fd unlocks, makes "entry
    /// absent" happen-before "fd unlocked". A concurrent `try_acquire` therefore
    /// can never observe {no live entry, fd still `flock`'d} and mistake this
    /// process's own just-released fd for ANOTHER process still holding the lock
    /// (a spurious `Ok(None)`). The released ops — `LOCK_UN` and `close` — are
    /// non-blocking, so holding the mutex across them never stalls another
    /// acquirer; the only blocking wait (the poll `sleep`) stays outside it.
    fn drop(&mut self) {
        // The same poison policy as `try_acquire`: a poisoned manager mutex is
        // fail-loud, not a path we expect to reach (nothing panics while holding
        // it). `Drop` taking the mutex cannot self-deadlock: no code path drops a
        // guard while already holding the manager mutex.
        let mut locks = manager().lock().expect("archive write-lock manager mutex poisoned");
        if let Some(owner) = self.inner.take() {
            // `Arc::into_inner` yields the owned `LockInner` IFF this guard held
            // the last strong ref. Authoritative under the mutex: every insert,
            // reentrant upgrade, and release happens under it, so no new strong
            // ref can appear between this check and the release below.
            if let Some(inner) = Arc::into_inner(owner) {
                // Remove the map entry FIRST, then release the OS lock: the
                // happens-before that closes MEDIUM-1.
                locks.remove(&inner.path);
                // `LockInner::Drop` runs `LOCK_UN` and the fd closes — both
                // non-blocking — still under the manager mutex.
                drop(inner);
            }
        }
    }
}

impl ArchiveWriteLock {
    /// Blocks until the exclusive archive write lock is held, then returns the
    /// guard. Use this from operations that must run to completion (e.g. a rekey
    /// the user explicitly triggered): it parks (by polling) until any in-flight
    /// destructive op in ANOTHER process releases the lock rather than racing it.
    ///
    /// If THIS process already holds the lock, the call returns immediately with
    /// a reentrant guard — nested archive ops never deadlock against themselves.
    pub fn acquire(paths: &ProjectPaths) -> Result<ArchiveWriteLock> {
        // A blocking acquire is an interruptible acquire that never times out and
        // is never cancelled, so the two share one poll loop. It therefore always
        // resolves to a guard (or a real error), never to `None`.
        let never_cancelled = AtomicBool::new(false);
        Self::acquire_interruptible(paths, None, &never_cancelled)
            .map(|guard| guard.expect("an untimed, uncancelled acquire always yields a guard"))
    }

    /// Tries to take the exclusive archive write lock without blocking. Returns
    /// `Ok(None)` ONLY when another OS PROCESS already holds it, so the caller can
    /// DEFER instead of racing — this is what the scheduled backup uses to step
    /// aside while a GUI rekey is mid-flight. If THIS process already holds the
    /// lock, returns `Ok(Some(..))` (a reentrant guard), never a spurious defer.
    pub fn try_acquire(paths: &ProjectPaths) -> Result<Option<ArchiveWriteLock>> {
        let lock_path = lock_file_path(paths);
        let mut locks = manager().lock().expect("archive write-lock manager mutex poisoned");

        if let Some(inner) = locks.get(&lock_path).and_then(Weak::upgrade) {
            // This process already holds the lock; hand back a reentrant guard
            // sharing the single fd (no second `flock` -> no self-deadlock).
            return Ok(Some(ArchiveWriteLock { inner: Some(inner) }));
        }

        let file = open_lock_file(&lock_path)?;
        if try_lock_exclusive(&file)? {
            // We just became this process's lock owner. Surface the network-FS
            // hazard once, at acquisition, rather than on every deferred poll.
            #[cfg(unix)]
            warn_if_archive_volume_non_local(&lock_path);
            let inner = Arc::new(LockInner { file, path: lock_path.clone() });
            locks.insert(lock_path, Arc::downgrade(&inner));
            Ok(Some(ArchiveWriteLock { inner: Some(inner) }))
        } else {
            // Non-blocking attempt found another OPEN FILE DESCRIPTION holding it,
            // which (per-process owner) means another PROCESS. Defer.
            Ok(None)
        }
    }

    /// Acquires the lock, polling until it is held, the optional `timeout`
    /// elapses, or `cancelled` is set. Returns `Ok(Some(..))` once held, or
    /// `Ok(None)` if it gave up (timed out or cancelled) — the same "step aside"
    /// signal as [`try_acquire`], so a UI-facing caller can show "waiting for an
    /// in-flight backup" and let the user back out.
    ///
    /// A real `flock`/open failure still surfaces as `Err`.
    pub fn acquire_interruptible(
        paths: &ProjectPaths,
        timeout: Option<Duration>,
        cancelled: &AtomicBool,
    ) -> Result<Option<ArchiveWriteLock>> {
        let deadline = timeout.map(|budget| Instant::now() + budget);
        loop {
            if cancelled.load(Ordering::Relaxed) {
                return Ok(None);
            }
            if let Some(guard) = Self::try_acquire(paths)? {
                return Ok(Some(guard));
            }
            if let Some(deadline) = deadline
                && Instant::now() >= deadline
            {
                return Ok(None);
            }
            thread::sleep(ACQUIRE_POLL_INTERVAL);
        }
    }

    /// The on-disk lock file this guard currently holds.
    pub fn path(&self) -> &Path {
        // `inner` is `None` only transiently inside `Drop`, never while a caller
        // can hold a `&self`, so this owner is always present here.
        &self.inner.as_ref().expect("a live archive write-lock guard always holds its owner").path
    }
}

/// Resolves the archive write-lock path: a hidden sentinel beside the canonical
/// archive database, so every process derives the identical contended path.
///
/// LOW-3: the `expect` is acceptable here — `archive_database_path` is always a
/// `<root>/archive/<file>` layout that has a parent (see `project_paths_with_root`),
/// so a `None` parent is structurally impossible. Returning `Result` would add an
/// error arm that no caller can ever trigger.
fn lock_file_path(paths: &ProjectPaths) -> PathBuf {
    paths
        .archive_database_path
        .parent()
        .expect("archive database path has a parent directory")
        .join(ARCHIVE_WRITE_LOCK_FILE)
}

/// Opens (creating if absent) the sentinel lock file at `path`. Creates the
/// archive directory first so the lock works on a fresh layout before any archive
/// database has been written.
fn open_lock_file(path: &Path) -> Result<File> {
    let parent = path.parent().expect("archive write lock path has a parent directory");
    fs::create_dir_all(parent)
        .with_context(|| format!("creating archive directory {}", parent.display()))?;
    OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        // Never truncate: the lock file is a pure flock sentinel, its bytes are
        // irrelevant, and a concurrent holder's file must be left untouched.
        .truncate(false)
        .open(path)
        .with_context(|| format!("opening archive write lock file {}", path.display()))
}

/// Attempts a single, non-blocking exclusive `flock` on `file`.
///
/// `Ok(true)` = the lock is now held; `Ok(false)` = a non-blocking attempt found
/// it already held by another open file description; `Err` = a real failure.
#[cfg(unix)]
fn try_lock_exclusive(file: &File) -> Result<bool> {
    flock_try_exclusive(file.as_raw_fd())
}

#[cfg(not(unix))]
fn try_lock_exclusive(_file: &File) -> Result<bool> {
    anyhow::bail!("archive write lock requires POSIX flock; this platform is unsupported")
}

/// Runs one non-blocking exclusive `flock(2)` against a raw fd, retrying on a
/// signal interruption.
///
/// Separated from [`try_lock_exclusive`] so the error arm can be exercised
/// against the REAL syscall with an invalid fd (the coverage gate forbids
/// stubbing out production I/O). `LOCK_NB` maps "already held" `EWOULDBLOCK` to
/// `Ok(false)` so try-acquire can defer; `EINTR` (LOW-1: a signal interrupted the
/// call before it resolved) is retried because `flock` is safe to repeat; any
/// other errno is a genuine failure.
#[cfg(unix)]
fn flock_try_exclusive(fd: RawFd) -> Result<bool> {
    loop {
        // Test seam (always compiled, never written in production): a queued
        // directive drives an errno arm — e.g. an EINTR retry — against this exact
        // code without needing a real signal. The real syscall always runs in
        // production because the queue is empty there.
        let errno = match next_flock_fault() {
            Some(injected) => injected,
            None => {
                // SAFETY: `fd` is the caller's contract — production passes a live
                // owned fd, while the EBADF test passes -1 to drive the error arm
                // against the unstubbed syscall. `LOCK_EX | LOCK_NB` never parks
                // the thread, so this returns promptly.
                let rc = unsafe { libc::flock(fd, libc::LOCK_EX | libc::LOCK_NB) };
                if rc == 0 {
                    return Ok(true);
                }
                std::io::Error::last_os_error().raw_os_error().unwrap_or(libc::EIO)
            }
        };
        match errno {
            libc::EWOULDBLOCK => return Ok(false),
            libc::EINTR => continue,
            other => {
                return Err(std::io::Error::from_raw_os_error(other))
                    .context("flock on the archive write lock failed");
            }
        }
    }
}

// --- best-effort non-local-volume detection (MEDIUM-1) -------------------------------------------

/// Emits a loud, best-effort warning when the archive sentinel lives on a volume
/// where `flock` may not actually serialize across machines/processes.
///
/// vault-core deliberately carries no logging-facade dependency, so this writes a
/// best-effort line to stderr; the authoritative mitigation is the prominent
/// module documentation and (later) the call sites, which can route this through
/// the desktop log pipeline. This is a WARNING, never a hard refusal — using a
/// network archive is the user's call; we just make the risk impossible to miss.
#[cfg(unix)]
fn warn_if_archive_volume_non_local(lock_path: &Path) {
    if let Some(message) = non_local_archive_warning(lock_path) {
        eprintln!("pathkeep: WARNING: {message}");
    }
}

/// Builds the non-local-volume warning, or `None` when the volume looks local or
/// detection is unavailable. Best-effort: it never blocks acquiring the lock.
#[cfg(unix)]
fn non_local_archive_warning(lock_path: &Path) -> Option<String> {
    let fstype = archive_volume_fstype(lock_path)?;
    if fstype_looks_non_local(&fstype) {
        Some(format!(
            "the archive write-lock sentinel is on a non-local volume (filesystem \"{fstype}\" at \
             {}). POSIX flock is often emulated or local-only on network filesystems \
             (SMB/CIFS/NFS/AFP/WebDAV), so a second machine or process can hold the lock at the \
             same time and corrupt the archive. Keep the archive on a local disk.",
            lock_path.display()
        ))
    } else {
        None
    }
}

/// `true` when `fstype` names a network filesystem on which `flock` cannot be
/// trusted to serialize across hosts. Substring match keeps it robust to the
/// platform-specific spellings (`smbfs`, `nfs`, `webdav` ...).
#[cfg(unix)]
fn fstype_looks_non_local(fstype: &str) -> bool {
    const NETWORK_FS_MARKERS: [&str; 5] = ["smb", "cifs", "nfs", "afp", "webdav"];
    let lowered = fstype.to_ascii_lowercase();
    NETWORK_FS_MARKERS.iter().any(|marker| lowered.contains(marker))
}

/// Best-effort filesystem-type label of the volume backing `path`.
///
/// `None` means "unknown / could not determine" and is treated as local (no
/// warning). A test-only seam lets a unit test drive the non-local branch without
/// a real network mount; production always reads the real volume.
#[cfg(unix)]
fn archive_volume_fstype(path: &Path) -> Option<String> {
    if let Some(forced) = fstype_override() {
        return Some(forced);
    }
    real_volume_fstype(path)
}

/// macOS/iOS: the real `statfs.f_fstypename` (e.g. "apfs", "smbfs", "nfs").
#[cfg(all(unix, any(target_os = "macos", target_os = "ios")))]
fn real_volume_fstype(path: &Path) -> Option<String> {
    use std::os::unix::ffi::OsStrExt;
    // LOW-2: a path with an interior NUL byte must NOT panic here — this runs
    // under the manager mutex (acquire -> warn), so a panic would poison it and
    // cascade-panic every future acquire. Treat an unrepresentable path as
    // "unknown / local" (None), the same as any other detection failure.
    let c_path = match std::ffi::CString::new(path.as_os_str().as_bytes()) {
        Ok(c_path) => c_path,
        Err(_) => return None,
    };
    // SAFETY: `statfs` only writes into the zeroed buffer; `c_path` is a valid
    // NUL-terminated path pointer that outlives the call.
    let mut buffer: libc::statfs = unsafe { std::mem::zeroed() };
    let rc = unsafe { libc::statfs(c_path.as_ptr(), &mut buffer) };
    if rc != 0 {
        return None;
    }
    let name: Vec<u8> =
        buffer.f_fstypename.iter().take_while(|&&byte| byte != 0).map(|&byte| byte as u8).collect();
    Some(String::from_utf8_lossy(&name).into_owned())
}

/// Other unix: local-volume detection currently relies on macOS/iOS
/// `statfs.f_fstypename`; rather than guess from Linux's numeric `f_type` we
/// conservatively report "unknown" (None). Best-effort by contract.
#[cfg(all(unix, not(any(target_os = "macos", target_os = "ios"))))]
fn real_volume_fstype(_path: &Path) -> Option<String> {
    None
}

// --- test-only fault / detection seams -----------------------------------------------------------
//
// These stay in the MEASURED build on purpose: the quality gate forbids `cfg`-compiling production
// I/O out of the coverage binary. The real `flock`/`statfs` paths always run in production (the
// thread-locals default to "no fault / real volume"); only `#[cfg(all(unix, test))]` setters ever
// write them, so test threads are isolated from each other and from production.

#[cfg(unix)]
thread_local! {
    /// FIFO of injected `flock` errnos. Each `flock_try_exclusive` pops the front:
    /// `Some(errno)` drives an errno arm as if the syscall had returned it (e.g. an
    /// EINTR retry); `None`/empty runs the real syscall. Always empty in production.
    static FLOCK_FAULTS: RefCell<VecDeque<Option<i32>>> = const { RefCell::new(VecDeque::new()) };
    /// Forced `f_fstypename` for non-local-volume detection. `None` (production
    /// default) reads the real volume via `statfs`.
    static FSTYPE_OVERRIDE: RefCell<Option<String>> = const { RefCell::new(None) };
}

/// Pops the next injected `flock` directive. `None` => run the real syscall
/// (always so in production, where the queue is never written).
#[cfg(unix)]
fn next_flock_fault() -> Option<i32> {
    FLOCK_FAULTS.with(|queue| queue.borrow_mut().pop_front().flatten())
}

/// Reads the forced filesystem-type label. `None` => read the real volume
/// (always so in production).
#[cfg(unix)]
fn fstype_override() -> Option<String> {
    FSTYPE_OVERRIDE.with(|cell| cell.borrow().clone())
}

/// Queues per-`flock` fault directives for the current test thread (replacing any
/// prior queue).
#[cfg(all(unix, test))]
fn inject_flock_faults(faults: Vec<Option<i32>>) {
    FLOCK_FAULTS.with(|queue| {
        let mut queue = queue.borrow_mut();
        queue.clear();
        queue.extend(faults);
    });
}

/// Forces the next non-local-volume detection on the current test thread.
#[cfg(all(unix, test))]
fn set_fstype_override(value: Option<String>) {
    FSTYPE_OVERRIDE.with(|cell| *cell.borrow_mut() = value);
}

/// Test-only: holds the archive write lock via a RAW second open file description,
/// bypassing this process's reentrant manager so the held lock looks like ANOTHER OS
/// process owns it.
///
/// While the returned `File` stays alive, a [`ArchiveWriteLock::try_acquire`] from THIS
/// process opens a DISTINCT open file description that the kernel refuses (`Ok(None)`) —
/// exactly the cross-process contention the scheduled backup must defer on (an in-process
/// `ArchiveWriteLock` guard would instead hand back a reentrant guard and could not
/// reproduce the defer). Dropping the `File` releases the `flock`. Used by the
/// backup-defer crash-window regression test in the sibling `backup`/`tests` modules.
#[cfg(all(unix, test))]
pub(crate) fn hold_write_lock_as_foreign_process_for_test(paths: &ProjectPaths) -> File {
    let lock_path = lock_file_path(paths);
    let file = open_lock_file(&lock_path).expect("open the archive write-lock sentinel");
    assert!(
        flock_try_exclusive(file.as_raw_fd()).expect("the foreign-holder flock must not error"),
        "the simulated foreign process must win the uncontended write lock"
    );
    file
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use crate::config::project_paths_with_root;
    use std::sync::mpsc;
    use tempfile::tempdir;

    #[test]
    fn acquire_then_drop_then_reacquire_succeeds() {
        let dir = tempdir().expect("tempdir");
        let paths = project_paths_with_root(dir.path());

        let guard = ArchiveWriteLock::acquire(&paths).expect("first acquire");
        // The guard's path is the sentinel beside the archive database.
        assert!(guard.path().ends_with(ARCHIVE_WRITE_LOCK_FILE));
        assert_eq!(guard.path().parent(), paths.archive_database_path.parent());
        // Debug should be printable for future log lines and name the lock file.
        assert!(format!("{guard:?}").contains(ARCHIVE_WRITE_LOCK_FILE));

        drop(guard);

        // Releasing the last guard frees the lock and removes its registry entry
        // (eagerly, in `Drop`, under the manager mutex) for a fresh acquire.
        let _second = ArchiveWriteLock::acquire(&paths).expect("reacquire after drop");
    }

    #[test]
    fn try_acquire_right_after_the_last_drop_wins_never_spuriously_defers() {
        // MEDIUM-1 (deterministic): `try_acquire`'s `Ok(None)` is reserved for
        // ANOTHER OS PROCESS holding the lock. Dropping the last guard removes the
        // registry entry AND releases the fd as one step under the manager mutex,
        // so the very next `try_acquire` in THIS process must win a guard, never
        // mistake the just-released fd for cross-process contention.
        let dir = tempdir().expect("tempdir");
        let paths = project_paths_with_root(dir.path());

        let guard = ArchiveWriteLock::acquire(&paths).expect("acquire");
        drop(guard);

        let reacquired = ArchiveWriteLock::try_acquire(&paths)
            .expect("try_acquire must not error right after the last drop")
            .expect("try_acquire right after the last drop must win, never a spurious Ok(None)");
        assert!(reacquired.path().ends_with(ARCHIVE_WRITE_LOCK_FILE));
    }

    #[test]
    fn last_guard_drop_is_observable_to_a_concurrent_try_acquire() {
        // MEDIUM-1 (concurrent proof): one thread drops the last guard while
        // another races to `try_acquire`. Because `Drop` removes the registry
        // entry BEFORE it releases the fd, both under the manager mutex the racing
        // acquire must also take, that acquire can never observe {entry absent, fd
        // still locked}. It therefore ALWAYS yields a guard — a reentrant one
        // while the seed guard is still alive, or a fresh one once it has dropped
        // — and NEVER `Ok(None)` (which is reserved for another OS process). Many
        // iterations under a watchdog turn a regression to the old unsynchronized
        // release into a hard failure rather than a rare flake.
        let dir = tempdir().expect("tempdir");
        let paths = project_paths_with_root(dir.path());

        for _ in 0..256 {
            let seed = ArchiveWriteLock::acquire(&paths).expect("seed acquire");
            let racer_paths = paths.clone();
            let (done_tx, done_rx) = mpsc::channel();
            let racer = thread::spawn(move || {
                let outcome = ArchiveWriteLock::try_acquire(&racer_paths);
                done_tx.send(()).expect("racer signals completion");
                outcome
            });

            // Race the racing `try_acquire` against this drop of the only guard.
            drop(seed);

            done_rx
                .recv_timeout(Duration::from_secs(10))
                .expect("racing try_acquire wedged (watchdog tripped)");
            let racing_guard = racer
                .join()
                .expect("racer thread")
                .expect("racing try_acquire must not error")
                .expect("racing try_acquire must win a guard, never a spurious Ok(None)");
            drop(racing_guard);
        }

        // The registry self-cleans, so a final acquire still wins after the loop.
        let _final = ArchiveWriteLock::acquire(&paths).expect("final reacquire");
    }

    #[test]
    fn nested_same_process_acquire_is_reentrant_and_never_hangs() {
        // HIGH-1: a nested same-process acquire must SUCCEED (reentrant) and must
        // NOT hang. We run the nesting on a worker thread under a watchdog so a
        // regression to the self-deadlock fails fast instead of wedging the suite.
        let dir = tempdir().expect("tempdir");
        let paths = project_paths_with_root(dir.path());
        let worker_paths = paths.clone();

        let (done_tx, done_rx) = mpsc::channel();
        let handle = thread::spawn(move || {
            let outer = ArchiveWriteLock::acquire(&worker_paths).expect("outer acquire");
            // Nested blocking acquire: would block forever with the old per-call-fd
            // design; must be instant and reentrant now.
            let inner =
                ArchiveWriteLock::acquire(&worker_paths).expect("nested acquire must not deadlock");
            // try_acquire while self-held must report a reentrant guard, NOT a defer.
            let nested_try = ArchiveWriteLock::try_acquire(&worker_paths)
                .expect("nested try must not error")
                .expect("nested try while self-held must be reentrant, not Ok(None)");

            assert!(inner.path().ends_with(ARCHIVE_WRITE_LOCK_FILE));
            assert!(nested_try.path().ends_with(ARCHIVE_WRITE_LOCK_FILE));

            drop(nested_try);
            drop(inner);
            drop(outer);
            done_tx.send(()).expect("signal completion");
        });

        done_rx
            .recv_timeout(Duration::from_secs(10))
            .expect("nested same-process acquire deadlocked (HIGH-1 regression)");
        handle.join().expect("worker thread");

        // Once every nested guard has dropped, the lock is free again.
        let _again = ArchiveWriteLock::acquire(&paths).expect("reacquire after nested release");
    }

    #[test]
    fn distinct_open_file_descriptions_exclude_each_other() {
        // Cross-process exclusion proof (LOW-4b): two distinct open file
        // descriptions of the same path conflict EVEN within this one process —
        // the exact OS mechanism that gives cross-PROCESS exclusion (each process
        // opens its own description). Driven against the REAL syscall, no stubs.
        let dir = tempdir().expect("tempdir");
        let paths = project_paths_with_root(dir.path());
        let lock_path = lock_file_path(&paths);

        let first = open_lock_file(&lock_path).expect("open first description");
        let second = open_lock_file(&lock_path).expect("open second description");

        assert!(
            flock_try_exclusive(first.as_raw_fd()).expect("first flock must not error"),
            "the first open file description must win the lock"
        );
        assert!(
            !flock_try_exclusive(second.as_raw_fd()).expect("second flock must not error"),
            "a second open file description (the cross-process case) must be refused"
        );

        // Closing the first description releases the lock for the second.
        drop(first);
        assert!(
            flock_try_exclusive(second.as_raw_fd()).expect("re-flock must not error"),
            "the lock must be takeable once the first description closes"
        );
    }

    #[test]
    fn acquire_errors_when_the_archive_directory_cannot_be_created() {
        // Error path: a regular FILE sits where the `archive/` directory must be,
        // so `create_dir_all` fails (a parent component is not a directory) and
        // both acquire paths surface an `Err` instead of panicking.
        let dir = tempdir().expect("tempdir");
        fs::write(dir.path().join("archive"), b"not a directory").expect("seed blocking file");
        let paths = project_paths_with_root(dir.path());

        let blocking_error = ArchiveWriteLock::acquire(&paths)
            .expect_err("acquire must fail on an uncreatable lock dir");
        assert!(blocking_error.to_string().contains("creating archive directory"));

        let try_error = ArchiveWriteLock::try_acquire(&paths)
            .expect_err("try_acquire must fail on an uncreatable lock dir");
        assert!(try_error.to_string().contains("creating archive directory"));
    }

    #[test]
    fn flock_try_exclusive_surfaces_a_non_would_block_errno_as_error() {
        // Drive the real syscall's error arm: -1 is never a valid fd, so `flock`
        // returns EBADF (not EWOULDBLOCK) and we must surface `Err` rather than
        // masquerading the failure as "would block" -> defer-forever.
        let error = flock_try_exclusive(-1)
            .expect_err("flock on an invalid fd must be a real error, not Ok");
        assert!(error.to_string().contains("flock on the archive write lock failed"));
    }

    #[test]
    fn flock_try_exclusive_retries_after_a_signal_interruption() {
        // LOW-1: an EINTR must be retried, not surfaced. The injected EINTR drives
        // the retry branch; the next iteration runs the real syscall and wins.
        let dir = tempdir().expect("tempdir");
        let paths = project_paths_with_root(dir.path());
        let lock_path = lock_file_path(&paths);
        let file = open_lock_file(&lock_path).expect("open lock file");

        inject_flock_faults(vec![Some(libc::EINTR)]);
        assert!(
            flock_try_exclusive(file.as_raw_fd())
                .expect("an EINTR must be retried, never surfaced as Err"),
            "after the EINTR retry the real flock should win the lock"
        );
    }

    #[test]
    fn acquire_interruptible_returns_a_guard_when_uncontended() {
        let dir = tempdir().expect("tempdir");
        let paths = project_paths_with_root(dir.path());

        let cancelled = AtomicBool::new(false);
        let guard = ArchiveWriteLock::acquire_interruptible(
            &paths,
            Some(Duration::from_secs(5)),
            &cancelled,
        )
        .expect("interruptible acquire must not error")
        .expect("an uncontended interruptible acquire yields a guard");
        assert!(guard.path().ends_with(ARCHIVE_WRITE_LOCK_FILE));
    }

    #[test]
    fn acquire_interruptible_polls_until_the_lock_frees() {
        // MEDIUM-2: the first attempt defers (injected EWOULDBLOCK as if another
        // process held it), then the poll loop sleeps and the next attempt wins.
        // `timeout: None` also exercises the no-deadline branch of the loop.
        let dir = tempdir().expect("tempdir");
        let paths = project_paths_with_root(dir.path());

        let cancelled = AtomicBool::new(false);
        inject_flock_faults(vec![Some(libc::EWOULDBLOCK)]);
        let guard = ArchiveWriteLock::acquire_interruptible(&paths, None, &cancelled)
            .expect("interruptible acquire must not error")
            .expect("the poll loop must eventually acquire the lock");
        assert!(guard.path().ends_with(ARCHIVE_WRITE_LOCK_FILE));
    }

    #[test]
    fn acquire_interruptible_times_out_when_the_deadline_passes() {
        // MEDIUM-2: a zero budget with the lock "held elsewhere" must defer rather
        // than block, exercising the deadline-reached branch.
        let dir = tempdir().expect("tempdir");
        let paths = project_paths_with_root(dir.path());

        let cancelled = AtomicBool::new(false);
        inject_flock_faults(vec![Some(libc::EWOULDBLOCK)]);
        let outcome =
            ArchiveWriteLock::acquire_interruptible(&paths, Some(Duration::ZERO), &cancelled)
                .expect("a timeout is not an error");
        assert!(outcome.is_none(), "an exhausted deadline must defer, not block");
    }

    #[test]
    fn acquire_interruptible_honors_cancellation() {
        // MEDIUM-2: a pre-set cancel flag abandons the acquire immediately.
        let dir = tempdir().expect("tempdir");
        let paths = project_paths_with_root(dir.path());

        let cancelled = AtomicBool::new(true);
        let outcome = ArchiveWriteLock::acquire_interruptible(&paths, None, &cancelled)
            .expect("cancellation is not an error");
        assert!(outcome.is_none(), "a set cancel flag must abandon the acquire");
    }

    #[test]
    fn non_local_volume_is_detected_and_local_is_not() {
        // MEDIUM-1: the warning fires for a network filesystem label and stays
        // silent for a local one (or when detection is unavailable).
        let dir = tempdir().expect("tempdir");
        let paths = project_paths_with_root(dir.path());
        let lock_path = lock_file_path(&paths);

        // Real local tempdir (or "unknown" on non-macOS) never warns.
        assert!(
            non_local_archive_warning(&lock_path).is_none(),
            "a local archive volume must not warn"
        );
        // An unstattable path yields no detection, hence no warning.
        assert!(
            non_local_archive_warning(Path::new("/pathkeep-nonexistent-volume/lock")).is_none(),
            "an undetectable volume must not warn"
        );

        set_fstype_override(Some("smbfs".to_string()));
        let warning = non_local_archive_warning(&lock_path).expect("a network FS must warn");
        assert!(warning.contains("smbfs"));
        assert!(warning.contains("non-local volume"));

        set_fstype_override(Some("apfs".to_string()));
        assert!(
            non_local_archive_warning(&lock_path).is_none(),
            "a local fstype label must not warn"
        );
        set_fstype_override(None);
    }

    #[test]
    fn archive_volume_fstype_reads_the_real_volume() {
        // MEDIUM-1: the real `statfs` path runs (no override). A tempdir on a local
        // disk must not look non-local; an unstattable path returns None safely.
        let dir = tempdir().expect("tempdir");
        let lock_path = lock_file_path(&project_paths_with_root(dir.path()));
        set_fstype_override(None);

        let fstype = archive_volume_fstype(&lock_path);
        assert_ne!(
            fstype.as_deref().map(fstype_looks_non_local),
            Some(true),
            "a tempdir on a local disk must not be classified as non-local"
        );
        assert!(
            archive_volume_fstype(Path::new("/pathkeep-nonexistent-volume/lock")).is_none(),
            "statfs on a missing volume must yield None, never panic"
        );
    }

    #[test]
    fn real_volume_fstype_returns_none_for_a_path_with_an_interior_nul() {
        // LOW-2: an interior-NUL path must NOT panic. The detection runs under the
        // manager mutex (acquire -> warn), so a panic would poison that mutex and
        // cascade-panic every future acquire. `CString::new` rejects the interior
        // NUL and we return None (treated as "unknown / local"). On non-macOS unix
        // this variant returns None unconditionally, so the assertion holds there
        // too.
        use std::ffi::OsStr;
        use std::os::unix::ffi::OsStrExt;
        let nul_path = Path::new(OsStr::from_bytes(b"/pathkeep\0interior-nul/lock"));
        assert!(
            real_volume_fstype(nul_path).is_none(),
            "an interior-NUL path must yield None, never panic and poison the manager mutex"
        );
    }

    #[test]
    fn warn_if_archive_volume_non_local_emits_only_for_network_filesystems() {
        let dir = tempdir().expect("tempdir");
        let lock_path = lock_file_path(&project_paths_with_root(dir.path()));

        // Network label: exercises the warning-emitting branch.
        set_fstype_override(Some("nfs".to_string()));
        warn_if_archive_volume_non_local(&lock_path);
        // Local label: exercises the no-op branch.
        set_fstype_override(Some("apfs".to_string()));
        warn_if_archive_volume_non_local(&lock_path);
        set_fstype_override(None);
    }
}
