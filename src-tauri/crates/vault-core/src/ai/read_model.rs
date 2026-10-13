//! Responsibilities: expose AI readiness, counters, queue controls and saved run read models.
//! Not responsible for: provider execution, indexing loops or database ownership.
//! Failure modes: status opens another archive, metadata counters disagree with rows,
//! recent-job pages hide work, or a read mutates recovery state.
//!
//! Operations receive Vault and use its Intelligence read pool.

use super::native_ledger::{
    ai_embedding_token_estimate, ai_embeddings_storage_bytes, load_index_ledger,
    provider_embedding_count, semantic_index_staleness_reason,
};
use super::provider::ProviderReadiness;
use super::*;
use crate::models::{BUILT_IN_STATIC_EMBEDDING_PROVIDER_ID, StaticEmbeddingStatus};
use vault_store::{Db, OptionalRow, Session, params_owned as params};

/// Reports semantic-index readiness, queue state, and storage/readiness notes.
pub fn ai_index_status(vault: &crate::Vault) -> Result<AiIndexStatus> {
    let paths = vault.paths();
    let config = vault.config();
    let watermark = super::native_ledger::current_source_watermark(vault)?;
    let paths = paths.clone();
    let config = config.clone();
    vault.read(Db::Intelligence, move |s| {
        let mut status = ai_index_status_from(&paths, &config, s, watermark)?;
        if !config.initialized && config.ai.enabled {
            status.state = "blocked".into();
            status.ready = false;
            status.warning = Some(ai_index_warning_text(&AiIndexWarning::ArchiveNotInitialized));
            status.warning_code = Some(AiIndexWarning::ArchiveNotInitialized);
        }
        Ok(status)
    })
}

/// Reads the semantic-index status from an open intelligence connection. The caller has already
/// checked that the archive is initialized.
pub(crate) fn ai_index_status_from(
    paths: &ProjectPaths,
    config: &AppConfig,
    connection: &Session<'_>,
    watermark: i64,
) -> Result<AiIndexStatus> {
    let queue_status = ai_queue::store::load_ai_queue_status(
        connection,
        config.ai.job_queue_paused,
        config.ai.job_queue_concurrency,
        AI_QUEUE_RECENT_LIMIT,
    )?;
    let index_queue_counts = ai_queue::store::load_queue_job_counts(
        connection,
        &[AiQueueJobType::IndexBuild, AiQueueJobType::IndexClear],
    )?;

    let provider_id = config.ai.embedding_provider_id.clone();
    let provider_readiness = embedding_provider_readiness(config);
    let ledger = if let Some(provider_id) = provider_id.as_deref() {
        provider_readiness
            .selected_model
            .as_deref()
            .map(|model| load_index_ledger(connection, provider_id, model))
            .transpose()?
            .unwrap_or_default()
    } else {
        AiIndexLedgerRow::default()
    };
    let indexed_items = if let Some((provider_id, model)) =
        provider_id.as_deref().zip(provider_readiness.selected_model.as_deref())
    {
        provider_embedding_count(connection, provider_id, model)?
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
    let semantic_metadata_bytes = ai_embeddings_storage_bytes(connection)?;
    let estimated_embedding_tokens = ai_embedding_token_estimate(connection)?;
    let staleness_reason = provider_id
        .as_deref()
        .zip(provider_readiness.selected_model.as_deref())
        .map(|(provider_id, model)| {
            semantic_index_staleness_reason(
                connection,
                provider_id,
                model,
                ledger.source_watermark,
                watermark,
                ledger.last_indexed_at.as_deref(),
            )
        })
        .transpose()?
        .flatten();
    let last_indexed_at = if ledger.last_indexed_at.is_some() {
        ledger.last_indexed_at.clone()
    } else if let Some((provider, model)) =
        provider_id.as_deref().zip(provider_readiness.selected_model.as_deref())
    {
        connection
            .query_one(
                "SELECT last_indexed_at FROM ai_embedding_totals WHERE provider_id=?1 AND model=?2",
                params![provider, model],
                |r| Ok(r.get::<Option<String>>(0)?),
            )
            .optional()?
            .flatten()
    } else {
        None
    };
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
        recent_jobs_next_cursor: queue_status.recent_jobs_next_cursor,
        semantic_sidecar_bytes,
        semantic_metadata_bytes,
        estimated_embedding_tokens,
        warning,
        warning_code,
    })
}

/// Renders the English MODEL-facing / legacy text for an index-health warning CODE.
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
    let model_dir = model_dir_for_repo(paths, repo);
    let model_downloaded = model_is_present_and_verified(&model_dir, DEFAULT_STATIC_MODEL_FILES);
    // On-disk size only AFTER the files are present + verified; 0 before download (the FE shows a
    // static pre-download estimate). Summed from the real files so the Settings card states the honest
    // footprint instead of a guess.
    let model_size_bytes = if model_downloaded { static_model_size_bytes(&model_dir) } else { 0 };
    Some(StaticEmbeddingStatus {
        provider_id: provider.id.clone(),
        model_repo: repo.to_string(),
        model_downloaded,
        selected: config.ai.embedding_provider_id.as_deref() == Some(provider.id.as_str()),
        is_default: true,
        // The honest pooled width (D4: read from the safetensors at load, never the 1536 the FE would
        // otherwise infer from an absent provider-config dimension).
        dimensions: STATIC_EMBEDDING_DIM,
        model_size_bytes,
    })
}

/// Sums the on-disk byte size of the static model's manifest files under `model_dir`.
///
/// A missing/unreadable file contributes 0 (defensive — the caller only calls this once the model is
/// present + verified, so every file should exist; a transient stat failure must not panic the status
/// read). One `stat` per file (3 files), so this is cheap.
fn static_model_size_bytes(model_dir: &std::path::Path) -> u64 {
    DEFAULT_STATIC_MODEL_FILES
        .iter()
        .map(|file| {
            std::fs::metadata(model_dir.join(file.name)).map(|meta| meta.len()).unwrap_or(0)
        })
        .sum()
}

/// Loads the persisted AI queue read model.
pub fn ai_queue_status(vault: &crate::Vault) -> Result<AiQueueStatus> {
    ai_queue::load_ai_queue_status(
        vault,
        vault.config().ai.job_queue_paused,
        vault.config().ai.job_queue_concurrency,
        AI_QUEUE_RECENT_LIMIT,
    )
}

/// How long after its last heartbeat a running AI job is considered orphaned + recoverable.
/// Mirrors the drain's `claim_next_ai_job(&conn, 300)` stale window so the kick signal and the
/// eventual stale-sweep + reclaim agree on which running jobs are dead.
const AI_QUEUE_RECOVERY_STALE_SECONDS: i64 = 300;

/// Counts AI queue jobs a background drain could make progress on right now — queued/stale work AND
/// jobs orphaned in `running` by a worker that died (app quit/crash).
///
/// This exists because the drain pool is spawned only when there is recoverable work; gating that on
/// the read-model `queued` count (which never sees an orphaned `running` job) let an interrupted index
/// build sit wedged across every launch and never resume. Polling this lets the next status tick kick
/// the drain, which reclaims the orphaned job from its persisted cursor.
pub fn recoverable_ai_jobs(vault: &crate::Vault) -> Result<u32> {
    ai_queue::count_recoverable_ai_jobs(vault, AI_QUEUE_RECOVERY_STALE_SECONDS)
}

/// Synchronizes persisted queue pause/resume controls with updated Settings.
pub fn reconcile_ai_queue_controls(
    vault: &crate::Vault,
    previous_config: &AppConfig,
    next_config: &AppConfig,
) -> Result<()> {
    if previous_config.ai.job_queue_paused != next_config.ai.job_queue_paused {
        if next_config.ai.job_queue_paused {
            ai_queue::pause_queued_jobs(vault)?;
        } else {
            ai_queue::resume_paused_jobs(vault)?;
        }
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
    vault: &crate::Vault,
    run_id: i64,
) -> Result<AiAssistantResponse> {
    vault.read(Db::Intelligence, move |connection| {
        connection
            .query_one(
                "SELECT answer, provider_id, embedding_provider_id, citations_json, notes_json
             FROM ai_assistant_runs
             WHERE run_id = ?1",
                [run_id],
                |row| {
                    let citations_json: String = row.get(3)?;
                    let notes_json: String = row.get(4)?;
                    let citations = serde_json::from_str::<Vec<AiCitation>>(&citations_json)
                        .map_err(|error| vault_store::Error::Operation(Box::new(error)))?;
                    let notes = serde_json::from_str::<Vec<String>>(&notes_json)
                        .map_err(|error| vault_store::Error::Operation(Box::new(error)))?;
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
    })
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
