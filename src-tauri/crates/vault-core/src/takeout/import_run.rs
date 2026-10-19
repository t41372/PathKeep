//! Responsibilities: mark partially applied imports interrupted on every exit path.
//! Not responsible for: parsing, transaction scheduling or rolling back committed batches.
//! Failure modes: parse/cancel/panic leaves a phantom running run or batch; committed
//! progress is erased by error finalization. Tests: takeout cancellation tests.

use crate::{Vault, utils::now_rfc3339};
use anyhow::Result;
use vault_store::{Db, Priority, params};

pub(super) struct ImportRun<'a> {
    vault: &'a Vault,
    run_id: i64,
    batch_id: i64,
    finished: bool,
}
impl<'a> ImportRun<'a> {
    pub(super) fn new(vault: &'a Vault, run_id: i64, batch_id: i64) -> Self {
        Self { vault, run_id, batch_id, finished: false }
    }
    pub(super) fn interrupt(&self, error: &anyhow::Error) -> Result<()> {
        let status = crate::archive::write_runs::failure_status(error);
        let (run, batch, message) = (self.run_id, self.batch_id, format!("{error:#}"));
        self.vault.write(Db::Archive,Priority::Batch,move |s| {
            s.execute("UPDATE runs SET status=?4,finished_at=?2,error_message=?3 WHERE id=?1 AND status='running'",
                params![run,now_rfc3339(),message,status])?;
            s.execute("UPDATE import_batches SET status=?2,previous_status=?2 WHERE id=?1 AND status='running'",params![batch,status])?;
            Ok(())
        })
    }
    pub(super) fn run<T>(mut self, work: impl FnOnce() -> Result<T>) -> Result<T> {
        let result = match work() {
            Ok(value) => Ok(value),
            Err(error) => {
                let saved = self.interrupt(&error);
                Err(with_review_context(error, saved))
            }
        };
        self.finished = true;
        result
    }
}
impl Drop for ImportRun<'_> {
    fn drop(&mut self) {
        if !self.finished {
            let _ = self.interrupt(&anyhow::Error::from(vault_store::Error::cancelled()));
        }
    }
}

pub(super) fn with_review_context(error: anyhow::Error, saved: Result<()>) -> anyhow::Error {
    match saved {
        Ok(()) => error,
        Err(save_error) => {
            error.context(format!("also failed to save import review: {save_error:#}"))
        }
    }
}
