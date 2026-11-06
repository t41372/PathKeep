//! Failure modes: the wrong owner is used, a claim loses its cursor, cancellation becomes success, or current provider settings replace a queued snapshot.
//! Worker-side AI queue and assistant/search orchestration.
//!
//! ## Responsibilities
//! - own index draining, heartbeat, per-job pause, continuation and cancellation
//! - retain ready indexes during updates; report cleanup warnings without failing builds
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
    load_unlocked_config, provider_config_for_request, queue_failure_from_error,
    resolve_provider_runtime, search_response_with_resolution_note,
    selected_embedding_provider_runtime, selected_optional_embedding_runtime, tokio_runtime,
};
use crate::job_runtime::{
    BackgroundJobControl, maybe_spawn_worker_pool, wait_unless_stopped, worker_stopped,
};
use anyhow::{Context, Result};
use std::{
    sync::{Arc, OnceLock},
    time::Duration,
};
use vault_core::{
    AiCapability, AiIndexReport, AiIndexRequest, AiIntegrationPreview,
    AiProviderConnectionTestReport, AiProviderConnectionTestRequest, AiProviderPurpose, AiQueueJob,
    AiQueueStatus, AiSearchRequest, AiSearchResponse, AppConfig, ReembedEstimate, ReembedScope,
    WorkingSetConfig, ai_queue, build_ai_index_with_control, ensure_ai_capability_enabled,
    estimate_reembed, preview_ai_integrations, semantic_search_history, test_provider_connection,
};

// Claims and physical index cleanup share one owner; a queued replacement never writes early.
static INDEX_EXECUTION: vault_core::ai::SemanticWorkGate = vault_core::ai::SemanticWorkGate::new();

type IndexJobEndedListener = Box<dyn Fn(vault_core::AiIndexJobEndedEvent) + Send + Sync>;
static INDEX_JOB_ENDED_LISTENER: OnceLock<IndexJobEndedListener> = OnceLock::new();

/// Connects background index attempts to the shell without giving workers an app handle.
/// Register once at startup, before any queue work can begin. Headless workers need no listener.
pub fn register_ai_index_job_ended_listener(
    listener: impl Fn(vault_core::AiIndexJobEndedEvent) + Send + Sync + 'static,
) -> Result<()> {
    INDEX_JOB_ENDED_LISTENER
        .set(Box::new(listener))
        .map_err(|_| anyhow::anyhow!("AI index job listener is already registered"))
}

const AI_JOB_CONTROL_POLL_INTERVAL: Duration = Duration::from_millis(500);
const AI_JOB_CONTROL_HEARTBEAT_EVERY: Duration = Duration::from_secs(30);

/// Starts the heartbeat and cancellation polling loop for one running AI job.
///
/// The worker needs this helper because index builds and assistant runs can outlive
/// the foreground caller. It keeps SQLite queue state fresh and lets UI-triggered
/// cancellation requests interrupt long-running jobs without guessing whether a
/// provider call is still alive.
pub(crate) fn start_ai_job_control(
    vault: &vault_core::Vault,
    job_id: i64,
) -> Arc<BackgroundJobControl> {
    let heartbeat = vault.clone();
    let cancellation = vault.clone();
    Arc::new(BackgroundJobControl::spawn(
        AI_JOB_CONTROL_POLL_INTERVAL,
        AI_JOB_CONTROL_HEARTBEAT_EVERY,
        move || ai_queue::heartbeat_ai_job(&heartbeat, job_id),
        move || ai_queue::ai_job_stop_requested(&cancellation, job_id),
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
    vault: vault_core::Vault,
    job_id: i64,
}

impl vault_core::IndexBackfillLedger for IndexCursorLedger {
    fn job_id(&self) -> Option<i64> {
        Some(self.job_id)
    }

    fn record(&self, progress: vault_core::IndexBackfillProgress) -> Result<()> {
        let cursor = ai_queue::IndexBackfillCursor {
            next_history_id: progress.next_history_id,
            embedded_so_far: progress.embedded_so_far,
            updated_so_far: progress.updated_so_far,
            // The determinate scan denominator (Change 1): the backfill reports the captured max on a
            // fresh build and 0 on a resume; `persist_index_cursor` preserves the stored value on 0.
            scan_target: progress.scan_target,
            // The honest embed-progress denominator (total candidate pages), same capture/preserve
            // contract as `scan_target`.
            embed_target: progress.embed_target,
            ..Default::default()
        };
        let summary = format!("Embedded {} row(s) so far.", progress.embedded_so_far);
        match ai_queue::persist_index_cursor(&self.vault, self.job_id, &cursor, Some(&summary))? {
            // A reclaimed lease (the stale-sweep moved this row out of `running`) means another
            // worker now owns the job. We MUST abort here so this de-leased worker stops embedding +
            // appending to the vector store while the new owner also writes it: the Err
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

/// Resolves the cumulative count of pages already embedded by this build from a claimed job's payload.
///
/// Seeds the backfill's running total so a reclaim continues the persisted `embedded_so_far` instead of
/// restarting it at this session's partial count (which would collapse the progress bar on resume). A
/// non-index payload starts at 0.
fn index_start_embedded(payload: &ai_queue::AiJobPayload) -> u64 {
    match payload {
        ai_queue::AiJobPayload::Index { cursor, .. } => cursor.embedded_so_far,
        _ => 0,
    }
}

/// Completes one claimed index job and persists the queue outcome.
///
/// This keeps queue bookkeeping in one place so index-build callers do not have
/// to manually coordinate success/failure writes, cancellation checks, and the
/// final user-visible summary.
pub(crate) fn complete_claimed_index_job(
    vault: &vault_core::Vault,
    paths: &vault_core::ProjectPaths,
    config: &AppConfig,
    session_database_key: Option<&str>,
    claimed: ai_queue::StoredAiJob,
    request: &AiIndexRequest,
) -> Result<AiIndexReport> {
    let job_id = claimed.id;
    let outcome =
        execute_claimed_index_job(vault, paths, config, session_database_key, claimed, request);
    // Read after all outcome and cleanup writes, even when execution returned an error.
    // A retry or service pause is an ended attempt too; the result alone cannot tell its state.
    if let Some(listener) = INDEX_JOB_ENDED_LISTENER.get() {
        match ai_queue::load_ai_job(vault, job_id) {
            Ok(job) => listener(vault_core::AiIndexJobEndedEvent { job_id, state: job.state }),
            Err(error) => log::warn!("Could not read ended AI index job {job_id}: {error:#}"),
        }
    }
    outcome
}

fn execute_claimed_index_job(
    vault: &vault_core::Vault,
    _paths: &vault_core::ProjectPaths,
    config: &AppConfig,
    _session_database_key: Option<&str>,
    claimed: ai_queue::StoredAiJob,
    request: &AiIndexRequest,
) -> Result<AiIndexReport> {
    // All nested ownership and recovery decisions need the same current settings snapshot as
    // provider selection. The base archive view can predate services added while unlocked.
    let execution_vault = vault.with_config(config.clone());
    let vault = &execution_vault;
    // Provider selection and Tokio startup are part of the claimed job execution. Keep them inside
    // the persisted outcome boundary: a preflight failure must not strand the row in `running`.
    let mut cleanup_warnings = Vec::new();
    let result = (|| {
        let pair = vault_core::semantic_index::chosen(config);
        let payload_pair = match &claimed.payload {
            ai_queue::AiJobPayload::Index { request, model: Some(model), .. } => {
                request.provider_id.as_ref().map(|id| vault_core::EmbeddingIndexRef {
                    provider_id: id.clone(),
                    model: model.clone(),
                })
            }
            _ => None,
        };
        if pair.is_none() || payload_pair != pair {
            ai_queue::cancel_ai_job(vault, claimed.id)?;
            return Err(vault_core::AiRunCancelled::new(
                "The index job no longer belongs to the chosen model.",
            )
            .into());
        }
        let provider = selected_embedding_provider_runtime(config, request.provider_id.as_deref())?;
        let runtime = tokio_runtime()?;
        // Resume from the persisted backfill watermark so a restart never re-embeds from scratch, and
        // seed the cumulative embedded count so the progress bar keeps climbing across the reclaim.
        let start_history_id = index_start_history_id(&claimed.payload);
        let start_embedded = index_start_embedded(&claimed.payload);
        let run_control = start_ai_job_control(vault, claimed.id);
        let state = vault_core::semantic_index::load_state(&vault.with_config(config.clone()))?;
        let kept: Vec<(String, String)> = state
            .chosen
            .into_iter()
            .chain(state.serving)
            .map(|pair| (pair.provider_id, pair.model))
            .collect();
        let control: Arc<dyn vault_core::AiRunControl> = run_control.clone();
        // Retired generations and their isolated provenance can be reclaimed independently.
        // Paused/queued lineages remain owned until their jobs end.
        if let Err(error) = vault_core::collect_index_generations(vault, true) {
            cleanup_warnings.push(format!("{error:#}"));
        }
        if let Err(error) =
            runtime.block_on(vault_core::forget_unconfigured_indexes(vault, &kept, Some(&control)))
        {
            cleanup_warnings.push(format!("{error:#}"));
        }
        let ledger: Arc<dyn vault_core::IndexBackfillLedger> =
            Arc::new(IndexCursorLedger { vault: vault.clone(), job_id: claimed.id });
        let result = runtime.block_on(build_ai_index_with_control(
            vault,
            &provider,
            request,
            Some(run_control.clone()),
            start_history_id,
            start_embedded,
            Some(ledger),
        ));
        run_control.shutdown();
        result
    })();

    let outcome = match result {
        Ok(mut report) => {
            report.job_id = Some(claimed.id);
            let summary =
                vault_core::ai::index_success_summary(report.indexed_items, report.updated_items);
            let stopped = mark_successful_ai_job_or_cancelled(
                vault,
                claimed.id,
                report.run_id,
                Some(summary.as_str()),
                Some("Index build cancelled from the UI."),
            )?;
            if !stopped && !request.clear_only && request.scope != ReembedScope::WorkingSet {
                let cleanup = (|| {
                    let current = load_unlocked_config(vault.paths())?;
                    let state =
                        vault_core::semantic_index::load_state(&vault.with_config(current))?;
                    let kept = state
                        .chosen
                        .into_iter()
                        .chain(state.serving)
                        .map(|p| (p.provider_id, p.model))
                        .collect::<Vec<_>>();
                    tokio_runtime()?
                        .block_on(vault_core::forget_unconfigured_indexes(vault, &kept, None))
                })();
                if let Err(error) = cleanup {
                    cleanup_warnings.push(format!("{error:#}"));
                }
            }
            Ok(report)
        }
        Err(error) => {
            let mut failure = queue_failure_from_error(&error);
            if failure.retryable {
                failure.retry_after_seconds *= 1u64 << claimed.attempt.saturating_sub(1).min(4);
            }
            ai_queue::mark_index_job_incomplete(vault, claimed.id)?;
            if ai_queue::ai_job_stop_requested(vault, claimed.id)? {
                let _ = ai_queue::mark_running_ai_job_cancelled(
                    vault,
                    claimed.id,
                    Some("Index build cancelled from the UI."),
                )?;
                if !load_unlocked_config(vault.paths())?.ai.job_queue_paused {
                    ai_queue::resume_paused_jobs(vault)?;
                }
            } else {
                ai_queue::mark_ai_job_failed(
                    vault,
                    claimed.id,
                    None,
                    &failure,
                    config.ai.job_queue_paused,
                )?;
                if !load_unlocked_config(vault.paths())?.ai.job_queue_paused {
                    ai_queue::resume_paused_jobs(vault)?;
                }
            }
            Err(error)
        }
    };
    // Every ended attempt retries collection, including cancellation and service pauses.
    // Active paused jobs retain their source lineage; readers retain their leased generation.
    if let Err(error) = vault_core::collect_index_generations(vault, true) {
        cleanup_warnings.push(format!("{error:#}"));
    }
    for warning in cleanup_warnings {
        if let Err(error) = ai_queue::fail_index_cleanup(vault, claimed.id, &warning) {
            log::warn!(
                "Could not record index cleanup warning for job {}: {error:#}; {warning}",
                claimed.id
            );
        }
    }
    outcome
}

fn mark_successful_ai_job_or_cancelled(
    vault: &vault_core::Vault,
    job_id: i64,
    run_id: Option<i64>,
    success_summary: Option<&str>,
    cancellation_summary: Option<&str>,
) -> Result<bool> {
    if ai_queue::load_ai_job(vault, job_id)?.state == "succeeded" {
        return Ok(false);
    }
    let cancelled = if ai_queue::ai_job_stop_requested(vault, job_id)? {
        true
    } else {
        !ai_queue::mark_ai_job_succeeded(vault, job_id, run_id, success_summary)?
    };
    if cancelled {
        let _ = ai_queue::mark_running_ai_job_cancelled(vault, job_id, cancellation_summary)?;
        if !load_unlocked_config(vault.paths())?.ai.job_queue_paused {
            ai_queue::resume_paused_jobs(vault)?;
        }
    }
    Ok(cancelled)
}

/// Starts background AI queue workers when there is queued work to drain.
///
/// The worker shell calls this after foreground actions that might have enqueued
/// new AI jobs. Keeping the spawn decision here ensures archive flows and queue
/// callers share the same paused-state and concurrency guard.
pub(crate) fn maybe_spawn_ai_queue_drain(
    vault: &vault_core::Vault,
    paths: &vault_core::ProjectPaths,
    config: &AppConfig,
    session_database_key: Option<&str>,
    queued_jobs: u32,
) {
    if config.ai.job_queue_paused || queued_jobs == 0 {
        return;
    }
    spawn_ai_queue_drain(
        vault,
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
    vault: &vault_core::Vault,
    paths: vault_core::ProjectPaths,
    desired_workers: usize,
    session_database_key: Option<String>,
) {
    let vault = vault.clone();
    maybe_spawn_worker_pool(
        "pathkeep-ai-queue",
        &AI_QUEUE_ACTIVE_WORKERS,
        desired_workers,
        move || loop {
            if worker_stopped() {
                break;
            }
            match drain_one_ai_queue_job(&vault, &paths, session_database_key.as_deref()) {
                Ok(true) => {}
                Ok(false) => match ai_queue::next_index_retry_at(&vault) {
                    Ok(Some(available_at)) => {
                        if load_unlocked_config(&paths)
                            .is_ok_and(|config| !config.ai.job_queue_paused)
                        {
                            let due = chrono::DateTime::parse_from_rfc3339(&available_at)
                                .context("parse AI retry availability");
                            let delay = match due {
                                Ok(due) => (due.with_timezone(&chrono::Utc) - chrono::Utc::now())
                                    .to_std()
                                    .unwrap_or_default()
                                    .min(Duration::from_secs(30)),
                                Err(error) => {
                                    eprintln!("PathKeep could not read AI retry time: {error:#}");
                                    break;
                                }
                            };
                            if !wait_unless_stopped(delay) {
                                break;
                            }
                        } else {
                            break;
                        }
                    }
                    Ok(None) => break,
                    Err(error) => {
                        eprintln!("PathKeep could not read AI retries: {error:#}");
                        break;
                    }
                },
                Err(error) => {
                    eprintln!("PathKeep could not drain AI queue work: {error:#}");
                    break;
                }
            }
        },
    );
}

pub(crate) fn drain_one_ai_queue_job(
    vault: &vault_core::Vault,
    paths: &vault_core::ProjectPaths,
    session_database_key: Option<&str>,
) -> Result<bool> {
    let _execution = INDEX_EXECUTION.enter();
    let config = load_unlocked_config(paths).context("load AI queue config")?;
    if !config.initialized || config.ai.job_queue_paused {
        return Ok(false);
    }
    let Some(job) = ai_queue::claim_next_ai_job(vault, 300).context("claim next AI queue job")?
    else {
        return Ok(false);
    };
    match job.payload.clone() {
        ai_queue::AiJobPayload::Index { request, .. } => {
            let _ = complete_claimed_index_job(
                vault,
                paths,
                &config,
                session_database_key,
                job,
                &request,
            );
        }
        // Claims skip these (`claim_next_ai_job`); an older archive's are retired when it opens.
        ai_queue::AiJobPayload::Assistant { .. } => {}
    }
    Ok(true)
}

/// Loads the persisted AI queue state for the current archive.
///
/// The returned snapshot doubles as a lazy "kick the drain loop if needed"
/// surface, which keeps queue work moving even when the UI only polls status.
pub fn load_ai_queue(
    vault: &vault_core::Vault,
    session_database_key: Option<&str>,
) -> Result<AiQueueStatus> {
    let paths = vault.paths().clone();
    let config = load_unlocked_config(&paths)?;
    let status = vault_core::ai_queue_status(&vault.with_config(config.clone()))?;
    // Kick the drain on RECOVERABLE work, not just `status.queued`. `queued` never includes a job
    // orphaned in `running` by a dead worker (app quit/crash), so gating on it left an interrupted
    // index build wedged forever — the drain that would reclaim it from its cursor was never spawned.
    // `recoverable_ai_jobs` also counts those orphans, so the periodic status poll self-heals the wedge.
    // It reads the same intelligence connection `ai_queue_status` just opened above, so propagating its
    // error (rather than swallowing it) keeps the two reads consistent.
    let drainable = vault_core::recoverable_ai_jobs(vault)?;
    maybe_spawn_ai_queue_drain(vault, &paths, &config, session_database_key, drainable);
    Ok(status)
}

/// Drains queued AI jobs up to the requested limit in the foreground caller.
///
/// This is the explicit "run now" path used by review/debug surfaces that want
/// deterministic progress instead of waiting for background workers.
pub fn run_ai_queue_jobs(
    vault: &vault_core::Vault,
    session_database_key: Option<&str>,
    max_jobs: Option<u32>,
) -> Result<AiQueueStatus> {
    let paths = vault.paths().clone();
    let config = load_unlocked_config(&paths)?;
    if config.ai.job_queue_paused {
        return vault_core::ai_queue_status(&vault.with_config(config.clone()));
    }

    let limit = max_jobs.unwrap_or(config.ai.job_queue_concurrency.max(1));
    for _ in 0..limit {
        let Some(_execution) = INDEX_EXECUTION.try_enter() else { break };
        let config = load_unlocked_config(&paths)?;
        if config.ai.job_queue_paused {
            break;
        }
        let Some(job) = ai_queue::claim_next_ai_job(vault, 300)? else {
            break;
        };
        match job.payload.clone() {
            ai_queue::AiJobPayload::Index { request, .. } => {
                let _ = complete_claimed_index_job(
                    vault,
                    &paths,
                    &config,
                    session_database_key,
                    job,
                    &request,
                );
            }
            ai_queue::AiJobPayload::Assistant { .. } => {}
        }
    }

    let current = load_unlocked_config(&paths)?;
    vault_core::ai_queue_status(&vault.with_config(current))
}

/// Marks one AI job as replayable and restarts the drain loop if it became queued again.
pub fn replay_ai_job(
    vault: &vault_core::Vault,
    session_database_key: Option<&str>,
    job_id: i64,
) -> Result<AiQueueJob> {
    let paths = vault.paths().clone();
    let config = load_unlocked_config(&paths)?;
    if let ai_queue::AiJobPayload::Index { request, model, .. } =
        ai_queue::load_ai_job_payload(vault, job_id)?
    {
        let chosen = vault_core::semantic_index::chosen(&config)
            .context("Select an embedding model before continuing its build.")?;
        if request.provider_id.as_deref().is_some_and(|id| id != chosen.provider_id)
            || model.as_ref().is_some_and(|model| model != &chosen.model)
            || model.is_none()
        {
            anyhow::bail!(
                "This job belongs to a previous model. Build the model chosen in Settings instead."
            );
        }
    }
    let job = ai_queue::replay_ai_job(vault, job_id, config.ai.job_queue_paused)?;
    maybe_spawn_ai_queue_drain(
        vault,
        &paths,
        &config,
        session_database_key,
        if job.state == "queued" { 1 } else { 0 },
    );
    Ok(job)
}

/// Pauses one index job without discarding its last durable cursor.
pub fn pause_ai_job(vault: &vault_core::Vault, job_id: i64) -> Result<AiQueueJob> {
    let _config = load_unlocked_config(vault.paths())?;
    ai_queue::pause_ai_job(vault, job_id)
}

/// Cancels one queued or retryable AI job without touching finished run artifacts.
pub fn cancel_ai_job(vault: &vault_core::Vault, job_id: i64) -> Result<AiQueueJob> {
    let paths = vault.paths().clone();
    let _config = load_unlocked_config(&paths)?;
    let job = ai_queue::cancel_ai_job(vault, job_id)?;
    // A paused or queued build ends here without a pass of its own to clean up after it, so its
    // lineage would hold its disk until the next job.
    vault_core::collect_index_generations_in_background(vault);
    Ok(job)
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
/// wholesale by an incremental pass and stay broken forever. A full rebuild embeds every page into a
/// new copy whatever the rows say (`indexing/rebuild.rs`) and puts it in place of the broken files
/// once complete, so a 0-vector index genuinely self-heals. The returned [`AiIndexReport`] is the new
/// job's acknowledgement, exactly like a manual build.
pub fn reset_ai_index_build(
    vault: &vault_core::Vault,
    session_database_key: Option<&str>,
    request: &AiIndexRequest,
) -> Result<AiIndexReport> {
    let paths = vault.paths().clone();
    let _config = load_unlocked_config(&paths)?;
    let cleared = ai_queue::clear_index_jobs(vault)?;
    vault_core::collect_index_generations_in_background(vault);
    // Drop the connection before `build_ai_index_now` opens its own (it re-resolves paths/config).
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
    let mut report = enqueue_index_request(vault, session_database_key, &rebuild, true)?;
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
    vault: &vault_core::Vault,
    session_database_key: Option<&str>,
    request: &AiIndexRequest,
) -> Result<AiIndexReport> {
    enqueue_index_request(vault, session_database_key, request, false)
}

fn enqueue_index_request(
    vault: &vault_core::Vault,
    session_database_key: Option<&str>,
    request: &AiIndexRequest,
    reset: bool,
) -> Result<AiIndexReport> {
    let paths = vault.paths().clone();
    let config = load_unlocked_config(&paths)?;
    // Consent gate at the firing site: a BUILD/re-embed enqueues an embedding job (provider
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
    let current = vault.with_config(config.clone());
    let queued = if request.clear_only || reset {
        ai_queue::enqueue_index_job(&current, &queued_request, config.ai.job_queue_paused)?
    } else {
        ai_queue::request_index_job(&current, &queued_request, false)?
            .context("a user build must return a job")?
    };

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
    maybe_spawn_ai_queue_drain(vault, &paths, &config, session_database_key, 1);
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

/// Counts what a re-embed run for one scope would index (W-AI-9 Sub-block D, 05 §7), shown before it
/// starts. Read-only: it reads only the bounded working set or the `COUNT(*)` of unique pages, so it
/// never loads a model, embeds, or touches the network.
pub fn estimate_reembed_now(
    vault: &vault_core::Vault,
    scope: ReembedScope,
) -> Result<ReembedEstimate> {
    estimate_reembed(
        vault,
        &WorkingSetConfig::default(),
        chrono::Utc::now().timestamp_millis(),
        scope,
    )
}

/// Runs semantic search and degrades to lexical recall when embeddings are unavailable.
pub fn search_ai_history(
    vault: &vault_core::Vault,
    session_database_key: Option<&str>,
    request: &AiSearchRequest,
) -> Result<AiSearchResponse> {
    let paths = vault.paths().clone();
    let config = load_unlocked_config(&paths)?;
    let embedding_provider = match selected_optional_embedding_runtime(vault, &config) {
        Ok(provider) => provider,
        Err(error) => {
            let lexical =
                run_semantic_search(vault, &paths, &config, session_database_key, None, request)?;
            return Ok(search_response_with_resolution_note(lexical, Some(error)));
        }
    };
    run_semantic_search(
        vault,
        &paths,
        &config,
        session_database_key,
        embedding_provider.as_ref(),
        request,
    )
}

/// Executes one semantic-search request using the optional embedding provider.
fn run_semantic_search(
    vault: &vault_core::Vault,
    paths: &vault_core::ProjectPaths,
    config: &AppConfig,
    session_database_key: Option<&str>,
    embedding_provider: Option<&vault_core::AiProviderRuntime>,
    request: &AiSearchRequest,
) -> Result<AiSearchResponse> {
    tokio_runtime()?.block_on(semantic_search_history(
        vault,
        paths,
        config,
        session_database_key,
        embedding_provider,
        request,
    ))
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

/// Asks a service which models it has, for the model field in Settings.
///
/// The form's values are used whether or not the service is saved yet. The key is the one typed
/// in the form; when that is empty and the service is saved, its stored key.
pub fn list_ai_provider_models(
    request: &vault_core::AiProviderModelsRequest,
) -> Result<Vec<vault_core::AiProviderModel>> {
    let typed = request.api_key.as_deref().map(str::trim).filter(|key| !key.is_empty());
    let stored = match (typed, request.provider_id.as_deref()) {
        (None, Some(provider_id)) => vault_platform::keyring_get_provider_api_key(provider_id)?,
        _ => None,
    };
    let key = typed.map(str::to_owned).or(stored).filter(|key| !key.trim().is_empty());
    tokio_runtime()?.block_on(vault_core::list_provider_models(
        &request.request_format,
        request.base_url.as_deref().map(str::trim).filter(|url| !url.is_empty()),
        key.as_deref(),
        request.api_root_only,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use vault_core::test_support::native::TestDatabase;
    use vault_store::Db;

    fn claimed_index_job(connection: &TestDatabase) -> i64 {
        let queued =
            ai_queue::enqueue_index_job(&connection.vault, &AiIndexRequest::default(), false)
                .expect("enqueue index job");
        ai_queue::claim_ai_job_by_id(&connection.vault, queued.id, 300)
            .expect("claim index job")
            .expect("claimed index job");
        queued.id
    }

    #[test]
    fn index_cursor_ledger_aborts_when_lease_is_lost() {
        // When another worker reclaims the job (it leaves `running`), the ledger's cursor
        // write matches 0 rows. The ledger MUST surface that as an Err so the de-leased worker's
        // embed loop aborts instead of continuing to write the shared vector store.
        let dir = tempfile::tempdir().expect("tempdir");
        let paths = vault_core::config::project_paths_with_root(dir.path());
        let config = AppConfig {
            initialized: true,
            archive_mode: vault_core::ArchiveMode::Plaintext,
            ..AppConfig::default()
        };
        vault_core::config::ensure_paths(&paths).expect("paths");
        let vault = vault_core::Vault::open_temp().unwrap().with_config(config.clone());
        let paths = vault.paths().clone();
        vault_core::save_config(&paths, &config).unwrap();
        let connection = TestDatabase::new(&vault, Db::Intelligence);

        let job_id = claimed_index_job(&connection);
        // Reclaim: move the job out of `running` as a competing worker / stale-sweep would.
        connection
            .execute("UPDATE ai_jobs SET state = 'stale' WHERE id = ?1", [job_id])
            .expect("reclaim lease");

        let ledger = IndexCursorLedger { vault: vault.clone(), job_id };
        let error = vault_core::IndexBackfillLedger::record(
            &ledger,
            vault_core::IndexBackfillProgress {
                updated_so_far: 0,
                next_history_id: 5,
                embedded_so_far: 4,
                ..Default::default()
            },
        )
        .expect_err("lease loss must abort the backfill");
        assert!(error.to_string().contains("lost its queue lease"));
    }
}

#[cfg(test)]
mod generation_success_tests {
    use super::*;
    use vault_core::test_support::native::TestDatabase;
    use vault_core::{AiProviderConfig, AiProviderPurpose, AiProviderRuntime};
    use vault_store::Db;

    /// Failure mode: the worker's second stop check cancels success after pointer publication.
    #[test]
    fn final_worker_bookkeeping_cannot_cancel_a_committed_index_pass() {
        let vault = vault_core::Vault::open_temp().unwrap();
        let db = TestDatabase::new(&vault, Db::Intelligence);
        let job = ai_queue::enqueue_index_job(&vault, &AiIndexRequest::default(), false).unwrap();
        ai_queue::claim_ai_job_by_id(&vault, job.id, 300).unwrap().unwrap();
        let provider = AiProviderRuntime {
            config: AiProviderConfig {
                id: "generation-test".into(),
                default_model: "model".into(),
                purpose: AiProviderPurpose::Embedding,
                enabled: true,
                base_url: Some("http://127.0.0.1:1/v1".into()),
                ..Default::default()
            },
            api_key: None,
        };
        let ledger: Arc<dyn vault_core::IndexBackfillLedger> =
            Arc::new(IndexCursorLedger { vault: vault.clone(), job_id: job.id });
        let report = tokio_runtime()
            .unwrap()
            .block_on(build_ai_index_with_control(
                &vault,
                &provider,
                &AiIndexRequest::default(),
                None,
                0,
                0,
                Some(ledger),
            ))
            .unwrap();
        db.execute("UPDATE ai_jobs SET stop_requested=1 WHERE id=?1", [job.id]).unwrap();
        assert!(
            !mark_successful_ai_job_or_cancelled(
                &vault,
                job.id,
                report.run_id,
                Some("complete"),
                Some("cancelled")
            )
            .unwrap()
        );
        assert_eq!(ai_queue::load_ai_job(&vault, job.id).unwrap().state, "succeeded");
    }
}

#[cfg(test)]
mod config_tests;
