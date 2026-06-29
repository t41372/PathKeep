//! AI read models and integration preview surface.
//!
//! This module collects the non-generation-facing AI functions:
//! schema bootstrapping, readiness/status, queue controls, connection reports,
//! persisted assistant run loading, and manual integration preview.
//!
//! Keeping these together makes the public AI surface easier to scan without
//! digging through embedding/search/assistant execution internals.

use super::provider::ProviderReadiness;
use super::*;
use crate::models::{BUILT_IN_STATIC_EMBEDDING_PROVIDER_ID, StaticEmbeddingStatus};

/// Ensures the AI compatibility tables exist in the rebuildable intelligence plane.
pub fn ensure_ai_schema(connection: &Connection) -> Result<()> {
    connection.execute_batch(AI_SCHEMA_SQL)?;
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
            // The static tier surface does not need the archive — surface it even before init so the
            // Settings selector can render the download/ready state (F1).
            static_embedding: static_embedding_status(paths, config),
            queue_paused: default_queue_status.paused,
            queue_concurrency: default_queue_status.concurrency,
            warning: if config.ai.enabled {
                Some(ai_index_warning_text(&AiIndexWarning::ArchiveNotInitialized))
            } else {
                None
            },
            warning_code: config.ai.enabled.then_some(AiIndexWarning::ArchiveNotInitialized),
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
    // F3 (0-byte honesty): the REAL vector count on the `.pkvec` plane, read O(1) from the store
    // header + file length — NOT the SQLite metadata-row count above. A build that wrote metadata rows
    // but zero vectors leaves `indexed_items > 0` while this stays `0`, the exact dishonest case the
    // index health must surface. `count()` errors on a torn store; degrade to 0 (treated as "no
    // vectors") rather than failing the whole status read.
    let semantic_vector_count = provider_id
        .as_deref()
        .zip(provider_readiness.selected_model.as_deref())
        .map(|(provider_id, model)| {
            VectorStore::for_provider(paths, provider_id, model).count().unwrap_or(0)
        })
        .unwrap_or(0);
    // The dishonest "indexed N with an empty sidecar" case: metadata rows exist but the vector plane is
    // empty/absent, AND the provider is otherwise usable (an unusable provider is reported as degraded).
    let vectors_missing =
        provider_readiness.available && indexed_items > 0 && semantic_vector_count == 0;
    let semantic_sidecar_bytes = ai_sidecar::sidecar_storage_bytes(paths);
    let semantic_metadata_bytes = ai_embeddings_storage_bytes(&connection)?;
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
    // F3: readiness keys off the REAL vector count, not the SQLite metadata-row count — "ready" must
    // mean "there are vectors semantic search can actually match", never "we recorded N rows".
    let ready = semantic_vector_count > 0 && provider_readiness.available;
    let (state, warning, warning_code) = ai_index_state_and_warning(
        config,
        &provider_readiness,
        &queue_status,
        &index_queue_counts,
        &ledger,
        staleness_reason,
        ready,
        vectors_missing,
    );
    Ok(AiIndexStatus {
        enabled: config.ai.enabled,
        assistant_enabled: config.ai.assistant_enabled,
        mcp_enabled: config.ai.mcp_enabled,
        skill_enabled: config.ai.skill_enabled,
        state,
        ready,
        indexed_items: indexed_items as usize,
        semantic_vector_count,
        static_embedding: static_embedding_status(paths, config),
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
        semantic_metadata_bytes,
        estimated_embedding_tokens,
        warning,
        warning_code,
    })
}

/// Renders the English MODEL-facing / legacy text for an index-health warning CODE (review-fix M-7).
///
/// The wire now carries the stable [`AiIndexWarning`] CODE so the FE localizes it; this keeps the
/// legacy `AiIndexStatus.warning` string populated (for backward compatibility and any non-localizing
/// consumer) from the SAME code, so the code and its English rendering can never drift. Interpolated
/// variants compose the same sentence shape the prior `format!` calls produced.
pub(super) fn ai_index_warning_text(code: &AiIndexWarning) -> String {
    match code {
        AiIndexWarning::ArchiveNotInitialized => {
            "Initialize the archive before using AI analysis features.".to_string()
        }
        AiIndexWarning::NoEmbeddingProvider => {
            "Select an embedding provider in Settings before enabling semantic retrieval."
                .to_string()
        }
        AiIndexWarning::EmbeddingProviderMissing { provider_id } => {
            format!("Embedding provider {provider_id} is no longer available in Settings.")
        }
        AiIndexWarning::EmbeddingProviderDisabled { provider_name } => {
            format!("Enable provider {provider_name} before using semantic retrieval.")
        }
        AiIndexWarning::EmbeddingProviderNoApiKey { provider_name } => {
            format!(
                "Store an API key for provider {provider_name} before using semantic retrieval."
            )
        }
        AiIndexWarning::EmbeddingProviderNoModel { provider_name } => {
            format!(
                "Choose a default model for provider {provider_name} before using semantic retrieval."
            )
        }
        AiIndexWarning::IndexNotBuilt => {
            "Run Build index after configuring an embedding provider to enable semantic search."
                .to_string()
        }
        AiIndexWarning::IndexVectorsMissing => {
            "The semantic index has metadata rows but no vectors were written. Rebuild the index."
                .to_string()
        }
        AiIndexWarning::IndexStale { reason } => reason.model_facing_text().to_string(),
        AiIndexWarning::BuildFailed { reason } => reason.clone(),
    }
}

#[allow(clippy::too_many_arguments)]
fn ai_index_state_and_warning(
    config: &AppConfig,
    provider_readiness: &ProviderReadiness,
    queue_status: &AiQueueStatus,
    index_queue_counts: &ai_queue::QueueJobCounts,
    ledger: &AiIndexLedgerRow,
    staleness_reason: Option<AiSemanticStaleness>,
    ready: bool,
    vectors_missing: bool,
) -> (String, Option<String>, Option<AiIndexWarning>) {
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
    } else if vectors_missing {
        // Metadata rows but no vectors (F3): the index is present-but-unusable → degraded, not ready.
        "degraded".to_string()
    } else if staleness_reason.is_some() {
        "stale".to_string()
    } else if ready {
        "ready".to_string()
    } else if index_queue_counts.queued > 0 {
        "queued".to_string()
    } else {
        "empty".to_string()
    };
    // Resolve the legacy English STRING and the stable CODE together so the FE always has a code to
    // localize and the legacy string stays populated for any non-localizing consumer. A failed build
    // carries its opaque transport reason (no fixed vocabulary, so the code wraps the same text);
    // when it has no failure_reason it falls back to the last-failure timestamp (uncoded). The
    // unavailable-provider branch reuses the readiness's own pre-rendered string + code so the exact
    // missing prerequisite is preserved on both surfaces.
    let (warning, warning_code) = if ledger.state == "failed" {
        match ledger.failure_reason.clone() {
            Some(reason) => (Some(reason.clone()), Some(AiIndexWarning::BuildFailed { reason })),
            None => (ledger.last_failure_at.clone(), None),
        }
    } else if !provider_readiness.available {
        (provider_readiness.warning.clone(), provider_readiness.warning_code.clone())
    } else if vectors_missing {
        let code = AiIndexWarning::IndexVectorsMissing;
        (Some(ai_index_warning_text(&code)), Some(code))
    } else if let Some(reason) = staleness_reason {
        let code = AiIndexWarning::IndexStale { reason };
        (Some(ai_index_warning_text(&code)), Some(code))
    } else if config.ai.enabled && !ready {
        let code = AiIndexWarning::IndexNotBuilt;
        (Some(ai_index_warning_text(&code)), Some(code))
    } else {
        (None, None)
    };
    (state, warning, warning_code)
}

/// Resolves the built-in static embedding provider's readiness + download state for the UI (F1).
///
/// Returns `None` only when the merged config somehow lacks the built-in static provider (it is
/// normally always present via [`crate::models::merge_embedding_providers`]). `model_downloaded`
/// checks the consent-gated download target on disk (files present + SHA-verified); `selected` mirrors
/// the active embedding selection. Pure over the config + filesystem — no network, no archive — so it
/// can be surfaced even before the archive is initialized.
fn static_embedding_status(
    paths: &ProjectPaths,
    config: &AppConfig,
) -> Option<StaticEmbeddingStatus> {
    let provider = config
        .ai
        .embedding_providers
        .iter()
        .find(|provider| provider.id == BUILT_IN_STATIC_EMBEDDING_PROVIDER_ID)?;
    let repo = if provider.default_model.trim().is_empty() {
        DEFAULT_STATIC_MODEL_REPO
    } else {
        provider.default_model.as_str()
    };
    let model_downloaded =
        model_is_present_and_verified(&model_dir_for_repo(paths, repo), DEFAULT_STATIC_MODEL_FILES);
    Some(StaticEmbeddingStatus {
        provider_id: provider.id.clone(),
        model_repo: repo.to_string(),
        model_downloaded,
        selected: config.ai.embedding_provider_id.as_deref() == Some(provider.id.as_str()),
        is_default: true,
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
    let supports_interactive_chat_format = matches!(
        config.request_format,
        AiRequestFormat::OpenAi
            | AiRequestFormat::Anthropic
            | AiRequestFormat::Google
            | AiRequestFormat::Ollama
            | AiRequestFormat::LmStudio
    );
    let supports_tool_use = supports_chat && supports_interactive_chat_format;
    let supports_structured_output = supports_chat && supports_interactive_chat_format;
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
        // Resolution failed before a runtime existed, so there is no probed LLM detail to add.
        llm_capabilities: None,
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
                if config.ai.mcp_enabled {
                    "Usage guide is enabled: the MCP server serves a read-only guide teaching connected tools how to query effectively. It exposes no extra data.".to_string()
                } else {
                    "Usage guide is enabled but unreachable: it is only served while the MCP server above is also on. It exposes no extra data when reachable.".to_string()
                }
            } else {
                "Usage guide is disabled in saved Settings, so connected tools receive only a short disabled notice instead of the querying guide.".to_string()
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::AiSettings;

    fn enabled_config() -> AppConfig {
        AppConfig {
            ai: AiSettings { enabled: true, ..AiSettings::default() },
            ..AppConfig::default()
        }
    }

    #[test]
    fn ai_index_state_helper_covers_ordered_status_and_warning_edges() {
        let mut config = enabled_config();
        let ready_provider = ProviderReadiness {
            available: true,
            warning: None,
            warning_code: None,
            selected_model: Some("model".to_string()),
        };
        let unavailable_provider = ProviderReadiness {
            available: false,
            warning: Some("provider warning".to_string()),
            warning_code: Some(AiIndexWarning::NoEmbeddingProvider),
            selected_model: None,
        };
        let queue = AiQueueStatus::default();
        let empty_counts = ai_queue::QueueJobCounts { queued: 0, running: 0, failed: 0 };
        let queued_counts = ai_queue::QueueJobCounts { queued: 1, running: 0, failed: 0 };
        let running_counts = ai_queue::QueueJobCounts { queued: 0, running: 1, failed: 0 };
        let failed_ledger = AiIndexLedgerRow {
            state: "failed".to_string(),
            failure_reason: Some("embedding run failed".to_string()),
            ..AiIndexLedgerRow::default()
        };

        config.ai.enabled = false;
        assert_eq!(
            ai_index_state_and_warning(
                &config,
                &ready_provider,
                &queue,
                &empty_counts,
                &AiIndexLedgerRow::default(),
                None,
                false,
                false,
            )
            .0,
            "disabled"
        );

        config.ai.enabled = true;
        let degraded = ai_index_state_and_warning(
            &config,
            &unavailable_provider,
            &queue,
            &empty_counts,
            &AiIndexLedgerRow::default(),
            None,
            false,
            false,
        );
        assert_eq!(
            degraded,
            (
                "degraded".to_string(),
                Some("provider warning".to_string()),
                Some(AiIndexWarning::NoEmbeddingProvider),
            )
        );
        assert_eq!(
            ai_index_state_and_warning(
                &config,
                &ready_provider,
                &queue,
                &running_counts,
                &AiIndexLedgerRow::default(),
                None,
                false,
                false,
            )
            .0,
            "rebuilding"
        );
        assert_eq!(
            ai_index_state_and_warning(
                &config,
                &ready_provider,
                &AiQueueStatus { paused: true, ..AiQueueStatus::default() },
                &queued_counts,
                &AiIndexLedgerRow::default(),
                None,
                false,
                false,
            )
            .0,
            "paused"
        );
        assert_eq!(
            ai_index_state_and_warning(
                &config,
                &ready_provider,
                &queue,
                &empty_counts,
                &failed_ledger,
                None,
                false,
                false,
            ),
            (
                "failed".to_string(),
                Some("embedding run failed".to_string()),
                Some(AiIndexWarning::BuildFailed { reason: "embedding run failed".to_string() }),
            )
        );
        // A failed ledger with NO failure_reason falls back to the (uncoded) last-failure timestamp.
        let failed_no_reason = AiIndexLedgerRow {
            state: "failed".to_string(),
            failure_reason: None,
            last_failure_at: Some("2026-06-23T00:00:00Z".to_string()),
            ..AiIndexLedgerRow::default()
        };
        assert_eq!(
            ai_index_state_and_warning(
                &config,
                &ready_provider,
                &queue,
                &empty_counts,
                &failed_no_reason,
                None,
                false,
                false,
            ),
            ("failed".to_string(), Some("2026-06-23T00:00:00Z".to_string()), None)
        );
        // F3: an available provider with metadata rows but ZERO vectors → degraded + IndexVectorsMissing
        // (the honest "indexed N with an empty sidecar" surface), and it OUTRANKS the stale/not-built
        // warnings below it.
        let vectors_missing = ai_index_state_and_warning(
            &config,
            &ready_provider,
            &queue,
            &empty_counts,
            &AiIndexLedgerRow::default(),
            Some(AiSemanticStaleness::Watermark),
            false,
            true,
        );
        assert_eq!(vectors_missing.0, "degraded");
        assert_eq!(vectors_missing.2, Some(AiIndexWarning::IndexVectorsMissing));
        assert!(vectors_missing.1.expect("missing warning").contains("no vectors"));
        let stale = ai_index_state_and_warning(
            &config,
            &ready_provider,
            &queue,
            &empty_counts,
            &AiIndexLedgerRow::default(),
            Some(AiSemanticStaleness::Watermark),
            true,
            false,
        );
        assert_eq!(stale.0, "stale");
        assert_eq!(
            stale.2,
            Some(AiIndexWarning::IndexStale { reason: AiSemanticStaleness::Watermark })
        );
        // The legacy string is the watermark sentence derived from the same code.
        assert!(stale.1.expect("stale warning").contains("import watermark"));
        assert_eq!(
            ai_index_state_and_warning(
                &config,
                &ready_provider,
                &queue,
                &empty_counts,
                &AiIndexLedgerRow::default(),
                None,
                true,
                false,
            )
            .0,
            "ready"
        );
        assert_eq!(
            ai_index_state_and_warning(
                &config,
                &ready_provider,
                &queue,
                &queued_counts,
                &AiIndexLedgerRow::default(),
                None,
                false,
                false,
            )
            .0,
            "queued"
        );
        let empty = ai_index_state_and_warning(
            &config,
            &ready_provider,
            &queue,
            &empty_counts,
            &AiIndexLedgerRow::default(),
            None,
            false,
            false,
        );
        assert_eq!(empty.0, "empty");
        assert!(empty.1.expect("empty warning").contains("Build index"));
    }
}
