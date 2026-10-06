//! Responsibilities: durable queue payloads and progress contracts.
//! Not responsible for: SQL or provider execution.
//! Failure modes: retry loses cursor, provider selection drifts, or lease loss is hidden.
use super::*;

/// Every queue job remains reachable beyond the status preview.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiJobPage {
    pub items: Vec<AiQueueJob>,
    pub next_cursor: Option<i64>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
/// Payload stored for assistant-question jobs.
pub struct AssistantJobPayload {
    pub request: AiAssistantRequest,
    pub llm_provider_id: String,
    pub embedding_provider_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
/// Aggregate queued/running/failed counts for a subset of AI job types.
pub struct QueueJobCounts {
    pub queued: u32,
    pub running: u32,
    pub failed: u32,
}

/// Resumable progress for a chunked embedding backfill (02 §C.6 R1).
///
/// Persisted INSIDE the index job payload so a worker restart resumes from where it stopped
/// instead of re-embedding from scratch. `next_history_id` is the lowest canonical `history_id`
/// the backfill has NOT yet processed; the embed loop scans candidates with
/// `history_id >= next_history_id` in ascending order and advances the watermark after each chunk
/// is durably persisted. `embedded_so_far` is a monotone progress counter surfaced to status.
///
/// Default (`{ next_history_id: 0, embedded_so_far: 0 }`) means "start from the beginning", so a
/// legacy payload without this field (serde default) resumes correctly as a fresh backfill.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IndexBackfillCursor {
    /// Lowest canonical `history_id` not yet embedded; the next chunk starts here.
    pub next_history_id: i64,
    /// Count of vectors durably persisted so far across all chunks of this backfill.
    pub embedded_so_far: u64,
    /// The max canonical `history_id` captured at the TRUE start of the build — the honest determinate
    /// scan denominator, so `next_history_id / scan_target` reaches ~100% as the scan completes.
    ///
    /// `#[serde(default)]` (default 0 = unknown) keeps EXISTING persisted payloads back-compatible: a
    /// cursor written before this field deserializes with `scan_target = 0`. Set ONCE at the build's
    /// true start; a resume PRESERVES it (see [`persist_index_cursor`]) rather than recomputing against
    /// a corpus that may have grown, so the bar measures the build's own original scan range.
    #[serde(default)]
    pub scan_target: i64,
    /// The number of candidate pages this build set out to embed, captured ONCE at the build's true
    /// start (non-reverted visits NOT yet embedded for this provider — so a full rebuild, whose wipe
    /// already cleared them, counts every candidate, while an incremental pass counts only the new
    /// pages). `embedded_so_far / embed_target` is the HONEST progress the user reads as "{embedded} of
    /// {N} pages" — it tracks vectors actually written, not a sparse history-id position. Same
    /// back-compat + preserve-on-resume contract as [`Self::scan_target`]: `#[serde(default)]`
    /// (0 = unknown) keeps legacy payloads valid, and a resume keeps the stored value rather than
    /// recounting a corpus that grew (or got partially embedded) under the build.
    #[serde(default)]
    pub embed_target: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
/// Typed payload stored inside one persistent AI queue row.
pub enum AiJobPayload {
    Index {
        request: AiIndexRequest,
        /// Resumable backfill watermark; `default` for jobs enqueued before resumability or for
        /// a fresh build. Updated in place as chunks are persisted so a restart resumes.
        #[serde(default)]
        cursor: IndexBackfillCursor,
    },
    Assistant {
        payload: AssistantJobPayload,
    },
}

#[derive(Debug, Clone)]
/// Raw queue row loaded from SQLite before it becomes a public read model.
pub struct StoredAiJob {
    pub id: i64,
    pub job_type: AiQueueJobType,
    pub attempt: u32,
    pub max_attempts: u32,
    pub payload: AiJobPayload,
}

#[derive(Debug, Clone, Default)]
/// Failure bookkeeping used when transitioning a queue job to failed state.
pub struct AiJobFailure {
    pub error_code: Option<String>,
    pub error_message: String,
    pub retryable: bool,
    pub retry_after_seconds: u64,
    pub summary: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CursorPersistOutcome {
    /// The running index job's cursor was updated (the `WHERE … state='running'` UPDATE matched).
    Persisted,
    /// The job's lease was reclaimed (it exists as an index job but is no longer `running` under
    /// this worker), so the UPDATE matched 0 rows. The embed loop treats this as a lease loss.
    LeaseLost,
    /// The job vanished or is not an index payload — nothing to persist, and not a lease loss.
    NotIndexJob,
}
