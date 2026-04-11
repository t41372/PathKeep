mod m5b;
mod site_adapters;

use crate::{
    ai::{AiProviderRuntime, ensure_ai_schema},
    archive::{create_schema, open_archive_connection},
    config::ProjectPaths,
    deterministic::{
        DomainCategory, EvidenceTier, InteractionKind, PageCategory, VisitAnalysisInput,
        analyze_visit, extract_search_query_from_url, tokenize_text,
    },
    intelligence_runtime::{
        DeterministicModuleRuntimeUpdate, ENRICHMENT_JOB_TYPE, EnrichmentJobPayload,
        LOCAL_PLUGIN_SOURCE_KIND, NETWORK_PLUGIN_SOURCE_KIND, built_in_deterministic_modules,
        built_in_enrichment_plugin, built_in_enrichment_plugins, claim_enrichment_jobs,
        enqueue_enrichment_job, enrichment_plugin_enabled, ensure_intelligence_runtime_schema,
        mark_intelligence_job_failed, mark_intelligence_job_succeeded,
        persist_deterministic_module_runtime_updates, requeue_running_enrichment_jobs,
        requeue_running_enrichment_jobs_for_run,
    },
    models::{
        AppConfig, ClearDerivedIntelligenceReport, ExplainInsightRequest, InsightCanonicalSummary,
        InsightCard, InsightDomainStat, InsightEvidenceItem, InsightExplanation,
        InsightProfileFacet, InsightQueryLadder, InsightSnapshot, InsightStatus,
        InsightThreadDetail, InsightThreadSummary, InsightTopicSummary, InsightWorkflowEdge,
        InsightWorkflowMap, InsightWorkflowRole, QUERY_GROUPS_MODULE_ID,
        READABLE_CONTENT_PLUGIN_ID, REFERENCE_PAGES_MODULE_ID, RunInsightsReport,
        RunInsightsRequest, SOURCE_EFFECTIVENESS_MODULE_ID, TEMPLATE_SUMMARIES_MODULE_ID,
        THREADS_MODULE_ID, TITLE_NORMALIZATION_PLUGIN_ID,
    },
    utils::{chrome_time_to_rfc3339, now_rfc3339, sha256_hex, url_domain},
};
use anyhow::{Context, Result};
use chrono::{DateTime, Duration, Local, Utc};
use reqwest::blocking::Client;
use rusqlite::{Connection, OptionalExtension, Row, params};
use scraper::{Html, Selector};
use serde_json::{Value, json};
use site_adapters::adapt_site_content;
use std::collections::{HashMap, HashSet};

use self::m5b::{
    build_bursts, build_cards as build_m5b_cards, build_query_groups, build_reference_pages,
    build_source_effectiveness, build_template_summaries, build_threads as build_m5b_threads,
    build_topics as build_m5b_topics, load_query_groups, load_reference_pages,
    load_source_effectiveness, load_thread_query_groups, persist_bursts, persist_query_groups,
    persist_reference_pages, persist_source_effectiveness, persist_threads as persist_m5b_threads,
    persist_topic_summaries, query_group_summaries_from_records, thread_summaries_from_records,
};

const INSIGHT_PIPELINE_VERSION: &str = "insights-v2";
const ENRICH_TEXT_LIMIT: usize = 12_000;
const SNIPPET_LIMIT: usize = 3;
const DEFAULT_WINDOW_DAYS: u32 = 30;
const DEFAULT_ANALYSIS_LIMIT: usize = 600;
const MAX_NETWORK_ENRICHMENT_JOBS_PER_RUN: usize = 4;
const SESSION_GAP_MINUTES: i64 = 30;

const INSIGHT_SCHEMA_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS visit_content_enrichments (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  history_id INTEGER NOT NULL,
  content_source TEXT NOT NULL,
  fetch_status TEXT NOT NULL,
  fetched_at TEXT NOT NULL,
  final_url TEXT,
  language TEXT,
  readable_title TEXT,
  readable_text TEXT,
  text_hash TEXT,
  snippet_json TEXT NOT NULL,
  extraction_json TEXT NOT NULL,
  pipeline_version TEXT NOT NULL,
  UNIQUE(history_id, content_source)
);
CREATE TABLE IF NOT EXISTS visit_insight_features (
  history_id INTEGER PRIMARY KEY,
  profile_id TEXT NOT NULL,
  burst_id TEXT,
  query_group_id TEXT,
  topic_id TEXT,
  thread_id TEXT,
  page_type TEXT NOT NULL,
  source_role TEXT NOT NULL,
  domain_category TEXT NOT NULL DEFAULT 'unknown',
  page_category TEXT NOT NULL DEFAULT 'unknown',
  interaction_kind TEXT NOT NULL DEFAULT 'unknown',
  evidence_tier TEXT NOT NULL DEFAULT 'tier-c',
  taxonomy_source TEXT NOT NULL DEFAULT 'unknown',
  taxonomy_pack TEXT,
  taxonomy_version TEXT,
  taxonomy_reason TEXT,
  query_term TEXT,
  query_stage TEXT,
  novelty_score REAL NOT NULL,
  importance_score REAL NOT NULL,
  explore_score REAL NOT NULL,
  keywords_json TEXT NOT NULL,
  entities_json TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  pipeline_version TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS insight_bursts (
  burst_id TEXT PRIMARY KEY,
  profile_id TEXT NOT NULL,
  first_seen_at TEXT NOT NULL,
  last_seen_at TEXT NOT NULL,
  visit_count INTEGER NOT NULL,
  confidence REAL NOT NULL,
  evidence_json TEXT NOT NULL,
  pipeline_version TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS insight_query_groups (
  query_group_id TEXT PRIMARY KEY,
  profile_id TEXT NOT NULL,
  thread_id TEXT,
  title TEXT NOT NULL,
  root_query TEXT NOT NULL,
  latest_query TEXT NOT NULL,
  first_seen_at TEXT NOT NULL,
  last_seen_at TEXT NOT NULL,
  visit_count INTEGER NOT NULL,
  burst_count INTEGER NOT NULL,
  step_count INTEGER NOT NULL,
  confidence REAL NOT NULL,
  evidence_tier TEXT NOT NULL,
  chromium_enhanced INTEGER NOT NULL,
  steps_json TEXT NOT NULL,
  stages_json TEXT NOT NULL,
  evidence_json TEXT NOT NULL,
  pipeline_version TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS insight_query_group_members (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  query_group_id TEXT NOT NULL,
  history_id INTEGER NOT NULL,
  ordinal INTEGER NOT NULL,
  visited_at TEXT NOT NULL,
  UNIQUE(query_group_id, history_id)
);
CREATE TABLE IF NOT EXISTS insight_topics (
  topic_id TEXT NOT NULL,
  profile_scope TEXT NOT NULL,
  window_days INTEGER NOT NULL,
  label TEXT NOT NULL,
  first_seen_at TEXT NOT NULL,
  last_seen_at TEXT NOT NULL,
  visit_count INTEGER NOT NULL,
  revisit_count INTEGER NOT NULL,
  trend_slope REAL NOT NULL,
  burst_score REAL NOT NULL,
  evidence_json TEXT NOT NULL,
  PRIMARY KEY(topic_id, profile_scope, window_days)
);
CREATE TABLE IF NOT EXISTS insight_threads (
  thread_id TEXT PRIMARY KEY,
  profile_id TEXT NOT NULL,
  title TEXT NOT NULL,
  status TEXT NOT NULL,
  first_seen_at TEXT NOT NULL,
  last_seen_at TEXT NOT NULL,
  visit_count INTEGER NOT NULL,
  query_group_count INTEGER NOT NULL DEFAULT 0,
  reopen_count INTEGER NOT NULL,
  open_loop_score REAL NOT NULL,
  confidence REAL NOT NULL DEFAULT 0,
  evidence_tier TEXT NOT NULL DEFAULT 'tier-c',
  dominant_topic_id TEXT,
  chromium_enhanced INTEGER NOT NULL,
  evidence_json TEXT NOT NULL,
  summary_json TEXT NOT NULL,
  pipeline_version TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS insight_thread_members (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  thread_id TEXT NOT NULL,
  history_id INTEGER NOT NULL,
  ordinal INTEGER NOT NULL,
  visited_at TEXT NOT NULL,
  UNIQUE(thread_id, history_id)
);
CREATE TABLE IF NOT EXISTS insight_reference_pages (
  reference_page_id TEXT PRIMARY KEY,
  profile_scope TEXT NOT NULL,
  url TEXT NOT NULL,
  title TEXT,
  domain TEXT NOT NULL,
  first_seen_at TEXT NOT NULL,
  last_seen_at TEXT NOT NULL,
  revisit_count INTEGER NOT NULL,
  cross_day_revisits INTEGER NOT NULL,
  query_group_count INTEGER NOT NULL,
  thread_count INTEGER NOT NULL,
  score REAL NOT NULL,
  evidence_tier TEXT NOT NULL,
  evidence_json TEXT NOT NULL,
  pipeline_version TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS insight_source_effectiveness (
  source_id TEXT PRIMARY KEY,
  profile_scope TEXT NOT NULL,
  domain TEXT NOT NULL,
  source_role TEXT NOT NULL,
  query_group_count INTEGER NOT NULL,
  thread_count INTEGER NOT NULL,
  stable_landing_count INTEGER NOT NULL,
  reference_page_count INTEGER NOT NULL,
  reopen_support_count INTEGER NOT NULL,
  effectiveness_score REAL NOT NULL,
  evidence_tier TEXT NOT NULL,
  evidence_json TEXT NOT NULL,
  pipeline_version TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS insight_cards (
  card_id TEXT PRIMARY KEY,
  profile_scope TEXT NOT NULL,
  window_days INTEGER NOT NULL,
  kind TEXT NOT NULL,
  title TEXT NOT NULL,
  summary TEXT NOT NULL,
  score REAL NOT NULL,
  chromium_enhanced INTEGER NOT NULL,
  evidence_json TEXT NOT NULL,
  generated_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS insight_runs (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  started_at TEXT NOT NULL,
  finished_at TEXT,
  status TEXT NOT NULL,
  mode TEXT NOT NULL,
  profile_scope TEXT NOT NULL,
  window_days INTEGER NOT NULL,
  processed_visits INTEGER NOT NULL DEFAULT 0,
  enriched_visits INTEGER NOT NULL DEFAULT 0,
  failed_enrichments INTEGER NOT NULL DEFAULT 0,
  query_group_count INTEGER NOT NULL DEFAULT 0,
  topic_count INTEGER NOT NULL DEFAULT 0,
  thread_count INTEGER NOT NULL DEFAULT 0,
  reference_page_count INTEGER NOT NULL DEFAULT 0,
  source_count INTEGER NOT NULL DEFAULT 0,
  template_summary_count INTEGER NOT NULL DEFAULT 0,
  card_count INTEGER NOT NULL DEFAULT 0,
  content_coverage REAL NOT NULL DEFAULT 0,
  warning TEXT,
  notes_json TEXT NOT NULL DEFAULT '[]'
);
CREATE INDEX IF NOT EXISTS idx_visit_content_enrichments_history_id
  ON visit_content_enrichments(history_id);
CREATE INDEX IF NOT EXISTS idx_visit_content_enrichments_status
  ON visit_content_enrichments(fetch_status, fetched_at);
CREATE INDEX IF NOT EXISTS idx_visit_insight_features_topic_id
  ON visit_insight_features(topic_id);
CREATE INDEX IF NOT EXISTS idx_visit_insight_features_thread_id
  ON visit_insight_features(thread_id);
CREATE INDEX IF NOT EXISTS idx_visit_insight_features_burst_id
  ON visit_insight_features(burst_id);
CREATE INDEX IF NOT EXISTS idx_visit_insight_features_query_group_id
  ON visit_insight_features(query_group_id);
CREATE INDEX IF NOT EXISTS idx_insight_bursts_profile_last_seen
  ON insight_bursts(profile_id, last_seen_at DESC);
CREATE INDEX IF NOT EXISTS idx_insight_query_groups_profile_last_seen
  ON insight_query_groups(profile_id, last_seen_at DESC);
CREATE INDEX IF NOT EXISTS idx_insight_threads_profile_last_seen
  ON insight_threads(profile_id, last_seen_at DESC);
CREATE INDEX IF NOT EXISTS idx_insight_reference_pages_scope_score
  ON insight_reference_pages(profile_scope, score DESC, last_seen_at DESC);
CREATE INDEX IF NOT EXISTS idx_insight_source_effectiveness_scope_score
  ON insight_source_effectiveness(profile_scope, effectiveness_score DESC);
CREATE INDEX IF NOT EXISTS idx_insight_cards_scope_window
  ON insight_cards(profile_scope, window_days, generated_at DESC);
CREATE INDEX IF NOT EXISTS idx_insight_runs_scope_window
  ON insight_runs(profile_scope, window_days, started_at DESC);
"#;

#[derive(Debug, Clone)]
struct VisitRecord {
    history_id: i64,
    profile_id: String,
    source_visit_id: i64,
    source_url_id: i64,
    url: String,
    title: Option<String>,
    visited_at: String,
    visit_time: i64,
    from_visit: Option<i64>,
    #[allow(dead_code)]
    transition: Option<i64>,
    #[allow(dead_code)]
    duration_ms: Option<i64>,
    external_referrer_url: Option<String>,
    #[allow(dead_code)]
    app_id: Option<String>,
    query_term: Option<String>,
    has_canonical_search_term: bool,
    readable_title: Option<String>,
    readable_text: Option<String>,
    snippets: Vec<String>,
    source_role: String,
    page_type: String,
    domain_category: DomainCategory,
    page_category_v2: PageCategory,
    interaction_kind: InteractionKind,
    evidence_tier: EvidenceTier,
    taxonomy_source: String,
    taxonomy_pack: Option<String>,
    taxonomy_version: Option<String>,
    taxonomy_reason: Option<String>,
    registrable_domain: String,
    keywords: Vec<String>,
    entities: Vec<String>,
    novelty_score: f32,
    importance_score: f32,
    explore_score: f32,
    burst_id: Option<String>,
    query_group_id: Option<String>,
    topic_id: Option<String>,
    thread_id: Option<String>,
    vector: Option<Vec<f32>>,
}

#[derive(Debug, Clone, Default)]
struct EnrichmentResult {
    status: String,
    final_url: Option<String>,
    language: Option<String>,
    readable_title: Option<String>,
    readable_text: Option<String>,
    snippets: Vec<String>,
    extraction: Value,
}

#[derive(Debug, Clone)]
struct StoredEnrichment {
    fetch_status: String,
    fetched_at: String,
    readable_title: Option<String>,
    readable_text: Option<String>,
    snippet_json: String,
}

#[derive(Debug, Default)]
struct EnrichmentProcessingReport {
    enriched_visits: usize,
    failed_enrichments: usize,
    queued_network_jobs: usize,
}

#[derive(Debug, Default)]
struct InterruptedInsightRecovery {
    recovered_runs: usize,
    requeued_enrichment_jobs: usize,
}

pub(crate) fn ensure_insight_schema(connection: &Connection) -> Result<()> {
    connection.execute_batch(INSIGHT_SCHEMA_SQL)?;
    ensure_visit_insight_feature_column(connection, "burst_id", "TEXT")?;
    ensure_visit_insight_feature_column(connection, "query_group_id", "TEXT")?;
    ensure_visit_insight_feature_column(
        connection,
        "domain_category",
        "TEXT NOT NULL DEFAULT 'unknown'",
    )?;
    ensure_visit_insight_feature_column(
        connection,
        "page_category",
        "TEXT NOT NULL DEFAULT 'unknown'",
    )?;
    ensure_visit_insight_feature_column(
        connection,
        "interaction_kind",
        "TEXT NOT NULL DEFAULT 'unknown'",
    )?;
    ensure_visit_insight_feature_column(
        connection,
        "evidence_tier",
        "TEXT NOT NULL DEFAULT 'tier-c'",
    )?;
    ensure_visit_insight_feature_column(
        connection,
        "taxonomy_source",
        "TEXT NOT NULL DEFAULT 'unknown'",
    )?;
    ensure_visit_insight_feature_column(connection, "taxonomy_pack", "TEXT")?;
    ensure_visit_insight_feature_column(connection, "taxonomy_version", "TEXT")?;
    ensure_visit_insight_feature_column(connection, "taxonomy_reason", "TEXT")?;
    ensure_table_column(
        connection,
        "insight_threads",
        "query_group_count",
        "INTEGER NOT NULL DEFAULT 0",
    )?;
    ensure_table_column(connection, "insight_threads", "confidence", "REAL NOT NULL DEFAULT 0")?;
    ensure_table_column(
        connection,
        "insight_threads",
        "evidence_tier",
        "TEXT NOT NULL DEFAULT 'tier-c'",
    )?;
    ensure_table_column(
        connection,
        "insight_runs",
        "query_group_count",
        "INTEGER NOT NULL DEFAULT 0",
    )?;
    ensure_table_column(
        connection,
        "insight_runs",
        "reference_page_count",
        "INTEGER NOT NULL DEFAULT 0",
    )?;
    ensure_table_column(connection, "insight_runs", "source_count", "INTEGER NOT NULL DEFAULT 0")?;
    ensure_table_column(
        connection,
        "insight_runs",
        "template_summary_count",
        "INTEGER NOT NULL DEFAULT 0",
    )?;
    Ok(())
}

fn ensure_visit_insight_feature_column(
    connection: &Connection,
    column: &str,
    definition: &str,
) -> Result<()> {
    let mut statement = connection.prepare("PRAGMA table_info(visit_insight_features)")?;
    let columns = statement
        .query_map([], |row: &Row<'_>| row.get::<_, String>(1))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    if columns.iter().any(|existing| existing == column) {
        return Ok(());
    }
    connection.execute(
        &format!("ALTER TABLE visit_insight_features ADD COLUMN {column} {definition}"),
        [],
    )?;
    Ok(())
}

fn ensure_table_column(
    connection: &Connection,
    table: &str,
    column: &str,
    definition: &str,
) -> Result<()> {
    let mut statement = connection.prepare(&format!("PRAGMA table_info({table})"))?;
    let columns = statement
        .query_map([], |row: &Row<'_>| row.get::<_, String>(1))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    if columns.iter().any(|existing| existing == column) {
        return Ok(());
    }
    connection.execute(&format!("ALTER TABLE {table} ADD COLUMN {column} {definition}"), [])?;
    Ok(())
}

pub(crate) fn preferred_embedding_content(
    connection: &Connection,
    history_id: i64,
    profile_id: &str,
    url: &str,
    title: Option<&str>,
    visited_at: &str,
) -> Result<String> {
    let enrichment = best_enrichment_for_history(connection, history_id)?;
    Ok(build_embedding_content_from_parts(
        profile_id,
        url,
        title,
        visited_at,
        enrichment.as_ref().and_then(|value| value.readable_title.as_deref()),
        enrichment.as_ref().and_then(|value| value.readable_text.as_deref()),
    ))
}

pub fn insight_status(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
) -> Result<InsightStatus> {
    if !config.initialized || !paths.archive_database_path.exists() {
        return Ok(InsightStatus::default());
    }
    let connection = open_archive_connection(paths, config, key)?;
    create_schema(&connection)?;
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

pub fn run_insights(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    embedding_provider: Option<&AiProviderRuntime>,
    request: &RunInsightsRequest,
) -> Result<RunInsightsReport> {
    let connection = open_archive_connection(paths, config, key)?;
    create_schema(&connection)?;
    ensure_ai_schema(&connection)?;
    ensure_insight_schema(&connection)?;
    ensure_intelligence_runtime_schema(&connection)?;

    let recovery = recover_interrupted_insight_runs(&connection)?;

    if request.full_rebuild {
        clear_derived_insight_state(&connection)?;
    }

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
        let analysis_limit = request.limit.unwrap_or(DEFAULT_ANALYSIS_LIMIT as u32) as usize;
        let mut visits =
            load_visits(&connection, request.profile_id.as_deref(), window_days, analysis_limit)?;
        let query_terms = load_search_term_map(&connection, request.profile_id.as_deref())?;
        hydrate_query_terms(&mut visits, &query_terms);

        schedule_enrichment_jobs(&connection, config, run_id, &visits)?;
        let enrichment_report = process_enrichment_jobs(&connection, config, &visits)?;

        if embedding_provider.is_some() {
            // Embeddings are refreshed by the worker before analytics. Here we only consume them.
        }

        let enrichments = load_best_enrichment_map(&connection, &visits)?;
        let embeddings = load_embedding_map(&connection, embedding_provider, &visits)?;
        hydrate_enrichment_and_embeddings(&mut visits, &enrichments, &embeddings);
        compute_feature_scores(&mut visits);

        let module_enabled = |module_id: &str| {
            config
                .deterministic
                .modules
                .iter()
                .find(|candidate| candidate.id == module_id)
                .map(|candidate| candidate.enabled)
                .unwrap_or(false)
        };
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
            build_m5b_threads(&mut visits, &mut query_groups)
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
        let topics = if module_enabled(THREADS_MODULE_ID) {
            build_m5b_topics(&visits, &query_group_summaries, &thread_summaries, window_days)
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
        let template_summaries = if module_enabled(TEMPLATE_SUMMARIES_MODULE_ID) {
            build_template_summaries(
                &visits,
                &query_group_summaries,
                &thread_summaries,
                &reference_pages,
                &source_effectiveness,
                request.profile_id.as_deref(),
            )
        } else {
            Vec::new()
        };
        let cards = build_m5b_cards(
            &template_summaries,
            &thread_summaries,
            &reference_pages,
            window_days,
            request.profile_id.as_deref(),
        );

        persist_features(&connection, &visits)?;
        persist_bursts(&connection, &bursts)?;
        persist_query_groups(&connection, &query_groups, &visits)?;
        persist_topic_summaries(
            &connection,
            request.profile_id.as_deref().unwrap_or("all"),
            window_days,
            &topics,
        )?;
        persist_m5b_threads(&connection, &thread_records, &visits)?;
        persist_reference_pages(
            &connection,
            request.profile_id.as_deref().unwrap_or("all"),
            &reference_pages,
        )?;
        persist_source_effectiveness(
            &connection,
            request.profile_id.as_deref().unwrap_or("all"),
            &source_effectiveness,
        )?;
        persist_cards(&connection, &cards, request.profile_id.as_deref(), window_days)?;

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
        persist_deterministic_module_runtime_updates(&connection, &module_updates)?;

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
        notes.extend(build_taxonomy_review_notes(&visits));
        connection.execute(
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
            let failure_warning = format!("Insight refresh stopped before completion: {}", error);
            let failure_notes = vec![
                "PathKeep kept the canonical archive unchanged.".to_string(),
                "Any interrupted enrichment work was re-queued for a later refresh.".to_string(),
            ];
            let _ = connection.execute(
                "UPDATE insight_runs
                 SET finished_at = ?1, status = 'failed', warning = ?2, notes_json = ?3
                 WHERE id = ?4",
                params![
                    now_rfc3339(),
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

pub fn load_insights(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    request: &RunInsightsRequest,
) -> Result<InsightSnapshot> {
    let connection = open_archive_connection(paths, config, key)?;
    create_schema(&connection)?;
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
        load_query_groups(&connection, request.profile_id.as_deref(), window_days)?
    } else {
        Vec::new()
    };
    let topics = if module_enabled(THREADS_MODULE_ID) {
        load_topics(&connection, &profile_scope, window_days)?
    } else {
        Vec::new()
    };
    let threads = if module_enabled(THREADS_MODULE_ID) {
        load_threads(&connection, request.profile_id.as_deref(), window_days)?
    } else {
        Vec::new()
    };
    let reference_pages = if module_enabled(REFERENCE_PAGES_MODULE_ID) {
        load_reference_pages(&connection, &profile_scope)?
    } else {
        Vec::new()
    };
    let source_effectiveness = if module_enabled(SOURCE_EFFECTIVENESS_MODULE_ID) {
        load_source_effectiveness(&connection, &profile_scope)?
    } else {
        Vec::new()
    };
    let visits = load_visits(
        &connection,
        request.profile_id.as_deref(),
        window_days,
        DEFAULT_ANALYSIS_LIMIT,
    )?;
    let on_this_day = load_on_this_day(&connection, request.profile_id.as_deref(), 8)?;
    let features = load_feature_rows(&connection, request.profile_id.as_deref())?;
    let query_ladders = if !module_enabled(QUERY_GROUPS_MODULE_ID) {
        Vec::new()
    } else if query_groups.is_empty() {
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
    let template_summaries = if module_enabled(TEMPLATE_SUMMARIES_MODULE_ID) {
        build_template_summaries(
            &visits,
            &query_groups,
            &threads,
            &reference_pages,
            &source_effectiveness,
            request.profile_id.as_deref(),
        )
    } else {
        Vec::new()
    };
    cards.retain(|card| match card.kind.as_str() {
        "open-loop" => module_enabled(THREADS_MODULE_ID),
        "reference-page" => module_enabled(REFERENCE_PAGES_MODULE_ID),
        "query-groups" | "reference-pages" | "source-effectiveness" => {
            module_enabled(TEMPLATE_SUMMARIES_MODULE_ID)
        }
        _ => true,
    });
    let workflow_map = build_workflow_map(&visits, &features, request.profile_id.as_deref());
    let profile_facets = build_profile_facets(&visits, &topics, &threads);
    let canonical = build_canonical_summary(&visits, on_this_day);
    let status = insight_status(paths, config, key)?;
    let notes = connection
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

pub fn load_insight_thread_detail(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    thread_id: &str,
) -> Result<InsightThreadDetail> {
    let connection = open_archive_connection(paths, config, key)?;
    create_schema(&connection)?;
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
        "SELECT visit_events.id, visit_events.profile_id, visit_events.url, visit_events.title, visit_events.visit_time
         FROM insight_thread_members
         JOIN visit_events ON visit_events.id = insight_thread_members.history_id
         WHERE insight_thread_members.thread_id = ?1
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

pub fn clear_derived_intelligence_state(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
) -> Result<ClearDerivedIntelligenceReport> {
    let connection = open_archive_connection(paths, config, key)?;
    create_schema(&connection)?;
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

    clear_derived_insight_state(&connection)?;
    Ok(report)
}

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

fn clear_derived_insight_state(connection: &Connection) -> Result<()> {
    connection.execute("DELETE FROM visit_content_enrichments", [])?;
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
    connection.execute("DELETE FROM insight_runs", [])?;
    connection
        .execute("DELETE FROM intelligence_jobs WHERE job_type = ?1", [ENRICHMENT_JOB_TYPE])?;
    crate::intelligence_runtime::mark_all_deterministic_modules_stale(
        connection,
        "Derived intelligence state was cleared manually.",
    )?;
    Ok(())
}

fn table_row_count(connection: &Connection, table_name: &str) -> Result<usize> {
    let sql = format!("SELECT COUNT(*) FROM {table_name}");
    Ok(connection.query_row(&sql, [], |row: &Row<'_>| row.get::<_, i64>(0))?.max(0) as usize)
}

fn load_visits(
    connection: &Connection,
    profile_id: Option<&str>,
    window_days: u32,
    limit: usize,
) -> Result<Vec<VisitRecord>> {
    let start = (Utc::now() - Duration::days(window_days as i64)).to_rfc3339();
    let start_chrome = crate::utils::iso_to_chrome_time_micros(&start).unwrap_or(0);
    let sql = if profile_id.is_some() {
        "SELECT id, profile_id, source_visit_id, source_url_id, url, title, visit_time,
                from_visit, transition, visit_duration, external_referrer_url, app_id
         FROM visit_events
         WHERE profile_id = ?1 AND visit_time >= ?2
         ORDER BY visit_time ASC
         LIMIT ?3"
    } else {
        "SELECT id, profile_id, source_visit_id, source_url_id, url, title, visit_time,
                from_visit, transition, visit_duration, external_referrer_url, app_id
         FROM visit_events
         WHERE visit_time >= ?1
         ORDER BY visit_time ASC
         LIMIT ?2"
    };
    let mut statement = connection.prepare(sql)?;
    let rows = if let Some(profile_id) = profile_id {
        statement
            .query_map(params![profile_id, start_chrome, limit as i64], visit_record_from_row)?
    } else {
        statement.query_map(params![start_chrome, limit as i64], visit_record_from_row)?
    };
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

fn load_on_this_day(
    connection: &Connection,
    profile_id: Option<&str>,
    limit: usize,
) -> Result<Vec<InsightEvidenceItem>> {
    let today_key = Local::now().format("%m-%d").to_string();
    let sql = if profile_id.is_some() {
        "SELECT id, profile_id, url, title, visit_time
         FROM visit_events
         WHERE profile_id = ?1
           AND strftime('%m-%d', datetime(visit_time / 1000000 - 11644473600, 'unixepoch', 'localtime')) = ?2
         ORDER BY visit_time DESC
         LIMIT ?3"
    } else {
        "SELECT id, profile_id, url, title, visit_time
         FROM visit_events
         WHERE strftime('%m-%d', datetime(visit_time / 1000000 - 11644473600, 'unixepoch', 'localtime')) = ?1
         ORDER BY visit_time DESC
         LIMIT ?2"
    };
    let mut statement = connection.prepare(sql)?;
    let rows = if let Some(profile_id) = profile_id {
        statement
            .query_map(params![profile_id, today_key, limit as i64], insight_evidence_from_row)?
    } else {
        statement.query_map(params![today_key, limit as i64], insight_evidence_from_row)?
    };
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

fn build_canonical_summary(
    visits: &[VisitRecord],
    on_this_day: Vec<InsightEvidenceItem>,
) -> InsightCanonicalSummary {
    let mut domain_counts = HashMap::<String, usize>::new();
    for visit in visits {
        *domain_counts.entry(url_domain(&visit.url)).or_default() += 1;
    }
    let mut top_domains = domain_counts
        .into_iter()
        .map(|(domain, visit_count)| InsightDomainStat { domain, visit_count })
        .collect::<Vec<_>>();
    top_domains.sort_by(|left, right| {
        right.visit_count.cmp(&left.visit_count).then_with(|| left.domain.cmp(&right.domain))
    });
    top_domains.truncate(5);

    InsightCanonicalSummary {
        window_visit_count: visits.len(),
        window_unique_domains: visits
            .iter()
            .map(|visit| url_domain(&visit.url))
            .collect::<HashSet<_>>()
            .len(),
        on_this_day,
        top_domains,
    }
}

fn visit_record_from_row(row: &Row<'_>) -> rusqlite::Result<VisitRecord> {
    let url: String = row.get(4)?;
    let visit_time: i64 = row.get(6)?;
    Ok(VisitRecord {
        history_id: row.get(0)?,
        profile_id: row.get(1)?,
        source_visit_id: row.get(2)?,
        source_url_id: row.get(3)?,
        url: url.clone(),
        title: row.get(5)?,
        visited_at: chrome_time_to_rfc3339(visit_time),
        visit_time,
        from_visit: row.get(7)?,
        transition: row.get(8)?,
        duration_ms: row.get(9)?,
        external_referrer_url: row.get(10)?,
        app_id: row.get(11)?,
        query_term: None,
        has_canonical_search_term: false,
        readable_title: None,
        readable_text: None,
        snippets: Vec::new(),
        source_role: "general".to_string(),
        page_type: "page".to_string(),
        domain_category: DomainCategory::Unknown,
        page_category_v2: PageCategory::Unknown,
        interaction_kind: InteractionKind::Unknown,
        evidence_tier: EvidenceTier::TierC,
        taxonomy_source: "unknown".to_string(),
        taxonomy_pack: None,
        taxonomy_version: None,
        taxonomy_reason: None,
        registrable_domain: String::new(),
        keywords: Vec::new(),
        entities: Vec::new(),
        novelty_score: 0.0,
        importance_score: 0.0,
        explore_score: 0.0,
        burst_id: None,
        query_group_id: None,
        topic_id: None,
        thread_id: None,
        vector: None,
    })
}

fn insight_evidence_from_row(row: &Row<'_>) -> rusqlite::Result<InsightEvidenceItem> {
    let url: String = row.get(2)?;
    let visit_time: i64 = row.get(4)?;
    Ok(InsightEvidenceItem {
        history_id: row.get(0)?,
        profile_id: row.get(1)?,
        url: url.clone(),
        title: row.get(3)?,
        visited_at: chrome_time_to_rfc3339(visit_time),
        note: Some(url_domain(&url)),
    })
}

fn load_search_term_map(
    connection: &Connection,
    profile_id: Option<&str>,
) -> Result<HashMap<(String, i64), String>> {
    let sql = if profile_id.is_some() {
        "SELECT profile_id, url_id, normalized_term
         FROM search_terms
         WHERE profile_id = ?1
           AND reverted_at IS NULL"
    } else {
        "SELECT profile_id, url_id, normalized_term
         FROM search_terms
         WHERE reverted_at IS NULL"
    };
    let mut statement = connection.prepare(sql)?;
    let mut map = HashMap::new();
    if let Some(profile_id) = profile_id {
        let rows = statement.query_map([profile_id], |row: &Row<'_>| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?, row.get::<_, String>(2)?))
        })?;
        for row in rows {
            let (profile_id, url_id, term) = row?;
            map.entry((profile_id, url_id)).or_insert(term);
        }
    } else {
        let rows = statement.query_map([], |row: &Row<'_>| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?, row.get::<_, String>(2)?))
        })?;
        for row in rows {
            let (profile_id, url_id, term) = row?;
            map.entry((profile_id, url_id)).or_insert(term);
        }
    }
    Ok(map)
}

fn hydrate_query_terms(visits: &mut [VisitRecord], query_terms: &HashMap<(String, i64), String>) {
    for visit in visits {
        let canonical_query = query_terms.get(&(visit.profile_id.clone(), visit.source_url_id));
        visit.has_canonical_search_term = canonical_query.is_some();
        visit.query_term = canonical_query.cloned().or_else(|| query_term_from_url(&visit.url));
    }
}

fn is_refetch_eligible(visit: &VisitRecord) -> bool {
    is_chromium_profile(&visit.profile_id)
        && (visit.url.starts_with("https://") || visit.url.starts_with("http://"))
        && !visit.url.contains("localhost")
        && !visit.url.contains("127.0.0.1")
        && !visit.url.starts_with("http://chrome")
        && !visit.url.starts_with("https://chrome")
}

fn schedule_enrichment_jobs(
    connection: &Connection,
    config: &AppConfig,
    run_id: i64,
    visits: &[VisitRecord],
) -> Result<()> {
    if !config.ai.enrichment_enabled {
        return Ok(());
    }

    for visit in visits {
        for plugin in built_in_enrichment_plugins() {
            if !enrichment_plugin_enabled(config, plugin.id) {
                continue;
            }
            if !plugin_is_eligible(plugin.id, visit) {
                continue;
            }
            if plugin_enrichment_is_fresh(connection, visit.history_id, plugin.id)? {
                continue;
            }
            enqueue_enrichment_job(
                connection,
                run_id,
                plugin,
                &EnrichmentJobPayload {
                    history_id: visit.history_id,
                    profile_id: visit.profile_id.clone(),
                    url: visit.url.clone(),
                    title: visit.title.clone(),
                },
            )?;
        }
    }

    Ok(())
}

fn process_enrichment_jobs(
    connection: &Connection,
    config: &AppConfig,
    visits: &[VisitRecord],
) -> Result<EnrichmentProcessingReport> {
    if !config.ai.enrichment_enabled {
        return Ok(EnrichmentProcessingReport::default());
    }

    let allowed_plugins = built_in_enrichment_plugins()
        .iter()
        .filter(|plugin| enrichment_plugin_enabled(config, plugin.id))
        .copied()
        .collect::<Vec<_>>();
    if allowed_plugins.is_empty() {
        return Ok(EnrichmentProcessingReport::default());
    }

    let allowed_history_ids = visits.iter().map(|visit| visit.history_id).collect::<HashSet<_>>();
    let local_plugin_ids = allowed_plugins
        .iter()
        .filter(|plugin| plugin.source_kind == LOCAL_PLUGIN_SOURCE_KIND)
        .map(|plugin| plugin.id.to_string())
        .collect::<Vec<_>>();
    let network_plugin_ids = allowed_plugins
        .iter()
        .filter(|plugin| plugin.source_kind == NETWORK_PLUGIN_SOURCE_KIND)
        .map(|plugin| plugin.id.to_string())
        .collect::<Vec<_>>();

    let mut claimed = Vec::new();
    if !local_plugin_ids.is_empty() {
        claimed.extend(claim_enrichment_jobs(
            connection,
            &local_plugin_ids,
            &allowed_history_ids,
            visits.len().max(1) * local_plugin_ids.len(),
        )?);
    }
    if !network_plugin_ids.is_empty() {
        claimed.extend(claim_enrichment_jobs(
            connection,
            &network_plugin_ids,
            &allowed_history_ids,
            MAX_NETWORK_ENRICHMENT_JOBS_PER_RUN,
        )?);
    }
    if claimed.is_empty() {
        return Ok(EnrichmentProcessingReport::default());
    }

    let visit_map = visits.iter().map(|visit| (visit.history_id, visit)).collect::<HashMap<_, _>>();
    let mut report = EnrichmentProcessingReport::default();
    let mut refetch_client = None::<Client>;

    for job in claimed {
        let Some(visit) = visit_map.get(&job.payload.history_id) else {
            mark_intelligence_job_failed(
                connection,
                job.id,
                "The queued visit no longer exists in the current insight window.",
            )?;
            report.failed_enrichments += 1;
            continue;
        };

        let enrichment = match job.plugin_id.as_str() {
            TITLE_NORMALIZATION_PLUGIN_ID => title_normalization_enrichment(visit),
            READABLE_CONTENT_PLUGIN_ID => {
                let client = match refetch_client.as_ref() {
                    Some(client) => client,
                    None => {
                        refetch_client = Some(build_refetch_client()?);
                        refetch_client.as_ref().expect("refetch client")
                    }
                };
                refetch_visit_content(client, &visit.url)
            }
            _ => {
                mark_intelligence_job_failed(
                    connection,
                    job.id,
                    &format!("Unknown enrichment plugin {}", job.plugin_id),
                )?;
                report.failed_enrichments += 1;
                continue;
            }
        };

        store_enrichment(connection, visit.history_id, &job.plugin_id, &enrichment)?;
        let artifact = json!({
            "status": enrichment.status,
            "snippetCount": enrichment.snippets.len(),
            "textLength": enrichment
                .readable_text
                .as_ref()
                .map(|value| value.len())
                .unwrap_or(0),
            "attempt": job.attempt,
        });
        if enrichment_is_terminal_failure(&enrichment) {
            report.failed_enrichments += 1;
            mark_intelligence_job_failed(
                connection,
                job.id,
                &enrichment_failure_message(&enrichment),
            )?;
        } else {
            if enrichment.status == "success" {
                report.enriched_visits += 1;
            }
            mark_intelligence_job_succeeded(connection, job.id, &artifact)?;
        }
    }

    if !network_plugin_ids.is_empty() {
        report.queued_network_jobs = queued_enrichment_jobs(connection, &network_plugin_ids)?;
    }

    Ok(report)
}

fn recover_interrupted_insight_runs(connection: &Connection) -> Result<InterruptedInsightRecovery> {
    let recovered_runs = connection
        .query_row(
            "SELECT COUNT(*) FROM insight_runs WHERE status = 'running'",
            [],
            |row: &Row<'_>| row.get::<_, i64>(0),
        )
        .unwrap_or(0)
        .max(0) as usize;
    if recovered_runs == 0 {
        return Ok(InterruptedInsightRecovery::default());
    }

    let now = now_rfc3339();
    connection.execute(
        "UPDATE insight_runs
         SET finished_at = ?1, status = 'failed', warning = ?2
         WHERE status = 'running'",
        params![now, "A previous insight refresh was interrupted before completion.",],
    )?;
    let requeued_enrichment_jobs = requeue_running_enrichment_jobs(connection)?;
    Ok(InterruptedInsightRecovery { recovered_runs, requeued_enrichment_jobs })
}

fn queued_enrichment_jobs(connection: &Connection, plugin_ids: &[String]) -> Result<usize> {
    let mut total = 0usize;
    for plugin_id in plugin_ids {
        let queued = connection
            .query_row(
                "SELECT COUNT(*)
                 FROM intelligence_jobs
                 WHERE job_type = ?1 AND plugin_id = ?2 AND state = 'queued'",
                params![ENRICHMENT_JOB_TYPE, plugin_id],
                |row: &Row<'_>| row.get::<_, i64>(0),
            )
            .unwrap_or(0)
            .max(0) as usize;
        total += queued;
    }
    Ok(total)
}

fn plugin_is_eligible(plugin_id: &str, visit: &VisitRecord) -> bool {
    match plugin_id {
        TITLE_NORMALIZATION_PLUGIN_ID => {
            visit.title.as_deref().is_some_and(|value| !value.trim().is_empty())
                || visit.url.starts_with("https://")
                || visit.url.starts_with("http://")
        }
        READABLE_CONTENT_PLUGIN_ID => is_refetch_eligible(visit),
        _ => false,
    }
}

fn plugin_enrichment_is_fresh(
    connection: &Connection,
    history_id: i64,
    plugin_id: &str,
) -> Result<bool> {
    let record = enrichment_for_history_and_source(connection, history_id, plugin_id)?;
    let Some(record) = record else {
        return Ok(false);
    };
    if record.fetch_status != "success" {
        return Ok(false);
    }
    let definition = built_in_enrichment_plugin(plugin_id);

    match plugin_id {
        TITLE_NORMALIZATION_PLUGIN_ID => {
            Ok(record.readable_title.as_deref().is_some_and(|value| !value.trim().is_empty()))
        }
        READABLE_CONTENT_PLUGIN_ID => {
            let fetched_at = DateTime::parse_from_rfc3339(&record.fetched_at)
                .ok()
                .map(|value| value.with_timezone(&Utc));
            let still_fresh = fetched_at
                .zip(definition.and_then(|plugin| plugin.freshness_window_days))
                .map(|(value, days)| Utc::now() - value <= Duration::days(days))
                .unwrap_or(false);
            Ok(still_fresh
                && record.readable_text.as_deref().is_some_and(|value| !value.trim().is_empty()))
        }
        _ => Ok(false),
    }
}

fn title_normalization_enrichment(visit: &VisitRecord) -> EnrichmentResult {
    let readable_title = visit
        .title
        .as_deref()
        .map(normalize_whitespace)
        .filter(|value| !value.is_empty())
        .or_else(|| normalized_title_from_url(&visit.url));
    let status = if readable_title.is_some() { "success" } else { "empty" };
    let snippets = readable_title.clone().into_iter().collect::<Vec<_>>();
    EnrichmentResult {
        status: status.to_string(),
        final_url: Some(visit.url.clone()),
        language: None,
        readable_title,
        readable_text: None,
        snippets,
        extraction: json!({
            "strategy": if visit.title.is_some() { "browser-title" } else { "url-fallback" },
        }),
    }
}

fn normalized_title_from_url(url: &str) -> Option<String> {
    let parsed = reqwest::Url::parse(url).ok()?;
    let last_segment = parsed
        .path_segments()
        .and_then(|mut segments| segments.rfind(|segment| !segment.is_empty()))
        .map(percent_decode)?;
    let candidate = normalize_whitespace(&last_segment.replace(['-', '_'], " "));
    if candidate.is_empty() { None } else { Some(candidate) }
}

fn enrichment_is_terminal_failure(enrichment: &EnrichmentResult) -> bool {
    matches!(enrichment.status.as_str(), "fetch-error" | "decode-error" | "unsupported-content")
}

fn enrichment_failure_message(enrichment: &EnrichmentResult) -> String {
    enrichment
        .extraction
        .get("error")
        .and_then(Value::as_str)
        .map(ToString::to_string)
        .unwrap_or_else(|| format!("Enrichment failed with status {}", enrichment.status))
}

fn build_refetch_client() -> Result<Client> {
    Client::builder()
        .redirect(reqwest::redirect::Policy::limited(5))
        .connect_timeout(std::time::Duration::from_secs(3))
        .timeout(std::time::Duration::from_secs(4))
        .user_agent("PathKeep Insights/0.1")
        .build()
        .context("building content refetch client")
}

fn refetch_visit_content(client: &Client, url: &str) -> EnrichmentResult {
    let response = match client.get(url).send() {
        Ok(response) => response,
        Err(error) => {
            return EnrichmentResult {
                status: "fetch-error".to_string(),
                extraction: json!({ "error": error.to_string() }),
                ..EnrichmentResult::default()
            };
        }
    };
    let final_url = Some(response.url().to_string());
    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_string();
    let text = match response.text() {
        Ok(text) => text,
        Err(error) => {
            return EnrichmentResult {
                status: "decode-error".to_string(),
                final_url,
                extraction: json!({ "error": error.to_string(), "contentType": content_type }),
                ..EnrichmentResult::default()
            };
        }
    };
    if !content_type.is_empty() && !content_type.contains("html") {
        return EnrichmentResult {
            status: "unsupported-content".to_string(),
            final_url,
            extraction: json!({ "contentType": content_type }),
            ..EnrichmentResult::default()
        };
    }
    let document = Html::parse_document(&text);
    let title_selector = Selector::parse("title").expect("selector");
    let html_selector = Selector::parse("html").expect("selector");
    let body_selector = Selector::parse("main, article, body").expect("selector");
    let block_selector =
        Selector::parse("p, li, h1, h2, h3, h4, h5, h6, pre, code").expect("selector");

    let readable_title = document
        .select(&title_selector)
        .next()
        .map(|node| normalize_whitespace(&node.text().collect::<Vec<_>>().join(" ")))
        .filter(|value| !value.is_empty());
    let language = document
        .select(&html_selector)
        .next()
        .and_then(|node| node.value().attr("lang"))
        .map(ToString::to_string);

    let mut blocks = Vec::new();
    if let Some(root) = document.select(&body_selector).next() {
        for node in root.select(&block_selector) {
            let value = normalize_whitespace(&node.text().collect::<Vec<_>>().join(" "));
            if !value.is_empty() {
                blocks.push(value);
            }
        }
        if blocks.is_empty() {
            let fallback = normalize_whitespace(&root.text().collect::<Vec<_>>().join(" "));
            if !fallback.is_empty() {
                blocks.push(fallback);
            }
        }
    }

    let snippets = blocks.iter().take(SNIPPET_LIMIT).cloned().collect::<Vec<_>>();
    let readable_text = truncate_text(&blocks.join("\n\n"), ENRICH_TEXT_LIMIT);
    let generic_result = EnrichmentResult {
        status: if readable_text.is_empty() { "empty".to_string() } else { "success".to_string() },
        final_url,
        language,
        readable_title,
        readable_text: (!readable_text.is_empty()).then_some(readable_text),
        snippets: snippets.clone(),
        extraction: json!({
            "contentType": content_type,
            "snippetCount": snippets.len(),
            "textLength": snippets.iter().map(|value| value.len()).sum::<usize>(),
        }),
    };

    if let Some(adapter) = adapt_site_content(url, &document) {
        let readable_title =
            adapter.readable_title.or_else(|| generic_result.readable_title.clone());
        let readable_text = adapter
            .readable_text
            .map(|value| truncate_text(&value, ENRICH_TEXT_LIMIT))
            .or_else(|| generic_result.readable_text.clone());
        let snippets = if adapter.snippets.is_empty() {
            generic_result.snippets.clone()
        } else {
            adapter.snippets.into_iter().take(SNIPPET_LIMIT).collect()
        };

        return EnrichmentResult {
            status: if readable_text.as_deref().is_some_and(|value| !value.is_empty()) {
                "success".to_string()
            } else {
                generic_result.status
            },
            final_url: generic_result.final_url,
            language: generic_result.language,
            readable_title,
            readable_text,
            snippets: snippets.clone(),
            extraction: json!({
                "contentType": content_type,
                "snippetCount": snippets.len(),
                "textLength": snippets.iter().map(|value| value.len()).sum::<usize>(),
                "siteAdapter": {
                    "id": adapter.adapter_id,
                    "metadata": adapter.metadata,
                },
            }),
        };
    }

    generic_result
}

fn store_enrichment(
    connection: &Connection,
    history_id: i64,
    content_source: &str,
    enrichment: &EnrichmentResult,
) -> Result<()> {
    let text_hash = enrichment.readable_text.as_deref().map(|value| sha256_hex(value.as_bytes()));
    connection.execute(
        "INSERT OR REPLACE INTO visit_content_enrichments
         (history_id, content_source, fetch_status, fetched_at, final_url, language, readable_title,
          readable_text, text_hash, snippet_json, extraction_json, pipeline_version)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
        params![
            history_id,
            content_source,
            enrichment.status,
            now_rfc3339(),
            enrichment.final_url,
            enrichment.language,
            enrichment.readable_title,
            enrichment.readable_text,
            text_hash,
            serde_json::to_string(&enrichment.snippets)?,
            serde_json::to_string(&enrichment.extraction)?,
            INSIGHT_PIPELINE_VERSION,
        ],
    )?;
    Ok(())
}

fn enrichment_for_history_and_source(
    connection: &Connection,
    history_id: i64,
    content_source: &str,
) -> Result<Option<StoredEnrichment>> {
    let mut statement = connection.prepare(
        "SELECT content_source, fetch_status, fetched_at, final_url, language, readable_title,
                readable_text, snippet_json
         FROM visit_content_enrichments
         WHERE history_id = ?1 AND content_source = ?2
         ORDER BY fetched_at DESC
         LIMIT 1",
    )?;
    statement
        .query_row(params![history_id, content_source], |row| {
            let _: Option<String> = row.get(3)?;
            let _: Option<String> = row.get(4)?;
            Ok(StoredEnrichment {
                fetch_status: row.get(1)?,
                fetched_at: row.get(2)?,
                readable_title: row.get(5)?,
                readable_text: row.get(6)?,
                snippet_json: row.get(7)?,
            })
        })
        .optional()
        .map_err(Into::into)
}

fn best_enrichment_for_history(
    connection: &Connection,
    history_id: i64,
) -> Result<Option<StoredEnrichment>> {
    let mut statement = connection.prepare(
        "SELECT content_source, fetch_status, fetched_at, final_url, language, readable_title,
                readable_text, snippet_json
         FROM visit_content_enrichments
         WHERE history_id = ?1
         ORDER BY
           CASE fetch_status WHEN 'success' THEN 0 WHEN 'empty' THEN 1 ELSE 2 END,
           CASE content_source
             WHEN 'capture' THEN 0
             WHEN 'readable-content-refetch' THEN 1
             WHEN 'title-normalization' THEN 2
             ELSE 3
           END,
           fetched_at DESC",
    )?;
    statement
        .query_row([history_id], |row: &Row<'_>| {
            let _: Option<String> = row.get(3)?;
            let _: Option<String> = row.get(4)?;
            Ok(StoredEnrichment {
                fetch_status: row.get(1)?,
                fetched_at: row.get(2)?,
                readable_title: row.get(5)?,
                readable_text: row.get(6)?,
                snippet_json: row.get(7)?,
            })
        })
        .optional()
        .map_err(Into::into)
}

fn load_best_enrichment_map(
    connection: &Connection,
    visits: &[VisitRecord],
) -> Result<HashMap<i64, StoredEnrichment>> {
    let mut map = HashMap::new();
    for visit in visits {
        if let Some(value) = best_enrichment_for_history(connection, visit.history_id)? {
            map.insert(visit.history_id, value);
        }
    }
    Ok(map)
}

fn load_embedding_map(
    connection: &Connection,
    embedding_provider: Option<&AiProviderRuntime>,
    visits: &[VisitRecord],
) -> Result<HashMap<i64, Vec<f32>>> {
    let provider_filter = embedding_provider.map(|value| value.config.id.clone());
    let model_filter = embedding_provider.map(|value| value.config.default_model.clone());
    let sql = if provider_filter.is_some() && model_filter.is_some() {
        "SELECT history_id, embedding_json
         FROM ai_embeddings
         WHERE provider_id = ?1 AND model = ?2"
    } else {
        "SELECT history_id, embedding_json
         FROM ai_embeddings
         WHERE rowid IN (
           SELECT MAX(rowid) FROM ai_embeddings GROUP BY history_id
         )"
    };
    let mut statement = connection.prepare(sql)?;
    let allowed = visits.iter().map(|visit| visit.history_id).collect::<HashSet<_>>();
    let mut map = HashMap::new();
    if let (Some(provider_id), Some(model)) = (provider_filter, model_filter) {
        let rows = statement.query_map(params![provider_id, model], |row: &Row<'_>| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
        })?;
        for row in rows {
            let (history_id, embedding_json) = row?;
            if !allowed.contains(&history_id) {
                continue;
            }
            if let Ok(vector) = serde_json::from_str::<Vec<f32>>(&embedding_json) {
                map.insert(history_id, vector);
            }
        }
    } else {
        let rows = statement
            .query_map([], |row: &Row<'_>| Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?)))?;
        for row in rows {
            let (history_id, embedding_json) = row?;
            if !allowed.contains(&history_id) {
                continue;
            }
            if let Ok(vector) = serde_json::from_str::<Vec<f32>>(&embedding_json) {
                map.insert(history_id, vector);
            }
        }
    }
    Ok(map)
}

fn hydrate_enrichment_and_embeddings(
    visits: &mut [VisitRecord],
    enrichments: &HashMap<i64, StoredEnrichment>,
    embeddings: &HashMap<i64, Vec<f32>>,
) {
    for visit in visits {
        if let Some(enrichment) = enrichments.get(&visit.history_id) {
            visit.readable_title = enrichment.readable_title.clone();
            visit.readable_text = enrichment.readable_text.clone();
            visit.snippets =
                serde_json::from_str::<Vec<String>>(&enrichment.snippet_json).unwrap_or_default();
        }
        let analysis = analyze_visit(
            VisitAnalysisInput {
                url: &visit.url,
                title: visit.readable_title.as_deref().or(visit.title.as_deref()),
                query: visit.query_term.as_deref(),
                has_canonical_search_term: visit.has_canonical_search_term,
                external_referrer_url: visit.external_referrer_url.as_deref(),
                from_visit: visit.from_visit,
            },
            &[],
        );
        visit.domain_category = analysis.taxonomy.domain_category;
        visit.page_category_v2 = analysis.taxonomy.page_category;
        visit.interaction_kind = analysis.taxonomy.interaction_kind;
        visit.evidence_tier = analysis.evidence.tier;
        visit.taxonomy_source = analysis.taxonomy.source.as_str().to_string();
        visit.taxonomy_pack = analysis.taxonomy.rule_pack.clone();
        visit.taxonomy_version = Some(analysis.taxonomy.version.clone());
        visit.taxonomy_reason = Some(analysis.taxonomy.reasons.join("; "));
        visit.registrable_domain = analysis
            .normalized_url
            .as_ref()
            .map(|value| value.registrable_domain.clone())
            .unwrap_or_else(|| url_domain(&visit.url));
        visit.source_role = legacy_source_role(visit).to_string();
        visit.page_type = legacy_page_type(visit).to_string();
        visit.keywords = extract_keywords(visit);
        visit.entities = extract_entities(visit);
        visit.vector = embeddings.get(&visit.history_id).cloned();
    }
}

fn compute_feature_scores(visits: &mut [VisitRecord]) {
    let mut prior_tokens = Vec::<HashSet<String>>::new();
    let mut revisit_counts = HashMap::<String, usize>::new();
    for visit in visits.iter_mut() {
        let tokens = visit.keywords.iter().cloned().collect::<HashSet<_>>();
        let max_similarity = prior_tokens
            .iter()
            .map(|prior| token_similarity(&tokens, prior))
            .fold(0.0f32, f32::max);
        let revisit_key = canonical_visit_key(visit);
        let revisit_count = revisit_counts.entry(revisit_key).or_insert(0);
        *revisit_count += 1;
        let evidence_bonus = match visit.evidence_tier {
            EvidenceTier::TierA => 0.6,
            EvidenceTier::TierB => 0.3,
            EvidenceTier::TierC => 0.0,
        };
        let domain_bonus = match visit.domain_category {
            DomainCategory::Docs | DomainCategory::Developer => 0.35,
            DomainCategory::Community | DomainCategory::Shopping | DomainCategory::Work => 0.2,
            DomainCategory::Search | DomainCategory::Ai => 0.15,
            _ => 0.0,
        };
        let interaction_bonus = match visit.interaction_kind {
            InteractionKind::Resolve | InteractionKind::Compare => 0.3,
            InteractionKind::Learn => 0.2,
            InteractionKind::Manage => 0.15,
            InteractionKind::Discuss | InteractionKind::Discover | InteractionKind::Watch => 0.1,
            InteractionKind::Transact | InteractionKind::Unknown => 0.0,
        };
        visit.novelty_score = (1.0 - max_similarity).clamp(0.0, 1.0);
        visit.importance_score = ((*revisit_count as f32 - 1.0) * 0.35
            + evidence_bonus
            + domain_bonus
            + interaction_bonus)
            .clamp(0.0, 4.0);
        visit.explore_score = ((visit.novelty_score * 0.7)
            + if *revisit_count <= 1 { 0.2 } else { 0.0 }
            + if visit.interaction_kind == InteractionKind::Discover { 0.1 } else { 0.0 })
        .clamp(0.0, 1.0);
        prior_tokens.push(tokens);
    }
}

fn persist_features(connection: &Connection, visits: &[VisitRecord]) -> Result<()> {
    for visit in visits {
        connection.execute(
            "INSERT OR REPLACE INTO visit_insight_features
             (history_id, profile_id, burst_id, query_group_id, topic_id, thread_id, page_type, source_role,
              domain_category, page_category, interaction_kind, evidence_tier,
              taxonomy_source, taxonomy_pack, taxonomy_version, taxonomy_reason,
              query_term, query_stage, novelty_score, importance_score, explore_score,
              keywords_json, entities_json, updated_at, pipeline_version)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17,
                     ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25)",
            params![
                visit.history_id,
                visit.profile_id,
                visit.burst_id,
                visit.query_group_id,
                visit.topic_id,
                visit.thread_id,
                visit.page_type,
                visit.source_role,
                visit.domain_category.as_str(),
                visit.page_category_v2.as_str(),
                visit.interaction_kind.as_str(),
                visit.evidence_tier.as_str(),
                visit.taxonomy_source.as_str(),
                visit.taxonomy_pack.as_deref(),
                visit.taxonomy_version.as_deref(),
                visit.taxonomy_reason.as_deref(),
                visit.query_term,
                classify_query_stage(visit.query_term.as_deref(), None),
                visit.novelty_score,
                visit.importance_score,
                visit.explore_score,
                serde_json::to_string(&visit.keywords)?,
                serde_json::to_string(&visit.entities)?,
                now_rfc3339(),
                INSIGHT_PIPELINE_VERSION,
            ],
        )?;
    }
    Ok(())
}

fn persist_cards(
    connection: &Connection,
    cards: &[InsightCard],
    profile_id: Option<&str>,
    window_days: u32,
) -> Result<()> {
    let profile_scope = profile_id.unwrap_or("all");
    connection.execute(
        "DELETE FROM insight_cards WHERE profile_scope = ?1 AND window_days = ?2",
        params![profile_scope, window_days as i64],
    )?;
    for card in cards {
        connection.execute(
            "INSERT INTO insight_cards
             (card_id, profile_scope, window_days, kind, title, summary, score, chromium_enhanced,
              evidence_json, generated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                card.card_id,
                profile_scope,
                window_days as i64,
                card.kind,
                card.title,
                card.summary,
                card.score,
                card.chromium_enhanced as i64,
                serde_json::to_string(&card.evidence)?,
                now_rfc3339(),
            ],
        )?;
    }
    Ok(())
}

fn load_cards(
    connection: &Connection,
    profile_scope: &str,
    window_days: u32,
) -> Result<Vec<InsightCard>> {
    let mut statement = connection.prepare(
        "SELECT card_id, kind, title, summary, window_days, score, chromium_enhanced, evidence_json
         FROM insight_cards
         WHERE profile_scope = ?1 AND window_days = ?2
         ORDER BY score DESC, generated_at DESC",
    )?;
    let rows =
        statement.query_map(params![profile_scope, window_days as i64], |row: &Row<'_>| {
            Ok(InsightCard {
                card_id: row.get(0)?,
                kind: row.get(1)?,
                title: row.get(2)?,
                summary: row.get(3)?,
                window_days: row.get::<_, i64>(4)?.max(0) as u32,
                profile_id: None,
                score: row.get(5)?,
                chromium_enhanced: row.get::<_, i64>(6)? != 0,
                evidence: serde_json::from_str::<Vec<InsightEvidenceItem>>(
                    &row.get::<_, String>(7)?,
                )
                .unwrap_or_default(),
            })
        })?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

fn load_topics(
    connection: &Connection,
    profile_scope: &str,
    window_days: u32,
) -> Result<Vec<InsightTopicSummary>> {
    let mut statement = connection.prepare(
        "SELECT topic_id, label, profile_scope, window_days, first_seen_at, last_seen_at, visit_count,
                revisit_count, trend_slope, burst_score, evidence_json
         FROM insight_topics
         WHERE profile_scope = ?1 AND window_days = ?2
         ORDER BY trend_slope DESC, visit_count DESC",
    )?;
    let rows =
        statement.query_map(params![profile_scope, window_days as i64], |row: &Row<'_>| {
            Ok(InsightTopicSummary {
                topic_id: row.get(0)?,
                label: row.get(1)?,
                profile_scope: row.get(2)?,
                window_days: row.get::<_, i64>(3)?.max(0) as u32,
                first_seen_at: row.get(4)?,
                last_seen_at: row.get(5)?,
                visit_count: row.get::<_, i64>(6)?.max(0) as usize,
                revisit_count: row.get::<_, i64>(7)?.max(0) as usize,
                trend_slope: row.get(8)?,
                burst_score: row.get(9)?,
                evidence: serde_json::from_str::<Vec<InsightEvidenceItem>>(
                    &row.get::<_, String>(10)?,
                )
                .unwrap_or_default(),
            })
        })?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

fn load_threads(
    connection: &Connection,
    profile_id: Option<&str>,
    window_days: u32,
) -> Result<Vec<InsightThreadSummary>> {
    let start = (Utc::now() - Duration::days(window_days as i64)).to_rfc3339();
    let sql = if profile_id.is_some() {
        "SELECT thread_id, profile_id, title, status, first_seen_at, last_seen_at, visit_count,
                query_group_count, reopen_count, open_loop_score, confidence, evidence_tier,
                dominant_topic_id, chromium_enhanced, evidence_json
         FROM insight_threads
         WHERE profile_id = ?1 AND last_seen_at >= ?2
         ORDER BY last_seen_at DESC"
    } else {
        "SELECT thread_id, profile_id, title, status, first_seen_at, last_seen_at, visit_count,
                query_group_count, reopen_count, open_loop_score, confidence, evidence_tier,
                dominant_topic_id, chromium_enhanced, evidence_json
         FROM insight_threads
         WHERE last_seen_at >= ?1
         ORDER BY last_seen_at DESC"
    };
    let mut statement = connection.prepare(sql)?;
    let rows = if let Some(profile_id) = profile_id {
        statement.query_map(params![profile_id, start], thread_summary_from_row)?
    } else {
        statement.query_map(params![start], thread_summary_from_row)?
    };
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

fn thread_summary_from_row(row: &Row<'_>) -> rusqlite::Result<InsightThreadSummary> {
    Ok(InsightThreadSummary {
        thread_id: row.get(0)?,
        profile_id: row.get(1)?,
        title: row.get(2)?,
        status: row.get(3)?,
        first_seen_at: row.get(4)?,
        last_seen_at: row.get(5)?,
        visit_count: row.get::<_, i64>(6)?.max(0) as usize,
        query_group_count: row.get::<_, i64>(7)?.max(0) as usize,
        reopen_count: row.get::<_, i64>(8)?.max(0) as usize,
        open_loop_score: row.get(9)?,
        confidence: row.get(10)?,
        evidence_tier: row.get(11)?,
        dominant_topic_id: row.get(12)?,
        chromium_enhanced: row.get::<_, i64>(13)? != 0,
        evidence: serde_json::from_str::<Vec<InsightEvidenceItem>>(&row.get::<_, String>(14)?)
            .unwrap_or_default(),
    })
}

fn load_feature_rows(
    connection: &Connection,
    profile_id: Option<&str>,
) -> Result<HashMap<i64, (String, Option<String>, String)>> {
    let sql = if profile_id.is_some() {
        "SELECT history_id, source_role, query_term, query_stage
         FROM visit_insight_features
         WHERE profile_id = ?1"
    } else {
        "SELECT history_id, source_role, query_term, query_stage FROM visit_insight_features"
    };
    let mut statement = connection.prepare(sql)?;
    let mut map = HashMap::new();
    if let Some(profile_id) = profile_id {
        let rows = statement.query_map([profile_id], |row: &Row<'_>| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, Option<String>>(2)?,
                row.get::<_, String>(3)?,
            ))
        })?;
        for row in rows {
            let (history_id, source_role, query_term, query_stage) = row?;
            map.insert(history_id, (source_role, query_term, query_stage));
        }
    } else {
        let rows = statement.query_map([], |row: &Row<'_>| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, Option<String>>(2)?,
                row.get::<_, String>(3)?,
            ))
        })?;
        for row in rows {
            let (history_id, source_role, query_term, query_stage) = row?;
            map.insert(history_id, (source_role, query_term, query_stage));
        }
    }
    Ok(map)
}

fn build_query_ladders(
    visits: &[VisitRecord],
    features: &HashMap<i64, (String, Option<String>, String)>,
) -> Vec<InsightQueryLadder> {
    let mut ladders = Vec::<InsightQueryLadder>::new();
    let mut current: Option<InsightQueryLadder> = None;
    let mut previous_query: Option<(String, i64)> = None;
    for visit in visits {
        let Some((_, query_term, _)) = features.get(&visit.history_id) else {
            continue;
        };
        let Some(query_term) = query_term.clone() else {
            continue;
        };
        let continues = previous_query.as_ref().is_some_and(|(previous, previous_time)| {
            visit.profile_id
                == current.as_ref().map(|value| value.profile_id.as_str()).unwrap_or("")
                && chrome_gap_hours(*previous_time, visit.visit_time) <= 2
                && token_similarity(
                    &tokenize_text(previous).into_iter().collect(),
                    &tokenize_text(&query_term).into_iter().collect(),
                ) >= 0.2
        });
        if !continues {
            if let Some(current) = current.take()
                && current.steps.len() > 1
            {
                ladders.push(current);
            }
            current = Some(InsightQueryLadder {
                query_group_id: None,
                root_term: query_term.clone(),
                profile_id: visit.profile_id.clone(),
                steps: vec![query_term.clone()],
                stages: vec![classify_query_stage(Some(&query_term), None)],
                count: 1,
                confidence: 0.45,
                evidence_tier: "tier-c".to_string(),
                chromium_only: true,
            });
        } else if let Some(current) = &mut current {
            let previous = current.steps.last().cloned();
            if current.steps.last() != Some(&query_term) {
                current.steps.push(query_term.clone());
                current.stages.push(classify_query_stage(Some(&query_term), previous.as_deref()));
            }
            current.count += 1;
        }
        previous_query = Some((query_term, visit.visit_time));
    }
    if let Some(current) = current
        && current.steps.len() > 1
    {
        ladders.push(current);
    }
    ladders.sort_by(|left, right| {
        right.steps.len().cmp(&left.steps.len()).then(right.count.cmp(&left.count))
    });
    ladders.truncate(6);
    ladders
}

fn build_workflow_map(
    visits: &[VisitRecord],
    features: &HashMap<i64, (String, Option<String>, String)>,
    profile_id: Option<&str>,
) -> InsightWorkflowMap {
    let mut role_counts = HashMap::<String, usize>::new();
    let mut edge_counts = HashMap::<(String, String), usize>::new();
    let mut previous: Option<(&VisitRecord, String)> = None;
    for visit in visits {
        let role = features
            .get(&visit.history_id)
            .map(|value| value.0.clone())
            .unwrap_or_else(|| visit.source_role.clone());
        *role_counts.entry(role.clone()).or_insert(0) += 1;
        if let Some((previous_visit, previous_role)) = previous.as_ref() {
            if previous_visit.profile_id == visit.profile_id
                && chrome_gap_minutes(previous_visit.visit_time, visit.visit_time)
                    <= SESSION_GAP_MINUTES
            {
                *edge_counts.entry((previous_role.clone(), role.clone())).or_insert(0) += 1;
            }
        }
        previous = Some((visit, role));
    }
    let mut roles = role_counts
        .into_iter()
        .map(|(role, count)| InsightWorkflowRole { role, count })
        .collect::<Vec<_>>();
    roles.sort_by(|left, right| right.count.cmp(&left.count));
    let mut edges = edge_counts
        .into_iter()
        .map(|((from_role, to_role), count)| InsightWorkflowEdge { from_role, to_role, count })
        .collect::<Vec<_>>();
    edges.sort_by(|left, right| right.count.cmp(&left.count));
    edges.truncate(10);
    InsightWorkflowMap {
        profile_id: profile_id.map(ToString::to_string),
        roles,
        edges,
        chromium_enhanced: visits.iter().any(|visit| is_chromium_profile(&visit.profile_id)),
    }
}

fn build_profile_facets(
    visits: &[VisitRecord],
    topics: &[InsightTopicSummary],
    threads: &[InsightThreadSummary],
) -> Vec<InsightProfileFacet> {
    let mut facets = Vec::new();
    let mean_explore = if visits.is_empty() {
        0.0
    } else {
        visits.iter().map(|visit| visit.explore_score).sum::<f32>() / visits.len() as f32
    };
    facets.push(InsightProfileFacet {
        key: "explore-exploit".to_string(),
        label: "Explore vs exploit".to_string(),
        value: if mean_explore >= 0.55 {
            "Explore-heavy".to_string()
        } else {
            "Exploit-heavy".to_string()
        },
        confidence: (0.4 + (mean_explore - 0.5).abs()).clamp(0.0, 1.0),
        evidence: visits
            .iter()
            .rev()
            .take(3)
            .map(|visit| {
                evidence_from_visit(
                    visit,
                    Some(format!("Explore score {:.2}", visit.explore_score)),
                )
            })
            .collect(),
    });

    let docs_count = visits.iter().filter(|visit| visit.source_role == "docs").count();
    let forum_count = visits.iter().filter(|visit| visit.source_role == "forum").count();
    facets.push(InsightProfileFacet {
        key: "source-preference".to_string(),
        label: "Source preference".to_string(),
        value: if docs_count >= forum_count {
            "Docs-first".to_string()
        } else {
            "Forum-first".to_string()
        },
        confidence: if visits.is_empty() {
            0.0
        } else {
            ((docs_count.max(forum_count) as f32) / visits.len() as f32).clamp(0.0, 1.0)
        },
        evidence: visits
            .iter()
            .filter(|visit| visit.source_role == "docs" || visit.source_role == "forum")
            .take(3)
            .map(|visit| evidence_from_visit(visit, Some(visit.source_role.clone())))
            .collect(),
    });

    let rising_topics = topics.iter().filter(|topic| topic.trend_slope > 0.12).count();
    facets.push(InsightProfileFacet {
        key: "interest-shape".to_string(),
        label: "Interest shape".to_string(),
        value: if rising_topics >= 2 { "Emerging".to_string() } else { "Stable".to_string() },
        confidence: (0.45 + rising_topics as f32 * 0.1).clamp(0.0, 1.0),
        evidence: topics
            .iter()
            .take(3)
            .flat_map(|topic| topic.evidence.iter().take(1).cloned())
            .collect(),
    });

    let reopen_count = threads.iter().map(|thread| thread.reopen_count).sum::<usize>();
    facets.push(InsightProfileFacet {
        key: "work-rhythm".to_string(),
        label: "Work rhythm".to_string(),
        value: if reopen_count >= 2 {
            "Interrupted and resumptive".to_string()
        } else {
            "Steadier sessions".to_string()
        },
        confidence: (0.4 + reopen_count as f32 * 0.08).clamp(0.0, 1.0),
        evidence: threads
            .iter()
            .take(3)
            .flat_map(|thread| thread.evidence.iter().take(1).cloned())
            .collect(),
    });

    facets
}

fn evidence_from_visit(visit: &VisitRecord, note: Option<String>) -> InsightEvidenceItem {
    InsightEvidenceItem {
        history_id: visit.history_id,
        profile_id: visit.profile_id.clone(),
        url: visit.url.clone(),
        title: visit.readable_title.clone().or_else(|| visit.title.clone()),
        visited_at: visit.visited_at.clone(),
        note,
    }
}

fn build_run_notes(
    embeddings_available: bool,
    enrichment_enabled: bool,
    enriched_visits: usize,
    failed_enrichments: usize,
    queued_network_jobs: usize,
    recovery: &InterruptedInsightRecovery,
) -> Vec<String> {
    let mut notes = Vec::new();
    if recovery.recovered_runs > 0 {
        notes.push(format!(
            "Recovered {} interrupted insight run(s) and re-queued {} stuck enrichment job(s).",
            recovery.recovered_runs, recovery.requeued_enrichment_jobs
        ));
    }
    notes.push(if embeddings_available {
        "Insight run used semantic vectors when available.".to_string()
    } else {
        "Insight run fell back to lexical and structural signals because no embedding provider was ready.".to_string()
    });
    notes.push(if enrichment_enabled {
        format!("Enrichment runtime completed {enriched_visits} successful plugin jobs.")
    } else {
        "Enrichment runtime is disabled, so this run used canonical archive signals only."
            .to_string()
    });
    if failed_enrichments > 0 {
        notes.push(format!(
            "{failed_enrichments} enrichment jobs failed or returned unsupported content."
        ));
    }
    if queued_network_jobs > 0 {
        notes.push(format!(
            "Deferred {queued_network_jobs} network enrichment job(s) to later refreshes so this run stays responsive."
        ));
    }
    notes
}

fn build_taxonomy_review_notes(visits: &[VisitRecord]) -> Vec<String> {
    let mut notes = Vec::new();
    let taxonomy_version = visits
        .iter()
        .find_map(|visit| visit.taxonomy_version.clone())
        .unwrap_or_else(|| "m5-taxonomy-v1".to_string());
    notes.push(format!(
        "Deterministic taxonomy {taxonomy_version} used script-aware tokens plus checked-in regional rule packs; external tokenizer and registrable-domain wheels stay gated behind PG-RD-AI-010."
    ));

    let unknown_visits = visits
        .iter()
        .filter(|visit| visit.domain_category == DomainCategory::Unknown)
        .collect::<Vec<_>>();
    if unknown_visits.is_empty() {
        notes.push("Deterministic taxonomy classified the analyzed window without falling back to unknown.".to_string());
        return notes;
    }

    let mut counts = HashMap::<String, usize>::new();
    for visit in unknown_visits {
        let key = if visit.registrable_domain.is_empty() {
            url_domain(&visit.url)
        } else {
            visit.registrable_domain.clone()
        };
        *counts.entry(key).or_insert(0) += 1;
    }
    let mut ranked = counts.into_iter().collect::<Vec<_>>();
    ranked.sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(&right.0)));
    let preview = ranked
        .into_iter()
        .take(5)
        .map(|(domain, count)| format!("{domain} ({count})"))
        .collect::<Vec<_>>()
        .join(", ");
    notes.push(format!(
        "Deterministic taxonomy fell back to unknown for {} visit(s); top unmatched domains: {}.",
        visits.iter().filter(|visit| visit.domain_category == DomainCategory::Unknown).count(),
        preview
    ));
    notes
}

fn query_term_from_url(url: &str) -> Option<String> {
    extract_search_query_from_url(url)
}

fn percent_decode(input: &str) -> String {
    let mut output = String::new();
    let bytes = input.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'+' => output.push(' '),
            b'%' if index + 2 < bytes.len() => {
                let hi = (bytes[index + 1] as char).to_digit(16);
                let lo = (bytes[index + 2] as char).to_digit(16);
                if let (Some(hi), Some(lo)) = (hi, lo) {
                    output.push(char::from_u32(hi * 16 + lo).unwrap_or('%'));
                    index += 2;
                } else {
                    output.push('%');
                }
            }
            value => output.push(value as char),
        }
        index += 1;
    }
    output
}

fn normalize_whitespace(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    let mut last_was_space = false;
    for ch in input.chars() {
        if ch.is_whitespace() {
            if !last_was_space {
                output.push(' ');
                last_was_space = true;
            }
        } else {
            output.push(ch);
            last_was_space = false;
        }
    }
    output.trim().to_string()
}

fn truncate_text(input: &str, limit: usize) -> String {
    if input.chars().count() <= limit {
        return input.to_string();
    }
    input.chars().take(limit).collect::<String>()
}

fn build_embedding_content_from_parts(
    profile_id: &str,
    url: &str,
    title: Option<&str>,
    visited_at: &str,
    readable_title: Option<&str>,
    readable_text: Option<&str>,
) -> String {
    let title = title.unwrap_or("(untitled)");
    let mut content = format!(
        "Profile: {profile_id}\nVisited at: {visited_at}\nURL: {url}\nDomain: {}\nTitle: {title}",
        url_domain(url)
    );
    if let Some(readable_title) = readable_title.filter(|value| !value.trim().is_empty()) {
        content.push_str(&format!("\nReadable title: {}", readable_title.trim()));
    }
    if let Some(readable_text) = readable_text.filter(|value| !value.trim().is_empty()) {
        content.push_str("\nReadable text:\n");
        content.push_str(readable_text.trim());
    }
    content
}

fn legacy_source_role(visit: &VisitRecord) -> &'static str {
    match visit.page_category_v2 {
        PageCategory::SearchResults => "search",
        PageCategory::Repo | PageCategory::Issue | PageCategory::PullRequest => "repo",
        PageCategory::DocsPage => "docs",
        PageCategory::ForumThread => "forum",
        PageCategory::VideoPage => "video",
        PageCategory::ProductPage | PageCategory::CategoryPage => "shopping",
        PageCategory::Dashboard if visit.domain_category == DomainCategory::Work => "notes",
        PageCategory::ArticlePage if visit.domain_category == DomainCategory::News => "news",
        PageCategory::Profile if visit.domain_category == DomainCategory::Social => "social",
        _ => match visit.domain_category {
            DomainCategory::Docs => "docs",
            DomainCategory::Developer => "repo",
            DomainCategory::Community => "forum",
            DomainCategory::Video => "video",
            DomainCategory::Shopping => "shopping",
            DomainCategory::Work => "notes",
            DomainCategory::Social => "social",
            DomainCategory::News => "news",
            DomainCategory::Search => "search",
            _ => "general",
        },
    }
}

fn legacy_page_type(visit: &VisitRecord) -> &'static str {
    match visit.page_category_v2 {
        PageCategory::Issue => "issue",
        PageCategory::PullRequest => "pull-request",
        PageCategory::DocsPage => "documentation",
        PageCategory::VideoPage => "video",
        PageCategory::SearchResults => "search-results",
        PageCategory::ForumThread => "discussion",
        PageCategory::ProductPage | PageCategory::CategoryPage => "comparison",
        PageCategory::ArticlePage => "article",
        _ => "page",
    }
}

fn extract_keywords(visit: &VisitRecord) -> Vec<String> {
    let mut counts = HashMap::<String, usize>::new();
    let mut push_tokens = |value: &str| {
        for token in tokenize_text(value) {
            *counts.entry(token).or_insert(0) += 1;
        }
    };
    if let Some(title) = &visit.title {
        push_tokens(title);
    }
    if let Some(readable_title) = &visit.readable_title {
        push_tokens(readable_title);
    }
    if let Some(query_term) = &visit.query_term {
        push_tokens(query_term);
    }
    if let Some(readable_text) = &visit.readable_text {
        push_tokens(&truncate_text(readable_text, 600));
    } else {
        push_tokens(&visit.url);
    }
    let mut keywords = counts.into_iter().collect::<Vec<_>>();
    keywords.sort_by(|left, right| right.1.cmp(&left.1).then(left.0.cmp(&right.0)));
    keywords.into_iter().take(8).map(|(token, _)| token).collect()
}

fn extract_entities(visit: &VisitRecord) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut entities = Vec::new();
    for source in
        [visit.title.as_deref(), visit.readable_title.as_deref(), visit.query_term.as_deref()]
            .into_iter()
            .flatten()
    {
        for token in source.split_whitespace() {
            let token = token.trim_matches(|ch: char| !ch.is_alphanumeric());
            if token.len() >= 3
                && token.chars().any(|ch| ch.is_uppercase())
                && seen.insert(token.to_string())
            {
                entities.push(token.to_string());
            }
        }
    }
    entities.truncate(6);
    entities
}

fn token_similarity(left: &HashSet<String>, right: &HashSet<String>) -> f32 {
    if left.is_empty() || right.is_empty() {
        return 0.0;
    }
    let intersection = left.intersection(right).count() as f32;
    let union = left.union(right).count() as f32;
    if union == 0.0 { 0.0 } else { intersection / union }
}

fn canonical_visit_key(visit: &VisitRecord) -> String {
    format!("{}::{}", visit.profile_id, visit.url)
}

fn classify_query_stage(query_term: Option<&str>, previous: Option<&str>) -> String {
    let Some(query_term) = query_term.map(|value| value.trim().to_ascii_lowercase()) else {
        return "none".to_string();
    };
    if query_term.contains("site:") {
        "site-restrict".to_string()
    } else if query_term.contains(" vs ") || query_term.contains("compare") {
        "compare".to_string()
    } else if ["error", "exception", "failed", "bug", "panic"]
        .iter()
        .any(|token| query_term.contains(token))
    {
        "error-driven".to_string()
    } else if let Some(previous) = previous {
        if query_term.contains(previous)
            || tokenize_text(&query_term).len() > tokenize_text(previous).len()
        {
            "narrowing".to_string()
        } else {
            "broadening".to_string()
        }
    } else if tokenize_text(&query_term).len() >= 5 {
        "narrowing".to_string()
    } else {
        "broad".to_string()
    }
}

fn chrome_gap_minutes(left: i64, right: i64) -> i64 {
    (right.saturating_sub(left) / 60_000_000).max(0)
}

fn chrome_gap_hours(left: i64, right: i64) -> i64 {
    (right.saturating_sub(left) / 3_600_000_000).max(0)
}

fn is_chromium_profile(profile_id: &str) -> bool {
    matches!(
        profile_id.split(':').next().unwrap_or_default(),
        "chrome"
            | "chromium"
            | "edge"
            | "edge-dev"
            | "brave"
            | "vivaldi"
            | "arc"
            | "opera"
            | "opera-gx"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        archive::{ensure_archive_initialized, open_archive_connection},
        config::project_paths_with_root,
        models::{AiProviderConfig, AiProviderPurpose, AiRequestFormat, AiSettings, ArchiveMode},
        utils::iso_to_chrome_time_micros,
    };
    use tempfile::tempdir;

    fn test_paths() -> ProjectPaths {
        let dir = tempdir().expect("tempdir");
        project_paths_with_root(dir.path())
    }

    fn test_config() -> AppConfig {
        let mut config = AppConfig {
            initialized: true,
            archive_mode: ArchiveMode::Plaintext,
            ai: AiSettings::default(),
            ..AppConfig::default()
        };
        config.ai.embedding_provider_id = Some("embed".to_string());
        config.ai.embedding_providers = vec![AiProviderConfig {
            id: "embed".to_string(),
            name: "Embed".to_string(),
            purpose: AiProviderPurpose::Embedding,
            request_format: AiRequestFormat::OpenAi,
            enabled: true,
            default_model: "text-embedding-3-large".to_string(),
            dimensions: Some(8),
            ..AiProviderConfig::default()
        }];
        config
    }

    fn seed_visits(connection: &Connection) {
        let visit_one = (Utc::now() - Duration::days(10)).to_rfc3339();
        let visit_two = (Utc::now() - Duration::days(10) + Duration::minutes(12)).to_rfc3339();
        let visit_three = (Utc::now() - Duration::days(8)).to_rfc3339();
        let visit_four = (Utc::now() - Duration::days(8) + Duration::minutes(28)).to_rfc3339();
        let visit_five = (Utc::now() - Duration::days(2)).to_rfc3339();
        let visit_six = (Utc::now() - Duration::days(365)).to_rfc3339();
        connection
            .execute(
                "INSERT INTO visit_events
                 (id, profile_id, source_visit_id, source_url_id, url, title, visit_time, from_visit, transition, visit_duration, is_known_to_sync, visited_link_id, external_referrer_url, app_id, event_fingerprint, payload_hash, recorded_at)
                 VALUES
                 (1, 'chrome:Default', 1, 1, 'https://example.com/docs/archive', 'Archive docs', ?1, NULL, 805306368, 24000, 1, NULL, 'https://google.com', NULL, 'a', 'a', ?6),
                 (2, 'chrome:Default', 2, 2, 'https://github.com/example/repo/issues/1', 'Issue one', ?2, 1, 805306368, 12000, 1, NULL, NULL, NULL, 'b', 'b', ?7),
                 (3, 'chrome:Default', 3, 3, 'https://www.google.com/search?q=archive+tool+compare', 'Google Search', ?3, NULL, 805306368, 6000, 1, NULL, NULL, NULL, 'c', 'c', ?8),
                 (4, 'chrome:Default', 4, 4, 'https://www.google.com/search?q=archive+tool+compare+github', 'Google Search Refined', ?4, NULL, 805306368, 8000, 1, NULL, NULL, NULL, 'd', 'd', ?9),
                 (5, 'chrome:Default', 5, 5, 'https://example.com/pricing', 'Pricing', ?5, NULL, 805306368, 5000, 1, NULL, NULL, NULL, 'e', 'e', ?10),
                 (6, 'chrome:Default', 6, 6, 'https://example.com/on-this-day', 'On this day', ?11, NULL, 805306368, 5000, 1, NULL, NULL, NULL, 'f', 'f', ?12)",
                params![
                    iso_to_chrome_time_micros(&visit_one).expect("visit one chrome time"),
                    iso_to_chrome_time_micros(&visit_two).expect("visit two chrome time"),
                    iso_to_chrome_time_micros(&visit_three).expect("visit three chrome time"),
                    iso_to_chrome_time_micros(&visit_four).expect("visit four chrome time"),
                    iso_to_chrome_time_micros(&visit_five).expect("visit five chrome time"),
                    visit_one,
                    visit_two,
                    visit_three,
                    visit_four,
                    visit_five,
                    iso_to_chrome_time_micros(&visit_six).expect("visit six chrome time"),
                    visit_six,
                ],
            )
            .expect("insert visits");
        connection
            .execute(
                "INSERT INTO search_terms (
                   url_id,
                   term,
                   normalized_term,
                   source_profile_id,
                   created_by_run_id,
                   profile_id,
                   keyword_id,
                   recorded_at
                 )
                 VALUES
                 (
                   3,
                   'archive tool compare',
                   'archive tool compare',
                   (SELECT id FROM source_profiles WHERE profile_key = 'chrome:Default'),
                   0,
                   'chrome:Default',
                   1,
                   ?1
                 ),
                 (
                   4,
                   'archive tool compare github',
                   'archive tool compare github',
                   (SELECT id FROM source_profiles WHERE profile_key = 'chrome:Default'),
                   0,
                   'chrome:Default',
                   2,
                   ?2
                 )",
                params![visit_three, visit_four],
            )
            .expect("insert search term");
    }

    #[test]
    fn insight_schema_and_snapshot_roundtrip() {
        let paths = test_paths();
        let config = test_config();
        ensure_archive_initialized(&paths, &config, None).expect("init archive");
        let connection = open_archive_connection(&paths, &config, None).expect("open archive");
        create_schema(&connection).expect("schema");
        ensure_insight_schema(&connection).expect("insight schema");
        seed_visits(&connection);

        let report = run_insights(
            &paths,
            &config,
            None,
            None,
            &RunInsightsRequest {
                profile_id: Some("chrome:Default".to_string()),
                window_days: Some(30),
                full_rebuild: false,
                limit: Some(20),
            },
        )
        .expect("run insights");
        assert!(report.processed_visits >= 4);

        let snapshot = load_insights(
            &paths,
            &config,
            None,
            &RunInsightsRequest {
                profile_id: Some("chrome:Default".to_string()),
                window_days: Some(30),
                full_rebuild: false,
                limit: None,
            },
        )
        .expect("load insights");
        assert!(!snapshot.cards.is_empty());
        assert!(!snapshot.threads.is_empty());
        assert!(!snapshot.canonical.on_this_day.is_empty());
        assert!(!snapshot.canonical.top_domains.is_empty());
        assert!(!snapshot.query_ladders.is_empty());
        assert!(snapshot.query_ladders[0].steps.len() > 1);
        assert!(snapshot.workflow_map.chromium_enhanced);
    }

    #[test]
    fn readable_content_plugin_can_be_disabled_and_cleared() {
        let paths = test_paths();
        let mut config = test_config();
        if let Some(plugin) = config
            .enrichment
            .plugins
            .iter_mut()
            .find(|plugin| plugin.id == READABLE_CONTENT_PLUGIN_ID)
        {
            plugin.enabled = false;
        }
        ensure_archive_initialized(&paths, &config, None).expect("init archive");
        let connection = open_archive_connection(&paths, &config, None).expect("open archive");
        create_schema(&connection).expect("schema");
        ensure_insight_schema(&connection).expect("insight schema");
        seed_visits(&connection);

        let report = run_insights(
            &paths,
            &config,
            None,
            None,
            &RunInsightsRequest {
                profile_id: Some("chrome:Default".to_string()),
                window_days: Some(30),
                full_rebuild: false,
                limit: Some(20),
            },
        )
        .expect("run insights with plugin disabled");
        assert!(report.enriched_visits > 0);
        let readable_content_rows = connection
            .query_row(
                "SELECT COUNT(*) FROM visit_content_enrichments WHERE content_source = ?1",
                [READABLE_CONTENT_PLUGIN_ID],
                |row: &Row<'_>| row.get::<_, i64>(0),
            )
            .expect("readable content row count");
        assert_eq!(readable_content_rows, 0);
        assert!(report.notes.iter().any(|note| note.contains("successful plugin jobs")));

        let cleared =
            clear_derived_intelligence_state(&paths, &config, None).expect("clear derived state");
        assert!(cleared.cleared_card_rows > 0);
        let snapshot = load_insights(
            &paths,
            &config,
            None,
            &RunInsightsRequest {
                profile_id: Some("chrome:Default".to_string()),
                window_days: Some(30),
                full_rebuild: false,
                limit: None,
            },
        )
        .expect("load cleared snapshot");
        assert!(snapshot.cards.is_empty());
        assert!(snapshot.threads.is_empty());
        assert!(!snapshot.canonical.on_this_day.is_empty());
    }

    #[test]
    fn preferred_embedding_content_prefers_enriched_text() {
        let connection = Connection::open_in_memory().expect("db");
        ensure_insight_schema(&connection).expect("schema");
        store_enrichment(
            &connection,
            7,
            "refetch",
            &EnrichmentResult {
                status: "success".to_string(),
                final_url: Some("https://example.com/final".to_string()),
                language: Some("en".to_string()),
                readable_title: Some("Readable".to_string()),
                readable_text: Some("Readable text body".to_string()),
                snippets: vec!["Readable text body".to_string()],
                extraction: json!({}),
            },
        )
        .expect("store enrichment");
        let content = preferred_embedding_content(
            &connection,
            7,
            "chrome:Default",
            "https://example.com",
            Some("Original"),
            "2026-04-03T00:00:00Z",
        )
        .expect("embedding content");
        assert!(content.contains("Readable text body"));
        assert!(content.contains("Readable title"));
    }

    #[test]
    fn run_insights_marks_runs_failed_when_job_payload_is_invalid() {
        let paths = test_paths();
        let mut config = test_config();
        if let Some(plugin) = config
            .enrichment
            .plugins
            .iter_mut()
            .find(|plugin| plugin.id == READABLE_CONTENT_PLUGIN_ID)
        {
            plugin.enabled = false;
        }
        ensure_archive_initialized(&paths, &config, None).expect("init archive");
        let connection = open_archive_connection(&paths, &config, None).expect("open archive");
        create_schema(&connection).expect("schema");
        ensure_insight_schema(&connection).expect("insight schema");
        ensure_intelligence_runtime_schema(&connection).expect("runtime schema");
        seed_visits(&connection);

        let now = now_rfc3339();
        connection
            .execute(
                "INSERT INTO intelligence_jobs
                 (job_type, plugin_id, run_id, state, priority, attempt, dedupe_key, payload_json,
                  artifact_json, created_at, scheduled_at, updated_at)
                 VALUES (?1, ?2, NULL, 'queued', 0, 0, 'broken-job', '{', '{}', ?3, ?3, ?3)",
                params![ENRICHMENT_JOB_TYPE, TITLE_NORMALIZATION_PLUGIN_ID, now],
            )
            .expect("insert malformed job");

        let error = run_insights(&paths, &config, None, None, &RunInsightsRequest::default())
            .expect_err("invalid payload should fail the run");
        assert!(error.to_string().contains("parsing enrichment payload"));

        let (status, warning) = connection
            .query_row(
                "SELECT status, warning
                 FROM insight_runs
                 ORDER BY id DESC
                 LIMIT 1",
                [],
                |row: &Row<'_>| Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?)),
            )
            .expect("load failed run");
        assert_eq!(status, "failed");
        assert!(warning.expect("warning").contains("Insight refresh stopped before completion"));
    }

    #[test]
    fn run_insights_recovers_interrupted_runs_and_requeues_stuck_jobs() {
        let paths = test_paths();
        let mut config = test_config();
        if let Some(plugin) = config
            .enrichment
            .plugins
            .iter_mut()
            .find(|plugin| plugin.id == READABLE_CONTENT_PLUGIN_ID)
        {
            plugin.enabled = false;
        }
        ensure_archive_initialized(&paths, &config, None).expect("init archive");
        let connection = open_archive_connection(&paths, &config, None).expect("open archive");
        create_schema(&connection).expect("schema");
        ensure_insight_schema(&connection).expect("insight schema");
        ensure_intelligence_runtime_schema(&connection).expect("runtime schema");
        seed_visits(&connection);

        let now = now_rfc3339();
        connection
            .execute(
                "INSERT INTO insight_runs (id, started_at, status, mode, profile_scope, window_days, notes_json)
                 VALUES (41, ?1, 'running', 'manual', 'all', 30, '[]')",
                [now.clone()],
            )
            .expect("insert interrupted run");
        connection
            .execute(
                "INSERT INTO intelligence_jobs
                 (job_type, plugin_id, run_id, state, priority, attempt, dedupe_key, payload_json,
                  artifact_json, created_at, scheduled_at, started_at, updated_at)
                 VALUES (?1, ?2, 41, 'running', 10, 1, 'title-normalization:1', ?3, '{}', ?4, ?4, ?4, ?4)",
                params![
                    ENRICHMENT_JOB_TYPE,
                    TITLE_NORMALIZATION_PLUGIN_ID,
                    serde_json::to_string(&EnrichmentJobPayload {
                        history_id: 1,
                        profile_id: "chrome:Default".to_string(),
                        url: "https://example.com/docs/archive".to_string(),
                        title: Some("Archive docs".to_string()),
                    })
                    .expect("payload"),
                    now,
                ],
            )
            .expect("insert interrupted job");

        let report = run_insights(&paths, &config, None, None, &RunInsightsRequest::default())
            .expect("run insights after recovery");
        assert!(
            report.notes.iter().any(|note| note.contains("Recovered 1 interrupted insight run"))
        );

        let previous_status = connection
            .query_row("SELECT status FROM insight_runs WHERE id = 41", [], |row: &Row<'_>| {
                row.get::<_, String>(0)
            })
            .expect("previous run status");
        assert_eq!(previous_status, "failed");

        let job_state = connection
            .query_row(
                "SELECT state FROM intelligence_jobs WHERE dedupe_key = 'title-normalization:1'",
                [],
                |row: &Row<'_>| row.get::<_, String>(0),
            )
            .expect("job state");
        assert_eq!(job_state, "succeeded");
    }

    #[test]
    fn query_stage_heuristics_cover_compare_and_site_restrict() {
        assert_eq!(classify_query_stage(Some("best archive tool vs obsidian"), None), "compare");
        assert_eq!(
            classify_query_stage(Some("site:github.com archive tool"), None),
            "site-restrict"
        );
    }
}
