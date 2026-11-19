//! Responsibilities: freeze and measure a visit-id range through short, resumable reads.
//! Not responsible for embedding, source files, publication or change-driven update scheduling.
use super::super::{AiRunControl, IndexBackfillProgress};
use crate::{
    IndexBuildPhase, Vault,
    ai_queue::{AiJobPayload, IndexBackfillCursor, IndexScan},
};
use anyhow::{Result, ensure};
use std::sync::Arc;
use vault_store::{Db, Priority, params_owned as params};

pub(super) const PAGE_SIZE: usize = 4096;
const PLAN_CHECKPOINT_ROWS: u64 = 16_384;
pub(super) const VISIT_CHECKPOINT_ROWS: u64 = 1024;

/// Captures both inputs in one read transaction. The watermark must never acknowledge later visits.
pub(super) fn capture_input(vault: &Vault) -> Result<(i64, i64)> {
    vault.read(Db::Archive, |s| {
        let upper = s.query_one(
            "SELECT COALESCE((SELECT id FROM visits ORDER BY id DESC LIMIT 1),0)",
            (),
            |r| Ok(r.get::<i64>(0)?),
        )?;
        let watermark = super::super::source_watermark::read(s, Db::Archive)?;
        Ok((upper, watermark))
    })
}

/// Preparation can stop before the first visit cursor advances; its source identity still resumes.
pub(super) fn has_scope(vault: &Vault, job: Option<i64>) -> Result<bool> {
    let Some(job) = job else { return Ok(false) };
    vault.read(Db::Intelligence, move |s| {
        let AiJobPayload::Index { cursor, .. } =
            crate::ai_queue::store::load_ai_job_payload(s, job)?
        else {
            return Ok(false);
        };
        Ok(cursor.scan.is_some_and(|scan| scan.version == 1))
    })
}

pub(super) fn phase(vault: &Vault, job: Option<i64>, phase: IndexBuildPhase) -> Result<()> {
    if let Some(job) = job {
        vault.write(Db::Intelligence, Priority::Batch, move |s| {
            let AiJobPayload::Index { mut cursor, .. } =
                crate::ai_queue::store::load_ai_job_payload(s, job)?
            else {
                anyhow::bail!("index payload changed")
            };
            if let Some(scan) = &mut cursor.scan {
                scan.phase = phase;
            }
            ensure!(
                crate::ai_queue::store::persist_index_cursor(s, job, &cursor, None)?
                    == crate::ai_queue::CursorPersistOutcome::Persisted,
                "index job lost its queue lease"
            );
            Ok(())
        })?;
    }
    Ok(())
}

pub(super) struct ScanPlan {
    pub cursor: IndexBackfillCursor,
    job: Option<i64>,
}
impl ScanPlan {
    pub fn open(
        vault: &Vault,
        job: Option<i64>,
        upper: i64,
        watermark: i64,
        resume: IndexBackfillProgress,
        control: Option<&Arc<dyn AiRunControl>>,
    ) -> Result<Self> {
        let mut cursor = if let Some(job) = job {
            vault.read(Db::Intelligence, move |s| {
                let AiJobPayload::Index { cursor, .. } =
                    crate::ai_queue::store::load_ai_job_payload(s, job)?
                else {
                    anyhow::bail!("index payload changed")
                };
                Ok(cursor)
            })?
        } else {
            IndexBackfillCursor::default()
        };
        let known = cursor.scan.is_some_and(|scan| scan.version == 1);
        let rewind = known && resume.next_history_id < cursor.next_history_id;
        if known {
            ensure!(
                resume.next_history_id <= cursor.next_history_id,
                "index resume is ahead of durable progress"
            );
        }
        // Legacy payloads cannot prove the new measurement. Recheck their prefix without wiping sources.
        cursor.next_history_id = if known || job.is_none() { resume.next_history_id } else { 0 };
        cursor.embedded_so_far = resume.embedded_so_far;
        cursor.updated_so_far = resume.updated_so_far;
        if !known {
            cursor.scan = Some(IndexScan {
                version: 1,
                upper_history_id: upper,
                source_watermark: watermark,
                rebuild_from_history_id: resume.next_history_id,
                ..Default::default()
            });
            cursor.scan_target = upper;
        }
        let mut plan = Self { cursor, job };
        if plan.scan().phase == IndexBuildPhase::Planning {
            plan.save(vault)?;
            let mut saved_total = plan.scan().total;
            while plan.scan().planning_after < plan.scan().upper_history_id {
                if let Err(stop) = super::super::control::checkpoint_ai_run(
                    control,
                    super::backfill::COUNT_STOPPED,
                ) {
                    if saved_total != plan.scan().total {
                        plan.save(vault)?;
                    }
                    return Err(stop);
                }
                let scan = plan.scan();
                let ids = read_ids(
                    vault,
                    scan.planning_after.saturating_add(1),
                    scan.upper_history_id,
                    PAGE_SIZE,
                )?;
                let progress = plan.cursor.scan.as_mut().unwrap();
                progress.total += ids.len() as u64;
                progress.planning_after = ids.last().copied().unwrap_or(scan.upper_history_id);
                if progress.total - saved_total >= PLAN_CHECKPOINT_ROWS {
                    saved_total = progress.total;
                    plan.save(vault)?;
                }
            }
            plan.cursor.scan.as_mut().unwrap().phase = IndexBuildPhase::Sources;
            plan.save(vault)?;
        }
        if rewind || (!known && plan.cursor.next_history_id > 0) {
            let mut after = 0;
            let mut examined = 0;
            while after < plan.cursor.next_history_id {
                super::super::control::checkpoint_ai_run(control, super::backfill::COUNT_STOPPED)?;
                let ids = read_ids(vault, after, plan.cursor.next_history_id - 1, PAGE_SIZE)?;
                let Some(last) = ids.last() else { break };
                examined += ids.len() as u64;
                after = last + 1;
            }
            plan.cursor.scan.as_mut().unwrap().examined = examined;
            plan.save(vault)?;
        }
        Ok(plan)
    }
    pub fn scan(&self) -> IndexScan {
        self.cursor.scan.expect("scope precedes scan")
    }
    pub fn progress(&self) -> IndexBackfillProgress {
        IndexBackfillProgress {
            scan: self.cursor.scan,
            next_history_id: self.cursor.next_history_id,
            embedded_so_far: self.cursor.embedded_so_far,
            updated_so_far: self.cursor.updated_so_far,
            scan_target: self.cursor.scan_target,
            embed_target: self.cursor.embed_target,
        }
    }
    fn save(&self, vault: &Vault) -> Result<()> {
        if let Some(job) = self.job {
            let cursor = self.cursor.clone();
            vault.write(Db::Intelligence, Priority::Batch, move |s| {
                ensure!(
                    crate::ai_queue::store::persist_index_cursor(s, job, &cursor, None)?
                        == crate::ai_queue::CursorPersistOutcome::Persisted,
                    "index job lost its queue lease"
                );
                Ok(())
            })?;
        }
        Ok(())
    }
}
fn read_ids(vault: &Vault, start: i64, upper: i64, limit: usize) -> Result<Vec<i64>> {
    vault.read(Db::Archive, move |s| {
        Ok(s.query(
            "SELECT id FROM visits WHERE id>=?1 AND id<=?2 ORDER BY id LIMIT ?3",
            params![start, upper, limit as i64],
            |r| Ok(r.get::<i64>(0)?),
        )?)
    })
}
