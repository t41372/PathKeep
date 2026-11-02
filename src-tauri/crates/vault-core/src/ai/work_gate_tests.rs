//! Responsibilities: preserve queued work while a previous semantic task owns admission.
//! Not responsible for: executing tasks. Failure modes: busy polling, dropped warm requests,
//! overlapping writers, or a mutex retained across the admitted task's database/file work.
use super::work_gate::SemanticWorkGate;
use std::sync::{Arc, mpsc};
use std::time::Duration;
#[test]
fn waiting_work_runs_after_release_and_the_task_holds_no_mutex() {
    let gate = Arc::new(SemanticWorkGate::new());
    let first = gate.enter();
    assert!(gate.busy.try_lock().is_ok());
    assert!(gate.try_enter().is_none());
    let (entered, rx) = mpsc::channel();
    let waiting = gate.clone();
    let task = std::thread::spawn(move || {
        let _permit = waiting.enter();
        entered.send(()).unwrap();
    });
    assert!(rx.recv_timeout(Duration::from_millis(20)).is_err());
    drop(first);
    rx.recv_timeout(Duration::from_secs(2)).unwrap();
    task.join().unwrap();
    assert!(gate.try_enter().is_some());
}
