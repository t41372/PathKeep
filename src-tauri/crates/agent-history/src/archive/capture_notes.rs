//! A file condition has one identity across backups. An occurrence links it
//! to each observing run. Source cards read counts committed with the occurrence.
use super::Archive;
use crate::{Diagnostic, Harness, Result};
use vault_store::{Db, Priority, params};

impl Archive {
    pub(crate) fn record_capture_notes(
        &self,
        run: i64,
        root: &str,
        tool: Harness,
        file: Option<&str>,
        notes: Vec<Diagnostic>,
    ) -> Result<(u64, u64)> {
        let root = root.to_owned();
        let file = file.map(str::to_owned);
        Ok(self.store.write(Db::AgentHistoryEvidence, Priority::Batch, move |tx| {
            let mut seen = 0;
            let mut fresh = 0;
            for note in notes {
                let subject = file.as_deref().or(note.path.as_deref()).unwrap_or("");
                let version = note.future_version().unwrap_or("");
                // A redacted reason distinguishes attachment conditions without storing targets.
                let qualifier = if note.code == "attachment_not_retained" {
                    note.detail.as_deref().unwrap_or("")
                } else {
                    version
                };
                tx.execute(
                    "INSERT OR IGNORE INTO ah_capture_notes(root_id,subject,code,qualifier,first_run,tool)
                     VALUES(?1,?2,?3,?4,?5,?6)",
                    params![root.as_str(), subject, note.code.as_str(), qualifier, run, tool.as_str()],
                )?;
                let (id, first_run) = tx.query_one(
                    "SELECT id,first_run FROM ah_capture_notes
                     WHERE root_id=?1 AND subject=?2 AND code=?3 AND qualifier=?4",
                    params![root.as_str(), subject, note.code.as_str(), qualifier],
                    |row| Ok((row.get::<i64>(0)?, row.get::<i64>(1)?)),
                )?;
                let inserted = tx.execute(
                    "INSERT OR IGNORE INTO ah_run_notes(run_id,note_id,path,ordinal,detail)
                     VALUES(?1,?2,?3,?4,?5)",
                    params![run, id, note.path.as_deref().or(file.as_deref()),
                        note.ordinal.map(|n| n.to_string()), note.detail.as_deref()],
                )?;
                if inserted == 0 {
                    continue;
                }
                let is_new = first_run == run;
                tx.execute(
                    "INSERT INTO ah_run_note_counts(run_id,root_id,tool,code,version,seen,fresh)
                     VALUES(?1,?2,?3,?4,?5,1,?6) ON CONFLICT(run_id,root_id,code,version)
                     DO UPDATE SET seen=seen+1,fresh=fresh+excluded.fresh",
                    params![run, root.as_str(), tool.as_str(), note.code.as_str(), version, i64::from(is_new)],
                )?;
                seen += 1;
                fresh += u64::from(is_new);
            }
            Ok((seen, fresh))
        })?)
    }
}
