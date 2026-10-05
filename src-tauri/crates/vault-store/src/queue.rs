//! Responsibilities: bound pending work and choose interactive work before each batch.
//! Not responsible for: executing SQL or limiting caller-owned inputs.
//! Failure modes: batch backlog starves interactive work; producers flood the queue;
//! shutdown strands blocked producers; wakeups are lost. Tests: queue tests.
use crate::{Error, Result};
use std::{
    collections::VecDeque,
    sync::{Condvar, Mutex},
};

/// Interactive work is always chosen before the next pending batch.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Priority {
    Interactive,
    Batch,
}
struct State<T> {
    interactive: VecDeque<T>,
    batch: VecDeque<T>,
    closed: bool,
    wake: bool,
}
pub(crate) enum Next<T> {
    Job(Priority, T),
    Maintenance,
    Closed,
}
pub(crate) struct Queue<T> {
    state: Mutex<State<T>>,
    changed: Condvar,
    capacity: usize,
}

impl<T> Queue<T> {
    pub fn new(capacity: usize) -> Self {
        Self {
            state: Mutex::new(State {
                interactive: VecDeque::new(),
                batch: VecDeque::new(),
                closed: false,
                wake: false,
            }),
            changed: Condvar::new(),
            capacity,
        }
    }
    pub fn push(&self, priority: Priority, job: T) -> Result<()> {
        let mut s = self.state.lock().unwrap();
        loop {
            if s.closed {
                return Err(Error::Closed);
            }
            let full = match priority {
                Priority::Interactive => s.interactive.len() >= self.capacity,
                Priority::Batch => !s.batch.is_empty(),
            };
            if !full {
                break;
            }
            s = self.changed.wait(s).unwrap();
        }
        match priority {
            Priority::Interactive => s.interactive.push_back(job),
            Priority::Batch => s.batch.push_back(job),
        }
        self.changed.notify_all();
        Ok(())
    }
    pub fn next(&self) -> Next<T> {
        let mut s = self.state.lock().unwrap();
        loop {
            let job = s
                .interactive
                .pop_front()
                .map(|j| (Priority::Interactive, j))
                .or_else(|| s.batch.pop_front().map(|j| (Priority::Batch, j)));
            if let Some((p, j)) = job {
                self.changed.notify_all();
                return Next::Job(p, j);
            }
            if s.closed {
                return Next::Closed;
            }
            if s.wake {
                s.wake = false;
                return Next::Maintenance;
            }
            s = self.changed.wait(s).unwrap();
        }
    }
    pub fn pending(&self) -> (usize, usize) {
        let s = self.state.lock().unwrap();
        (s.interactive.len(), s.batch.len())
    }
    pub fn wake(&self) {
        self.state.lock().unwrap().wake = true;
        self.changed.notify_all();
    }
    pub fn close(&self) {
        self.state.lock().unwrap().closed = true;
        self.changed.notify_all();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{sync::Arc, thread};
    #[test]
    fn interactive_precedes_pending_batch_and_close_wakes_producer() {
        let q = Arc::new(Queue::new(2));
        q.push(Priority::Batch, 1).unwrap();
        q.push(Priority::Interactive, 2).unwrap();
        assert!(matches!(q.next(), Next::Job(Priority::Interactive, 2)));
        let blocked = q.clone();
        let task = thread::spawn(move || blocked.push(Priority::Batch, 3));
        q.close();
        assert!(task.join().unwrap().is_err());
        assert!(matches!(q.next(), Next::Job(Priority::Batch, 1)));
        assert!(matches!(q.next(), Next::Closed));
    }
    #[test]
    fn maintenance_notification_is_not_lost() {
        let q = Queue::<()>::new(1);
        q.wake();
        assert!(matches!(q.next(), Next::Maintenance));
    }
}
