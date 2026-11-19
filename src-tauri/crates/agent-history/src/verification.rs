//! Responsibilities: audit retained evidence through the same authenticated readers as export.
//! Not responsible for: repairing sources, rebuilding projections or deciding retention.
use crate::{Archive, Error, Result, SearchQuery};
use serde::Serialize;
use vault_store::{Db, params};

/// Integrity and interpretation coverage are separate: intact bytes may be unparseable.
#[derive(Debug, Serialize)]
pub struct VerifyReport {
    pub status: String,
    pub slices: SliceUsage,
    pub revisions: u64,
    pub bytes: u64,
    pub databases: usize,
    pub search_current: bool,
    pub normalization_complete: bool,
}

/// Stored compressed size is separate from exported logical revision lengths.
#[derive(Debug, Serialize)]
pub struct SliceUsage {
    pub count: u64,
    pub bytes: u64,
}

impl Archive {
    /// Re-reads every published raw revision in bounded chunks, including old and
    /// unparseable sources. No source files or derived state are changed.
    pub fn verify(&self) -> Result<VerifyReport> {
        let _job = self.jobs.enter(None)?;
        for db in [Db::AgentHistoryReadModel, Db::AgentHistoryEvidence] {
            let messages = self.store.read(db, |tx| tx.integrity_check())?;
            if messages.len() != 1 || messages[0] != "ok" {
                return Err(Error::Integrity("database consistency check failed".into()));
            }
        }
        let mut report = VerifyReport {
            status: "complete".into(),
            slices: self.store.read(Db::AgentHistoryEvidence, |tx| {
                tx.query_one(
                    "SELECT count(*), coalesce(sum(length(data)), 0) FROM ah_chunks",
                    (),
                    |r| {
                        Ok(SliceUsage {
                            count: r.get::<i64>(0)? as u64,
                            bytes: r.get::<i64>(1)? as u64,
                        })
                    },
                )
            })?,
            revisions: 0,
            bytes: 0,
            databases: 2,
            search_current: true,
            normalization_complete: self.normalization_complete()?,
        };
        let mut after = 0i64;
        loop {
            let page = self.store.read(Db::AgentHistoryEvidence, move |tx| {
                let mut statement = tx.prepare(
                    "SELECT id FROM ah_revisions WHERE state='complete' AND bytes_retained=1
                        AND id>?1 ORDER BY id LIMIT 128",
                )?;
                statement
                    .query_map(params![after], |row| Ok(row.get::<i64>(0)?))?
                    .collect::<vault_store::Result<Vec<_>>>()
            })?;
            if page.is_empty() {
                break;
            }
            for id in &page {
                after = *id;
                let input = self.raw_reader(*id);
                let mut reader = match input {
                    Ok(reader) => reader,
                    Err(Error::NotFound(_)) if !self.revision(*id)?.bytes_retained => continue,
                    Err(error) => return Err(error),
                };
                match std::io::copy(&mut reader, &mut std::io::sink()) {
                    Ok(bytes) => {
                        report.bytes += bytes;
                        report.revisions += 1;
                    }
                    Err(error) if crate::archive::expired_read(&error) => {}
                    Err(error) => return Err(error.into()),
                }
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
