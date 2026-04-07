use crate::{
    ai::{AiProviderRuntime, ensure_ai_schema},
    archive::{create_schema, open_archive_connection},
    config::ProjectPaths,
    models::{
        AppConfig, ExplainInsightRequest, InsightCard, InsightEvidenceItem, InsightExplanation,
        InsightProfileFacet, InsightQueryLadder, InsightSnapshot, InsightStatus,
        InsightThreadDetail, InsightThreadSummary, InsightTopicSummary, InsightWorkflowEdge,
        InsightWorkflowMap, InsightWorkflowRole, RunInsightsReport, RunInsightsRequest,
    },
    utils::{chrome_time_to_rfc3339, now_rfc3339, sha256_hex, url_domain},
};
use anyhow::{Context, Result};
use chrono::{DateTime, Duration, Utc};
use reqwest::blocking::Client;
use rusqlite::{Connection, OptionalExtension, Row, params};
use scraper::{Html, Selector};
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet};

const INSIGHT_PIPELINE_VERSION: &str = "insights-v1";
const ENRICH_TEXT_LIMIT: usize = 12_000;
const SNIPPET_LIMIT: usize = 3;
const DEFAULT_WINDOW_DAYS: u32 = 30;
const DEFAULT_ANALYSIS_LIMIT: usize = 600;
const SESSION_GAP_MINUTES: i64 = 30;
const THREAD_GAP_DAYS: i64 = 14;
const REOPEN_GAP_HOURS: i64 = 24;

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
  topic_id TEXT,
  thread_id TEXT,
  page_type TEXT NOT NULL,
  source_role TEXT NOT NULL,
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
  reopen_count INTEGER NOT NULL,
  open_loop_score REAL NOT NULL,
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
  topic_count INTEGER NOT NULL DEFAULT 0,
  thread_count INTEGER NOT NULL DEFAULT 0,
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
CREATE INDEX IF NOT EXISTS idx_insight_threads_profile_last_seen
  ON insight_threads(profile_id, last_seen_at DESC);
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
    duration_ms: Option<i64>,
    external_referrer_url: Option<String>,
    #[allow(dead_code)]
    app_id: Option<String>,
    query_term: Option<String>,
    readable_title: Option<String>,
    readable_text: Option<String>,
    snippets: Vec<String>,
    source_role: String,
    page_type: String,
    keywords: Vec<String>,
    entities: Vec<String>,
    novelty_score: f32,
    importance_score: f32,
    explore_score: f32,
    topic_id: Option<String>,
    thread_id: Option<String>,
    vector: Option<Vec<f32>>,
}

#[derive(Debug, Clone, Default)]
struct SessionRecord {
    profile_id: String,
    visit_indexes: Vec<usize>,
}

#[derive(Debug, Clone, Default)]
struct TopicAccumulator {
    topic_id: String,
    label: String,
    first_seen_at: String,
    last_seen_at: String,
    visit_indexes: Vec<usize>,
    revisit_count: usize,
    trend_slope: f32,
    burst_score: f32,
    centroid: Option<Vec<f32>>,
    keyword_counts: HashMap<String, usize>,
}

#[derive(Debug, Clone, Default)]
struct ThreadAccumulator {
    thread_id: String,
    profile_id: String,
    visit_indexes: Vec<usize>,
    first_seen_at: String,
    last_seen_at: String,
    reopen_count: usize,
    open_loop_score: f32,
    dominant_topic_id: Option<String>,
    status: String,
    chromium_enhanced: bool,
    title: String,
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
    content_source: String,
    fetch_status: String,
    fetched_at: String,
    readable_title: Option<String>,
    readable_text: Option<String>,
    snippet_json: String,
}

pub(crate) fn ensure_insight_schema(connection: &Connection) -> Result<()> {
    connection.execute_batch(INSIGHT_SCHEMA_SQL)?;
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

    let analysis_limit = request.limit.unwrap_or(DEFAULT_ANALYSIS_LIMIT as u32) as usize;
    let mut visits =
        load_visits(&connection, request.profile_id.as_deref(), window_days, analysis_limit)?;
    let query_terms = load_search_term_map(&connection, request.profile_id.as_deref())?;
    hydrate_query_terms(&mut visits, &query_terms);

    let eligible_ids = visits
        .iter()
        .filter(|visit| is_refetch_eligible(visit))
        .map(|visit| visit.history_id)
        .collect::<HashSet<_>>();
    let mut enriched_visits = 0usize;
    let mut failed_enrichments = 0usize;
    let client = build_refetch_client()?;
    for visit in &visits {
        if !eligible_ids.contains(&visit.history_id) {
            continue;
        }
        if enrichment_is_fresh(&connection, visit.history_id)? {
            continue;
        }
        let enrichment = refetch_visit_content(&client, &visit.url);
        store_enrichment(&connection, visit.history_id, "refetch", &enrichment)?;
        if enrichment.status == "success" {
            enriched_visits += 1;
        } else {
            failed_enrichments += 1;
        }
    }

    if embedding_provider.is_some() {
        // Embeddings are refreshed by the worker before analytics. Here we only consume them.
    }

    let enrichments = load_best_enrichment_map(&connection, &visits)?;
    let embeddings = load_embedding_map(&connection, embedding_provider, &visits)?;
    hydrate_enrichment_and_embeddings(&mut visits, &enrichments, &embeddings);
    compute_feature_scores(&mut visits);

    let sessions = build_sessions(&visits);
    let topics = assign_topics(&mut visits, window_days);
    let threads = assign_threads(&mut visits, &sessions);
    persist_features(&connection, &visits)?;
    persist_topics(&connection, &topics, &visits, request.profile_id.as_deref(), window_days)?;
    persist_threads(&connection, &threads, &visits)?;

    let cards = build_cards(&visits, &topics, &threads, window_days, request.profile_id.as_deref());
    persist_cards(&connection, &cards, request.profile_id.as_deref(), window_days)?;

    let content_coverage = if visits.is_empty() {
        0.0
    } else {
        visits
            .iter()
            .filter(|visit| visit.readable_text.as_deref().is_some_and(|value| !value.is_empty()))
            .count() as f32
            / visits.len() as f32
    };
    let notes = build_run_notes(embedding_provider.is_some(), enriched_visits, failed_enrichments);
    connection.execute(
        "UPDATE insight_runs
         SET finished_at = ?1, status = 'success', processed_visits = ?2, enriched_visits = ?3,
             failed_enrichments = ?4, topic_count = ?5, thread_count = ?6, card_count = ?7,
             content_coverage = ?8, notes_json = ?9
         WHERE id = ?10",
        params![
            now_rfc3339(),
            visits.len() as i64,
            enriched_visits as i64,
            failed_enrichments as i64,
            topics.len() as i64,
            threads.len() as i64,
            cards.len() as i64,
            content_coverage,
            serde_json::to_string(&notes)?,
            run_id,
        ],
    )?;

    Ok(RunInsightsReport {
        run_id,
        processed_visits: visits.len(),
        enriched_visits,
        failed_enrichments,
        topic_count: topics.len(),
        thread_count: threads.len(),
        card_count: cards.len(),
        content_coverage,
        last_run_at: started_at,
        notes,
    })
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

    let cards = load_cards(&connection, &profile_scope, window_days)?;
    let topics = load_topics(&connection, &profile_scope, window_days)?;
    let threads = load_threads(&connection, request.profile_id.as_deref(), window_days)?;
    let visits = load_visits(
        &connection,
        request.profile_id.as_deref(),
        window_days,
        DEFAULT_ANALYSIS_LIMIT,
    )?;
    let features = load_feature_rows(&connection, request.profile_id.as_deref())?;
    let query_ladders = build_query_ladders(&visits, &features);
    let workflow_map = build_workflow_map(&visits, &features, request.profile_id.as_deref());
    let profile_facets = build_profile_facets(&visits, &topics, &threads);
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
        topics,
        threads,
        query_ladders,
        workflow_map,
        profile_facets,
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
                reopen_count, open_loop_score, dominant_topic_id, chromium_enhanced, evidence_json
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
    Ok(InsightThreadDetail { summary, visits })
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
                "{} spans {} visits between {} and {}. It reopened {} times and has an open-loop score of {:.2}, which suggests the task was revisited rather than completed in a single pass.",
                detail.summary.title,
                detail.summary.visit_count,
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
    connection.execute("DELETE FROM visit_insight_features", [])?;
    connection.execute("DELETE FROM insight_topics", [])?;
    connection.execute("DELETE FROM insight_threads", [])?;
    connection.execute("DELETE FROM insight_thread_members", [])?;
    connection.execute("DELETE FROM insight_cards", [])?;
    Ok(())
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
        readable_title: None,
        readable_text: None,
        snippets: Vec::new(),
        source_role: "general".to_string(),
        page_type: "page".to_string(),
        keywords: Vec::new(),
        entities: Vec::new(),
        novelty_score: 0.0,
        importance_score: 0.0,
        explore_score: 0.0,
        topic_id: None,
        thread_id: None,
        vector: None,
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
        visit.query_term = query_terms
            .get(&(visit.profile_id.clone(), visit.source_url_id))
            .cloned()
            .or_else(|| query_term_from_url(&visit.url));
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

fn enrichment_is_fresh(connection: &Connection, history_id: i64) -> Result<bool> {
    let record = best_enrichment_for_history(connection, history_id)?;
    let Some(record) = record else {
        return Ok(false);
    };
    if record.content_source == "capture" {
        return Ok(record.fetch_status == "success"
            && record.readable_text.as_deref().is_some_and(|value| !value.trim().is_empty()));
    }
    let fetched_at = DateTime::parse_from_rfc3339(&record.fetched_at)
        .ok()
        .map(|value| value.with_timezone(&Utc));
    let still_fresh =
        fetched_at.map(|value| Utc::now() - value <= Duration::days(7)).unwrap_or(false);
    Ok(record.fetch_status == "success"
        && still_fresh
        && record.readable_text.as_deref().is_some_and(|value| !value.trim().is_empty()))
}

fn build_refetch_client() -> Result<Client> {
    Client::builder()
        .redirect(reqwest::redirect::Policy::limited(5))
        .timeout(std::time::Duration::from_secs(10))
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
    EnrichmentResult {
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
    }
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

fn best_enrichment_for_history(
    connection: &Connection,
    history_id: i64,
) -> Result<Option<StoredEnrichment>> {
    let mut statement = connection.prepare(
        "SELECT content_source, fetch_status, fetched_at, final_url, language, readable_title,
                readable_text, snippet_json
         FROM visit_content_enrichments
         WHERE history_id = ?1
         ORDER BY CASE content_source WHEN 'capture' THEN 0 ELSE 1 END, fetched_at DESC",
    )?;
    statement
        .query_row([history_id], |row: &Row<'_>| {
            let _: Option<String> = row.get(3)?;
            let _: Option<String> = row.get(4)?;
            Ok(StoredEnrichment {
                content_source: row.get(0)?,
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
        visit.source_role = classify_source_role(visit);
        visit.page_type = classify_page_type(visit);
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
        visit.novelty_score = (1.0 - max_similarity).clamp(0.0, 1.0);
        visit.importance_score = ((*revisit_count as f32 - 1.0) * 0.35
            + (visit.duration_ms.unwrap_or(0).max(0) as f32 / 60_000.0).min(2.0)
            + if visit.source_role == "docs" || visit.source_role == "repo" { 0.3 } else { 0.0 })
        .clamp(0.0, 4.0);
        visit.explore_score = ((visit.novelty_score * 0.7)
            + if *revisit_count <= 1 { 0.3 } else { 0.0 })
        .clamp(0.0, 1.0);
        prior_tokens.push(tokens);
    }
}

fn build_sessions(visits: &[VisitRecord]) -> Vec<SessionRecord> {
    let mut sessions = Vec::new();
    let mut current: Option<SessionRecord> = None;
    let mut previous: Option<&VisitRecord> = None;
    for (index, visit) in visits.iter().enumerate() {
        let same_session = previous.is_some_and(|previous| {
            previous.profile_id == visit.profile_id
                && (visit.from_visit == Some(previous.source_visit_id)
                    || chrome_gap_minutes(previous.visit_time, visit.visit_time)
                        <= SESSION_GAP_MINUTES)
        });
        if !same_session {
            if let Some(current) = current.take() {
                sessions.push(current);
            }
            current = Some(SessionRecord {
                profile_id: visit.profile_id.clone(),
                visit_indexes: vec![index],
            });
        } else if let Some(current) = &mut current {
            current.visit_indexes.push(index);
        }
        previous = Some(visit);
    }
    if let Some(current) = current {
        sessions.push(current);
    }
    sessions
}

fn assign_topics(visits: &mut [VisitRecord], window_days: u32) -> Vec<TopicAccumulator> {
    let mut topics = Vec::<TopicAccumulator>::new();
    for (index, visit) in visits.iter_mut().enumerate() {
        let visit_tokens = visit.keywords.iter().cloned().collect::<HashSet<_>>();
        let mut best_index = None;
        let mut best_score = 0.0f32;
        for (topic_index, topic) in topics.iter().enumerate() {
            let score = topic_similarity(topic, visit, &visit_tokens);
            if score > best_score {
                best_score = score;
                best_index = Some(topic_index);
            }
        }

        let topic_index = if best_score >= 0.38 {
            best_index.unwrap_or_default()
        } else {
            topics.push(TopicAccumulator {
                topic_id: format!("topic-{:03}", topics.len() + 1),
                label: String::new(),
                first_seen_at: visit.visited_at.clone(),
                last_seen_at: visit.visited_at.clone(),
                visit_indexes: Vec::new(),
                revisit_count: 0,
                trend_slope: 0.0,
                burst_score: 0.0,
                centroid: None,
                keyword_counts: HashMap::new(),
            });
            topics.len() - 1
        };

        visit.topic_id = Some(topics[topic_index].topic_id.clone());
        topics[topic_index].visit_indexes.push(index);
        topics[topic_index].last_seen_at = visit.visited_at.clone();
        if topics[topic_index].first_seen_at.is_empty() {
            topics[topic_index].first_seen_at = visit.visited_at.clone();
        }
        for keyword in &visit.keywords {
            *topics[topic_index].keyword_counts.entry(keyword.clone()).or_insert(0) += 1;
        }
        if let Some(vector) = &visit.vector {
            update_centroid(&mut topics[topic_index].centroid, vector);
        }
    }

    for topic in &mut topics {
        topic.label = build_topic_label(topic, visits);
        let (trend, burst) = topic_trend_scores(topic, visits, window_days);
        topic.trend_slope = trend;
        topic.burst_score = burst;
        topic.revisit_count = count_topic_revisits(topic, visits);
    }
    topics.sort_by(|left, right| right.visit_indexes.len().cmp(&left.visit_indexes.len()));
    topics
}

fn assign_threads(
    visits: &mut [VisitRecord],
    sessions: &[SessionRecord],
) -> Vec<ThreadAccumulator> {
    let mut threads = Vec::<ThreadAccumulator>::new();
    for session in sessions {
        let mut best_index = None;
        let mut best_score = 0.0f32;
        for (thread_index, thread) in threads.iter().enumerate() {
            if thread.profile_id != session.profile_id {
                continue;
            }
            let gap_days = chrome_gap_hours(
                visits[*thread.visit_indexes.last().expect("thread visit")].visit_time,
                visits[*session.visit_indexes.first().expect("session visit")].visit_time,
            ) / 24;
            if gap_days > THREAD_GAP_DAYS {
                continue;
            }
            let score = session_thread_similarity(session, thread, visits);
            if score > best_score {
                best_score = score;
                best_index = Some(thread_index);
            }
        }

        let thread_index = if best_score >= 0.34 {
            best_index.unwrap_or_default()
        } else {
            threads.push(ThreadAccumulator {
                thread_id: format!("thread-{:03}", threads.len() + 1),
                profile_id: session.profile_id.clone(),
                visit_indexes: Vec::new(),
                first_seen_at: visits[*session.visit_indexes.first().expect("session start")]
                    .visited_at
                    .clone(),
                last_seen_at: visits[*session.visit_indexes.last().expect("session end")]
                    .visited_at
                    .clone(),
                reopen_count: 0,
                open_loop_score: 0.0,
                dominant_topic_id: None,
                status: "active".to_string(),
                chromium_enhanced: false,
                title: String::new(),
            });
            threads.len() - 1
        };

        if let Some(previous_index) = threads[thread_index].visit_indexes.last().copied() {
            let gap_hours = chrome_gap_hours(
                visits[previous_index].visit_time,
                visits[*session.visit_indexes.first().unwrap()].visit_time,
            );
            if gap_hours >= REOPEN_GAP_HOURS {
                threads[thread_index].reopen_count += 1;
            }
        }
        threads[thread_index].visit_indexes.extend(session.visit_indexes.iter().copied());
        threads[thread_index].last_seen_at =
            visits[*session.visit_indexes.last().expect("session end")].visited_at.clone();
        threads[thread_index].chromium_enhanced |= session.visit_indexes.iter().any(|index| {
            visits[*index].query_term.is_some() || visits[*index].external_referrer_url.is_some()
        });
    }

    for thread in &mut threads {
        for index in &thread.visit_indexes {
            visits[*index].thread_id = Some(thread.thread_id.clone());
        }
        thread.open_loop_score = compute_open_loop_score(thread, visits);
        thread.dominant_topic_id = dominant_topic(thread, visits);
        thread.status = if thread.open_loop_score >= 1.8 {
            "open-loop".to_string()
        } else if DateTime::parse_from_rfc3339(&thread.last_seen_at)
            .ok()
            .is_some_and(|value| Utc::now() - value.with_timezone(&Utc) <= Duration::days(7))
        {
            "active".to_string()
        } else {
            "archived".to_string()
        };
        thread.title = build_thread_title(thread, visits);
    }
    threads.sort_by(|left, right| right.last_seen_at.cmp(&left.last_seen_at));
    threads
}

fn persist_features(connection: &Connection, visits: &[VisitRecord]) -> Result<()> {
    for visit in visits {
        connection.execute(
            "INSERT OR REPLACE INTO visit_insight_features
             (history_id, profile_id, topic_id, thread_id, page_type, source_role, query_term,
              query_stage, novelty_score, importance_score, explore_score, keywords_json,
              entities_json, updated_at, pipeline_version)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
            params![
                visit.history_id,
                visit.profile_id,
                visit.topic_id,
                visit.thread_id,
                visit.page_type,
                visit.source_role,
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

fn persist_topics(
    connection: &Connection,
    topics: &[TopicAccumulator],
    visits: &[VisitRecord],
    profile_id: Option<&str>,
    window_days: u32,
) -> Result<()> {
    let profile_scope = profile_id.unwrap_or("all");
    connection.execute(
        "DELETE FROM insight_topics WHERE profile_scope = ?1 AND window_days = ?2",
        params![profile_scope, window_days as i64],
    )?;
    for topic in topics {
        let evidence = topic_evidence(topic, visits);
        connection.execute(
            "INSERT OR REPLACE INTO insight_topics
             (topic_id, profile_scope, window_days, label, first_seen_at, last_seen_at, visit_count,
              revisit_count, trend_slope, burst_score, evidence_json)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                topic.topic_id,
                profile_scope,
                window_days as i64,
                topic.label,
                topic.first_seen_at,
                topic.last_seen_at,
                topic.visit_indexes.len() as i64,
                topic.revisit_count as i64,
                topic.trend_slope,
                topic.burst_score,
                serde_json::to_string(&evidence)?,
            ],
        )?;
    }
    Ok(())
}

fn persist_threads(
    connection: &Connection,
    threads: &[ThreadAccumulator],
    visits: &[VisitRecord],
) -> Result<()> {
    connection.execute("DELETE FROM insight_threads", [])?;
    connection.execute("DELETE FROM insight_thread_members", [])?;
    for thread in threads {
        let evidence = thread_evidence(thread, visits);
        connection.execute(
            "INSERT INTO insight_threads
             (thread_id, profile_id, title, status, first_seen_at, last_seen_at, visit_count,
              reopen_count, open_loop_score, dominant_topic_id, chromium_enhanced, evidence_json,
              summary_json, pipeline_version)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
            params![
                thread.thread_id,
                thread.profile_id,
                thread.title,
                thread.status,
                thread.first_seen_at,
                thread.last_seen_at,
                thread.visit_indexes.len() as i64,
                thread.reopen_count as i64,
                thread.open_loop_score,
                thread.dominant_topic_id,
                thread.chromium_enhanced as i64,
                serde_json::to_string(&evidence)?,
                serde_json::to_string(&json!({
                    "visitCount": thread.visit_indexes.len(),
                    "reopenCount": thread.reopen_count,
                    "openLoopScore": thread.open_loop_score,
                }))?,
                INSIGHT_PIPELINE_VERSION,
            ],
        )?;
        for (ordinal, visit_index) in thread.visit_indexes.iter().enumerate() {
            connection.execute(
                "INSERT INTO insight_thread_members (thread_id, history_id, ordinal, visited_at)
                 VALUES (?1, ?2, ?3, ?4)",
                params![
                    thread.thread_id,
                    visits[*visit_index].history_id,
                    ordinal as i64,
                    visits[*visit_index].visited_at,
                ],
            )?;
        }
    }
    Ok(())
}

fn build_cards(
    visits: &[VisitRecord],
    topics: &[TopicAccumulator],
    threads: &[ThreadAccumulator],
    window_days: u32,
    profile_id: Option<&str>,
) -> Vec<InsightCard> {
    let mut cards = Vec::new();
    if let Some(topic) = topics
        .iter()
        .filter(|topic| topic.trend_slope > 0.12)
        .max_by(|left, right| left.trend_slope.total_cmp(&right.trend_slope))
    {
        cards.push(InsightCard {
            card_id: format!("card-rising-{}", topic.topic_id),
            kind: "rising-topic".to_string(),
            title: format!("Rising topic: {}", topic.label),
            summary: format!(
                "{} is gaining momentum with {} visits in the last {} days.",
                topic.label,
                topic.visit_indexes.len(),
                window_days
            ),
            window_days,
            profile_id: profile_id.map(ToString::to_string),
            score: topic.trend_slope,
            chromium_enhanced: false,
            evidence: topic_evidence(topic, visits),
        });
    }
    if let Some(thread) =
        threads.iter().max_by(|left, right| left.open_loop_score.total_cmp(&right.open_loop_score))
    {
        cards.push(InsightCard {
            card_id: format!("card-open-loop-{}", thread.thread_id),
            kind: "open-loop".to_string(),
            title: format!("Open loop: {}", thread.title),
            summary: format!(
                "This thread reopened {} times and still looks unresolved.",
                thread.reopen_count
            ),
            window_days,
            profile_id: profile_id.map(ToString::to_string),
            score: thread.open_loop_score,
            chromium_enhanced: thread.chromium_enhanced,
            evidence: thread_evidence(thread, visits),
        });
    }
    if let Some(page) = revisited_pages(visits).into_iter().next() {
        cards.push(InsightCard {
            card_id: format!("card-revisit-{}", page.history_id),
            kind: "revisit".to_string(),
            title: "Important but unsaved".to_string(),
            summary: format!(
                "{} keeps resurfacing across sessions and is a good candidate to save or summarize.",
                page.title.clone().unwrap_or_else(|| page.url.clone())
            ),
            window_days,
            profile_id: profile_id.map(ToString::to_string),
            score: page.importance_score,
            chromium_enhanced: false,
            evidence: vec![evidence_from_visit(page, Some("High revisit importance".to_string()))],
        });
    }
    let explore_mean = if visits.is_empty() {
        0.0
    } else {
        visits.iter().map(|visit| visit.explore_score).sum::<f32>() / visits.len() as f32
    };
    cards.push(InsightCard {
        card_id: "card-focus-balance".to_string(),
        kind: "focus-balance".to_string(),
        title: if explore_mean >= 0.55 {
            "Explore-heavy window".to_string()
        } else {
            "Exploit-heavy window".to_string()
        },
        summary: if explore_mean >= 0.55 {
            "Browsing leaned toward new topics and wider exploration."
        } else {
            "Browsing leaned toward revisits and deeper follow-through."
        }
        .to_string(),
        window_days,
        profile_id: profile_id.map(ToString::to_string),
        score: explore_mean,
        chromium_enhanced: false,
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
    cards
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
                reopen_count, open_loop_score, dominant_topic_id, chromium_enhanced, evidence_json
         FROM insight_threads
         WHERE profile_id = ?1 AND last_seen_at >= ?2
         ORDER BY last_seen_at DESC"
    } else {
        "SELECT thread_id, profile_id, title, status, first_seen_at, last_seen_at, visit_count,
                reopen_count, open_loop_score, dominant_topic_id, chromium_enhanced, evidence_json
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
        reopen_count: row.get::<_, i64>(7)?.max(0) as usize,
        open_loop_score: row.get(8)?,
        dominant_topic_id: row.get(9)?,
        chromium_enhanced: row.get::<_, i64>(10)? != 0,
        evidence: serde_json::from_str::<Vec<InsightEvidenceItem>>(&row.get::<_, String>(11)?)
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
        let Some((_, query_term, stage)) = features.get(&visit.history_id) else {
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
                    &tokenize(previous).into_iter().collect(),
                    &tokenize(&query_term).into_iter().collect(),
                ) >= 0.2
        });
        if !continues {
            if let Some(current) = current.take() {
                ladders.push(current);
            }
            current = Some(InsightQueryLadder {
                root_term: query_term.clone(),
                profile_id: visit.profile_id.clone(),
                steps: vec![query_term.clone()],
                stages: vec![stage.clone()],
                count: 1,
                chromium_only: true,
            });
        } else if let Some(current) = &mut current {
            current.steps.push(query_term.clone());
            current.stages.push(stage.clone());
            current.count += 1;
        }
        previous_query = Some((query_term, visit.visit_time));
    }
    if let Some(current) = current {
        ladders.push(current);
    }
    ladders.sort_by(|left, right| right.count.cmp(&left.count));
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

fn topic_trend_scores(
    topic: &TopicAccumulator,
    visits: &[VisitRecord],
    window_days: u32,
) -> (f32, f32) {
    if topic.visit_indexes.is_empty() {
        return (0.0, 0.0);
    }
    let window_start = Utc::now() - Duration::days(window_days as i64);
    let last_week = Utc::now() - Duration::days(7);
    let mut prior = 0usize;
    let mut recent = 0usize;
    for index in &topic.visit_indexes {
        if let Ok(timestamp) = DateTime::parse_from_rfc3339(&visits[*index].visited_at) {
            let timestamp = timestamp.with_timezone(&Utc);
            if timestamp < window_start {
                continue;
            }
            if timestamp >= last_week {
                recent += 1;
            } else {
                prior += 1;
            }
        }
    }
    let baseline = (prior.max(1) as f32) / ((window_days.saturating_sub(7)).max(1) as f32 / 7.0);
    let trend = ((recent as f32) - baseline) / (topic.visit_indexes.len().max(1) as f32);
    let burst = recent as f32 / baseline.max(1.0);
    (trend, burst)
}

fn count_topic_revisits(topic: &TopicAccumulator, visits: &[VisitRecord]) -> usize {
    let mut counts = HashMap::<String, usize>::new();
    for index in &topic.visit_indexes {
        *counts.entry(canonical_visit_key(&visits[*index])).or_insert(0) += 1;
    }
    counts.values().filter(|count| **count > 1).count()
}

fn topic_evidence(topic: &TopicAccumulator, visits: &[VisitRecord]) -> Vec<InsightEvidenceItem> {
    topic
        .visit_indexes
        .iter()
        .rev()
        .take(4)
        .map(|index| evidence_from_visit(&visits[*index], Some(topic.label.clone())))
        .collect()
}

fn thread_evidence(thread: &ThreadAccumulator, visits: &[VisitRecord]) -> Vec<InsightEvidenceItem> {
    thread
        .visit_indexes
        .iter()
        .rev()
        .take(4)
        .map(|index| evidence_from_visit(&visits[*index], Some(thread.status.clone())))
        .collect()
}

fn evidence_from_visit(visit: &VisitRecord, note: Option<String>) -> InsightEvidenceItem {
    InsightEvidenceItem {
        history_id: visit.history_id,
        profile_id: visit.profile_id.clone(),
        url: visit.url.clone(),
        title: visit.title.clone().or_else(|| visit.readable_title.clone()),
        visited_at: visit.visited_at.clone(),
        note,
    }
}

fn revisited_pages(visits: &[VisitRecord]) -> Vec<&VisitRecord> {
    let mut counts = HashMap::<String, usize>::new();
    for visit in visits {
        *counts.entry(canonical_visit_key(visit)).or_insert(0) += 1;
    }
    let mut pages = visits
        .iter()
        .filter(|visit| counts.get(&canonical_visit_key(visit)).copied().unwrap_or(0) > 1)
        .collect::<Vec<_>>();
    pages.sort_by(|left, right| right.importance_score.total_cmp(&left.importance_score));
    pages
}

fn build_run_notes(
    embeddings_available: bool,
    enriched_visits: usize,
    failed_enrichments: usize,
) -> Vec<String> {
    let mut notes = Vec::new();
    notes.push(if embeddings_available {
        "Insight run used semantic vectors when available.".to_string()
    } else {
        "Insight run fell back to lexical and structural signals because no embedding provider was ready.".to_string()
    });
    notes.push(format!("Enriched {enriched_visits} visits with readable content."));
    if failed_enrichments > 0 {
        notes.push(format!(
            "{failed_enrichments} visit refetch attempts failed or returned unsupported content."
        ));
    }
    notes
}

fn query_term_from_url(url: &str) -> Option<String> {
    let (_, query) = url.split_once('?')?;
    for pair in query.split('&') {
        let (key, value) = pair.split_once('=')?;
        if matches!(key, "q" | "query" | "search_query" | "p") {
            let value = percent_decode(value);
            if !value.trim().is_empty() {
                return Some(value);
            }
        }
    }
    None
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

fn classify_source_role(visit: &VisitRecord) -> String {
    let domain = visit.domain();
    let url = visit.url.to_lowercase();
    if query_term_from_url(&visit.url).is_some()
        || matches!(
            domain.as_str(),
            "google.com" | "www.google.com" | "bing.com" | "duckduckgo.com" | "search.brave.com"
        )
    {
        "search".to_string()
    } else if domain.contains("github.com")
        || domain.contains("gitlab.com")
        || url.contains("/issues/")
        || url.contains("/pull/")
    {
        "repo".to_string()
    } else if domain.contains("developer.")
        || url.contains("/docs")
        || url.contains("/reference")
        || url.contains("/api/")
    {
        "docs".to_string()
    } else if domain.contains("reddit.com")
        || domain.contains("stackoverflow.com")
        || domain.contains("news.ycombinator.com")
        || url.contains("/forum")
        || url.contains("/discuss")
    {
        "forum".to_string()
    } else if domain.contains("youtube.com")
        || domain.contains("youtu.be")
        || domain.contains("vimeo.com")
    {
        "video".to_string()
    } else if domain.contains("amazon.")
        || url.contains("/pricing")
        || url.contains("/compare")
        || url.contains("/shop")
    {
        "shopping".to_string()
    } else if domain.contains("notion.so")
        || domain.contains("docs.google.com")
        || domain.contains("obsidian.md")
    {
        "notes".to_string()
    } else if domain.contains("x.com")
        || domain.contains("twitter.com")
        || domain.contains("linkedin.com")
    {
        "social".to_string()
    } else if domain.contains("medium.com")
        || domain.contains("substack.com")
        || domain.contains("nytimes.com")
    {
        "news".to_string()
    } else {
        "general".to_string()
    }
}

fn classify_page_type(visit: &VisitRecord) -> String {
    let url = visit.url.to_lowercase();
    if visit.source_role == "repo" && url.contains("/issues/") {
        "issue".to_string()
    } else if visit.source_role == "repo" && url.contains("/pull/") {
        "pull-request".to_string()
    } else if visit.source_role == "docs" {
        "documentation".to_string()
    } else if visit.source_role == "video" {
        "video".to_string()
    } else if visit.source_role == "search" {
        "search-results".to_string()
    } else if visit.source_role == "forum" {
        "discussion".to_string()
    } else if visit.source_role == "shopping" {
        "comparison".to_string()
    } else if url.contains("/blog") || url.contains("/article") {
        "article".to_string()
    } else {
        "page".to_string()
    }
}

fn extract_keywords(visit: &VisitRecord) -> Vec<String> {
    let mut counts = HashMap::<String, usize>::new();
    let mut push_tokens = |value: &str| {
        for token in tokenize(value) {
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

fn tokenize(input: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    for ch in input.chars() {
        if ch.is_alphanumeric() {
            current.push(ch.to_ascii_lowercase());
        } else if !current.is_empty() {
            if current.len() > 1 && !STOP_WORDS.contains(&current.as_str()) {
                tokens.push(current.clone());
            }
            current.clear();
        }
    }
    if !current.is_empty() && current.len() > 1 && !STOP_WORDS.contains(&current.as_str()) {
        tokens.push(current);
    }
    tokens
}

const STOP_WORDS: &[&str] = &[
    "the", "and", "for", "that", "with", "from", "into", "this", "your", "what", "how", "why",
    "when", "where", "about", "http", "https", "www", "com", "org", "net", "html",
];

fn token_similarity(left: &HashSet<String>, right: &HashSet<String>) -> f32 {
    if left.is_empty() || right.is_empty() {
        return 0.0;
    }
    let intersection = left.intersection(right).count() as f32;
    let union = left.union(right).count() as f32;
    if union == 0.0 { 0.0 } else { intersection / union }
}

fn topic_similarity(
    topic: &TopicAccumulator,
    visit: &VisitRecord,
    visit_tokens: &HashSet<String>,
) -> f32 {
    if let (Some(left), Some(right)) = (topic.centroid.as_ref(), visit.vector.as_ref()) {
        return cosine_similarity(left, right);
    }
    let topic_tokens = topic.keyword_counts.keys().cloned().collect::<HashSet<_>>();
    token_similarity(&topic_tokens, visit_tokens)
}

fn session_thread_similarity(
    session: &SessionRecord,
    thread: &ThreadAccumulator,
    visits: &[VisitRecord],
) -> f32 {
    let session_tokens = session
        .visit_indexes
        .iter()
        .flat_map(|index| visits[*index].keywords.iter().cloned())
        .collect::<HashSet<_>>();
    let thread_tokens = thread
        .visit_indexes
        .iter()
        .flat_map(|index| visits[*index].keywords.iter().cloned())
        .collect::<HashSet<_>>();
    token_similarity(&session_tokens, &thread_tokens)
}

fn update_centroid(centroid: &mut Option<Vec<f32>>, vector: &[f32]) {
    if let Some(centroid) = centroid {
        let len = centroid.len().min(vector.len());
        for index in 0..len {
            centroid[index] = (centroid[index] + vector[index]) / 2.0;
        }
    } else {
        *centroid = Some(vector.to_vec());
    }
}

fn build_topic_label(topic: &TopicAccumulator, visits: &[VisitRecord]) -> String {
    let top_keyword = topic
        .keyword_counts
        .iter()
        .max_by(|left, right| left.1.cmp(right.1))
        .map(|(keyword, _)| keyword.clone())
        .unwrap_or_else(|| "topic".to_string());
    let title = topic
        .visit_indexes
        .iter()
        .rev()
        .find_map(|index| {
            visits[*index].title.clone().or_else(|| visits[*index].readable_title.clone())
        })
        .unwrap_or_else(|| top_keyword.clone());
    if title.to_lowercase().contains(&top_keyword) {
        title
    } else {
        format!("{top_keyword} · {title}")
    }
}

fn compute_open_loop_score(thread: &ThreadAccumulator, visits: &[VisitRecord]) -> f32 {
    let revisit_count = thread
        .visit_indexes
        .iter()
        .map(|index| canonical_visit_key(&visits[*index]))
        .collect::<Vec<_>>();
    let revisit_unique = revisit_count.iter().collect::<HashSet<_>>().len();
    let compare_signals = thread
        .visit_indexes
        .iter()
        .filter(|index| {
            visits[**index]
                .query_term
                .as_deref()
                .is_some_and(|term| term.contains(" vs ") || term.contains("compare"))
                || visits[**index].page_type == "comparison"
        })
        .count();
    revisit_unique as f32 * 0.4 + thread.reopen_count as f32 * 0.75 + compare_signals as f32 * 0.2
}

fn dominant_topic(thread: &ThreadAccumulator, visits: &[VisitRecord]) -> Option<String> {
    let mut counts = HashMap::<String, usize>::new();
    for index in &thread.visit_indexes {
        if let Some(topic_id) = &visits[*index].topic_id {
            *counts.entry(topic_id.clone()).or_insert(0) += 1;
        }
    }
    counts.into_iter().max_by(|left, right| left.1.cmp(&right.1)).map(|(topic, _)| topic)
}

fn build_thread_title(thread: &ThreadAccumulator, visits: &[VisitRecord]) -> String {
    if let Some(query) =
        thread.visit_indexes.iter().find_map(|index| visits[*index].query_term.clone())
    {
        return query;
    }
    if let Some(title) = thread.visit_indexes.iter().rev().find_map(|index| {
        visits[*index].title.clone().or_else(|| visits[*index].readable_title.clone())
    }) {
        return title;
    }
    "Untitled thread".to_string()
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
        if query_term.contains(previous) || tokenize(&query_term).len() > tokenize(previous).len() {
            "narrowing".to_string()
        } else {
            "broadening".to_string()
        }
    } else if tokenize(&query_term).len() >= 5 {
        "narrowing".to_string()
    } else {
        "broad".to_string()
    }
}

fn cosine_similarity(left: &[f32], right: &[f32]) -> f32 {
    let len = left.len().min(right.len());
    if len == 0 {
        return 0.0;
    }
    let mut dot = 0.0f32;
    let mut left_norm = 0.0f32;
    let mut right_norm = 0.0f32;
    for index in 0..len {
        dot += left[index] * right[index];
        left_norm += left[index] * left[index];
        right_norm += right[index] * right[index];
    }
    if left_norm == 0.0 || right_norm == 0.0 {
        0.0
    } else {
        dot / (left_norm.sqrt() * right_norm.sqrt())
    }
}

fn chrome_gap_minutes(left: i64, right: i64) -> i64 {
    (right.saturating_sub(left) / 60_000_000).max(0)
}

fn chrome_gap_hours(left: i64, right: i64) -> i64 {
    (right.saturating_sub(left) / 3_600_000_000).max(0)
}

trait VisitDomain {
    fn domain(&self) -> String;
}

impl VisitDomain for VisitRecord {
    fn domain(&self) -> String {
        url_domain(&self.url)
    }
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
        models::{AiProviderConfig, AiProviderPurpose, AiRequestFormat, AiSettings, ArchiveMode},
        utils::iso_to_chrome_time_micros,
    };
    use tempfile::tempdir;

    fn test_paths() -> ProjectPaths {
        let dir = tempdir().expect("tempdir");
        ProjectPaths {
            app_root: dir.path().to_path_buf(),
            config_path: dir.path().join("config.json"),
            archive_database_path: dir.path().join("archive/history-vault.sqlite"),
            audit_repo_path: dir.path().join("audit"),
            manifests_dir: dir.path().join("audit/manifests"),
            exports_dir: dir.path().join("exports"),
            raw_snapshots_dir: dir.path().join("raw-snapshots"),
            staging_dir: dir.path().join("staging"),
            quarantine_dir: dir.path().join("quarantine"),
            schedule_dir: dir.path().join("schedule"),
            stronghold_path: dir.path().join("vault.hold"),
            stronghold_salt_path: dir.path().join("stronghold-salt.txt"),
        }
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
        let visit_four = (Utc::now() - Duration::days(2)).to_rfc3339();
        connection
            .execute(
                "INSERT INTO visit_events
                 (id, profile_id, source_visit_id, source_url_id, url, title, visit_time, from_visit, transition, visit_duration, is_known_to_sync, visited_link_id, external_referrer_url, app_id, event_fingerprint, payload_hash, recorded_at)
                 VALUES
                 (1, 'chrome:Default', 1, 1, 'https://example.com/docs/archive', 'Archive docs', ?1, NULL, 805306368, 24000, 1, NULL, 'https://google.com', NULL, 'a', 'a', ?5),
                 (2, 'chrome:Default', 2, 2, 'https://github.com/example/repo/issues/1', 'Issue one', ?2, 1, 805306368, 12000, 1, NULL, NULL, NULL, 'b', 'b', ?6),
                 (3, 'chrome:Default', 3, 3, 'https://www.google.com/search?q=archive+tool+compare', 'Google Search', ?3, NULL, 805306368, 6000, 1, NULL, NULL, NULL, 'c', 'c', ?7),
                 (4, 'chrome:Default', 4, 4, 'https://example.com/pricing', 'Pricing', ?4, NULL, 805306368, 8000, 1, NULL, NULL, NULL, 'd', 'd', ?8)",
                params![
                    iso_to_chrome_time_micros(&visit_one).expect("visit one chrome time"),
                    iso_to_chrome_time_micros(&visit_two).expect("visit two chrome time"),
                    iso_to_chrome_time_micros(&visit_three).expect("visit three chrome time"),
                    iso_to_chrome_time_micros(&visit_four).expect("visit four chrome time"),
                    visit_one,
                    visit_two,
                    visit_three,
                    visit_four,
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
                 VALUES (
                   3,
                   'archive tool compare',
                   'archive tool compare',
                   (SELECT id FROM source_profiles WHERE profile_key = 'chrome:Default'),
                   0,
                   'chrome:Default',
                   1,
                   ?1
                 )",
                [visit_three],
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
        assert!(snapshot.workflow_map.chromium_enhanced);
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
    fn query_stage_heuristics_cover_compare_and_site_restrict() {
        assert_eq!(classify_query_stage(Some("best archive tool vs obsidian"), None), "compare");
        assert_eq!(
            classify_query_stage(Some("site:github.com archive tool"), None),
            "site-restrict"
        );
    }
}
