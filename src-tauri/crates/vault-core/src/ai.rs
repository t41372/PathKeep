//! Responsibilities: optional AI, embedding service errors and semantic retrieval contracts.
//!
//! This module coordinates provider readiness, semantic indexing, semantic
//! search, assistant runs, and manual integration previews. It sits on top of
//! the canonical archive and must respect PathKeep's core AI boundaries:
//!
//! - AI is additive and optional; the archive remains usable without it
//! - vector/assistant state is rebuildable derived state, not canonical truth
//! - lexical fallback must stay explicit whenever semantic readiness is missing
//!
//! Not responsible for: provider wire encoding or a third-party agent runtime.
//! Why: callers share owned AI boundaries rather than rig types.

mod agent_harness;
mod agent_tools;
mod chat_stream;
mod code_mode;
mod control;
mod dedup;
mod embedding_engine;
mod embedding_error;
mod embedding_external;
mod embedding_prompt;
mod embedding_static;
mod fingerprint;
pub(crate) mod generations;
mod indexing;
mod llm;
mod model_files;
mod model_list;
mod native_ledger;
mod projection_sort;
mod provider;
mod read_model;
mod related_pages;
#[cfg(test)]
mod related_pages_bench;
mod report_pages;
mod reverse_visit_map;
mod search;
#[cfg(test)]
mod search_group_fixture;
mod search_group_signatures;
mod search_group_snapshots;
#[cfg(test)]
mod search_group_snapshots_tests;
mod search_groups;
#[cfg(test)]
mod search_groups_bench;
mod search_groups_cluster;
mod search_pages;
mod search_ranking;
mod search_store;
mod search_vectors;
mod similar_searches;
mod tool_arguments;
mod topics;
mod traits;
mod vector_index;
mod vector_planes;
mod vector_store;
mod visit_content_map;
mod visit_position_map;
mod work_gate;
#[cfg(test)]
mod work_gate_tests;
mod working_set;
pub use work_gate::SemanticWorkGate;

use crate::{
    ai_queue::{self},
    ai_sidecar::{self},
    config::ProjectPaths,
    enrichment::{build_embedding_document, load_best_enrichment_map_by_history_ids},
    models::{
        AiIndexReport, AiIndexRequest, AiIndexStatus, AiIndexWarning, AiProviderCapabilityReport,
        AiProviderConfig, AiProviderConnectionTestReport, AiProviderPurpose, AiQueueJobType,
        AiQueueStatus, AiRequestFormat, AiSearchEntry, AiSearchNote, AiSearchRequest,
        AiSearchResponse, AiSemanticStaleness, AppConfig, HistoryEntry,
    },
    utils::{now_rfc3339, url_domain},
};
use anyhow::{Context, Result};
use iana_time_zone::get_timezone;
use secrecy::SecretString;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::{
    collections::HashMap,
    future::Future,
    sync::Arc,
    time::{Duration, Instant},
};
use thiserror::Error;

/// The enrichment summary an embedding's dedup hash includes; conversion recomputes hashes with it.
pub(crate) use self::indexing::enrichment_summary_for as enrichment_summary_for_embedding;
pub use self::indexing::{
    build_ai_index, build_ai_index_with_control, collect_index_generations,
    collect_index_generations_in_background, forget_unconfigured_indexes,
};
pub use self::model_list::{chat_context_length, list_provider_models};
pub use self::native_ledger::index_success_summary;
pub use self::provider::test_provider_connection;
pub use self::read_model::{
    ai_index_status, ai_queue_status, preview_ai_integrations, provider_capabilities,
    provider_connection_failure_report, reconcile_ai_queue_controls, recoverable_ai_jobs,
};
pub use self::related_pages::{RelatedPage, RelatedPages, related_pages};
pub use self::search::semantic_search_history;

pub use self::agent_harness::{
    AgentJournal, AgentRunOutcome, AgentRunSink, AgentSystemContext, DEFAULT_MAX_ITERATIONS,
    DEFAULT_TOKEN_BUDGET, build_agent_system_context, drive_agent_run, estimate_request_tokens,
    estimate_tokens, resolve_agent_system_context,
};
pub use self::agent_tools::{
    AgentTool, AgentToolContext, HistorySearchTool, ToolOutcome, ToolRegistry,
};
pub use self::chat_stream::{
    deregister_run as deregister_ai_chat_run, drive_chat_stream as drive_ai_chat_stream,
    register_run as register_ai_chat_run, request_cancel as request_ai_chat_cancel,
    request_cancel_all as request_cancel_all_ai_chat_runs,
};
pub use self::code_mode::{
    CodeOutcome, HostCallRecord, LimitsHit, MAX_FETCH_IDS, MAX_GUEST_MEMORY_BYTES, MAX_HOST_CALLS,
    MAX_OUTPUT_BYTES, MAX_ROWS_PER_CALL, WALL_TIME_BUDGET, run_code_in_sandbox,
};
pub use self::dedup::{build_dedup_content_hash, content_key_from_hash};
pub use self::embedding_engine::{AnyEmbeddingProvider, select_embedding_provider};
pub use self::embedding_error::EmbeddingServiceError;
pub use self::embedding_external::ExternalEmbeddingProvider;
pub use self::embedding_static::{
    DEFAULT_STATIC_MODEL_FILES, DEFAULT_STATIC_MODEL_REPO, STATIC_EMBEDDING_DIM,
    STATIC_INAPP_BASE_URL, STATIC_MAX_INPUT_TOKENS, StaticEmbeddingMatrix, StaticEmbeddingProvider,
    StaticTokenizer, parse_static_config, runtime_uses_static, static_embed_ids,
    static_l2_normalize, static_repo_for_runtime,
};
pub use self::fingerprint::{EMBEDDING_FINGERPRINT_VERSION, EmbeddingFingerprint};
pub use self::llm::{RigLlmProvider, probe_tool_capability};
pub use self::model_files::{
    ModelDownloadProgress, ModelFile, NoopDownloadProgress, ensure_model_downloaded,
    model_dir_for_repo, model_files_present, model_files_verified, model_is_present_and_verified,
};
pub use self::reverse_visit_map::{ReverseMapHeader, ReverseVisitMap};
pub use self::traits::{
    EmbeddingDescriptor, EmbeddingDtype, EmbeddingPooling, EmbeddingProvider, EmbeddingRole,
    LlmCapabilities, LlmChatRequest, LlmChatResponse, LlmChunkStream, LlmFinishReason, LlmMessage,
    LlmProvider, LlmReasoning, LlmResponseFormat, LlmRole, LlmStreamChunk, LlmStreamError,
    LlmToolCall, LlmToolDef, LlmUsage, VectorIndex,
};
pub use self::vector_index::FlatVectorIndex;
pub use self::vector_planes::{
    BinaryPlane, Int8Plane, Int8Vector, PlaneBuildReport, PlaneHeader, binarize,
    binary_bytes_for_dim, build_planes_from_store, dequantize_int8, dot_product, hamming_distance,
    planes_are_stale, quantize_int8,
};
pub use self::vector_store::{VectorStore, VectorStoreHeader};
pub use self::visit_content_map::VisitContentMap;
pub use self::working_set::{
    CandidateSignals, WorkingSetCandidate, WorkingSetConfig, WorkingSetPage, estimate_reembed,
    score_candidate, select_working_set,
};

use self::control::{await_with_ai_cancellation, checkpoint_ai_run};
use self::native_ledger::AiIndexLedgerRow;
use self::provider::{
    classify_provider_error, embed_query, embedding_provider_readiness, validate_provider,
};

/// Resolved provider configuration plus the OPTIONAL secret for one AI operation.
///
/// `api_key` is `Option<SecretString>` because a key is a per-provider OPTION, not a
/// precondition: a local/LAN self-hosted server (LM Studio, Ollama, llama-server, …) needs no
/// key, so PathKeep must never block a provider call on a missing key of its own accord. `None`
/// means "the user stored no key" — the transport then sends NO `Authorization` header rather
/// than a hollow `Bearer ` a key-enforcing cloud server would (rightly) reject. When present, the
/// [`SecretString`] is zeroized on drop, redacted in `Debug`/logs, and never serialized into a
/// trace or sidecar; the plaintext is only exposed at the transport `Authorization`/rig
/// `.api_key(...)` boundary via [`ExposeSecret`]. Only an error the PROVIDER itself returns (a real
/// 401/403, …) may surface as a failure — we never assume "can't be done" before trying.
#[derive(Debug, Clone)]
pub struct AiProviderRuntime {
    pub config: AiProviderConfig,
    pub api_key: Option<SecretString>,
}

impl AiProviderRuntime {
    /// Borrows the plaintext key for the transport boundary, treating a blank key as absent.
    ///
    /// Returns `Some(key)` only for a NON-EMPTY stored secret; `None` (no key OR a whitespace-only
    /// key) tells the transport to omit the `Authorization` header entirely. Centralizing the
    /// "blank == absent" rule here means every transport site agrees, and a user who clears a key
    /// down to whitespace is treated exactly like one who never set it.
    pub fn api_key_for_transport(&self) -> Option<&str> {
        use secrecy::ExposeSecret;
        self.api_key
            .as_ref()
            .map(|secret| secret.expose_secret())
            .filter(|key| !key.trim().is_empty())
    }
}

/// Cooperative cancellation/progress hook for long-running AI work.
pub trait AiRunControl: Send + Sync {
    /// Checks whether the current run should stop at this safe boundary.
    fn checkpoint(&self, detail: &str) -> Result<()>;

    /// Returns whether the current run has already been asked to stop.
    fn cancelled(&self) -> bool {
        false
    }
}

/// Resumable backfill watermark reported by the embed loop after each durable chunk (02 §C.6 R1).
///
/// Carries the lowest canonical `history_id` not yet embedded (`next_history_id`) and a monotone
/// count of vectors persisted so far (`embedded_so_far`). The worker persists these into the
/// index job's payload via an [`IndexBackfillLedger`], so a restart resumes from the watermark
/// instead of re-embedding 14.4M rows from scratch.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct IndexBackfillProgress {
    /// Lowest canonical `history_id` not yet embedded; the next chunk starts here.
    pub next_history_id: i64,
    /// Vectors durably persisted so far across all chunks of this backfill.
    pub embedded_so_far: u64,
    /// Changed existing rows, retained with the cursor for an honest final queue summary.
    pub updated_so_far: u64,
    /// The determinate scan denominator: the max candidate `history_id` captured at the build's TRUE
    /// start, so `next_history_id / scan_target` is honest scan progress that reaches ~100%. Reported
    /// as the captured value on a fresh build and as `0` ("unknown — keep the stored target") on a
    /// resume, so [`crate::ai_queue::persist_index_cursor`] preserves the original denominator rather
    /// than chasing a corpus that grew under the build.
    pub scan_target: i64,
    /// The total candidate pages this build set out to embed, captured at the build's TRUE start
    /// (`COUNT(visits WHERE reverted_at IS NULL)`). Drives the user-facing `embedded_so_far /
    /// embed_target` progress. Same fresh-capture / resume-preserve contract as `scan_target` (0 on a
    /// resume so the stored value is kept).
    pub embed_target: u64,
}

/// Sink the embed loop calls to persist the resumable backfill watermark after each chunk.
///
/// Implemented by the worker (which writes the cursor into the `ai_jobs` payload). vault-core
/// stays free of queue-row mechanics: it only knows it must announce durable progress at each
/// safe boundary so a later restart can resume. A `None` ledger (e.g. the foreground
/// `build_ai_index` convenience path) simply runs to completion without resumption.
pub trait IndexBackfillLedger: Send + Sync {
    /// Records that all vectors up to (but excluding) `progress.next_history_id` are durable.
    ///
    /// Called only AFTER the chunk's vectors are flushed to the vector store, so the watermark
    /// can never claim progress that is not on disk.
    fn record(&self, progress: IndexBackfillProgress) -> Result<()>;

    /// Couples readiness to the persisted stop flag in the final Intelligence transaction.
    fn job_id(&self) -> Option<i64> {
        None
    }
}

/// Error raised when a cooperative AI run stop request is observed.
#[derive(Debug, Error)]
#[error("{reason}")]
pub struct AiRunCancelled {
    reason: String,
}

/// Builds a cooperative-cancellation error with a user-facing reason.
impl AiRunCancelled {
    pub fn new(reason: impl Into<String>) -> Self {
        Self { reason: reason.into() }
    }
}

/// Preview artifact describing how PathKeep can be connected to external AI tooling.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiIntegrationPreview {
    pub mcp_command: String,
    pub consent_summary: String,
    pub manual_steps: Vec<String>,
    pub capability_notes: Vec<String>,
    pub scope_boundary: Vec<String>,
    pub audit_trace: Vec<String>,
    pub generated_files: Vec<crate::models::GeneratedFile>,
    pub warnings: Vec<String>,
}

/// Correlated `NOT EXISTS` so each row is checked by a primary-key lookup; a `NOT IN` subquery
/// would first copy every visible visit id into a temporary index.
const UPSERT_EMBEDDING_SQL: &str = "INSERT OR REPLACE INTO ai_embeddings (history_id, profile_id, url, title, domain, visited_at, content_hash, content_bytes, provider_id, model, indexed_at, metadata_bytes, lineage) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)";
const AI_QUEUE_RECENT_LIMIT: usize = 8;
const AI_INDEX_LEDGER_VERSION: &str = "semantic-sidecar-v1";
/// Canonical rows embedded + persisted per resumable backfill chunk.
///
/// Sized to amortize embed-request overhead while keeping each durable checkpoint small enough
/// that a crash loses at most one chunk of progress and the watermark advances frequently. Kept
/// modest so the 4-core host stays responsive (the embed loop yields between chunks and the
/// vector store append is O(chunk), never O(store)).
///
/// LEASE SAFETY: bounded to ONE `EMBEDDING_HTTP_BATCH` so a single chunk issues at most
/// one HTTP embed call (≤ `EMBEDDING_HTTP_TIMEOUT` = 120s), comfortably under the 300s queue lease
/// even if the 30s heartbeat thread stalls. The earlier 256-row chunk fanned out to 4 HTTP calls
/// (4 × 120s = 480s) which could outlive the lease and let a reclaimed worker keep writing — the
/// lease-loss abort plus this smaller chunk together close that double-write window.
const EMBEDDING_BACKFILL_CHUNK: usize = embedding_external::EMBEDDING_HTTP_BATCH;
const SQLITE_BATCH_SIZE: usize = 400;

#[cfg(test)]
mod tests;

pub use self::search_group_snapshots::{
    SearchGroup, SearchGroupWindow, SearchGroupsCard, search_groups_card,
    warm_search_group_snapshots,
};
pub use self::similar_searches::{SimilarSearch, SimilarSearches, similar_searches};
