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
//!   side is test-only. Unit tests can exit at a checkpoint. Integration-test children use
//!   the explicit `test-fixtures` feature to arm [`exit_process_at_next_checkpoint`].
//! - Let the desktop E2E runs fail a checkpoint in the real app, whose threads the registry cannot
//!   reach: a debug build also returns an error at a checkpoint while a file of its name is in the
//!   folder `PATHKEEP_TEST_FAULTS_DIR` names ([`requested_by_the_e2e_run`]). Release builds never
//!   look.
//!
//! Not responsible for:
//! - Cross-thread fault delivery. The registry is THREAD-LOCAL (mirroring `durable_io`), so a
//!   `checkpoint` that runs on a DIFFERENT thread than the one that armed the fault will NOT see it.
//!   That isolation makes each `#[test]` independent with no global lock. A future cross-thread flow
//!   that needs fault injection must carry the directive across the thread boundary itself.
//! - Real durability / disk barriers (see `durable_io`).

use anyhow::Result;
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};

thread_local! {
    /// Per-thread checkpoint names armed to panic, each with the number of hits to let pass first.
    /// Read on EVERY [`checkpoint`] call (production default: empty => no fault). Written ONLY by
    /// the `#[cfg(test)]` [`FaultGuard`], so production never arms anything and two test threads
    /// can never see each other's faults.
    static PANIC_AT: RefCell<HashMap<String, u32>> = RefCell::new(HashMap::new());
    /// The same for checkpoints armed to return an error once: a step that reports a failure after
    /// its effect landed (a flush that fails after the rename it follows).
    static ERROR_AT: RefCell<HashSet<String>> = RefCell::new(HashSet::new());
}

/// Takes the armed fault for `name`, if any, so it fires once.
///
/// This is the PRODUCTION read path: in a non-test build the set is never written, so it is
/// always empty and this is a single thread-local borrow plus a lookup miss.
fn take_armed(name: &str) -> bool {
    PANIC_AT.with(|cell| {
        let mut armed = cell.borrow_mut();
        match armed.get_mut(name) {
            None => false,
            Some(0) => armed.remove(name).is_some(),
            Some(left) => {
                *left -= 1;
                false
            }
        }
    })
}

fn take_armed_error(name: &str) -> bool {
    ERROR_AT.with(|cell| cell.borrow_mut().remove(name))
}

/// A named abort point a destructive op can hand control to.
///
/// In production NOTHING ever arms a fault, so this consults an empty registry and returns
/// `Ok(())` — a cheap no-op. Under test, a fault armed for `name` makes this `panic!` (a crash the
/// test catches with `std::panic::catch_unwind`), and a `test-fixtures` child process armed with
/// [`exit_process_at_next_checkpoint`] exits here without running destructors.
pub fn checkpoint(name: &str) -> Result<()> {
    #[cfg(any(test, feature = "test-fixtures"))]
    EXIT_CHECKPOINT.with(|armed| {
        if armed.borrow().as_deref() == Some(name) {
            std::process::exit(86);
        }
    });
    if take_armed(name) {
        panic!("fault_inject: simulated panic at checkpoint {name:?}");
    }
    if take_armed_error(name) || requested_by_the_e2e_run(name) {
        anyhow::bail!("fault_inject: simulated error at checkpoint {name:?}");
    }
    Ok(())
}

/// Whether the E2E run asks this debug build to fail at `name`: a file of that name is in the folder
/// `PATHKEEP_TEST_FAULTS_DIR` names. It fails every time until the run removes the file.
fn requested_by_the_e2e_run(name: &str) -> bool {
    cfg!(debug_assertions)
        && std::env::var_os("PATHKEEP_TEST_FAULTS_DIR")
            .is_some_and(|folder| std::path::Path::new(&folder).join(name).exists())
}

#[cfg(any(test, feature = "test-fixtures"))]
thread_local! {
    static EXIT_CHECKPOINT: RefCell<Option<String>> = const { RefCell::new(None) };
}

/// Integration-test children exit without destructors at a real durable boundary.
/// The arming API is absent from normal application builds.
#[cfg(any(test, feature = "test-fixtures"))]
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
    error: bool,
}

#[cfg(test)]
impl FaultGuard {
    /// Arms a fire-once `panic!` at `name` and returns a guard that disarms on drop.
    pub(crate) fn panic_at(name: &str) -> Self {
        Self::panic_at_hit(name, 1)
    }

    /// Arms a fire-once `panic!` at the `hit`-th time `name` is reached from now (1 is the next):
    /// a cut between two steps of a loop that passes the same checkpoint at every step.
    pub(crate) fn panic_at_hit(name: &str, hit: u32) -> Self {
        assert!(hit >= 1, "the first hit is 1");
        PANIC_AT.with(|cell| {
            cell.borrow_mut().insert(name.to_string(), hit - 1);
        });
        Self { name: name.to_string(), error: false }
    }

    /// Arms a fire-once error at `name`: the checkpoint returns `Err` and the step goes on as if
    /// what came before it had reported a failure.
    pub(crate) fn error_at(name: &str) -> Self {
        ERROR_AT.with(|cell| {
            cell.borrow_mut().insert(name.to_string());
        });
        Self { name: name.to_string(), error: true }
    }
}

#[cfg(test)]
impl Drop for FaultGuard {
    fn drop(&mut self) {
        if self.error {
            take_armed_error(&self.name);
        } else {
            PANIC_AT.with(|cell| cell.borrow_mut().remove(&self.name));
        }
    }
}
