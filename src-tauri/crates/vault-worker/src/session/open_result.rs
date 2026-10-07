//! Responsibilities: retain one opening attempt's result for all its waiters.
//! Not responsible for: opening storage or holding the session state mutex.
//! Failure modes: a later close/open erases the result a waiting caller joined.
//! Tests: concurrent_openers_receive_the_startup_result and retained_result_survives_new_attempts.
use std::sync::{Condvar, Mutex};
use vault_core::Vault;

#[derive(Default)]
pub(super) struct OpenResult {
    value: Mutex<Option<Result<Vault, String>>>,
    ready: Condvar,
}
impl OpenResult {
    pub(super) fn finish(&self, value: Result<Vault, String>) {
        *self.value.lock().expect("opening result") = Some(value);
        self.ready.notify_all();
    }
    pub(super) fn wait(&self) -> Result<Vault, String> {
        let mut value = self.value.lock().expect("opening result");
        while value.is_none() {
            value = self.ready.wait(value).expect("opening result");
        }
        value.as_ref().unwrap().clone()
    }
}
