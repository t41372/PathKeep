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
    AiQueueStatus, AiSearchRequest, AiSearchResponse, AppConfig, ReembedEstimate, ReembedScope,
    WorkingSetConfig, ai_queue, answer_history_question_with_control, build_ai_index_with_control,
    estimate_reembed, load_assistant_run_response, preview_ai_integrations,
    semantic_search_history, test_provider_connection,
};

#[cfg(coverage)]
const AI_JOB_CONTROL_POLL_INTERVAL: Duration = Duration::from_millis(5);
#[cfg(not(coverage))]
const AI_JOB_CONTROL_POLL_INTERVAL: Duration = Duration::from_millis(500);
#[cfg(coverage)]
const AI_JOB_CONTROL_HEARTBEAT_EVERY: Duration = Duration::from_millis(5);
#[cfg(not(coverage))]
const AI_JOB_CONTROL_HEARTBEAT_EVERY: Duration = Duration::from_secs(30);

/// Starts the heartbeat and cancellation polling loop for one running AI job.
///
/// The worker needs this helper because index builds and assistant runs can outlive
/// the foreground caller. It keeps SQLite queue state fresh and lets UI-triggered
/// cancellation requests interrupt long-running jobs without guessing whether a
/// provider call is still alive.
pub(crate) fn start_ai_job_control(
    paths: &vault_core::ProjectPaths,
    config: &AppConfig,
    session_database_key: Option<&str>,
    job_id: i64,
) -> Arc<BackgroundJobControl> {
    let paths = paths.clone();
    let config = config.clone();
    let session_database_key = session_database_key.map(ToOwned::to_owned);
    Arc::new(BackgroundJobControl::spawn(
        AI_JOB_CONTROL_POLL_INTERVAL,
        AI_JOB_CONTROL_HEARTBEAT_EVERY,
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

/// Persists the resumable backfill watermark into the running index job's payload.
///
/// vault-core's embed loop calls [`record`](vault_core::IndexBackfillLedger::record) after each
/// chunk's vectors are durably written; this writes the cursor + a progress summary back into the
/// `ai_jobs` row so a worker restart resumes from `next_history_id` instead of re-embedding the
/// whole archive (02 §C.6 R1). It opens a fresh connection per call (cheap, bounded) so it never
/// has to share the drain loop's connection across the async embed boundary.
struct IndexCursorLedger {
    paths: vault_core::ProjectPaths,
    config: AppConfig,
    session_database_key: Option<String>,
    job_id: i64,
}

impl vault_core::IndexBackfillLedger for IndexCursorLedger {
    fn record(&self, progress: vault_core::IndexBackfillProgress) -> Result<()> {
        let connection =
            ai_archive_connection(&self.paths, &self.config, self.session_database_key.as_deref())?;
        let cursor = ai_queue::IndexBackfillCursor {
            next_history_id: progress.next_history_id,
            embedded_so_far: progress.embedded_so_far,
        };
        let summary = format!("Embedded {} row(s) so far.", progress.embedded_so_far);
        match ai_queue::persist_index_cursor(&connection, self.job_id, &cursor, Some(&summary))? {
            // A reclaimed lease (the stale-sweep moved this row out of `running`) means another
            // worker now owns the job. We MUST abort here so this de-leased worker stops embedding +
            // appending to the vector store while the new owner also writes it (HIGH-3): the Err
            // propagates up through the embed loop's `?`, ending this run before the next chunk.
            ai_queue::CursorPersistOutcome::LeaseLost => anyhow::bail!(
                "index job {} lost its queue lease (reclaimed by another worker); aborting backfill",
                self.job_id
            ),
            ai_queue::CursorPersistOutcome::Persisted
            | ai_queue::CursorPersistOutcome::NotIndexJob => Ok(()),
        }
    }
}

/// Resolves the backfill resume watermark from a claimed job's payload.
///
/// An index payload carries the persisted cursor; any other payload (defensive — only index jobs
/// reach the index path) starts from the beginning.
fn index_start_history_id(payload: &ai_queue::AiJobPayload) -> i64 {
    match payload {
        ai_queue::AiJobPayload::Index { cursor, .. } => cursor.next_history_id,
        _ => 0,
    }
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
    // Resume from the persisted backfill watermark so a restart never re-embeds from scratch.
    let start_history_id = index_start_history_id(&claimed.payload);
    let run_control = start_ai_job_control(paths, config, session_database_key, claimed.id);
    let ledger: Arc<dyn vault_core::IndexBackfillLedger> = Arc::new(IndexCursorLedger {
        paths: paths.clone(),
        config: config.clone(),
        session_database_key: session_database_key.map(ToOwned::to_owned),
        job_id: claimed.id,
    });
    let result = tokio_runtime()?.block_on(build_ai_index_with_control(
        paths,
        config,
        session_database_key,
        &provider,
        request,
        Some(run_control.clone()),
        start_history_id,
        Some(ledger),
    ));
    run_control.shutdown();

    match result {
        Ok(mut report) => {
            report.job_id = Some(claimed.id);
            let summary = format!(
                "Indexed {} new / {} updated row(s).",
                report.indexed_items, report.updated_items
            );
            mark_successful_ai_job_or_cancelled(
                connection,
                claimed.id,
                report.run_id,
                Some(summary.as_str()),
                Some("Index build cancelled from the UI."),
            )?;
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
            mark_successful_ai_job_or_cancelled(
                connection,
                claimed.id,
                response.run_id,
                Some(summary.as_str()),
                Some("Assistant run cancelled from the UI."),
            )?;
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

fn mark_successful_ai_job_or_cancelled(
    connection: &rusqlite::Connection,
    job_id: i64,
    run_id: Option<i64>,
    success_summary: Option<&str>,
    cancellation_summary: Option<&str>,
) -> Result<bool> {
    let cancelled = if ai_queue::ai_job_stop_requested(connection, job_id)? {
        true
    } else {
        !ai_queue::mark_ai_job_succeeded(connection, job_id, run_id, success_summary)?
    };
    if cancelled {
        let _ = ai_queue::mark_running_ai_job_cancelled(connection, job_id, cancellation_summary)?;
    }
    Ok(cancelled)
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
            match drain_one_ai_queue_job(&paths, session_database_key.as_deref()) {
                Ok(true) => {}
                Ok(false) => break,
                Err(error) => {
                    eprintln!("PathKeep could not drain AI queue work: {error:#}");
                    break;
                }
            }
        },
    );
}

pub(crate) fn drain_one_ai_queue_job(
    paths: &vault_core::ProjectPaths,
    session_database_key: Option<&str>,
) -> Result<bool> {
    let config = load_unlocked_config(paths).context("load AI queue config")?;
    if !config.initialized || config.ai.job_queue_paused {
        return Ok(false);
    }
    let connection = ai_archive_connection(paths, &config, session_database_key)
        .context("open archive for AI queue work")?;
    let Some(job) =
        ai_queue::claim_next_ai_job(&connection, 300).context("claim next AI queue job")?
    else {
        return Ok(false);
    };
    match job.payload.clone() {
        ai_queue::AiJobPayload::Index { request, .. } => {
            let _ = complete_claimed_index_job(
                &connection,
                paths,
                &config,
                session_database_key,
                job,
                &request,
            );
        }
        ai_queue::AiJobPayload::Assistant { payload } => {
            let _ = complete_claimed_assistant_job(
                &connection,
                paths,
                &config,
                session_database_key,
                job,
                &payload.request,
            );
        }
    }
    Ok(true)
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
            ai_queue::AiJobPayload::Index { request, .. } => {
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

/// Estimates the cost/time of a re-embed run for one scope (W-AI-9 Sub-block D, 05 §7).
///
/// Read-only: it opens the archive-attached connection and calls the PURE [`estimate_reembed`] (which
/// reads only the bounded working set or the `COUNT(*)` of unique pages), so it NEVER loads a model,
/// embeds, or touches the network. Surfaced to the FE BEFORE a re-embed fires so the user sees the
/// cost (PME). `gpu_available` reflects whether THIS binary was built with the `metal` feature.
pub fn estimate_reembed_now(
    session_database_key: Option<&str>,
    scope: ReembedScope,
) -> Result<ReembedEstimate> {
    let paths = vault_core::project_paths()?;
    let config = load_unlocked_config(&paths)?;
    // The working-set selector + unique-page count read the canonical `archive` schema; the
    // intelligence connection attaches it, but the selector queries unqualified table names, so run
    // the estimate against the archive connection directly (mirrors `enqueue_content_fetch_working_set`).
    let archive =
        vault_core::archive::open_archive_connection(&paths, &config, session_database_key)?;
    // `urls.last_visit_ms` is Chrome-epoch ms; derive "now" in the same frame for the recency window.
    const CHROME_EPOCH_OFFSET_MS: i64 = 11_644_473_600_000;
    let now_ms = chrono::Utc::now().timestamp_millis() + CHROME_EPOCH_OFFSET_MS;
    estimate_reembed(&archive, &WorkingSetConfig::default(), now_ms, scope)
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

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    fn claimed_index_job(connection: &Connection) -> i64 {
        ai_queue::ensure_ai_queue_schema(connection).expect("queue schema");
        let queued = ai_queue::enqueue_index_job(connection, &AiIndexRequest::default(), false)
            .expect("enqueue index job");
        ai_queue::claim_ai_job_by_id(connection, queued.id, 300)
            .expect("claim index job")
            .expect("claimed index job");
        queued.id
    }

    fn job_state(connection: &Connection, job_id: i64) -> String {
        connection
            .query_row("SELECT state FROM ai_jobs WHERE id = ?1", [job_id], |row| row.get(0))
            .expect("job state")
    }

    #[test]
    fn mark_successful_ai_job_or_cancelled_honors_stop_requests() {
        let connection = Connection::open_in_memory().expect("memory connection");
        let job_id = claimed_index_job(&connection);
        ai_queue::cancel_ai_job(&connection, job_id).expect("request cancellation");

        let cancelled = mark_successful_ai_job_or_cancelled(
            &connection,
            job_id,
            Some(42),
            Some("success summary"),
            Some("cancelled summary"),
        )
        .expect("mark cancelled");

        assert!(cancelled);
        assert_eq!(job_state(&connection, job_id), "cancelled");
    }

    #[test]
    fn mark_successful_ai_job_or_cancelled_marks_uncancelled_jobs_succeeded() {
        let connection = Connection::open_in_memory().expect("memory connection");
        let job_id = claimed_index_job(&connection);

        let cancelled = mark_successful_ai_job_or_cancelled(
            &connection,
            job_id,
            Some(42),
            Some("success summary"),
            Some("cancelled summary"),
        )
        .expect("mark succeeded");

        assert!(!cancelled);
        assert_eq!(job_state(&connection, job_id), "succeeded");
    }

    #[test]
    fn index_cursor_ledger_aborts_when_lease_is_lost() {
        // HIGH-3: when another worker reclaims the job (it leaves `running`), the ledger's cursor
        // write matches 0 rows. The ledger MUST surface that as an Err so the de-leased worker's
        // embed loop aborts instead of continuing to write the shared vector store.
        let dir = tempfile::tempdir().expect("tempdir");
        let paths = vault_core::config::project_paths_with_root(dir.path());
        let config = AppConfig {
            initialized: true,
            archive_mode: vault_core::ArchiveMode::Plaintext,
            ..AppConfig::default()
        };
        vault_core::ensure_archive_initialized(&paths, &config, None).expect("init archive");
        let connection =
            ai_archive_connection(&paths, &config, None).expect("intelligence connection");

        let job_id = claimed_index_job(&connection);
        // Reclaim: move the job out of `running` as a competing worker / stale-sweep would.
        connection
            .execute("UPDATE ai_jobs SET state = 'stale' WHERE id = ?1", [job_id])
            .expect("reclaim lease");

        let ledger = IndexCursorLedger {
            paths: paths.clone(),
            config: config.clone(),
            session_database_key: None,
            job_id,
        };
        let error = vault_core::IndexBackfillLedger::record(
            &ledger,
            vault_core::IndexBackfillProgress { next_history_id: 5, embedded_so_far: 4 },
        )
        .expect_err("lease loss must abort the backfill");
        assert!(error.to_string().contains("lost its queue lease"));
    }

    #[test]
    fn index_cursor_ledger_persists_progress_for_a_running_job() {
        // The happy path: a still-running job's cursor is persisted and the ledger returns Ok.
        let dir = tempfile::tempdir().expect("tempdir");
        let paths = vault_core::config::project_paths_with_root(dir.path());
        let config = AppConfig {
            initialized: true,
            archive_mode: vault_core::ArchiveMode::Plaintext,
            ..AppConfig::default()
        };
        vault_core::ensure_archive_initialized(&paths, &config, None).expect("init archive");
        let connection =
            ai_archive_connection(&paths, &config, None).expect("intelligence connection");
        let job_id = claimed_index_job(&connection);

        let ledger = IndexCursorLedger {
            paths: paths.clone(),
            config: config.clone(),
            session_database_key: None,
            job_id,
        };
        vault_core::IndexBackfillLedger::record(
            &ledger,
            vault_core::IndexBackfillProgress { next_history_id: 9, embedded_so_far: 8 },
        )
        .expect("running job cursor persists");

        match ai_queue::load_ai_job_payload(&connection, job_id).expect("payload") {
            ai_queue::AiJobPayload::Index { cursor, .. } => {
                assert_eq!(cursor.next_history_id, 9);
                assert_eq!(cursor.embedded_so_far, 8);
            }
            other => panic!("expected index payload, got {other:?}"),
        }
    }

    #[test]
    fn index_start_history_id_reads_cursor_and_defaults_non_index_payloads() {
        let index = ai_queue::AiJobPayload::Index {
            request: AiIndexRequest::default(),
            cursor: ai_queue::IndexBackfillCursor { next_history_id: 77, embedded_so_far: 5 },
        };
        assert_eq!(index_start_history_id(&index), 77);

        let assistant = ai_queue::AiJobPayload::Assistant {
            payload: ai_queue::AssistantJobPayload {
                request: vault_core::AiAssistantRequest {
                    question: "q".to_string(),
                    profile_id: None,
                    domain: None,
                },
                llm_provider_id: "llm".to_string(),
                embedding_provider_id: None,
            },
        };
        // A non-index payload defensively resumes from the beginning.
        assert_eq!(index_start_history_id(&assistant), 0);
    }
}
