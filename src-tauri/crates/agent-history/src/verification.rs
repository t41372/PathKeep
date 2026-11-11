//! Responsibilities: audit retained evidence through the same authenticated readers as export.
//! Not responsible for: repairing sources, rebuilding projections or deciding retention.
use crate::{Archive, Error, Result, SearchQuery};
use serde::Serialize;
use vault_store::{Db, params};

/// Integrity and interpretation coverage are separate: intact bytes may be unparseable.
#[derive(Debug, Serialize)]
pub struct VerifyReport {
    pub status: String,
    pub revisions: u64,
    pub bytes: u64,
    pub databases: usize,
    pub search_current: bool,
    pub normalization_complete: bool,
}

impl Archive {
    /// Re-reads every published raw revision in bounded chunks, including old and
    /// unparseable sources. No source files or derived state are changed.
    pub fn verify(&self) -> Result<VerifyReport> {
        for db in [Db::Archive, Db::SourceEvidence] {
            let messages = self.store.read(db, |tx| tx.integrity_check())?;
            if messages.len() != 1 || messages[0] != "ok" {
                return Err(Error::Integrity("database consistency check failed".into()));
            }
        }
        let mut report = VerifyReport {
            status: "complete".into(),
            revisions: 0,
            bytes: 0,
            databases: 2,
            search_current: true,
            normalization_complete: self.store.read(Db::Archive, |tx| {
                tx.query_one(
                    "SELECT NOT EXISTS(SELECT 1 FROM ah_coverage WHERE complete=0)",
                    (),
                    |row| Ok(row.get::<i64>(0)?),
                )
                .map(|value| value != 0)
            })?,
        };
        let mut after = 0i64;
        loop {
            let page = self.store.read(Db::SourceEvidence, move |tx| {
                let mut statement = tx.prepare(
                    "SELECT id FROM ah_revisions WHERE state='complete' AND id>?1 ORDER BY id LIMIT 128",
                )?;
                statement.query_map(params![after], |row| Ok(row.get::<i64>(0)?))?
                    .collect::<vault_store::Result<Vec<_>>>()
            })?;
            if page.is_empty() {
                break;
            }
            for id in &page {
                report.bytes += std::io::copy(&mut self.raw_reader(*id)?, &mut std::io::sink())?;
                report.revisions += 1;
                after = *id;
            }
        }
        if self.projection_generation()? > 0 {
            match self.search(&SearchQuery {
                text: "pathkeepverificationprobe".into(),
                harness: None,
                project_path: None,
                limit: 1,
            }) {
                Ok(_) => {}
                Err(Error::IndexBehind) => {
                    report.search_current = false;
                    report.status = "partial".into();
                }
                Err(error) => return Err(error),
            }
        }
        Ok(report)
    }
}
