//! Responsibilities: serialize semantic workers without retaining a mutex during their work.
//! Not responsible for: queue policy, database transactions or cancellation.
use std::sync::{Condvar, Mutex};

/// Admission survives DB/provider/file calls as a token; waiters sleep without losing their task.
#[derive(Default)]
pub struct SemanticWorkGate {
    pub(super) busy: Mutex<bool>,
    available: Condvar,
}
impl SemanticWorkGate {
    /// Static gates serialize writers and snapshot warming without holding a lock across work.
    pub const fn new() -> Self {
        Self { busy: Mutex::new(false), available: Condvar::new() }
    }
    /// Background tasks wait for their turn instead of dropping required follow-up work.
    pub fn enter(&self) -> SemanticWorkPermit<'_> {
        let mut busy = self.busy.lock().unwrap_or_else(|e| e.into_inner());
        while *busy {
            busy = self.available.wait(busy).unwrap_or_else(|e| e.into_inner());
        }
        *busy = true;
        SemanticWorkPermit(self)
    }
    /// Foreground run-now can return status immediately when another writer already owns work.
    pub fn try_enter(&self) -> Option<SemanticWorkPermit<'_>> {
        let mut busy = self.busy.lock().unwrap_or_else(|e| e.into_inner());
        if *busy {
            return None;
        }
        *busy = true;
        Some(SemanticWorkPermit(self))
    }
}
/// Releasing ownership wakes a waiter even when the task returns through an error or panic.
pub struct SemanticWorkPermit<'a>(&'a SemanticWorkGate);
impl Drop for SemanticWorkPermit<'_> {
    fn drop(&mut self) {
        *self.0.busy.lock().unwrap_or_else(|e| e.into_inner()) = false;
        self.0.available.notify_one();
    }
}
