//! Responsibilities: preserve recorded model ownership, ready indexes, manual pauses and backup consent.
//! Not responsible for: provider execution, vector files or schema changes.
use super::*;
use crate::{AppConfig, EmbeddingIndexRef, semantic_index};
use vault_store::{OptionalRow, Session, params_owned as params};

pub(super) fn all_active_index_jobs(s: &Session<'_>) -> Result<Vec<(AiQueueJob, AiJobPayload)>> {
    let rows = s.query(
        "SELECT id,job_type,state,priority,attempt,max_attempts,run_id,summary,
         created_at,available_at,started_at,finished_at,heartbeat_at,error_code,error_message,payload_json
         FROM ai_jobs WHERE state IN ('queued','running','paused','stale')
         AND job_type IN ('index-build','index-clear') ORDER BY CASE WHEN state='running' THEN 0 ELSE 1 END,id",
        (), |r| Ok((store::decode_ai_job_row_with_progress(r)?, r.get::<String>(15)?)))?;
    Ok(rows
        .into_iter()
        .filter_map(|(job, json)| serde_json::from_str(&json).ok().map(|payload| (job, payload)))
        .collect())
}

/// Build progress excludes clear-only work; ownership checks still include every index writer.
pub(crate) fn active_jobs(s: &Session<'_>) -> Result<Vec<(AiQueueJob, AiJobPayload)>> {
    Ok(all_active_index_jobs(s)?
        .into_iter()
        .filter(|(job, _)| job.job_type == "index-build")
        .collect())
}

pub(crate) fn payload_pair(
    payload: &AiJobPayload,
    config: &AppConfig,
) -> Option<EmbeddingIndexRef> {
    let AiJobPayload::Index { request, model: Some(model), .. } = payload else {
        return None;
    };
    let id = request.provider_id.as_ref().or(config.ai.embedding_provider_id.as_ref())?;
    let model = model.clone();
    Some(EmbeddingIndexRef { provider_id: id.clone(), model })
}

pub(crate) fn request_index_job(
    s: &Session<'_>,
    config: &AppConfig,
    request: &AiIndexRequest,
    automatic: bool,
) -> Result<Option<AiQueueJob>> {
    if automatic && semantic_index::chosen(config).is_none() {
        return Ok(None);
    }
    let pair = semantic_index::chosen(config)
        .context("select an embedding provider in Settings before building the semantic index")?;
    if request.provider_id.as_deref().is_some_and(|id| id != pair.provider_id) {
        anyhow::bail!("Index builds must use the embedding provider chosen in Settings.");
    }
    let active = all_active_index_jobs(s)?;
    if automatic {
        if !semantic_index::complete(s, &pair)?
            || active
                .iter()
                .any(|(_, payload)| payload_pair(payload, config).as_ref() == Some(&pair))
        {
            return Ok(None);
        }
    } else {
        let mut same = None;
        for (job, payload) in active {
            if job.job_type == "index-build"
                && payload_pair(&payload, config).as_ref() == Some(&pair)
                && same.is_none()
            {
                same = Some(job);
            } else {
                store::cancel_ai_job(s, job.id)?;
            }
        }
        if let Some(job) = same {
            return Ok(Some(if job.state == "paused" {
                store::replay_ai_job(s, job.id, config.ai.job_queue_paused)?
            } else {
                job
            }));
        }
    }
    let request = AiIndexRequest { provider_id: Some(pair.provider_id), ..request.clone() };
    store::enqueue_index_job(s, &request, Some(&pair.model), config.ai.job_queue_paused).map(Some)
}

/// No legacy job may assume today's model, even at cursor zero. Keep its index files.
pub(crate) fn retire_ambiguous_progress(s: &Session<'_>) -> vault_store::Result<()> {
    let rows = s.query("SELECT id,payload_json FROM ai_jobs WHERE state IN ('queued','running','paused','stale') AND job_type IN ('index-build','index-clear')", (), |r| Ok((r.get::<i64>(0)?, r.get::<String>(1)?)))?;
    for (id, json) in rows {
        if let Ok(AiJobPayload::Index { model: None, .. }) = serde_json::from_str(&json) {
            s.execute("UPDATE ai_jobs SET state='cancelled',stop_requested=1,error_code='index-model-unknown',lease_owner=NULL,lease_expires_at=NULL,finished_at=?1,updated_at=?1 WHERE id=?2", params![now_rfc3339(),id])?;
        }
    }
    s.execute(
        "UPDATE ai_index_ledger SET state='incomplete' WHERE state IN ('building','clearing')",
        (),
    )?;
    Ok(())
}

pub(crate) fn mark_incomplete(s: &Session<'_>, job_id: i64) -> Result<()> {
    let json = s
        .query_one("SELECT payload_json FROM ai_jobs WHERE id=?1", [job_id], |r| {
            Ok(r.get::<String>(0)?)
        })
        .optional()?;
    // A corrupt payload has no trustworthy pair identity. The claim path quarantines it;
    // status and cancellation must still work, while actual database errors propagate.
    if let Some(json) = json
        && let Ok(AiJobPayload::Index { request, model: Some(model), .. }) =
            serde_json::from_str(&json)
        && let Some(id) = request.provider_id
    {
        s.execute(
            "UPDATE ai_index_ledger SET state='incomplete' WHERE provider_id=?1 AND model=?2 AND state IN ('building','clearing')",
            params![id, model],
        )?;
    }
    Ok(())
}

pub(crate) fn pause_ai_job(s: &Session<'_>, job_id: i64) -> Result<AiQueueJob> {
    let job = store::load_ai_job(s, job_id)?;
    if !matches!(job.job_type.as_str(), "index-build" | "index-clear") {
        anyhow::bail!("Only index jobs can be paused.");
    }
    if !matches!(job.state.as_str(), "queued" | "running" | "stale") {
        anyhow::bail!("Only queued, running or stale index jobs can be paused.");
    }
    // A cancellation already requested is final; pause must never turn it into resumable work.
    s.execute("UPDATE ai_jobs SET state=CASE WHEN state='running' THEN 'running' ELSE 'paused' END,
        stop_requested=CASE WHEN state='running' THEN 2 ELSE 0 END,error_code='user-paused',error_message=NULL,updated_at=?1
        WHERE id=?2 AND stop_requested<>1", params![now_rfc3339(),job_id])?;
    mark_incomplete(s, job_id)?;
    store::load_ai_job(s, job_id)
}
