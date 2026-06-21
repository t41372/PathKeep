//! Optional AI and semantic retrieval domain.
//!
//! This module coordinates provider readiness, semantic indexing, semantic
//! search, assistant runs, and manual integration previews. It sits on top of
//! the canonical archive and must respect PathKeep's core AI boundaries:
//!
//! - AI is additive and optional; the archive remains usable without it
//! - vector/assistant state is rebuildable derived state, not canonical truth
//! - lexical fallback must stay explicit whenever semantic readiness is missing

mod chat_stream;
mod control;
mod fingerprint;
mod indexing;
mod ledger;
mod llm;
mod narrative;
mod provider;
mod read_model;
mod search;
mod traits;

#[cfg(test)]
use crate::archive::create_schema;
use crate::{
    ai_queue::{self},
    ai_sidecar::{self},
    archive::{list_history, open_archive_connection, open_intelligence_connection},
    config::ProjectPaths,
    enrichment::{build_embedding_content_from_parts, load_best_enrichment_map_by_history_ids},
    models::{
        AiAssistantRequest, AiAssistantResponse, AiCitation, AiIndexReport, AiIndexRequest,
        AiIndexStatus, AiProviderCapabilityReport, AiProviderConfig,
        AiProviderConnectionTestReport, AiProviderPurpose, AiQueueJobType, AiQueueStatus,
        AiRequestFormat, AiSearchEntry, AiSearchRequest, AiSearchResponse, AppConfig, HistoryEntry,
        HistoryQuery,
    },
    utils::{now_rfc3339, sha256_hex, url_domain},
};
use anyhow::{Context, Result};
use iana_time_zone::get_timezone;
#[cfg(not(any(test, coverage)))]
use rig::{
    client::{CompletionClient, EmbeddingsClient},
    completion::Prompt,
    embeddings::EmbeddingModel as _,
    providers::{anthropic, gemini, openai},
};
use rig::{
    completion::ToolDefinition,
    tool::{Tool, ToolDyn},
};
use rusqlite::{Connection, OptionalExtension, Row, params};
use secrecy::SecretString;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::{
    cmp::Ordering,
    collections::HashMap,
    future::Future,
    sync::Arc,
    time::{Duration, Instant},
};
use thiserror::Error;
use tokio::sync::Mutex;

pub use self::indexing::{build_ai_index, build_ai_index_with_control};
pub use self::provider::test_provider_connection;
pub use self::read_model::{
    ai_index_status, ai_queue_status, ensure_ai_schema, load_assistant_run_response,
    preview_ai_integrations, provider_capabilities, provider_connection_failure_report,
    reconcile_ai_queue_controls,
};
pub use self::search::{
    answer_history_question, answer_history_question_with_control, semantic_search_history,
};

pub use self::chat_stream::{
    deregister_run as deregister_ai_chat_run, drive_chat_stream as drive_ai_chat_stream,
    register_run as register_ai_chat_run, request_cancel as request_ai_chat_cancel,
};
pub use self::fingerprint::{EMBEDDING_FINGERPRINT_VERSION, EmbeddingFingerprint};
pub use self::llm::RigLlmProvider;
pub use self::narrative::{
    NarrativeSummary, QueryFamilyFacts, TopicFacts, summarize_query_family, summarize_topic,
};
pub use self::traits::{
    EmbeddingDescriptor, EmbeddingDtype, EmbeddingPooling, EmbeddingProvider, EmbeddingRole,
    LlmCapabilities, LlmChatRequest, LlmChatResponse, LlmChunkStream, LlmMessage, LlmProvider,
    LlmResponseFormat, LlmRole, LlmStreamChunk, LlmToolDef, LlmUsage, VectorIndex,
};

use self::control::{await_with_ai_cancellation, checkpoint_ai_run};
use self::indexing::provider_embedding_count;
#[cfg(test)]
use self::indexing::{build_embedding_content, cleanup_stale_embeddings, collect_visits_to_index};
use self::ledger::{
    AiIndexLedgerRow, ai_embedding_token_estimate, ai_embeddings_storage_bytes, begin_ai_run,
    current_source_watermark, finalize_ai_run_failure, finalize_ai_run_success, load_index_ledger,
    record_index_ledger_failure, record_index_ledger_start, record_index_ledger_success,
};
use self::provider::{
    classify_provider_error, embedding_provider_readiness, run_llm_agent, validate_provider,
};
#[cfg(test)]
use self::provider::{
    embed_batch_with_retry, embed_query, embed_single_with_retry, embedding_error_is_rate_limited,
    provider_connection_report_from_probe,
};
use self::search::semantic_index_staleness_reason;
#[cfg(test)]
use self::search::{
    SearchContext, SearchHistoryArgs, SearchHistoryTool, StoredEmbedding, build_assistant_preamble,
    cosine_similarity, lexical_boost, lexical_score, search_history_internal, semantic_matches,
    sort_stored_embeddings_desc,
};

/// Resolved provider configuration plus the usable secret for one AI operation.
///
/// `api_key` is a [`SecretString`] so it is zeroized on drop, redacted in `Debug`/logs, and
/// never serialized into a trace or sidecar. The plaintext is only exposed at the rig
/// `.api_key(...)` boundary in `provider.rs` via [`ExposeSecret`].
#[derive(Debug, Clone)]
pub struct AiProviderRuntime {
    pub config: AiProviderConfig,
    pub api_key: SecretString,
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

const AI_SCHEMA_SQL: &str = r#"
    CREATE TABLE IF NOT EXISTS ai_embeddings (
      id INTEGER PRIMARY KEY AUTOINCREMENT,
      history_id INTEGER NOT NULL,
      profile_id TEXT NOT NULL,
      url TEXT NOT NULL,
      title TEXT,
      domain TEXT NOT NULL,
      visited_at TEXT NOT NULL,
      content_hash TEXT NOT NULL,
      content_bytes INTEGER NOT NULL,
      provider_id TEXT NOT NULL,
      model TEXT NOT NULL,
      indexed_at TEXT NOT NULL,
      UNIQUE(history_id, provider_id, model, content_hash)
    );
    CREATE TABLE IF NOT EXISTS ai_assistant_runs (
      id INTEGER PRIMARY KEY AUTOINCREMENT,
      run_id INTEGER,
      question TEXT NOT NULL,
      answer TEXT NOT NULL,
      provider_id TEXT NOT NULL,
      embedding_provider_id TEXT NOT NULL,
      citations_json TEXT NOT NULL,
      notes_json TEXT NOT NULL,
      created_at TEXT NOT NULL
    );
    CREATE TABLE IF NOT EXISTS ai_index_ledger (
      provider_id TEXT NOT NULL,
      model TEXT NOT NULL,
      sidecar_table TEXT NOT NULL,
      index_version TEXT NOT NULL,
      state TEXT NOT NULL,
      source_watermark INTEGER,
      last_run_id INTEGER,
      build_started_at TEXT,
      build_finished_at TEXT,
      last_indexed_at TEXT,
      last_cleared_at TEXT,
      last_failure_at TEXT,
      failure_reason TEXT,
      PRIMARY KEY(provider_id, model)
    );
    CREATE INDEX IF NOT EXISTS idx_ai_embeddings_provider_model
      ON ai_embeddings(provider_id, model);
    CREATE INDEX IF NOT EXISTS idx_ai_embeddings_history_id
      ON ai_embeddings(history_id);
"#;

const CLEAR_PROVIDER_EMBEDDINGS_SQL: &str =
    "DELETE FROM ai_embeddings WHERE provider_id = ?1 AND model = ?2";
const DELETE_STALE_EMBEDDINGS_SQL: &str = "DELETE FROM ai_embeddings WHERE provider_id = ?1 AND model = ?2 AND history_id NOT IN (SELECT id FROM archive.visits WHERE reverted_at IS NULL)";
#[cfg(test)]
const UPSERT_EMBEDDING_SQL: &str = "INSERT OR REPLACE INTO ai_embeddings (history_id, profile_id, url, title, domain, visited_at, content_hash, content_bytes, provider_id, model, indexed_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)";
const INSERT_ASSISTANT_RUN_SQL: &str = "INSERT INTO ai_assistant_runs (run_id, question, answer, provider_id, embedding_provider_id, citations_json, notes_json, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)";
const AI_QUEUE_RECENT_LIMIT: usize = 8;
const AI_INDEX_LEDGER_VERSION: &str = "semantic-sidecar-v1";
#[cfg(test)]
const EMBEDDING_RETRY_ATTEMPTS: usize = 2;
const SQLITE_BATCH_SIZE: usize = 400;

#[cfg(test)]
mod tests;
