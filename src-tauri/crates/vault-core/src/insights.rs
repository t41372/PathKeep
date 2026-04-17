//! Deterministic intelligence and enrichment pipeline.
//!
//! This module transforms canonical visits plus optional enrichment into
//! deterministic insights such as grouped queries, threads, reference pages,
//! source effectiveness, and summaries. It keeps the derived-state story
//! explicit: everything here can be rebuilt from canonical facts.
//!
//! Legacy note: this module is now crate-internal. The accepted deterministic
//! product contract has moved to `intelligence/`; `insights` remains only so
//! optional enrichment/readable-content evidence can keep using the older
//! storage helpers while that code is retired in place.

#![allow(dead_code, unused_imports)]

mod grouping;
mod runtime;
mod shared;
mod site_adapters;
mod storage;
mod surfaces;
mod topics;

use crate::{
    ai::{AiProviderRuntime, ensure_ai_schema},
    archive::open_intelligence_connection,
    config::ProjectPaths,
    deterministic::{
        DomainCategory, EvidenceTier, InteractionKind, PageCategory, VisitAnalysisInput,
        analyze_visit, extract_search_query_from_url, tokenize_text,
    },
    enrichment::{
        StoredEnrichment, build_embedding_content_from_parts,
        ensure_visit_content_enrichment_schema, load_best_enrichment_map_by_history_ids,
    },
    intelligence_blobs::{load_readable_text_blob, store_readable_text_blob},
    intelligence_runtime::{
        DeterministicModuleRuntimeUpdate, ENRICHMENT_JOB_TYPE, EnrichmentJobPayload,
        LOCAL_PLUGIN_SOURCE_KIND, NETWORK_PLUGIN_SOURCE_KIND, built_in_deterministic_modules,
        built_in_enrichment_plugin, built_in_enrichment_plugins, claim_enrichment_job_by_id,
        claim_enrichment_jobs, enqueue_enrichment_job, enrichment_plugin_enabled,
        ensure_intelligence_runtime_schema, intelligence_job_stop_requested,
        mark_intelligence_job_failed, mark_intelligence_job_succeeded,
        mark_running_intelligence_job_cancelled, persist_deterministic_module_runtime_updates,
        requeue_running_enrichment_jobs, requeue_running_enrichment_jobs_for_run,
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
    utils::{chrome_time_to_rfc3339, now_rfc3339, url_domain},
};
use anyhow::{Context, Result};
use chrono::{DateTime, Duration, Local, Utc};
use reqwest::blocking::Client;
use rusqlite::{Connection, OptionalExtension, Row, params};
use scraper::{Html, Selector};
use serde_json::{Value, json};
use site_adapters::adapt_site_content;
use std::collections::{HashMap, HashSet};

#[cfg_attr(not(test), allow(unused_imports))]
pub(crate) use self::runtime::{
    InsightsRunCancelled, clear_derived_intelligence_state, explain_insight, insight_status,
    load_insight_thread_detail, load_insights, run_insights, run_insights_with_progress,
};
use self::{
    grouping::{
        build_bursts, build_query_groups, build_threads as build_thread_records,
        query_group_summaries_from_records, thread_summaries_from_records,
    },
    storage::{
        load_query_groups, load_reference_pages, load_snapshot_payloads, load_source_effectiveness,
        load_thread_query_groups, persist_bursts, persist_query_groups, persist_reference_pages,
        persist_snapshot_payloads, persist_source_effectiveness,
        persist_threads as persist_thread_records, persist_topic_summaries,
    },
    surfaces::{
        build_cards as build_insight_cards, build_reference_pages, build_source_effectiveness,
        build_template_summaries,
    },
    topics::build_topics as build_topic_summaries,
};

const INSIGHT_PIPELINE_VERSION: &str = "insights-v2";
const ENRICH_TEXT_LIMIT: usize = 12_000;
const SNIPPET_LIMIT: usize = 3;
const DEFAULT_WINDOW_DAYS: u32 = 30;
const SESSION_GAP_MINUTES: i64 = 30;

const INSIGHT_SCHEMA_SQL: &str = r#"
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
  profile_scope TEXT NOT NULL,
  window_days INTEGER NOT NULL,
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
  profile_scope TEXT NOT NULL,
  window_days INTEGER NOT NULL,
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
  window_days INTEGER NOT NULL,
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
  window_days INTEGER NOT NULL,
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
CREATE TABLE IF NOT EXISTS insight_snapshot_payloads (
  profile_scope TEXT NOT NULL,
  window_days INTEGER NOT NULL,
  generated_at TEXT NOT NULL,
  query_ladders_json TEXT NOT NULL,
  template_summaries_json TEXT NOT NULL,
  workflow_map_json TEXT NOT NULL,
  profile_facets_json TEXT NOT NULL,
  canonical_json TEXT NOT NULL,
  PRIMARY KEY(profile_scope, window_days)
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
CREATE INDEX IF NOT EXISTS idx_visit_insight_features_topic_id
  ON visit_insight_features(topic_id);
CREATE INDEX IF NOT EXISTS idx_visit_insight_features_thread_id
  ON visit_insight_features(thread_id);
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
CREATE INDEX IF NOT EXISTS idx_insight_snapshot_payloads_scope_window
  ON insight_snapshot_payloads(profile_scope, window_days, generated_at DESC);
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

const SQLITE_BATCH_SIZE: usize = 400;

/// Ensures all deterministic-insight schema tables and indexes exist.
pub(crate) fn ensure_insight_schema(connection: &Connection) -> Result<()> {
    ensure_visit_content_enrichment_schema(connection)?;
    connection.execute_batch(INSIGHT_SCHEMA_SQL)?;
    connection.execute_batch(
        r#"
CREATE INDEX IF NOT EXISTS idx_insight_query_groups_scope_window_last_seen
  ON insight_query_groups(profile_scope, window_days, last_seen_at DESC);
CREATE INDEX IF NOT EXISTS idx_insight_threads_scope_window_last_seen
  ON insight_threads(profile_scope, window_days, last_seen_at DESC);
CREATE INDEX IF NOT EXISTS idx_insight_reference_pages_scope_window_score
  ON insight_reference_pages(profile_scope, window_days, score DESC, last_seen_at DESC);
CREATE INDEX IF NOT EXISTS idx_insight_source_effectiveness_scope_window_score
  ON insight_source_effectiveness(profile_scope, window_days, effectiveness_score DESC);
"#,
    )?;
    ensure_visit_insight_feature_indexes(connection)?;
    Ok(())
}

fn ensure_visit_insight_feature_indexes(connection: &Connection) -> Result<()> {
    connection.execute_batch(
        r#"
CREATE INDEX IF NOT EXISTS idx_visit_insight_features_burst_id
  ON visit_insight_features(burst_id);
CREATE INDEX IF NOT EXISTS idx_visit_insight_features_query_group_id
  ON visit_insight_features(query_group_id);
"#,
    )?;
    Ok(())
}

#[allow(dead_code)]
/// Chooses the strongest available text payload for semantic indexing or assistant context.
pub(crate) fn preferred_embedding_content(
    paths: &ProjectPaths,
    connection: &Connection,
    history_id: i64,
    profile_id: &str,
    url: &str,
    title: Option<&str>,
    visited_at: &str,
) -> Result<String> {
    let mut enrichments =
        load_best_enrichment_map_by_history_ids(paths, connection, &[history_id])?;
    let enrichment = enrichments.remove(&history_id);
    Ok(build_embedding_content_from_parts(
        profile_id,
        url,
        title,
        visited_at,
        enrichment.as_ref().and_then(|value| value.readable_title.as_deref()),
        enrichment.as_ref().and_then(|value| value.readable_text.as_deref()),
    ))
}

fn collect_history_ids(visits: &[VisitRecord]) -> Vec<i64> {
    let mut seen = HashSet::new();
    let mut ids = Vec::with_capacity(visits.len());
    for visit in visits {
        if seen.insert(visit.history_id) {
            ids.push(visit.history_id);
        }
    }
    ids
}

fn collect_profile_url_ids(visits: &[VisitRecord]) -> HashMap<String, Vec<i64>> {
    let mut grouped = HashMap::<String, HashSet<i64>>::new();
    for visit in visits {
        grouped.entry(visit.profile_id.clone()).or_default().insert(visit.source_url_id);
    }
    grouped
        .into_iter()
        .map(|(profile_id, url_ids)| {
            let mut url_ids = url_ids.into_iter().collect::<Vec<_>>();
            url_ids.sort_unstable();
            (profile_id, url_ids)
        })
        .collect()
}

fn load_visits(
    connection: &Connection,
    profile_id: Option<&str>,
    window_days: u32,
    limit: Option<usize>,
) -> Result<Vec<VisitRecord>> {
    let start = (Utc::now() - Duration::days(window_days as i64)).to_rfc3339();
    let start_chrome = crate::utils::iso_to_chrome_time_micros(&start).unwrap_or(0);
    load_visits_in_range(connection, profile_id, start_chrome, None, limit)
}

fn load_visits_in_range(
    connection: &Connection,
    profile_id: Option<&str>,
    start_chrome: i64,
    end_chrome: Option<i64>,
    limit: Option<usize>,
) -> Result<Vec<VisitRecord>> {
    let mut sql = String::from(
        "SELECT visits.id,
                source_profiles.profile_key AS profile_id,
                CAST(visits.source_visit_id AS INTEGER) AS source_visit_id,
                urls.id AS source_url_id,
                urls.url,
                urls.title,
                (visits.visit_time_ms * 1000 + 11644473600000000) AS visit_time,
                visits.from_visit,
                visits.transition_type AS transition,
                visits.visit_duration_ms AS visit_duration,
                visits.external_referrer_url,
                visits.app_id
         FROM archive.visits AS visits
         JOIN archive.urls AS urls ON urls.id = visits.url_id
         JOIN archive.source_profiles AS source_profiles ON source_profiles.id = visits.source_profile_id
         WHERE visits.reverted_at IS NULL
           AND (visits.visit_time_ms * 1000 + 11644473600000000) >= ?1",
    );
    let mut params: Vec<&dyn rusqlite::ToSql> = vec![&start_chrome];
    let mut next_index = 2;
    let end_value = end_chrome;
    let profile_value = profile_id.map(ToString::to_string);
    if let Some(end_chrome) = end_value.as_ref() {
        sql.push_str(&format!(" AND visit_time < ?{next_index}"));
        params.push(end_chrome);
        next_index += 1;
    }
    if let Some(profile_id) = profile_value.as_ref() {
        sql.push_str(&format!(" AND profile_id = ?{next_index}"));
        params.push(profile_id);
        next_index += 1;
    }
    sql.push_str(" ORDER BY visits.visit_time_ms ASC");
    let limit_value = limit.map(|limit| limit.max(1) as i64);
    if let Some(limit_value) = limit_value.as_ref() {
        sql.push_str(&format!(" LIMIT ?{next_index}"));
        params.push(limit_value);
    }
    let mut statement = connection.prepare(&sql)?;
    let rows = statement.query_map(rusqlite::params_from_iter(params), visit_record_from_row)?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

fn load_on_this_day(
    connection: &Connection,
    profile_id: Option<&str>,
    limit: usize,
) -> Result<Vec<InsightEvidenceItem>> {
    let today_key = Local::now().format("%m-%d").to_string();
    let current_year = Local::now().format("%Y").to_string();
    let sql = if profile_id.is_some() {
        "SELECT visits.id,
                source_profiles.profile_key AS profile_id,
                urls.url,
                urls.title,
                (visits.visit_time_ms * 1000 + 11644473600000000) AS visit_time
         FROM archive.visits AS visits
         JOIN archive.urls AS urls ON urls.id = visits.url_id
         JOIN archive.source_profiles AS source_profiles ON source_profiles.id = visits.source_profile_id
         WHERE visits.reverted_at IS NULL
           AND source_profiles.profile_key = ?1
           AND strftime('%m-%d', datetime(visit_time / 1000000 - 11644473600, 'unixepoch', 'localtime')) = ?2
           AND strftime('%Y', datetime(visit_time / 1000000 - 11644473600, 'unixepoch', 'localtime')) != ?3
         ORDER BY visit_time DESC
         LIMIT ?4"
    } else {
        "SELECT visits.id,
                source_profiles.profile_key AS profile_id,
                urls.url,
                urls.title,
                (visits.visit_time_ms * 1000 + 11644473600000000) AS visit_time
         FROM archive.visits AS visits
         JOIN archive.urls AS urls ON urls.id = visits.url_id
         JOIN archive.source_profiles AS source_profiles ON source_profiles.id = visits.source_profile_id
         WHERE visits.reverted_at IS NULL
           AND strftime('%m-%d', datetime(visit_time / 1000000 - 11644473600, 'unixepoch', 'localtime')) = ?1
           AND strftime('%Y', datetime(visit_time / 1000000 - 11644473600, 'unixepoch', 'localtime')) != ?2
         ORDER BY visit_time DESC
         LIMIT ?3"
    };
    let mut statement = connection.prepare(sql)?;
    let rows = if let Some(profile_id) = profile_id {
        statement.query_map(
            params![profile_id, today_key, current_year, limit as i64],
            insight_evidence_from_row,
        )?
    } else {
        statement
            .query_map(params![today_key, current_year, limit as i64], insight_evidence_from_row)?
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
    visits: &[VisitRecord],
) -> Result<HashMap<(String, i64), String>> {
    let mut map = HashMap::new();
    for (profile_id, url_ids) in collect_profile_url_ids(visits) {
        for chunk in url_ids.chunks(SQLITE_BATCH_SIZE) {
            let placeholders = vec!["?"; chunk.len()].join(", ");
            let sql = format!(
                "SELECT source_profiles.profile_key, search_terms.url_id, search_terms.normalized_term
                 FROM archive.search_terms AS search_terms
                 JOIN archive.source_profiles AS source_profiles
                   ON source_profiles.id = search_terms.source_profile_id
                 WHERE source_profiles.profile_key = ?1
                   AND search_terms.reverted_at IS NULL
                   AND search_terms.url_id IN ({placeholders})"
            );
            let mut statement = connection.prepare(&sql)?;
            let params = std::iter::once(&profile_id as &dyn rusqlite::ToSql)
                .chain(chunk.iter().map(|url_id| url_id as &dyn rusqlite::ToSql));
            let rows =
                statement.query_map(rusqlite::params_from_iter(params), |row: &Row<'_>| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?, row.get::<_, String>(2)?))
                })?;
            for row in rows {
                let (profile_id, url_id, term) = row?;
                map.entry((profile_id, url_id)).or_insert(term);
            }
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
    paths: &ProjectPaths,
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
            if plugin_enrichment_is_fresh(paths, connection, visit.history_id, plugin.id)? {
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
    paths: &ProjectPaths,
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
    let mut report = EnrichmentProcessingReport::default();
    if !local_plugin_ids.is_empty() {
        claimed.extend(claim_enrichment_jobs(
            connection,
            &local_plugin_ids,
            &allowed_history_ids,
            visits.len().max(1) * local_plugin_ids.len(),
        )?);
    }
    if !network_plugin_ids.is_empty() {
        report.queued_network_jobs = queued_enrichment_jobs(connection, &network_plugin_ids)?;
    }
    if claimed.is_empty() {
        return Ok(report);
    }

    let visit_map = visits.iter().map(|visit| (visit.history_id, visit)).collect::<HashMap<_, _>>();

    for job in claimed {
        let Some(visit) = visit_map.get(&job.payload.history_id) else {
            if !mark_intelligence_job_failed(
                connection,
                job.id,
                "The queued visit no longer exists in the current insight window.",
            )? {
                let _ = mark_running_intelligence_job_cancelled(
                    connection,
                    job.id,
                    "cancelled from UI",
                );
            }
            report.failed_enrichments += 1;
            continue;
        };

        let enrichment = match job.plugin_id.as_str() {
            TITLE_NORMALIZATION_PLUGIN_ID => {
                title_normalization_enrichment(&visit.url, visit.title.as_deref())
            }
            _ => {
                if !mark_intelligence_job_failed(
                    connection,
                    job.id,
                    &format!("Unknown enrichment plugin {}", job.plugin_id),
                )? {
                    let _ = mark_running_intelligence_job_cancelled(
                        connection,
                        job.id,
                        "cancelled from UI",
                    );
                }
                report.failed_enrichments += 1;
                continue;
            }
        };

        store_enrichment(paths, connection, visit.history_id, &job.plugin_id, &enrichment)?;
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
            if !mark_intelligence_job_failed(
                connection,
                job.id,
                &enrichment_failure_message(&enrichment),
            )? {
                let _ = mark_running_intelligence_job_cancelled(
                    connection,
                    job.id,
                    "cancelled from UI",
                );
            }
        } else {
            if enrichment.status == "success" {
                report.enriched_visits += 1;
            }
            if !mark_intelligence_job_succeeded(connection, job.id, &artifact)? {
                let _ = mark_running_intelligence_job_cancelled(
                    connection,
                    job.id,
                    "cancelled from UI",
                );
            }
        }
    }

    Ok(report)
}

/// Claims and executes one persisted enrichment job from the intelligence queue.
pub fn execute_enrichment_job_by_id(
    paths: &ProjectPaths,
    connection: &Connection,
    job_id: i64,
) -> Result<bool> {
    let Some(job) = claim_enrichment_job_by_id(connection, job_id)? else {
        return Ok(false);
    };
    if intelligence_job_stop_requested(connection, job.id)? {
        let _ = mark_running_intelligence_job_cancelled(connection, job.id, "cancelled from UI");
        return Ok(true);
    }

    let enrichment = match job.plugin_id.as_str() {
        TITLE_NORMALIZATION_PLUGIN_ID => {
            title_normalization_enrichment(&job.payload.url, job.payload.title.as_deref())
        }
        READABLE_CONTENT_PLUGIN_ID => {
            let client = build_refetch_client()?;
            refetch_visit_content(&client, &job.payload.url)
        }
        _ => {
            if !mark_intelligence_job_failed(
                connection,
                job.id,
                &format!("Unknown enrichment plugin {}", job.plugin_id),
            )? {
                let _ = mark_running_intelligence_job_cancelled(
                    connection,
                    job.id,
                    "cancelled from UI",
                );
            }
            return Ok(true);
        }
    };
    if intelligence_job_stop_requested(connection, job.id)? {
        let _ = mark_running_intelligence_job_cancelled(connection, job.id, "cancelled from UI");
        return Ok(true);
    }

    store_enrichment(paths, connection, job.payload.history_id, &job.plugin_id, &enrichment)?;
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
        if !mark_intelligence_job_failed(
            connection,
            job.id,
            &enrichment_failure_message(&enrichment),
        )? {
            let _ =
                mark_running_intelligence_job_cancelled(connection, job.id, "cancelled from UI");
        }
    } else {
        if !mark_intelligence_job_succeeded(connection, job.id, &artifact)? {
            let _ =
                mark_running_intelligence_job_cancelled(connection, job.id, "cancelled from UI");
        }
    }

    Ok(true)
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
    paths: &ProjectPaths,
    connection: &Connection,
    history_id: i64,
    plugin_id: &str,
) -> Result<bool> {
    let record = enrichment_for_history_and_source(paths, connection, history_id, plugin_id)?;
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

fn title_normalization_enrichment(url: &str, title: Option<&str>) -> EnrichmentResult {
    let readable_title = title
        .map(normalize_whitespace)
        .filter(|value| !value.is_empty())
        .or_else(|| normalized_title_from_url(url));
    let status = if readable_title.is_some() { "success" } else { "empty" };
    let snippets = readable_title.clone().into_iter().collect::<Vec<_>>();
    EnrichmentResult {
        status: status.to_string(),
        final_url: Some(url.to_string()),
        language: None,
        readable_title,
        readable_text: None,
        snippets,
        extraction: json!({
            "strategy": if title.is_some() { "browser-title" } else { "url-fallback" },
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
    match enrichment.status.as_str() {
        "unsupported-content" => {
            let content_type = enrichment
                .extraction
                .get("contentType")
                .and_then(Value::as_str)
                .filter(|value| !value.trim().is_empty())
                .unwrap_or("non-HTML response");
            format!("Skipped non-readable content ({content_type}).")
        }
        "fetch-error" => enrichment
            .extraction
            .get("error")
            .and_then(Value::as_str)
            .map(|error| format!("Could not fetch the page again. {error}"))
            .unwrap_or_else(|| "Could not fetch the page again.".to_string()),
        "decode-error" => {
            let content_type = enrichment
                .extraction
                .get("contentType")
                .and_then(Value::as_str)
                .filter(|value| !value.trim().is_empty());
            let error = enrichment
                .extraction
                .get("error")
                .and_then(Value::as_str)
                .filter(|value| !value.trim().is_empty());
            match (content_type, error) {
                (Some(content_type), Some(error)) => {
                    format!("Could not decode the response body ({content_type}). {error}")
                }
                (Some(content_type), None) => {
                    format!("Could not decode the response body ({content_type}).")
                }
                (None, Some(error)) => format!("Could not decode the response body. {error}"),
                (None, None) => "Could not decode the response body.".to_string(),
            }
        }
        _ => enrichment
            .extraction
            .get("error")
            .and_then(Value::as_str)
            .map(ToString::to_string)
            .unwrap_or_else(|| format!("Enrichment failed with status {}", enrichment.status)),
    }
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
    paths: &ProjectPaths,
    connection: &Connection,
    history_id: i64,
    content_source: &str,
    enrichment: &EnrichmentResult,
) -> Result<()> {
    let stored_blob = store_readable_text_blob(paths, enrichment.readable_text.as_deref())?;
    connection.execute(
        "INSERT OR REPLACE INTO visit_content_enrichments
         (history_id, content_source, fetch_status, fetched_at, final_url, language, readable_title,
          readable_text_blob_path, readable_text_bytes, text_hash, snippet_json, extraction_json, pipeline_version)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
        params![
            history_id,
            content_source,
            enrichment.status,
            now_rfc3339(),
            enrichment.final_url,
            enrichment.language,
            enrichment.readable_title,
            stored_blob.as_ref().map(|blob| blob.relative_path.as_str()),
            stored_blob.as_ref().map(|blob| blob.byte_len as i64).unwrap_or(0),
            stored_blob.as_ref().map(|blob| blob.content_hash.as_str()),
            serde_json::to_string(&enrichment.snippets)?,
            serde_json::to_string(&enrichment.extraction)?,
            INSIGHT_PIPELINE_VERSION,
        ],
    )?;
    Ok(())
}

fn enrichment_for_history_and_source(
    paths: &ProjectPaths,
    connection: &Connection,
    history_id: i64,
    content_source: &str,
) -> Result<Option<StoredEnrichment>> {
    let mut statement = connection.prepare(
        "SELECT content_source, fetch_status, fetched_at, final_url, language, readable_title,
                readable_text_blob_path, snippet_json
         FROM visit_content_enrichments
         WHERE history_id = ?1 AND content_source = ?2
         ORDER BY fetched_at DESC
         LIMIT 1",
    )?;
    let row = statement
        .query_row(params![history_id, content_source], |row| {
            let _: Option<String> = row.get(3)?;
            let _: Option<String> = row.get(4)?;
            Ok((
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, Option<String>>(5)?,
                row.get::<_, Option<String>>(6)?,
                row.get::<_, String>(7)?,
            ))
        })
        .optional()?;
    row.map_or(Ok(None), |(fetch_status, fetched_at, readable_title, blob_path, snippet_json)| {
        Ok(Some(StoredEnrichment {
            fetch_status,
            fetched_at,
            readable_title,
            readable_text: load_readable_text_blob(paths, blob_path.as_deref())?,
            snippet_json,
        }))
    })
}

fn load_best_enrichment_map(
    paths: &ProjectPaths,
    connection: &Connection,
    visits: &[VisitRecord],
) -> Result<HashMap<i64, StoredEnrichment>> {
    load_best_enrichment_map_by_history_ids(paths, connection, &collect_history_ids(visits))
}

fn hydrate_enrichments(visits: &mut [VisitRecord], enrichments: &HashMap<i64, StoredEnrichment>) {
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
    }
}

#[cfg(test)]
#[allow(dead_code)]
fn compute_feature_scores(visits: &mut [VisitRecord]) {
    compute_feature_scores_with_progress(visits, |_processed, _total| Ok(()))
        .expect("score visits");
}

fn compute_feature_scores_with_progress<F>(
    visits: &mut [VisitRecord],
    mut on_progress: F,
) -> Result<()>
where
    F: FnMut(usize, usize) -> Result<()>,
{
    let mut seen_token_counts = HashMap::<String, usize>::new();
    let mut revisit_counts = HashMap::<String, usize>::new();
    let total = visits.len();
    if total == 0 {
        on_progress(0, 0)?;
        return Ok(());
    }
    let progress_interval = (total / 24).max(2_048);
    on_progress(0, total)?;
    for (index, visit) in visits.iter_mut().enumerate() {
        let tokens = visit.keywords.iter().cloned().collect::<HashSet<_>>();
        let repeated_token_ratio = if tokens.is_empty() {
            0.0
        } else {
            tokens.iter().filter(|token| seen_token_counts.contains_key(*token)).count() as f32
                / tokens.len() as f32
        };
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
        visit.novelty_score = (1.0 - repeated_token_ratio).clamp(0.0, 1.0);
        visit.importance_score = ((*revisit_count as f32 - 1.0) * 0.35
            + evidence_bonus
            + domain_bonus
            + interaction_bonus)
            .clamp(0.0, 4.0);
        visit.explore_score = ((visit.novelty_score * 0.7)
            + if *revisit_count <= 1 { 0.2 } else { 0.0 }
            + if visit.interaction_kind == InteractionKind::Discover { 0.1 } else { 0.0 })
        .clamp(0.0, 1.0);
        for token in tokens {
            *seen_token_counts.entry(token).or_insert(0) += 1;
        }
        let processed = index + 1;
        if processed == total || processed % progress_interval == 0 {
            on_progress(processed, total)?;
        }
    }
    Ok(())
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
        let card_id = format!("{profile_scope}:{window_days}:{}", card.card_id);
        connection.execute(
            "INSERT INTO insight_cards
             (card_id, profile_scope, window_days, kind, title, summary, score, chromium_enhanced,
              evidence_json, generated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                card_id,
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
    profile_scope: &str,
    window_days: u32,
) -> Result<Vec<InsightThreadSummary>> {
    let mut statement = connection.prepare(
        "SELECT thread_id, profile_id, title, status, first_seen_at, last_seen_at, visit_count,
                query_group_count, reopen_count, open_loop_score, confidence, evidence_tier,
                dominant_topic_id, chromium_enhanced, evidence_json
         FROM insight_threads
         WHERE profile_scope = ?1 AND window_days = ?2
         ORDER BY last_seen_at DESC",
    )?;
    let rows =
        statement.query_map(params![profile_scope, window_days as i64], thread_summary_from_row)?;
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
    visits: &[VisitRecord],
) -> Result<HashMap<i64, (String, Option<String>, String)>> {
    let history_ids = collect_history_ids(visits);
    let mut map = HashMap::new();
    for chunk in history_ids.chunks(SQLITE_BATCH_SIZE) {
        let placeholders = vec!["?"; chunk.len()].join(", ");
        let sql = format!(
            "SELECT history_id, source_role, query_term, query_stage
             FROM visit_insight_features
             WHERE history_id IN ({placeholders})"
        );
        let mut statement = connection.prepare(&sql)?;
        let params = chunk.iter().map(|history_id| history_id as &dyn rusqlite::ToSql);
        let rows = statement.query_map(rusqlite::params_from_iter(params), |row: &Row<'_>| {
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
