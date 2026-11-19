//! Responsibilities: let one writer at a time change a live archive's control (`config.json`) in
//! this process, for as long as it takes to read the control, change its own fields and write it.
//! Not responsible for: other processes (only the owner process writes a live control), the
//! databases (the operation gate and the owner lock), or which fields a writer may change.
//!
//! Every write of a live control is a read-modify-write under this gate, and each writer changes
//! only the fields it owns: a settings save the settings it was given, a mode change and the
//! at-rest repair the mode, the bring-up the stamp, App Lock its own fields. That is what lets
//! settings be saved while an export, a snapshot or a mode change runs for minutes: those take
//! the gate only for their own write, built from the control as it is at that moment, so a
//! setting saved meanwhile is kept. "Delete all data" holds it from reading what to delete to
//! its last removal, so nothing writes a control into a folder it is emptying.
//!
//! Holds are short: no database work and no waiting on another lock while holding it. A writer
//! that also needs the operation gate takes that one first.
//!
//! Tests: below (exclusion); `maintenance::tests` (the at-rest repair and a mode change keep a
//! setting saved meanwhile); vault-worker `control_gate_tests` (a save during a long operation,
//! after a wipe, and provider keys).

use crate::ProjectPaths;
use std::{
    collections::HashSet,
    path::PathBuf,
    sync::{Condvar, Mutex, OnceLock},
};

fn held() -> &'static (Mutex<HashSet<PathBuf>>, Condvar) {
    static HELD: OnceLock<(Mutex<HashSet<PathBuf>>, Condvar)> = OnceLock::new();
    HELD.get_or_init(Default::default)
}

/// The right to write one archive's control. [`crate::save_config`] takes it, so a write without
/// it does not compile.
#[derive(Debug)]
pub struct ControlGate {
    paths: ProjectPaths,
    key: PathBuf,
}

impl ControlGate {
    /// Waits until no other writer in this process holds the gate for this archive.
    pub fn acquire(paths: &ProjectPaths) -> ControlGate {
        let key =
            std::path::absolute(&paths.config_path).unwrap_or_else(|_| paths.config_path.clone());
        let (mutex, released) = held();
        let mut held = mutex.lock().unwrap_or_else(|e| e.into_inner());
        while held.contains(&key) {
            held = released.wait(held).unwrap_or_else(|e| e.into_inner());
        }
        held.insert(key.clone());
        ControlGate { paths: paths.clone(), key }
    }

    pub(crate) fn try_acquire(paths: &ProjectPaths) -> Option<Self> {
        let key =
            std::path::absolute(&paths.config_path).unwrap_or_else(|_| paths.config_path.clone());
        held()
            .0
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(key.clone())
            .then(|| Self { paths: paths.clone(), key })
    }

    /// The archive whose control this gate is for.
    pub fn paths(&self) -> &ProjectPaths {
        &self.paths
    }
}

impl Drop for ControlGate {
    fn drop(&mut self) {
        let (mutex, released) = held();
        mutex.lock().unwrap_or_else(|e| e.into_inner()).remove(&self.key);
        released.notify_all();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{sync::mpsc, time::Duration};

    #[test]
    fn a_second_writer_waits_until_the_first_is_done_and_another_archive_does_not() {
        let root = tempfile::tempdir().unwrap();
        let paths = crate::project_paths_with_root(&root.path().join("one"));
        let other = crate::project_paths_with_root(&root.path().join("two"));
        let first = ControlGate::acquire(&paths);
        drop(ControlGate::acquire(&other));
        let (sent, took) = mpsc::channel();
        let waiting = paths.clone();
        let second = std::thread::spawn(move || {
            let gate = ControlGate::acquire(&waiting);
            sent.send(()).unwrap();
            drop(gate);
        });
        assert!(took.recv_timeout(Duration::from_millis(300)).is_err(), "it did not wait");
        drop(first);
        took.recv_timeout(Duration::from_secs(10)).expect("it went on once the first was done");
        second.join().unwrap();
    }
}
