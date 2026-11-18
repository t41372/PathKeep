//! Responsibilities: frozen provider/model payloads and durable visit-progress contracts.
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
    /// Fixed visit scope and durable measurement; absent on legacy jobs.
    #[serde(default)]
    pub scan: Option<IndexScan>,
    /// Source identity that makes this scan position meaningful; older cursors restart safely.
    #[serde(default)]
    pub lineage: Option<String>,
    /// Lowest canonical `history_id` not yet embedded; the next chunk starts here.
    pub next_history_id: i64,
    /// Count of visits durably indexed across all chunks of this backfill.
    pub embedded_so_far: u64,
    /// Updated rows are a subset of embedded_so_far; both counts survive retry and pause.
    #[serde(default)]
    pub updated_so_far: u64,
    /// Diagnostic highest id, not a visit-count denominator.
    #[serde(default)]
    pub scan_target: i64,
    /// Legacy missing-row target; retained for diagnostics, never used as scan progress.
    #[serde(default)]
    pub embed_target: u64,
}

/// Physical visit rows are append-only during a job; visibility changes do not change this set.
/// Archive replacement closes the owner. A lineage switch starts another scope.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct IndexScan {
    pub version: u8,
    pub upper_history_id: i64,
    pub source_watermark: i64,
    pub rebuild_from_history_id: i64,
    pub planning_after: i64,
    pub total: u64,
    pub examined: u64,
    pub phase: crate::IndexBuildPhase,
}

impl IndexBackfillCursor {
    /// Unknown legacy totals stay unknown; outcomes never become a scan numerator.
    pub fn progress(&self) -> crate::IndexBuildProgress {
        let scan = self.scan.unwrap_or_default();
        let visits = scan.version == 1 && scan.phase == crate::IndexBuildPhase::Visits;
        crate::IndexBuildProgress {
            phase: scan.phase,
            done: if visits { scan.examined } else { 0 },
            total: (visits && scan.examined <= scan.total).then_some(scan.total),
            new_visits: self.embedded_so_far.saturating_sub(self.updated_so_far),
            updated_visits: self.updated_so_far,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
/// Typed payload stored inside one persistent AI queue row.
pub enum AiJobPayload {
    Index {
        request: AiIndexRequest,
        /// Freeze model identity without changing the archive schema.
        #[serde(default)]
        model: Option<String>,
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
/// Failure bookkeeping for retry, service pause or a local failed outcome.
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
