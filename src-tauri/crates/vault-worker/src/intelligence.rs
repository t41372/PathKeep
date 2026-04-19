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
    selected_optional_embedding_runtime, tokio_runtime,
};
use crate::job_runtime::{BackgroundJobControl, maybe_spawn_worker_pool};
use anyhow::{Context, Result};
use chrono::Local;
use serde_json::json;
use std::{
    sync::{Arc, atomic::AtomicUsize},
    time::{Duration, Instant},
};
use vault_core::{
    ActivityMix, ActivityMixTrend, AiAssistantRequest, AiAssistantResponse, AiIndexReport,
    AiIndexRequest, AiIntegrationPreview, AiProviderConnectionTestReport,
    AiProviderConnectionTestRequest, AiProviderPurpose, AiQueueJob, AiQueueStatus, AiSearchRequest,
    AiSearchResponse, AppConfig, BreadthIndex, BrowserDiff, CategoryFilteredDateRangeRequest,
    CompareSet, CoreIntelligencePrimaryOverview, CoreIntelligenceQueueReport,
    CoreIntelligenceRebuildReport, CoreIntelligenceRebuildRequest,
    CoreIntelligenceSecondaryOverview, CoreIntelligenceSectionResult,
    CoreIntelligenceSectionTiming, CoreIntelligenceSectionWindow, DayInsights, DayInsightsRequest,
    DigestSummary, DiscoveryTrend, DomainDeepDive, DomainDeepDiveRequest, DomainTrend,
    DomainTrendRequest, EngineRanking, EntityExplanationRequest, Explanation, FrictionSignal,
    GranularityDateRangeRequest, HabitPattern, HubPage, IntelligenceEmbedCardPayload,
    IntelligenceEmbedCardsRequest, IntelligenceLocalHostBuildResult, IntelligenceLocalHostPreview,
    IntelligenceLocalHostRequest, IntelligencePublicSnapshot, IntelligenceRuntimeSnapshot,
    IntelligenceWidgetSnapshot, InterruptedHabit, NavigationPath, ObservedInteraction,
    OnThisDayEntry, PagedDateRangeRequest, PathFlow, PathFlowRequest, ProfileScopedRequest,
    QueryFamilyDetail, QueryFamilyDetailRequest, QueryFamilyResult, RefindExplanation, RefindPage,
    RefindPageDetail, RefindPageDetailRequest, RefindPagesRequest, ReopenedInvestigation,
    RhythmHeatmap, ScopedDateRangeRequest, SearchConcept, SearchEffectiveness,
    SearchEffectivenessRequest, SearchEngineRule, SearchEngineRuleInput, SearchQueryListRequest,
    SearchQueryListResult, SearchTrailQueryRequest, SessionDetail, SessionListResult, StableSource,
    TopSearchConceptsRequest, TopSite, TopSitesRequest, TrailDetail, TrailListResult, ai_queue,
    answer_history_question_with_control, build_ai_index_with_control,
    build_core_intelligence_section_meta, cancel_intelligence_job, execute_enrichment_job_by_id,
    intelligence, intelligence_job_stop_requested,
    intelligence_runtime::{
        DAILY_ROLLUP_JOB_TYPE, STRUCTURAL_REBUILD_JOB_TYPE, VISIT_DERIVE_JOB_TYPE,
        claim_core_intelligence_job, enqueue_deterministic_rebuild_job,
        mark_intelligence_job_failed, mark_intelligence_job_succeeded,
        mark_running_intelligence_job_cancelled, next_queued_enrichment_job,
        next_queued_intelligence_job, update_intelligence_job_artifact,
    },
    load_assistant_run_response, load_intelligence_runtime, preview_ai_integrations,
    retry_intelligence_job, semantic_search_history, test_provider_connection,
};

static AI_QUEUE_ACTIVE_WORKERS: AtomicUsize = AtomicUsize::new(0);
static INTELLIGENCE_PRIORITY_WORKERS: AtomicUsize = AtomicUsize::new(0);
static INTELLIGENCE_ENRICHMENT_WORKERS: AtomicUsize = AtomicUsize::new(0);

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
    spawn_ai_queue_drain(
        paths.clone(),
        config.ai.job_queue_concurrency.max(1) as usize,
        session_database_key.map(ToOwned::to_owned),
    );
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
    spawn_intelligence_queue_drain(
        paths.clone(),
        config.ai.job_queue_concurrency.max(1) as usize,
        session_database_key.map(ToOwned::to_owned),
    );
}

fn spawn_ai_queue_drain(
    paths: vault_core::ProjectPaths,
    desired_workers: usize,
    session_database_key: Option<String>,
) {
    maybe_spawn_worker_pool(
        "pathkeep-ai-queue",
        &AI_QUEUE_ACTIVE_WORKERS,
        desired_workers,
        move || {
            loop {
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
            }
        },
    );
}

fn spawn_intelligence_queue_drain(
    paths: vault_core::ProjectPaths,
    desired_workers: usize,
    session_database_key: Option<String>,
) {
    maybe_spawn_worker_pool("pathkeep-intelligence-priority", &INTELLIGENCE_PRIORITY_WORKERS, 1, {
        let paths = paths.clone();
        let session_database_key = session_database_key.clone();
        move || {
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
                let connection = match ai_archive_connection(
                    &paths,
                    &config,
                    session_database_key.as_deref(),
                ) {
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

                let _ = execute_core_intelligence_job(
                    &paths,
                    &config,
                    session_database_key.as_deref(),
                    job.id,
                    &job.job_type,
                );
            }
        }
    });

    let enrichment_workers = desired_workers.saturating_sub(1);
    if enrichment_workers == 0 {
        return;
    }
    maybe_spawn_worker_pool(
        "pathkeep-intelligence-enrichment",
        &INTELLIGENCE_ENRICHMENT_WORKERS,
        enrichment_workers,
        move || {
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
                let connection = match ai_archive_connection(
                    &paths,
                    &config,
                    session_database_key.as_deref(),
                ) {
                    Ok(connection) => connection,
                    Err(error) => {
                        eprintln!(
                            "PathKeep could not open the archive for intelligence queue work: {error:#}"
                        );
                        break;
                    }
                };
                let Some(job_id) = (match next_queued_enrichment_job(&connection) {
                    Ok(job) => job,
                    Err(error) => {
                        eprintln!(
                            "PathKeep could not load the next queued enrichment job: {error:#}"
                        );
                        break;
                    }
                }) else {
                    break;
                };
                let _ = execute_enrichment_job_by_id(&paths, &connection, job_id);
            }
        },
    );
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

fn with_core_intelligence<R>(
    _session_database_key: Option<&str>,
    f: impl FnOnce(&vault_core::ProjectPaths, &AppConfig) -> Result<R>,
) -> Result<R> {
    let paths = vault_core::project_paths()?;
    let config = load_unlocked_config(&paths)?;
    f(&paths, &config)
}

fn with_core_intelligence_section<R>(
    session_database_key: Option<&str>,
    section_id: &str,
    window: CoreIntelligenceSectionWindow,
    fetch: impl FnOnce(&vault_core::ProjectPaths, &AppConfig) -> Result<R>,
    is_empty: impl FnOnce(&R) -> bool,
) -> Result<CoreIntelligenceSectionResult<R>> {
    with_core_intelligence(session_database_key, |paths, config| {
        let data = fetch(paths, config)?;
        let meta = build_core_intelligence_section_meta(
            paths,
            config,
            session_database_key,
            section_id,
            window,
            is_empty(&data),
        )?;
        Ok(CoreIntelligenceSectionResult { data, meta })
    })
}

fn build_timed_section_result<R>(
    paths: &vault_core::ProjectPaths,
    config: &AppConfig,
    session_database_key: Option<&str>,
    section_id: &str,
    window: CoreIntelligenceSectionWindow,
    fetch: impl FnOnce() -> Result<R>,
    is_empty: impl FnOnce(&R) -> bool,
) -> Result<(CoreIntelligenceSectionResult<R>, CoreIntelligenceSectionTiming)> {
    let started_at = Instant::now();
    let data = fetch()?;
    let duration_ms = started_at.elapsed().as_millis() as u64;
    let meta = build_core_intelligence_section_meta(
        paths,
        config,
        session_database_key,
        section_id,
        window,
        is_empty(&data),
    )?;
    Ok((
        CoreIntelligenceSectionResult { data, meta },
        CoreIntelligenceSectionTiming { section_id: section_id.to_string(), duration_ms },
    ))
}

/// Runs a Core Intelligence rebuild immediately.
#[cfg_attr(not(test), allow(dead_code))]
pub fn run_core_intelligence_now(
    session_database_key: Option<&str>,
    request: &CoreIntelligenceRebuildRequest,
) -> Result<CoreIntelligenceRebuildReport> {
    with_core_intelligence(session_database_key, |paths, config| {
        intelligence::run_core_intelligence(paths, config, session_database_key, request)
    })
}

/// Queues one manual Core Intelligence rebuild so heavy work can stay off the foreground UI thread.
#[cfg_attr(not(test), allow(dead_code))]
pub fn queue_core_intelligence_rebuild(
    session_database_key: Option<&str>,
    request: &CoreIntelligenceRebuildRequest,
) -> Result<CoreIntelligenceQueueReport> {
    let paths = vault_core::project_paths()?;
    let config = load_unlocked_config(&paths)?;
    let connection = ai_archive_connection(&paths, &config, session_database_key)?;
    let job_id = enqueue_deterministic_rebuild_job(
        &connection,
        request,
        "User requested a Core Intelligence rebuild from the UI.",
    )?;
    let state = connection
        .query_row("SELECT state FROM intelligence_jobs WHERE id = ?1", [job_id], |row| {
            row.get::<_, String>(0)
        })
        .unwrap_or_else(|_| "queued".to_string());
    maybe_spawn_intelligence_queue_drain(&paths, &config, session_database_key, 1);
    Ok(CoreIntelligenceQueueReport {
        job_id,
        state: state.clone(),
        notes: vec![if config.ai.job_queue_paused {
            format!("Queued Core Intelligence job {} while the runtime queue is paused.", job_id)
        } else if state == "running" {
            format!("Core Intelligence job {} is already running in the background.", job_id)
        } else {
            format!(
                "Queued Core Intelligence job {}. PathKeep is processing it in the background.",
                job_id
            )
        }],
    })
}

fn execute_core_intelligence_job(
    paths: &vault_core::ProjectPaths,
    config: &AppConfig,
    session_database_key: Option<&str>,
    job_id: i64,
    job_type: &str,
) -> Result<bool> {
    let connection = ai_archive_connection(paths, config, session_database_key)?;
    let Some(payload) = claim_core_intelligence_job(&connection, job_id)? else {
        return Ok(false);
    };
    if payload.job_type != job_type {
        return Ok(false);
    }
    let initial_profile =
        payload.request.profile_id.as_deref().unwrap_or("all profiles").to_string();
    let job_label = match job_type {
        VISIT_DERIVE_JOB_TYPE => "visit-derived facts refresh",
        DAILY_ROLLUP_JOB_TYPE => "daily rollup refresh",
        STRUCTURAL_REBUILD_JOB_TYPE => "structural entity rebuild",
        _ => "Core Intelligence rebuild",
    };
    let cancel_requested = |detail: &str| -> Result<()> {
        if intelligence_job_stop_requested(&connection, job_id)? {
            let _ =
                mark_running_intelligence_job_cancelled(&connection, job_id, "cancelled from UI");
            anyhow::bail!(detail.to_string());
        }
        Ok(())
    };
    cancel_requested(&format!("{job_label} was cancelled before work started."))?;
    let _ = update_intelligence_job_artifact(
        &connection,
        job_id,
        &json!({
            "kind": job_type,
            "phase": "queued",
            "detail": format!("Preparing a {job_label} for {initial_profile}."),
            "progressPercent": 0.0
        }),
    );
    match intelligence::run_core_intelligence_job_type_with_progress(
        paths,
        config,
        session_database_key,
        job_type,
        &payload.request,
        |progress| {
            cancel_requested(&format!(
                "{job_label} was cancelled while progress was being reported."
            ))?;
            let artifact = json!({
                "kind": job_type,
                "phase": progress.phase,
                "detail": progress.detail,
                "processedItems": progress.processed_items,
                "totalItems": progress.total_items,
                "progressPercent": progress.progress_percent,
            });
            let _ = update_intelligence_job_artifact(&connection, job_id, &artifact);
            cancel_requested(&format!(
                "{job_label} was cancelled after the latest progress update."
            ))?;
            Ok(())
        },
    ) {
        Ok(report) => {
            if intelligence_job_stop_requested(&connection, job_id)? {
                let _ = mark_running_intelligence_job_cancelled(
                    &connection,
                    job_id,
                    "cancelled from UI",
                );
                return Ok(true);
            }
            if !mark_intelligence_job_succeeded(
                &connection,
                job_id,
                &json!({
                    "kind": job_type,
                    "phase": "completed",
                    "detail": format!(
                        "{} finished with {} visits processed.",
                        job_label,
                        report.processed_visits,
                    ),
                    "processedItems": report.processed_visits,
                    "totalItems": report.processed_visits,
                    "progressPercent": 100.0,
                    "processedVisits": report.processed_visits,
                    "sessionCount": report.sessions,
                    "trailCount": report.search_trails,
                    "queryFamilyCount": report.query_families,
                    "refindPageCount": report.refind_pages,
                    "executionMode": report.execution_mode,
                    "affectedProfiles": report.affected_profiles,
                    "dirtyVisitCount": report.dirty_visit_count,
                    "dirtyDateKeys": report.dirty_date_keys,
                    "fallbackReason": report.fallback_reason,
                    "notes": report.notes,
                }),
            )? {
                let _ = mark_running_intelligence_job_cancelled(
                    &connection,
                    job_id,
                    "cancelled from UI",
                );
            }
            Ok(true)
        }
        Err(error) => {
            if error.to_string().contains("cancelled") {
                let _ = mark_running_intelligence_job_cancelled(
                    &connection,
                    job_id,
                    "cancelled from UI",
                );
                return Ok(true);
            }
            mark_intelligence_job_failed(&connection, job_id, &error.to_string())?;
            Err(error)
        }
    }
}

/// Loads one paginated sessions list.
pub fn get_sessions(
    session_database_key: Option<&str>,
    request: &PagedDateRangeRequest,
) -> Result<SessionListResult> {
    with_core_intelligence(session_database_key, |paths, config| {
        intelligence::get_sessions(paths, config, session_database_key, request)
    })
}

/// Loads one session detail read model.
pub fn get_session_detail(
    session_database_key: Option<&str>,
    session_id: &str,
) -> Result<SessionDetail> {
    with_core_intelligence(session_database_key, |paths, config| {
        intelligence::get_session_detail(paths, config, session_database_key, session_id)
    })
}

/// Loads one paginated search trail list.
pub fn get_search_trails(
    session_database_key: Option<&str>,
    request: &SearchTrailQueryRequest,
) -> Result<TrailListResult> {
    with_core_intelligence(session_database_key, |paths, config| {
        intelligence::get_search_trails(paths, config, session_database_key, request)
    })
}

pub fn get_trail_detail(session_database_key: Option<&str>, trail_id: &str) -> Result<TrailDetail> {
    with_core_intelligence(session_database_key, |paths, config| {
        intelligence::get_trail_detail(paths, config, session_database_key, trail_id)
    })
}

pub fn get_navigation_path(
    session_database_key: Option<&str>,
    visit_id: i64,
) -> Result<NavigationPath> {
    with_core_intelligence(session_database_key, |paths, config| {
        intelligence::get_navigation_path(paths, config, session_database_key, visit_id)
    })
}

pub fn get_hub_pages(
    session_database_key: Option<&str>,
    request: &TopSitesRequest,
) -> Result<Vec<HubPage>> {
    with_core_intelligence(session_database_key, |paths, config| {
        intelligence::get_hub_pages(paths, config, session_database_key, request)
    })
}

pub fn get_search_engine_ranking(
    session_database_key: Option<&str>,
    request: &ScopedDateRangeRequest,
) -> Result<CoreIntelligenceSectionResult<Vec<EngineRanking>>> {
    with_core_intelligence_section(
        session_database_key,
        "search-activity",
        CoreIntelligenceSectionWindow::DateRange { date_range: request.date_range.clone() },
        |paths, config| {
            intelligence::get_search_engine_ranking(paths, config, session_database_key, request)
        },
        |data| data.is_empty(),
    )
}

pub fn list_search_engine_rules(
    session_database_key: Option<&str>,
) -> Result<Vec<SearchEngineRule>> {
    with_core_intelligence(session_database_key, |paths, config| {
        intelligence::list_search_engine_rules_for_settings(paths, config, session_database_key)
    })
}

pub fn upsert_search_engine_rule(
    session_database_key: Option<&str>,
    input: &SearchEngineRuleInput,
) -> Result<Vec<SearchEngineRule>> {
    with_core_intelligence(session_database_key, |paths, config| {
        intelligence::upsert_search_engine_rule_for_settings(
            paths,
            config,
            session_database_key,
            input,
        )
    })
}

pub fn delete_search_engine_rule(
    session_database_key: Option<&str>,
    rule_id: &str,
) -> Result<Vec<SearchEngineRule>> {
    with_core_intelligence(session_database_key, |paths, config| {
        intelligence::delete_search_engine_rule_for_settings(
            paths,
            config,
            session_database_key,
            rule_id,
        )
    })
}

pub fn get_intelligence_primary_overview(
    session_database_key: Option<&str>,
    request: &ScopedDateRangeRequest,
) -> Result<CoreIntelligencePrimaryOverview> {
    with_core_intelligence(session_database_key, |paths, config| {
        let overview_started_at = Instant::now();
        let top_sites_request = TopSitesRequest {
            date_range: request.date_range.clone(),
            profile_id: request.profile_id.clone(),
            sort_by: Some("visit_count".to_string()),
            limit: Some(40),
        };
        let query_family_request = PagedDateRangeRequest {
            date_range: request.date_range.clone(),
            profile_id: request.profile_id.clone(),
            page: 0,
            page_size: 10,
        };
        let top_search_concepts_request = TopSearchConceptsRequest {
            date_range: request.date_range.clone(),
            profile_id: request.profile_id.clone(),
            limit: Some(50),
        };
        let refind_pages_request = RefindPagesRequest {
            date_range: request.date_range.clone(),
            profile_id: request.profile_id.clone(),
            limit: Some(5),
        };
        let discovery_trend_day_request = GranularityDateRangeRequest {
            date_range: request.date_range.clone(),
            profile_id: request.profile_id.clone(),
            granularity: "day".to_string(),
        };
        let interrupted_habits_request =
            ProfileScopedRequest { profile_id: request.profile_id.clone() };

        let mut timings = Vec::new();
        let (digest_summary, timing) = build_timed_section_result(
            paths,
            config,
            session_database_key,
            "digest-summary",
            CoreIntelligenceSectionWindow::DateRange { date_range: request.date_range.clone() },
            || intelligence::get_digest_summary(paths, config, session_database_key, request),
            |data| {
                data.total_visits.value == 0
                    && data.total_searches.value == 0
                    && data.new_domains.value == 0
                    && data.deep_read_pages.value == 0
                    && data.refind_pages.value == 0
            },
        )?;
        timings.push(timing);
        let (on_this_day, timing) = build_timed_section_result(
            paths,
            config,
            session_database_key,
            "on-this-day",
            CoreIntelligenceSectionWindow::CalendarDayHistory {
                reference_date: Local::now().format("%Y-%m-%d").to_string(),
            },
            || {
                intelligence::get_on_this_day(
                    paths,
                    config,
                    session_database_key,
                    request.profile_id.as_deref(),
                )
            },
            |data| data.is_empty(),
        )?;
        timings.push(timing);
        let (top_sites, timing) = build_timed_section_result(
            paths,
            config,
            session_database_key,
            "top-sites",
            CoreIntelligenceSectionWindow::DateRange { date_range: request.date_range.clone() },
            || intelligence::get_top_sites(paths, config, session_database_key, &top_sites_request),
            |data| data.is_empty(),
        )?;
        timings.push(timing);
        let (refind_pages, timing) = build_timed_section_result(
            paths,
            config,
            session_database_key,
            "refind-pages",
            CoreIntelligenceSectionWindow::DateRange { date_range: request.date_range.clone() },
            || {
                intelligence::get_refind_pages(
                    paths,
                    config,
                    session_database_key,
                    &refind_pages_request,
                )
            },
            |data| data.is_empty(),
        )?;
        timings.push(timing);
        let (search_engine_ranking, timing) = build_timed_section_result(
            paths,
            config,
            session_database_key,
            "search-activity",
            CoreIntelligenceSectionWindow::DateRange { date_range: request.date_range.clone() },
            || {
                intelligence::get_search_engine_ranking(
                    paths,
                    config,
                    session_database_key,
                    request,
                )
            },
            |data| data.is_empty(),
        )?;
        timings.push(timing);
        let (top_search_concepts, timing) = build_timed_section_result(
            paths,
            config,
            session_database_key,
            "search-activity",
            CoreIntelligenceSectionWindow::DateRange { date_range: request.date_range.clone() },
            || {
                intelligence::get_top_search_concepts(
                    paths,
                    config,
                    session_database_key,
                    &top_search_concepts_request,
                )
            },
            |data| data.is_empty(),
        )?;
        timings.push(timing);
        let (query_families, timing) = build_timed_section_result(
            paths,
            config,
            session_database_key,
            "search-activity",
            CoreIntelligenceSectionWindow::DateRange { date_range: request.date_range.clone() },
            || {
                intelligence::get_query_families(
                    paths,
                    config,
                    session_database_key,
                    &query_family_request,
                )
            },
            |data| data.families.is_empty(),
        )?;
        timings.push(timing);
        let (activity_mix, timing) = build_timed_section_result(
            paths,
            config,
            session_database_key,
            "activity-mix",
            CoreIntelligenceSectionWindow::DateRange { date_range: request.date_range.clone() },
            || intelligence::get_activity_mix(paths, config, session_database_key, request),
            |data| data.categories.is_empty(),
        )?;
        timings.push(timing);
        let (discovery_trend_day, timing) = build_timed_section_result(
            paths,
            config,
            session_database_key,
            "browsing-rhythm",
            CoreIntelligenceSectionWindow::DateRange { date_range: request.date_range.clone() },
            || {
                intelligence::get_discovery_trend(
                    paths,
                    config,
                    session_database_key,
                    &discovery_trend_day_request,
                )
            },
            |data| data.points.is_empty(),
        )?;
        timings.push(timing);
        let (habit_patterns, timing) = build_timed_section_result(
            paths,
            config,
            session_database_key,
            "habits",
            CoreIntelligenceSectionWindow::DateRange { date_range: request.date_range.clone() },
            || intelligence::get_habit_patterns(paths, config, session_database_key, request),
            |data| data.is_empty(),
        )?;
        timings.push(timing);
        let (interrupted_habits, timing) = build_timed_section_result(
            paths,
            config,
            session_database_key,
            "habits",
            CoreIntelligenceSectionWindow::DateRange { date_range: request.date_range.clone() },
            || {
                intelligence::get_interrupted_habits(
                    paths,
                    config,
                    session_database_key,
                    &interrupted_habits_request,
                )
            },
            |data| data.is_empty(),
        )?;
        timings.push(timing);

        Ok(CoreIntelligencePrimaryOverview {
            digest_summary,
            on_this_day,
            top_sites,
            refind_pages,
            search_engine_ranking,
            top_search_concepts,
            query_families,
            activity_mix,
            discovery_trend_day,
            habit_patterns,
            interrupted_habits,
            total_duration_ms: overview_started_at.elapsed().as_millis() as u64,
            timings,
        })
    })
}

pub fn get_top_search_concepts(
    session_database_key: Option<&str>,
    request: &TopSearchConceptsRequest,
) -> Result<CoreIntelligenceSectionResult<Vec<SearchConcept>>> {
    with_core_intelligence_section(
        session_database_key,
        "search-activity",
        CoreIntelligenceSectionWindow::DateRange { date_range: request.date_range.clone() },
        |paths, config| {
            intelligence::get_top_search_concepts(paths, config, session_database_key, request)
        },
        |data| data.is_empty(),
    )
}

pub fn get_search_queries(
    session_database_key: Option<&str>,
    request: &SearchQueryListRequest,
) -> Result<CoreIntelligenceSectionResult<SearchQueryListResult>> {
    with_core_intelligence_section(
        session_database_key,
        "search-activity",
        CoreIntelligenceSectionWindow::DateRange { date_range: request.date_range.clone() },
        |paths, config| {
            intelligence::get_search_queries(paths, config, session_database_key, request)
        },
        |data| data.rows.is_empty(),
    )
}

pub fn get_query_families(
    session_database_key: Option<&str>,
    request: &PagedDateRangeRequest,
) -> Result<CoreIntelligenceSectionResult<QueryFamilyResult>> {
    with_core_intelligence_section(
        session_database_key,
        "search-activity",
        CoreIntelligenceSectionWindow::DateRange { date_range: request.date_range.clone() },
        |paths, config| {
            intelligence::get_query_families(paths, config, session_database_key, request)
        },
        |data| data.families.is_empty(),
    )
}

pub fn get_query_family_detail(
    session_database_key: Option<&str>,
    request: &QueryFamilyDetailRequest,
) -> Result<CoreIntelligenceSectionResult<QueryFamilyDetail>> {
    with_core_intelligence_section(
        session_database_key,
        "query-family-detail",
        CoreIntelligenceSectionWindow::DateRange { date_range: request.date_range.clone() },
        |paths, config| {
            intelligence::get_query_family_detail(paths, config, session_database_key, request)
        },
        |data| data.related_trails.is_empty(),
    )
}

pub fn get_top_sites(
    session_database_key: Option<&str>,
    request: &TopSitesRequest,
) -> Result<CoreIntelligenceSectionResult<Vec<TopSite>>> {
    with_core_intelligence_section(
        session_database_key,
        "top-sites",
        CoreIntelligenceSectionWindow::DateRange { date_range: request.date_range.clone() },
        |paths, config| intelligence::get_top_sites(paths, config, session_database_key, request),
        |data| data.is_empty(),
    )
}

pub fn get_domain_trend(
    session_database_key: Option<&str>,
    request: &DomainTrendRequest,
) -> Result<DomainTrend> {
    with_core_intelligence(session_database_key, |paths, config| {
        intelligence::get_domain_trend(paths, config, session_database_key, request)
    })
}

pub fn get_refind_pages(
    session_database_key: Option<&str>,
    request: &RefindPagesRequest,
) -> Result<CoreIntelligenceSectionResult<Vec<RefindPage>>> {
    with_core_intelligence_section(
        session_database_key,
        "refind-pages",
        CoreIntelligenceSectionWindow::DateRange { date_range: request.date_range.clone() },
        |paths, config| {
            intelligence::get_refind_pages(paths, config, session_database_key, request)
        },
        |data| data.is_empty(),
    )
}

pub fn get_refind_page_detail(
    session_database_key: Option<&str>,
    request: &RefindPageDetailRequest,
) -> Result<CoreIntelligenceSectionResult<RefindPageDetail>> {
    with_core_intelligence_section(
        session_database_key,
        "refind-page-detail",
        CoreIntelligenceSectionWindow::DateRange { date_range: request.date_range.clone() },
        |paths, config| {
            intelligence::get_refind_page_detail(paths, config, session_database_key, request)
        },
        |data| {
            data.explanation.visit_ids.is_empty()
                && data.related_trails.is_empty()
                && data.recent_days.is_empty()
        },
    )
}

pub fn explain_refind(
    session_database_key: Option<&str>,
    request: &vault_core::ExplainRefindRequest,
) -> Result<RefindExplanation> {
    with_core_intelligence(session_database_key, |paths, config| {
        intelligence::explain_refind(paths, config, session_database_key, request)
    })
}

pub fn explain_entity(
    session_database_key: Option<&str>,
    request: &EntityExplanationRequest,
) -> Result<Explanation> {
    with_core_intelligence(session_database_key, |paths, config| {
        intelligence::explain_entity(paths, config, session_database_key, request)
    })
}

pub fn get_activity_mix(
    session_database_key: Option<&str>,
    request: &ScopedDateRangeRequest,
) -> Result<CoreIntelligenceSectionResult<ActivityMix>> {
    with_core_intelligence_section(
        session_database_key,
        "activity-mix",
        CoreIntelligenceSectionWindow::DateRange { date_range: request.date_range.clone() },
        |paths, config| {
            intelligence::get_activity_mix(paths, config, session_database_key, request)
        },
        |data| data.categories.is_empty(),
    )
}

pub fn get_activity_mix_trend(
    session_database_key: Option<&str>,
    request: &GranularityDateRangeRequest,
) -> Result<ActivityMixTrend> {
    with_core_intelligence(session_database_key, |paths, config| {
        intelligence::get_activity_mix_trend(paths, config, session_database_key, request)
    })
}

pub fn get_digest_summary(
    session_database_key: Option<&str>,
    request: &ScopedDateRangeRequest,
) -> Result<CoreIntelligenceSectionResult<DigestSummary>> {
    with_core_intelligence_section(
        session_database_key,
        "digest-summary",
        CoreIntelligenceSectionWindow::DateRange { date_range: request.date_range.clone() },
        |paths, config| {
            intelligence::get_digest_summary(paths, config, session_database_key, request)
        },
        |data| {
            data.total_visits.value == 0
                && data.total_searches.value == 0
                && data.new_domains.value == 0
                && data.deep_read_pages.value == 0
                && data.refind_pages.value == 0
        },
    )
}

pub fn get_stable_sources(
    session_database_key: Option<&str>,
    request: &ScopedDateRangeRequest,
) -> Result<CoreIntelligenceSectionResult<Vec<StableSource>>> {
    with_core_intelligence_section(
        session_database_key,
        "stable-sources",
        CoreIntelligenceSectionWindow::DateRange { date_range: request.date_range.clone() },
        |paths, config| {
            intelligence::get_stable_sources(paths, config, session_database_key, request)
        },
        |data| data.is_empty(),
    )
}

pub fn get_search_effectiveness(
    session_database_key: Option<&str>,
    request: &SearchEffectivenessRequest,
) -> Result<CoreIntelligenceSectionResult<SearchEffectiveness>> {
    with_core_intelligence_section(
        session_database_key,
        "search-effectiveness",
        CoreIntelligenceSectionWindow::DateRange { date_range: request.date_range.clone() },
        |paths, config| {
            intelligence::get_search_effectiveness(paths, config, session_database_key, request)
        },
        |data| {
            data.engine_stats.is_empty()
                && data.top_resolving_sources.is_empty()
                && data.hardest_topics.is_empty()
        },
    )
}

pub fn get_friction_signals(
    session_database_key: Option<&str>,
    request: &ScopedDateRangeRequest,
) -> Result<CoreIntelligenceSectionResult<Vec<FrictionSignal>>> {
    with_core_intelligence_section(
        session_database_key,
        "friction-signals",
        CoreIntelligenceSectionWindow::DateRange { date_range: request.date_range.clone() },
        |paths, config| {
            intelligence::get_friction_signals(paths, config, session_database_key, request)
        },
        |data| data.is_empty(),
    )
}

pub fn get_reopened_investigations(
    session_database_key: Option<&str>,
    request: &ScopedDateRangeRequest,
) -> Result<CoreIntelligenceSectionResult<Vec<ReopenedInvestigation>>> {
    with_core_intelligence_section(
        session_database_key,
        "reopened-investigations",
        CoreIntelligenceSectionWindow::DateRange { date_range: request.date_range.clone() },
        |paths, config| {
            intelligence::get_reopened_investigations(paths, config, session_database_key, request)
        },
        |data| data.is_empty(),
    )
}

pub fn get_domain_deep_dive(
    session_database_key: Option<&str>,
    request: &DomainDeepDiveRequest,
) -> Result<CoreIntelligenceSectionResult<DomainDeepDive>> {
    with_core_intelligence_section(
        session_database_key,
        "domain-deep-dive",
        CoreIntelligenceSectionWindow::DateRange { date_range: request.date_range.clone() },
        |paths, config| {
            intelligence::get_domain_deep_dive(paths, config, session_database_key, request)
        },
        |data| {
            data.total_visits == 0
                && data.active_days == 0
                && data.trail_count == 0
                && data.top_pages.is_empty()
                && data.top_referrers.is_empty()
                && data.top_exits.is_empty()
                && data.visit_trend.is_empty()
        },
    )
}

pub fn get_day_insights(
    session_database_key: Option<&str>,
    request: &DayInsightsRequest,
) -> Result<CoreIntelligenceSectionResult<DayInsights>> {
    with_core_intelligence_section(
        session_database_key,
        "day-insights",
        CoreIntelligenceSectionWindow::DateRange {
            date_range: vault_core::DateRange {
                start: request.date.clone(),
                end: request.date.clone(),
            },
        },
        |paths, config| {
            intelligence::get_day_insights(paths, config, session_database_key, request)
        },
        |data| {
            data.digest_summary.total_visits.value == 0
                && data.digest_summary.total_searches.value == 0
                && data.digest_summary.new_domains.value == 0
                && data.digest_summary.deep_read_pages.value == 0
                && data.digest_summary.refind_pages.value == 0
                && data.top_sites.is_empty()
                && data.query_families.families.is_empty()
                && data.refind_pages.is_empty()
        },
    )
}

pub fn get_browsing_rhythm(
    session_database_key: Option<&str>,
    request: &CategoryFilteredDateRangeRequest,
) -> Result<CoreIntelligenceSectionResult<RhythmHeatmap>> {
    with_core_intelligence_section(
        session_database_key,
        "browsing-rhythm",
        CoreIntelligenceSectionWindow::DateRange { date_range: request.date_range.clone() },
        |paths, config| {
            intelligence::get_browsing_rhythm(paths, config, session_database_key, request)
        },
        |data| data.cells.is_empty(),
    )
}

pub fn get_discovery_trend(
    session_database_key: Option<&str>,
    request: &GranularityDateRangeRequest,
) -> Result<CoreIntelligenceSectionResult<DiscoveryTrend>> {
    with_core_intelligence_section(
        session_database_key,
        "discovery-trend",
        CoreIntelligenceSectionWindow::DateRange { date_range: request.date_range.clone() },
        |paths, config| {
            intelligence::get_discovery_trend(paths, config, session_database_key, request)
        },
        |data| data.points.is_empty(),
    )
}

pub fn get_on_this_day(
    session_database_key: Option<&str>,
    profile_id: Option<&str>,
) -> Result<CoreIntelligenceSectionResult<Vec<OnThisDayEntry>>> {
    with_core_intelligence_section(
        session_database_key,
        "on-this-day",
        CoreIntelligenceSectionWindow::CalendarDayHistory {
            reference_date: Local::now().format("%Y-%m-%d").to_string(),
        },
        |paths, config| {
            intelligence::get_on_this_day(paths, config, session_database_key, profile_id)
        },
        |data| data.is_empty(),
    )
}

pub fn get_intelligence_embed_cards(
    session_database_key: Option<&str>,
    request: &IntelligenceEmbedCardsRequest,
) -> Result<Vec<IntelligenceEmbedCardPayload>> {
    with_core_intelligence(session_database_key, |paths, config| {
        intelligence::get_intelligence_embed_cards(paths, config, session_database_key, request)
    })
}

pub fn get_intelligence_widget_snapshot(
    session_database_key: Option<&str>,
    request: &IntelligenceEmbedCardsRequest,
) -> Result<IntelligenceWidgetSnapshot> {
    with_core_intelligence(session_database_key, |paths, config| {
        intelligence::get_intelligence_widget_snapshot(paths, config, session_database_key, request)
    })
}

pub fn get_intelligence_public_snapshot(
    session_database_key: Option<&str>,
    request: &ScopedDateRangeRequest,
) -> Result<IntelligencePublicSnapshot> {
    with_core_intelligence(session_database_key, |paths, config| {
        intelligence::get_intelligence_public_snapshot(paths, config, session_database_key, request)
    })
}

pub fn preview_intelligence_local_host(
    session_database_key: Option<&str>,
    request: &IntelligenceLocalHostRequest,
) -> Result<IntelligenceLocalHostPreview> {
    with_core_intelligence(session_database_key, |paths, config| {
        intelligence::preview_intelligence_local_host(paths, config, session_database_key, request)
    })
}

pub fn build_intelligence_local_host(
    session_database_key: Option<&str>,
    request: &IntelligenceLocalHostRequest,
) -> Result<IntelligenceLocalHostBuildResult> {
    with_core_intelligence(session_database_key, |paths, config| {
        intelligence::build_intelligence_local_host(paths, config, session_database_key, request)
    })
}

pub fn get_breadth_index(
    session_database_key: Option<&str>,
    request: &ScopedDateRangeRequest,
) -> Result<CoreIntelligenceSectionResult<BreadthIndex>> {
    with_core_intelligence_section(
        session_database_key,
        "breadth-index",
        CoreIntelligenceSectionWindow::DateRange { date_range: request.date_range.clone() },
        |paths, config| {
            intelligence::get_breadth_index(paths, config, session_database_key, request)
        },
        |_| false,
    )
}

pub fn get_habit_patterns(
    session_database_key: Option<&str>,
    request: &ScopedDateRangeRequest,
) -> Result<CoreIntelligenceSectionResult<Vec<HabitPattern>>> {
    with_core_intelligence_section(
        session_database_key,
        "habits",
        CoreIntelligenceSectionWindow::DateRange { date_range: request.date_range.clone() },
        |paths, config| {
            intelligence::get_habit_patterns(paths, config, session_database_key, request)
        },
        |data| data.is_empty(),
    )
}

pub fn get_interrupted_habits(
    session_database_key: Option<&str>,
    request: &ProfileScopedRequest,
) -> Result<CoreIntelligenceSectionResult<Vec<InterruptedHabit>>> {
    with_core_intelligence_section(
        session_database_key,
        "habits",
        CoreIntelligenceSectionWindow::DateRange {
            date_range: vault_core::DateRange { start: "".to_string(), end: "".to_string() },
        },
        |paths, config| {
            intelligence::get_interrupted_habits(paths, config, session_database_key, request)
        },
        |data| data.is_empty(),
    )
}

pub fn get_path_flows(
    session_database_key: Option<&str>,
    request: &PathFlowRequest,
) -> Result<CoreIntelligenceSectionResult<Vec<PathFlow>>> {
    with_core_intelligence_section(
        session_database_key,
        "path-flows",
        CoreIntelligenceSectionWindow::DateRange { date_range: request.date_range.clone() },
        |paths, config| intelligence::get_path_flows(paths, config, session_database_key, request),
        |data| data.is_empty(),
    )
}

pub fn get_observed_interactions(
    session_database_key: Option<&str>,
    request: &ScopedDateRangeRequest,
) -> Result<CoreIntelligenceSectionResult<Vec<ObservedInteraction>>> {
    with_core_intelligence_section(
        session_database_key,
        "observed-interactions",
        CoreIntelligenceSectionWindow::DateRange { date_range: request.date_range.clone() },
        |paths, config| {
            intelligence::get_observed_interactions(paths, config, session_database_key, request)
        },
        |data| data.is_empty(),
    )
}

pub fn get_compare_sets(
    session_database_key: Option<&str>,
    request: &ScopedDateRangeRequest,
) -> Result<CoreIntelligenceSectionResult<Vec<CompareSet>>> {
    with_core_intelligence_section(
        session_database_key,
        "compare-sets",
        CoreIntelligenceSectionWindow::DateRange { date_range: request.date_range.clone() },
        |paths, config| {
            intelligence::get_compare_sets(paths, config, session_database_key, request)
        },
        |data| data.is_empty(),
    )
}

pub fn get_multi_browser_diff(
    session_database_key: Option<&str>,
    request: &ScopedDateRangeRequest,
) -> Result<CoreIntelligenceSectionResult<BrowserDiff>> {
    with_core_intelligence_section(
        session_database_key,
        "multi-browser-diff",
        CoreIntelligenceSectionWindow::DateRange { date_range: request.date_range.clone() },
        |paths, config| {
            intelligence::get_multi_browser_diff(paths, config, session_database_key, request)
        },
        |data| data.profiles.is_empty() && data.category_distributions.is_empty(),
    )
}

pub fn get_intelligence_secondary_overview(
    session_database_key: Option<&str>,
    request: &ScopedDateRangeRequest,
) -> Result<CoreIntelligenceSecondaryOverview> {
    with_core_intelligence(session_database_key, |paths, config| {
        let overview_started_at = Instant::now();
        let search_effectiveness_request = SearchEffectivenessRequest {
            date_range: request.date_range.clone(),
            profile_id: request.profile_id.clone(),
            engine: None,
        };
        let discovery_trend_week_request = GranularityDateRangeRequest {
            date_range: request.date_range.clone(),
            profile_id: request.profile_id.clone(),
            granularity: "week".to_string(),
        };
        let path_flow_request = PathFlowRequest {
            date_range: request.date_range.clone(),
            profile_id: request.profile_id.clone(),
            step_count: 3,
            limit: Some(15),
        };

        let mut timings = Vec::new();
        let (stable_sources, timing) = build_timed_section_result(
            paths,
            config,
            session_database_key,
            "stable-sources",
            CoreIntelligenceSectionWindow::DateRange { date_range: request.date_range.clone() },
            || intelligence::get_stable_sources(paths, config, session_database_key, request),
            |data| data.is_empty(),
        )?;
        timings.push(timing);
        let (search_effectiveness, timing) = build_timed_section_result(
            paths,
            config,
            session_database_key,
            "search-effectiveness",
            CoreIntelligenceSectionWindow::DateRange { date_range: request.date_range.clone() },
            || {
                intelligence::get_search_effectiveness(
                    paths,
                    config,
                    session_database_key,
                    &search_effectiveness_request,
                )
            },
            |data| {
                data.engine_stats.is_empty()
                    && data.top_resolving_sources.is_empty()
                    && data.hardest_topics.is_empty()
            },
        )?;
        timings.push(timing);
        let (friction_signals, timing) = build_timed_section_result(
            paths,
            config,
            session_database_key,
            "friction-signals",
            CoreIntelligenceSectionWindow::DateRange { date_range: request.date_range.clone() },
            || intelligence::get_friction_signals(paths, config, session_database_key, request),
            |data| data.is_empty(),
        )?;
        timings.push(timing);
        let (reopened_investigations, timing) = build_timed_section_result(
            paths,
            config,
            session_database_key,
            "reopened-investigations",
            CoreIntelligenceSectionWindow::DateRange { date_range: request.date_range.clone() },
            || {
                intelligence::get_reopened_investigations(
                    paths,
                    config,
                    session_database_key,
                    request,
                )
            },
            |data| data.is_empty(),
        )?;
        timings.push(timing);
        let (discovery_trend_week, timing) = build_timed_section_result(
            paths,
            config,
            session_database_key,
            "discovery-trend",
            CoreIntelligenceSectionWindow::DateRange { date_range: request.date_range.clone() },
            || {
                intelligence::get_discovery_trend(
                    paths,
                    config,
                    session_database_key,
                    &discovery_trend_week_request,
                )
            },
            |data| data.points.is_empty(),
        )?;
        timings.push(timing);
        let (breadth_index, timing) = build_timed_section_result(
            paths,
            config,
            session_database_key,
            "breadth-index",
            CoreIntelligenceSectionWindow::DateRange { date_range: request.date_range.clone() },
            || intelligence::get_breadth_index(paths, config, session_database_key, request),
            |_| false,
        )?;
        timings.push(timing);
        let (path_flows, timing) = build_timed_section_result(
            paths,
            config,
            session_database_key,
            "path-flows",
            CoreIntelligenceSectionWindow::DateRange { date_range: request.date_range.clone() },
            || {
                intelligence::get_path_flows(
                    paths,
                    config,
                    session_database_key,
                    &path_flow_request,
                )
            },
            |data| data.is_empty(),
        )?;
        timings.push(timing);
        let (compare_sets, timing) = build_timed_section_result(
            paths,
            config,
            session_database_key,
            "compare-sets",
            CoreIntelligenceSectionWindow::DateRange { date_range: request.date_range.clone() },
            || intelligence::get_compare_sets(paths, config, session_database_key, request),
            |data| data.is_empty(),
        )?;
        timings.push(timing);
        let (multi_browser_diff, timing) = build_timed_section_result(
            paths,
            config,
            session_database_key,
            "multi-browser-diff",
            CoreIntelligenceSectionWindow::DateRange { date_range: request.date_range.clone() },
            || intelligence::get_multi_browser_diff(paths, config, session_database_key, request),
            |data| data.profiles.is_empty() && data.category_distributions.is_empty(),
        )?;
        timings.push(timing);
        let (observed_interactions, timing) = build_timed_section_result(
            paths,
            config,
            session_database_key,
            "observed-interactions",
            CoreIntelligenceSectionWindow::DateRange { date_range: request.date_range.clone() },
            || {
                intelligence::get_observed_interactions(
                    paths,
                    config,
                    session_database_key,
                    request,
                )
            },
            |data| data.is_empty(),
        )?;
        timings.push(timing);

        Ok(CoreIntelligenceSecondaryOverview {
            stable_sources,
            search_effectiveness,
            friction_signals,
            reopened_investigations,
            discovery_trend_week,
            breadth_index,
            path_flows,
            compare_sets,
            multi_browser_diff,
            observed_interactions,
            total_duration_ms: overview_started_at.elapsed().as_millis() as u64,
            timings,
        })
    })
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
