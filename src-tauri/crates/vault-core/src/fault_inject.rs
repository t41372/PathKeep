//! Named-checkpoint fault-injection seam for crash-window regression tests.
//!
//! ## Responsibilities
//! - Give the destructive archive operations (rekey / import / backup) a single,
//!   named place to ask "should I abort RIGHT HERE?", so a crash-window test can
//!   simulate the process dying at an EXACT point inside the op. Phase B uses this
//!   to prove recoverability claims like "killed AFTER the file swap but BEFORE
//!   writing config => the archive is still recoverable" — the 2026-06-30 incident
//!   window.
//! - Stay a true no-op in production: nothing ever arms a fault and nothing ever
//!   registers a must-fire watch, so [`checkpoint`] consults two empty thread-local
//!   registries (the armed-fault map and the must-fire hit map) and returns `Ok(())`
//!   with no I/O, no allocation, and no behavioural change — each is a single
//!   thread-local borrow plus a lookup miss.
//! - Catch the inverse mistake too: a test that arms a fault at a checkpoint the op
//!   NEVER reaches (a mis-typed name, or a destructive section that runs on a
//!   different thread than the arming thread) would otherwise pass as a false "no
//!   crash here". [`checkpoint`] records that a name was hit, and the must-fire
//!   [`FaultGuard`] PANICS on drop if its armed checkpoint was never hit.
//! - Keep the consult/read path (both [`consult`] and [`record_hit`]) in the MEASURED
//!   (coverage/CI) build. It is NOT `cfg`-compiled out, because the quality gate
//!   forbids stubbing production paths out of coverage (that hides the real failure
//!   modes). Only the ARMING side — the writers, including the must-fire watch
//!   registration — is `#[cfg(test)]`, so production can never arm anything and test
//!   threads are isolated from each other for free.
//!
//! ## Not responsible for
//! - Inserting `checkpoint(...)` calls into the rekey / import / backup paths — that
//!   wiring is Phase B. This module ships ONLY the seam plus its own tests.
//! - Cross-thread fault delivery. The registry is THREAD-LOCAL (mirroring
//!   `durable_io`), so a `checkpoint` that runs on a DIFFERENT thread than the one
//!   that armed the fault will NOT see it. That is exactly the isolation the
//!   synchronous rekey/import/backup paths Phase B targets want (each `#[test]` is
//!   independent for free, no global lock). A future cross-thread flow that needs
//!   fault injection must carry the directive across the thread boundary itself.
//! - Real durability / disk barriers (see `durable_io`).

use anyhow::Result;
use std::cell::RefCell;
use std::collections::HashMap;

/// What an armed checkpoint does when it is hit.
///
/// Read in production (matched in [`checkpoint`], its fields consulted in
/// [`consult`]); constructed ONLY by the `#[cfg(test)]` arming helpers, so a
/// non-test build sees it read-but-never-built — hence the `not(test)` dead-code
/// allowance, which silences the lint WITHOUT compiling the type out of the
/// measured (test/coverage) build.
#[cfg_attr(not(test), allow(dead_code))]
#[derive(Clone, Copy)]
enum FaultKind {
    /// [`checkpoint`] returns an `Err` naming the checkpoint, so the op aborts there
    /// cleanly (the surrounding op's normal error path runs).
    Error,
    /// [`checkpoint`] `panic!`s, simulating a harder abort that a test catches with
    /// `std::panic::catch_unwind`.
    Panic,
}

/// A directive parked on a checkpoint name: what to do, and whether it survives the
/// first hit.
///
/// `fire_once == true` (the default for [`arm_error_at`] / [`arm_panic_at`]) disarms
/// itself the moment it fires — the common case, since a rekey/import typically
/// passes a given checkpoint name exactly once and a leftover armed fault would
/// surprise the next checkpoint. `fire_once == false` (see [`arm_error_repeating`])
/// keeps firing until explicitly disarmed.
#[cfg_attr(not(test), allow(dead_code))]
#[derive(Clone, Copy)]
struct ArmedFault {
    kind: FaultKind,
    fire_once: bool,
}

thread_local! {
    /// Per-thread map of `checkpoint name -> armed directive`. Read on EVERY
    /// [`checkpoint`] call (production default: empty => no fault). Written ONLY by
    /// the `#[cfg(test)]` arming helpers below, so production never arms anything and
    /// two test threads can never see each other's faults.
    static FAULTS: RefCell<HashMap<String, ArmedFault>> = RefCell::new(HashMap::new());

    /// Per-thread map of `watched checkpoint name -> hit count`, used ONLY by the
    /// must-fire [`FaultGuard`] to detect "armed a fault the op never reached". A name
    /// is registered (count 0) only when a `#[cfg(test)]` must-fire guard is created;
    /// [`record_hit`] then increments it on each [`checkpoint`] hit, and the guard's
    /// Drop reads it back. Empty in production, so [`record_hit`] is a lookup miss.
    static HITS: RefCell<HashMap<String, u32>> = RefCell::new(HashMap::new());
}

/// Looks up `name` in the thread-local registry, honoring fire-once auto-disarm.
///
/// Returns the directive to act on, or `None` to proceed normally. This is the
/// PRODUCTION read path: in a non-test build the map is never written, so it is
/// always empty and this is a single thread-local borrow plus a lookup miss.
fn consult(name: &str) -> Option<FaultKind> {
    FAULTS.with(|cell| {
        let mut map = cell.borrow_mut();
        let fault = *map.get(name)?;
        if fault.fire_once {
            map.remove(name);
        }
        Some(fault.kind)
    })
}

/// Records that checkpoint `name` was reached, for any must-fire watch tracking it.
///
/// This is part of the PRODUCTION read path (deliberately NOT `#[cfg(test)]`): every
/// [`checkpoint`] call routes through it so the must-fire accounting reflects REAL
/// production control flow, not a test-only mirror. It stays inert in production — the
/// watch map is registered ONLY by the `#[cfg(test)]` must-fire guard, so in a
/// non-test build [`HITS`] is always empty and this is one thread-local borrow plus a
/// lookup miss: no allocation, no write, no behavioural change. Only a name a guard is
/// actively watching gets its counter bumped.
fn record_hit(name: &str) {
    HITS.with(|cell| {
        if let Some(count) = cell.borrow_mut().get_mut(name) {
            *count = count.saturating_add(1);
        }
    });
}

/// A named abort point a destructive op can hand control to.
///
/// In production NOTHING ever arms a fault, so this consults an empty registry and
/// returns `Ok(())` — a cheap no-op (one thread-local borrow plus a hash-lookup
/// miss, no I/O, no allocation). Under test, an armed fault for `name` makes this
/// return a recognizable `Err` (its message names the checkpoint) or `panic!`, so a
/// crash-window test can drive the op to abort at exactly this point.
///
/// Phase B will sprinkle `checkpoint("...")` calls at the load-bearing seams inside
/// rekey / import / backup; this module deliberately wires NONE of them yet.
pub fn checkpoint(name: &str) -> Result<()> {
    record_hit(name);
    match consult(name) {
        Some(FaultKind::Error) => {
            Err(anyhow::anyhow!("fault_inject: simulated error at checkpoint {name:?}"))
        }
        Some(FaultKind::Panic) => {
            panic!("fault_inject: simulated panic at checkpoint {name:?}")
        }
        None => Ok(()),
    }
}

// --- test-only arming API (writers) --------------------------------------------------------------
//
// Everything below is `#[cfg(test)]`: it exists only when the crate is compiled for tests, so a
// production build literally cannot arm a fault. In-crate unit tests (including Phase B's crash-window
// tests in archive/ and migration.rs) reach these as `crate::fault_inject::*`; that is the same
// in-crate-only reach `durable_io`'s injectors have. Each helper writes the current thread's registry,
// so `#[test]` isolation comes for free.

/// Arms `name` to return an `Err` on its next hit, then auto-disarm (fire-once).
#[cfg(test)]
pub(crate) fn arm_error_at(name: &str) {
    arm(name, FaultKind::Error, true);
}

/// Arms `name` to `panic!` on its next hit, then auto-disarm (fire-once).
#[cfg(test)]
pub(crate) fn arm_panic_at(name: &str) {
    arm(name, FaultKind::Panic, true);
}

/// Arms `name` to return an `Err` on EVERY hit until explicitly disarmed.
///
/// For tests that pass the same checkpoint more than once (e.g. a retry loop) and
/// want every pass to fail.
#[cfg(test)]
pub(crate) fn arm_error_repeating(name: &str) {
    arm(name, FaultKind::Error, false);
}

/// Parks `kind`/`fire_once` on `name` for the current thread, replacing any prior
/// directive on that name.
#[cfg(test)]
fn arm(name: &str, kind: FaultKind, fire_once: bool) {
    FAULTS.with(|cell| {
        cell.borrow_mut().insert(name.to_string(), ArmedFault { kind, fire_once });
    });
}

/// Removes any armed fault for `name` on the current thread (no-op if none).
#[cfg(test)]
pub(crate) fn disarm(name: &str) {
    FAULTS.with(|cell| {
        cell.borrow_mut().remove(name);
    });
}

/// Clears every armed fault on the current thread.
#[cfg(test)]
pub(crate) fn disarm_all() {
    FAULTS.with(|cell| cell.borrow_mut().clear());
}

/// Registers `name` for must-fire hit tracking (count 0). [`record_hit`] then bumps
/// the counter on each [`checkpoint`] hit; the must-fire [`FaultGuard`] reads it back
/// on drop. Test-only: production never registers a watch, so [`record_hit`] stays a
/// pure lookup miss.
#[cfg(test)]
fn watch_must_fire(name: &str) {
    HITS.with(|cell| {
        cell.borrow_mut().insert(name.to_string(), 0);
    });
}

/// Removes and returns `name`'s recorded hit count (0 if it was never registered or
/// never hit). Called by the must-fire [`FaultGuard`]'s Drop to decide whether to
/// panic, and to clean its entry out of [`HITS`].
#[cfg(test)]
fn take_hits(name: &str) -> u32 {
    HITS.with(|cell| cell.borrow_mut().remove(name).unwrap_or(0))
}

/// RAII guard that disarms its checkpoint on drop.
///
/// Prevents a test from leaking an armed fault into a later test on the same thread:
/// even if the test panics (e.g. an `assert!` between arming and the checkpoint),
/// unwinding drops the guard and clears the fault. Prefer this over a bare
/// [`arm_error_at`] when the armed window spans code that can fail an assertion.
#[cfg(test)]
pub(crate) struct FaultGuard {
    name: String,
    /// When `true`, Drop additionally PANICS if `name` was never hit while the guard
    /// was alive (a fault armed at a checkpoint the op never reached). `false` for the
    /// plain [`FaultGuard::error_at`] / [`FaultGuard::panic_at`] guards, which keep
    /// their original "arm + disarm-on-drop, never panic" behaviour.
    must_fire: bool,
}

#[cfg(test)]
impl FaultGuard {
    /// Arms a fire-once `Err` at `name` and returns a guard that disarms on drop.
    pub(crate) fn error_at(name: &str) -> Self {
        arm_error_at(name);
        Self { name: name.to_string(), must_fire: false }
    }

    /// Arms a fire-once `panic!` at `name` and returns a guard that disarms on drop.
    pub(crate) fn panic_at(name: &str) -> Self {
        arm_panic_at(name);
        Self { name: name.to_string(), must_fire: false }
    }

    /// Arms a fire-once `Err` at `name` AND requires it to be hit: if the guarded scope
    /// ends without the op ever reaching `checkpoint(name)`, Drop PANICS.
    ///
    /// This makes a crash-window test fail LOUDLY instead of passing as a false "no
    /// crash here" when the fault was armed at a name the op never visits — a mis-typed
    /// checkpoint, or a destructive section that ran on a different thread than the one
    /// that armed the fault (faults are thread-local). Use this whenever a test's whole
    /// point is that a specific checkpoint MUST be exercised. The Drop panic is
    /// suppressed if the thread is already unwinding (so an earlier assertion failure,
    /// not this guard, surfaces — avoiding a double-panic abort).
    pub(crate) fn error_at_must_fire(name: &str) -> Self {
        arm_error_at(name);
        watch_must_fire(name);
        Self { name: name.to_string(), must_fire: true }
    }
}

#[cfg(test)]
impl Drop for FaultGuard {
    fn drop(&mut self) {
        disarm(&self.name);
        if self.must_fire {
            let hits = take_hits(&self.name);
            assert!(
                hits > 0 || std::thread::panicking(),
                "fault_inject: must-fire checkpoint {:?} was never hit — the destructive \
                 op never reached it (wrong checkpoint name, or it ran on a different \
                 thread than the one that armed the fault)",
                self.name
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::panic::{self, AssertUnwindSafe};

    #[test]
    fn unarmed_checkpoint_returns_ok() {
        // The production default path: nothing armed => Ok with no side effects.
        checkpoint("anything").expect("an un-armed checkpoint must be Ok");
        checkpoint("another").expect("a second un-armed checkpoint must also be Ok");
    }

    #[test]
    fn armed_error_fires_only_for_its_own_name() {
        arm_error_at("swap");
        // A DIFFERENT checkpoint name is unaffected while "swap" is armed.
        checkpoint("write_config").expect("an un-armed checkpoint stays Ok while swap is armed");

        let err = checkpoint("swap").expect_err("the armed checkpoint must return Err");
        assert!(
            err.to_string().contains("swap"),
            "the error message must name the checkpoint, got: {err}"
        );
    }

    #[test]
    fn fire_once_auto_disarms_after_the_first_hit() {
        arm_error_at("once");
        checkpoint("once").expect_err("the first hit must fire");
        checkpoint("once").expect("the second hit must be Ok — fire-once auto-disarmed");
    }

    #[test]
    fn repeating_fault_fires_every_hit_until_disarmed() {
        arm_error_repeating("loop");
        checkpoint("loop").expect_err("the first hit must fire");
        checkpoint("loop").expect_err("the second hit must still fire (repeating)");
        disarm("loop");
        checkpoint("loop").expect("Ok after an explicit disarm");
    }

    #[test]
    fn armed_panic_unwinds_and_is_catchable() {
        arm_panic_at("boom");
        let caught = panic::catch_unwind(AssertUnwindSafe(|| checkpoint("boom")));
        assert!(caught.is_err(), "an armed panic checkpoint must unwind");
        // Fire-once: the panic disarmed it before unwinding, so a re-hit is Ok.
        checkpoint("boom").expect("the panic fault was fire-once");
    }

    #[test]
    fn disarm_all_clears_every_armed_fault() {
        arm_error_at("a");
        arm_panic_at("b");
        disarm_all();
        checkpoint("a").expect("disarm_all must clear a");
        checkpoint("b").expect("disarm_all must clear b (without firing the panic)");
    }

    #[test]
    fn fault_guard_arms_within_scope_and_disarms_on_drop() {
        // Arms while the guard is alive:
        {
            let _guard = FaultGuard::error_at("g1");
            checkpoint("g1").expect_err("the guard must arm the fault while it is alive");
        }
        // Disarms on drop even when the checkpoint was never hit (so fire-once can't
        // be what cleared it — only Drop can):
        {
            let _guard = FaultGuard::error_at("g2");
        }
        checkpoint("g2").expect("the guard's Drop must disarm a fault that was never hit");
    }

    #[test]
    fn fault_guard_panic_at_arms_and_disarms_on_drop() {
        {
            let _guard = FaultGuard::panic_at("pg");
            let caught = panic::catch_unwind(AssertUnwindSafe(|| checkpoint("pg")));
            assert!(caught.is_err(), "the panic guard must arm a panic fault");
        }
        checkpoint("pg").expect("the panic guard must disarm on drop");
    }

    #[test]
    fn must_fire_guard_is_satisfied_when_its_checkpoint_is_hit() {
        {
            let _guard = FaultGuard::error_at_must_fire("reached");
            // The op reaches the checkpoint: it records the hit AND fires the armed
            // fire-once error, exactly as a real aborting op would.
            checkpoint("reached").expect_err("the must-fire guard must arm the error");
        }
        // Drop saw the hit, so it must NOT panic, and it cleans up after itself: the
        // fire-once fault disarmed and the HITS entry was taken, so a later checkpoint
        // is a clean Ok.
        checkpoint("reached").expect("a satisfied must-fire guard leaves no residue");
    }

    #[test]
    fn must_fire_guard_panics_on_drop_when_its_checkpoint_is_never_hit() {
        // Run on a spawned thread so its Drop-time panic is captured by `join` instead
        // of failing this test thread. The guard arms a fault at "never_reached" but the
        // closure never calls `checkpoint("never_reached")` — simulating a fault armed at
        // a name the op never visits (a typo, or a different-thread destructive section).
        let outcome = std::thread::spawn(|| {
            let _guard = FaultGuard::error_at_must_fire("never_reached");
        })
        .join();

        let payload = outcome.expect_err("dropping an un-hit must-fire guard must panic");
        let message = payload
            .downcast_ref::<String>()
            .map(String::as_str)
            .or_else(|| payload.downcast_ref::<&str>().copied())
            .unwrap_or("");
        assert!(
            message.contains("never_reached") && message.contains("never hit"),
            "the panic must name the un-hit checkpoint, got: {message:?}"
        );
    }

    #[test]
    fn faults_are_thread_local_and_invisible_across_threads() {
        // Repeating so the fault is still armed on THIS thread after the spawned
        // thread's check, proving isolation in both directions.
        arm_error_repeating("cross");
        let seen_on_other_thread =
            std::thread::spawn(|| checkpoint("cross").is_ok()).join().expect("spawned thread join");
        assert!(
            seen_on_other_thread,
            "a fault armed on this thread must be invisible to another thread"
        );
        checkpoint("cross").expect_err("the fault must still be armed on the arming thread");
        disarm("cross");
    }
}
