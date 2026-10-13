//! Cross-process EXCLUSIVE advisory lock that serializes destructive archive ops.
//!
//! ## Responsibilities
//! - Provide one CROSS-PROCESS mutual-exclusion primitive ([`ArchiveWriteLock`])
//!   for the canonical archive: while a guard is held by this process, no OTHER OS
//!   process may hold it. This is the single chokepoint that keeps the separate
//!   scheduled-backup process from racing a GUI rekey / mode-toggle.
//! - Be PROCESS-REENTRANT: nested or concurrent acquisitions WITHIN one process
//!   all share a single underlying `flock`. A backup that takes the lock and then
//!   calls reconcile (which also takes it) must NOT deadlock against itself.
//! - Provide the COMPLEMENTARY in-process primitive ([`ArchiveOpGate`]): a
//!   process-global, NON-reentrant, keyed-by-archive gate the TOP-LEVEL destructive
//!   ops take so two of them dispatched in the SAME process serialize (the reentrant
//!   flock cannot — see "The in-process top-level gate" below).
//! - Expose RAII guards whose lifetime IS the hold: the LAST live [`ArchiveWriteLock`]
//!   in this process releases the OS lock when it drops; an [`ArchiveOpGate`] frees
//!   the in-process gate when it drops.
//! - Offer a blocking acquire (callers that MUST run), a non-blocking try-acquire
//!   (the scheduled backup uses this to DEFER instead of racing), and an
//!   interruptible/timeout acquire (UI-facing waits that show "waiting for an
//!   in-flight backup" and can be cancelled).
//!
//! ## Not responsible for
//! - Deciding WHICH operations take the lock. This module delivers the two
//!   serialization PRIMITIVES (the cross-process [`ArchiveWriteLock`] and the
//!   in-process [`ArchiveOpGate`]); the rekey/backup/import/retention-prune/
//!   reconcile/snapshot-restore call sites choose to acquire them.
//! - Serializing NON-destructive in-process work (background search/AI projection
//!   rebuilds, read queries). [`ArchiveOpGate`] excludes only TOP-LEVEL DESTRUCTIVE
//!   ops from each other in-process; unrelated workers that never `fs::rename` a
//!   canonical DB are deliberately out of scope.
//! - SQLite-internal locking (per-connection `BEGIN IMMEDIATE` / WAL). Those do
//!   not cross the `fs::rename` swap and do not coordinate a separate process, so
//!   they cannot prevent the corruption this lock exists to stop.
//! - Crash durability of the bytes a held operation writes (see `durable_io`).
//! - Windows: `flock(2)` is a POSIX advisory lock. Non-unix targets compile a
//!   stub that fails loudly rather than silently pretending to serialize.
//!
//! ## Why this exists (the write-serialization contract)
//! The 2026-06-30 data-integrity work found two critical data-corruption
//! bugs that share one root cause — there is no lock anywhere that a backup
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
//! ## The in-process top-level gate ([`ArchiveOpGate`]) — the same-process variant
//! [`ArchiveWriteLock`] is process-REENTRANT BY DESIGN (so `backup -> reconcile`
//! nesting never self-deadlocks), which means it serializes only ACROSS processes.
//! Two destructive ops dispatched in the SAME GUI process — e.g. a manual backup
//! running its canonical write transaction while the user toggles encryption (rekey)
//! or triggers an import from Settings — each get a reentrant guard sharing ONE fd,
//! so they are NOT mutually excluded. One op's `fs::rename` can then pull the
//! canonical DB out from under the other's open write transaction; on macOS the
//! committed rows land in the renamed-away inode = silent loss (same-process
//! trigger). The cross-process flock cannot close this — it IS the same process.
//!
//! [`ArchiveOpGate`] layers a second, NON-reentrant mutual exclusion ABOVE the
//! reentrant flock. A TOP-LEVEL destructive op acquires the gate FIRST, then the
//! [`ArchiveWriteLock`]; so two same-process top-level ops exclude on the gate while
//! all processes still exclude on the flock. A NESTED helper (reconcile / migrate /
//! `recover_interrupted_import` called WITHIN a top-level op) takes ONLY the reentrant
//! flock, NEVER the gate — re-taking a non-reentrant gate it already holds would
//! self-deadlock. The split is enforced TYPE-WISE: public `*` entries take the gate
//! and delegate to a `*_locked`/`*_inner` variant that assumes the caller holds it,
//! NOT by a fragile thread-local "am I nested?" flag.
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
use std::collections::{HashMap, HashSet};
use std::fs::{self, File, OpenOptions};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex, OnceLock, Weak};
use std::thread;
use std::time::{Duration, Instant};

#[cfg(unix)]
use std::os::unix::io::{AsRawFd, RawFd};

/// Sentinel lock file placed beside `history-vault.sqlite` inside the archive dir
/// so that every process touching the archive contends on one stable path.
///
/// Retention / cleanup MUST skip `.pk-*` dotfiles: deleting and recreating this
/// file defeats the lock (two inodes, two winners). See the module-level
/// "sentinel lock file must never be deleted" note.
pub(crate) const ARCHIVE_WRITE_LOCK_FILE: &str = ".pk-archive-write.lock";

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
    /// Doing release under the same mutex that [`ArchiveWriteLock::try_acquire`]
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
                // happens-before that rules out a spurious `Ok(None)`.
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

// --- in-process top-level op gate (NON-reentrant; same-process variant) -------------------

/// Process-global record of which archives have a TOP-LEVEL destructive op in flight
/// IN THIS PROCESS, paired with a condvar so a blocking [`ArchiveOpGate::acquire`]
/// parks (never busy-polls) until the in-flight op releases.
///
/// Keyed by the SAME resolved sentinel path as [`manager`] (via [`lock_file_path`]) so
/// the two serialization layers agree on archive identity even though one process
/// normally serves one archive. A path is present in the set for exactly as long as one
/// [`ArchiveOpGate`] guard lives; absent means free. Mirrors the [`manager`] shape (a
/// `OnceLock`-managed map keyed by archive path) so both layers self-clean their entries
/// on release and a process that ever juggled two archive roots would still serialize
/// each independently.
fn op_gate() -> &'static (Mutex<HashSet<PathBuf>>, Condvar) {
    static OP_GATE: OnceLock<(Mutex<HashSet<PathBuf>>, Condvar)> = OnceLock::new();
    OP_GATE.get_or_init(|| (Mutex::new(HashSet::new()), Condvar::new()))
}

/// RAII guard for the in-process TOP-LEVEL archive-op gate.
///
/// ## Why this layer exists
/// See the module-level "The in-process top-level gate" note: [`ArchiveWriteLock`] is
/// process-REENTRANT, so two destructive ops dispatched in ONE process each get a
/// reentrant guard and are NOT mutually excluded — the same-process rename-out-
/// from-under-an-open-transaction loss. This gate is a process-global, NON-reentrant,
/// keyed-by-archive mutual exclusion the TOP-LEVEL ops take so two of them in one process
/// serialize. The cross-process flock cannot do this (it is the same process).
///
/// ## Layering contract
/// A TOP-LEVEL op acquires this gate FIRST, then [`ArchiveWriteLock`], releasing in the
/// reverse order. A NESTED helper (reconcile / migrate / `recover_interrupted_import`
/// called WITHIN a top-level op) takes ONLY the reentrant [`ArchiveWriteLock`], NEVER
/// this gate: because the gate is non-reentrant, a nested re-acquire would self-deadlock.
/// The split is enforced by the public-vs-`_locked` function variants, not a runtime flag.
#[derive(Debug)]
pub struct ArchiveOpGate {
    /// The sentinel path this guard holds; removed from the held-set on Drop. Read by
    /// [`Drop`] (and `Debug`), so the gate frees exactly its own archive's entry.
    path: PathBuf,
}

impl ArchiveOpGate {
    /// Blocks until no other TOP-LEVEL op holds the in-process gate for this archive,
    /// then marks it held and returns the guard.
    ///
    /// Parks on a condvar rather than busy-polling. Unlike [`ArchiveWriteLock::acquire`],
    /// which polls so the manager mutex never enters a multi-minute kernel `LOCK_EX`
    /// wait, this gate is PURE in-process state: a condvar wait RELEASES the mutex while
    /// parked, so other acquirers and `try_acquire` probes still run, and the waiter wakes
    /// the instant the holder drops — no poll interval, no wasted wakeups. Infallible: it
    /// does no I/O (the path derivation cannot fail), so there is no error arm to surface.
    pub fn acquire(paths: &ProjectPaths) -> ArchiveOpGate {
        let path = lock_file_path(paths);
        let (mutex, condvar) = op_gate();
        let mut held = mutex.lock().expect("archive op-gate mutex poisoned");
        while held.contains(&path) {
            held = condvar.wait(held).expect("archive op-gate mutex poisoned");
        }
        held.insert(path.clone());
        ArchiveOpGate { path }
    }

    /// Non-blocking variant: `None` when another top-level op already holds the gate for
    /// this archive (the in-process DEFER signal — the scheduled backup steps aside rather
    /// than racing a foreground op in THIS process), or a guard when it just took it.
    ///
    /// Deterministic with no parking, so a unit test can prove the mutual exclusion
    /// (held => `None`; free => `Some`) without any sleep-as-synchronization.
    #[cfg(test)]
    pub(crate) fn try_acquire(paths: &ProjectPaths) -> Option<ArchiveOpGate> {
        let path = lock_file_path(paths);
        let (mutex, _condvar) = op_gate();
        let mut held = mutex.lock().expect("archive op-gate mutex poisoned");
        if held.contains(&path) {
            None
        } else {
            held.insert(path.clone());
            Some(ArchiveOpGate { path })
        }
    }
}

impl Drop for ArchiveOpGate {
    /// Frees this archive's gate entry and wakes every parked acquirer.
    fn drop(&mut self) {
        let (mutex, condvar) = op_gate();
        let mut held = mutex.lock().expect("archive op-gate mutex poisoned");
        held.remove(&self.path);
        // `notify_all` (not `notify_one`): parked acquirers may key on DIFFERENT archive
        // paths, so each must re-check ITS OWN path under the mutex; a waiter for another
        // archive simply re-parks. With one archive per process this wakes the one waiter.
        condvar.notify_all();
    }
}

/// Resolves the archive write-lock path: a hidden sentinel beside the canonical
/// archive database, so every process derives the identical contended path.
///
/// The `expect` is acceptable here — `archive_database_path` is always a
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
/// `LOCK_NB` maps "already held" `EWOULDBLOCK` to `Ok(false)` so try-acquire can
/// defer; `EINTR` (a signal interrupted the call before it resolved) is retried
/// because `flock` is safe to repeat; any other errno is a genuine failure.
#[cfg(unix)]
fn flock_try_exclusive(fd: RawFd) -> Result<bool> {
    loop {
        // SAFETY: `fd` is a live fd owned by the caller. `LOCK_EX | LOCK_NB`
        // never parks the thread, so this returns promptly.
        let rc = unsafe { libc::flock(fd, libc::LOCK_EX | libc::LOCK_NB) };
        if rc == 0 {
            return Ok(true);
        }
        match std::io::Error::last_os_error().raw_os_error().unwrap_or(libc::EIO) {
            libc::EWOULDBLOCK => return Ok(false),
            libc::EINTR => continue,
            other => {
                return Err(std::io::Error::from_raw_os_error(other))
                    .context("flock on the archive write lock failed");
            }
        }
    }
}

// --- best-effort non-local-volume detection -------------------------------------------

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
/// warning). macOS/iOS: the real `statfs.f_fstypename` (e.g. "apfs", "smbfs", "nfs").
#[cfg(all(unix, any(target_os = "macos", target_os = "ios")))]
fn archive_volume_fstype(path: &Path) -> Option<String> {
    use std::os::unix::ffi::OsStrExt;
    // A path with an interior NUL byte must NOT panic here — this runs
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
fn archive_volume_fstype(_path: &Path) -> Option<String> {
    None
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
    use tempfile::tempdir;

    #[test]
    fn distinct_open_file_descriptions_exclude_each_other() {
        // Cross-process exclusion proof: two distinct open file
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
        //
        // Bounded retry, NOT a single shot: sibling tests in this binary spawn
        // subprocesses (`git_audit` shells out to `git`), and `fork` duplicates
        // every open file descriptor — including `first`'s — until the child's
        // `exec` closes it via CLOEXEC. Inside that fork→exec window the open
        // file description stays referenced by the child, so dropping `first`
        // here does not release the flock at that exact instant. The guarantee
        // under test is "released once the description truly closes", which the
        // bounded wait still proves strictly (the exclusion assertions above
        // remain single-shot).
        drop(first);
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        loop {
            if flock_try_exclusive(second.as_raw_fd()).expect("re-flock must not error") {
                break;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "the lock must become takeable once the first description closes"
            );
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    }

    #[test]
    fn op_gate_is_mutually_exclusive_in_process() {
        // Same-process: the in-process top-level gate is a NON-reentrant
        // mutual exclusion. While one top-level op holds it, a SECOND top-level acquire
        // for the same archive is refused; once the holder drops, the gate is free again.
        // Proven with the deterministic `try_acquire` probe — no sleep-as-synchronization.
        let dir = tempdir().expect("tempdir");
        let paths = project_paths_with_root(dir.path());

        let first = ArchiveOpGate::acquire(&paths);
        assert!(format!("{first:?}").contains(ARCHIVE_WRITE_LOCK_FILE));
        assert!(
            ArchiveOpGate::try_acquire(&paths).is_none(),
            "a second same-process top-level acquire must be excluded while the gate is held"
        );

        drop(first);
        let _second = ArchiveOpGate::try_acquire(&paths)
            .expect("the gate must be free for a fresh top-level acquire once the holder drops");
    }
}
