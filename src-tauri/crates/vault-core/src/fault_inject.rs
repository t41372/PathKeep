//! Failure modes: a requested crash point never fires, hooks leak between threads or production work
//! arms a test fault.
//!
//! Named-checkpoint fault-injection seam for crash-window regression tests.
//!
//! Responsibilities:
//! - Give the destructive archive operations a single, named place to ask "should I abort RIGHT
//!   HERE?", so a crash-window test can simulate the process dying at an EXACT point inside the op
//!   (for example "killed AFTER the file swap but BEFORE writing config => the archive is still
//!   recoverable", the 2026-06-30 incident window).
//! - Stay a true no-op in production: nothing ever arms a fault, so [`checkpoint`] consults an empty
//!   thread-local registry and returns `Ok(())` with no I/O, no allocation, and no behavioural
//!   change — a single thread-local borrow plus a lookup miss.
//! - Keep the consult path in the measured (coverage/CI) build. It is NOT `cfg`-compiled out,
//!   because the quality gate forbids stubbing production paths out of coverage. Only the arming
//!   side is `#[cfg(test)]` ([`FaultGuard`]); process-exit integration fixtures require the explicit
//!   `test-fixtures` feature ([`exit_process_at_next_checkpoint`]). Normal production builds cannot
//!   arm anything.
//!
//! Not responsible for:
//! - Cross-thread fault delivery. The registry is THREAD-LOCAL (mirroring `durable_io`), so a
//!   `checkpoint` that runs on a DIFFERENT thread than the one that armed the fault will NOT see it.
//!   That isolation makes each `#[test]` independent with no global lock. A future cross-thread flow
//!   that needs fault injection must carry the directive across the thread boundary itself.
//! - Real durability / disk barriers (see `durable_io`).

use anyhow::Result;
use std::cell::RefCell;
use std::collections::HashSet;

thread_local! {
    /// Per-thread set of checkpoint names armed to panic on their next hit. Read on EVERY
    /// [`checkpoint`] call (production default: empty => no fault). Written ONLY by the
    /// `#[cfg(test)]` [`FaultGuard`], so production never arms anything and two test threads can
    /// never see each other's faults.
    static PANIC_AT: RefCell<HashSet<String>> = RefCell::new(HashSet::new());
}

/// Takes the armed fault for `name`, if any, so it fires once.
///
/// This is the PRODUCTION read path: in a non-test build the set is never written, so it is
/// always empty and this is a single thread-local borrow plus a lookup miss.
fn take_armed(name: &str) -> bool {
    PANIC_AT.with(|cell| cell.borrow_mut().remove(name))
}

/// A named abort point a destructive op can hand control to.
///
/// In production NOTHING ever arms a fault, so this consults an empty registry and returns
/// `Ok(())` — a cheap no-op. Under test, a fault armed for `name` makes this `panic!` (a crash the
/// test catches with `std::panic::catch_unwind`), and a `test-fixtures` child process armed with
/// [`exit_process_at_next_checkpoint`] exits here without running destructors.
pub fn checkpoint(name: &str) -> Result<()> {
    #[cfg(feature = "test-fixtures")]
    EXIT_CHECKPOINT.with(|armed| {
        if armed.borrow().as_deref() == Some(name) {
            std::process::exit(86);
        }
    });
    if take_armed(name) {
        panic!("fault_inject: simulated panic at checkpoint {name:?}");
    }
    Ok(())
}

#[cfg(feature = "test-fixtures")]
thread_local! {
    static EXIT_CHECKPOINT: RefCell<Option<String>> = const { RefCell::new(None) };
}

/// Integration-test children exit without destructors at a real durable boundary.
/// The arming API is absent from normal application builds.
#[cfg(feature = "test-fixtures")]
pub fn exit_process_at_next_checkpoint(name: &str) {
    EXIT_CHECKPOINT.with(|armed| *armed.borrow_mut() = Some(name.into()));
}

/// RAII guard that arms a fire-once panic at a checkpoint and disarms it on drop.
///
/// Prevents a test from leaking an armed fault into a later test on the same thread: even if the
/// test panics between arming and the checkpoint, unwinding drops the guard and clears the fault.
#[cfg(test)]
pub(crate) struct FaultGuard {
    name: String,
}

#[cfg(test)]
impl FaultGuard {
    /// Arms a fire-once `panic!` at `name` and returns a guard that disarms on drop.
    pub(crate) fn panic_at(name: &str) -> Self {
        PANIC_AT.with(|cell| {
            cell.borrow_mut().insert(name.to_string());
        });
        Self { name: name.to_string() }
    }
}

#[cfg(test)]
impl Drop for FaultGuard {
    fn drop(&mut self) {
        take_armed(&self.name);
    }
}
