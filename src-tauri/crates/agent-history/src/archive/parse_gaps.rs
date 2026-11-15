//! Responsibilities: retain bounded parser-gap positions for inherited-prefix coverage.
//! Not responsible for: turning missing attachments or unresolved lineage into parser errors.
use super::{Archive, ProjectionAttempt, projection::require_work};
use crate::{ParseReport, Result};
use vault_store::{Db, Priority, params};

impl Archive {
    pub(crate) fn save_projection_gaps(
        &self,
        attempt: &ProjectionAttempt,
        report: &ParseReport,
    ) -> Result<()> {
        if report.complete {
            return Ok(());
        }
        let gaps = report
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.detail.as_deref() != Some("remote"))
            .map(|diagnostic| (diagnostic.ordinal, diagnostic.code.clone()))
            .collect::<Vec<_>>();
        let attempt = attempt.clone();
        self.store.write(Db::AgentHistoryReadModel, Priority::Batch, move |tx| {
            require_work(tx, &attempt)?;
            if gaps.is_empty() {
                tx.execute(
                    "INSERT INTO ah_projection_gaps VALUES(?1,NULL,'unknown')",
                    [attempt.id],
                )?;
            }
            for (ordinal, code) in gaps {
                tx.execute(
                    "INSERT INTO ah_projection_gaps VALUES(?1,?2,?3)",
                    params![attempt.id, ordinal.map(|ordinal| ordinal as i64), code],
                )?;
            }
            Ok(())
        })?;
        Ok(())
    }
}
