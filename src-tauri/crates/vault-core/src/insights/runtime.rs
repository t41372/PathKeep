//! Insights runtime and read-model surface.
//!
//! This module keeps the Settings/Insights-facing entrypoints together:
//! readiness/status, rebuild execution, snapshot loading, thread detail, clear,
//! and explanation.
//!
//! It intentionally delegates the heavy feature engineering and grouping work to
//! the rest of `insights.rs` and its existing submodules. The goal here is to
//! keep the user-facing orchestration readable without blending it into lower
//! level storage or enrichment helpers.

use super::surfaces::ContrastWindowSummary;
use super::*;
use crate::models::{
    InsightQueryGroupSummary, InsightReferencePageSummary, InsightSourceEffectivenessSummary,
    InsightTemplateSummary,
};
use std::collections::HashSet;
use thiserror::Error;

const INSIGHT_PROGRESS_PHASES: usize = 8;

#[derive(Debug, Error)]
#[error("{reason}")]
pub struct InsightsRunCancelled {
    reason: String,
}

impl InsightsRunCancelled {
    pub fn new(reason: impl Into<String>) -> Self {
        Self { reason: reason.into() }
    }
}

/// Progress snapshot emitted while a deterministic rebuild is running.
#[derive(Debug, Clone)]
pub struct InsightsRunProgress {
    pub phase_label: &'static str,
    pub phase_step: usize,
    pub phase_count: usize,
    pub processed_items: Option<usize>,
    pub total_items: Option<usize>,
    pub detail: Option<String>,
}

impl InsightsRunProgress {
    fn new(
        phase_label: &'static str,
        phase_step: usize,
        processed_items: Option<usize>,
        total_items: Option<usize>,
        detail: Option<String>,
    ) -> Self {
        Self {
            phase_label,
            phase_step,
            phase_count: INSIGHT_PROGRESS_PHASES,
            processed_items,
            total_items,
            detail,
        }
    }

    pub fn percent(&self) -> f32 {
        let completed_phases = self.phase_step.saturating_sub(1) as f32;
        let phase_fraction = match (self.processed_items, self.total_items) {
            (Some(processed), Some(total)) if total > 0 => {
                (processed.min(total) as f32 / total as f32).clamp(0.0, 1.0)
            }
            _ => 1.0,
        };
        (((completed_phases + phase_fraction) / self.phase_count.max(1) as f32) * 100.0)
            .clamp(0.0, 100.0)
    }
}

fn load_contrast_window_summary(
    connection: &Connection,
    profile_id: Option<&str>,
    window_days: u32,
    limit: Option<usize>,
) -> Result<ContrastWindowSummary> {
    let current_start = Utc::now() - Duration::days(window_days as i64);
    let previous_start = current_start - Duration::days(window_days as i64);
    let current_start_chrome =
        crate::utils::iso_to_chrome_time_micros(&current_start.to_rfc3339()).unwrap_or(0);
    let previous_start_chrome =
        crate::utils::iso_to_chrome_time_micros(&previous_start.to_rfc3339()).unwrap_or(0);
    let previous_visits = load_visits_in_range(
        connection,
        profile_id,
        previous_start_chrome,
        Some(current_start_chrome),
        limit,
    )?;
    Ok(ContrastWindowSummary {
        previous_visit_count: previous_visits.len(),
        previous_unique_domains: previous_visits
            .iter()
            .map(|visit| url_domain(&visit.url))
            .collect::<HashSet<_>>()
            .len(),
    })
}

fn fallback_snapshot_payloads(
    connection: &Connection,
    request: &RunInsightsRequest,
    window_days: u32,
    query_groups: &[InsightQueryGroupSummary],
    topics: &[InsightTopicSummary],
    threads: &[InsightThreadSummary],
    reference_pages: &[InsightReferencePageSummary],
    source_effectiveness: &[InsightSourceEffectivenessSummary],
) -> Result<(
    Vec<InsightQueryLadder>,
    Vec<InsightTemplateSummary>,
    InsightWorkflowMap,
    Vec<InsightProfileFacet>,
    InsightCanonicalSummary,
)> {
    let visits = load_visits(
        connection,
        request.profile_id.as_deref(),
        window_days,
        request.limit.map(|limit| limit.max(1) as usize),
    )?;
    let on_this_day = load_on_this_day(connection, request.profile_id.as_deref(), 8)?;
    let features = load_feature_rows(connection, &visits)?;
    let query_ladders = if query_groups.is_empty() {
        build_query_ladders(&visits, &features)
    } else {
        query_groups
            .iter()
            .filter(|group| group.steps.len() > 1)
            .map(|group| InsightQueryLadder {
                query_group_id: Some(group.query_group_id.clone()),
                root_term: group.root_query.clone(),
                profile_id: group.profile_id.clone(),
                steps: group.steps.clone(),
                stages: group.stages.clone(),
                count: group.visit_count,
                confidence: group.confidence,
                evidence_tier: group.evidence_tier.clone(),
                chromium_only: group.chromium_enhanced,
            })
            .collect()
    };
    let template_summaries = if !query_groups.is_empty()
        || !threads.is_empty()
        || !reference_pages.is_empty()
        || !source_effectiveness.is_empty()
    {
        let contrast_window = load_contrast_window_summary(
            connection,
            request.profile_id.as_deref(),
            window_days,
            request.limit.map(|limit| limit.max(1) as usize),
        )?;
        build_template_summaries(
            &visits,
            query_groups,
            threads,
            reference_pages,
            source_effectiveness,
            request.profile_id.as_deref(),
            window_days,
            Some(&contrast_window),
        )
    } else {
        Vec::new()
    };
    let workflow_map = build_workflow_map(&visits, &features, request.profile_id.as_deref());
    let profile_facets = build_profile_facets(&visits, topics, threads);
    let canonical = build_canonical_summary(&visits, on_this_day);
    Ok((query_ladders, template_summaries, workflow_map, profile_facets, canonical))
}

/// Reports whether derived insight state exists and when it last ran successfully.
pub fn insight_status(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
) -> Result<InsightStatus> {
    if !config.initialized || !paths.archive_database_path.exists() {
        return Ok(InsightStatus::default());
    }
    let connection = open_intelligence_connection(paths, config, key)?;
    ensure_insight_schema(&connection)?;
    let runs = connection
        .query_row("SELECT COUNT(*) FROM insight_runs", [], |row: &Row<'_>| row.get::<_, i64>(0))
        .unwrap_or(0);
    let cards = connection
        .query_row("SELECT COUNT(*) FROM insight_cards", [], |row: &Row<'_>| row.get::<_, i64>(0))
        .unwrap_or(0);
    let topics = connection
        .query_row("SELECT COUNT(*) FROM insight_topics", [], |row: &Row<'_>| row.get::<_, i64>(0))
        .unwrap_or(0);
    let threads = connection
        .query_row("SELECT COUNT(*) FROM insight_threads", [], |row: &Row<'_>| row.get::<_, i64>(0))
        .unwrap_or(0);
    let query_groups = connection
        .query_row("SELECT COUNT(*) FROM insight_query_groups", [], |row: &Row<'_>| {
            row.get::<_, i64>(0)
        })
        .unwrap_or(0);
    let reference_pages = connection
        .query_row("SELECT COUNT(*) FROM insight_reference_pages", [], |row: &Row<'_>| {
            row.get::<_, i64>(0)
        })
        .unwrap_or(0);
    let latest = connection
        .query_row(
            "SELECT finished_at, content_coverage, warning
             FROM insight_runs
             WHERE status = 'success'
             ORDER BY id DESC
             LIMIT 1",
            [],
            |row: &Row<'_>| {
                Ok((
                    row.get::<_, Option<String>>(0)?,
                    row.get::<_, f32>(1)?,
                    row.get::<_, Option<String>>(2)?,
                ))
            },
        )
        .optional()?;
    Ok(InsightStatus {
        ready: cards > 0 || topics > 0 || threads > 0,
        last_run_at: latest.as_ref().and_then(|value| value.0.clone()),
        runs: runs.max(0) as usize,
        cards: cards.max(0) as usize,
        topics: topics.max(0) as usize,
        threads: threads.max(0) as usize,
        query_groups: query_groups.max(0) as usize,
        reference_pages: reference_pages.max(0) as usize,
        content_coverage: latest.as_ref().map(|value| value.1).unwrap_or_default(),
        warning: latest.and_then(|value| value.2),
    })
}

/// Rebuilds derived insight state for one profile/window scope.
pub fn run_insights(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    embedding_provider: Option<&AiProviderRuntime>,
    request: &RunInsightsRequest,
) -> Result<RunInsightsReport> {
    run_insights_with_progress(paths, config, key, embedding_provider, request, |_| Ok(()))
}

/// Rebuilds derived insight state and emits coarse progress snapshots as it advances.
pub fn run_insights_with_progress<F>(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    embedding_provider: Option<&AiProviderRuntime>,
    request: &RunInsightsRequest,
    mut on_progress: F,
) -> Result<RunInsightsReport>
where
    F: FnMut(InsightsRunProgress) -> Result<()>,
{
    let mut connection = open_intelligence_connection(paths, config, key)?;
    ensure_ai_schema(&connection)?;
    ensure_insight_schema(&connection)?;
    ensure_intelligence_runtime_schema(&connection)?;

    let recovery = recover_interrupted_insight_runs(&connection)?;

    let window_days = request.window_days.unwrap_or(DEFAULT_WINDOW_DAYS).clamp(7, 365);
    let profile_scope = request.profile_id.clone().unwrap_or_else(|| "all".to_string());
    let started_at = now_rfc3339();
    connection.execute(
        "INSERT INTO insight_runs (started_at, status, mode, profile_scope, window_days, notes_json)
         VALUES (?1, 'running', 'manual', ?2, ?3, '[]')",
        params![started_at, profile_scope, window_days as i64],
    )?;
    let run_id = connection.last_insert_rowid();
    let run_result = (|| -> Result<RunInsightsReport> {
        let analysis_limit = request.limit.map(|limit| limit.max(1) as usize);
        on_progress(InsightsRunProgress::new(
            "Loading visits",
            1,
            None,
            None,
            Some(format!(
                "Preparing {}-day scope for {}.",
                window_days,
                request.profile_id.as_deref().unwrap_or("all profiles")
            )),
        ))?;
        let mut visits =
            load_visits(&connection, request.profile_id.as_deref(), window_days, analysis_limit)?;
        let query_terms = load_search_term_map(&connection, &visits)?;
        on_progress(InsightsRunProgress::new(
            "Hydrating search terms",
            2,
            Some(visits.len()),
            Some(visits.len()),
            Some(format!("Loaded {} visit rows.", visits.len())),
        ))?;
        hydrate_query_terms(&mut visits, &query_terms);

        on_progress(InsightsRunProgress::new(
            "Refreshing local enrichments",
            3,
            None,
            None,
            Some("Scheduling first-party enrichment jobs for the current window.".to_string()),
        ))?;
        schedule_enrichment_jobs(paths, &connection, config, run_id, &visits)?;
        let enrichment_report = process_enrichment_jobs(paths, &connection, config, &visits)?;

        on_progress(InsightsRunProgress::new(
            "Hydrating visit evidence",
            4,
            None,
            None,
            Some(
                "Joining readable content, normalized titles, and optional embeddings.".to_string(),
            ),
        ))?;
        let enrichments = load_best_enrichment_map(paths, &connection, &visits)?;
        let _ = embedding_provider;
        hydrate_enrichments(&mut visits, &enrichments);
        on_progress(InsightsRunProgress::new(
            "Scoring visits",
            5,
            Some(0),
            Some(visits.len()),
            Some(format!("0 / {} visits", visits.len())),
        ))?;
        compute_feature_scores_with_progress(&mut visits, |processed, total| {
            on_progress(InsightsRunProgress::new(
                "Scoring visits",
                5,
                Some(processed),
                Some(total),
                Some(format!("{processed} / {total} visits")),
            ))
        })?;

        let module_enabled = |module_id: &str| {
            config
                .deterministic
                .modules
                .iter()
                .find(|candidate| candidate.id == module_id)
                .map(|candidate| candidate.enabled)
                .unwrap_or(false)
        };
        on_progress(InsightsRunProgress::new(
            "Building groups and threads",
            6,
            None,
            None,
            Some("Assembling bursts, query groups, threads, and topics.".to_string()),
        ))?;
        let bursts = if module_enabled(QUERY_GROUPS_MODULE_ID) {
            build_bursts(&mut visits)
        } else {
            Vec::new()
        };
        let (mut query_groups, _query_ladders) = if module_enabled(QUERY_GROUPS_MODULE_ID) {
            build_query_groups(&mut visits)
        } else {
            (Vec::new(), Vec::new())
        };
        let thread_records = if module_enabled(THREADS_MODULE_ID) {
            build_thread_records(&mut visits, &mut query_groups)
        } else {
            Vec::new()
        };
        let query_group_summaries = if module_enabled(QUERY_GROUPS_MODULE_ID) {
            query_group_summaries_from_records(&query_groups, &visits)
        } else {
            Vec::new()
        };
        let thread_summaries = if module_enabled(THREADS_MODULE_ID) {
            thread_summaries_from_records(&thread_records)
        } else {
            Vec::new()
        };
        let query_ladders = if module_enabled(QUERY_GROUPS_MODULE_ID) {
            query_group_summaries
                .iter()
                .filter(|group| group.step_count >= 2)
                .map(|group| InsightQueryLadder {
                    query_group_id: Some(group.query_group_id.clone()),
                    root_term: group.root_query.clone(),
                    profile_id: group.profile_id.clone(),
                    steps: group.steps.clone(),
                    stages: group.stages.clone(),
                    count: group.visit_count,
                    confidence: group.confidence,
                    evidence_tier: group.evidence_tier.clone(),
                    chromium_only: group.chromium_enhanced,
                })
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        };
        let topics = if module_enabled(THREADS_MODULE_ID) {
            build_topic_summaries(&visits, &query_group_summaries, &thread_summaries, window_days)
        } else {
            Vec::new()
        };
        let reference_pages = if module_enabled(REFERENCE_PAGES_MODULE_ID) {
            build_reference_pages(&visits, request.profile_id.as_deref().unwrap_or("all"))
        } else {
            Vec::new()
        };
        let source_effectiveness = if module_enabled(SOURCE_EFFECTIVENESS_MODULE_ID) {
            build_source_effectiveness(
                &visits,
                request.profile_id.as_deref().unwrap_or("all"),
                &reference_pages,
            )
        } else {
            Vec::new()
        };
        let contrast_window = if module_enabled(TEMPLATE_SUMMARIES_MODULE_ID) {
            Some(load_contrast_window_summary(
                &connection,
                request.profile_id.as_deref(),
                window_days,
                analysis_limit,
            )?)
        } else {
            None
        };
        let template_summaries = if module_enabled(TEMPLATE_SUMMARIES_MODULE_ID) {
            build_template_summaries(
                &visits,
                &query_group_summaries,
                &thread_summaries,
                &reference_pages,
                &source_effectiveness,
                request.profile_id.as_deref(),
                window_days,
                contrast_window.as_ref(),
            )
        } else {
            Vec::new()
        };
        let cards = build_insight_cards(
            &template_summaries,
            &thread_summaries,
            &reference_pages,
            window_days,
            request.profile_id.as_deref(),
        );
        let features = load_feature_rows(&connection, &visits)?;
        let on_this_day = load_on_this_day(&connection, request.profile_id.as_deref(), 8)?;
        let workflow_map = build_workflow_map(&visits, &features, request.profile_id.as_deref());
        let profile_facets = build_profile_facets(&visits, &topics, &thread_summaries);
        let canonical = build_canonical_summary(&visits, on_this_day);

        on_progress(InsightsRunProgress::new(
            "Persisting deterministic tables",
            7,
            None,
            None,
            Some("Writing rebuilt insight tables back to the local archive.".to_string()),
        ))?;
        let transaction = connection.transaction()?;

        persist_features(&transaction, &visits)?;
        persist_bursts(&transaction, &bursts)?;
        persist_query_groups(&transaction, &profile_scope, window_days, &query_groups, &visits)?;
        persist_topic_summaries(
            &transaction,
            request.profile_id.as_deref().unwrap_or("all"),
            window_days,
            &topics,
        )?;
        persist_thread_records(
            &transaction,
            &profile_scope,
            window_days,
            &thread_records,
            &visits,
        )?;
        persist_reference_pages(&transaction, &profile_scope, window_days, &reference_pages)?;
        persist_source_effectiveness(
            &transaction,
            &profile_scope,
            window_days,
            &source_effectiveness,
        )?;
        persist_cards(&transaction, &cards, request.profile_id.as_deref(), window_days)?;
        persist_snapshot_payloads(
            &transaction,
            &profile_scope,
            window_days,
            &query_ladders,
            &template_summaries,
            &workflow_map,
            &profile_facets,
            &canonical,
        )?;

        let finished_at = now_rfc3339();
        let module_updates = built_in_deterministic_modules()
            .iter()
            .map(|module| DeterministicModuleRuntimeUpdate {
                module_id: module.id.to_string(),
                status: if config
                    .deterministic
                    .modules
                    .iter()
                    .find(|candidate| candidate.id == module.id)
                    .map(|candidate| candidate.enabled)
                    .unwrap_or(false)
                {
                    "ready".to_string()
                } else {
                    "disabled".to_string()
                },
                last_run_id: Some(run_id),
                last_built_at: Some(finished_at.clone()),
                last_invalidated_at: None,
                stale_reason: None,
                notes: vec![format!(
                    "Module {} rebuilt as part of deterministic insights v2.",
                    module.id
                )],
            })
            .collect::<Vec<_>>();
        persist_deterministic_module_runtime_updates(&transaction, &module_updates)?;

        let content_coverage = if visits.is_empty() {
            0.0
        } else {
            visits
                .iter()
                .filter(|visit| {
                    visit.readable_text.as_deref().is_some_and(|value| !value.is_empty())
                })
                .count() as f32
                / visits.len() as f32
        };
        let mut notes = build_run_notes(
            embedding_provider.is_some(),
            config.ai.enrichment_enabled,
            enrichment_report.enriched_visits,
            enrichment_report.failed_enrichments,
            enrichment_report.queued_network_jobs,
            &recovery,
        );
        if let Some(limit) = request.limit.map(|limit| limit.max(1)) {
            notes.push(format!(
                "This refresh analyzed only the latest {} visits because an explicit limit was requested.",
                limit
            ));
        }
        notes.extend(build_taxonomy_review_notes(&visits));
        transaction.execute(
            "UPDATE insight_runs
             SET finished_at = ?1, status = 'success', processed_visits = ?2, enriched_visits = ?3,
                 failed_enrichments = ?4, query_group_count = ?5, topic_count = ?6,
                 thread_count = ?7, reference_page_count = ?8, source_count = ?9,
                 template_summary_count = ?10, card_count = ?11, content_coverage = ?12,
                 warning = NULL, notes_json = ?13
             WHERE id = ?14",
            params![
                finished_at,
                visits.len() as i64,
                enrichment_report.enriched_visits as i64,
                enrichment_report.failed_enrichments as i64,
                query_group_summaries.len() as i64,
                topics.len() as i64,
                thread_summaries.len() as i64,
                reference_pages.len() as i64,
                source_effectiveness.len() as i64,
                template_summaries.len() as i64,
                cards.len() as i64,
                content_coverage,
                serde_json::to_string(&notes)?,
                run_id,
            ],
        )?;
        transaction.commit()?;

        on_progress(InsightsRunProgress::new(
            "Finalizing snapshot",
            8,
            Some(visits.len()),
            Some(visits.len()),
            Some(format!("{} visits processed, {} cards ready.", visits.len(), cards.len())),
        ))?;
        Ok(RunInsightsReport {
            run_id,
            processed_visits: visits.len(),
            enriched_visits: enrichment_report.enriched_visits,
            failed_enrichments: enrichment_report.failed_enrichments,
            query_group_count: query_group_summaries.len(),
            topic_count: topics.len(),
            thread_count: thread_summaries.len(),
            reference_page_count: reference_pages.len(),
            source_count: source_effectiveness.len(),
            template_summary_count: template_summaries.len(),
            card_count: cards.len(),
            content_coverage,
            last_run_at: started_at.clone(),
            notes,
        })
    })();

    match run_result {
        Ok(report) => Ok(report),
        Err(error) => {
            let cancelled = error.downcast_ref::<InsightsRunCancelled>().is_some();
            let failure_warning = if cancelled {
                error.to_string()
            } else {
                format!("Insight refresh stopped before completion: {}", error)
            };
            let failure_notes = if cancelled {
                vec![
                    "PathKeep cancelled the deterministic rebuild before completion.".to_string(),
                    "Any interrupted enrichment work was re-queued for a later refresh."
                        .to_string(),
                ]
            } else {
                vec![
                    "PathKeep kept the canonical archive unchanged.".to_string(),
                    "Any interrupted enrichment work was re-queued for a later refresh."
                        .to_string(),
                ]
            };
            let _ = connection.execute(
                "UPDATE insight_runs
                 SET finished_at = ?1, status = ?2, warning = ?3, notes_json = ?4
                 WHERE id = ?5",
                params![
                    now_rfc3339(),
                    if cancelled { "cancelled" } else { "failed" },
                    failure_warning,
                    serde_json::to_string(&failure_notes)?,
                    run_id,
                ],
            );
            let _ = requeue_running_enrichment_jobs_for_run(&connection, run_id);
            Err(error)
        }
    }
}

/// Loads the persisted insight snapshot for one scope/window.
pub fn load_insights(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    request: &RunInsightsRequest,
) -> Result<InsightSnapshot> {
    let connection = open_intelligence_connection(paths, config, key)?;
    ensure_insight_schema(&connection)?;
    let window_days = request.window_days.unwrap_or(DEFAULT_WINDOW_DAYS).clamp(7, 365);
    let profile_scope = request.profile_id.clone().unwrap_or_else(|| "all".to_string());
    let module_enabled = |module_id: &str| {
        config
            .deterministic
            .modules
            .iter()
            .find(|candidate| candidate.id == module_id)
            .map(|candidate| candidate.enabled)
            .unwrap_or(false)
    };

    let mut cards = load_cards(&connection, &profile_scope, window_days)?;
    let query_groups = if module_enabled(QUERY_GROUPS_MODULE_ID) {
        load_query_groups(&connection, &profile_scope, window_days)?
    } else {
        Vec::new()
    };
    let topics = if module_enabled(THREADS_MODULE_ID) {
        load_topics(&connection, &profile_scope, window_days)?
    } else {
        Vec::new()
    };
    let threads = if module_enabled(THREADS_MODULE_ID) {
        load_threads(&connection, &profile_scope, window_days)?
    } else {
        Vec::new()
    };
    let reference_pages = if module_enabled(REFERENCE_PAGES_MODULE_ID) {
        load_reference_pages(&connection, &profile_scope, window_days)?
    } else {
        Vec::new()
    };
    let source_effectiveness = if module_enabled(SOURCE_EFFECTIVENESS_MODULE_ID) {
        load_source_effectiveness(&connection, &profile_scope, window_days)?
    } else {
        Vec::new()
    };
    let (
        mut query_ladders,
        mut template_summaries,
        mut workflow_map,
        mut profile_facets,
        mut canonical,
    ) = load_snapshot_payloads(&connection, &profile_scope, window_days).or_else(|_| {
        fallback_snapshot_payloads(
            &connection,
            request,
            window_days,
            &query_groups,
            &topics,
            &threads,
            &reference_pages,
            &source_effectiveness,
        )
    })?;
    if !module_enabled(QUERY_GROUPS_MODULE_ID) {
        query_ladders.clear();
    }
    if !module_enabled(TEMPLATE_SUMMARIES_MODULE_ID) {
        template_summaries.clear();
    }
    cards.retain(|card| match card.kind.as_str() {
        "open-loop" => module_enabled(THREADS_MODULE_ID),
        "reference-page" => module_enabled(REFERENCE_PAGES_MODULE_ID),
        "query-groups" | "reference-pages" | "source-effectiveness" => {
            module_enabled(TEMPLATE_SUMMARIES_MODULE_ID)
        }
        _ => true,
    });
    workflow_map.profile_id = request.profile_id.clone();
    canonical.on_this_day.retain(|item| {
        request.profile_id.as_deref().is_none_or(|profile_id| item.profile_id == profile_id)
    });
    if !module_enabled(THREADS_MODULE_ID) {
        profile_facets.retain(|facet| facet.key != "work-rhythm" && facet.key != "interest-shape");
    }
    let status = insight_status(paths, config, key)?;
    let mut notes = connection
        .query_row(
            "SELECT notes_json
             FROM insight_runs
             WHERE status = 'success' AND profile_scope = ?1 AND window_days = ?2
             ORDER BY id DESC LIMIT 1",
            params![profile_scope, window_days as i64],
            |row: &Row<'_>| row.get::<_, String>(0),
        )
        .optional()?
        .and_then(|value| serde_json::from_str::<Vec<String>>(&value).ok())
        .unwrap_or_default();
    if let Some(limit) = request.limit.map(|limit| limit.max(1)) {
        notes.push(format!(
            "This snapshot is limited to the latest {} visits because the caller requested a sampled view.",
            limit
        ));
    }

    Ok(InsightSnapshot {
        generated_at: now_rfc3339(),
        window_days,
        profile_id: request.profile_id.clone(),
        status,
        cards,
        query_groups,
        topics,
        threads,
        query_ladders,
        reference_pages,
        source_effectiveness,
        template_summaries,
        workflow_map,
        profile_facets,
        canonical,
        notes,
    })
}

/// Loads one persisted thread detail and its visit/query-group evidence.
pub fn load_insight_thread_detail(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    thread_id: &str,
) -> Result<InsightThreadDetail> {
    let connection = open_intelligence_connection(paths, config, key)?;
    ensure_insight_schema(&connection)?;
    let summary = connection.query_row(
        "SELECT thread_id, profile_id, title, status, first_seen_at, last_seen_at, visit_count,
                query_group_count, reopen_count, open_loop_score, confidence, evidence_tier,
                dominant_topic_id, chromium_enhanced, evidence_json
         FROM insight_threads WHERE thread_id = ?1",
        [thread_id],
        thread_summary_from_row,
    )?;
    let mut statement = connection.prepare(
        "SELECT visits.id,
                source_profiles.profile_key AS profile_id,
                urls.url,
                urls.title,
                (visits.visit_time_ms * 1000 + 11644473600000000) AS visit_time
         FROM insight_thread_members
         JOIN archive.visits AS visits ON visits.id = insight_thread_members.history_id
         JOIN archive.urls AS urls ON urls.id = visits.url_id
         JOIN archive.source_profiles AS source_profiles ON source_profiles.id = visits.source_profile_id
         WHERE insight_thread_members.thread_id = ?1
           AND visits.reverted_at IS NULL
         ORDER BY insight_thread_members.ordinal ASC",
    )?;
    let visits = statement
        .query_map([thread_id], |row: &Row<'_>| {
            let url: String = row.get(2)?;
            Ok(InsightEvidenceItem {
                history_id: row.get(0)?,
                profile_id: row.get(1)?,
                url: url.clone(),
                title: row.get(3)?,
                visited_at: chrome_time_to_rfc3339(row.get(4)?),
                note: Some(url_domain(&url)),
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let query_groups = load_thread_query_groups(&connection, thread_id)?;
    Ok(InsightThreadDetail { summary, query_groups, visits })
}

/// Clears all rebuildable intelligence tables while keeping canonical facts intact.
pub fn clear_derived_intelligence_state(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
) -> Result<ClearDerivedIntelligenceReport> {
    let connection = open_intelligence_connection(paths, config, key)?;
    ensure_insight_schema(&connection)?;
    ensure_intelligence_runtime_schema(&connection)?;

    let report = ClearDerivedIntelligenceReport {
        cleared_enrichment_rows: table_row_count(&connection, "visit_content_enrichments")?,
        cleared_feature_rows: table_row_count(&connection, "visit_insight_features")?,
        cleared_burst_rows: table_row_count(&connection, "insight_bursts")?,
        cleared_query_group_rows: table_row_count(&connection, "insight_query_groups")?,
        cleared_topic_rows: table_row_count(&connection, "insight_topics")?,
        cleared_thread_rows: table_row_count(&connection, "insight_threads")?,
        cleared_reference_page_rows: table_row_count(&connection, "insight_reference_pages")?,
        cleared_source_rows: table_row_count(&connection, "insight_source_effectiveness")?,
        cleared_module_rows: table_row_count(&connection, "deterministic_module_runtime")?,
        cleared_card_rows: table_row_count(&connection, "insight_cards")?,
        cleared_run_rows: table_row_count(&connection, "insight_runs")?,
        notes: vec![
            "Only derived enrichment and insight tables were cleared.".to_string(),
            "Canonical archive visits, manifests, and rollback state were left untouched."
                .to_string(),
        ],
    };

    clear_derived_insight_state(paths, &connection)?;
    Ok(report)
}

/// Produces an explainability payload for one persisted insight surface.
pub fn explain_insight(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    request: &ExplainInsightRequest,
) -> Result<InsightExplanation> {
    let snapshot = load_insights(
        paths,
        config,
        key,
        &RunInsightsRequest {
            profile_id: request.profile_id.clone(),
            window_days: request.window_days,
            full_rebuild: false,
            limit: None,
        },
    )?;

    match request.insight_kind.as_str() {
        "thread" => {
            let detail = load_insight_thread_detail(paths, config, key, &request.insight_id)?;
            let explanation = format!(
                "{} spans {} visits across {} query groups between {} and {}. It reopened {} times and has an open-loop score of {:.2}, which suggests the task was revisited rather than completed in a single pass.",
                detail.summary.title,
                detail.summary.visit_count,
                detail.summary.query_group_count,
                detail.summary.first_seen_at,
                detail.summary.last_seen_at,
                detail.summary.reopen_count,
                detail.summary.open_loop_score,
            );
            Ok(InsightExplanation {
                explanation,
                used_llm: false,
                citations: detail.visits.into_iter().take(6).collect(),
                notes: vec![
                    "Explanation is generated from persisted thread structure.".to_string(),
                ],
            })
        }
        "query-group" => {
            let group = snapshot
                .query_groups
                .iter()
                .find(|group| group.query_group_id == request.insight_id)
                .cloned()
                .with_context(|| format!("query group {} was not found", request.insight_id))?;
            Ok(InsightExplanation {
                explanation: format!(
                    "\"{}\" evolved through {} steps with {} visits and confidence {:.2}.",
                    group.root_query, group.step_count, group.visit_count, group.confidence
                ),
                used_llm: false,
                citations: group.evidence,
                notes: vec![
                    "Explanation is generated from deterministic query-group evidence.".to_string(),
                ],
            })
        }
        "reference-page" => {
            let page = snapshot
                .reference_pages
                .iter()
                .find(|page| page.reference_page_id == request.insight_id)
                .cloned()
                .with_context(|| format!("reference page {} was not found", request.insight_id))?;
            Ok(InsightExplanation {
                explanation: format!(
                    "{} resurfaced across {} query groups and {} threads.",
                    page.title.clone().unwrap_or_else(|| page.url.clone()),
                    page.query_group_count,
                    page.thread_count,
                ),
                used_llm: false,
                citations: page.evidence,
                notes: vec![
                    "Explanation is generated from deterministic reference-page reuse.".to_string(),
                ],
            })
        }
        "template-summary" => {
            let summary = snapshot
                .template_summaries
                .iter()
                .find(|summary| summary.summary_id == request.insight_id)
                .cloned()
                .with_context(|| {
                    format!("template summary {} was not found", request.insight_id)
                })?;
            Ok(InsightExplanation {
                explanation: summary.body,
                used_llm: false,
                citations: summary.evidence,
                notes: vec![
                    "Explanation is generated from deterministic summary templates.".to_string(),
                ],
            })
        }
        "topic" => {
            let topic = snapshot
                .topics
                .iter()
                .find(|topic| topic.topic_id == request.insight_id)
                .cloned()
                .with_context(|| format!("topic {} was not found", request.insight_id))?;
            let explanation = format!(
                "{} appears {} times in the last {} days. The current trend score is {:.2} and burst score is {:.2}, so it is {} within this window.",
                topic.label,
                topic.visit_count,
                topic.window_days,
                topic.trend_slope,
                topic.burst_score,
                if topic.trend_slope >= 0.12 { "gaining momentum" } else { "stable or cooling" },
            );
            Ok(InsightExplanation {
                explanation,
                used_llm: false,
                citations: topic.evidence,
                notes: vec!["Explanation is generated from topic aggregates.".to_string()],
            })
        }
        _ => {
            let card = snapshot
                .cards
                .iter()
                .find(|card| card.card_id == request.insight_id)
                .cloned()
                .with_context(|| format!("card {} was not found", request.insight_id))?;
            Ok(InsightExplanation {
                explanation: format!("{} {}", card.title, card.summary),
                used_llm: false,
                citations: card.evidence,
                notes: vec![
                    "Explanation is generated from the precomputed card summary.".to_string(),
                ],
            })
        }
    }
}

/// Clears persisted derived intelligence tables and marks runtime modules stale.
fn clear_derived_insight_state(paths: &ProjectPaths, connection: &Connection) -> Result<()> {
    connection.execute("DELETE FROM visit_content_enrichments", [])?;
    crate::intelligence_blobs::clear_readable_text_blobs(paths)?;
    ensure_intelligence_runtime_schema(connection)?;
    connection.execute("DELETE FROM visit_insight_features", [])?;
    connection.execute("DELETE FROM insight_bursts", [])?;
    connection.execute("DELETE FROM insight_query_groups", [])?;
    connection.execute("DELETE FROM insight_query_group_members", [])?;
    connection.execute("DELETE FROM insight_topics", [])?;
    connection.execute("DELETE FROM insight_threads", [])?;
    connection.execute("DELETE FROM insight_thread_members", [])?;
    connection.execute("DELETE FROM insight_reference_pages", [])?;
    connection.execute("DELETE FROM insight_source_effectiveness", [])?;
    connection.execute("DELETE FROM insight_cards", [])?;
    connection.execute("DELETE FROM insight_snapshot_payloads", [])?;
    connection.execute("DELETE FROM insight_runs", [])?;
    connection.execute("DELETE FROM intelligence_jobs", [])?;
    crate::intelligence_runtime::mark_all_deterministic_modules_stale(
        connection,
        "Derived intelligence state was cleared manually.",
    )?;
    Ok(())
}

/// Counts the rows in one derived-intelligence table for clear-report bookkeeping.
fn table_row_count(connection: &Connection, table_name: &str) -> Result<usize> {
    let sql = format!("SELECT COUNT(*) FROM {table_name}");
    Ok(connection.query_row(&sql, [], |row: &Row<'_>| row.get::<_, i64>(0))?.max(0) as usize)
}

#[cfg(test)]
mod tests {
    use super::InsightsRunProgress;

    #[test]
    fn progress_percent_accounts_for_intra_phase_work() {
        let progress = InsightsRunProgress::new(
            "Scoring visits",
            5,
            Some(32),
            Some(64),
            Some("32 / 64 visits".to_string()),
        );

        assert!((progress.percent() - 56.25).abs() < f32::EPSILON);
    }
}
