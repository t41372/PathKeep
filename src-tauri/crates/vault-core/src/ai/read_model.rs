//! AI read models and integration preview surface.
//!
//! This module collects the non-generation-facing AI functions:
//! schema bootstrapping, readiness/status, queue controls, connection reports,
//! persisted assistant run loading, and manual integration preview.
//!
//! Keeping these together makes the public AI surface easier to scan without
//! digging through embedding/search/assistant execution internals.

use super::*;

/// Ensures the AI compatibility tables exist in the rebuildable intelligence plane.
pub fn ensure_ai_schema(connection: &Connection) -> Result<()> {
    connection.execute_batch(AI_SCHEMA_SQL)?;
    migrate_ai_embeddings_to_blob(connection)?;
    ensure_ai_assistant_run_columns(connection)?;
    Ok(())
}

fn migrate_ai_embeddings_to_blob(connection: &Connection) -> Result<()> {
    let mut statement = connection.prepare("PRAGMA table_info(ai_embeddings)")?;
    let columns = statement
        .query_map([], |row| row.get::<_, String>(1))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let has_blob = columns.iter().any(|column| column == "embedding_blob");
    let has_json = columns.iter().any(|column| column == "embedding_json");
    if has_blob && !has_json {
        return Ok(());
    }
    if !has_json {
        anyhow::bail!("ai_embeddings is missing both embedding_blob and embedding_json");
    }

    #[derive(Debug)]
    struct LegacyEmbeddingRow {
        id: i64,
        history_id: i64,
        profile_id: String,
        url: String,
        title: Option<String>,
        domain: String,
        visited_at: String,
        content: String,
        content_hash: String,
        provider_id: String,
        model: String,
        embedding_blob: Option<Vec<u8>>,
        embedding_json: Option<String>,
        dimensions: i64,
        indexed_at: String,
    }

    let legacy_sql = if has_blob {
        "SELECT id, history_id, profile_id, url, title, domain, visited_at, content, content_hash,
                provider_id, model, embedding_blob, embedding_json, dimensions, indexed_at
         FROM ai_embeddings
         ORDER BY id ASC"
    } else {
        "SELECT id, history_id, profile_id, url, title, domain, visited_at, content, content_hash,
                provider_id, model, NULL as embedding_blob, embedding_json, dimensions, indexed_at
         FROM ai_embeddings
         ORDER BY id ASC"
    };
    let mut legacy_statement = connection.prepare(legacy_sql)?;
    let legacy_rows = legacy_statement
        .query_map([], |row: &Row<'_>| {
            Ok(LegacyEmbeddingRow {
                id: row.get(0)?,
                history_id: row.get(1)?,
                profile_id: row.get(2)?,
                url: row.get(3)?,
                title: row.get(4)?,
                domain: row.get(5)?,
                visited_at: row.get(6)?,
                content: row.get(7)?,
                content_hash: row.get(8)?,
                provider_id: row.get(9)?,
                model: row.get(10)?,
                embedding_blob: row.get(11)?,
                embedding_json: row.get(12)?,
                dimensions: row.get(13)?,
                indexed_at: row.get(14)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    connection.execute_batch("ALTER TABLE ai_embeddings RENAME TO ai_embeddings_legacy_v1;")?;
    connection.execute_batch(AI_EMBEDDINGS_TABLE_SQL)?;
    let mut insert = connection.prepare(
        "INSERT INTO ai_embeddings
         (id, history_id, profile_id, url, title, domain, visited_at, content, content_hash,
          provider_id, model, embedding_blob, dimensions, indexed_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
    )?;
    for row in legacy_rows {
        let embedding_blob = match (row.embedding_blob, row.embedding_json) {
            (Some(blob), _) if !blob.is_empty() => blob,
            (_, Some(json)) => embedding_blob_from_json(&json)?,
            _ => anyhow::bail!("ai_embeddings row {} is missing vector payload", row.id),
        };
        insert.execute(rusqlite::params![
            row.id,
            row.history_id,
            row.profile_id,
            row.url,
            row.title,
            row.domain,
            row.visited_at,
            row.content,
            row.content_hash,
            row.provider_id,
            row.model,
            embedding_blob,
            row.dimensions,
            row.indexed_at,
        ])?;
    }
    drop(insert);
    connection.execute_batch("DROP TABLE ai_embeddings_legacy_v1;")?;
    Ok(())
}

/// Reports semantic-index readiness, queue state, and storage/readiness notes.
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

    let connection = open_intelligence_connection(paths, config, key)?;
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

/// Loads the persisted AI queue read model.
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

    let connection = open_intelligence_connection(paths, config, key)?;
    ensure_ai_schema(&connection)?;
    ai_queue::load_ai_queue_status(
        &connection,
        config.ai.job_queue_paused,
        config.ai.job_queue_concurrency,
        AI_QUEUE_RECENT_LIMIT,
    )
}

/// Synchronizes persisted queue pause/resume controls with updated Settings.
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

    let connection = open_intelligence_connection(paths, next_config, key)?;
    ensure_ai_schema(&connection)?;
    ai_queue::ensure_ai_queue_schema(&connection)?;

    if next_config.ai.job_queue_paused {
        ai_queue::pause_queued_jobs(&connection)?;
    } else {
        ai_queue::resume_paused_jobs(&connection)?;
    }

    Ok(())
}

/// Describes what one configured AI provider can do inside PathKeep.
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

/// Shapes a provider connection failure into a user-facing report.
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

/// Loads one persisted assistant run into the public response shape.
pub fn load_assistant_run_response(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    run_id: i64,
) -> Result<AiAssistantResponse> {
    let connection = open_intelligence_connection(paths, config, key)?;
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

/// Builds the manual MCP/skill integration preview shown in Settings.
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
            "Every MCP request is recorded as a dedicated `mcp_query` run in the unified archive ledger.".to_string(),
            "Assistant answers keep their provider snapshot, retrieval provider, and citations inside `ai_assistant_runs`.".to_string(),
            format!(
                "Derived AI state lives beside the archive at {} and can be cleared/rebuilt without touching canonical visits.",
                paths.app_root.display()
            ),
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

/// Ensures the assistant-run table contains the latest additive columns.
fn ensure_ai_assistant_run_columns(connection: &Connection) -> Result<()> {
    let mut statement = connection.prepare("PRAGMA table_info(ai_assistant_runs)")?;
    let columns = statement
        .query_map([], |row: &Row<'_>| row.get::<_, String>(1))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    if !columns.iter().any(|column| column == "embedding_provider_id") {
        connection.execute(
            "ALTER TABLE ai_assistant_runs ADD COLUMN embedding_provider_id TEXT NOT NULL DEFAULT 'lexical-fallback'",
            [],
        )?;
    }
    Ok(())
}
