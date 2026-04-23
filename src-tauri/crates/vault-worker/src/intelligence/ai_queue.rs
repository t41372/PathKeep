//! Worker-side AI queue and assistant/search orchestration.
//!
//! ## Responsibilities
//! - own persisted AI queue draining, heartbeat, replay, and cancellation flows
//! - execute semantic search and assistant runs against the resolved provider runtime
//! - enqueue foreground AI requests as background jobs when the queue is enabled
//! - surface Settings-facing provider connectivity and integration preview helpers
//!
//! ## Not responsible for
//! - deterministic Core Intelligence rebuild jobs
//! - route-level read-model passthroughs for Core Intelligence surfaces
//! - canonical archive schema, provider config persistence, or Tauri command naming
//!
//! ## Dependencies
//! - `crate::context` for unlocked config, provider resolution, and Tokio runtime access
//! - `crate::job_runtime` for bounded worker-pool spawning
//! - `vault_core::ai_queue` plus AI runtime helpers for persisted queue state
//!
//! ## Performance notes
//! - queue drains intentionally cap concurrent workers through the shared
//!   `AI_QUEUE_ACTIVE_WORKERS` counter so background AI work does not stampede a
//!   4-core host
//! - assistant and semantic-search execution still bridge async provider work
//!   through the worker runtime, keeping heavy network/model calls off the UI thread

use super::AI_QUEUE_ACTIVE_WORKERS;
use crate::context::{
    ai_archive_connection, load_unlocked_config, provider_config_for_request,
    queue_failure_from_error, resolve_provider_runtime, search_response_with_resolution_note,
    selected_embedding_provider_runtime, selected_llm_provider_runtime,
    selected_optional_embedding_runtime, tokio_runtime,
};
use crate::job_runtime::{BackgroundJobControl, maybe_spawn_worker_pool};
use anyhow::{Context, Result};
use std::{sync::Arc, time::Duration};
use vault_core::{
    AiAssistantRequest, AiAssistantResponse, AiIndexReport, AiIndexRequest, AiIntegrationPreview,
    AiProviderConnectionTestReport, AiProviderConnectionTestRequest, AiProviderPurpose, AiQueueJob,
    AiQueueStatus, AiSearchRequest, AiSearchResponse, AppConfig, ai_queue,
    answer_history_question_with_control, build_ai_index_with_control, load_assistant_run_response,
    preview_ai_integrations, semantic_search_history, test_provider_connection,
};

/// Starts the heartbeat and cancellation polling loop for one running AI job.
///
/// The worker needs this helper because index builds and assistant runs can outlive
/// the foreground caller. It keeps SQLite queue state fresh and lets UI-triggered
/// cancellation requests interrupt long-running jobs without guessing whether a
/// provider call is still alive.
fn start_ai_job_control(
    paths: &vault_core::ProjectPaths,
    config: &AppConfig,
    session_database_key: Option<&str>,
    job_id: i64,
) -> Arc<BackgroundJobControl> {
    let paths = paths.clone();
    let config = config.clone();
    let session_database_key = session_database_key.map(ToOwned::to_owned);
    Arc::new(BackgroundJobControl::spawn(
        Duration::from_millis(500),
        Duration::from_secs(30),
        {
            let paths = paths.clone();
            let config = config.clone();
            let session_database_key = session_database_key.clone();
            move || {
                let connection =
                    ai_archive_connection(&paths, &config, session_database_key.as_deref())?;
                ai_queue::heartbeat_ai_job(&connection, job_id)
            }
        },
        move || {
            let connection =
                ai_archive_connection(&paths, &config, session_database_key.as_deref())?;
            ai_queue::ai_job_stop_requested(&connection, job_id)
        },
    ))
}

/// Completes one claimed index job and persists the queue outcome.
///
/// This keeps queue bookkeeping in one place so index-build callers do not have
/// to manually coordinate success/failure writes, cancellation checks, and the
/// final user-visible summary.
pub(crate) fn complete_claimed_index_job(
    connection: &rusqlite::Connection,
    paths: &vault_core::ProjectPaths,
    config: &AppConfig,
    session_database_key: Option<&str>,
    claimed: ai_queue::StoredAiJob,
    request: &AiIndexRequest,
) -> Result<AiIndexReport> {
    let provider = selected_embedding_provider_runtime(config, request.provider_id.as_deref())?;
    let run_control = start_ai_job_control(paths, config, session_database_key, claimed.id);
    let result = tokio_runtime()?.block_on(build_ai_index_with_control(
        paths,
        config,
        session_database_key,
        &provider,
        request,
        Some(run_control.clone()),
    ));
    run_control.shutdown();

    match result {
        Ok(mut report) => {
            report.job_id = Some(claimed.id);
            let summary = format!(
                "Indexed {} new / {} updated row(s).",
                report.indexed_items, report.updated_items
            );
            let cancelled = if ai_queue::ai_job_stop_requested(connection, claimed.id)? {
                true
            } else {
                !ai_queue::mark_ai_job_succeeded(
                    connection,
                    claimed.id,
                    report.run_id,
                    Some(summary.as_str()),
                )?
            };
            if cancelled {
                let _ = ai_queue::mark_running_ai_job_cancelled(
                    connection,
                    claimed.id,
                    Some("Index build cancelled from the UI."),
                )?;
            }
            Ok(report)
        }
        Err(error) => {
            let failure = queue_failure_from_error(&error);
            if ai_queue::ai_job_stop_requested(connection, claimed.id)? {
                let _ = ai_queue::mark_running_ai_job_cancelled(
                    connection,
                    claimed.id,
                    Some("Index build cancelled from the UI."),
                )?;
            } else {
                ai_queue::mark_ai_job_failed(
                    connection,
                    claimed.id,
                    None,
                    &failure,
                    config.ai.job_queue_paused,
                )?;
            }
            Err(error)
        }
    }
}

/// Completes one claimed assistant job and stores the persisted run trace.
///
/// Assistant runs can fail for queue, provider, or user-cancellation reasons.
/// Centralizing the state transition logic here keeps the queue contract honest
/// for both background drains and foreground "run now" paths.
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
    let run_control = start_ai_job_control(paths, config, session_database_key, claimed.id);
    let result = tokio_runtime()?.block_on(answer_history_question_with_control(
        paths,
        config,
        session_database_key,
        &llm_provider,
        embedding_provider.as_ref(),
        &payload.request,
        Some(run_control.clone()),
    ));
    run_control.shutdown();

    match result {
        Ok(mut response) => {
            response.job_id = Some(claimed.id);
            let summary = format!("Answered with {} citation(s).", response.citations.len());
            let cancelled = if ai_queue::ai_job_stop_requested(connection, claimed.id)? {
                true
            } else {
                !ai_queue::mark_ai_job_succeeded(
                    connection,
                    claimed.id,
                    response.run_id,
                    Some(summary.as_str()),
                )?
            };
            if cancelled {
                let _ = ai_queue::mark_running_ai_job_cancelled(
                    connection,
                    claimed.id,
                    Some("Assistant run cancelled from the UI."),
                )?;
            }
            Ok(response)
        }
        Err(error) => {
            let failure = queue_failure_from_error(&error);
            if ai_queue::ai_job_stop_requested(connection, claimed.id)? {
                let _ = ai_queue::mark_running_ai_job_cancelled(
                    connection,
                    claimed.id,
                    Some("Assistant run cancelled from the UI."),
                )?;
            } else {
                ai_queue::mark_ai_job_failed(
                    connection,
                    claimed.id,
                    None,
                    &failure,
                    config.ai.job_queue_paused,
                )?;
            }
            Err(error)
        }
    }
}

/// Claims one assistant job by id and executes it immediately.
///
/// This path is used when the UI wants an answer right away instead of waiting
/// for the background drain loop to notice a newly enqueued request.
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

/// Starts background AI queue workers when there is queued work to drain.
///
/// The worker shell calls this after foreground actions that might have enqueued
/// new AI jobs. Keeping the spawn decision here ensures archive flows and queue
/// callers share the same paused-state and concurrency guard.
pub(crate) fn maybe_spawn_ai_queue_drain(
    paths: &vault_core::ProjectPaths,
    config: &AppConfig,
    session_database_key: Option<&str>,
    queued_jobs: u32,
) {
    if config.ai.job_queue_paused || queued_jobs == 0 {
        return;
    }
    spawn_ai_queue_drain(
        paths.clone(),
        config.ai.job_queue_concurrency.max(1) as usize,
        session_database_key.map(ToOwned::to_owned),
    );
}

/// Drains queued AI jobs through the bounded worker pool.
///
/// Each spawned worker loops until the queue is empty, the archive cannot be
/// opened, or config says the AI queue is paused. The shared atomic counter keeps
/// the host from exceeding the configured concurrency.
fn spawn_ai_queue_drain(
    paths: vault_core::ProjectPaths,
    desired_workers: usize,
    session_database_key: Option<String>,
) {
    maybe_spawn_worker_pool(
        "pathkeep-ai-queue",
        &AI_QUEUE_ACTIVE_WORKERS,
        desired_workers,
        move || loop {
            let config = match load_unlocked_config(&paths) {
                Ok(config) => config,
                Err(error) => {
                    eprintln!("PathKeep could not load AI queue config: {error:#}");
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
                            "PathKeep could not open the archive for AI queue work: {error:#}"
                        );
                        break;
                    }
                };
            let Some(job) = (match ai_queue::claim_next_ai_job(&connection, 300) {
                Ok(job) => job,
                Err(error) => {
                    eprintln!("PathKeep could not claim the next AI queue job: {error:#}");
                    break;
                }
            }) else {
                break;
            };
            match job.payload.clone() {
                ai_queue::AiJobPayload::Index { request } => {
                    let _ = complete_claimed_index_job(
                        &connection,
                        &paths,
                        &config,
                        session_database_key.as_deref(),
                        job,
                        &request,
                    );
                }
                ai_queue::AiJobPayload::Assistant { payload } => {
                    let _ = complete_claimed_assistant_job(
                        &connection,
                        &paths,
                        &config,
                        session_database_key.as_deref(),
                        job,
                        &payload.request,
                    );
                }
            }
        },
    );
}

/// Loads the persisted AI queue state for the current archive.
///
/// The returned snapshot doubles as a lazy "kick the drain loop if needed"
/// surface, which keeps queue work moving even when the UI only polls status.
pub fn load_ai_queue(session_database_key: Option<&str>) -> Result<AiQueueStatus> {
    let paths = vault_core::project_paths()?;
    let config = load_unlocked_config(&paths)?;
    let status = vault_core::ai_queue_status(&paths, &config, session_database_key)?;
    maybe_spawn_ai_queue_drain(&paths, &config, session_database_key, status.queued);
    Ok(status)
}

/// Drains queued AI jobs up to the requested limit in the foreground caller.
///
/// This is the explicit "run now" path used by review/debug surfaces that want
/// deterministic progress instead of waiting for background workers.
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

/// Marks one AI job as replayable and restarts the drain loop if it became queued again.
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

/// Cancels one queued or retryable AI job without touching finished run artifacts.
pub fn cancel_ai_job(session_database_key: Option<&str>, job_id: i64) -> Result<AiQueueJob> {
    let paths = vault_core::project_paths()?;
    let config = load_unlocked_config(&paths)?;
    let connection = ai_archive_connection(&paths, &config, session_database_key)?;
    ai_queue::cancel_ai_job(&connection, job_id)
}

/// Enqueues an AI index build and starts background work when the queue is active.
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

/// Runs semantic search and degrades to lexical recall when embeddings are unavailable.
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

/// Executes one semantic-search request using the optional embedding provider.
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

/// Enqueues one assistant request and executes it immediately when the queue is active.
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

/// Loads one assistant job state, including the persisted answer when ready.
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

/// Builds the local MCP/skill/manual integration preview files shown in Settings.
pub fn preview_ai_integration_files() -> Result<AiIntegrationPreview> {
    let paths = vault_core::project_paths()?;
    let config = load_unlocked_config(&paths)?;
    preview_ai_integrations(&paths, &config)
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
