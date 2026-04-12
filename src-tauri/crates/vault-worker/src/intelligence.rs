//! Intelligence, queue, and derived-state worker flows.
//!
//! This module owns the worker-side orchestration for optional AI and
//! deterministic intelligence features:
//!
//! - provider connection tests and runtime resolution
//! - semantic search / assistant execution
//! - persisted AI queue draining, replay, cancel, and heartbeat
//! - insights rebuild/load/explain entrypoints
//! - enrichment/intelligence runtime controls surfaced in Settings
//!
//! The core product rule is that intelligence is additive. Failures here should
//! not rewrite canonical archive facts, and background queue work must keep its
//! run/job trace explicit so the UI can stay honest about stale, queued, or
//! degraded states.

use crate::context::{
    ai_archive_connection, load_unlocked_config, provider_config_for_request,
    queue_failure_from_error, resolve_provider_runtime, search_response_with_resolution_note,
    selected_embedding_provider_runtime, selected_llm_provider_runtime,
    selected_optional_embedding_runtime, start_ai_job_heartbeat, tokio_runtime,
};
use anyhow::{Context, Result};
use serde_json::json;
use std::{
    sync::atomic::{AtomicBool, Ordering},
    thread,
};
use vault_core::{
    AiAssistantRequest, AiAssistantResponse, AiIndexReport, AiIndexRequest, AiIntegrationPreview,
    AiProviderConnectionTestReport, AiProviderConnectionTestRequest, AiProviderPurpose, AiQueueJob,
    AiQueueStatus, AiSearchRequest, AiSearchResponse, AppConfig, DeterministicRebuildQueueReport,
    ExplainInsightRequest, InsightExplanation, InsightSnapshot, InsightThreadDetail,
    IntelligenceRuntimeSnapshot, RunInsightsReport, RunInsightsRequest, ai_queue,
    answer_history_question, build_ai_index, cancel_intelligence_job, execute_enrichment_job_by_id,
    explain_insight,
    intelligence_runtime::{
        DETERMINISTIC_REBUILD_JOB_TYPE, claim_deterministic_rebuild_job,
        enqueue_deterministic_rebuild_job, mark_intelligence_job_failed,
        mark_intelligence_job_succeeded, next_queued_intelligence_job,
        update_intelligence_job_artifact,
    },
    load_assistant_run_response, load_insight_thread_detail, load_insights,
    load_intelligence_runtime, preview_ai_integrations, retry_intelligence_job, run_insights,
    run_insights_with_progress, semantic_search_history, test_provider_connection,
};

static AI_QUEUE_DRAIN_ACTIVE: AtomicBool = AtomicBool::new(false);
static INTELLIGENCE_QUEUE_DRAIN_ACTIVE: AtomicBool = AtomicBool::new(false);

/// Completes one claimed index job and writes the queue outcome back to SQLite.
pub(crate) fn complete_claimed_index_job(
    connection: &rusqlite::Connection,
    paths: &vault_core::ProjectPaths,
    config: &AppConfig,
    session_database_key: Option<&str>,
    claimed: ai_queue::StoredAiJob,
    request: &AiIndexRequest,
) -> Result<AiIndexReport> {
    let provider = selected_embedding_provider_runtime(config, request.provider_id.as_deref())?;
    let heartbeat = start_ai_job_heartbeat(paths, config, session_database_key, claimed.id);
    let result = tokio_runtime()?.block_on(build_ai_index(
        paths,
        config,
        session_database_key,
        &provider,
        request,
    ));
    let _ = heartbeat.send(());

    match result {
        Ok(mut report) => {
            report.job_id = Some(claimed.id);
            let summary = format!(
                "Indexed {} new / {} updated row(s).",
                report.indexed_items, report.updated_items
            );
            ai_queue::mark_ai_job_succeeded(
                connection,
                claimed.id,
                report.run_id,
                Some(summary.as_str()),
            )?;
            Ok(report)
        }
        Err(error) => {
            let failure = queue_failure_from_error(&error);
            ai_queue::mark_ai_job_failed(
                connection,
                claimed.id,
                None,
                &failure,
                config.ai.job_queue_paused,
            )?;
            Err(error)
        }
    }
}

/// Completes one claimed assistant job and persists the response trace.
pub(crate) fn complete_claimed_assistant_job(
    connection: &rusqlite::Connection,
    paths: &vault_core::ProjectPaths,
    config: &AppConfig,
    session_database_key: Option<&str>,
    claimed: ai_queue::StoredAiJob,
    _request: &AiAssistantRequest,
) -> Result<AiAssistantResponse> {
    let ai_queue::AiJobPayload::Assistant { payload } = claimed.payload.clone() else {
        anyhow::bail!("AI job {} did not contain an assistant payload.", claimed.id);
    };
    let llm_provider = selected_llm_provider_runtime(config, Some(&payload.llm_provider_id))?;
    let embedding_provider = payload
        .embedding_provider_id
        .as_deref()
        .map(|provider_id| selected_embedding_provider_runtime(config, Some(provider_id)))
        .transpose()?;
    let heartbeat = start_ai_job_heartbeat(paths, config, session_database_key, claimed.id);
    let result = tokio_runtime()?.block_on(answer_history_question(
        paths,
        config,
        session_database_key,
        &llm_provider,
        embedding_provider.as_ref(),
        &payload.request,
    ));
    let _ = heartbeat.send(());

    match result {
        Ok(mut response) => {
            response.job_id = Some(claimed.id);
            let summary = format!("Answered with {} citation(s).", response.citations.len());
            ai_queue::mark_ai_job_succeeded(
                connection,
                claimed.id,
                response.run_id,
                Some(summary.as_str()),
            )?;
            Ok(response)
        }
        Err(error) => {
            let failure = queue_failure_from_error(&error);
            ai_queue::mark_ai_job_failed(
                connection,
                claimed.id,
                None,
                &failure,
                config.ai.job_queue_paused,
            )?;
            Err(error)
        }
    }
}

/// Claims a queued assistant job by id and executes it.
pub(crate) fn execute_assistant_job(
    connection: &rusqlite::Connection,
    paths: &vault_core::ProjectPaths,
    config: &AppConfig,
    session_database_key: Option<&str>,
    job_id: i64,
    request: &AiAssistantRequest,
) -> Result<AiAssistantResponse> {
    let claimed = ai_queue::claim_ai_job_by_id(connection, job_id, 300)?
        .with_context(|| format!("AI assistant job {job_id} is not ready to run"))?;
    complete_claimed_assistant_job(
        connection,
        paths,
        config,
        session_database_key,
        claimed,
        request,
    )
}

pub(crate) fn maybe_spawn_ai_queue_drain(
    paths: &vault_core::ProjectPaths,
    config: &AppConfig,
    session_database_key: Option<&str>,
    queued_jobs: u32,
) {
    if config.ai.job_queue_paused || queued_jobs == 0 {
        return;
    }
    spawn_ai_queue_drain(paths.clone(), session_database_key.map(ToOwned::to_owned));
}

pub(crate) fn maybe_spawn_intelligence_queue_drain(
    paths: &vault_core::ProjectPaths,
    config: &AppConfig,
    session_database_key: Option<&str>,
    queued_jobs: usize,
) {
    if config.ai.job_queue_paused || queued_jobs == 0 {
        return;
    }
    spawn_intelligence_queue_drain(paths.clone(), session_database_key.map(ToOwned::to_owned));
}

fn spawn_ai_queue_drain(_paths: vault_core::ProjectPaths, session_database_key: Option<String>) {
    if AI_QUEUE_DRAIN_ACTIVE
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        return;
    }

    let _ = thread::Builder::new().name("pathkeep-ai-queue".to_string()).spawn(move || {
        loop {
            match run_ai_queue_jobs(session_database_key.as_deref(), None) {
                Ok(status) => {
                    if status.paused || status.queued == 0 {
                        break;
                    }
                }
                Err(error) => {
                    eprintln!("PathKeep could not drain the AI queue in the background: {error:#}");
                    break;
                }
            }
        }

        AI_QUEUE_DRAIN_ACTIVE.store(false, Ordering::Release);
    });
}

fn spawn_intelligence_queue_drain(
    paths: vault_core::ProjectPaths,
    session_database_key: Option<String>,
) {
    if INTELLIGENCE_QUEUE_DRAIN_ACTIVE
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        return;
    }

    let _ = thread::Builder::new()
        .name("pathkeep-intelligence-queue".to_string())
        .spawn(move || {
            loop {
                let config = match load_unlocked_config(&paths) {
                    Ok(config) => config,
                    Err(error) => {
                        eprintln!("PathKeep could not load intelligence queue config: {error:#}");
                        break;
                    }
                };
                if !config.initialized || config.ai.job_queue_paused {
                    break;
                }
                let connection =
                    match ai_archive_connection(&paths, &config, session_database_key.as_deref()) {
                        Ok(connection) => connection,
                        Err(error) => {
                            eprintln!(
                                "PathKeep could not open the archive for intelligence queue work: {error:#}"
                            );
                            break;
                        }
                    };
                let Some(job) = (match next_queued_intelligence_job(&connection) {
                    Ok(job) => job,
                    Err(error) => {
                        eprintln!(
                            "PathKeep could not load the next intelligence queue job: {error:#}"
                        );
                        break;
                    }
                }) else {
                    break;
                };

                if job.job_type == DETERMINISTIC_REBUILD_JOB_TYPE {
                    let _ = execute_deterministic_rebuild_job(
                        &paths,
                        &config,
                        session_database_key.as_deref(),
                        job.id,
                    );
                } else {
                    let _ = execute_enrichment_job_by_id(&connection, job.id);
                }
            }

            INTELLIGENCE_QUEUE_DRAIN_ACTIVE.store(false, Ordering::Release);
        });
}

/// Loads the persisted AI queue status for the current archive.
pub fn load_ai_queue(session_database_key: Option<&str>) -> Result<AiQueueStatus> {
    let paths = vault_core::project_paths()?;
    let config = load_unlocked_config(&paths)?;
    let status = vault_core::ai_queue_status(&paths, &config, session_database_key)?;
    maybe_spawn_ai_queue_drain(&paths, &config, session_database_key, status.queued);
    Ok(status)
}

/// Drains queued AI jobs up to the requested limit.
pub fn run_ai_queue_jobs(
    session_database_key: Option<&str>,
    max_jobs: Option<u32>,
) -> Result<AiQueueStatus> {
    let paths = vault_core::project_paths()?;
    let config = load_unlocked_config(&paths)?;
    if config.ai.job_queue_paused {
        return vault_core::ai_queue_status(&paths, &config, session_database_key);
    }

    let connection = ai_archive_connection(&paths, &config, session_database_key)?;
    let limit = max_jobs.unwrap_or(config.ai.job_queue_concurrency.max(1));
    for _ in 0..limit {
        let Some(job) = ai_queue::claim_next_ai_job(&connection, 300)? else {
            break;
        };
        match job.payload.clone() {
            ai_queue::AiJobPayload::Index { request } => {
                let _ = complete_claimed_index_job(
                    &connection,
                    &paths,
                    &config,
                    session_database_key,
                    job,
                    &request,
                );
            }
            ai_queue::AiJobPayload::Assistant { payload } => {
                let _ = complete_claimed_assistant_job(
                    &connection,
                    &paths,
                    &config,
                    session_database_key,
                    job,
                    &payload.request,
                );
            }
        }
    }

    vault_core::ai_queue_status(&paths, &config, session_database_key)
}

/// Marks one AI job as replayable again.
pub fn replay_ai_job(session_database_key: Option<&str>, job_id: i64) -> Result<AiQueueJob> {
    let paths = vault_core::project_paths()?;
    let config = load_unlocked_config(&paths)?;
    let connection = ai_archive_connection(&paths, &config, session_database_key)?;
    let job = ai_queue::replay_ai_job(&connection, job_id, config.ai.job_queue_paused)?;
    maybe_spawn_ai_queue_drain(
        &paths,
        &config,
        session_database_key,
        if job.state == "queued" { 1 } else { 0 },
    );
    Ok(job)
}

/// Cancels one queued/retryable AI job.
pub fn cancel_ai_job(session_database_key: Option<&str>, job_id: i64) -> Result<AiQueueJob> {
    let paths = vault_core::project_paths()?;
    let config = load_unlocked_config(&paths)?;
    let connection = ai_archive_connection(&paths, &config, session_database_key)?;
    ai_queue::cancel_ai_job(&connection, job_id)
}

/// Queues and, when allowed, immediately runs an AI index build.
pub fn build_ai_index_now(
    session_database_key: Option<&str>,
    request: &AiIndexRequest,
) -> Result<AiIndexReport> {
    let paths = vault_core::project_paths()?;
    let config = load_unlocked_config(&paths)?;
    let provider_config = provider_config_for_request(
        &config,
        request.provider_id.as_deref(),
        AiProviderPurpose::Embedding,
    )?;
    let queued_request =
        AiIndexRequest { provider_id: Some(provider_config.id.clone()), ..request.clone() };
    let connection = ai_archive_connection(&paths, &config, session_database_key)?;
    let queued =
        ai_queue::enqueue_index_job(&connection, &queued_request, config.ai.job_queue_paused)?;

    if config.ai.job_queue_paused {
        return Ok(AiIndexReport {
            job_id: Some(queued.id),
            run_id: None,
            provider_id: provider_config.id,
            model: provider_config.default_model,
            indexed_items: 0,
            updated_items: 0,
            skipped_items: 0,
            removed_items: 0,
            last_indexed_at: chrono::Utc::now().to_rfc3339(),
            notes: vec![format!(
                "Queued AI index job {}. Resume the AI queue to process it.",
                queued.id
            )],
        });
    }
    maybe_spawn_ai_queue_drain(&paths, &config, session_database_key, 1);
    Ok(AiIndexReport {
        job_id: Some(queued.id),
        run_id: None,
        provider_id: provider_config.id,
        model: provider_config.default_model,
        indexed_items: 0,
        updated_items: 0,
        skipped_items: 0,
        removed_items: 0,
        last_indexed_at: chrono::Utc::now().to_rfc3339(),
        notes: vec![format!(
            "Queued AI index job {}. PathKeep is processing it in the background.",
            queued.id
        )],
    })
}

/// Runs semantic search, falling back to lexical search when semantic runtime is unavailable.
pub fn search_ai_history(
    session_database_key: Option<&str>,
    request: &AiSearchRequest,
) -> Result<AiSearchResponse> {
    let paths = vault_core::project_paths()?;
    let config = load_unlocked_config(&paths)?;
    let embedding_provider = match selected_optional_embedding_runtime(&config) {
        Ok(provider) => provider,
        Err(error) => {
            let lexical =
                run_semantic_search(&paths, &config, session_database_key, None, request)?;
            return Ok(search_response_with_resolution_note(lexical, Some(error)));
        }
    };
    run_semantic_search(&paths, &config, session_database_key, embedding_provider.as_ref(), request)
}

/// Executes one semantic-search request using the optional embedding provider when available.
fn run_semantic_search(
    paths: &vault_core::ProjectPaths,
    config: &AppConfig,
    session_database_key: Option<&str>,
    embedding_provider: Option<&vault_core::AiProviderRuntime>,
    request: &AiSearchRequest,
) -> Result<AiSearchResponse> {
    tokio_runtime()?.block_on(semantic_search_history(
        paths,
        config,
        session_database_key,
        embedding_provider,
        request,
    ))
}

/// Queues and, when allowed, immediately runs one assistant request.
pub fn ask_ai_assistant(
    session_database_key: Option<&str>,
    request: &AiAssistantRequest,
) -> Result<AiAssistantResponse> {
    let paths = vault_core::project_paths()?;
    let config = load_unlocked_config(&paths)?;
    let llm_provider = provider_config_for_request(&config, None, AiProviderPurpose::Llm)?;
    let embedding_provider = config.ai.embedding_provider_id.clone();
    let connection = ai_archive_connection(&paths, &config, session_database_key)?;
    let queued = ai_queue::enqueue_assistant_job(
        &connection,
        request,
        &llm_provider.id,
        embedding_provider.as_deref(),
        config.ai.job_queue_paused,
    )?;

    if config.ai.job_queue_paused {
        return Ok(AiAssistantResponse {
            state: "queued".to_string(),
            answer: String::new(),
            job_id: Some(queued.id),
            run_id: None,
            provider_id: llm_provider.id,
            embedding_provider_id: embedding_provider
                .unwrap_or_else(|| "lexical-fallback".to_string()),
            citations: Vec::new(),
            notes: vec![format!(
                "The AI queue is paused. Assistant request queued as job {}. Resume or drain the queue to finish it.",
                queued.id
            )],
        });
    }

    execute_assistant_job(&connection, &paths, &config, session_database_key, queued.id, request)
}

/// Loads the current state for one assistant job, including the persisted run trace when ready.
pub fn load_ai_assistant_job(
    session_database_key: Option<&str>,
    job_id: i64,
) -> Result<AiAssistantResponse> {
    let paths = vault_core::project_paths()?;
    let config = load_unlocked_config(&paths)?;
    let connection = ai_archive_connection(&paths, &config, session_database_key)?;
    let job = ai_queue::load_ai_job(&connection, job_id)?;
    let Some(run_id) = job.run_id else {
        let (provider_id, embedding_provider_id) =
            match ai_queue::load_ai_job_payload(&connection, job_id)? {
                ai_queue::AiJobPayload::Assistant { payload } => (
                    payload.llm_provider_id,
                    payload.embedding_provider_id.unwrap_or_else(|| "lexical-fallback".to_string()),
                ),
                _ => (
                    config.ai.llm_provider_id.clone().unwrap_or_default(),
                    config
                        .ai
                        .embedding_provider_id
                        .clone()
                        .unwrap_or_else(|| "lexical-fallback".to_string()),
                ),
            };
        return Ok(AiAssistantResponse {
            state: job.state,
            answer: String::new(),
            job_id: Some(job.id),
            run_id: None,
            provider_id,
            embedding_provider_id,
            citations: Vec::new(),
            notes: job
                .summary
                .map(|summary| vec![summary])
                .unwrap_or_else(|| vec!["Assistant job has not finished yet.".to_string()]),
        });
    };
    let mut response = load_assistant_run_response(&paths, &config, session_database_key, run_id)?;
    response.job_id = Some(job.id);
    response.state = job.state;
    Ok(response)
}

/// Builds the MCP/skill/manual integration preview files.
pub fn preview_ai_integration_files() -> Result<AiIntegrationPreview> {
    let paths = vault_core::project_paths()?;
    let config = load_unlocked_config(&paths)?;
    preview_ai_integrations(&paths, &config)
}

/// Runs the insights rebuild flow, using embeddings only when the runtime is ready.
pub fn run_insights_now(
    session_database_key: Option<&str>,
    request: &RunInsightsRequest,
) -> Result<RunInsightsReport> {
    let paths = vault_core::project_paths()?;
    let config = load_unlocked_config(&paths)?;
    let embedding_provider = selected_optional_embedding_runtime(&config).ok().flatten();
    run_insights(&paths, &config, session_database_key, embedding_provider.as_ref(), request)
}

/// Queues one manual deterministic rebuild so heavy work can stay off the foreground UI thread.
#[cfg_attr(not(test), allow(dead_code))]
pub fn queue_insights_rebuild(
    session_database_key: Option<&str>,
    request: &RunInsightsRequest,
) -> Result<DeterministicRebuildQueueReport> {
    let paths = vault_core::project_paths()?;
    let config = load_unlocked_config(&paths)?;
    let connection = ai_archive_connection(&paths, &config, session_database_key)?;
    let job_id = enqueue_deterministic_rebuild_job(
        &connection,
        request,
        "User requested a deterministic rebuild from the UI.",
    )?;
    let state = connection
        .query_row("SELECT state FROM intelligence_jobs WHERE id = ?1", [job_id], |row| {
            row.get::<_, String>(0)
        })
        .unwrap_or_else(|_| "queued".to_string());
    maybe_spawn_intelligence_queue_drain(&paths, &config, session_database_key, 1);
    Ok(DeterministicRebuildQueueReport {
        job_id,
        state: state.clone(),
        notes: vec![if config.ai.job_queue_paused {
            format!(
                "Queued deterministic rebuild job {}. Resume background work to process it.",
                job_id
            )
        } else if state == "running" {
            format!("Deterministic rebuild job {} is already running in the background.", job_id)
        } else {
            format!(
                "Queued deterministic rebuild job {}. PathKeep is processing it in the background.",
                job_id
            )
        }],
    })
}

fn execute_deterministic_rebuild_job(
    paths: &vault_core::ProjectPaths,
    config: &AppConfig,
    session_database_key: Option<&str>,
    job_id: i64,
) -> Result<bool> {
    let connection = ai_archive_connection(paths, config, session_database_key)?;
    let Some(payload) = claim_deterministic_rebuild_job(&connection, job_id)? else {
        return Ok(false);
    };
    let mut refresh_config = config.clone();
    // Deterministic rebuilds must stay responsive and should not get stuck
    // processing optional readable-content enrichment inline.
    refresh_config.ai.enrichment_enabled = false;
    let embedding_provider = selected_optional_embedding_runtime(&refresh_config).ok().flatten();
    let initial_profile =
        payload.request.profile_id.as_deref().unwrap_or("all profiles").to_string();
    let _ = update_intelligence_job_artifact(
        &connection,
        job_id,
        &json!({
            "kind": "deterministic-rebuild",
            "phase": "Queued work started",
            "detail": format!("Preparing a deterministic rebuild for {initial_profile}."),
            "completedSteps": 0,
            "totalSteps": 8,
            "progressPercent": 0.0
        }),
    );
    match run_insights_with_progress(
        paths,
        &refresh_config,
        session_database_key,
        embedding_provider.as_ref(),
        &payload.request,
        |progress| {
            let artifact = json!({
                "kind": "deterministic-rebuild",
                "phase": progress.phase_label,
                "detail": progress.detail,
                "completedSteps": progress.phase_step,
                "totalSteps": progress.phase_count,
                "processedItems": progress.processed_items,
                "totalItems": progress.total_items,
                "progressPercent": progress.percent(),
            });
            let _ = update_intelligence_job_artifact(&connection, job_id, &artifact);
        },
    ) {
        Ok(report) => {
            mark_intelligence_job_succeeded(
                &connection,
                job_id,
                &json!({
                    "kind": "deterministic-rebuild",
                    "phase": "Completed",
                    "detail": format!(
                        "{} visits processed, {} cards ready.",
                        report.processed_visits,
                        report.card_count
                    ),
                    "completedSteps": 8,
                    "totalSteps": 8,
                    "processedItems": report.processed_visits,
                    "totalItems": report.processed_visits,
                    "progressPercent": 100.0,
                    "processedVisits": report.processed_visits,
                    "cardCount": report.card_count,
                    "queryGroupCount": report.query_group_count,
                    "threadCount": report.thread_count,
                    "notes": report.notes,
                }),
            )?;
            Ok(true)
        }
        Err(error) => {
            mark_intelligence_job_failed(&connection, job_id, &error.to_string())?;
            Err(error)
        }
    }
}

/// Loads the current insight snapshot read model.
pub fn load_insights_snapshot(
    session_database_key: Option<&str>,
    request: &RunInsightsRequest,
) -> Result<InsightSnapshot> {
    let paths = vault_core::project_paths()?;
    let config = load_unlocked_config(&paths)?;
    load_insights(&paths, &config, session_database_key, request)
}

/// Loads one insight thread detail record.
pub fn load_insight_thread(
    session_database_key: Option<&str>,
    thread_id: &str,
) -> Result<InsightThreadDetail> {
    let paths = vault_core::project_paths()?;
    let config = load_unlocked_config(&paths)?;
    load_insight_thread_detail(&paths, &config, session_database_key, thread_id)
}

/// Explains one persisted insight card or thread summary.
pub fn explain_insight_now(
    session_database_key: Option<&str>,
    request: &ExplainInsightRequest,
) -> Result<InsightExplanation> {
    let paths = vault_core::project_paths()?;
    let config = load_unlocked_config(&paths)?;
    explain_insight(&paths, &config, session_database_key, request)
}

/// Loads the Settings-facing intelligence runtime snapshot.
pub fn load_intelligence_runtime_snapshot(
    session_database_key: Option<&str>,
) -> Result<IntelligenceRuntimeSnapshot> {
    let paths = vault_core::project_paths()?;
    let mut config = vault_core::load_config(&paths)?;
    crate::context::hydrate_derived_config_state(&mut config);
    let snapshot = load_intelligence_runtime(&paths, &config, session_database_key)?;
    maybe_spawn_intelligence_queue_drain(
        &paths,
        &config,
        session_database_key,
        snapshot.queue.queued,
    );
    Ok(snapshot)
}

/// Retries one enrichment/intelligence runtime job.
pub fn retry_intelligence_job_now(
    session_database_key: Option<&str>,
    job_id: i64,
) -> Result<IntelligenceRuntimeSnapshot> {
    let paths = vault_core::project_paths()?;
    let mut config = vault_core::load_config(&paths)?;
    crate::context::hydrate_derived_config_state(&mut config);
    retry_intelligence_job(&paths, &config, session_database_key, job_id)?;
    let snapshot = load_intelligence_runtime(&paths, &config, session_database_key)?;
    maybe_spawn_intelligence_queue_drain(
        &paths,
        &config,
        session_database_key,
        snapshot.queue.queued,
    );
    Ok(snapshot)
}

/// Cancels one queued enrichment/intelligence runtime job.
pub fn cancel_intelligence_job_now(
    session_database_key: Option<&str>,
    job_id: i64,
) -> Result<IntelligenceRuntimeSnapshot> {
    let paths = vault_core::project_paths()?;
    let mut config = vault_core::load_config(&paths)?;
    crate::context::hydrate_derived_config_state(&mut config);
    cancel_intelligence_job(&paths, &config, session_database_key, job_id)
}

/// Checks whether one configured AI provider can be contacted successfully.
pub fn test_ai_provider_connection_report(
    _session_database_key: Option<&str>,
    request: &AiProviderConnectionTestRequest,
) -> Result<AiProviderConnectionTestReport> {
    let paths = vault_core::project_paths()?;
    let config = load_unlocked_config(&paths)?;
    let provider_config =
        provider_config_for_request(&config, Some(&request.provider_id), request.purpose.clone())?;

    match resolve_provider_runtime(
        match request.purpose {
            AiProviderPurpose::Embedding => &config.ai.embedding_providers,
            AiProviderPurpose::Llm => &config.ai.llm_providers,
        },
        &request.provider_id,
        request.purpose.clone(),
    ) {
        Ok(provider) => tokio_runtime()?.block_on(test_provider_connection(&provider)),
        Err(error) => {
            Ok(vault_core::provider_connection_failure_report(&provider_config, &error.to_string()))
        }
    }
}
