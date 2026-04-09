use crate::{
    ai_queue::{self},
    ai_sidecar::{self, SidecarEmbeddingRow},
    archive::{create_schema, list_history, open_archive_connection},
    config::ProjectPaths,
    insights::preferred_embedding_content,
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
use std::{cmp::Ordering, collections::HashMap, sync::Arc, time::Instant};
use thiserror::Error;
use tokio::sync::Mutex;

#[derive(Debug, Clone)]
pub struct AiProviderRuntime {
    pub config: AiProviderConfig,
    pub api_key: String,
}

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

type SemanticRow = (i64, String, String, Option<String>, String, String, String);

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
      content TEXT NOT NULL,
      content_hash TEXT NOT NULL,
      provider_id TEXT NOT NULL,
      model TEXT NOT NULL,
      embedding_json TEXT NOT NULL,
      dimensions INTEGER NOT NULL,
      indexed_at TEXT NOT NULL,
      UNIQUE(history_id, provider_id, model, content_hash)
    );
    CREATE TABLE IF NOT EXISTS ai_assistant_runs (
      id INTEGER PRIMARY KEY AUTOINCREMENT,
      run_id INTEGER REFERENCES runs(id),
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
      last_run_id INTEGER REFERENCES runs(id),
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

const SEMANTIC_MATCHES_SQL: &str = r#"
    SELECT
      ai_embeddings.history_id,
      ai_embeddings.profile_id,
      ai_embeddings.url,
      ai_embeddings.title,
      ai_embeddings.domain,
      ai_embeddings.visited_at,
      ai_embeddings.embedding_json
    FROM ai_embeddings
    JOIN visit_events
      ON visit_events.id = ai_embeddings.history_id
    WHERE ai_embeddings.provider_id = ?1
      AND ai_embeddings.model = ?2
      AND (?3 IS NULL OR ai_embeddings.profile_id = ?3)
      AND (?4 IS NULL OR ai_embeddings.domain LIKE '%' || ?4 || '%')
"#;

const CLEAR_PROVIDER_EMBEDDINGS_SQL: &str =
    "DELETE FROM ai_embeddings WHERE provider_id = ?1 AND model = ?2";
const DELETE_STALE_EMBEDDINGS_SQL: &str = "DELETE FROM ai_embeddings WHERE provider_id = ?1 AND model = ?2 AND history_id NOT IN (SELECT id FROM visit_events)";
const UPSERT_EMBEDDING_SQL: &str = "INSERT OR REPLACE INTO ai_embeddings (history_id, profile_id, url, title, domain, visited_at, content, content_hash, provider_id, model, embedding_json, dimensions, indexed_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)";
const INSERT_ASSISTANT_RUN_SQL: &str = "INSERT INTO ai_assistant_runs (run_id, question, answer, provider_id, embedding_provider_id, citations_json, notes_json, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)";
const AI_QUEUE_RECENT_LIMIT: usize = 8;
const AI_INDEX_LEDGER_VERSION: &str = "semantic-sidecar-v1";
const EMBEDDING_BATCH_SIZE: usize = 32;
const EMBEDDING_RETRY_ATTEMPTS: usize = 2;

#[derive(Debug, Clone)]
struct SearchContext {
    paths: ProjectPaths,
    config: AppConfig,
    database_key: Option<String>,
    embedding_provider: Option<AiProviderRuntime>,
    default_profile_id: Option<String>,
    default_domain: Option<String>,
    default_limit: u32,
    citations: Arc<Mutex<Vec<AiCitation>>>,
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

pub fn ensure_ai_schema(connection: &Connection) -> Result<()> {
    connection.execute_batch(AI_SCHEMA_SQL)?;
    ensure_ai_assistant_run_columns(connection)?;
    Ok(())
}

pub fn ai_index_status(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
) -> Result<AiIndexStatus> {
    let default_queue_status = AiQueueStatus {
        paused: config.ai.job_queue_paused,
        concurrency: config.ai.job_queue_concurrency,
        ..AiQueueStatus::default()
    };
    if !config.initialized || !paths.archive_database_path.exists() {
        return Ok(AiIndexStatus {
            enabled: config.ai.enabled,
            assistant_enabled: config.ai.assistant_enabled,
            mcp_enabled: config.ai.mcp_enabled,
            skill_enabled: config.ai.skill_enabled,
            state: if config.ai.enabled { "blocked".to_string() } else { "disabled".to_string() },
            llm_provider_id: config.ai.llm_provider_id.clone(),
            embedding_provider_id: config.ai.embedding_provider_id.clone(),
            queue_paused: default_queue_status.paused,
            queue_concurrency: default_queue_status.concurrency,
            warning: if config.ai.enabled {
                Some("Initialize the archive before using AI analysis features.".to_string())
            } else {
                None
            },
            ..AiIndexStatus::default()
        });
    }

    let connection = open_archive_connection(paths, config, key)?;
    create_schema(&connection)?;
    ensure_ai_schema(&connection)?;
    let queue_status = ai_queue::load_ai_queue_status(
        &connection,
        config.ai.job_queue_paused,
        config.ai.job_queue_concurrency,
        AI_QUEUE_RECENT_LIMIT,
    )?;
    let index_queue_counts = ai_queue::load_queue_job_counts(
        &connection,
        &[AiQueueJobType::IndexBuild, AiQueueJobType::IndexClear],
    )?;

    let provider_id = config.ai.embedding_provider_id.clone();
    let provider_readiness = embedding_provider_readiness(config);
    let ledger = if let Some(provider_id) = provider_id.as_deref() {
        provider_readiness
            .selected_model
            .as_deref()
            .map(|model| load_index_ledger(&connection, provider_id, model))
            .transpose()?
            .unwrap_or_default()
    } else {
        AiIndexLedgerRow::default()
    };
    let indexed_items = if let Some((provider_id, model)) =
        provider_id.as_deref().zip(provider_readiness.selected_model.as_deref())
    {
        provider_embedding_count(&connection, provider_id, model)?
    } else {
        0
    };
    let semantic_sidecar_bytes = ai_sidecar::sidecar_storage_bytes(paths);
    let semantic_mirror_bytes = ai_embeddings_storage_bytes(&connection)?;
    let estimated_embedding_tokens = ai_embedding_token_estimate(&connection)?;
    let staleness_reason = provider_id
        .as_deref()
        .zip(provider_readiness.selected_model.as_deref())
        .map(|(provider_id, model)| {
            semantic_index_staleness_reason(
                &connection,
                provider_id,
                model,
                ledger.source_watermark,
                ledger.last_indexed_at.as_deref(),
            )
        })
        .transpose()?
        .flatten();
    let last_indexed_at = ledger.last_indexed_at.clone().or_else(|| {
        provider_id.as_deref().zip(provider_readiness.selected_model.as_deref()).and_then(
            |(provider_id, model)| {
                connection
                    .query_row(
                        "SELECT indexed_at
                     FROM ai_embeddings
                     WHERE provider_id = ?1
                       AND model = ?2
                     ORDER BY indexed_at DESC
                     LIMIT 1",
                        params![provider_id, model],
                        |row: &Row<'_>| row.get(0),
                    )
                    .optional()
                    .ok()
                    .flatten()
            },
        )
    });
    let ready = indexed_items > 0 && provider_readiness.available;
    let state = if !config.ai.enabled {
        "disabled".to_string()
    } else if !provider_readiness.available {
        "degraded".to_string()
    } else if index_queue_counts.running > 0 {
        "rebuilding".to_string()
    } else if queue_status.paused && index_queue_counts.queued > 0 {
        "paused".to_string()
    } else if ledger.state == "failed" {
        "failed".to_string()
    } else if staleness_reason.is_some() {
        "stale".to_string()
    } else if ready {
        "ready".to_string()
    } else if index_queue_counts.queued > 0 {
        "queued".to_string()
    } else {
        "empty".to_string()
    };
    Ok(AiIndexStatus {
        enabled: config.ai.enabled,
        assistant_enabled: config.ai.assistant_enabled,
        mcp_enabled: config.ai.mcp_enabled,
        skill_enabled: config.ai.skill_enabled,
        state,
        ready,
        indexed_items: indexed_items as usize,
        last_indexed_at,
        llm_provider_id: config.ai.llm_provider_id.clone(),
        embedding_provider_id: config.ai.embedding_provider_id.clone(),
        queue_paused: queue_status.paused,
        queue_concurrency: queue_status.concurrency,
        queued_jobs: queue_status.queued,
        running_jobs: queue_status.running,
        failed_jobs: queue_status.failed,
        recent_jobs: queue_status.recent_jobs,
        semantic_sidecar_bytes,
        semantic_mirror_bytes,
        estimated_embedding_tokens,
        warning: if ledger.state == "failed" {
            ledger.failure_reason.or(ledger.last_failure_at)
        } else if !provider_readiness.available {
            provider_readiness.warning
        } else if staleness_reason.is_some() {
            staleness_reason
        } else if config.ai.enabled && !ready {
            Some("Run Build index after configuring an embedding provider to enable semantic search.".to_string())
        } else {
            None
        },
    })
}

pub fn ai_queue_status(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
) -> Result<AiQueueStatus> {
    if !config.initialized || !paths.archive_database_path.exists() {
        return Ok(AiQueueStatus {
            paused: config.ai.job_queue_paused,
            concurrency: config.ai.job_queue_concurrency,
            ..AiQueueStatus::default()
        });
    }

    let connection = open_archive_connection(paths, config, key)?;
    create_schema(&connection)?;
    ensure_ai_schema(&connection)?;
    ai_queue::load_ai_queue_status(
        &connection,
        config.ai.job_queue_paused,
        config.ai.job_queue_concurrency,
        AI_QUEUE_RECENT_LIMIT,
    )
}

pub fn reconcile_ai_queue_controls(
    paths: &ProjectPaths,
    previous_config: &AppConfig,
    next_config: &AppConfig,
    key: Option<&str>,
) -> Result<()> {
    if !next_config.initialized || !paths.archive_database_path.exists() {
        return Ok(());
    }
    if previous_config.ai.job_queue_paused == next_config.ai.job_queue_paused {
        return Ok(());
    }

    let connection = open_archive_connection(paths, next_config, key)?;
    create_schema(&connection)?;
    ensure_ai_schema(&connection)?;
    ai_queue::ensure_ai_queue_schema(&connection)?;

    if next_config.ai.job_queue_paused {
        ai_queue::pause_queued_jobs(&connection)?;
    } else {
        ai_queue::resume_paused_jobs(&connection)?;
    }

    Ok(())
}

pub fn provider_capabilities(config: &AiProviderConfig) -> AiProviderCapabilityReport {
    let supports_embeddings = matches!(
        (config.purpose.clone(), config.request_format.clone()),
        (
            AiProviderPurpose::Embedding,
            AiRequestFormat::OpenAi
                | AiRequestFormat::Google
                | AiRequestFormat::Ollama
                | AiRequestFormat::LmStudio
        )
    );
    let supports_chat = matches!(config.purpose, AiProviderPurpose::Llm);
    let supports_streaming = supports_chat;
    let supports_tool_use = supports_chat
        && matches!(
            config.request_format,
            AiRequestFormat::OpenAi
                | AiRequestFormat::Anthropic
                | AiRequestFormat::Google
                | AiRequestFormat::Ollama
                | AiRequestFormat::LmStudio
        );
    let supports_structured_output = supports_chat
        && matches!(
            config.request_format,
            AiRequestFormat::OpenAi
                | AiRequestFormat::Anthropic
                | AiRequestFormat::Google
                | AiRequestFormat::Ollama
                | AiRequestFormat::LmStudio
        );
    AiProviderCapabilityReport {
        supports_chat,
        supports_embeddings,
        supports_streaming,
        supports_tool_use,
        supports_structured_output,
    }
}

pub fn provider_connection_failure_report(
    config: &AiProviderConfig,
    message: &str,
) -> AiProviderConnectionTestReport {
    let (error_code, action_hint, retry_hint) = classify_provider_error(message);
    AiProviderConnectionTestReport {
        provider_id: config.id.clone(),
        purpose: match config.purpose {
            AiProviderPurpose::Embedding => "embedding".to_string(),
            AiProviderPurpose::Llm => "llm".to_string(),
        },
        model: config.default_model.clone(),
        ok: false,
        latency_ms: 0,
        capabilities: provider_capabilities(config),
        error_code,
        action_hint,
        retry_hint,
        warnings: Vec::new(),
        message: message.to_string(),
    }
}

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

pub async fn build_ai_index(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    provider: &AiProviderRuntime,
    request: &AiIndexRequest,
) -> Result<AiIndexReport> {
    validate_provider(provider, AiProviderPurpose::Embedding)?;
    let connection = open_archive_connection(paths, config, key)?;
    create_schema(&connection)?;
    ensure_ai_schema(&connection)?;
    let started_at = now_rfc3339();
    let source_watermark = current_source_watermark(&connection)?;
    let sidecar_table =
        ai_sidecar::provider_table_name(&provider.config.id, &provider.config.default_model);
    let run_id = begin_ai_run(
        &connection,
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

        let candidates = collect_visits_to_index(&connection, provider, request.limit)?;
        if candidates.is_empty() {
            ai_sidecar::sync_provider_embeddings(
                paths,
                &provider.config.id,
                &provider.config.default_model,
                &[],
                request.full_rebuild,
                false,
                &stale_history_ids,
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

        for batch in candidates.chunks(EMBEDDING_BATCH_SIZE) {
            let texts = batch.iter().map(|visit| visit.content.clone()).collect::<Vec<_>>();
            match embed_batch_with_retry(provider, &texts).await {
                Ok(vectors) if vectors.len() == batch.len() => {
                    for (visit, vector) in batch.iter().zip(vectors.into_iter()) {
                        let had_prior_index = connection
                            .query_row(
                                "SELECT id
                                 FROM ai_embeddings
                                 WHERE history_id = ?1
                                   AND provider_id = ?2
                                   AND model = ?3
                                 LIMIT 1",
                                params![
                                    visit.history_id,
                                    provider.config.id,
                                    provider.config.default_model
                                ],
                                |row: &Row<'_>| row.get::<_, i64>(0),
                            )
                            .optional()?
                            .is_some();
                        upsert_embedding(&connection, provider, visit, &vector, &timestamp)?;
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
                        let had_prior_index = connection
                            .query_row(
                                "SELECT id
                                 FROM ai_embeddings
                                 WHERE history_id = ?1
                                   AND provider_id = ?2
                                   AND model = ?3
                                 LIMIT 1",
                                params![
                                    visit.history_id,
                                    provider.config.id,
                                    provider.config.default_model
                                ],
                                |row: &Row<'_>| row.get::<_, i64>(0),
                            )
                            .optional()?
                            .is_some();
                        match embed_single_with_retry(provider, &visit.content).await {
                            Ok(vector) => {
                                upsert_embedding(&connection, provider, visit, &vector, &timestamp)?;
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
        let sidecar_synced = ai_sidecar::sync_provider_embeddings(
            paths,
            &provider.config.id,
            &provider.config.default_model,
            &sidecar_rows,
            request.full_rebuild,
            false,
            &stale_history_ids,
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
                    "Synced {} row(s) into the LanceDB semantic sidecar. Search still keeps a temporary SQLite compatibility mirror as a fallback path.",
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
                &connection,
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
                &connection,
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

pub async fn semantic_search_history(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    provider: Option<&AiProviderRuntime>,
    request: &AiSearchRequest,
) -> Result<AiSearchResponse> {
    search_history_internal(paths, config, key, provider, request).await
}

pub async fn answer_history_question(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    llm_provider: &AiProviderRuntime,
    embedding_provider: Option<&AiProviderRuntime>,
    request: &AiAssistantRequest,
) -> Result<AiAssistantResponse> {
    validate_provider(llm_provider, AiProviderPurpose::Llm)?;
    if !config.ai.enabled || !config.ai.assistant_enabled {
        anyhow::bail!("Enable AI analysis and the assistant in Settings before asking questions.")
    }
    let connection = open_archive_connection(paths, config, key)?;
    create_schema(&connection)?;
    ensure_ai_schema(&connection)?;
    let run_id = begin_ai_run(
        &connection,
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
        let search_response =
            search_history_internal(paths, config, key, embedding_provider, &retrieval_request)
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
        };
        let tools: Vec<Box<dyn ToolDyn>> =
            vec![Box::new(SearchHistoryTool { context: tool_context })];
        let preamble = build_assistant_preamble(config, &search_response);
        let answer = run_llm_agent(llm_provider, &preamble, tools, &request.question).await?;

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
                &connection,
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
                &connection,
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

pub fn load_assistant_run_response(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    run_id: i64,
) -> Result<AiAssistantResponse> {
    let connection = open_archive_connection(paths, config, key)?;
    create_schema(&connection)?;
    ensure_ai_schema(&connection)?;
    connection
        .query_row(
            "SELECT answer, provider_id, embedding_provider_id, citations_json, notes_json
             FROM ai_assistant_runs
             WHERE run_id = ?1",
            [run_id],
            |row| {
                let citations_json: String = row.get(3)?;
                let notes_json: String = row.get(4)?;
                let citations =
                    serde_json::from_str::<Vec<AiCitation>>(&citations_json).map_err(|error| {
                        rusqlite::Error::FromSqlConversionFailure(
                            3,
                            rusqlite::types::Type::Text,
                            Box::new(error),
                        )
                    })?;
                let notes = serde_json::from_str::<Vec<String>>(&notes_json).map_err(|error| {
                    rusqlite::Error::FromSqlConversionFailure(
                        4,
                        rusqlite::types::Type::Text,
                        Box::new(error),
                    )
                })?;
                Ok(AiAssistantResponse {
                    state: if citations.is_empty() {
                        "insufficient-evidence".to_string()
                    } else {
                        "completed".to_string()
                    },
                    answer: row.get(0)?,
                    job_id: None,
                    run_id: Some(run_id),
                    provider_id: row.get(1)?,
                    embedding_provider_id: row.get(2)?,
                    citations,
                    notes,
                })
            },
        )
        .with_context(|| format!("loading AI assistant run {run_id}"))
}

pub fn preview_ai_integrations(
    paths: &ProjectPaths,
    config: &AppConfig,
) -> Result<AiIntegrationPreview> {
    let executable = std::env::current_exe()
        .ok()
        .map(|path| path.display().to_string())
        .unwrap_or_else(|| "<path-to-pathkeep>".to_string());
    let mcp_command = format!("{executable} --worker mcp-server");
    let codex_skill = "# PathKeep Search\n\nUse this skill when the user wants evidence from PathKeep.\n\n1. Make sure the local MCP server is configured in your Codex MCP settings.\n2. Use the `search_history` tool to find visits relevant to the question.\n3. Quote the visit date, URL, and profile when answering.\n\nIf the archive is encrypted, remind the user that the database key must be available in the system keyring before MCP queries can work.\n".to_string();
    let mcp_config = json!({
        "mcpServers": {
            "pathkeep": {
                "command": executable,
                "args": ["--worker", "mcp-server"]
            }
        }
    });
    let providerless_note = if config.ai.embedding_provider_id.is_some() {
        "Semantic retrieval can use the configured embedding provider when the semantic index is built.".to_string()
    } else {
        "No embedding provider is selected right now, so MCP and external assistants fall back to lexical recall only. They still respect archive visibility and App Lock."
            .to_string()
    };
    Ok(AiIntegrationPreview {
        mcp_command,
        consent_summary:
            "External AI integrations stay local-first and explicit. PathKeep only exposes localhost MCP tools after you turn on AI + MCP in Settings, and the current app session must stay unlocked."
                .to_string(),
        manual_steps: vec![
            "Enable MCP or Skill integration in Settings first. Both are off by default.".to_string(),
            "Store the database key in the native keyring if the archive is encrypted, so background and MCP lookups can unlock the archive.".to_string(),
            "Copy the generated MCP JSON into your local MCP client configuration and restart that client.".to_string(),
            "Copy the generated skill markdown into your local skills directory if you want a reusable history-research workflow.".to_string(),
        ],
        capability_notes: vec![
            if config.ai.mcp_enabled {
                "MCP server toggle is currently enabled in saved Settings.".to_string()
            } else {
                "MCP server toggle is currently disabled in saved Settings.".to_string()
            },
            if config.ai.skill_enabled {
                "Skill integration toggle is currently enabled in saved Settings.".to_string()
            } else {
                "Skill integration toggle is currently disabled in saved Settings.".to_string()
            },
            providerless_note,
        ],
        scope_boundary: vec![
            "Queries only see currently visible archive facts. Reverted visits stay hidden even if an old embedding row still exists.".to_string(),
            "If App Lock re-locks the session, MCP search returns a locked refusal instead of reading the archive behind the UI.".to_string(),
            "The MCP surface is localhost-only and never publishes the archive to a remote PathKeep service.".to_string(),
        ],
        audit_trace: vec![
            "Each MCP search writes a dedicated run-ledger entry so Audit can show that an external tool queried the archive.".to_string(),
            "Assistant and semantic-index runs keep their own run IDs and do not masquerade as backup runs.".to_string(),
        ],
        generated_files: vec![
            crate::models::GeneratedFile {
                relative_path: "integrations/pathkeep-mcp.json".to_string(),
                absolute_path: Some(
                    paths.app_root
                        .join("integrations/pathkeep-mcp.json")
                        .display()
                        .to_string(),
                ),
                purpose: "Local MCP client configuration snippet for PathKeep.".to_string(),
                contents: serde_json::to_string_pretty(&mcp_config)?,
            },
            crate::models::GeneratedFile {
                relative_path: "integrations/codex-pathkeep-skill/SKILL.md".to_string(),
                absolute_path: Some(
                    paths.app_root
                        .join("integrations/codex-pathkeep-skill/SKILL.md")
                        .display()
                        .to_string(),
                ),
                purpose: "Codex skill starter that teaches an external assistant how to query PathKeep through MCP.".to_string(),
                contents: codex_skill,
            },
        ],
        warnings: if config.ai.mcp_enabled || config.ai.skill_enabled {
            Vec::new()
        } else {
            vec!["MCP and skill integration are both disabled in Settings right now.".to_string()]
        },
    })
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

fn ensure_ai_assistant_run_columns(connection: &Connection) -> Result<()> {
    let mut statement = connection.prepare("PRAGMA table_info(ai_assistant_runs)")?;
    let columns = statement
        .query_map([], |row| row.get::<_, String>(1))?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    if !columns.iter().any(|column| column == "run_id") {
        connection.execute(
            "ALTER TABLE ai_assistant_runs ADD COLUMN run_id INTEGER REFERENCES runs(id)",
            [],
        )?;
    }
    Ok(())
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
        .query_row("SELECT COALESCE(MAX(id), 0) FROM visit_events", [], |row| row.get(0))
        .context("loading latest visit watermark for AI indexing")
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
    let connection = open_archive_connection(paths, config, key)?;
    create_schema(&connection)?;
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
                    "The LanceDB semantic sidecar is missing or empty, so PathKeep fell back to the SQLite compatibility mirror."
                        .to_string(),
                );
            }
        }
        Err(error) => {
            notes.push(format!(
                "The LanceDB semantic sidecar could not answer this search ({}). PathKeep fell back to the SQLite compatibility mirror.",
                error
            ));
        }
    }

    let rows = load_semantic_rows(&connection, provider, request)?;

    let mut scored = Vec::new();
    for row in rows {
        let (history_id, profile_id, url, title, domain, visited_at, embedding_json) = row;
        let stored_vector = serde_json::from_str::<Vec<f32>>(&embedding_json)
            .with_context(|| format!("parsing ai embedding for history row {history_id}"))?;
        let score = cosine_similarity(&query_vector, &stored_vector);
        if score.is_finite() {
            scored.push(StoredEmbedding {
                history_id,
                profile_id,
                url,
                title,
                domain,
                visited_at,
                score,
            });
        }
    }

    scored.sort_by(sort_stored_embeddings_desc);
    Ok(SemanticMatchReport { items: scored, notes })
}

fn collect_visits_to_index(
    connection: &Connection,
    provider: &AiProviderRuntime,
    limit: Option<u32>,
) -> Result<Vec<IndexedVisit>> {
    let limit_sql = limit.unwrap_or(0).max(1);
    let sql = if limit.is_some() {
        "SELECT id, profile_id, url, title, visit_time
         FROM visit_events
         ORDER BY visit_time DESC
         LIMIT ?1"
    } else {
        "SELECT id, profile_id, url, title, visit_time
         FROM visit_events
         ORDER BY visit_time DESC"
    };

    let mut statement = connection.prepare(sql)?;
    let mut rows =
        if limit.is_some() { statement.query(params![limit_sql])? } else { statement.query([])? };

    let mut visits = Vec::new();
    while let Some(row) = rows.next()? {
        let history_id: i64 = row.get(0)?;
        let profile_id: String = row.get(1)?;
        let url: String = row.get(2)?;
        let title: Option<String> = row.get(3)?;
        let visited_at = crate::utils::chrome_time_to_rfc3339(row.get::<_, i64>(4)?);
        let domain = url_domain(&url);
        let content = preferred_embedding_content(
            connection,
            history_id,
            &profile_id,
            &url,
            title.as_deref(),
            &visited_at,
        )?;
        let content_hash = sha256_hex(content.as_bytes());

        let exists: Option<i64> = connection
            .query_row(
                "SELECT id
                 FROM ai_embeddings
                 WHERE history_id = ?1
                   AND provider_id = ?2
                   AND model = ?3
                   AND content_hash = ?4
                 LIMIT 1",
                params![
                    history_id,
                    provider.config.id,
                    provider.config.default_model,
                    content_hash
                ],
                |inner_row| inner_row.get(0),
            )
            .optional()?;
        if exists.is_none() {
            visits.push(IndexedVisit {
                history_id,
                profile_id,
                url,
                title,
                domain,
                visited_at,
                content,
                content_hash,
            });
        }
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
           AND history_id NOT IN (SELECT id FROM visit_events)",
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
            "SELECT 1 FROM visit_events WHERE id = ?1 LIMIT 1",
            [history_id],
            |row: &Row<'_>| row.get::<_, i64>(0),
        )
        .optional()
        .map(|value| value.is_some())
        .context("checking semantic visibility against visit_events")
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
            LENGTH(IFNULL(content, '')) +
            LENGTH(IFNULL(content_hash, '')) +
            LENGTH(IFNULL(provider_id, '')) +
            LENGTH(IFNULL(model, '')) +
            LENGTH(IFNULL(embedding_json, ''))
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
        "SELECT COALESCE(SUM(LENGTH(IFNULL(content, ''))), 0) FROM ai_embeddings",
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
    vector: &[f32],
    indexed_at: &str,
) -> Result<()> {
    #[rustfmt::skip]
    connection.execute(UPSERT_EMBEDDING_SQL, params![visit.history_id, visit.profile_id, visit.url, visit.title, visit.domain, visit.visited_at, visit.content, visit.content_hash, provider.config.id, provider.config.default_model, serde_json::to_string(vector)?, vector.len() as i64, indexed_at])?;
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
            cursor: None,
            regex_mode: Some(false),
        },
    )
}

fn load_semantic_rows(
    connection: &Connection,
    provider: &AiProviderRuntime,
    request: &AiSearchRequest,
) -> Result<Vec<SemanticRow>> {
    let mut statement = connection.prepare(SEMANTIC_MATCHES_SQL)?;
    #[rustfmt::skip]
    let mut rows = statement.query(params![provider.config.id, provider.config.default_model, request.profile_id, request.domain])?;
    let mut collected = Vec::new();
    while let Some(row) = rows.next()? {
        let embedding_json: String = row.get(6)?;
        collected.push((
            row.get::<_, i64>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, Option<String>>(3)?,
            row.get::<_, String>(4)?,
            row.get::<_, String>(5)?,
            embedding_json,
        ));
    }
    Ok(collected)
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
mod tests {
    use super::*;
    use crate::{
        archive::ensure_archive_initialized,
        models::{AiSettings, ArchiveMode},
    };
    use rusqlite::params;
    use std::{
        fs,
        sync::atomic::{AtomicU64, Ordering},
    };
    use tokio::runtime::Runtime;

    fn test_paths() -> ProjectPaths {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time")
            .as_nanos();
        let sequence = COUNTER.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "pathkeep-ai-test-{}-{}-{}",
            std::process::id(),
            unique,
            sequence
        ));
        fs::create_dir_all(&root).expect("create temp root");
        ProjectPaths {
            app_root: root.clone(),
            config_path: root.join("config.json"),
            archive_database_path: root.join("archive/history-vault.sqlite"),
            audit_repo_path: root.join("audit"),
            manifests_dir: root.join("audit/manifests"),
            exports_dir: root.join("exports"),
            raw_snapshots_dir: root.join("raw-snapshots"),
            staging_dir: root.join("staging"),
            quarantine_dir: root.join("quarantine"),
            schedule_dir: root.join("schedule"),
            stronghold_path: root.join("vault.hold"),
            stronghold_salt_path: root.join("stronghold-salt.txt"),
        }
    }

    fn base_config() -> AppConfig {
        let mut llm_config = llm_provider().config;
        llm_config.api_key_saved = true;
        let mut embedding_config = embedding_provider().config;
        embedding_config.api_key_saved = true;
        AppConfig {
            initialized: true,
            archive_mode: ArchiveMode::Plaintext,
            git_enabled: false,
            ai: AiSettings {
                enabled: true,
                assistant_enabled: true,
                semantic_index_enabled: true,
                llm_provider_id: Some("llm".to_string()),
                embedding_provider_id: Some("embed".to_string()),
                llm_providers: vec![llm_config],
                embedding_providers: vec![embedding_config],
                ..AiSettings::default()
            },
            ..AppConfig::default()
        }
    }

    fn embedding_provider() -> AiProviderRuntime {
        AiProviderRuntime {
            config: AiProviderConfig {
                id: "embed".to_string(),
                name: "Embedding provider".to_string(),
                purpose: AiProviderPurpose::Embedding,
                request_format: AiRequestFormat::OpenAi,
                enabled: true,
                default_model: "text-embedding-3-small".to_string(),
                dimensions: Some(3),
                ..AiProviderConfig::default()
            },
            api_key: "secret".to_string(),
        }
    }

    fn llm_provider() -> AiProviderRuntime {
        AiProviderRuntime {
            config: AiProviderConfig {
                id: "llm".to_string(),
                name: "LLM provider".to_string(),
                purpose: AiProviderPurpose::Llm,
                request_format: AiRequestFormat::OpenAi,
                enabled: true,
                default_model: "gpt-4.1-mini".to_string(),
                ..AiProviderConfig::default()
            },
            api_key: "secret".to_string(),
        }
    }

    fn llm_provider_with_format(request_format: AiRequestFormat) -> AiProviderRuntime {
        let mut provider = llm_provider();
        provider.config.request_format = request_format;
        provider
    }

    fn expected_stub_embedding(provider_id: &str, query: &str, dimensions: usize) -> Vec<f32> {
        let fingerprint = sha256_hex(format!("{provider_id}::{query}").as_bytes());
        let bytes = fingerprint.as_bytes();
        (0..dimensions)
            .map(|index| ((bytes[index % bytes.len()] % 13) as f32 + 1.0) / 13.0)
            .collect()
    }

    fn seed_visit(
        connection: &Connection,
        history_id: i64,
        profile_id: &str,
        url: &str,
        title: Option<&str>,
        visit_time: i64,
    ) {
        connection
            .execute(
                "INSERT INTO visit_events
                 (id, profile_id, source_visit_id, source_url_id, url, title, visit_time, from_visit, transition, visit_duration, is_known_to_sync, visited_link_id, external_referrer_url, app_id, event_fingerprint, payload_hash, recorded_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, NULL, 805306368, 0, 1, 0, NULL, NULL, ?8, ?9, ?10)",
                params![
                    history_id,
                    profile_id,
                    history_id,
                    history_id,
                    url,
                    title,
                    visit_time,
                    format!("fp-{history_id}"),
                    format!("payload-{history_id}"),
                    now_rfc3339()
                ],
            )
            .expect("insert visit");
    }

    fn seed_embedding(
        connection: &Connection,
        history_id: i64,
        provider: &AiProviderRuntime,
        content_hash: &str,
    ) {
        connection
            .execute(
                "INSERT INTO ai_embeddings
                 (history_id, profile_id, url, title, domain, visited_at, content, content_hash, provider_id, model, embedding_json, dimensions, indexed_at)
                 VALUES (?1, 'chrome:Default', 'https://example.com', 'Example', 'example.com', '2026-04-04T00:00:00Z', 'content', ?2, ?3, ?4, '[1.0,0.0,0.0]', 3, ?5)",
                params![
                    history_id,
                    content_hash,
                    provider.config.id,
                    provider.config.default_model,
                    now_rfc3339()
                ],
            )
            .expect("insert embedding");
    }

    fn seed_embedding_with_vector(
        connection: &Connection,
        history_id: i64,
        provider: &AiProviderRuntime,
        vector: &[f32],
    ) {
        let vector_json = serde_json::to_string(vector).expect("serialize vector");
        connection
            .execute(
                "INSERT INTO ai_embeddings
                 (history_id, profile_id, url, title, domain, visited_at, content, content_hash, provider_id, model, embedding_json, dimensions, indexed_at)
                 VALUES (?1, 'chrome:Default', 'https://example.com', 'Example', 'example.com', '2026-04-04T00:00:00Z', 'content', ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    history_id,
                    format!("hash-{history_id}"),
                    provider.config.id,
                    provider.config.default_model,
                    vector_json,
                    vector.len() as i64,
                    now_rfc3339()
                ],
            )
            .expect("insert embedding with vector");
    }

    fn prepared_archive() -> (ProjectPaths, AppConfig, Connection) {
        let paths = test_paths();
        let config = base_config();
        ensure_archive_initialized(&paths, &config, None).expect("init archive");
        let connection = open_archive_connection(&paths, &config, None).expect("open archive");
        create_schema(&connection).expect("create schema");
        ensure_ai_schema(&connection).expect("ensure ai schema");
        (paths, config, connection)
    }

    fn seed_failed_index_ledger(
        connection: &Connection,
        provider: &AiProviderRuntime,
        failure_reason: &str,
    ) {
        connection
            .execute(
                "INSERT OR REPLACE INTO ai_index_ledger (
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
                 VALUES (?1, ?2, 'ai_embeddings', 'test-v1', 'failed', NULL, NULL, NULL, NULL, NULL, NULL, ?3, ?4)",
                params![
                    provider.config.id,
                    provider.config.default_model,
                    now_rfc3339(),
                    failure_reason,
                ],
            )
            .expect("insert failed ledger");
    }

    #[test]
    fn cosine_similarity_handles_empty_vectors() {
        assert_eq!(cosine_similarity(&[], &[]), 0.0);
        assert_eq!(cosine_similarity(&[1.0], &[0.0]), 0.0);
    }

    #[test]
    fn build_embedding_content_stays_stable() {
        let rendered = build_embedding_content(
            "chrome:Default",
            "https://example.com/docs",
            Some("Docs"),
            "2026-04-04T00:00:00Z",
        );
        assert!(rendered.contains("chrome:Default"));
        assert!(rendered.contains("example.com"));
        assert!(rendered.contains("Docs"));
    }

    #[test]
    fn preview_ai_integrations_returns_mcp_and_skill_artifacts() {
        let paths = test_paths();
        let preview = preview_ai_integrations(&paths, &AppConfig::default()).expect("preview");
        assert_eq!(preview.generated_files.len(), 2);
        assert!(preview.mcp_command.contains("--worker mcp-server"));
        assert!(!preview.manual_steps.is_empty());
        assert_eq!(
            preview.warnings,
            vec!["MCP and skill integration are both disabled in Settings right now.".to_string()]
        );

        let mut partially_enabled = AppConfig::default();
        partially_enabled.ai.mcp_enabled = true;
        let enabled_preview =
            preview_ai_integrations(&paths, &partially_enabled).expect("enabled preview");
        assert!(enabled_preview.warnings.is_empty());
    }

    #[test]
    fn validate_provider_rejects_anthropic_embeddings() {
        let error = validate_provider(
            &AiProviderRuntime {
                config: AiProviderConfig {
                    id: "embed".to_string(),
                    name: "Anthropic embeddings".to_string(),
                    purpose: AiProviderPurpose::Embedding,
                    request_format: AiRequestFormat::Anthropic,
                    enabled: true,
                    default_model: "claude-3-7-sonnet".to_string(),
                    ..AiProviderConfig::default()
                },
                api_key: "secret".to_string(),
            },
            AiProviderPurpose::Embedding,
        )
        .expect_err("anthropic embeddings should fail");
        assert!(error.to_string().contains("Anthropic"));
    }

    #[test]
    fn validate_provider_rejects_disabled_wrong_purpose_and_missing_model() {
        let disabled = validate_provider(
            &AiProviderRuntime {
                config: AiProviderConfig {
                    id: "embed".to_string(),
                    name: "Disabled".to_string(),
                    purpose: AiProviderPurpose::Embedding,
                    request_format: AiRequestFormat::OpenAi,
                    enabled: false,
                    default_model: "text-embedding-3-small".to_string(),
                    ..AiProviderConfig::default()
                },
                api_key: "secret".to_string(),
            },
            AiProviderPurpose::Embedding,
        )
        .expect_err("disabled provider should fail");
        assert!(disabled.to_string().contains("Enable provider"));

        let wrong_purpose = validate_provider(&embedding_provider(), AiProviderPurpose::Llm)
            .expect_err("purpose mismatch should fail");
        assert!(wrong_purpose.to_string().contains("configured for"));

        let missing_model = validate_provider(
            &AiProviderRuntime {
                config: AiProviderConfig {
                    id: "llm".to_string(),
                    name: "Missing model".to_string(),
                    purpose: AiProviderPurpose::Llm,
                    request_format: AiRequestFormat::OpenAi,
                    enabled: true,
                    default_model: String::new(),
                    ..AiProviderConfig::default()
                },
                api_key: "secret".to_string(),
            },
            AiProviderPurpose::Llm,
        )
        .expect_err("missing model should fail");
        assert!(missing_model.to_string().contains("default model"));
    }

    #[test]
    fn ai_index_status_warns_when_archive_is_missing() {
        let paths = test_paths();
        let mut config = base_config();
        config.ai.mcp_enabled = true;
        config.ai.skill_enabled = true;
        config.ai.job_queue_paused = true;
        config.ai.job_queue_concurrency = 7;

        let status = ai_index_status(&paths, &config, None).expect("status");
        assert!(status.enabled);
        assert!(status.assistant_enabled);
        assert!(status.mcp_enabled);
        assert!(status.skill_enabled);
        assert_eq!(status.state, "blocked");
        assert_eq!(status.llm_provider_id.as_deref(), Some("llm"));
        assert_eq!(status.embedding_provider_id.as_deref(), Some("embed"));
        assert!(status.queue_paused);
        assert_eq!(status.queue_concurrency, 7);
        assert_eq!(status.queued_jobs, 0);
        assert_eq!(status.running_jobs, 0);
        assert_eq!(status.failed_jobs, 0);
        assert!(status.recent_jobs.is_empty());
        assert_eq!(
            status.warning.as_deref(),
            Some("Initialize the archive before using AI analysis features.")
        );
        assert!(!status.ready);
        assert_eq!(status.indexed_items, 0);
        assert!(status.last_indexed_at.is_none());
    }

    #[test]
    fn ai_index_status_reports_ready_with_existing_embeddings() {
        let (paths, config, connection) = prepared_archive();
        let provider = embedding_provider();
        seed_visit(&connection, 1, "chrome:Default", "https://example.com/ready", Some("Ready"), 1);
        seed_embedding(&connection, 1, &provider, "hash-ready");

        let status = ai_index_status(&paths, &config, None).expect("status");
        assert!(status.ready);
        assert_eq!(status.state, "ready");
        assert_eq!(status.indexed_items, 1);
        assert!(status.last_indexed_at.is_some());
    }

    #[test]
    fn ai_index_status_requires_initialized_archive_even_if_embeddings_exist() {
        let (paths, mut config, connection) = prepared_archive();
        let provider = embedding_provider();
        seed_visit(&connection, 1, "chrome:Default", "https://example.com/ready", Some("Ready"), 1);
        seed_embedding(&connection, 1, &provider, "hash-ready");
        config.initialized = false;

        let status = ai_index_status(&paths, &config, None).expect("status");
        assert!(!status.ready);
        assert_eq!(status.indexed_items, 0);
        assert!(status.last_indexed_at.is_none());
        assert_eq!(
            status.warning.as_deref(),
            Some("Initialize the archive before using AI analysis features.")
        );
    }

    #[test]
    fn ai_index_status_requires_indexed_rows_and_respects_warning_gate() {
        let (paths, config, _connection) = prepared_archive();

        let status = ai_index_status(&paths, &config, None).expect("status");
        assert!(!status.ready);
        assert_eq!(status.state, "empty");
        assert_eq!(status.indexed_items, 0);
        assert!(status.last_indexed_at.is_none());
        assert_eq!(
            status.warning.as_deref(),
            Some(
                "Run Build index after configuring an embedding provider to enable semantic search."
            )
        );

        let mut disabled = config.clone();
        disabled.ai.enabled = false;
        let disabled_status = ai_index_status(&paths, &disabled, None).expect("disabled status");
        assert!(!disabled_status.ready);
        assert_eq!(disabled_status.state, "disabled");
        assert_eq!(disabled_status.warning, None);
    }

    #[test]
    fn ai_index_status_treats_selected_model_without_embeddings_as_empty() {
        let (paths, mut config, connection) = prepared_archive();
        let provider = embedding_provider();
        seed_visit(&connection, 1, "chrome:Default", "https://example.com/ready", Some("Ready"), 1);
        seed_embedding(&connection, 1, &provider, "hash-ready");
        config.ai.embedding_providers[0].default_model = "text-embedding-3-large".to_string();

        let status = ai_index_status(&paths, &config, None).expect("status");
        assert!(!status.ready);
        assert_eq!(status.state, "empty");
        assert_eq!(status.indexed_items, 0);
        assert!(status.last_indexed_at.is_none());
    }

    #[test]
    fn ai_index_status_covers_degraded_queued_paused_rebuilding_and_failed_states() {
        let provider = embedding_provider();

        let (paths, mut degraded_config, _connection) = prepared_archive();
        degraded_config.ai.embedding_provider_id = None;
        let degraded = ai_index_status(&paths, &degraded_config, None).expect("degraded status");
        assert_eq!(degraded.state, "degraded");
        assert!(!degraded.ready);
        assert_eq!(degraded.indexed_items, 0);
        assert!(degraded.warning.is_some());

        let (paths, config, connection) = prepared_archive();
        ai_queue::enqueue_index_job(&connection, &AiIndexRequest::default(), false)
            .expect("enqueue queued job");
        let queued = ai_index_status(&paths, &config, None).expect("queued status");
        assert_eq!(queued.state, "queued");
        assert_eq!(queued.queued_jobs, 1);
        assert_eq!(queued.running_jobs, 0);
        assert_eq!(queued.failed_jobs, 0);
        assert_eq!(queued.recent_jobs.len(), 1);

        let (paths, mut paused_config, connection) = prepared_archive();
        paused_config.ai.job_queue_paused = true;
        ai_queue::enqueue_index_job(&connection, &AiIndexRequest::default(), true)
            .expect("enqueue paused job");
        let paused = ai_index_status(&paths, &paused_config, None).expect("paused status");
        assert_eq!(paused.state, "paused");
        assert!(paused.queue_paused);
        assert_eq!(paused.queued_jobs, 1);

        let (paths, mut paused_empty_config, _connection) = prepared_archive();
        paused_empty_config.ai.job_queue_paused = true;
        let paused_empty =
            ai_index_status(&paths, &paused_empty_config, None).expect("paused empty status");
        assert_eq!(paused_empty.state, "empty");

        let (paths, config, connection) = prepared_archive();
        let queued_job =
            ai_queue::enqueue_index_job(&connection, &AiIndexRequest::default(), false)
                .expect("enqueue job for rebuild");
        let running_job = ai_queue::claim_ai_job_by_id(&connection, queued_job.id, 300)
            .expect("claim job")
            .expect("running job");
        let rebuilding = ai_index_status(&paths, &config, None).expect("rebuilding status");
        assert_eq!(rebuilding.state, "rebuilding");
        assert_eq!(rebuilding.running_jobs, 1);
        assert!(rebuilding.recent_jobs.iter().any(|job| job.id == running_job.id));

        let (paths, config, connection) = prepared_archive();
        seed_failed_index_ledger(&connection, &provider, "embedding run failed");
        let failed = ai_index_status(&paths, &config, None).expect("failed status");
        assert_eq!(failed.state, "failed");
        assert_eq!(failed.warning.as_deref(), Some("embedding run failed"));
    }

    #[test]
    fn ai_queue_status_reflects_config_and_recent_jobs() {
        let paths = test_paths();
        let mut config = base_config();
        config.ai.job_queue_paused = true;
        config.ai.job_queue_concurrency = 3;
        let missing_status = ai_queue_status(&paths, &config, None).expect("missing queue status");
        assert!(missing_status.paused);
        assert_eq!(missing_status.concurrency, 3);
        assert_eq!(missing_status.queued, 0);
        assert!(missing_status.recent_jobs.is_empty());

        let (paths, mut config, connection) = prepared_archive();
        config.ai.job_queue_paused = false;
        config.ai.job_queue_concurrency = 2;
        ai_queue::enqueue_index_job(&connection, &AiIndexRequest::default(), false)
            .expect("enqueue queued job");
        ai_queue::enqueue_assistant_job(
            &connection,
            &AiAssistantRequest {
                question: "What changed?".to_string(),
                profile_id: None,
                domain: None,
            },
            "llm",
            Some("embed"),
            true,
        )
        .expect("enqueue paused assistant job");
        let status = ai_queue_status(&paths, &config, None).expect("queue status");
        assert!(!status.paused);
        assert_eq!(status.concurrency, 2);
        assert_eq!(status.queued, 2);
        assert_eq!(status.running, 0);
        assert_eq!(status.failed, 0);
        assert_eq!(status.recent_jobs.len(), 2);
        assert!(status.recent_jobs.iter().any(|job| job.state == "queued"));
        assert!(status.recent_jobs.iter().any(|job| job.state == "paused"));

        let (paths, mut uninitialized, connection) = prepared_archive();
        uninitialized.initialized = false;
        uninitialized.ai.job_queue_paused = true;
        ai_queue::enqueue_index_job(&connection, &AiIndexRequest::default(), false)
            .expect("enqueue job for uninitialized queue");
        let still_default =
            ai_queue_status(&paths, &uninitialized, None).expect("uninitialized queue status");
        assert!(still_default.paused);
        assert_eq!(still_default.queued, 0);
        assert!(still_default.recent_jobs.is_empty());
    }

    #[test]
    fn reconcile_ai_queue_controls_pauses_resumes_and_noops() {
        let (paths, config, connection) = prepared_archive();
        let queued_job =
            ai_queue::enqueue_index_job(&connection, &AiIndexRequest::default(), false)
                .expect("enqueue queued job");

        let mut paused = config.clone();
        paused.ai.job_queue_paused = true;
        reconcile_ai_queue_controls(&paths, &paused, &paused, None).expect("no-op reconcile");
        let unchanged =
            ai_queue::load_ai_queue_status(&connection, false, 1, 5).expect("load jobs");
        assert!(
            unchanged
                .recent_jobs
                .iter()
                .any(|job| job.id == queued_job.id && job.state == "queued")
        );

        reconcile_ai_queue_controls(&paths, &config, &paused, None).expect("pause reconcile");
        let paused_status =
            ai_queue::load_ai_queue_status(&connection, true, 1, 5).expect("load paused jobs");
        assert!(
            paused_status
                .recent_jobs
                .iter()
                .any(|job| job.id == queued_job.id && job.state == "paused")
        );

        reconcile_ai_queue_controls(&paths, &paused, &config, None).expect("resume reconcile");
        let resumed =
            ai_queue::load_ai_queue_status(&connection, false, 1, 5).expect("load resumed jobs");
        assert!(
            resumed.recent_jobs.iter().any(|job| job.id == queued_job.id && job.state == "queued")
        );

        let (paths, config, connection) = prepared_archive();
        let queued_job =
            ai_queue::enqueue_index_job(&connection, &AiIndexRequest::default(), false)
                .expect("enqueue queued job");
        let mut not_initialized = config.clone();
        not_initialized.initialized = false;
        let mut paused_next = config.clone();
        paused_next.initialized = false;
        paused_next.ai.job_queue_paused = true;
        reconcile_ai_queue_controls(&paths, &not_initialized, &paused_next, None)
            .expect("skip reconcile for uninitialized archive");
        let skipped =
            ai_queue::load_ai_queue_status(&connection, false, 1, 5).expect("load skipped jobs");
        assert!(
            skipped.recent_jobs.iter().any(|job| job.id == queued_job.id && job.state == "queued")
        );
    }

    #[test]
    fn provider_helpers_report_capabilities_and_failure_metadata() {
        let embedding_config = embedding_provider().config;
        let capabilities = provider_capabilities(&embedding_config);
        assert!(capabilities.supports_embeddings);
        assert!(!capabilities.supports_chat);
        assert!(!capabilities.supports_streaming);
        assert!(!capabilities.supports_tool_use);
        assert!(!capabilities.supports_structured_output);

        let failure = provider_connection_failure_report(&embedding_config, "unauthorized");
        assert_eq!(failure.provider_id, "embed");
        assert_eq!(failure.purpose, "embedding");
        assert_eq!(failure.model, "text-embedding-3-small");
        assert!(!failure.ok);
        assert_eq!(failure.latency_ms, 0);
        assert!(failure.capabilities.supports_embeddings);
        assert_eq!(failure.error_code.as_deref(), Some("secret-missing"));
        assert!(failure.action_hint.is_some());
        assert!(failure.retry_hint.is_some());
        assert!(failure.warnings.is_empty());
        assert_eq!(failure.message, "unauthorized");
    }

    #[test]
    fn test_provider_connection_reports_success_fields_and_anthropic_warning() {
        let runtime = Runtime::new().expect("runtime");
        let report = runtime
            .block_on(test_provider_connection(&llm_provider_with_format(
                AiRequestFormat::Anthropic,
            )))
            .expect("connection report");
        assert_eq!(report.provider_id, "llm");
        assert_eq!(report.purpose, "llm");
        assert_eq!(report.model, "gpt-4.1-mini");
        assert!(report.ok);
        assert!(report.latency_ms >= 1);
        assert!(report.capabilities.supports_chat);
        assert!(!report.capabilities.supports_embeddings);
        assert_eq!(report.warnings.len(), 1);
        assert!(report.warnings[0].contains("chat-only"));
        assert!(report.message.contains("successfully"));

        let openai = runtime
            .block_on(test_provider_connection(&llm_provider_with_format(AiRequestFormat::OpenAi)))
            .expect("openai connection report");
        assert!(openai.ok);
        assert!(openai.warnings.is_empty());
    }

    #[test]
    fn ensure_ai_schema_adds_tables() {
        let paths = test_paths();
        let config = base_config();
        ensure_archive_initialized(&paths, &config, None).expect("init archive");
        let connection = open_archive_connection(&paths, &config, None).expect("open");
        ensure_ai_schema(&connection).expect("schema");
        let count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'ai_embeddings'",
                [],
                |row: &Row<'_>| row.get(0),
        )
        .expect("count");
        assert_eq!(count, 1);
    }

    #[test]
    fn build_assistant_preamble_covers_empty_and_seeded_context() {
        let config = base_config();
        let empty = build_assistant_preamble(&config, &AiSearchResponse::default());
        assert!(empty.contains("No indexed evidence was found"));

        let with_context = build_assistant_preamble(
            &config,
            &AiSearchResponse {
                total: 1,
                provider_id: "embed".to_string(),
                model: "text-embedding-3-small".to_string(),
                items: vec![AiSearchEntry {
                    history_id: 1,
                    profile_id: "chrome:Default".to_string(),
                    url: "https://example.com/docs".to_string(),
                    title: Some("Docs".to_string()),
                    domain: "example.com".to_string(),
                    visited_at: "2026-04-04T00:00:00Z".to_string(),
                    score: 0.91,
                    match_reason: "Semantic match".to_string(),
                }],
                notes: Vec::new(),
                next_cursor: None,
            },
        );
        assert!(with_context.contains("Semantic match"));
        assert!(with_context.contains("https://example.com/docs"));
    }

    #[test]
    fn collect_visits_to_index_skips_already_indexed_rows_and_cleanup_removes_stale_rows() {
        let (_paths, _config, connection) = prepared_archive();
        let provider = embedding_provider();
        seed_visit(&connection, 1, "chrome:Default", "https://example.com/docs", Some("Docs"), 1);
        seed_visit(&connection, 2, "chrome:Default", "https://example.com/blog", Some("Blog"), 2);

        let visit_time = connection
            .query_row("SELECT visit_time FROM visit_events WHERE id = 1", [], |row| {
                row.get::<_, i64>(0)
            })
            .expect("load visit time");
        let first_content = build_embedding_content(
            "chrome:Default",
            "https://example.com/docs",
            Some("Docs"),
            &crate::utils::chrome_time_to_rfc3339(visit_time),
        );
        seed_embedding(&connection, 1, &provider, &sha256_hex(first_content.as_bytes()));
        seed_embedding(&connection, 999, &provider, "orphan-hash");

        let candidates =
            collect_visits_to_index(&connection, &provider, Some(10)).expect("collect");
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].history_id, 2);

        let removed = cleanup_stale_embeddings(&connection, &provider).expect("cleanup");
        assert_eq!(removed, 1);
    }

    #[test]
    fn cleanup_stale_embeddings_returns_zero_when_nothing_is_removed() {
        let (_paths, _config, connection) = prepared_archive();
        let removed =
            cleanup_stale_embeddings(&connection, &embedding_provider()).expect("cleanup");
        assert_eq!(removed, 0);
    }

    #[test]
    fn search_history_internal_requires_query_and_supports_lexical_fallback() {
        let runtime = Runtime::new().expect("runtime");
        let (paths, config, connection) = prepared_archive();
        seed_visit(&connection, 1, "chrome:Default", "https://example.com/docs", Some("Docs"), 1);

        let empty_error = runtime
            .block_on(search_history_internal(
                &paths,
                &config,
                None,
                None,
                &AiSearchRequest {
                    query: "   ".to_string(),
                    profile_id: None,
                    domain: None,
                    limit: Some(5),
                    cursor: None,
                },
            ))
            .expect_err("empty query should fail");
        assert!(empty_error.to_string().contains("Enter a question"));

        let response = runtime
            .block_on(search_history_internal(
                &paths,
                &config,
                None,
                None,
                &AiSearchRequest {
                    query: "docs".to_string(),
                    profile_id: None,
                    domain: None,
                    limit: Some(5),
                    cursor: None,
                },
            ))
            .expect("lexical search");
        assert_eq!(response.total, 1);
        assert_eq!(response.provider_id, "lexical-fallback");
        assert_eq!(response.items[0].score, 0.6);
        assert!(response.notes.iter().any(|note| note.contains("lexical retrieval")));
    }

    #[test]
    fn semantic_search_history_uses_public_wrapper_for_search_results() {
        let runtime = Runtime::new().expect("runtime");
        let (paths, config, connection) = prepared_archive();
        seed_visit(&connection, 1, "chrome:Default", "https://example.com/docs", Some("Docs"), 1);

        let response = runtime
            .block_on(semantic_search_history(
                &paths,
                &config,
                None,
                None,
                &AiSearchRequest {
                    query: "docs".to_string(),
                    profile_id: None,
                    domain: None,
                    limit: Some(5),
                    cursor: None,
                },
            ))
            .expect("public search wrapper");
        assert_eq!(response.total, 1);
        assert_eq!(response.provider_id, "lexical-fallback");
        assert!(response.items.iter().any(|item| item.url.contains("/docs")));
    }

    #[test]
    fn build_ai_index_returns_without_network_when_no_candidates_exist() {
        let runtime = Runtime::new().expect("runtime");
        let (paths, config, connection) = prepared_archive();
        drop(connection);
        let report = runtime
            .block_on(build_ai_index(
                &paths,
                &config,
                None,
                &embedding_provider(),
                &AiIndexRequest {
                    provider_id: None,
                    full_rebuild: true,
                    clear_only: false,
                    limit: Some(5),
                },
            ))
            .expect("empty build");
        assert_eq!(report.indexed_items, 0);
        assert!(report.notes.iter().any(|note| note.contains("No new or changed history rows")));
    }

    #[test]
    fn answer_history_question_checks_feature_gates_before_network() {
        let runtime = Runtime::new().expect("runtime");
        let (paths, mut config, connection) = prepared_archive();
        seed_visit(&connection, 1, "chrome:Default", "https://example.com/docs", Some("Docs"), 1);
        config.ai.assistant_enabled = false;
        let error = runtime
            .block_on(answer_history_question(
                &paths,
                &config,
                None,
                &llm_provider(),
                None,
                &AiAssistantRequest {
                    question: "What did I read?".to_string(),
                    profile_id: None,
                    domain: None,
                },
            ))
            .expect_err("assistant should require feature gate");
        assert!(error.to_string().contains("assistant"));
    }

    #[test]
    fn search_history_tool_definition_and_call_collect_citations() {
        let runtime = Runtime::new().expect("runtime");
        let (paths, config, connection) = prepared_archive();
        seed_visit(
            &connection,
            1,
            "chrome:Default",
            "https://example.com/history",
            Some("History"),
            1,
        );
        let citations = Arc::new(Mutex::new(Vec::new()));
        let tool = SearchHistoryTool {
            context: SearchContext {
                paths,
                config,
                database_key: None,
                embedding_provider: None,
                default_profile_id: None,
                default_domain: None,
                default_limit: 3,
                citations: Arc::clone(&citations),
            },
        };

        let definition = runtime.block_on(rig::tool::Tool::definition(&tool, String::new()));
        assert_eq!(definition.name, "search_history");

        let output = runtime
            .block_on(rig::tool::Tool::call(
                &tool,
                SearchHistoryArgs {
                    query: "history".to_string(),
                    profile_id: None,
                    domain: None,
                    limit: Some(3),
                },
            ))
            .expect("tool call");
        assert_eq!(output.items.len(), 1);
        let stored = runtime.block_on(async { citations.lock().await.clone() });
        assert_eq!(stored.len(), 1);
        assert_eq!(stored[0].history_id, 1);
    }

    #[test]
    fn ai_status_and_search_cover_non_ready_and_semantic_empty_branches() {
        let runtime = Runtime::new().expect("runtime");
        let mut disabled = base_config();
        disabled.ai.enabled = false;
        let missing_paths = test_paths();
        let disabled_status =
            ai_index_status(&missing_paths, &disabled, None).expect("disabled status");
        assert!(disabled_status.warning.is_none());

        let (paths, config, connection) = prepared_archive();
        seed_visit(&connection, 1, "chrome:Default", "https://example.com/docs", Some("Docs"), 1);

        let mut no_provider = config.clone();
        no_provider.ai.embedding_provider_id = None;
        let no_provider_status = ai_index_status(&paths, &no_provider, None).expect("no provider");
        assert_eq!(no_provider_status.indexed_items, 0);
        assert!(no_provider_status.warning.is_some());

        let collected =
            collect_visits_to_index(&connection, &embedding_provider(), None).expect("collect all");
        assert_eq!(collected.len(), 1);

        let response = runtime
            .block_on(search_history_internal(
                &paths,
                &config,
                None,
                Some(&embedding_provider()),
                &AiSearchRequest {
                    query: "docs".to_string(),
                    profile_id: None,
                    domain: None,
                    limit: Some(5),
                    cursor: None,
                },
            ))
            .expect("semantic empty fallback");
        assert_eq!(response.provider_id, "embed");
        assert!(
            response
                .notes
                .iter()
                .any(|note| note.contains("No indexed semantic matches were found"))
        );
    }

    #[test]
    fn build_index_search_and_assistant_cover_semantic_and_persistence_flows() {
        let runtime = Runtime::new().expect("runtime");
        let (paths, config, connection) = prepared_archive();
        let embedding = embedding_provider();
        seed_visit(&connection, 1, "chrome:Default", "https://example.com/docs", Some("Docs"), 1);
        seed_visit(&connection, 2, "chrome:Default", "https://example.com/blog", Some("Blog"), 2);
        seed_embedding(&connection, 1, &embedding, "stale-hash");
        drop(connection);

        let report = runtime
            .block_on(build_ai_index(&paths, &config, None, &embedding, &AiIndexRequest::default()))
            .expect("build index");
        assert_eq!(report.indexed_items, 1);
        assert_eq!(report.updated_items, 1);
        assert!(report.notes[0].contains("Indexed 2 history rows"));

        let rebuilt = runtime
            .block_on(build_ai_index(
                &paths,
                &config,
                None,
                &embedding,
                &AiIndexRequest {
                    provider_id: None,
                    full_rebuild: true,
                    clear_only: false,
                    limit: Some(1),
                },
            ))
            .expect("full rebuild");
        assert_eq!(rebuilt.indexed_items, 1);

        let search = runtime
            .block_on(search_history_internal(
                &paths,
                &config,
                None,
                Some(&embedding),
                &AiSearchRequest {
                    query: "docs".to_string(),
                    profile_id: None,
                    domain: None,
                    limit: Some(5),
                    cursor: None,
                },
            ))
            .expect("semantic search");
        assert_eq!(search.provider_id, "embed");
        assert!(search.items.iter().any(|item| item.match_reason.contains("Semantic")));

        let assistant = runtime
            .block_on(answer_history_question(
                &paths,
                &config,
                None,
                &llm_provider(),
                Some(&embedding),
                &AiAssistantRequest {
                    question: "Summarize my docs reading".to_string(),
                    profile_id: None,
                    domain: None,
                },
            ))
            .expect("assistant answer");
        assert!(assistant.answer.contains("Summarize my docs reading"));
        assert_eq!(assistant.provider_id, "llm");
        assert_eq!(assistant.embedding_provider_id, "embed");
        assert!(!assistant.citations.is_empty());

        let connection = open_archive_connection(&paths, &config, None).expect("open archive");
        let runs: i64 = connection
            .query_row("SELECT COUNT(*) FROM ai_assistant_runs", [], |row: &Row<'_>| row.get(0))
            .expect("assistant run count");
        assert_eq!(runs, 1);
    }

    #[test]
    fn semantic_matches_orders_results_by_score() {
        let runtime = Runtime::new().expect("runtime");
        let (paths, config, connection) = prepared_archive();
        let embedding = embedding_provider();
        seed_visit(&connection, 1, "chrome:Default", "https://example.com/docs", Some("Docs"), 1);
        seed_visit(&connection, 2, "chrome:Default", "https://example.com/hello", Some("Hello"), 2);
        seed_embedding_with_vector(&connection, 1, &embedding, &[0.0, 1.0, 0.0, 0.0]);
        seed_embedding_with_vector(&connection, 2, &embedding, &[0.25, 0.25, 0.25, 0.25]);

        let matches = runtime
            .block_on(semantic_matches(
                &paths,
                &config,
                None,
                &embedding,
                &AiSearchRequest {
                    query: "hello".to_string(),
                    profile_id: None,
                    domain: None,
                    limit: Some(5),
                    cursor: None,
                },
            ))
            .expect("semantic matches");
        assert_eq!(matches.items.len(), 2);
        assert_eq!(matches.items[0].history_id, 2);
        assert!(matches.items[0].score >= matches.items[1].score);
    }

    #[test]
    fn search_history_internal_blends_semantic_and_lexical_scores() {
        let runtime = Runtime::new().expect("runtime");
        let (paths, config, connection) = prepared_archive();
        let embedding = embedding_provider();
        seed_visit(&connection, 1, "chrome:Default", "https://example.com/docs", Some("Docs"), 1);
        let query_vector = runtime.block_on(embed_query(&embedding, "docs")).expect("query vector");
        seed_embedding_with_vector(&connection, 1, &embedding, &query_vector);

        let search = runtime
            .block_on(search_history_internal(
                &paths,
                &config,
                None,
                Some(&embedding),
                &AiSearchRequest {
                    query: "docs".to_string(),
                    profile_id: None,
                    domain: None,
                    limit: Some(5),
                    cursor: None,
                },
            ))
            .expect("semantic + lexical search");

        assert_eq!(search.items.len(), 1);
        assert_eq!(search.items[0].history_id, 1);
        assert_eq!(search.items[0].match_reason, "Semantic + lexical match");
        assert!((search.items[0].score - 1.08).abs() < 1e-6);
    }

    #[test]
    fn lexical_scoring_helpers_return_expected_values() {
        assert!((lexical_score(0, 5) - 0.6).abs() < 1e-6);
        assert!((lexical_score(4, 5) - 0.456).abs() < 1e-6);
        assert!((lexical_boost(0, 5) - 0.08).abs() < 1e-6);
        assert!((lexical_boost(4, 5) - 0.016).abs() < 1e-6);
    }

    #[test]
    fn stubbed_llm_and_embedding_helpers_cover_supported_formats() {
        let runtime = Runtime::new().expect("runtime");
        let openai_answer = runtime
            .block_on(run_llm_agent(
                &llm_provider_with_format(AiRequestFormat::OpenAi),
                "system preamble",
                Vec::new(),
                "hello",
            ))
            .expect("openai answer");
        assert!(openai_answer.contains("openai"));

        let ollama_answer = runtime
            .block_on(run_llm_agent(
                &llm_provider_with_format(AiRequestFormat::Ollama),
                "system preamble",
                Vec::new(),
                "hello",
            ))
            .expect("ollama answer");
        assert!(ollama_answer.contains("ollama"));

        let lmstudio_answer = runtime
            .block_on(run_llm_agent(
                &llm_provider_with_format(AiRequestFormat::LmStudio),
                "system preamble",
                Vec::new(),
                "hello",
            ))
            .expect("lmstudio answer");
        assert!(lmstudio_answer.contains("lmstudio"));

        let google_answer = runtime
            .block_on(run_llm_agent(
                &llm_provider_with_format(AiRequestFormat::Google),
                "system preamble",
                Vec::new(),
                "hello",
            ))
            .expect("google answer");
        assert!(google_answer.contains("google"));

        let anthropic_answer = runtime
            .block_on(run_llm_agent(
                &llm_provider_with_format(AiRequestFormat::Anthropic),
                "system preamble",
                Vec::new(),
                "hello",
            ))
            .expect("anthropic answer");
        assert!(anthropic_answer.contains("anthropic"));

        let google_embedding_provider = AiProviderRuntime {
            config: AiProviderConfig {
                id: "google-embed".to_string(),
                name: "Google embeddings".to_string(),
                purpose: AiProviderPurpose::Embedding,
                request_format: AiRequestFormat::Google,
                enabled: true,
                default_model: "text-embedding-004".to_string(),
                dimensions: Some(4),
                ..AiProviderConfig::default()
            },
            api_key: "secret".to_string(),
        };
        let embedding = runtime
            .block_on(embed_query(&google_embedding_provider, "hello"))
            .expect("google embedding");
        assert_eq!(embedding.len(), 4);
        assert_eq!(
            embedding,
            expected_stub_embedding(&google_embedding_provider.config.id, "hello", 4)
        );

        let anthropic_error = runtime
            .block_on(embed_query(
                &AiProviderRuntime {
                    config: AiProviderConfig {
                        id: "anthropic-embed".to_string(),
                        name: "Anthropic embeddings".to_string(),
                        purpose: AiProviderPurpose::Embedding,
                        request_format: AiRequestFormat::Anthropic,
                        enabled: true,
                        default_model: "claude-embedding".to_string(),
                        ..AiProviderConfig::default()
                    },
                    api_key: "secret".to_string(),
                },
                "hello",
            ))
            .expect_err("anthropic embeddings should fail");
        assert!(anthropic_error.to_string().contains("does not support embeddings"));

        let openai_embedding =
            runtime.block_on(embed_query(&embedding_provider(), "docs")).expect("openai embedding");
        assert_eq!(openai_embedding.len(), 3);
        assert_eq!(openai_embedding, expected_stub_embedding("embed", "docs", 3));
        assert_ne!(openai_embedding, embedding);
    }
}
