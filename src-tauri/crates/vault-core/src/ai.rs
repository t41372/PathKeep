//! Optional AI and semantic retrieval domain.
//!
//! This module coordinates provider readiness, semantic indexing, semantic
//! search, assistant runs, and manual integration previews. It sits on top of
//! the canonical archive and must respect PathKeep's core AI boundaries:
//!
//! - AI is additive and optional; the archive remains usable without it
//! - vector/assistant state is rebuildable derived state, not canonical truth
//! - lexical fallback must stay explicit whenever semantic readiness is missing

mod read_model;

#[cfg(test)]
use crate::archive::create_schema;
use crate::{
    ai_queue::{self},
    ai_sidecar::{self, SidecarEmbeddingRow},
    archive::{list_history, open_archive_connection, open_intelligence_connection},
    config::ProjectPaths,
    insights::{build_embedding_content_from_parts, load_best_enrichment_map_by_history_ids},
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
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::{
    cmp::Ordering,
    collections::{HashMap, HashSet},
    future::Future,
    sync::Arc,
    time::{Duration, Instant},
};
use thiserror::Error;
use tokio::sync::Mutex;

pub use self::read_model::{
    ai_index_status, ai_queue_status, ensure_ai_schema, load_assistant_run_response,
    preview_ai_integrations, provider_capabilities, provider_connection_failure_report,
    reconcile_ai_queue_controls,
};

#[derive(Debug, Clone)]
/// Resolved provider configuration plus the usable secret for one AI operation.
pub struct AiProviderRuntime {
    pub config: AiProviderConfig,
    pub api_key: String,
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

#[derive(Debug, Error)]
#[error("{reason}")]
/// Error raised when a cooperative AI run stop request is observed.
pub struct AiRunCancelled {
    reason: String,
}

impl AiRunCancelled {
    /// Builds a cooperative-cancellation error with a user-facing reason.
    pub fn new(reason: impl Into<String>) -> Self {
        Self { reason: reason.into() }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
/// Preview artifact describing how PathKeep can be connected to external AI tooling.
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

#[derive(Debug, Clone)]
struct IndexedVisit {
    history_id: i64,
    profile_id: String,
    url: String,
    title: Option<String>,
    domain: String,
    visited_at: String,
    content: String,
    content_hash: String,
}

#[derive(Debug, Clone)]
struct StoredEmbedding {
    history_id: i64,
    profile_id: String,
    url: String,
    title: Option<String>,
    domain: String,
    visited_at: String,
    score: f32,
}

#[derive(Debug, Clone, Default)]
struct AiIndexLedgerRow {
    state: String,
    source_watermark: i64,
    last_indexed_at: Option<String>,
    last_failure_at: Option<String>,
    failure_reason: Option<String>,
}

#[derive(Debug, Clone, Default)]
struct ProviderReadiness {
    available: bool,
    warning: Option<String>,
    selected_model: Option<String>,
}

#[derive(Debug, Default)]
struct SemanticMatchReport {
    items: Vec<StoredEmbedding>,
    notes: Vec<String>,
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
const UPSERT_EMBEDDING_SQL: &str = "INSERT OR REPLACE INTO ai_embeddings (history_id, profile_id, url, title, domain, visited_at, content_hash, content_bytes, provider_id, model, indexed_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)";
const INSERT_ASSISTANT_RUN_SQL: &str = "INSERT INTO ai_assistant_runs (run_id, question, answer, provider_id, embedding_provider_id, citations_json, notes_json, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)";
const AI_QUEUE_RECENT_LIMIT: usize = 8;
const AI_INDEX_LEDGER_VERSION: &str = "semantic-sidecar-v1";
const EMBEDDING_BATCH_SIZE: usize = 32;
const EMBEDDING_RETRY_ATTEMPTS: usize = 2;
const SQLITE_BATCH_SIZE: usize = 400;

#[derive(Clone)]
struct SearchContext {
    paths: ProjectPaths,
    config: AppConfig,
    database_key: Option<String>,
    embedding_provider: Option<AiProviderRuntime>,
    default_profile_id: Option<String>,
    default_domain: Option<String>,
    default_limit: u32,
    citations: Arc<Mutex<Vec<AiCitation>>>,
    run_control: Option<Arc<dyn AiRunControl>>,
}

#[derive(Debug, Deserialize)]
struct SearchHistoryArgs {
    query: String,
    profile_id: Option<String>,
    domain: Option<String>,
    limit: Option<u32>,
}

#[derive(Debug, Serialize)]
struct SearchHistoryOutput {
    items: Vec<AiSearchEntry>,
}

#[derive(Debug, Error)]
#[error("{0}")]
struct SearchToolError(String);

#[derive(Clone)]
struct SearchHistoryTool {
    context: SearchContext,
}

impl Tool for SearchHistoryTool {
    const NAME: &'static str = "search_history";
    type Error = SearchToolError;
    type Args = SearchHistoryArgs;
    type Output = SearchHistoryOutput;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_string(),
            description: "Search browser history by meaning, URL, title, profile, or domain and return the best matching visits.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "What to search for in the browser history archive."
                    },
                    "profile_id": {
                        "type": "string",
                        "description": "Optional browser profile identifier."
                    },
                    "domain": {
                        "type": "string",
                        "description": "Optional domain filter."
                    },
                    "limit": {
                        "type": "integer",
                        "description": "Maximum number of visits to return."
                    }
                },
                "required": ["query"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> std::result::Result<Self::Output, Self::Error> {
        if let Some(control) = self.context.run_control.as_ref() {
            control
                .checkpoint("Assistant run was cancelled before an additional history search.")
                .map_err(|error| SearchToolError(error.to_string()))?;
        }
        let request = AiSearchRequest {
            query: args.query,
            profile_id: args.profile_id.or_else(|| self.context.default_profile_id.clone()),
            domain: args.domain.or_else(|| self.context.default_domain.clone()),
            limit: args.limit.or(Some(self.context.default_limit)),
            cursor: None,
        };
        let response = search_history_internal(
            &self.context.paths,
            &self.context.config,
            self.context.database_key.as_deref(),
            self.context.embedding_provider.as_ref(),
            &request,
        )
        .await
        .map_err(|error| SearchToolError(error.to_string()))?;
        if let Some(control) = self.context.run_control.as_ref() {
            control
                .checkpoint("Assistant run was cancelled after the latest history search.")
                .map_err(|error| SearchToolError(error.to_string()))?;
        }
        let citations = response
            .items
            .iter()
            .map(|item| AiCitation {
                history_id: item.history_id,
                profile_id: item.profile_id.clone(),
                url: item.url.clone(),
                title: item.title.clone(),
                visited_at: item.visited_at.clone(),
                score: Some(item.score),
            })
            .collect::<Vec<_>>();
        self.context.citations.lock().await.extend(citations);
        Ok(SearchHistoryOutput { items: response.items })
    }
}

fn checkpoint_ai_run(control: Option<&Arc<dyn AiRunControl>>, detail: &str) -> Result<()> {
    if let Some(control) = control {
        control.checkpoint(detail)?;
    }
    Ok(())
}

async fn await_with_ai_cancellation<T, F>(
    control: Option<&Arc<dyn AiRunControl>>,
    detail: &str,
    future: F,
) -> Result<T>
where
    F: Future<Output = Result<T>>,
{
    checkpoint_ai_run(control, detail)?;
    if control.is_none() {
        return future.await;
    }

    tokio::pin!(future);
    loop {
        tokio::select! {
            result = &mut future => return result,
            _ = tokio::time::sleep(Duration::from_millis(250)) => {
                checkpoint_ai_run(control, detail)?;
            }
        }
    }
}

/// Executes a lightweight health probe against one configured AI provider.
pub async fn test_provider_connection(
    provider: &AiProviderRuntime,
) -> Result<AiProviderConnectionTestReport> {
    validate_provider(provider, provider.config.purpose.clone())?;
    let capabilities = provider_capabilities(&provider.config);
    let started = Instant::now();
    let probe_result = match provider.config.purpose {
        AiProviderPurpose::Embedding => {
            embed_query(provider, "PathKeep provider health check").await.map(|vector| {
                format!("Generated a {}-dimension probe embedding successfully.", vector.len())
            })
        }
        AiProviderPurpose::Llm => run_llm_agent(
            provider,
            "You are a PathKeep connection test. Reply with a short OK message.",
            Vec::new(),
            "Reply with OK.",
        )
        .await
        .map(|_| "Provider completed a short chat probe successfully.".to_string()),
    };
    let latency_ms = (started.elapsed().as_millis() as u64).max(1);
    match probe_result {
        Ok(message) => Ok(AiProviderConnectionTestReport {
            provider_id: provider.config.id.clone(),
            purpose: match provider.config.purpose {
                AiProviderPurpose::Embedding => "embedding".to_string(),
                AiProviderPurpose::Llm => "llm".to_string(),
            },
            model: provider.config.default_model.clone(),
            ok: true,
            latency_ms,
            capabilities,
            warnings: if provider.config.request_format == AiRequestFormat::Anthropic
                && provider.config.purpose == AiProviderPurpose::Llm
            {
                vec!["Anthropic remains day-one chat-only in the current rig.rs integration; embedding selection should use a separate provider.".to_string()]
            } else {
                Vec::new()
            },
            message,
            ..AiProviderConnectionTestReport::default()
        }),
        Err(error) => {
            let (error_code, action_hint, retry_hint) = classify_provider_error(&error.to_string());
            Ok(AiProviderConnectionTestReport {
                provider_id: provider.config.id.clone(),
                purpose: match provider.config.purpose {
                    AiProviderPurpose::Embedding => "embedding".to_string(),
                    AiProviderPurpose::Llm => "llm".to_string(),
                },
                model: provider.config.default_model.clone(),
                ok: false,
                latency_ms,
                capabilities,
                error_code,
                action_hint,
                retry_hint,
                warnings: Vec::new(),
                message: error.to_string(),
            })
        }
    }
}

/// Builds or refreshes the semantic sidecar for one embedding provider.
pub async fn build_ai_index(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    provider: &AiProviderRuntime,
    request: &AiIndexRequest,
) -> Result<AiIndexReport> {
    build_ai_index_with_control(paths, config, key, provider, request, None).await
}

/// Builds the semantic index with optional cooperative stop checkpoints.
pub async fn build_ai_index_with_control(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    provider: &AiProviderRuntime,
    request: &AiIndexRequest,
    run_control: Option<Arc<dyn AiRunControl>>,
) -> Result<AiIndexReport> {
    validate_provider(provider, AiProviderPurpose::Embedding)?;
    let archive = open_archive_connection(paths, config, key)?;
    let connection = open_intelligence_connection(paths, config, key)?;
    ensure_ai_schema(&connection)?;
    let started_at = now_rfc3339();
    let source_watermark = current_source_watermark(&connection)?;
    let sidecar_table =
        ai_sidecar::provider_table_name(&provider.config.id, &provider.config.default_model);
    let run_id = begin_ai_run(
        &archive,
        "ai_index",
        "manual",
        json!({
            "providerId": provider.config.id,
            "model": provider.config.default_model,
            "fullRebuild": request.full_rebuild,
            "clearOnly": request.clear_only,
            "limit": request.limit,
        }),
    )?;
    record_index_ledger_start(
        &connection,
        provider,
        run_id,
        &started_at,
        source_watermark,
        &sidecar_table,
        request,
    )?;

    let result: Result<AiIndexReport> = async {
        let run_control = run_control.as_ref();
        checkpoint_ai_run(run_control, "Index build was cancelled before collecting stale rows.")?;
        let stale_history_ids = collect_stale_history_ids(&connection, provider)?;

        if request.full_rebuild || request.clear_only {
            clear_provider_embeddings(&connection, provider)?;
        }

        let removed_items = cleanup_stale_embeddings(&connection, provider)?;
        let sidecar_removed = if request.full_rebuild || request.clear_only {
            ai_sidecar::clear_provider_embeddings(
                paths,
                &provider.config.id,
                &provider.config.default_model,
            )
            .await?
        } else {
            0
        };

        if request.clear_only {
            return Ok(AiIndexReport {
                job_id: None,
                run_id: Some(run_id),
                provider_id: provider.config.id.clone(),
                model: provider.config.default_model.clone(),
                indexed_items: 0,
                updated_items: 0,
                skipped_items: 0,
                removed_items: removed_items + sidecar_removed,
                last_indexed_at: now_rfc3339(),
                notes: vec![
                    "Cleared the semantic index compatibility rows and the LanceDB sidecar."
                        .to_string(),
                ],
            });
        }

        checkpoint_ai_run(run_control, "Index build was cancelled before collecting candidates.")?;
        let candidates = collect_visits_to_index(paths, &connection, provider, request.limit)?;
        if candidates.is_empty() {
            await_with_ai_cancellation(
                run_control,
                "Index build was cancelled before the empty sidecar sync finished.",
                ai_sidecar::sync_provider_embeddings(
                    paths,
                    &provider.config.id,
                    &provider.config.default_model,
                    &[],
                    request.full_rebuild,
                    false,
                    &stale_history_ids,
                ),
            )
            .await?;
            return Ok(AiIndexReport {
                job_id: None,
                run_id: Some(run_id),
                provider_id: provider.config.id.clone(),
                model: provider.config.default_model.clone(),
                indexed_items: 0,
                updated_items: 0,
                skipped_items: 0,
                removed_items: removed_items + sidecar_removed,
                last_indexed_at: now_rfc3339(),
                notes: vec!["No new or changed history rows required indexing.".to_string()],
            });
        }

        let timestamp = now_rfc3339();
        let mut indexed_items = 0usize;
        let mut updated_items = 0usize;
        let mut skipped_items = 0usize;
        let mut sidecar_rows = Vec::with_capacity(candidates.len());
        let mut partial_failure_notes = Vec::new();
        let existing_history_ids = load_existing_embedding_hashes(
            &connection,
            provider,
            &candidates.iter().map(|visit| visit.history_id).collect::<Vec<_>>(),
        )?
        .into_keys()
        .collect::<HashSet<_>>();

        for batch in candidates.chunks(EMBEDDING_BATCH_SIZE) {
            checkpoint_ai_run(
                run_control,
                "Index build was cancelled before the next embedding batch started.",
            )?;
            let texts = batch.iter().map(|visit| visit.content.clone()).collect::<Vec<_>>();
            match await_with_ai_cancellation(
                run_control,
                "Index build was cancelled while waiting for embedding batch results.",
                embed_batch_with_retry(provider, &texts),
            )
            .await
            {
                Ok(vectors) if vectors.len() == batch.len() => {
                    for (visit, vector) in batch.iter().zip(vectors.into_iter()) {
                        let had_prior_index = existing_history_ids.contains(&visit.history_id);
                        upsert_embedding(&connection, provider, visit, &timestamp)?;
                        sidecar_rows.push(SidecarEmbeddingRow {
                            history_id: visit.history_id,
                            profile_id: visit.profile_id.clone(),
                            url: visit.url.clone(),
                            title: visit.title.clone(),
                            domain: visit.domain.clone(),
                            visited_at: visit.visited_at.clone(),
                            provider_id: provider.config.id.clone(),
                            model: provider.config.default_model.clone(),
                            content_hash: visit.content_hash.clone(),
                            indexed_at: timestamp.clone(),
                            vector,
                        });
                        if had_prior_index {
                            updated_items += 1;
                        } else {
                            indexed_items += 1;
                        }
                    }
                }
                Ok(_) | Err(_) => {
                    for visit in batch {
                        checkpoint_ai_run(
                            run_control,
                            "Index build was cancelled before an individual retry embedding call.",
                        )?;
                        let had_prior_index = existing_history_ids.contains(&visit.history_id);
                        match await_with_ai_cancellation(
                            run_control,
                            "Index build was cancelled while retrying an individual embedding call.",
                            embed_single_with_retry(provider, &visit.content),
                        )
                        .await
                        {
                            Ok(vector) => {
                                upsert_embedding(&connection, provider, visit, &timestamp)?;
                                sidecar_rows.push(SidecarEmbeddingRow {
                                    history_id: visit.history_id,
                                    profile_id: visit.profile_id.clone(),
                                    url: visit.url.clone(),
                                    title: visit.title.clone(),
                                    domain: visit.domain.clone(),
                                    visited_at: visit.visited_at.clone(),
                                    provider_id: provider.config.id.clone(),
                                    model: provider.config.default_model.clone(),
                                    content_hash: visit.content_hash.clone(),
                                    indexed_at: timestamp.clone(),
                                    vector,
                                });
                                if had_prior_index {
                                    updated_items += 1;
                                } else {
                                    indexed_items += 1;
                                }
                            }
                            Err(error) => {
                                skipped_items += 1;
                                partial_failure_notes.push(format!(
                                    "Skipped history row {} after batch and per-row embedding retries: {}",
                                    visit.history_id, error
                                ));
                            }
                        }
                    }
                }
            }
        }
        let sidecar_synced = await_with_ai_cancellation(
            run_control,
            "Index build was cancelled while syncing the semantic sidecar.",
            ai_sidecar::sync_provider_embeddings(
                paths,
                &provider.config.id,
                &provider.config.default_model,
                &sidecar_rows,
                request.full_rebuild,
                false,
                &stale_history_ids,
            ),
        )
        .await?;

        Ok(AiIndexReport {
            job_id: None,
            run_id: Some(run_id),
            provider_id: provider.config.id.clone(),
            model: provider.config.default_model.clone(),
            indexed_items,
            updated_items,
            skipped_items,
            removed_items: removed_items + sidecar_removed,
            last_indexed_at: timestamp,
            notes: {
                let mut notes = vec![
                format!("Indexed {} history rows with {}.", candidates.len(), provider.config.name),
                format!(
                    "Processed {} embedding batch(es) with a batch size of {}.",
                    candidates.len().div_ceil(EMBEDDING_BATCH_SIZE),
                    EMBEDDING_BATCH_SIZE
                ),
                format!(
                    "Synced {} row(s) into the LanceDB semantic sidecar. PathKeep keeps the SQLite mirror only for metadata/debug compatibility, not for full-table semantic fallback scans.",
                    sidecar_synced
                ),
                ];
                if skipped_items > 0 {
                    notes.push(format!(
                        "Skipped {} row(s) after retrying failed embedding batches individually.",
                        skipped_items
                    ));
                    notes.extend(partial_failure_notes);
                }
                notes
            },
        })
    }
    .await;

    match result {
        Ok(report) => {
            finalize_ai_run_success(
                &archive,
                run_id,
                json!({
                    "providerId": report.provider_id,
                    "model": report.model,
                    "indexedItems": report.indexed_items,
                    "updatedItems": report.updated_items,
                    "removedItems": report.removed_items,
                }),
            )?;
            record_index_ledger_success(
                &connection,
                provider,
                run_id,
                &report.last_indexed_at,
                source_watermark,
                &sidecar_table,
                request,
            )?;
            Ok(report)
        }
        Err(error) => {
            finalize_ai_run_failure(
                &archive,
                run_id,
                &error.to_string(),
                json!({
                    "providerId": provider.config.id,
                    "model": provider.config.default_model,
                    "fullRebuild": request.full_rebuild,
                    "clearOnly": request.clear_only,
                }),
            )?;
            record_index_ledger_failure(
                &connection,
                provider,
                run_id,
                source_watermark,
                &sidecar_table,
                request,
                &error.to_string(),
            )?;
            Err(error)
        }
    }
}

/// Runs the semantic/keyword history search pipeline with explicit fallback behavior.
pub async fn semantic_search_history(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    provider: Option<&AiProviderRuntime>,
    request: &AiSearchRequest,
) -> Result<AiSearchResponse> {
    search_history_internal(paths, config, key, provider, request).await
}

/// Answers one user question against archive history with evidence-backed citations.
pub async fn answer_history_question(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    llm_provider: &AiProviderRuntime,
    embedding_provider: Option<&AiProviderRuntime>,
    request: &AiAssistantRequest,
) -> Result<AiAssistantResponse> {
    answer_history_question_with_control(
        paths,
        config,
        key,
        llm_provider,
        embedding_provider,
        request,
        None,
    )
    .await
}

/// Answers one assistant question with optional cooperative stop checkpoints.
pub async fn answer_history_question_with_control(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    llm_provider: &AiProviderRuntime,
    embedding_provider: Option<&AiProviderRuntime>,
    request: &AiAssistantRequest,
    run_control: Option<Arc<dyn AiRunControl>>,
) -> Result<AiAssistantResponse> {
    validate_provider(llm_provider, AiProviderPurpose::Llm)?;
    if !config.ai.enabled || !config.ai.assistant_enabled {
        anyhow::bail!("Enable AI analysis and the assistant in Settings before asking questions.")
    }
    let archive = open_archive_connection(paths, config, key)?;
    let connection = open_intelligence_connection(paths, config, key)?;
    ensure_ai_schema(&connection)?;
    let run_id = begin_ai_run(
        &archive,
        "assistant",
        "manual",
        json!({
            "providerId": llm_provider.config.id,
            "embeddingProviderId": embedding_provider
                .map(|provider| provider.config.id.clone())
                .unwrap_or_else(|| "lexical-fallback".to_string()),
            "questionLength": request.question.len(),
        }),
    )?;

    let result: Result<AiAssistantResponse> = async {
        let retrieval_request = AiSearchRequest {
            query: request.question.clone(),
            profile_id: request.profile_id.clone(),
            domain: request.domain.clone(),
            limit: Some(config.ai.retrieval_top_k.max(1)),
            cursor: None,
        };
        let run_control = run_control.as_ref();
        let search_response = await_with_ai_cancellation(
            run_control,
            "Assistant run was cancelled before retrieval finished.",
            search_history_internal(paths, config, key, embedding_provider, &retrieval_request),
        )
        .await?;
        let seeded_citations = search_response
            .items
            .iter()
            .map(|item| AiCitation {
                history_id: item.history_id,
                profile_id: item.profile_id.clone(),
                url: item.url.clone(),
                title: item.title.clone(),
                visited_at: item.visited_at.clone(),
                score: Some(item.score),
            })
            .collect::<Vec<_>>();
        let citations = Arc::new(Mutex::new(seeded_citations.clone()));
        let tool_context = SearchContext {
            paths: paths.clone(),
            config: config.clone(),
            database_key: key.map(ToOwned::to_owned),
            embedding_provider: embedding_provider.cloned(),
            default_profile_id: request.profile_id.clone(),
                default_domain: request.domain.clone(),
                default_limit: config.ai.retrieval_top_k.max(1),
                citations: Arc::clone(&citations),
                run_control: run_control.cloned(),
            };
        let tools: Vec<Box<dyn ToolDyn>> =
            vec![Box::new(SearchHistoryTool { context: tool_context })];
        let preamble = build_assistant_preamble(config, &search_response);
        let answer = await_with_ai_cancellation(
            run_control,
            "Assistant run was cancelled while waiting for the model response.",
            run_llm_agent(llm_provider, &preamble, tools, &request.question),
        )
        .await?;

        let mut final_citations = citations.lock().await.clone();
        final_citations.sort_by_key(|item| item.history_id);
        final_citations.dedup_by_key(|item| item.history_id);

        let embedding_provider_id = embedding_provider
            .map(|provider| provider.config.id.clone())
            .unwrap_or_else(|| "lexical-fallback".to_string());
        let final_answer = if final_citations.is_empty() {
            "I couldn't find enough matching history evidence to answer that confidently yet. Try narrowing the profile or domain, or rebuild the semantic index and ask again.".to_string()
        } else {
            answer
        };
        #[rustfmt::skip]
        record_assistant_run(&connection, run_id, request, &final_answer, &llm_provider.config.id, &embedding_provider_id, &final_citations, &search_response.notes)?;

        Ok(AiAssistantResponse {
            state: if final_citations.is_empty() {
                "insufficient-evidence".to_string()
            } else {
                "completed".to_string()
            },
            answer: final_answer,
            job_id: None,
            run_id: Some(run_id),
            provider_id: llm_provider.config.id.clone(),
            embedding_provider_id,
            citations: final_citations,
            notes: search_response.notes,
        })
    }
    .await;

    match result {
        Ok(response) => {
            finalize_ai_run_success(
                &archive,
                run_id,
                json!({
                    "providerId": response.provider_id,
                    "embeddingProviderId": response.embedding_provider_id,
                    "citations": response.citations.len(),
                }),
            )?;
            Ok(response)
        }
        Err(error) => {
            finalize_ai_run_failure(
                &archive,
                run_id,
                &error.to_string(),
                json!({
                    "providerId": llm_provider.config.id,
                    "embeddingProviderId": embedding_provider
                        .map(|provider| provider.config.id.clone())
                        .unwrap_or_else(|| "lexical-fallback".to_string()),
                }),
            )?;
            Err(error)
        }
    }
}

fn validate_provider(
    provider: &AiProviderRuntime,
    expected_purpose: AiProviderPurpose,
) -> Result<()> {
    if !provider.config.enabled {
        anyhow::bail!("Enable provider {} before using it.", provider.config.name)
    }
    if provider.config.purpose != expected_purpose {
        anyhow::bail!(
            "Provider {} is configured for {:?}, not {:?}.",
            provider.config.name,
            provider.config.purpose,
            expected_purpose
        )
    }
    if provider.config.default_model.trim().is_empty() {
        anyhow::bail!("Select a default model for provider {}.", provider.config.name)
    }
    if matches!(
        (provider.config.purpose.clone(), provider.config.request_format.clone()),
        (AiProviderPurpose::Embedding, AiRequestFormat::Anthropic)
    ) {
        anyhow::bail!("Anthropic request format is not available for embeddings in rig.rs.")
    }
    Ok(())
}

fn classify_provider_error(message: &str) -> (Option<String>, Option<String>, Option<String>) {
    let normalized = message.to_lowercase();
    if normalized.contains("enable provider") {
        return (
            Some("provider-disabled".to_string()),
            Some("Enable the provider in Settings before testing it again.".to_string()),
            None,
        );
    }
    if normalized.contains("api key")
        || normalized.contains("store an api key")
        || normalized.contains("unauthorized")
        || normalized.contains("forbidden")
    {
        return (
            Some("secret-missing".to_string()),
            Some("Store a valid API key in the native keyring for this provider.".to_string()),
            Some("After updating the key, run Test connection again.".to_string()),
        );
    }
    if normalized.contains("rate limit")
        || normalized.contains("quota")
        || normalized.contains("429")
    {
        return (
            Some("rate-limited".to_string()),
            Some(
                "Wait for the provider quota window to reset or reduce this model's usage."
                    .to_string(),
            ),
            Some("Retry after the provider cooldown ends.".to_string()),
        );
    }
    if normalized.contains("does not support embeddings")
        || normalized.contains("not configured for")
    {
        return (
            Some("unsupported-capability".to_string()),
            Some("Pick a provider whose day-one capabilities match this purpose.".to_string()),
            None,
        );
    }
    if normalized.contains("model") && normalized.contains("not found") {
        return (
            Some("bad-model".to_string()),
            Some("Select a valid default model for this provider.".to_string()),
            Some("Save the model selection and test again.".to_string()),
        );
    }
    if normalized.contains("timed out")
        || normalized.contains("dns")
        || normalized.contains("refused")
        || normalized.contains("network")
    {
        return (
            Some("network-error".to_string()),
            Some(
                "Check the base URL, local daemon, or network path for this provider.".to_string(),
            ),
            Some("Retry after the endpoint is reachable.".to_string()),
        );
    }
    (
        Some("provider-error".to_string()),
        None,
        Some("Review the provider error and retry after fixing it.".to_string()),
    )
}

fn embedding_provider_readiness(config: &AppConfig) -> ProviderReadiness {
    let Some(provider_id) = config.ai.embedding_provider_id.as_deref() else {
        return ProviderReadiness {
            available: false,
            warning: Some(
                "Select an embedding provider in Settings before enabling semantic retrieval."
                    .to_string(),
            ),
            selected_model: None,
        };
    };
    let Some(provider) =
        config.ai.embedding_providers.iter().find(|provider| provider.id == provider_id)
    else {
        return ProviderReadiness {
            available: false,
            warning: Some(format!(
                "Embedding provider {provider_id} is no longer available in Settings."
            )),
            selected_model: None,
        };
    };
    if !provider.enabled {
        return ProviderReadiness {
            available: false,
            warning: Some(format!(
                "Enable provider {} before using semantic retrieval.",
                provider.name
            )),
            selected_model: Some(provider.default_model.clone()),
        };
    }
    if !provider.api_key_saved {
        return ProviderReadiness {
            available: false,
            warning: Some(format!(
                "Store an API key for provider {} before using semantic retrieval.",
                provider.name
            )),
            selected_model: Some(provider.default_model.clone()),
        };
    }
    if provider.default_model.trim().is_empty() {
        return ProviderReadiness {
            available: false,
            warning: Some(format!(
                "Choose a default model for provider {} before using semantic retrieval.",
                provider.name
            )),
            selected_model: None,
        };
    }
    ProviderReadiness {
        available: true,
        warning: None,
        selected_model: Some(provider.default_model.clone()),
    }
}

fn load_index_ledger(
    connection: &Connection,
    provider_id: &str,
    model: &str,
) -> Result<AiIndexLedgerRow> {
    connection
        .query_row(
            "SELECT
                state,
                COALESCE(source_watermark, 0),
                last_indexed_at,
                last_failure_at,
                failure_reason
             FROM ai_index_ledger
             WHERE provider_id = ?1 AND model = ?2",
            params![provider_id, model],
            |row| {
                Ok(AiIndexLedgerRow {
                    state: row.get(0)?,
                    source_watermark: row.get(1)?,
                    last_indexed_at: row.get(2)?,
                    last_failure_at: row.get(3)?,
                    failure_reason: row.get(4)?,
                })
            },
        )
        .optional()
        .map(|row| row.unwrap_or_default())
        .context("loading AI index ledger")
}

fn current_source_watermark(connection: &Connection) -> Result<i64> {
    connection
        .query_row(
            "SELECT COUNT(*), COALESCE(MAX(id), 0)
             FROM archive.visits
             WHERE reverted_at IS NULL",
            [],
            |row| {
                let visible_rows = row.get::<_, i64>(0)?.max(0);
                let max_history_id = row.get::<_, i64>(1)?.max(0);
                Ok((visible_rows << 32) ^ max_history_id)
            },
        )
        .context("loading visibility-aware AI index watermark")
}

fn current_timezone_name() -> String {
    get_timezone().unwrap_or_else(|_| "UTC".to_string())
}

fn begin_ai_run(
    connection: &Connection,
    run_type: &str,
    trigger: &str,
    stats_json: serde_json::Value,
) -> Result<i64> {
    let started_at = now_rfc3339();
    connection.execute(
        "INSERT INTO runs (
           run_type,
           trigger,
           started_at,
           timezone,
           status,
           profile_scope_json,
           warnings_json,
           stats_json,
           due_only
         )
         VALUES (?1, ?2, ?3, ?4, 'running', '[]', '[]', ?5, 0)",
        params![
            run_type,
            trigger,
            started_at,
            current_timezone_name(),
            serde_json::to_string(&stats_json)?,
        ],
    )?;
    Ok(connection.last_insert_rowid())
}

fn finalize_ai_run_success(
    connection: &Connection,
    run_id: i64,
    stats_json: serde_json::Value,
) -> Result<()> {
    let finished_at = now_rfc3339();
    connection.execute(
        "UPDATE runs
         SET finished_at = ?1,
             status = 'success',
             error_message = NULL,
             warnings_json = '[]',
             stats_json = ?2
         WHERE id = ?3",
        params![finished_at, serde_json::to_string(&stats_json)?, run_id],
    )?;
    Ok(())
}

fn finalize_ai_run_failure(
    connection: &Connection,
    run_id: i64,
    error_message: &str,
    stats_json: serde_json::Value,
) -> Result<()> {
    let finished_at = now_rfc3339();
    connection.execute(
        "UPDATE runs
         SET finished_at = ?1,
             status = 'failed',
             error_message = ?2,
             stats_json = ?3
         WHERE id = ?4",
        params![finished_at, error_message, serde_json::to_string(&stats_json)?, run_id,],
    )?;
    Ok(())
}

fn record_index_ledger_start(
    connection: &Connection,
    provider: &AiProviderRuntime,
    run_id: i64,
    started_at: &str,
    source_watermark: i64,
    sidecar_table: &str,
    request: &AiIndexRequest,
) -> Result<()> {
    let state = if request.clear_only { "clearing" } else { "building" };
    connection.execute(
        "INSERT INTO ai_index_ledger (
           provider_id,
           model,
           sidecar_table,
           index_version,
           state,
           source_watermark,
           last_run_id,
           build_started_at,
           build_finished_at,
           last_indexed_at,
           last_cleared_at,
           last_failure_at,
           failure_reason
         )
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, NULL, NULL, NULL, NULL, NULL)
         ON CONFLICT(provider_id, model) DO UPDATE SET
           sidecar_table = excluded.sidecar_table,
           index_version = excluded.index_version,
           state = excluded.state,
           source_watermark = excluded.source_watermark,
           last_run_id = excluded.last_run_id,
           build_started_at = excluded.build_started_at,
           build_finished_at = NULL,
           last_failure_at = NULL,
           failure_reason = NULL",
        params![
            provider.config.id,
            provider.config.default_model,
            sidecar_table,
            AI_INDEX_LEDGER_VERSION,
            state,
            source_watermark,
            run_id,
            started_at,
        ],
    )?;
    Ok(())
}

fn record_index_ledger_success(
    connection: &Connection,
    provider: &AiProviderRuntime,
    run_id: i64,
    finished_at: &str,
    source_watermark: i64,
    sidecar_table: &str,
    request: &AiIndexRequest,
) -> Result<()> {
    connection.execute(
        "UPDATE ai_index_ledger
         SET state = ?1,
             sidecar_table = ?2,
             index_version = ?3,
             source_watermark = ?4,
             last_run_id = ?5,
             build_finished_at = ?6,
             last_indexed_at = ?7,
             last_cleared_at = CASE WHEN ?8 = 1 THEN ?7 ELSE last_cleared_at END,
             last_failure_at = NULL,
             failure_reason = NULL
         WHERE provider_id = ?9 AND model = ?10",
        params![
            if request.clear_only { "cleared" } else { "ready" },
            sidecar_table,
            AI_INDEX_LEDGER_VERSION,
            source_watermark,
            run_id,
            finished_at,
            finished_at,
            request.clear_only as i64,
            provider.config.id,
            provider.config.default_model,
        ],
    )?;
    Ok(())
}

fn record_index_ledger_failure(
    connection: &Connection,
    provider: &AiProviderRuntime,
    run_id: i64,
    source_watermark: i64,
    sidecar_table: &str,
    _request: &AiIndexRequest,
    error_message: &str,
) -> Result<()> {
    let failed_at = now_rfc3339();
    connection.execute(
        "UPDATE ai_index_ledger
         SET state = 'failed',
             sidecar_table = ?1,
             index_version = ?2,
             source_watermark = ?3,
             last_run_id = ?4,
             build_finished_at = ?5,
             last_failure_at = ?5,
             failure_reason = ?6
         WHERE provider_id = ?7 AND model = ?8",
        params![
            sidecar_table,
            AI_INDEX_LEDGER_VERSION,
            source_watermark,
            run_id,
            failed_at,
            error_message,
            provider.config.id,
            provider.config.default_model,
        ],
    )?;
    Ok(())
}

fn build_assistant_preamble(config: &AppConfig, search_response: &AiSearchResponse) -> String {
    let context = search_response
        .items
        .iter()
        .enumerate()
        .map(|(index, item)| {
            format!(
                "[{index}] {visited_at} | {profile_id} | {url}\nTitle: {title}\nMatch: {reason}\nScore: {score:.3}",
                index = index + 1,
                visited_at = item.visited_at,
                profile_id = item.profile_id,
                url = item.url,
                title = item.title.clone().unwrap_or_else(|| "(untitled)".to_string()),
                reason = item.match_reason,
                score = item.score
            )
        })
        .collect::<Vec<_>>()
        .join("\n\n");

    format!(
        "{system_prompt}\n\nYou are working inside PathKeep. Always ground answers in the history evidence below or by calling the search_history tool. Cite the visit date, profile, and URL you relied on. If the evidence is incomplete, say so.\n\nInitial evidence:\n{context}",
        system_prompt = config.ai.assistant_system_prompt,
        context = if context.is_empty() {
            "No indexed evidence was found. Use the search_history tool or explain that the archive has no matching records.".to_string()
        } else {
            context
        }
    )
}

#[cfg(not(any(test, coverage)))]
async fn run_llm_agent(
    provider: &AiProviderRuntime,
    preamble: &str,
    tools: Vec<Box<dyn ToolDyn>>,
    question: &str,
) -> Result<String> {
    match provider.config.request_format {
        AiRequestFormat::OpenAi | AiRequestFormat::Ollama | AiRequestFormat::LmStudio => {
            let mut builder =
                openai::CompletionsClient::builder().api_key(provider.api_key.clone());
            if let Some(base_url) = provider.config.base_url.as_deref() {
                builder = builder.base_url(base_url);
            }
            let client = builder.build()?;
            let agent = client
                .agent(provider.config.default_model.clone())
                .preamble(preamble)
                .temperature(provider.config.temperature.unwrap_or(0.2) as f64)
                .max_tokens(provider.config.max_tokens.unwrap_or(1200).into())
                .tools(tools)
                .build();
            Ok(agent.prompt(question).await?)
        }
        AiRequestFormat::Anthropic => {
            let mut builder = anthropic::Client::builder().api_key(provider.api_key.clone());
            if let Some(base_url) = provider.config.base_url.as_deref() {
                builder = builder.base_url(base_url);
            }
            let client = builder.build()?;
            let agent = client
                .agent(provider.config.default_model.clone())
                .preamble(preamble)
                .temperature(provider.config.temperature.unwrap_or(0.2) as f64)
                .max_tokens(provider.config.max_tokens.unwrap_or(1200).into())
                .tools(tools)
                .build();
            Ok(agent.prompt(question).await?)
        }
        AiRequestFormat::Google => {
            let mut builder = gemini::Client::builder().api_key(provider.api_key.clone());
            if let Some(base_url) = provider.config.base_url.as_deref() {
                builder = builder.base_url(base_url);
            }
            let client = builder.build()?;
            let agent = client
                .agent(provider.config.default_model.clone())
                .preamble(preamble)
                .temperature(provider.config.temperature.unwrap_or(0.2) as f64)
                .max_tokens(provider.config.max_tokens.unwrap_or(1200).into())
                .tools(tools)
                .build();
            Ok(agent.prompt(question).await?)
        }
    }
}

#[cfg(any(test, coverage))]
async fn run_llm_agent(
    provider: &AiProviderRuntime,
    preamble: &str,
    tools: Vec<Box<dyn ToolDyn>>,
    question: &str,
) -> Result<String> {
    let provider_label = match provider.config.request_format {
        AiRequestFormat::OpenAi => "openai",
        AiRequestFormat::Ollama => "ollama",
        AiRequestFormat::LmStudio => "lmstudio",
        AiRequestFormat::Anthropic => "anthropic",
        AiRequestFormat::Google => "google",
    };
    let preamble_summary =
        preamble.lines().next().unwrap_or_default().trim().chars().take(24).collect::<String>();
    Ok(format!(
        "{provider_label} stub answer to '{question}' with {} tools [{preamble_summary}]",
        tools.len()
    ))
}

async fn search_history_internal(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    provider: Option<&AiProviderRuntime>,
    request: &AiSearchRequest,
) -> Result<AiSearchResponse> {
    let query = request.query.trim();
    if query.is_empty() {
        anyhow::bail!("Enter a question or search query first.")
    }

    fn parse_search_cursor(cursor: Option<&str>) -> usize {
        cursor.and_then(|value| value.parse::<usize>().ok()).unwrap_or(0)
    }

    let lexical = lexical_history_results(paths, config, key, request, query)?;
    let mut merged = HashMap::<i64, AiSearchEntry>::new();
    let limit = request.limit.unwrap_or(8).clamp(1, 50) as usize;

    for (index, item) in lexical.items.iter().take(limit).enumerate() {
        merged.insert(
            item.id,
            history_entry_to_search_entry(item, lexical_score(index, limit), "Lexical match"),
        );
    }

    let mut notes = Vec::new();
    let mut provider_id = "lexical-fallback".to_string();
    let mut model = "none".to_string();

    if let Some(provider) = provider {
        validate_provider(provider, AiProviderPurpose::Embedding)?;
        provider_id = provider.config.id.clone();
        model = provider.config.default_model.clone();
        let semantic = semantic_matches(paths, config, key, provider, request).await?;
        notes.extend(semantic.notes.clone());
        if semantic.items.is_empty() {
            notes.push(
                "No indexed semantic matches were found; showing lexical results only.".to_string(),
            );
        }
        for (index, item) in semantic.items.into_iter().take(limit).enumerate() {
            let entry = merged.entry(item.history_id).or_insert_with(|| AiSearchEntry {
                history_id: item.history_id,
                profile_id: item.profile_id.clone(),
                url: item.url.clone(),
                title: item.title.clone(),
                domain: item.domain.clone(),
                visited_at: item.visited_at.clone(),
                score: item.score,
                match_reason: "Semantic match".to_string(),
            });
            entry.score = entry.score.max(item.score + lexical_boost(index, limit));
            entry.match_reason = if entry.match_reason.contains("Lexical") {
                "Semantic + lexical match".to_string()
            } else {
                "Semantic match".to_string()
            };
        }
    } else {
        notes.push(
            "No embedding provider is selected, so results use lexical retrieval only.".to_string(),
        );
    }

    let mut items = merged.into_values().collect::<Vec<_>>();
    items.sort_by(|left, right| {
        right
            .score
            .partial_cmp(&left.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(left.visited_at.cmp(&right.visited_at))
    });
    let total = items.len();
    let offset = parse_search_cursor(request.cursor.as_deref()).min(total);
    let next_offset = (offset + limit).min(total);
    let next_cursor = (next_offset < total).then(|| next_offset.to_string());
    let items = items.into_iter().skip(offset).take(limit).collect();

    Ok(AiSearchResponse { total, provider_id, model, items, notes, next_cursor })
}

async fn semantic_matches(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    provider: &AiProviderRuntime,
    request: &AiSearchRequest,
) -> Result<SemanticMatchReport> {
    let connection = open_intelligence_connection(paths, config, key)?;
    ensure_ai_schema(&connection)?;
    let mut notes = Vec::new();
    let ledger =
        load_index_ledger(&connection, &provider.config.id, &provider.config.default_model)?;
    if let Some(reason) = semantic_index_staleness_reason(
        &connection,
        &provider.config.id,
        &provider.config.default_model,
        ledger.source_watermark,
        ledger.last_indexed_at.as_deref(),
    )? {
        notes.push(reason);
    }

    let query_vector = embed_query(provider, request.query.trim()).await?;
    let limit = request.limit.unwrap_or(8).clamp(1, 50) as usize;
    match ai_sidecar::search_provider_embeddings(
        paths,
        &provider.config.id,
        &provider.config.default_model,
        &query_vector,
        request.profile_id.as_deref(),
        request.domain.as_deref(),
        limit,
    )
    .await
    {
        Ok(Some(rows)) if !rows.is_empty() => {
            let mut visible_rows = Vec::new();
            for row in rows {
                if !history_row_is_visible(&connection, row.history_id)? {
                    continue;
                }
                visible_rows.push(StoredEmbedding {
                    history_id: row.history_id,
                    profile_id: row.profile_id,
                    url: row.url,
                    title: row.title,
                    domain: row.domain,
                    visited_at: row.visited_at,
                    score: row.score,
                });
            }
            visible_rows.sort_by(sort_stored_embeddings_desc);
            return Ok(SemanticMatchReport { items: visible_rows, notes });
        }
        Ok(Some(_)) | Ok(None) => {
            if provider_embedding_count(
                &connection,
                &provider.config.id,
                &provider.config.default_model,
            )? > 0
            {
                notes.push(
                    "The LanceDB semantic sidecar is missing or empty, so PathKeep returned lexical matches only instead of relying on stale SQLite semantic metadata."
                        .to_string(),
                );
            }
        }
        Err(error) => {
            notes.push(format!(
                "The LanceDB semantic sidecar could not answer this search ({}). PathKeep returned lexical matches only instead of relying on stale SQLite semantic metadata.",
                error
            ));
        }
    }
    Ok(SemanticMatchReport { items: Vec::new(), notes })
}

fn load_existing_embedding_hashes(
    connection: &Connection,
    provider: &AiProviderRuntime,
    history_ids: &[i64],
) -> Result<HashMap<i64, String>> {
    let mut hashes = HashMap::new();
    for chunk in history_ids.chunks(SQLITE_BATCH_SIZE) {
        let placeholders = vec!["?"; chunk.len()].join(", ");
        let sql = format!(
            "SELECT history_id, content_hash
             FROM ai_embeddings
             WHERE provider_id = ?1
               AND model = ?2
               AND history_id IN ({placeholders})"
        );
        let mut statement = connection.prepare(&sql)?;
        let params = std::iter::once(&provider.config.id as &dyn rusqlite::ToSql)
            .chain(std::iter::once(&provider.config.default_model as &dyn rusqlite::ToSql))
            .chain(chunk.iter().map(|history_id| history_id as &dyn rusqlite::ToSql));
        let rows = statement.query_map(rusqlite::params_from_iter(params), |row: &Row<'_>| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
        })?;
        for row in rows {
            let (history_id, content_hash) = row?;
            hashes.insert(history_id, content_hash);
        }
    }
    Ok(hashes)
}

fn collect_visits_to_index(
    paths: &ProjectPaths,
    connection: &Connection,
    provider: &AiProviderRuntime,
    limit: Option<u32>,
) -> Result<Vec<IndexedVisit>> {
    let limit_sql = limit.unwrap_or(0).max(1);
    let sql = if limit.is_some() {
        "SELECT visits.id,
                source_profiles.profile_key,
                urls.url,
                urls.title,
                (visits.visit_time_ms * 1000 + 11644473600000000) AS visit_time
         FROM archive.visits AS visits
         JOIN archive.urls AS urls ON urls.id = visits.url_id
         JOIN archive.source_profiles AS source_profiles ON source_profiles.id = visits.source_profile_id
         WHERE visits.reverted_at IS NULL
         ORDER BY visits.visit_time_ms DESC
         LIMIT ?1"
    } else {
        "SELECT visits.id,
                source_profiles.profile_key,
                urls.url,
                urls.title,
                (visits.visit_time_ms * 1000 + 11644473600000000) AS visit_time
         FROM archive.visits AS visits
         JOIN archive.urls AS urls ON urls.id = visits.url_id
         JOIN archive.source_profiles AS source_profiles ON source_profiles.id = visits.source_profile_id
         WHERE visits.reverted_at IS NULL
         ORDER BY visits.visit_time_ms DESC"
    };

    let mut statement = connection.prepare(sql)?;
    let mut rows =
        if limit.is_some() { statement.query(params![limit_sql])? } else { statement.query([])? };

    let mut raw_visits = Vec::new();
    while let Some(row) = rows.next()? {
        raw_visits.push(IndexedVisit {
            history_id: row.get(0)?,
            profile_id: row.get(1)?,
            url: row.get(2)?,
            title: row.get(3)?,
            domain: String::new(),
            visited_at: crate::utils::chrome_time_to_rfc3339(row.get::<_, i64>(4)?),
            content: String::new(),
            content_hash: String::new(),
        });
    }

    let history_ids = raw_visits.iter().map(|visit| visit.history_id).collect::<Vec<_>>();
    let existing_hashes = load_existing_embedding_hashes(connection, provider, &history_ids)?;
    let enrichments = load_best_enrichment_map_by_history_ids(paths, connection, &history_ids)?;
    let mut visits = Vec::with_capacity(raw_visits.len());
    for mut visit in raw_visits {
        let enrichment = enrichments.get(&visit.history_id);
        let content = build_embedding_content_from_parts(
            &visit.profile_id,
            &visit.url,
            visit.title.as_deref(),
            &visit.visited_at,
            enrichment.and_then(|value| value.readable_title.as_deref()),
            enrichment.and_then(|value| value.readable_text.as_deref()),
        );
        let content_hash = sha256_hex(content.as_bytes());
        if existing_hashes.get(&visit.history_id) == Some(&content_hash) {
            continue;
        }
        visit.domain = url_domain(&visit.url);
        visit.content = content;
        visit.content_hash = content_hash;
        visits.push(visit);
    }
    Ok(visits)
}

fn cleanup_stale_embeddings(
    connection: &Connection,
    provider: &AiProviderRuntime,
) -> Result<usize> {
    #[rustfmt::skip]
    let removed = connection.execute(DELETE_STALE_EMBEDDINGS_SQL, params![provider.config.id, provider.config.default_model])?;
    Ok(removed)
}

fn collect_stale_history_ids(
    connection: &Connection,
    provider: &AiProviderRuntime,
) -> Result<Vec<i64>> {
    let mut statement = connection.prepare(
        "SELECT history_id
         FROM ai_embeddings
         WHERE provider_id = ?1
           AND model = ?2
           AND history_id NOT IN (
             SELECT id FROM archive.visits WHERE reverted_at IS NULL
           )",
    )?;
    statement
        .query_map(params![provider.config.id, provider.config.default_model], |row| row.get(0))?
        .collect::<std::result::Result<Vec<_>, _>>()
        .context("collecting stale AI embedding ids")
}

fn provider_embedding_count(
    connection: &Connection,
    provider_id: &str,
    model: &str,
) -> Result<i64> {
    #[rustfmt::skip]
    let count = connection.query_row(
        "SELECT COUNT(*) FROM ai_embeddings WHERE provider_id = ?1 AND model = ?2",
        params![provider_id, model],
        |row: &Row<'_>| row.get::<_, i64>(0),
    )?;
    Ok(count)
}

fn history_row_is_visible(connection: &Connection, history_id: i64) -> Result<bool> {
    connection
        .query_row(
            "SELECT 1
             FROM archive.visits
             WHERE id = ?1
               AND reverted_at IS NULL
             LIMIT 1",
            [history_id],
            |row: &Row<'_>| row.get::<_, i64>(0),
        )
        .optional()
        .map(|value| value.is_some())
        .context("checking semantic visibility against archive.visits")
}

fn sqlite_table_exists(connection: &Connection, table_name: &str) -> Result<bool> {
    let count: i64 = connection.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = ?1",
        [table_name],
        |row: &Row<'_>| row.get(0),
    )?;
    Ok(count > 0)
}

fn semantic_index_staleness_reason(
    connection: &Connection,
    provider_id: &str,
    _model: &str,
    source_watermark: i64,
    last_indexed_at: Option<&str>,
) -> Result<Option<String>> {
    if provider_embedding_count(connection, provider_id, _model)? == 0 {
        return Ok(None);
    }

    let visible_watermark = current_source_watermark(connection)?;
    if source_watermark != 0 && visible_watermark != source_watermark {
        return Ok(Some(
            "The semantic index no longer matches the current archive visibility or import watermark. Run Build index so semantic retrieval includes recent imports and reflects reverted rows."
                .to_string(),
        ));
    }

    if let Some(last_indexed_at) = last_indexed_at {
        if sqlite_table_exists(connection, "visit_content_enrichments")? {
            let latest_enrichment: Option<String> = connection
                .query_row(
                    "SELECT fetched_at
                     FROM visit_content_enrichments
                     WHERE fetch_status = 'success'
                     ORDER BY fetched_at DESC
                     LIMIT 1",
                    [],
                    |row: &Row<'_>| row.get(0),
                )
                .optional()?;
            if latest_enrichment.as_deref().is_some_and(|value| value > last_indexed_at) {
                return Ok(Some(
                    "Readable-content enrichment changed after the last semantic build. Run Build index to refresh embeddings with the latest extracted text."
                        .to_string(),
                ));
            }
        }
    }

    Ok(None)
}

fn ai_embeddings_storage_bytes(connection: &Connection) -> Result<u64> {
    if !sqlite_table_exists(connection, "ai_embeddings")? {
        return Ok(0);
    }
    let bytes: i64 = connection.query_row(
        "SELECT COALESCE(SUM(
            LENGTH(IFNULL(url, '')) +
            LENGTH(IFNULL(title, '')) +
            LENGTH(IFNULL(domain, '')) +
            LENGTH(IFNULL(visited_at, '')) +
            LENGTH(IFNULL(content_hash, '')) +
            LENGTH(IFNULL(provider_id, '')) +
            LENGTH(IFNULL(model, '')) +
            8
         ), 0)
         FROM ai_embeddings",
        [],
        |row: &Row<'_>| row.get(0),
    )?;
    Ok(bytes.max(0) as u64)
}

fn ai_embedding_token_estimate(connection: &Connection) -> Result<u64> {
    if !sqlite_table_exists(connection, "ai_embeddings")? {
        return Ok(0);
    }
    let characters: i64 = connection.query_row(
        "SELECT COALESCE(SUM(content_bytes), 0) FROM ai_embeddings",
        [],
        |row: &Row<'_>| row.get(0),
    )?;
    let characters = characters.max(0) as u64;
    Ok(characters.div_ceil(4))
}

fn clear_provider_embeddings(connection: &Connection, provider: &AiProviderRuntime) -> Result<()> {
    #[rustfmt::skip]
    connection.execute(CLEAR_PROVIDER_EMBEDDINGS_SQL, params![provider.config.id, provider.config.default_model])?;
    Ok(())
}

fn upsert_embedding(
    connection: &Connection,
    provider: &AiProviderRuntime,
    visit: &IndexedVisit,
    indexed_at: &str,
) -> Result<()> {
    #[rustfmt::skip]
    connection.execute(UPSERT_EMBEDDING_SQL, params![visit.history_id, visit.profile_id, visit.url, visit.title, visit.domain, visit.visited_at, visit.content_hash, visit.content.len() as i64, provider.config.id, provider.config.default_model, indexed_at])?;
    Ok(())
}

fn record_assistant_run(
    connection: &Connection,
    run_id: i64,
    request: &AiAssistantRequest,
    answer: &str,
    llm_provider_id: &str,
    embedding_provider_id: &str,
    citations: &[AiCitation],
    notes: &[String],
) -> Result<()> {
    #[rustfmt::skip]
    connection.execute(INSERT_ASSISTANT_RUN_SQL, params![run_id, request.question, answer, llm_provider_id, embedding_provider_id, serde_json::to_string(citations)?, serde_json::to_string(notes)?, now_rfc3339()])?;
    Ok(())
}

fn lexical_history_results(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    request: &AiSearchRequest,
    query: &str,
) -> Result<crate::models::HistoryQueryResponse> {
    list_history(
        paths,
        config,
        key,
        HistoryQuery {
            q: Some(query.to_string()),
            profile_id: request.profile_id.clone(),
            browser_kind: None,
            domain: request.domain.clone(),
            start_time_ms: None,
            end_time_ms: None,
            sort: Some("newest".to_string()),
            limit: Some(request.limit.unwrap_or(12).max(1)),
            page: None,
            cursor: None,
            regex_mode: Some(false),
        },
    )
}

fn sort_stored_embeddings_desc(left: &StoredEmbedding, right: &StoredEmbedding) -> Ordering {
    right.score.partial_cmp(&left.score).unwrap_or(Ordering::Equal)
}

#[cfg_attr(not(test), allow(dead_code))]
fn build_embedding_content(
    profile_id: &str,
    url: &str,
    title: Option<&str>,
    visited_at: &str,
) -> String {
    let title = title.unwrap_or("(untitled)");
    format!(
        "Profile: {profile_id}\nVisited at: {visited_at}\nURL: {url}\nDomain: {domain}\nTitle: {title}",
        domain = url_domain(url)
    )
}

fn history_entry_to_search_entry(item: &HistoryEntry, score: f32, reason: &str) -> AiSearchEntry {
    AiSearchEntry {
        history_id: item.id,
        profile_id: item.profile_id.clone(),
        url: item.url.clone(),
        title: item.title.clone(),
        domain: item.domain.clone(),
        visited_at: item.visited_at.clone(),
        score,
        match_reason: reason.to_string(),
    }
}

fn lexical_score(index: usize, limit: usize) -> f32 {
    0.42 + ((limit.saturating_sub(index)) as f32 / limit.max(1) as f32) * 0.18
}

fn lexical_boost(index: usize, limit: usize) -> f32 {
    ((limit.saturating_sub(index)) as f32 / limit.max(1) as f32) * 0.08
}

#[cfg(test)]
fn cosine_similarity(left: &[f32], right: &[f32]) -> f32 {
    let len = left.len().min(right.len());
    if len == 0 {
        return 0.0;
    }
    let mut dot = 0.0f32;
    let mut left_norm = 0.0f32;
    let mut right_norm = 0.0f32;
    for index in 0..len {
        dot += left[index] * right[index];
        left_norm += left[index] * left[index];
        right_norm += right[index] * right[index];
    }
    if left_norm == 0.0 || right_norm == 0.0 {
        0.0
    } else {
        dot / (left_norm.sqrt() * right_norm.sqrt())
    }
}

async fn embed_batch_with_retry(
    provider: &AiProviderRuntime,
    texts: &[String],
) -> Result<Vec<Vec<f32>>> {
    let mut attempts = 0usize;
    loop {
        match embed_text_batch(provider, texts).await {
            Ok(vectors) => return Ok(vectors),
            Err(error) if attempts < EMBEDDING_RETRY_ATTEMPTS => {
                attempts += 1;
                if embedding_error_is_rate_limited(&error) {
                    return Err(error);
                }
            }
            Err(error) => return Err(error),
        }
    }
}

async fn embed_single_with_retry(provider: &AiProviderRuntime, text: &str) -> Result<Vec<f32>> {
    let mut attempts = 0usize;
    loop {
        match embed_query(provider, text).await {
            Ok(vector) => return Ok(vector),
            Err(error) if attempts < EMBEDDING_RETRY_ATTEMPTS => {
                attempts += 1;
                if embedding_error_is_rate_limited(&error) {
                    return Err(error);
                }
            }
            Err(error) => return Err(error),
        }
    }
}

fn embedding_error_is_rate_limited(error: &anyhow::Error) -> bool {
    let message = error.to_string().to_lowercase();
    message.contains("rate limit") || message.contains("quota") || message.contains("429")
}

#[cfg(not(any(test, coverage)))]
async fn embed_text_batch(provider: &AiProviderRuntime, texts: &[String]) -> Result<Vec<Vec<f32>>> {
    match provider.config.request_format {
        AiRequestFormat::OpenAi | AiRequestFormat::Ollama | AiRequestFormat::LmStudio => {
            let mut builder = openai::Client::builder().api_key(provider.api_key.clone());
            if let Some(base_url) = provider.config.base_url.as_deref() {
                builder = builder.base_url(base_url);
            }
            let client = builder.build()?;
            let model = client.embedding_model_with_ndims(
                provider.config.default_model.clone(),
                provider.config.dimensions.unwrap_or(1536) as usize,
            );
            let embeddings = model.embed_texts(texts.to_vec()).await?;
            Ok(embeddings
                .into_iter()
                .map(|embedding| embedding.vec.into_iter().map(|value| value as f32).collect())
                .collect())
        }
        AiRequestFormat::Google => {
            let mut builder = gemini::Client::builder().api_key(provider.api_key.clone());
            if let Some(base_url) = provider.config.base_url.as_deref() {
                builder = builder.base_url(base_url);
            }
            let client = builder.build()?;
            let model = client.embedding_model_with_ndims(
                provider.config.default_model.clone(),
                provider.config.dimensions.unwrap_or(768) as usize,
            );
            let embeddings = model.embed_texts(texts.to_vec()).await?;
            Ok(embeddings
                .into_iter()
                .map(|embedding| embedding.vec.into_iter().map(|value| value as f32).collect())
                .collect())
        }
        AiRequestFormat::Anthropic => {
            anyhow::bail!("Anthropic request format does not support embeddings in rig.rs.")
        }
    }
}

#[cfg(any(test, coverage))]
async fn embed_text_batch(provider: &AiProviderRuntime, texts: &[String]) -> Result<Vec<Vec<f32>>> {
    let dimensions = match provider.config.request_format {
        AiRequestFormat::OpenAi | AiRequestFormat::Ollama | AiRequestFormat::LmStudio => {
            provider.config.dimensions.unwrap_or(1536)
        }
        AiRequestFormat::Google => provider.config.dimensions.unwrap_or(768),
        AiRequestFormat::Anthropic => {
            anyhow::bail!("Anthropic request format does not support embeddings in rig.rs.")
        }
    } as usize;

    Ok(texts
        .iter()
        .map(|text| {
            let fingerprint = sha256_hex(format!("{}::{text}", provider.config.id).as_bytes());
            let bytes = fingerprint.as_bytes();
            (0..dimensions)
                .map(|index| ((bytes[index % bytes.len()] % 13) as f32 + 1.0) / 13.0)
                .collect::<Vec<_>>()
        })
        .collect())
}

#[cfg(not(any(test, coverage)))]
async fn embed_query(provider: &AiProviderRuntime, query: &str) -> Result<Vec<f32>> {
    match provider.config.request_format {
        AiRequestFormat::OpenAi | AiRequestFormat::Ollama | AiRequestFormat::LmStudio => {
            let mut builder = openai::Client::builder().api_key(provider.api_key.clone());
            if let Some(base_url) = provider.config.base_url.as_deref() {
                builder = builder.base_url(base_url);
            }
            let client = builder.build()?;
            let model = client.embedding_model_with_ndims(
                provider.config.default_model.clone(),
                provider.config.dimensions.unwrap_or(1536) as usize,
            );
            let embedding = model.embed_text(query).await?;
            Ok(embedding.vec.iter().map(|value| *value as f32).collect())
        }
        AiRequestFormat::Google => {
            let mut builder = gemini::Client::builder().api_key(provider.api_key.clone());
            if let Some(base_url) = provider.config.base_url.as_deref() {
                builder = builder.base_url(base_url);
            }
            let client = builder.build()?;
            let model = client.embedding_model_with_ndims(
                provider.config.default_model.clone(),
                provider.config.dimensions.unwrap_or(768) as usize,
            );
            let embedding = model.embed_text(query).await?;
            Ok(embedding.vec.iter().map(|value| *value as f32).collect())
        }
        AiRequestFormat::Anthropic => {
            anyhow::bail!("Anthropic request format does not support embeddings in rig.rs.")
        }
    }
}

#[cfg(any(test, coverage))]
async fn embed_query(provider: &AiProviderRuntime, query: &str) -> Result<Vec<f32>> {
    let dimensions = match provider.config.request_format {
        AiRequestFormat::OpenAi | AiRequestFormat::Ollama | AiRequestFormat::LmStudio => {
            provider.config.dimensions.unwrap_or(1536)
        }
        AiRequestFormat::Google => provider.config.dimensions.unwrap_or(768),
        AiRequestFormat::Anthropic => {
            anyhow::bail!("Anthropic request format does not support embeddings in rig.rs.")
        }
    } as usize;

    let fingerprint = sha256_hex(format!("{}::{query}", provider.config.id).as_bytes());
    let bytes = fingerprint.as_bytes();
    Ok((0..dimensions)
        .map(|index| ((bytes[index % bytes.len()] % 13) as f32 + 1.0) / 13.0)
        .collect())
}

#[cfg(test)]
mod tests;
