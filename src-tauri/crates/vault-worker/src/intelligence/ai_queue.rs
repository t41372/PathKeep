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
    AiAssistantRequest, AiAssistantResponse, AiCapability, AiIndexReport, AiIndexRequest,
    AiIntegrationPreview, AiProviderConnectionTestReport, AiProviderConnectionTestRequest,
    AiProviderPurpose, AiQueueJob, AiQueueStatus, AiSearchRequest, AiSearchResponse, AppConfig,
    ReembedEstimate, ReembedScope, WorkingSetConfig, ai_queue,
    answer_history_question_with_control, build_ai_index_with_control,
    ensure_ai_capability_enabled, estimate_reembed, load_assistant_run_response,
    preview_ai_integrations, semantic_search_history, test_provider_connection,
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
            // The determinate scan denominator (Change 1): the backfill reports the captured max on a
            // fresh build and 0 on a resume; `persist_index_cursor` preserves the stored value on 0.
            scan_target: progress.scan_target,
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

/// Recovers a WEDGED or DEGRADED semantic-index build: clears the stuck index job(s) and re-enqueues
/// a clean FULL REBUILD.
///
/// The user-facing reset/retry path (F2). It first moves EVERY non-terminal `index-build` /
/// `index-clear` job to a terminal cancelled state ([`ai_queue::clear_index_jobs`]) so a stuck job can
/// never collide with the fresh one, then enqueues a build through [`build_ai_index_now`] (which
/// applies the SemanticIndex consent gate, persists the job, and kicks the drain).
///
/// Recovery COERCES the request to a full rebuild (`full_rebuild = true`, `clear_only = false`,
/// `scope = Full`) regardless of what the caller passed — only the provider selection is carried
/// over. This is deliberate and load-bearing: incremental dedup decides `needs_embedding` from the
/// SQLite `ai_embeddings` rows (`candidates.rs`), NOT from the `.pkvec`. So the DEGRADED case — SQLite
/// recorded "embedded" but the vector store is empty/torn (`IndexVectorsMissing`) — would be skipped
/// wholesale by an incremental pass and stay broken forever. A full rebuild wipes the provider's
/// `ai_embeddings` rows AND the `.pkvec`/derived planes (`backfill.rs`), so every page is re-embedded
/// and a 0-vector index genuinely self-heals. The returned [`AiIndexReport`] is the new job's
/// acknowledgement, exactly like a manual build.
pub fn reset_ai_index_build(
    session_database_key: Option<&str>,
    request: &AiIndexRequest,
) -> Result<AiIndexReport> {
    let paths = vault_core::project_paths()?;
    let config = load_unlocked_config(&paths)?;
    let connection = ai_archive_connection(&paths, &config, session_database_key)?;
    let cleared = ai_queue::clear_index_jobs(&connection)?;
    // Drop the connection before `build_ai_index_now` opens its own (it re-resolves paths/config).
    drop(connection);
    // Coerce to a full rebuild so the degraded 0-vector index actually re-embeds (see fn docs). Only
    // the provider selection survives from the caller; the rebuild owns the rest. This cannot be
    // subverted by a caller that forgot to ask for a full rebuild (the FE, the dev bridge, or a
    // future caller) — reset is ALWAYS a clean rebuild.
    let rebuild = AiIndexRequest {
        provider_id: request.provider_id.clone(),
        full_rebuild: true,
        clear_only: false,
        limit: None,
        scope: ReembedScope::Full,
    };
    let mut report = build_ai_index_now(session_database_key, &rebuild)?;
    report.notes.insert(
        0,
        format!(
            "Reset the semantic index: cleared {cleared} stuck job(s) and re-enqueued a clean full rebuild."
        ),
    );
    Ok(report)
}

/// Enqueues an AI index build and starts background work when the queue is active.
pub fn build_ai_index_now(
    session_database_key: Option<&str>,
    request: &AiIndexRequest,
) -> Result<AiIndexReport> {
    let paths = vault_core::project_paths()?;
    let config = load_unlocked_config(&paths)?;
    // Consent gate at the firing site (M-3): a BUILD/re-embed enqueues an embedding job (provider
    // egress + ~59 GB of derived vectors), so it requires the master AI switch AND the semantic-index
    // sub-flag — mirroring the auto-index gate in `archive_flows.rs`. A user with master ON but Smart
    // search deliberately OFF must NOT be able to trigger a full 14.4M re-embed. A `clear_only` job is
    // pure cleanup (no embedding, no egress), so it stays allowed even with the sub-flag off — turning
    // semantic search off and reclaiming the derived vectors is a legitimate locked-consent action.
    if !request.clear_only {
        ensure_ai_capability_enabled(&config, AiCapability::SemanticIndex)?;
    }
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

    /// A plaintext archive + config whose embedding provider is the built-in in-app static tier (F1),
    /// so `complete_claimed_index_job` resolves the REAL in-memory static engine with no network.
    ///
    /// `cfg(coverage)`: the in-app static engine only has its TINY in-memory stub under the coverage
    /// cfg (vault-core as a plain dependency builds the disk-loading engine, which would need the model
    /// downloaded). The established pattern for real-build worker tests is the coverage gate.
    #[cfg(coverage)]
    fn static_indexed_env() -> (vault_core::ProjectPaths, AppConfig, tempfile::TempDir) {
        let dir = tempfile::tempdir().expect("tempdir");
        let paths = vault_core::config::project_paths_with_root(dir.path());
        let provider = vault_core::built_in_static_embedding_provider();
        let config = AppConfig {
            initialized: true,
            archive_mode: vault_core::ArchiveMode::Plaintext,
            git_enabled: false,
            ai: vault_core::AiSettings {
                enabled: true,
                semantic_index_enabled: true,
                embedding_provider_id: Some(provider.id.clone()),
                embedding_providers: vec![provider],
                ..vault_core::AiSettings::default()
            },
            ..AppConfig::default()
        };
        vault_core::ensure_archive_initialized(&paths, &config, None).expect("init archive");
        (paths, config, dir)
    }

    /// Seeds one canonical visible page (run + profile + url + visit) on the attached archive.
    #[cfg(coverage)]
    fn seed_canonical_page(connection: &Connection, id: i64, url: &str, title: &str) {
        connection
            .execute(
                "INSERT OR IGNORE INTO archive.runs (id, run_type, trigger, started_at, timezone, status, profile_scope_json, warnings_json, stats_json, due_only)
                 VALUES (1, 'backup', 'test', '2026-06-21T00:00:00Z', 'UTC', 'success', '[]', '[]', '{}', 0)",
                [],
            )
            .expect("seed run");
        connection
            .execute(
                "INSERT OR IGNORE INTO archive.source_profiles (id, browser_kind, browser_version, profile_name, profile_path, discovered_at, enabled, profile_key, updated_at)
                 VALUES (1, 'chrome', 'test', 'Default', '/tmp/Default', '2026-06-21T00:00:00Z', 1, 'chrome:Default', '2026-06-21T00:00:00Z')",
                [],
            )
            .expect("seed profile");
        let visit_ms = 1_700_000_000_000_i64 + id;
        connection
            .execute(
                "INSERT INTO archive.urls
                 (id, url, title, visit_count, typed_count, first_visit_ms, first_visit_iso, last_visit_ms, last_visit_iso, source_profile_id, created_by_run_id, source_url_id, hidden, payload_hash, recorded_at)
                 VALUES (?1, ?2, ?3, 1, 0, ?4, '2026-06-21T00:00:00Z', ?4, '2026-06-21T00:00:00Z', 1, 1, ?1, 0, ?5, '2026-06-21T00:00:00Z')",
                rusqlite::params![id, url, title, visit_ms, format!("payload-{id}")],
            )
            .expect("seed url");
        connection
            .execute(
                "INSERT INTO archive.visits
                 (id, url_id, source_visit_id, visit_time_ms, visit_time_iso, transition_type, visit_duration_ms, source_profile_id, created_by_run_id, from_visit, is_known_to_sync, visited_link_id, external_referrer_url, app_id, event_fingerprint, payload_hash, recorded_at)
                 VALUES (?1, ?1, ?2, ?3, '2026-06-21T00:00:00Z', 805306368, 0, 1, 1, NULL, 1, 0, NULL, NULL, ?4, ?5, '2026-06-21T00:00:00Z')",
                rusqlite::params![id, id.to_string(), visit_ms, format!("fp-{id}"), format!("payload-{id}")],
            )
            .expect("seed visit");
    }

    #[cfg(coverage)]
    fn index_request_from(payload: &ai_queue::AiJobPayload) -> AiIndexRequest {
        match payload {
            ai_queue::AiJobPayload::Index { request, .. } => request.clone(),
            other => panic!("expected an index payload, got {other:?}"),
        }
    }

    /// Sets the keyring (and optionally project-root) test overrides for the duration of a guard.
    ///
    /// Resolving a provider runtime hits the native keyring through `vault-platform`; pointing it at a
    /// temp dir keeps these tests offline + deterministic. Returns the originals so the caller restores
    /// them. The `lock_env` MutexGuard the caller holds serializes env-mutating tests.
    fn set_keyring_override(dir: &std::path::Path) -> Option<std::ffi::OsString> {
        let original = std::env::var_os(crate::tests::TEST_KEYRING_OVERRIDE_ENV);
        let keyring_dir = dir.join("test-keyring");
        unsafe {
            std::env::set_var(crate::tests::TEST_KEYRING_OVERRIDE_ENV, &keyring_dir);
        }
        original
    }

    fn restore_keyring_override(original: Option<std::ffi::OsString>) {
        unsafe {
            match original {
                Some(value) => std::env::set_var(crate::tests::TEST_KEYRING_OVERRIDE_ENV, value),
                None => std::env::remove_var(crate::tests::TEST_KEYRING_OVERRIDE_ENV),
            }
        }
    }

    #[cfg(coverage)]
    #[test]
    fn real_worker_resume_finishes_the_exact_id_set_from_the_persisted_cursor() {
        // HEADLINE (F2 + F4): a limit-bounded index job stops mid-corpus and persists its cursor into
        // the payload; after the job is left STALE (an interrupted worker), it is RE-CLAIMED and the
        // REAL worker `complete_claimed_index_job` path resumes FROM THE PAYLOAD CURSOR (never a
        // hand-passed integer) and runs to completion. The final `.pkvec` holds the EXACT id-set with
        // no dup / no miss and non-zero bytes — exercised through the real in-memory static engine.
        let _guard = crate::tests::lock_env();
        let (paths, config, dir) = static_indexed_env();
        let original_keyring = set_keyring_override(dir.path());
        let connection = ai_archive_connection(&paths, &config, None).expect("connection");
        // Four distinct pages; a per-run limit of 2 forces a mid-corpus stop after the first run.
        for id in 1..=4 {
            seed_canonical_page(&connection, id, &format!("https://example.com/{id}"), "kestrel");
        }
        let provider = &config.ai.embedding_providers[0];

        // RUN 1: claim + complete with limit 2 → embeds 2 pages, persists the cursor, marks succeeded.
        let request = AiIndexRequest { limit: Some(2), ..AiIndexRequest::default() };
        let queued = ai_queue::enqueue_index_job(&connection, &request, false).expect("enqueue");
        let claimed = ai_queue::claim_ai_job_by_id(&connection, queued.id, 300)
            .expect("claim")
            .expect("claimable");
        let first = complete_claimed_index_job(
            &connection,
            &paths,
            &config,
            None,
            claimed.clone(),
            &index_request_from(&claimed.payload),
        )
        .expect("first partial build");
        assert_eq!(first.indexed_items, 2, "the limit stops the build at 2 rows");
        let store =
            vault_core::VectorStore::for_provider(&paths, &provider.id, &provider.default_model);
        assert_eq!(store.count().expect("count"), 2, "two vectors after the partial run");

        // The cursor advanced PAST the origin and is durable in the payload (real progress, not 0).
        match ai_queue::load_ai_job_payload(&connection, queued.id).expect("payload") {
            ai_queue::AiJobPayload::Index { cursor, .. } => {
                assert!(
                    cursor.next_history_id > 0,
                    "the resume cursor is persisted past the origin"
                );
            }
            other => panic!("expected index payload, got {other:?}"),
        }

        // Simulate an interrupted worker: the job is left STALE (lease lost) with its payload cursor
        // intact, exactly the state the stale-sweep produces for a crashed run.
        connection
            .execute(
                "UPDATE ai_jobs SET state = 'stale', heartbeat_at = NULL, lease_owner = NULL, lease_expires_at = NULL, finished_at = NULL WHERE id = ?1",
                [queued.id],
            )
            .expect("leave stale");

        // RUN 2 (RESUME): the real claim → complete path reads the cursor from the PAYLOAD and finishes.
        let reclaimed = ai_queue::claim_next_ai_job(&connection, 300)
            .expect("re-claim")
            .expect("stale job is re-claimable within budget");
        assert_eq!(reclaimed.id, queued.id);
        // The resume watermark comes from the payload, NOT a hand-passed integer.
        assert!(index_start_history_id(&reclaimed.payload) > 0);
        let second = complete_claimed_index_job(
            &connection,
            &paths,
            &config,
            None,
            reclaimed.clone(),
            &index_request_from(&reclaimed.payload),
        )
        .expect("resumed build");
        assert_eq!(second.indexed_items, 2, "the resume embeds the remaining 2 rows");

        // The final `.pkvec` holds the EXACT id-set (4 distinct pages), no dup / no miss, non-zero.
        let records = store.read_all().expect("read all");
        assert_eq!(records.len(), 4, "exactly four deduped page vectors after resume");
        let unique: std::collections::HashSet<u64> = records.iter().map(|(key, _)| *key).collect();
        assert_eq!(unique.len(), 4, "no duplicate content keys (no dup)");
        assert!(
            records.iter().all(|(_, vector)| vector.iter().any(|c| *c != 0.0)),
            "non-zero vectors"
        );
        assert!(store.path().metadata().expect("meta").len() > 0, "non-zero .pkvec bytes");

        restore_keyring_override(original_keyring);
    }

    #[test]
    fn reset_ai_index_build_clears_stuck_jobs_and_reenqueues() {
        // HEADLINE (F2): a wedged build is recovered — `reset_ai_index_build` clears the stuck index
        // job(s) and re-enqueues a clean one, so the queue is no longer jammed by the old job. Runs
        // against the GLOBAL project root (the reset path resolves `project_paths()` itself), with the
        // queue PAUSED so the re-enqueue stages a job without spawning a background drain.
        let _guard = crate::tests::lock_env();
        let dir = tempfile::tempdir().expect("tempdir");
        let original_root = std::env::var_os(crate::tests::PROJECT_ROOT_OVERRIDE_ENV);
        let original_keyring = set_keyring_override(dir.path());
        unsafe {
            std::env::set_var(crate::tests::PROJECT_ROOT_OVERRIDE_ENV, dir.path());
        }

        let paths = vault_core::project_paths().expect("project paths");
        let config = AppConfig {
            initialized: true,
            archive_mode: vault_core::ArchiveMode::Plaintext,
            git_enabled: false,
            ai: vault_core::AiSettings {
                enabled: true,
                semantic_index_enabled: true,
                // Paused so `build_ai_index_now` STAGES the fresh job without spawning a drain thread.
                job_queue_paused: true,
                ..vault_core::AiSettings::default()
            },
            ..AppConfig::default()
        };
        vault_core::ensure_archive_initialized(&paths, &config, None).expect("init archive");
        vault_core::save_config(&paths, &config).expect("persist config");
        let connection = ai_archive_connection(&paths, &config, None).expect("connection");
        // A stuck index job left in `stale` (the interrupted state the reset must clear).
        let stuck = ai_queue::enqueue_index_job(&connection, &AiIndexRequest::default(), false)
            .expect("enqueue stuck");
        connection
            .execute("UPDATE ai_jobs SET state = 'stale' WHERE id = ?1", [stuck.id])
            .expect("wedge it");

        let report = reset_ai_index_build(None, &AiIndexRequest::default()).expect("reset");
        assert!(
            report.notes.iter().any(|note| note.contains("Reset the semantic index")),
            "the reset reports what it cleared: {:?}",
            report.notes
        );
        // The stuck job is now terminal (cancelled), and a fresh job was staged (paused) in its place.
        assert_eq!(job_state(&connection, stuck.id), "cancelled");
        let fresh: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM ai_jobs WHERE job_type = 'index-build' AND state = 'paused'",
                [],
                |row| row.get(0),
            )
            .expect("fresh count");
        assert_eq!(fresh, 1, "a clean build was re-enqueued (staged paused)");
        // The re-enqueued job is a FULL REBUILD even though the caller passed the default request
        // (full_rebuild=false). Reset COERCES recovery to a full rebuild so the degraded 0-vector case
        // re-embeds every page instead of being skipped by incremental dedup (see fn docs). The
        // persisted payload is the source of truth the drain will replay.
        let payload_json: String = connection
            .query_row(
                "SELECT payload_json FROM ai_jobs \
                 WHERE job_type = 'index-build' AND state = 'paused'",
                [],
                |row| row.get(0),
            )
            .expect("fresh payload");
        assert!(
            payload_json.contains("\"fullRebuild\":true"),
            "reset stages a FULL rebuild so a 0-vector index self-heals: {payload_json}"
        );

        restore_keyring_override(original_keyring);
        unsafe {
            match original_root {
                Some(value) => std::env::set_var(crate::tests::PROJECT_ROOT_OVERRIDE_ENV, value),
                None => std::env::remove_var(crate::tests::PROJECT_ROOT_OVERRIDE_ENV),
            }
        }
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
            vault_core::IndexBackfillProgress {
                next_history_id: 5,
                embedded_so_far: 4,
                ..Default::default()
            },
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
            vault_core::IndexBackfillProgress {
                next_history_id: 9,
                embedded_so_far: 8,
                scan_target: 12,
            },
        )
        .expect("running job cursor persists");

        match ai_queue::load_ai_job_payload(&connection, job_id).expect("payload") {
            ai_queue::AiJobPayload::Index { cursor, .. } => {
                assert_eq!(cursor.next_history_id, 9);
                assert_eq!(cursor.embedded_so_far, 8);
                // The worker ledger forwards the captured scan denominator into the persisted cursor.
                assert_eq!(cursor.scan_target, 12);
            }
            other => panic!("expected index payload, got {other:?}"),
        }
    }

    #[test]
    fn index_start_history_id_reads_cursor_and_defaults_non_index_payloads() {
        let index = ai_queue::AiJobPayload::Index {
            request: AiIndexRequest::default(),
            cursor: ai_queue::IndexBackfillCursor {
                next_history_id: 77,
                embedded_so_far: 5,
                ..Default::default()
            },
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
