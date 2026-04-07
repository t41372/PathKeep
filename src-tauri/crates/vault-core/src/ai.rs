use crate::{
    archive::{create_schema, list_history, open_archive_connection},
    config::ProjectPaths,
    insights::preferred_embedding_content,
    models::{
        AiAssistantRequest, AiAssistantResponse, AiCitation, AiIndexReport, AiIndexRequest,
        AiIndexStatus, AiProviderConfig, AiProviderPurpose, AiRequestFormat, AiSearchEntry,
        AiSearchRequest, AiSearchResponse, AppConfig, HistoryEntry, HistoryQuery,
    },
    utils::{now_rfc3339, sha256_hex, url_domain},
};
use anyhow::{Context, Result};
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
use std::{cmp::Ordering, collections::HashMap, sync::Arc};
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
    pub manual_steps: Vec<String>,
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
      question TEXT NOT NULL,
      answer TEXT NOT NULL,
      provider_id TEXT NOT NULL,
      embedding_provider_id TEXT NOT NULL,
      citations_json TEXT NOT NULL,
      notes_json TEXT NOT NULL,
      created_at TEXT NOT NULL
    );
    CREATE INDEX IF NOT EXISTS idx_ai_embeddings_provider_model
      ON ai_embeddings(provider_id, model);
    CREATE INDEX IF NOT EXISTS idx_ai_embeddings_history_id
      ON ai_embeddings(history_id);
"#;

const SEMANTIC_MATCHES_SQL: &str = r#"
    SELECT history_id, profile_id, url, title, domain, visited_at, embedding_json
    FROM ai_embeddings
    WHERE provider_id = ?1
      AND model = ?2
      AND (?3 IS NULL OR profile_id = ?3)
      AND (?4 IS NULL OR domain LIKE '%' || ?4 || '%')
"#;

const CLEAR_PROVIDER_EMBEDDINGS_SQL: &str =
    "DELETE FROM ai_embeddings WHERE provider_id = ?1 AND model = ?2";
const DELETE_STALE_EMBEDDINGS_SQL: &str = "DELETE FROM ai_embeddings WHERE provider_id = ?1 AND model = ?2 AND history_id NOT IN (SELECT id FROM visit_events)";
const UPSERT_EMBEDDING_SQL: &str = "INSERT OR REPLACE INTO ai_embeddings (history_id, profile_id, url, title, domain, visited_at, content, content_hash, provider_id, model, embedding_json, dimensions, indexed_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)";
const INSERT_ASSISTANT_RUN_SQL: &str = "INSERT INTO ai_assistant_runs (question, answer, provider_id, embedding_provider_id, citations_json, notes_json, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)";

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
    Ok(())
}

pub fn ai_index_status(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
) -> Result<AiIndexStatus> {
    if !config.initialized || !paths.archive_database_path.exists() {
        return Ok(AiIndexStatus {
            enabled: config.ai.enabled,
            assistant_enabled: config.ai.assistant_enabled,
            mcp_enabled: config.ai.mcp_enabled,
            skill_enabled: config.ai.skill_enabled,
            llm_provider_id: config.ai.llm_provider_id.clone(),
            embedding_provider_id: config.ai.embedding_provider_id.clone(),
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

    let provider_id = config.ai.embedding_provider_id.clone();
    let indexed_items = if let Some(provider_id) = provider_id.as_deref() {
        provider_embedding_count(&connection, provider_id)?
    } else {
        0
    };
    let last_indexed_at = if let Some(provider_id) = provider_id.as_deref() {
        connection
            .query_row(
                "SELECT indexed_at
                 FROM ai_embeddings
                 WHERE provider_id = ?1
                 ORDER BY indexed_at DESC
                 LIMIT 1",
                [provider_id],
                |row: &Row<'_>| row.get(0),
            )
            .optional()?
    } else {
        None
    };
    let ready = indexed_items > 0 && config.ai.embedding_provider_id.is_some();
    Ok(AiIndexStatus {
        enabled: config.ai.enabled,
        assistant_enabled: config.ai.assistant_enabled,
        mcp_enabled: config.ai.mcp_enabled,
        skill_enabled: config.ai.skill_enabled,
        ready,
        indexed_items: indexed_items as usize,
        last_indexed_at,
        llm_provider_id: config.ai.llm_provider_id.clone(),
        embedding_provider_id: config.ai.embedding_provider_id.clone(),
        warning: if config.ai.enabled && !ready {
            Some("Run Build index after configuring an embedding provider to enable semantic search.".to_string())
        } else {
            None
        },
    })
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

    if request.full_rebuild {
        clear_provider_embeddings(&connection, provider)?;
    }

    let removed_items = cleanup_stale_embeddings(&connection, provider)?;
    let candidates = collect_visits_to_index(&connection, provider, request.limit)?;
    if candidates.is_empty() {
        return Ok(AiIndexReport {
            provider_id: provider.config.id.clone(),
            model: provider.config.default_model.clone(),
            indexed_items: 0,
            updated_items: 0,
            skipped_items: 0,
            removed_items,
            last_indexed_at: now_rfc3339(),
            notes: vec!["No new or changed history rows required indexing.".to_string()],
        });
    }

    let timestamp = now_rfc3339();
    let mut indexed_items = 0usize;
    let mut updated_items = 0usize;

    for visit in &candidates {
        let had_prior_index = connection
            .query_row(
                "SELECT id
                 FROM ai_embeddings
                 WHERE history_id = ?1
                   AND provider_id = ?2
                   AND model = ?3
                 LIMIT 1",
                params![visit.history_id, provider.config.id, provider.config.default_model],
                |row: &Row<'_>| row.get::<_, i64>(0),
            )
            .optional()?
            .is_some();
        let vector = embed_query(provider, &visit.content).await?;
        upsert_embedding(&connection, provider, visit, &vector, &timestamp)?;
        if had_prior_index {
            updated_items += 1;
        } else {
            indexed_items += 1;
        }
    }

    Ok(AiIndexReport {
        provider_id: provider.config.id.clone(),
        model: provider.config.default_model.clone(),
        indexed_items,
        updated_items,
        skipped_items: 0,
        removed_items,
        last_indexed_at: timestamp,
        notes: vec![format!(
            "Indexed {} history rows with {}.",
            candidates.len(),
            provider.config.name
        )],
    })
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

    let retrieval_request = AiSearchRequest {
        query: request.question.clone(),
        profile_id: request.profile_id.clone(),
        domain: request.domain.clone(),
        limit: Some(config.ai.retrieval_top_k.max(1)),
    };
    let search_response =
        search_history_internal(paths, config, key, embedding_provider, &retrieval_request).await?;
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
    let tools: Vec<Box<dyn ToolDyn>> = vec![Box::new(SearchHistoryTool { context: tool_context })];
    let preamble = build_assistant_preamble(config, &search_response);
    let answer = run_llm_agent(llm_provider, &preamble, tools, &request.question).await?;

    let mut final_citations = citations.lock().await.clone();
    final_citations.sort_by_key(|item| item.history_id);
    final_citations.dedup_by_key(|item| item.history_id);

    let connection = open_archive_connection(paths, config, key)?;
    create_schema(&connection)?;
    ensure_ai_schema(&connection)?;
    let embedding_provider_id = embedding_provider
        .map(|provider| provider.config.id.clone())
        .unwrap_or_else(|| "lexical-fallback".to_string());
    #[rustfmt::skip]
    record_assistant_run(&connection, request, &answer, &llm_provider.config.id, &embedding_provider_id, &final_citations, &search_response.notes)?;

    Ok(AiAssistantResponse {
        answer,
        provider_id: llm_provider.config.id.clone(),
        embedding_provider_id,
        citations: final_citations,
        notes: search_response.notes,
    })
}

pub fn preview_ai_integrations(
    paths: &ProjectPaths,
    config: &AppConfig,
) -> Result<AiIntegrationPreview> {
    let executable = std::env::current_exe()
        .ok()
        .map(|path| path.display().to_string())
        .unwrap_or_else(|| "<path-to-browser-history-backup>".to_string());
    let mcp_command = format!("{executable} --worker mcp-server");
    let codex_skill = "# Browser History Backup Search\n\nUse this skill when the user wants evidence from Browser History Backup.\n\n1. Make sure the local MCP server is configured in your Codex MCP settings.\n2. Use the `search_history` tool to find visits relevant to the question.\n3. Quote the visit date, URL, and profile when answering.\n\nIf the archive is encrypted, remind the user that the database key must be available in the system keyring before MCP queries can work.\n".to_string();
    let mcp_config = json!({
        "mcpServers": {
            "browser-history-backup": {
                "command": executable,
                "args": ["--worker", "mcp-server"]
            }
        }
    });
    Ok(AiIntegrationPreview {
        mcp_command,
        manual_steps: vec![
            "Enable MCP or Skill integration in Settings first. Both are off by default.".to_string(),
            "Store the database key in the native keyring if the archive is encrypted, so background and MCP lookups can unlock the archive.".to_string(),
            "Copy the generated MCP JSON into your local MCP client configuration and restart that client.".to_string(),
            "Copy the generated skill markdown into your local skills directory if you want a reusable history-research workflow.".to_string(),
        ],
        generated_files: vec![
            crate::models::GeneratedFile {
                relative_path: "integrations/browser-history-backup-mcp.json".to_string(),
                absolute_path: Some(
                    paths.app_root
                        .join("integrations/browser-history-backup-mcp.json")
                        .display()
                        .to_string(),
                ),
                purpose: "Local MCP client configuration snippet for Browser History Backup.".to_string(),
                contents: serde_json::to_string_pretty(&mcp_config)?,
            },
            crate::models::GeneratedFile {
                relative_path: "integrations/codex-browser-history-skill/SKILL.md".to_string(),
                absolute_path: Some(
                    paths.app_root
                        .join("integrations/codex-browser-history-skill/SKILL.md")
                        .display()
                        .to_string(),
                ),
                purpose: "Codex skill starter that teaches an external assistant how to query Browser History Backup through MCP.".to_string(),
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
        "{system_prompt}\n\nYou are working inside Browser History Backup. Always ground answers in the history evidence below or by calling the search_history tool. Cite the visit date, profile, and URL you relied on. If the evidence is incomplete, say so.\n\nInitial evidence:\n{context}",
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
        if semantic.is_empty() {
            notes.push(
                "No indexed semantic matches were found; showing lexical results only.".to_string(),
            );
        }
        for (index, item) in semantic.into_iter().take(limit).enumerate() {
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
    items.truncate(limit);

    Ok(AiSearchResponse { total: items.len(), provider_id, model, items, notes })
}

async fn semantic_matches(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    provider: &AiProviderRuntime,
    request: &AiSearchRequest,
) -> Result<Vec<StoredEmbedding>> {
    let connection = open_archive_connection(paths, config, key)?;
    create_schema(&connection)?;
    ensure_ai_schema(&connection)?;
    let query_vector = embed_query(provider, request.query.trim()).await?;
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
    Ok(scored)
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

fn provider_embedding_count(connection: &Connection, provider_id: &str) -> Result<i64> {
    #[rustfmt::skip]
    let count = connection.query_row(
        "SELECT COUNT(*) FROM ai_embeddings WHERE provider_id = ?1",
        [provider_id],
        |row: &Row<'_>| row.get::<_, i64>(0),
    )?;
    Ok(count)
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
    request: &AiAssistantRequest,
    answer: &str,
    llm_provider_id: &str,
    embedding_provider_id: &str,
    citations: &[AiCitation],
    notes: &[String],
) -> Result<()> {
    #[rustfmt::skip]
    connection.execute(INSERT_ASSISTANT_RUN_SQL, params![request.question, answer, llm_provider_id, embedding_provider_id, serde_json::to_string(citations)?, serde_json::to_string(notes)?, now_rfc3339()])?;
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
            domain: request.domain.clone(),
            limit: Some(request.limit.unwrap_or(12).max(1)),
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
            "browser-history-backup-ai-test-{}-{}-{}",
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

        let status = ai_index_status(&paths, &config, None).expect("status");
        assert!(status.enabled);
        assert!(status.assistant_enabled);
        assert!(status.mcp_enabled);
        assert!(status.skill_enabled);
        assert_eq!(status.llm_provider_id.as_deref(), Some("llm"));
        assert_eq!(status.embedding_provider_id.as_deref(), Some("embed"));
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
        assert_eq!(disabled_status.warning, None);
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

        let first_content = build_embedding_content(
            "chrome:Default",
            "https://example.com/docs",
            Some("Docs"),
            &crate::utils::chrome_time_to_rfc3339(1),
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
                &AiIndexRequest { provider_id: None, full_rebuild: true, limit: Some(5) },
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
                &AiIndexRequest { provider_id: None, full_rebuild: true, limit: Some(1) },
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
                },
            ))
            .expect("semantic matches");
        assert_eq!(matches.len(), 2);
        assert_eq!(matches[0].history_id, 2);
        assert!(matches[0].score >= matches[1].score);
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
