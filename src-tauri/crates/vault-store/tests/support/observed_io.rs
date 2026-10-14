//! Responsibilities: observe real filesystem opens while forwarding Turso IO unchanged.
//! Not responsible for: implementing encryption or faking engine responses.
//! Failure modes: caller IO is ignored for a database; opening's IO test checks every path.
use std::sync::{Arc, Mutex};
use turso::core::{
    Clock, Completion, File, IO, MonotonicInstant, OpenFlags, PlatformIO, WallClockInstant,
};
pub struct ObservedIo {
    inner: PlatformIO,
    pub opened: Mutex<Vec<String>>,
}
impl ObservedIo {
    pub fn new() -> Self {
        Self { inner: PlatformIO::new().unwrap(), opened: Mutex::new(Vec::new()) }
    }
}
impl Clock for ObservedIo {
    fn current_time_monotonic(&self) -> MonotonicInstant {
        self.inner.current_time_monotonic()
    }
    fn current_time_wall_clock(&self) -> WallClockInstant {
        self.inner.current_time_wall_clock()
    }
}
impl IO for ObservedIo {
    fn open_file(
        &self,
        path: &str,
        flags: OpenFlags,
        direct: bool,
    ) -> turso::core::Result<Arc<dyn File>> {
        self.opened.lock().unwrap().push(path.to_owned());
        self.inner.open_file(path, flags, direct)
    }
    fn remove_file(&self, path: &str) -> turso::core::Result<()> {
        self.inner.remove_file(path)
    }
    fn step(&self) -> turso::core::Result<()> {
        self.inner.step()
    }
    fn cancel(&self, c: &[Completion]) -> turso::core::Result<()> {
        self.inner.cancel(c)
    }
}
