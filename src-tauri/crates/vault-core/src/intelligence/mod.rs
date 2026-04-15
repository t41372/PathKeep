//! Core Intelligence backend.
//!
//! This module owns the deterministic, non-LLM Core Intelligence read/write
//! plane introduced by the 2026-04-15 hard reset. The intelligence SQLite
//! plane stays rebuildable: canonical archive facts remain in
//! `archive/history-vault.sqlite`, while this module materializes sessions,
//! search trails, refind pages, rollups, and related analytics in
//! `derived/history-intelligence.sqlite`.

mod phase_four;
mod phase_three;
mod site_dictionary;

use self::site_dictionary::{
    SiteDictionaryEntry, classify_visit, display_name_for_domain, display_name_for_search_engine,
    normalize_query,
};
use crate::{
    archive::open_intelligence_connection,
    config::ProjectPaths,
    intelligence_runtime::DeterministicModuleRuntimeUpdate,
    intelligence_runtime::persist_deterministic_module_runtime_updates,
    models::{
        ACTIVITY_MIX_MODULE_ID, ActivityMix, ActivityMixTrend, ActivityMixTrendPoint, AppConfig,
        ArrivalBreakdown, CategoryChangeEntry, CategoryFilteredDateRangeRequest, CategoryMixEntry,
        ClearDerivedIntelligenceReport, CoreIntelligenceRebuildReport,
        CoreIntelligenceRebuildRequest, DAILY_ROLLUPS_MODULE_ID, DOMAIN_DEEP_DIVE_MODULE_ID,
        DateRange, DigestSummary, DiscoveryTrend, DiscoveryTrendPoint, DomainDeepDive,
        DomainDeepDiveRequest, DomainFlowStat, DomainPageStat, DomainTrend, DomainTrendPoint,
        DomainTrendRequest, EngineEffectiveness, EngineRanking, ExplainRefindRequest,
        FrictionSignal, GranularityDateRangeRequest, HardTopic, HubPage, InsightStatus, KpiMetric,
        NavigationPath, NavigationPathStep, OnThisDayEntry, PagedDateRangeRequest, QueryFamily,
        QueryFamilyResult, REFIND_PAGES_MODULE_ID, RefindExplanation, RefindPage,
        RefindPagesRequest, RefindScoreFactor, ReopenedInvestigation, RhythmHeatmap,
        RhythmHeatmapCell, SEARCH_EFFECTIVENESS_MODULE_ID, SEARCH_TRAILS_MODULE_ID,
        SESSIONS_MODULE_ID, ScopedDateRangeRequest, SearchConcept, SearchEffectiveness,
        SearchEffectivenessRequest, SearchTrailQueryRequest, SessionDetail, SessionListResult,
        SessionSummary, SessionVisit, StableSource, TopSearchConceptsRequest, TopSite,
        TopSitesRequest, TrailDetail, TrailListResult, TrailMember, TrailSummary,
        VISIT_DERIVED_FACTS_MODULE_ID,
    },
    utils::now_rfc3339,
};
use anyhow::{Context, Result};
use chrono::{Datelike, Duration, Local, LocalResult, NaiveDate, TimeZone, Timelike, Utc};
use rusqlite::{Connection, OptionalExtension, Row, params};
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

pub use self::phase_four::{get_compare_sets, get_multi_browser_diff};
pub use self::phase_three::{
    get_breadth_index, get_habit_patterns, get_interrupted_habits, get_observed_interactions,
    get_path_flows,
};

const CORE_INTELLIGENCE_SCHEMA_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS visit_content_enrichments (
  id                  INTEGER PRIMARY KEY AUTOINCREMENT,
  history_id          INTEGER NOT NULL,
  content_source      TEXT NOT NULL,
  fetch_status        TEXT NOT NULL,
  fetched_at          TEXT NOT NULL,
  final_url           TEXT,
  language            TEXT,
  readable_title      TEXT,
  readable_text_blob_path TEXT,
  readable_text_bytes INTEGER NOT NULL DEFAULT 0,
  text_hash           TEXT,
  snippet_json        TEXT NOT NULL,
  extraction_json     TEXT NOT NULL,
  pipeline_version    TEXT NOT NULL,
  UNIQUE(history_id, content_source)
);
CREATE INDEX IF NOT EXISTS idx_visit_content_enrichments_history_id
  ON visit_content_enrichments(history_id);
CREATE INDEX IF NOT EXISTS idx_visit_content_enrichments_status
  ON visit_content_enrichments(fetch_status, fetched_at);

CREATE TABLE IF NOT EXISTS visit_derived_facts (
  visit_id           INTEGER PRIMARY KEY,
  profile_id         TEXT NOT NULL,
  session_id         TEXT,
  trail_id           TEXT,
  registrable_domain TEXT NOT NULL,
  canonical_url      TEXT NOT NULL,
  domain_category    TEXT NOT NULL DEFAULT 'unknown',
  page_category      TEXT NOT NULL DEFAULT 'unknown',
  search_engine      TEXT,
  search_query       TEXT,
  is_new_domain      INTEGER NOT NULL DEFAULT 0,
  is_search_event    INTEGER NOT NULL DEFAULT 0,
  evidence_tier      TEXT NOT NULL DEFAULT 'tier-c',
  taxonomy_source    TEXT NOT NULL DEFAULT 'unknown',
  taxonomy_pack      TEXT,
  taxonomy_version   TEXT,
  computed_at        TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_vdf_profile_session ON visit_derived_facts(profile_id, session_id);
CREATE INDEX IF NOT EXISTS idx_vdf_profile_trail ON visit_derived_facts(profile_id, trail_id);
CREATE INDEX IF NOT EXISTS idx_vdf_profile_domain ON visit_derived_facts(profile_id, registrable_domain);
CREATE INDEX IF NOT EXISTS idx_vdf_profile_search ON visit_derived_facts(profile_id, is_search_event, search_engine);

CREATE TABLE IF NOT EXISTS domain_daily_rollups (
  date_key           TEXT NOT NULL,
  profile_id         TEXT NOT NULL,
  registrable_domain TEXT NOT NULL,
  domain_category    TEXT NOT NULL,
  visit_count        INTEGER NOT NULL,
  search_count       INTEGER NOT NULL,
  new_domain_visits  INTEGER NOT NULL,
  unique_urls        INTEGER NOT NULL,
  PRIMARY KEY(date_key, profile_id, registrable_domain)
);
CREATE TABLE IF NOT EXISTS category_daily_rollups (
  date_key           TEXT NOT NULL,
  profile_id         TEXT NOT NULL,
  domain_category    TEXT NOT NULL,
  visit_count        INTEGER NOT NULL,
  unique_domains     INTEGER NOT NULL,
  PRIMARY KEY(date_key, profile_id, domain_category)
);
CREATE TABLE IF NOT EXISTS engine_daily_rollups (
  date_key           TEXT NOT NULL,
  profile_id         TEXT NOT NULL,
  search_engine      TEXT NOT NULL,
  search_count       INTEGER NOT NULL,
  PRIMARY KEY(date_key, profile_id, search_engine)
);
CREATE TABLE IF NOT EXISTS daily_summary_rollups (
  date_key           TEXT NOT NULL,
  profile_id         TEXT NOT NULL,
  total_visits       INTEGER NOT NULL,
  total_searches     INTEGER NOT NULL,
  new_domains        INTEGER NOT NULL,
  unique_domains     INTEGER NOT NULL,
  hhi_score          REAL,
  discovery_rate     REAL,
  PRIMARY KEY(date_key, profile_id)
);

CREATE TABLE IF NOT EXISTS sessions (
  session_id         TEXT PRIMARY KEY,
  profile_id         TEXT NOT NULL,
  first_visit_ms     INTEGER NOT NULL,
  last_visit_ms      INTEGER NOT NULL,
  visit_count        INTEGER NOT NULL,
  search_count       INTEGER NOT NULL,
  domain_count       INTEGER NOT NULL,
  is_deep_dive       INTEGER NOT NULL DEFAULT 0,
  auto_title         TEXT,
  computed_at        TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_sessions_profile_time ON sessions(profile_id, first_visit_ms DESC);

CREATE TABLE IF NOT EXISTS search_trails (
  trail_id            TEXT PRIMARY KEY,
  profile_id          TEXT NOT NULL,
  session_id          TEXT,
  initial_query       TEXT NOT NULL,
  search_engine       TEXT NOT NULL,
  reformulation_count INTEGER NOT NULL DEFAULT 0,
  visit_count         INTEGER NOT NULL,
  landing_url         TEXT,
  landing_domain      TEXT,
  first_visit_ms      INTEGER NOT NULL,
  last_visit_ms       INTEGER NOT NULL,
  max_depth           INTEGER NOT NULL DEFAULT 0,
  queries_json        TEXT NOT NULL,
  computed_at         TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_search_trails_profile_time ON search_trails(profile_id, first_visit_ms DESC);

CREATE TABLE IF NOT EXISTS search_trail_members (
  trail_id           TEXT NOT NULL,
  profile_id         TEXT NOT NULL,
  visit_id           INTEGER NOT NULL,
  ordinal            INTEGER NOT NULL,
  role               TEXT NOT NULL,
  PRIMARY KEY(trail_id, visit_id)
);
CREATE INDEX IF NOT EXISTS idx_search_trail_members_profile_visit ON search_trail_members(profile_id, visit_id);

CREATE TABLE IF NOT EXISTS search_events (
  visit_id           INTEGER PRIMARY KEY,
  profile_id         TEXT NOT NULL,
  search_engine      TEXT NOT NULL,
  raw_query          TEXT NOT NULL,
  normalized_query   TEXT NOT NULL,
  trail_id           TEXT,
  computed_at        TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_search_events_profile_engine ON search_events(profile_id, search_engine);

CREATE TABLE IF NOT EXISTS search_event_terms (
  visit_id           INTEGER NOT NULL,
  profile_id         TEXT NOT NULL,
  term               TEXT NOT NULL,
  PRIMARY KEY(visit_id, term)
);
CREATE INDEX IF NOT EXISTS idx_search_event_terms_profile_term ON search_event_terms(profile_id, term);

CREATE TABLE IF NOT EXISTS query_families (
  family_id          TEXT PRIMARY KEY,
  profile_id         TEXT NOT NULL,
  anchor_query       TEXT NOT NULL,
  member_count       INTEGER NOT NULL,
  search_engine      TEXT NOT NULL,
  first_seen_ms      INTEGER NOT NULL,
  last_seen_ms       INTEGER NOT NULL,
  queries_json       TEXT NOT NULL,
  computed_at        TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_query_families_profile_time ON query_families(profile_id, last_seen_ms DESC);

CREATE TABLE IF NOT EXISTS refind_pages (
  profile_id            TEXT NOT NULL,
  canonical_url         TEXT NOT NULL,
  url                   TEXT NOT NULL,
  title                 TEXT,
  registrable_domain    TEXT NOT NULL,
  cross_day_count       INTEGER NOT NULL,
  trail_count           INTEGER NOT NULL,
  search_arrival_count  INTEGER NOT NULL,
  typed_revisit_count   INTEGER NOT NULL,
  refind_score          REAL NOT NULL,
  evidence_json         TEXT NOT NULL,
  first_seen_ms         INTEGER NOT NULL,
  last_seen_ms          INTEGER NOT NULL,
  computed_at           TEXT NOT NULL,
  PRIMARY KEY(profile_id, canonical_url)
);
CREATE INDEX IF NOT EXISTS idx_refind_pages_profile_score ON refind_pages(profile_id, refind_score DESC, last_seen_ms DESC);

CREATE TABLE IF NOT EXISTS source_effectiveness (
  profile_id             TEXT NOT NULL,
  registrable_domain     TEXT NOT NULL,
  source_role            TEXT NOT NULL,
  trail_count            INTEGER NOT NULL,
  stable_landing_count   INTEGER NOT NULL,
  effectiveness_score    REAL NOT NULL,
  evidence_json          TEXT NOT NULL,
  first_seen_ms          INTEGER NOT NULL,
  last_seen_ms           INTEGER NOT NULL,
  computed_at            TEXT NOT NULL,
  PRIMARY KEY(profile_id, registrable_domain)
);
CREATE INDEX IF NOT EXISTS idx_source_effectiveness_profile_score ON source_effectiveness(profile_id, effectiveness_score DESC);

CREATE TABLE IF NOT EXISTS habit_patterns (
  profile_id           TEXT NOT NULL,
  registrable_domain   TEXT NOT NULL,
  habit_type           TEXT NOT NULL,
  mean_interval_days   REAL NOT NULL,
  cv                   REAL NOT NULL,
  visit_count          INTEGER NOT NULL,
  last_visited_ms      INTEGER NOT NULL,
  is_interrupted       INTEGER NOT NULL DEFAULT 0,
  computed_at          TEXT NOT NULL,
  PRIMARY KEY(profile_id, registrable_domain)
);

CREATE TABLE IF NOT EXISTS reopened_investigations (
  investigation_id     TEXT PRIMARY KEY,
  profile_id           TEXT NOT NULL,
  anchor_type          TEXT NOT NULL,
  anchor_id            TEXT NOT NULL,
  anchor_label         TEXT NOT NULL,
  occurrence_count     INTEGER NOT NULL,
  distinct_days        INTEGER NOT NULL,
  first_seen_ms        INTEGER NOT NULL,
  last_seen_ms         INTEGER NOT NULL,
  evidence_json        TEXT NOT NULL,
  computed_at          TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_reopened_investigations_profile_time ON reopened_investigations(profile_id, last_seen_ms DESC);

CREATE TABLE IF NOT EXISTS path_flows (
  profile_id          TEXT NOT NULL,
  flow_pattern        TEXT NOT NULL,
  step_count          INTEGER NOT NULL,
  occurrence_count    INTEGER NOT NULL,
  last_seen_ms        INTEGER NOT NULL,
  PRIMARY KEY(profile_id, flow_pattern, step_count)
);
"#;

const LEGACY_INSIGHT_TABLES: &[&str] = &[
    "insight_bursts",
    "insight_query_groups",
    "insight_query_group_members",
    "insight_topics",
    "insight_threads",
    "insight_thread_members",
    "insight_reference_pages",
    "insight_source_effectiveness",
    "insight_cards",
    "insight_snapshot_payloads",
    "insight_runs",
    "visit_insight_features",
];
const SESSION_GAP_MS: i64 = 30 * 60 * 1_000;
const TRAIL_GAP_MS: i64 = 15 * 60 * 1_000;
const CORE_PHASES: &[(&str, &str)] = &[
    ("visit-derived-facts", "Normalizing visits and site dictionary facts"),
    ("daily-rollups", "Computing daily rollups"),
    ("sessions", "Building browsing sessions"),
    ("search-trails", "Building search trails and query families"),
    ("refind-pages", "Computing refind pages and source effectiveness"),
    ("activity-mix", "Composing digest and activity mix surfaces"),
    ("deep-intelligence", "Computing Phase 2 Core Intelligence surfaces"),
];

#[derive(Debug, Clone)]
pub struct CoreIntelligenceProgress {
    pub phase: String,
    pub detail: String,
    pub processed_items: Option<usize>,
    pub total_items: Option<usize>,
    pub progress_percent: Option<f32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CoreIntelligenceJobKind {
    VisitDerive,
    DailyRollup,
    StructuralRebuild,
    FullRebuild,
}

impl CoreIntelligenceJobKind {
    fn from_job_type(job_type: &str) -> Result<Self> {
        match job_type {
            "visit-derive" => Ok(Self::VisitDerive),
            "daily-rollup" => Ok(Self::DailyRollup),
            "structural-rebuild" => Ok(Self::StructuralRebuild),
            "full-rebuild" => Ok(Self::FullRebuild),
            _ => anyhow::bail!("'{job_type}' is not a supported Core Intelligence job type."),
        }
    }

    fn requires_visit_derived_facts(self) -> bool {
        matches!(self, Self::VisitDerive | Self::FullRebuild)
    }

    fn requires_daily_rollups(self) -> bool {
        matches!(self, Self::DailyRollup | Self::FullRebuild)
    }

    fn requires_structural_entities(self) -> bool {
        matches!(self, Self::StructuralRebuild | Self::FullRebuild)
    }

    fn module_ids(self) -> &'static [&'static str] {
        match self {
            Self::VisitDerive => &[VISIT_DERIVED_FACTS_MODULE_ID],
            Self::DailyRollup => &[DAILY_ROLLUPS_MODULE_ID, ACTIVITY_MIX_MODULE_ID],
            Self::StructuralRebuild => &[
                SESSIONS_MODULE_ID,
                SEARCH_TRAILS_MODULE_ID,
                REFIND_PAGES_MODULE_ID,
                SEARCH_EFFECTIVENESS_MODULE_ID,
                DOMAIN_DEEP_DIVE_MODULE_ID,
            ],
            Self::FullRebuild => &[
                VISIT_DERIVED_FACTS_MODULE_ID,
                DAILY_ROLLUPS_MODULE_ID,
                SESSIONS_MODULE_ID,
                SEARCH_TRAILS_MODULE_ID,
                REFIND_PAGES_MODULE_ID,
                ACTIVITY_MIX_MODULE_ID,
                SEARCH_EFFECTIVENESS_MODULE_ID,
                DOMAIN_DEEP_DIVE_MODULE_ID,
            ],
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::VisitDerive => "visit-derived facts refresh",
            Self::DailyRollup => "daily rollup refresh",
            Self::StructuralRebuild => "structural entity rebuild",
            Self::FullRebuild => "full Core Intelligence rebuild",
        }
    }
}

#[derive(Debug, Clone)]
struct VisitRecord {
    visit_id: i64,
    profile_id: String,
    source_profile_id: i64,
    #[allow(dead_code)]
    source_visit_id: i64,
    source_url_id: i64,
    url: String,
    title: Option<String>,
    visit_time_ms: i64,
    from_visit: Option<i64>,
    #[allow(dead_code)]
    transition_type: Option<i64>,
    external_referrer_url: Option<String>,
    canonical_url: String,
    registrable_domain: String,
    domain_category: String,
    page_category: String,
    search_engine: Option<String>,
    search_query: Option<String>,
    is_new_domain: bool,
    is_search_event: bool,
    evidence_tier: String,
    taxonomy_source: String,
    taxonomy_pack: Option<String>,
    taxonomy_version: Option<String>,
    display_name: Option<String>,
    session_id: Option<String>,
    trail_id: Option<String>,
}

#[derive(Debug, Clone)]
struct SessionRecord {
    session_id: String,
    profile_id: String,
    first_visit_ms: i64,
    last_visit_ms: i64,
    visit_count: i64,
    search_count: i64,
    domain_count: i64,
    is_deep_dive: bool,
    auto_title: Option<String>,
    visit_ids: Vec<i64>,
}

#[derive(Debug, Clone)]
struct SearchEventRecord {
    visit_id: i64,
    profile_id: String,
    search_engine: String,
    raw_query: String,
    normalized_query: String,
    trail_id: Option<String>,
    visit_time_ms: i64,
}

#[derive(Debug, Clone)]
struct TrailRecord {
    trail_id: String,
    profile_id: String,
    session_id: String,
    initial_query: String,
    search_engine: String,
    reformulation_count: i64,
    visit_count: i64,
    landing_url: Option<String>,
    landing_domain: Option<String>,
    first_visit_ms: i64,
    last_visit_ms: i64,
    max_depth: i64,
    queries: Vec<String>,
    members: Vec<TrailMemberRecord>,
}

#[derive(Debug, Clone)]
struct TrailMemberRecord {
    trail_id: String,
    profile_id: String,
    visit_id: i64,
    ordinal: i64,
    role: String,
}

#[derive(Debug, Clone)]
struct QueryFamilyRecord {
    family_id: String,
    profile_id: String,
    anchor_query: String,
    member_count: i64,
    search_engine: String,
    first_seen_ms: i64,
    last_seen_ms: i64,
    queries: Vec<String>,
}

#[derive(Debug, Clone)]
struct RefindPageRecord {
    profile_id: String,
    canonical_url: String,
    url: String,
    title: Option<String>,
    registrable_domain: String,
    cross_day_count: i64,
    trail_count: i64,
    search_arrival_count: i64,
    typed_revisit_count: i64,
    refind_score: f32,
    evidence_json: String,
    first_seen_ms: i64,
    last_seen_ms: i64,
}

#[derive(Debug, Clone)]
struct SourceEffectivenessRecord {
    profile_id: String,
    registrable_domain: String,
    source_role: String,
    trail_count: i64,
    stable_landing_count: i64,
    effectiveness_score: f32,
    evidence_json: String,
    first_seen_ms: i64,
    last_seen_ms: i64,
}

#[derive(Debug, Clone)]
struct ReopenedInvestigationRecord {
    investigation_id: String,
    profile_id: String,
    anchor_type: String,
    anchor_id: String,
    anchor_label: String,
    occurrence_count: i64,
    distinct_days: i64,
    first_seen_ms: i64,
    last_seen_ms: i64,
    evidence_json: String,
}

#[derive(Debug, Clone)]
struct PathFlowRecord {
    profile_id: String,
    flow_pattern: String,
    step_count: i64,
    occurrence_count: i64,
    last_seen_ms: i64,
}

#[derive(Debug, Clone)]
struct HabitPatternRecord {
    profile_id: String,
    registrable_domain: String,
    habit_type: String,
    mean_interval_days: f32,
    cv: f32,
    visit_count: i64,
    last_visited_ms: i64,
    is_interrupted: bool,
}

#[derive(Debug, Clone, Default)]
struct DailyRollupBundle {
    domain_rows: Vec<(String, String, String, String, i64, i64, i64, i64)>,
    category_rows: Vec<(String, String, String, i64, i64)>,
    engine_rows: Vec<(String, String, String, i64)>,
    summary_rows: Vec<(String, String, i64, i64, i64, i64, f32, f32)>,
}

pub(crate) fn ensure_core_intelligence_schema(connection: &Connection) -> Result<()> {
    connection.execute_batch(CORE_INTELLIGENCE_SCHEMA_SQL)?;
    drop_legacy_insight_tables(connection)?;
    Ok(())
}

pub fn insight_status(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
) -> Result<InsightStatus> {
    let connection = open_intelligence_connection(paths, config, key)?;
    ensure_core_intelligence_schema(&connection)?;
    let session_count = table_row_count(&connection, "sessions")?;
    let trail_count = table_row_count(&connection, "search_trails")?;
    let refind_count = table_row_count(&connection, "refind_pages")?;
    let last_run_at = connection
        .query_row(
            "SELECT MAX(updated_at) FROM intelligence_jobs
             WHERE job_type IN ('visit-derive', 'daily-rollup', 'structural-rebuild', 'full-rebuild')
               AND state = 'succeeded'",
            [],
            |row| row.get::<_, Option<String>>(0),
        )
        .optional()?
        .flatten();
    Ok(InsightStatus {
        ready: session_count > 0 || trail_count > 0 || refind_count > 0,
        last_run_at,
        runs: 0,
        cards: session_count,
        topics: 0,
        threads: 0,
        query_groups: trail_count,
        reference_pages: refind_count,
        content_coverage: 0.0,
        warning: None,
    })
}

pub fn clear_derived_intelligence_state(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
) -> Result<ClearDerivedIntelligenceReport> {
    let connection = open_intelligence_connection(paths, config, key)?;
    ensure_core_intelligence_schema(&connection)?;
    let report = ClearDerivedIntelligenceReport {
        cleared_enrichment_rows: 0,
        cleared_feature_rows: table_row_count(&connection, "visit_derived_facts")?,
        cleared_burst_rows: table_row_count(&connection, "sessions")?,
        cleared_query_group_rows: table_row_count(&connection, "search_trails")?,
        cleared_topic_rows: table_row_count(&connection, "query_families")?,
        cleared_thread_rows: table_row_count(&connection, "reopened_investigations")?,
        cleared_reference_page_rows: table_row_count(&connection, "refind_pages")?,
        cleared_source_rows: table_row_count(&connection, "source_effectiveness")?,
        cleared_module_rows: table_row_count(&connection, "deterministic_module_runtime")?,
        cleared_card_rows: table_row_count(&connection, "daily_summary_rollups")?,
        cleared_run_rows: connection
            .execute(
                "DELETE FROM intelligence_jobs
                 WHERE job_type IN ('visit-derive', 'daily-rollup', 'structural-rebuild', 'full-rebuild')",
                [],
            )
            .unwrap_or(0),
        notes: vec![
            "Cleared Core Intelligence derived rows and module/runtime traces without touching canonical archive facts."
                .to_string(),
        ],
    };
    clear_core_tables(&connection, None)?;
    connection.execute("DELETE FROM deterministic_module_runtime", [])?;
    Ok(report)
}

pub fn run_core_intelligence(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    request: &CoreIntelligenceRebuildRequest,
) -> Result<CoreIntelligenceRebuildReport> {
    run_core_intelligence_job_with_progress(
        paths,
        config,
        key,
        CoreIntelligenceJobKind::FullRebuild,
        request,
        |_progress| Ok(()),
    )
}

pub fn run_core_intelligence_with_progress<F>(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    request: &CoreIntelligenceRebuildRequest,
    on_progress: F,
) -> Result<CoreIntelligenceRebuildReport>
where
    F: FnMut(CoreIntelligenceProgress) -> Result<()>,
{
    run_core_intelligence_job_with_progress(
        paths,
        config,
        key,
        CoreIntelligenceJobKind::FullRebuild,
        request,
        on_progress,
    )
}

pub fn run_core_intelligence_job_type_with_progress<F>(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    job_type: &str,
    request: &CoreIntelligenceRebuildRequest,
    on_progress: F,
) -> Result<CoreIntelligenceRebuildReport>
where
    F: FnMut(CoreIntelligenceProgress) -> Result<()>,
{
    run_core_intelligence_job_with_progress(
        paths,
        config,
        key,
        CoreIntelligenceJobKind::from_job_type(job_type)?,
        request,
        on_progress,
    )
}

fn run_core_intelligence_job_with_progress<F>(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    job_kind: CoreIntelligenceJobKind,
    request: &CoreIntelligenceRebuildRequest,
    mut on_progress: F,
) -> Result<CoreIntelligenceRebuildReport>
where
    F: FnMut(CoreIntelligenceProgress) -> Result<()>,
{
    let connection = open_intelligence_connection(paths, config, key)?;
    ensure_core_intelligence_schema(&connection)?;
    let run_id = Utc::now().timestamp_millis();
    let computed_at = now_rfc3339();
    let mut notes = vec![format!("Completed a {}.", job_kind.label())];
    if request.profile_id.is_none()
        && request.full_rebuild
        && job_kind == CoreIntelligenceJobKind::FullRebuild
    {
        notes.push(
            "Performed a full Core Intelligence rebuild over all visible profiles.".to_string(),
        );
    }
    let visits = load_visible_visits(&connection, request.profile_id.as_deref(), request.limit)?;
    if visits.is_empty() {
        clear_core_tables_for_job_kind(&connection, request.profile_id.as_deref(), job_kind)?;
        persist_ready_module_updates(
            &connection,
            run_id,
            Some(computed_at.clone()),
            job_kind.module_ids(),
            &["No visible visits matched the requested rebuild scope.".to_string()],
        )?;
        return Ok(CoreIntelligenceRebuildReport {
            run_id,
            processed_visits: 0,
            visit_derived_facts: 0,
            sessions: 0,
            search_trails: 0,
            query_families: 0,
            refind_pages: 0,
            source_effectiveness: 0,
            reopened_investigations: 0,
            notes,
            last_run_at: computed_at,
        });
    }

    let total_visible_visits = visits.len();
    on_progress(progress_for_phase(0, Some(0), Some(total_visible_visits)))?;
    let by_profile = build_profile_state(visits);
    on_progress(progress_for_phase(1, None, None))?;

    let needs_visit_derived_facts = job_kind.requires_visit_derived_facts();
    let needs_daily_rollups = job_kind.requires_daily_rollups();
    let needs_structural_entities = job_kind.requires_structural_entities();
    let mut all_visits = Vec::new();
    let mut all_sessions = Vec::new();
    let mut all_search_events = Vec::new();
    let mut all_trails = Vec::new();
    let mut all_query_families = Vec::new();
    let mut all_refind_pages = Vec::new();
    let mut all_source_effectiveness = Vec::new();
    let mut all_reopened = Vec::new();
    let mut all_path_flows = Vec::new();
    let mut all_habits = Vec::new();
    let mut rollups = DailyRollupBundle::default();

    let profile_total = by_profile.len();
    for (profile_index, (profile_id, mut profile_visits)) in by_profile.into_iter().enumerate() {
        compute_is_new_domain(&mut profile_visits);
        if needs_structural_entities {
            let sessions = build_sessions(&mut profile_visits);
            let (search_events, trails) = build_search_trails(&mut profile_visits);
            let query_families = build_query_families(&search_events);
            let refind_pages = build_refind_pages(&profile_visits);
            let source_effectiveness = build_source_effectiveness(&trails, &refind_pages);
            let reopened = build_reopened_investigations(&query_families, &refind_pages);
            let path_flows = build_path_flows(&profile_visits);
            let habits = build_habit_patterns(&profile_visits);
            all_sessions.extend(sessions);
            all_search_events.extend(search_events);
            all_trails.extend(trails);
            all_query_families.extend(query_families);
            all_refind_pages.extend(refind_pages);
            all_source_effectiveness.extend(source_effectiveness);
            all_reopened.extend(reopened);
            all_path_flows.extend(path_flows);
            all_habits.extend(habits);
        }
        if needs_daily_rollups {
            merge_rollups(&mut rollups, build_daily_rollups(&profile_visits));
        }
        if needs_visit_derived_facts {
            all_visits.extend(profile_visits);
        }
        on_progress(CoreIntelligenceProgress {
            phase: "profile-build".to_string(),
            detail: format!("Built Core Intelligence entities for profile {profile_id}"),
            processed_items: Some(profile_index + 1),
            total_items: Some(profile_total),
            progress_percent: Some(
                ((profile_index + 1) as f32 / profile_total.max(1) as f32) * 100.0,
            ),
        })?;
    }

    on_progress(progress_for_phase(4, None, None))?;
    persist_core_state_for_job_kind(
        &connection,
        request.profile_id.as_deref(),
        job_kind,
        &computed_at,
        &all_visits,
        &rollups,
        &all_sessions,
        &all_search_events,
        &all_trails,
        &all_query_families,
        &all_refind_pages,
        &all_source_effectiveness,
        &all_habits,
        &all_reopened,
        &all_path_flows,
    )?;
    on_progress(progress_for_phase(6, Some(total_visible_visits), Some(total_visible_visits)))?;

    persist_ready_module_updates(
        &connection,
        run_id,
        Some(computed_at.clone()),
        job_kind.module_ids(),
        &notes,
    )?;

    Ok(CoreIntelligenceRebuildReport {
        run_id,
        processed_visits: total_visible_visits,
        visit_derived_facts: all_visits.len(),
        sessions: all_sessions.len(),
        search_trails: all_trails.len(),
        query_families: all_query_families.len(),
        refind_pages: all_refind_pages.len(),
        source_effectiveness: all_source_effectiveness.len(),
        reopened_investigations: all_reopened.len(),
        notes,
        last_run_at: computed_at,
    })
}

fn progress_for_phase(
    phase_index: usize,
    processed_items: Option<usize>,
    total_items: Option<usize>,
) -> CoreIntelligenceProgress {
    let (phase, detail) = CORE_PHASES
        .get(phase_index.min(CORE_PHASES.len().saturating_sub(1)))
        .copied()
        .unwrap_or(("core-intelligence", "Updating Core Intelligence"));
    CoreIntelligenceProgress {
        phase: phase.to_string(),
        detail: detail.to_string(),
        processed_items,
        total_items,
        progress_percent: match (processed_items, total_items) {
            (Some(current), Some(total)) if total > 0 => {
                Some((current as f32 / total as f32) * 100.0)
            }
            _ => None,
        },
    }
}

fn persist_ready_module_updates(
    connection: &Connection,
    run_id: i64,
    built_at: Option<String>,
    module_ids: &[&str],
    notes: &[String],
) -> Result<()> {
    let shared_notes = if notes.is_empty() {
        vec!["Core Intelligence modules are in sync with the current derived plane.".to_string()]
    } else {
        notes.to_vec()
    };
    let updates = module_ids
        .iter()
        .map(|module_id| module_update(module_id, run_id, built_at.clone(), &shared_notes))
        .collect::<Vec<_>>();
    persist_deterministic_module_runtime_updates(connection, &updates)
}

fn module_update(
    module_id: &str,
    run_id: i64,
    built_at: Option<String>,
    notes: &[String],
) -> DeterministicModuleRuntimeUpdate {
    DeterministicModuleRuntimeUpdate {
        module_id: module_id.to_string(),
        status: "ready".to_string(),
        last_run_id: Some(run_id),
        last_built_at: built_at,
        last_invalidated_at: None,
        stale_reason: None,
        notes: notes.to_vec(),
    }
}

fn load_visible_visits(
    connection: &Connection,
    profile_id: Option<&str>,
    limit: Option<u32>,
) -> Result<Vec<VisitRecord>> {
    let mut statement = connection.prepare(
        "SELECT visits.id,
                source_profiles.profile_key,
                visits.source_profile_id,
                CAST(visits.source_visit_id AS INTEGER),
                urls.id,
                urls.url,
                urls.title,
                visits.visit_time_ms,
                visits.from_visit,
                visits.transition_type,
                visits.external_referrer_url
         FROM archive.visits AS visits
         JOIN archive.urls AS urls ON urls.id = visits.url_id
         JOIN archive.source_profiles AS source_profiles ON source_profiles.id = visits.source_profile_id
         WHERE visits.reverted_at IS NULL
           AND (?1 IS NULL OR source_profiles.profile_key = ?1)
         ORDER BY visits.visit_time_ms ASC, visits.id ASC",
    )?;
    let rows = statement.query_map([profile_id], visit_from_row)?;
    let mut visits = rows.collect::<rusqlite::Result<Vec<_>>>()?;
    hydrate_search_terms(connection, &mut visits)?;
    if let Some(limit) = limit {
        let keep = limit.max(1) as usize;
        if visits.len() > keep {
            visits = visits.split_off(visits.len() - keep);
        }
    }
    Ok(visits)
}

fn visit_from_row(row: &Row<'_>) -> rusqlite::Result<VisitRecord> {
    Ok(VisitRecord {
        visit_id: row.get(0)?,
        profile_id: row.get(1)?,
        source_profile_id: row.get(2)?,
        source_visit_id: row.get(3)?,
        source_url_id: row.get(4)?,
        url: row.get(5)?,
        title: row.get(6)?,
        visit_time_ms: row.get(7)?,
        from_visit: row.get(8)?,
        transition_type: row.get(9)?,
        external_referrer_url: row.get(10)?,
        canonical_url: String::new(),
        registrable_domain: String::new(),
        domain_category: "unknown".to_string(),
        page_category: "unknown".to_string(),
        search_engine: None,
        search_query: None,
        is_new_domain: false,
        is_search_event: false,
        evidence_tier: "tier-c".to_string(),
        taxonomy_source: "unknown".to_string(),
        taxonomy_pack: None,
        taxonomy_version: None,
        display_name: None,
        session_id: None,
        trail_id: None,
    })
}

fn hydrate_search_terms(connection: &Connection, visits: &mut [VisitRecord]) -> Result<()> {
    let profile_url_ids =
        visits.iter().fold(HashMap::<String, HashSet<i64>>::new(), |mut acc, visit| {
            acc.entry(visit.profile_id.clone()).or_default().insert(visit.source_url_id);
            acc
        });
    let mut query_map = HashMap::<(String, i64), String>::new();
    for (profile_id, url_ids) in profile_url_ids {
        let ids = url_ids.into_iter().collect::<Vec<_>>();
        for chunk in ids.chunks(400) {
            let placeholders = std::iter::repeat_n("?", chunk.len()).collect::<Vec<_>>().join(", ");
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
                .chain(chunk.iter().map(|value| value as &dyn rusqlite::ToSql));
            let rows = statement.query_map(rusqlite::params_from_iter(params), |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?, row.get::<_, String>(2)?))
            })?;
            for row in rows {
                let (profile_id, url_id, query) = row?;
                query_map.entry((profile_id, url_id)).or_insert(query);
            }
        }
    }

    for visit in visits {
        let query = query_map.get(&(visit.profile_id.clone(), visit.source_url_id)).cloned();
        let dictionary = classify_visit(
            &visit.url,
            visit.title.as_deref(),
            query.as_deref(),
            query.is_some(),
            visit.external_referrer_url.as_deref(),
            visit.from_visit,
        );
        apply_site_dictionary(visit, query, dictionary);
    }
    Ok(())
}

fn apply_site_dictionary(
    visit: &mut VisitRecord,
    query: Option<String>,
    dictionary: SiteDictionaryEntry,
) {
    visit.canonical_url = dictionary.canonical_url;
    visit.registrable_domain = dictionary.registrable_domain;
    visit.domain_category = dictionary.domain_category;
    visit.page_category = dictionary.page_category;
    visit.search_engine = dictionary.search_engine;
    visit.search_query = query
        .filter(|value| !value.trim().is_empty())
        .map(|value| normalize_query(&value))
        .or(dictionary.search_query);
    visit.is_search_event = visit.search_query.is_some() && visit.search_engine.is_some();
    visit.evidence_tier = dictionary.evidence_tier;
    visit.taxonomy_source = dictionary.taxonomy_source;
    visit.taxonomy_pack = dictionary.taxonomy_pack;
    visit.taxonomy_version = dictionary.taxonomy_version;
    visit.display_name = dictionary.display_name;
}

fn build_profile_state(visits: Vec<VisitRecord>) -> BTreeMap<String, Vec<VisitRecord>> {
    let mut profiles = BTreeMap::<String, Vec<VisitRecord>>::new();
    for visit in visits {
        profiles.entry(visit.profile_id.clone()).or_default().push(visit);
    }
    profiles
}

fn compute_is_new_domain(visits: &mut [VisitRecord]) {
    let mut seen = HashSet::<String>::new();
    for visit in visits {
        if seen.insert(visit.registrable_domain.clone()) {
            visit.is_new_domain = true;
        }
    }
}

fn build_sessions(visits: &mut [VisitRecord]) -> Vec<SessionRecord> {
    let mut sessions = Vec::new();
    let mut current: Option<SessionRecord> = None;
    for visit in visits.iter_mut() {
        let start_new = current
            .as_ref()
            .is_none_or(|session| visit.visit_time_ms - session.last_visit_ms > SESSION_GAP_MS);
        if start_new {
            if let Some(session) = current.take() {
                sessions.push(session);
            }
            current = Some(SessionRecord {
                session_id: format!("session:{}:{:04}", visit.profile_id, sessions.len() + 1),
                profile_id: visit.profile_id.clone(),
                first_visit_ms: visit.visit_time_ms,
                last_visit_ms: visit.visit_time_ms,
                visit_count: 0,
                search_count: 0,
                domain_count: 0,
                is_deep_dive: false,
                auto_title: None,
                visit_ids: Vec::new(),
            });
        }
        let session = current.as_mut().expect("current session");
        session.last_visit_ms = visit.visit_time_ms;
        session.visit_count += 1;
        session.search_count += i64::from(visit.is_search_event);
        session.visit_ids.push(visit.visit_id);
        visit.session_id = Some(session.session_id.clone());
    }
    if let Some(session) = current.take() {
        sessions.push(session);
    }

    for session in &mut sessions {
        let members = visits
            .iter()
            .filter(|visit| visit.session_id.as_deref() == Some(session.session_id.as_str()))
            .collect::<Vec<_>>();
        let domains =
            members.iter().map(|visit| visit.registrable_domain.clone()).collect::<HashSet<_>>();
        session.domain_count = domains.len() as i64;
        session.auto_title = build_session_title(&members);
        session.is_deep_dive = session.visit_count >= 10 && session.search_count >= 2;
    }
    sessions
}

fn build_session_title(visits: &[&VisitRecord]) -> Option<String> {
    let top_domain = visits
        .iter()
        .fold(HashMap::<String, usize>::new(), |mut acc, visit| {
            *acc.entry(visit.registrable_domain.clone()).or_default() += 1;
            acc
        })
        .into_iter()
        .max_by(|left, right| left.1.cmp(&right.1).then_with(|| right.0.cmp(&left.0)))
        .map(|(domain, _)| display_name_for_domain(&domain).unwrap_or(domain));
    let top_query = visits.iter().find_map(|visit| visit.search_query.clone());
    match (top_domain, top_query) {
        (Some(domain), Some(query)) => Some(format!("{domain} · {query}")),
        (Some(domain), None) => Some(domain),
        (None, Some(query)) => Some(query),
        (None, None) => None,
    }
}

fn build_search_trails(visits: &mut [VisitRecord]) -> (Vec<SearchEventRecord>, Vec<TrailRecord>) {
    let mut search_events = Vec::new();
    let mut trails = Vec::new();
    let mut current: Option<TrailRecord> = None;

    for visit in visits.iter_mut() {
        let visit_query = visit.search_query.clone();
        if visit.is_search_event {
            search_events.push(SearchEventRecord {
                visit_id: visit.visit_id,
                profile_id: visit.profile_id.clone(),
                search_engine: visit.search_engine.clone().unwrap_or_else(|| "unknown".to_string()),
                raw_query: visit_query.clone().unwrap_or_default(),
                normalized_query: visit_query.as_deref().map(normalize_query).unwrap_or_default(),
                trail_id: None,
                visit_time_ms: visit.visit_time_ms,
            });
            if let Some(trail) = current.take() {
                trails.push(trail);
            }
            let query = visit_query.unwrap_or_else(|| "search".to_string());
            let trail_id = format!("trail:{}:{:04}", visit.profile_id, trails.len() + 1);
            visit.trail_id = Some(trail_id.clone());
            if let Some(last) = search_events.last_mut() {
                last.trail_id = Some(trail_id.clone());
            }
            current = Some(TrailRecord {
                trail_id: trail_id.clone(),
                profile_id: visit.profile_id.clone(),
                session_id: visit.session_id.clone().unwrap_or_default(),
                initial_query: query.clone(),
                search_engine: visit.search_engine.clone().unwrap_or_else(|| "unknown".to_string()),
                reformulation_count: 0,
                visit_count: 1,
                landing_url: None,
                landing_domain: None,
                first_visit_ms: visit.visit_time_ms,
                last_visit_ms: visit.visit_time_ms,
                max_depth: 0,
                queries: vec![query],
                members: vec![TrailMemberRecord {
                    trail_id,
                    profile_id: visit.profile_id.clone(),
                    visit_id: visit.visit_id,
                    ordinal: 0,
                    role: "search_event".to_string(),
                }],
            });
            continue;
        }

        let Some(trail) = current.as_mut() else {
            continue;
        };
        if trail.session_id != visit.session_id.clone().unwrap_or_default()
            || visit.visit_time_ms - trail.last_visit_ms > TRAIL_GAP_MS
        {
            let finished = current.take().expect("trail");
            trails.push(finished);
            continue;
        }

        let depth = trail.members.len() as i64;
        let role = if trail.landing_url.is_none() { "landing" } else { "click" };
        trail.visit_count += 1;
        trail.last_visit_ms = visit.visit_time_ms;
        trail.max_depth = trail.max_depth.max(depth);
        trail.landing_url.get_or_insert_with(|| visit.url.clone());
        trail.landing_domain.get_or_insert_with(|| visit.registrable_domain.clone());
        trail.members.push(TrailMemberRecord {
            trail_id: trail.trail_id.clone(),
            profile_id: trail.profile_id.clone(),
            visit_id: visit.visit_id,
            ordinal: depth,
            role: role.to_string(),
        });
        visit.trail_id = Some(trail.trail_id.clone());
    }
    if let Some(trail) = current.take() {
        trails.push(trail);
    }

    let mut trail_events = HashMap::<String, Vec<String>>::new();
    for event in &search_events {
        if let Some(trail_id) = &event.trail_id {
            trail_events.entry(trail_id.clone()).or_default().push(event.raw_query.clone());
        }
    }
    for trail in &mut trails {
        let queries = trail_events.get(&trail.trail_id).cloned().unwrap_or_default();
        let deduped = queries.into_iter().fold(Vec::<String>::new(), |mut acc, query| {
            if acc.last().is_none_or(|last| normalize_query(last) != normalize_query(&query)) {
                acc.push(query);
            }
            acc
        });
        if let Some(first) = deduped.first() {
            trail.initial_query = first.clone();
            trail.queries = deduped.clone();
            trail.reformulation_count = deduped.len().saturating_sub(1) as i64;
        }
    }

    (search_events, trails)
}

fn build_query_families(events: &[SearchEventRecord]) -> Vec<QueryFamilyRecord> {
    let mut families: Vec<QueryFamilyRecord> = Vec::new();
    for event in events {
        let tokens = query_token_set(&event.normalized_query);
        if tokens.is_empty() {
            continue;
        }
        let mut matched = None;
        for (index, family) in families.iter_mut().enumerate() {
            if family.profile_id != event.profile_id || family.search_engine != event.search_engine
            {
                continue;
            }
            let family_tokens = query_token_set(&normalize_query(&family.anchor_query));
            if jaccard(&tokens, &family_tokens) >= 0.5
                || tokens.is_subset(&family_tokens)
                || family_tokens.is_subset(&tokens)
            {
                family.member_count += 1;
                family.last_seen_ms = family.last_seen_ms.max(event.visit_time_ms);
                if !family
                    .queries
                    .iter()
                    .any(|query| normalize_query(query) == event.normalized_query)
                {
                    family.queries.push(event.raw_query.clone());
                }
                matched = Some(index);
                break;
            }
        }
        if matched.is_none() {
            families.push(QueryFamilyRecord {
                family_id: format!("family:{}:{:04}", event.profile_id, families.len() + 1),
                profile_id: event.profile_id.clone(),
                anchor_query: event.raw_query.clone(),
                member_count: 1,
                search_engine: event.search_engine.clone(),
                first_seen_ms: event.visit_time_ms,
                last_seen_ms: event.visit_time_ms,
                queries: vec![event.raw_query.clone()],
            });
        }
    }
    families
}

fn build_refind_pages(visits: &[VisitRecord]) -> Vec<RefindPageRecord> {
    let mut grouped = HashMap::<String, Vec<&VisitRecord>>::new();
    for visit in visits.iter().filter(|visit| !visit.is_search_event) {
        grouped.entry(visit.canonical_url.clone()).or_default().push(visit);
    }
    grouped
        .into_iter()
        .filter_map(|(canonical_url, members)| {
            let first = members.first()?;
            let distinct_days = members
                .iter()
                .map(|visit| local_date_key(visit.visit_time_ms))
                .collect::<HashSet<_>>();
            let trail_ids = members
                .iter()
                .filter_map(|visit| visit.trail_id.clone())
                .collect::<HashSet<_>>();
            let search_arrival_count = members.iter().filter(|visit| visit.trail_id.is_some()).count() as i64;
            let typed_revisit_count = members
                .iter()
                .filter(|visit| visit.from_visit.is_none() && !visit.is_search_event)
                .count() as i64;
            let cross_day_count = distinct_days.len() as i64;
            let trail_count = trail_ids.len() as i64;
            let score = (cross_day_count as f32 * 2.0)
                + (trail_count as f32 * 1.5)
                + search_arrival_count as f32
                + (typed_revisit_count as f32 * 1.2);
            if cross_day_count < 2 && trail_count < 2 && typed_revisit_count < 2 {
                return None;
            }
            let visit_ids = members.iter().map(|visit| visit.visit_id).collect::<Vec<_>>();
            let evidence_json = json!({
                "factors": [
                    { "signal": "cross_day_count", "rawValue": cross_day_count, "weight": 2.0, "contribution": cross_day_count as f32 * 2.0 },
                    { "signal": "trail_count", "rawValue": trail_count, "weight": 1.5, "contribution": trail_count as f32 * 1.5 },
                    { "signal": "search_arrival_count", "rawValue": search_arrival_count, "weight": 1.0, "contribution": search_arrival_count as f32 },
                    { "signal": "typed_revisit_count", "rawValue": typed_revisit_count, "weight": 1.2, "contribution": typed_revisit_count as f32 * 1.2 }
                ],
                "visitIds": visit_ids
            })
            .to_string();
            Some(RefindPageRecord {
                profile_id: first.profile_id.clone(),
                canonical_url,
                url: first.url.clone(),
                title: first.title.clone(),
                registrable_domain: first.registrable_domain.clone(),
                cross_day_count,
                trail_count,
                search_arrival_count,
                typed_revisit_count,
                refind_score: score,
                evidence_json,
                first_seen_ms: members.iter().map(|visit| visit.visit_time_ms).min().unwrap_or(first.visit_time_ms),
                last_seen_ms: members.iter().map(|visit| visit.visit_time_ms).max().unwrap_or(first.visit_time_ms),
            })
        })
        .collect()
}

fn build_source_effectiveness(
    trails: &[TrailRecord],
    refind_pages: &[RefindPageRecord],
) -> Vec<SourceEffectivenessRecord> {
    let mut landing_counts = HashMap::<String, i64>::new();
    let mut trail_counts = HashMap::<String, HashSet<String>>::new();
    let mut first_seen = HashMap::<String, i64>::new();
    let mut last_seen = HashMap::<String, i64>::new();
    let profile_id = trails
        .first()
        .map(|trail| trail.profile_id.clone())
        .or_else(|| refind_pages.first().map(|page| page.profile_id.clone()))
        .unwrap_or_default();

    for trail in trails {
        if let Some(domain) = &trail.landing_domain {
            *landing_counts.entry(domain.clone()).or_default() += 1;
        }
        let members =
            trail.members.iter().map(|member| member.trail_id.clone()).collect::<HashSet<_>>();
        if let Some(domain) = &trail.landing_domain {
            trail_counts.entry(domain.clone()).or_default().extend(members.clone());
        }
        first_seen
            .entry(trail.landing_domain.clone().unwrap_or_else(|| "unknown".to_string()))
            .and_modify(|value| *value = (*value).min(trail.first_visit_ms))
            .or_insert(trail.first_visit_ms);
        last_seen
            .entry(trail.landing_domain.clone().unwrap_or_else(|| "unknown".to_string()))
            .and_modify(|value| *value = (*value).max(trail.last_visit_ms))
            .or_insert(trail.last_visit_ms);
    }

    let reference_counts =
        refind_pages.iter().fold(HashMap::<String, i64>::new(), |mut acc, page| {
            *acc.entry(page.registrable_domain.clone()).or_default() += 1;
            acc
        });

    let domains =
        landing_counts.keys().chain(reference_counts.keys()).cloned().collect::<BTreeSet<_>>();

    domains
        .into_iter()
        .map(|domain| {
            let stable_landing_count = *landing_counts.get(&domain).unwrap_or(&0);
            let reference_count = *reference_counts.get(&domain).unwrap_or(&0);
            let trail_count =
                trail_counts.get(&domain).map(|value| value.len()).unwrap_or(0) as i64;
            let source_role = if reference_count >= stable_landing_count && reference_count > 0 {
                "reference"
            } else if stable_landing_count > 0 {
                "landing"
            } else {
                "entry"
            };
            let effectiveness_score = (stable_landing_count as f32 * 2.0)
                + (reference_count as f32 * 1.5)
                + (trail_count as f32 * 0.5);
            let evidence_json = json!({
                "stableLandingCount": stable_landing_count,
                "referenceCount": reference_count,
                "trailCount": trail_count
            })
            .to_string();
            SourceEffectivenessRecord {
                profile_id: profile_id.clone(),
                registrable_domain: domain.clone(),
                source_role: source_role.to_string(),
                trail_count,
                stable_landing_count,
                effectiveness_score,
                evidence_json,
                first_seen_ms: *first_seen.get(&domain).unwrap_or(&0),
                last_seen_ms: *last_seen.get(&domain).unwrap_or(&0),
            }
        })
        .collect()
}

fn build_reopened_investigations(
    query_families: &[QueryFamilyRecord],
    refind_pages: &[RefindPageRecord],
) -> Vec<ReopenedInvestigationRecord> {
    let mut records = Vec::new();
    for family in query_families {
        let distinct_days =
            family.queries.iter().map(|query| normalize_query(query)).collect::<HashSet<_>>().len()
                as i64;
        if distinct_days > 1 || family.member_count > 1 {
            records.push(ReopenedInvestigationRecord {
                investigation_id: format!("reopened:{}:{}", family.profile_id, family.family_id),
                profile_id: family.profile_id.clone(),
                anchor_type: "query_family".to_string(),
                anchor_id: family.family_id.clone(),
                anchor_label: family.anchor_query.clone(),
                occurrence_count: family.member_count,
                distinct_days,
                first_seen_ms: family.first_seen_ms,
                last_seen_ms: family.last_seen_ms,
                evidence_json: json!({ "queries": family.queries }).to_string(),
            });
        }
    }
    for page in refind_pages {
        if page.cross_day_count > 1 {
            records.push(ReopenedInvestigationRecord {
                investigation_id: format!("reopened:{}:{}", page.profile_id, page.canonical_url),
                profile_id: page.profile_id.clone(),
                anchor_type: "reference_page".to_string(),
                anchor_id: page.canonical_url.clone(),
                anchor_label: page.title.clone().unwrap_or_else(|| page.url.clone()),
                occurrence_count: page.cross_day_count,
                distinct_days: page.cross_day_count,
                first_seen_ms: page.first_seen_ms,
                last_seen_ms: page.last_seen_ms,
                evidence_json: page.evidence_json.clone(),
            });
        }
    }
    records
}

fn build_path_flows(visits: &[VisitRecord]) -> Vec<PathFlowRecord> {
    let mut flows = HashMap::<(String, String, i64), (i64, i64)>::new();
    let mut current_session = None::<String>;
    let mut current_profile = None::<String>;
    let mut current_sequence = Vec::<(String, i64)>::new();

    let mut flush_sequence = |profile_id: &str, sequence: &[(String, i64)]| {
        for step_count in [2_usize, 3_usize] {
            if sequence.len() < step_count {
                continue;
            }
            for window in sequence.windows(step_count) {
                let flow_pattern = window
                    .iter()
                    .map(|(domain, _)| domain.as_str())
                    .collect::<Vec<_>>()
                    .join(" → ");
                let last_seen_ms =
                    window.iter().map(|(_, visit_time_ms)| *visit_time_ms).max().unwrap_or(0);
                let key = (profile_id.to_string(), flow_pattern, step_count as i64);
                let entry = flows.entry(key).or_insert((0, 0));
                entry.0 += 1;
                entry.1 = entry.1.max(last_seen_ms);
            }
        }
    };

    for visit in visits {
        let session_id = visit
            .session_id
            .clone()
            .unwrap_or_else(|| format!("sessionless:{}:{}", visit.profile_id, visit.visit_id));
        if current_session.as_deref() != Some(session_id.as_str()) {
            if let (Some(profile_id), false) =
                (current_profile.as_deref(), current_sequence.is_empty())
            {
                flush_sequence(profile_id, &current_sequence);
            }
            current_session = Some(session_id);
            current_profile = Some(visit.profile_id.clone());
            current_sequence.clear();
        }
        if current_sequence
            .last()
            .is_none_or(|(last_domain, _)| *last_domain != visit.registrable_domain)
        {
            current_sequence.push((visit.registrable_domain.clone(), visit.visit_time_ms));
        } else if let Some((_, last_seen_ms)) = current_sequence.last_mut() {
            *last_seen_ms = visit.visit_time_ms;
        }
    }
    if let (Some(profile_id), false) = (current_profile.as_deref(), current_sequence.is_empty()) {
        flush_sequence(profile_id, &current_sequence);
    }

    flows
        .into_iter()
        .map(|((profile_id, flow_pattern, step_count), (occurrence_count, last_seen_ms))| {
            PathFlowRecord { profile_id, flow_pattern, step_count, occurrence_count, last_seen_ms }
        })
        .collect()
}

fn build_habit_patterns(visits: &[VisitRecord]) -> Vec<HabitPatternRecord> {
    let mut by_domain = HashMap::<String, BTreeSet<NaiveDate>>::new();
    let mut last_visit = HashMap::<String, i64>::new();
    for visit in visits {
        by_domain
            .entry(visit.registrable_domain.clone())
            .or_default()
            .insert(local_datetime_from_millis(visit.visit_time_ms).date_naive());
        last_visit
            .entry(visit.registrable_domain.clone())
            .and_modify(|value| *value = (*value).max(visit.visit_time_ms))
            .or_insert(visit.visit_time_ms);
    }
    by_domain
        .into_iter()
        .filter_map(|(domain, days)| {
            if days.len() < 5 {
                return None;
            }
            let parsed_days = days.into_iter().collect::<Vec<_>>();
            if (*parsed_days.last()? - *parsed_days.first()?).num_days() < 14 {
                return None;
            }
            let intervals = parsed_days
                .windows(2)
                .map(|window| (window[1] - window[0]).num_days() as f32)
                .collect::<Vec<_>>();
            if intervals.is_empty() {
                return None;
            }
            let mean = intervals.iter().sum::<f32>() / intervals.len() as f32;
            let variance = intervals.iter().map(|value| (*value - mean).powi(2)).sum::<f32>()
                / intervals.len() as f32;
            let std_dev = variance.sqrt();
            let cv = if mean == 0.0 { 0.0 } else { std_dev / mean };
            let habit_type = if mean < 2.0 && cv < 0.5 {
                Some("daily_habit")
            } else if (5.0..=10.0).contains(&mean) && cv < 0.6 {
                Some("weekly_habit")
            } else if mean > 10.0 && cv < 0.8 {
                Some("periodic_reference")
            } else {
                None
            }?;
            let last_visited_ms = *last_visit.get(&domain).unwrap_or(&0);
            let days_since_last =
                ((Utc::now().timestamp_millis() - last_visited_ms) as f32 / 86_400_000.0).max(0.0);
            Some(HabitPatternRecord {
                profile_id: visits.first()?.profile_id.clone(),
                registrable_domain: domain,
                habit_type: habit_type.to_string(),
                mean_interval_days: mean,
                cv,
                visit_count: parsed_days.len() as i64,
                last_visited_ms,
                is_interrupted: days_since_last > mean * 2.0,
            })
        })
        .collect()
}

fn build_daily_rollups(visits: &[VisitRecord]) -> DailyRollupBundle {
    let mut domains =
        HashMap::<(String, String, String), (String, i64, i64, i64, HashSet<String>)>::new();
    let mut categories = HashMap::<(String, String, String), (i64, HashSet<String>)>::new();
    let mut engines = HashMap::<(String, String, String), i64>::new();
    let mut summaries = HashMap::<
        (String, String),
        (i64, i64, HashSet<String>, HashSet<String>, HashMap<String, i64>),
    >::new();

    for visit in visits {
        let date_key = local_date_key(visit.visit_time_ms);
        let domain_key =
            (date_key.clone(), visit.profile_id.clone(), visit.registrable_domain.clone());
        let domain_entry = domains.entry(domain_key).or_insert((
            visit.domain_category.clone(),
            0,
            0,
            0,
            HashSet::new(),
        ));
        domain_entry.1 += 1;
        domain_entry.2 += i64::from(visit.is_search_event);
        domain_entry.3 += i64::from(visit.is_new_domain);
        domain_entry.4.insert(visit.canonical_url.clone());

        let category_key =
            (date_key.clone(), visit.profile_id.clone(), visit.domain_category.clone());
        let category_entry = categories.entry(category_key).or_insert((0, HashSet::new()));
        category_entry.0 += 1;
        category_entry.1.insert(visit.registrable_domain.clone());

        if let Some(engine) = &visit.search_engine {
            *engines
                .entry((date_key.clone(), visit.profile_id.clone(), engine.clone()))
                .or_default() += 1;
        }

        let summary_key = (date_key, visit.profile_id.clone());
        let summary_entry = summaries.entry(summary_key).or_insert((
            0,
            0,
            HashSet::new(),
            HashSet::new(),
            HashMap::new(),
        ));
        summary_entry.0 += 1;
        summary_entry.1 += i64::from(visit.is_search_event);
        if visit.is_new_domain {
            summary_entry.2.insert(visit.registrable_domain.clone());
        }
        summary_entry.3.insert(visit.registrable_domain.clone());
        *summary_entry.4.entry(visit.registrable_domain.clone()).or_default() += 1;
    }

    DailyRollupBundle {
        domain_rows: domains
            .into_iter()
            .map(
                |(
                    (date_key, profile_id, registrable_domain),
                    (domain_category, visit_count, search_count, new_domain_visits, unique_urls),
                )| {
                    (
                        date_key,
                        profile_id,
                        registrable_domain,
                        domain_category,
                        visit_count,
                        search_count,
                        new_domain_visits,
                        unique_urls.len() as i64,
                    )
                },
            )
            .collect(),
        category_rows: categories
            .into_iter()
            .map(|((date_key, profile_id, domain_category), (visit_count, unique_domains))| {
                (date_key, profile_id, domain_category, visit_count, unique_domains.len() as i64)
            })
            .collect(),
        engine_rows: engines
            .into_iter()
            .map(|((date_key, profile_id, search_engine), search_count)| {
                (date_key, profile_id, search_engine, search_count)
            })
            .collect(),
        summary_rows: summaries
            .into_iter()
            .map(
                |(
                    (date_key, profile_id),
                    (total_visits, total_searches, new_domains, unique_domains, domain_counts),
                )| {
                    let hhi = if total_visits == 0 {
                        0.0
                    } else {
                        domain_counts
                            .values()
                            .map(|count| {
                                let share = *count as f32 / total_visits as f32;
                                share * share
                            })
                            .sum::<f32>()
                    };
                    let discovery_rate = if total_visits == 0 {
                        0.0
                    } else {
                        new_domains.len() as f32 / total_visits as f32
                    };
                    (
                        date_key,
                        profile_id,
                        total_visits,
                        total_searches,
                        new_domains.len() as i64,
                        unique_domains.len() as i64,
                        hhi,
                        discovery_rate,
                    )
                },
            )
            .collect(),
    }
}

fn merge_rollups(target: &mut DailyRollupBundle, next: DailyRollupBundle) {
    target.domain_rows.extend(next.domain_rows);
    target.category_rows.extend(next.category_rows);
    target.engine_rows.extend(next.engine_rows);
    target.summary_rows.extend(next.summary_rows);
}

#[allow(clippy::too_many_arguments)]
fn persist_core_state_for_job_kind(
    connection: &Connection,
    profile_id: Option<&str>,
    job_kind: CoreIntelligenceJobKind,
    computed_at: &str,
    visits: &[VisitRecord],
    rollups: &DailyRollupBundle,
    sessions: &[SessionRecord],
    search_events: &[SearchEventRecord],
    trails: &[TrailRecord],
    query_families: &[QueryFamilyRecord],
    refind_pages: &[RefindPageRecord],
    source_effectiveness: &[SourceEffectivenessRecord],
    habits: &[HabitPatternRecord],
    reopened: &[ReopenedInvestigationRecord],
    path_flows: &[PathFlowRecord],
) -> Result<()> {
    let tx = connection.unchecked_transaction()?;
    clear_core_tables_for_job_kind(&tx, profile_id, job_kind)?;

    for visit in visits {
        tx.execute(
            "INSERT INTO visit_derived_facts (
               visit_id, profile_id, session_id, trail_id, registrable_domain, canonical_url,
               domain_category, page_category, search_engine, search_query, is_new_domain,
               is_search_event, evidence_tier, taxonomy_source, taxonomy_pack, taxonomy_version,
               computed_at
             )
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17)",
            params![
                visit.visit_id,
                visit.profile_id,
                visit.session_id,
                visit.trail_id,
                visit.registrable_domain,
                visit.canonical_url,
                visit.domain_category,
                visit.page_category,
                visit.search_engine,
                visit.search_query,
                i64::from(visit.is_new_domain),
                i64::from(visit.is_search_event),
                visit.evidence_tier,
                visit.taxonomy_source,
                visit.taxonomy_pack,
                visit.taxonomy_version,
                computed_at,
            ],
        )?;
    }

    for (
        date_key,
        profile_id,
        registrable_domain,
        domain_category,
        visit_count,
        search_count,
        new_domain_visits,
        unique_urls,
    ) in &rollups.domain_rows
    {
        tx.execute(
            "INSERT INTO domain_daily_rollups
             (date_key, profile_id, registrable_domain, domain_category, visit_count, search_count, new_domain_visits, unique_urls)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![date_key, profile_id, registrable_domain, domain_category, visit_count, search_count, new_domain_visits, unique_urls],
        )?;
    }
    for (date_key, profile_id, domain_category, visit_count, unique_domains) in
        &rollups.category_rows
    {
        tx.execute(
            "INSERT INTO category_daily_rollups
             (date_key, profile_id, domain_category, visit_count, unique_domains)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![date_key, profile_id, domain_category, visit_count, unique_domains],
        )?;
    }
    for (date_key, profile_id, search_engine, search_count) in &rollups.engine_rows {
        tx.execute(
            "INSERT INTO engine_daily_rollups (date_key, profile_id, search_engine, search_count)
             VALUES (?1, ?2, ?3, ?4)",
            params![date_key, profile_id, search_engine, search_count],
        )?;
    }
    for (
        date_key,
        profile_id,
        total_visits,
        total_searches,
        new_domains,
        unique_domains,
        hhi_score,
        discovery_rate,
    ) in &rollups.summary_rows
    {
        tx.execute(
            "INSERT INTO daily_summary_rollups
             (date_key, profile_id, total_visits, total_searches, new_domains, unique_domains, hhi_score, discovery_rate)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![date_key, profile_id, total_visits, total_searches, new_domains, unique_domains, hhi_score, discovery_rate],
        )?;
    }

    for session in sessions {
        tx.execute(
            "INSERT INTO sessions
             (session_id, profile_id, first_visit_ms, last_visit_ms, visit_count, search_count, domain_count, is_deep_dive, auto_title, computed_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                session.session_id,
                session.profile_id,
                session.first_visit_ms,
                session.last_visit_ms,
                session.visit_count,
                session.search_count,
                session.domain_count,
                i64::from(session.is_deep_dive),
                session.auto_title,
                computed_at,
            ],
        )?;
    }

    for trail in trails {
        tx.execute(
            "INSERT INTO search_trails
             (trail_id, profile_id, session_id, initial_query, search_engine, reformulation_count, visit_count,
              landing_url, landing_domain, first_visit_ms, last_visit_ms, max_depth, queries_json, computed_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
            params![
                trail.trail_id,
                trail.profile_id,
                trail.session_id,
                trail.initial_query,
                trail.search_engine,
                trail.reformulation_count,
                trail.visit_count,
                trail.landing_url,
                trail.landing_domain,
                trail.first_visit_ms,
                trail.last_visit_ms,
                trail.max_depth,
                serde_json::to_string(&trail.queries)?,
                computed_at,
            ],
        )?;
        for member in &trail.members {
            tx.execute(
                "INSERT INTO search_trail_members (trail_id, profile_id, visit_id, ordinal, role)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    member.trail_id,
                    member.profile_id,
                    member.visit_id,
                    member.ordinal,
                    member.role
                ],
            )?;
        }
    }

    for event in search_events {
        tx.execute(
            "INSERT INTO search_events
             (visit_id, profile_id, search_engine, raw_query, normalized_query, trail_id, computed_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                event.visit_id,
                event.profile_id,
                event.search_engine,
                event.raw_query,
                event.normalized_query,
                event.trail_id,
                computed_at,
            ],
        )?;
        for term in tokenize_query_terms(&event.normalized_query) {
            tx.execute(
                "INSERT INTO search_event_terms (visit_id, profile_id, term) VALUES (?1, ?2, ?3)",
                params![event.visit_id, event.profile_id, term],
            )?;
        }
    }

    for family in query_families {
        tx.execute(
            "INSERT INTO query_families
             (family_id, profile_id, anchor_query, member_count, search_engine, first_seen_ms, last_seen_ms, queries_json, computed_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                family.family_id,
                family.profile_id,
                family.anchor_query,
                family.member_count,
                family.search_engine,
                family.first_seen_ms,
                family.last_seen_ms,
                serde_json::to_string(&family.queries)?,
                computed_at,
            ],
        )?;
    }

    for page in refind_pages {
        tx.execute(
            "INSERT INTO refind_pages
             (profile_id, canonical_url, url, title, registrable_domain, cross_day_count, trail_count, search_arrival_count,
              typed_revisit_count, refind_score, evidence_json, first_seen_ms, last_seen_ms, computed_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
            params![
                page.profile_id,
                page.canonical_url,
                page.url,
                page.title,
                page.registrable_domain,
                page.cross_day_count,
                page.trail_count,
                page.search_arrival_count,
                page.typed_revisit_count,
                page.refind_score,
                page.evidence_json,
                page.first_seen_ms,
                page.last_seen_ms,
                computed_at,
            ],
        )?;
    }

    for source in source_effectiveness {
        tx.execute(
            "INSERT INTO source_effectiveness
             (profile_id, registrable_domain, source_role, trail_count, stable_landing_count, effectiveness_score,
              evidence_json, first_seen_ms, last_seen_ms, computed_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                source.profile_id,
                source.registrable_domain,
                source.source_role,
                source.trail_count,
                source.stable_landing_count,
                source.effectiveness_score,
                source.evidence_json,
                source.first_seen_ms,
                source.last_seen_ms,
                computed_at,
            ],
        )?;
    }

    for habit in habits {
        tx.execute(
            "INSERT INTO habit_patterns
             (profile_id, registrable_domain, habit_type, mean_interval_days, cv, visit_count, last_visited_ms, is_interrupted, computed_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                habit.profile_id,
                habit.registrable_domain,
                habit.habit_type,
                habit.mean_interval_days,
                habit.cv,
                habit.visit_count,
                habit.last_visited_ms,
                i64::from(habit.is_interrupted),
                computed_at,
            ],
        )?;
    }

    for record in reopened {
        tx.execute(
            "INSERT INTO reopened_investigations
             (investigation_id, profile_id, anchor_type, anchor_id, anchor_label, occurrence_count, distinct_days,
              first_seen_ms, last_seen_ms, evidence_json, computed_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                record.investigation_id,
                record.profile_id,
                record.anchor_type,
                record.anchor_id,
                record.anchor_label,
                record.occurrence_count,
                record.distinct_days,
                record.first_seen_ms,
                record.last_seen_ms,
                record.evidence_json,
                computed_at,
            ],
        )?;
    }

    for flow in path_flows {
        tx.execute(
            "INSERT INTO path_flows
             (profile_id, flow_pattern, step_count, occurrence_count, last_seen_ms)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                flow.profile_id,
                flow.flow_pattern,
                flow.step_count,
                flow.occurrence_count,
                flow.last_seen_ms
            ],
        )?;
    }

    tx.commit()?;
    Ok(())
}

fn clear_core_tables(connection: &Connection, profile_id: Option<&str>) -> Result<()> {
    clear_core_tables_for_job_kind(connection, profile_id, CoreIntelligenceJobKind::FullRebuild)
}

fn clear_core_tables_for_job_kind(
    connection: &Connection,
    profile_id: Option<&str>,
    job_kind: CoreIntelligenceJobKind,
) -> Result<()> {
    let tables: &[&str] = match job_kind {
        CoreIntelligenceJobKind::VisitDerive => &["visit_derived_facts"],
        CoreIntelligenceJobKind::DailyRollup => &[
            "domain_daily_rollups",
            "category_daily_rollups",
            "engine_daily_rollups",
            "daily_summary_rollups",
        ],
        CoreIntelligenceJobKind::StructuralRebuild => &[
            "sessions",
            "search_trails",
            "search_trail_members",
            "search_events",
            "search_event_terms",
            "query_families",
            "refind_pages",
            "source_effectiveness",
            "habit_patterns",
            "reopened_investigations",
            "path_flows",
        ],
        CoreIntelligenceJobKind::FullRebuild => &[
            "visit_derived_facts",
            "domain_daily_rollups",
            "category_daily_rollups",
            "engine_daily_rollups",
            "daily_summary_rollups",
            "sessions",
            "search_trails",
            "search_trail_members",
            "search_events",
            "search_event_terms",
            "query_families",
            "refind_pages",
            "source_effectiveness",
            "habit_patterns",
            "reopened_investigations",
            "path_flows",
        ],
    };

    if let Some(profile_id) = profile_id {
        for table in tables {
            if *table == "search_trail_members" {
                connection.execute(
                    "DELETE FROM search_trail_members WHERE profile_id = ?1",
                    [profile_id],
                )?;
            } else if *table == "search_event_terms" {
                connection.execute(
                    "DELETE FROM search_event_terms WHERE profile_id = ?1",
                    [profile_id],
                )?;
            } else {
                connection
                    .execute(&format!("DELETE FROM {table} WHERE profile_id = ?1"), [profile_id])?;
            }
        }
    } else {
        for table in tables {
            connection.execute(&format!("DELETE FROM {table}"), [])?;
        }
    }
    Ok(())
}

fn drop_legacy_insight_tables(connection: &Connection) -> Result<()> {
    for table in LEGACY_INSIGHT_TABLES {
        if table_exists(connection, table)? {
            connection.execute(&format!("DROP TABLE IF EXISTS {table}"), [])?;
        }
    }
    Ok(())
}

fn table_exists(connection: &Connection, table: &str) -> Result<bool> {
    connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1)",
            [table],
            |row| row.get::<_, i64>(0),
        )
        .map(|value| value != 0)
        .map_err(Into::into)
}

fn table_row_count(connection: &Connection, table: &str) -> Result<usize> {
    if !table_exists(connection, table)? {
        return Ok(0);
    }
    connection
        .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| row.get::<_, i64>(0))
        .map(|value| value.max(0) as usize)
        .map_err(Into::into)
}

pub(super) fn local_date_key(visit_time_ms: i64) -> String {
    local_datetime_from_millis(visit_time_ms).format("%Y-%m-%d").to_string()
}

pub(super) fn rfc3339_from_millis(visit_time_ms: i64) -> String {
    local_datetime_from_millis(visit_time_ms).with_timezone(&Utc).to_rfc3339()
}

fn local_datetime_from_millis(visit_time_ms: i64) -> chrono::DateTime<Local> {
    match Local.timestamp_millis_opt(visit_time_ms) {
        LocalResult::Single(value) => value,
        _ => Local::now(),
    }
}

pub(super) fn date_range_bounds(range: &DateRange) -> Result<(i64, i64)> {
    let start = NaiveDate::parse_from_str(&range.start, "%Y-%m-%d")
        .with_context(|| format!("parsing start date {}", range.start))?;
    let end = NaiveDate::parse_from_str(&range.end, "%Y-%m-%d")
        .with_context(|| format!("parsing end date {}", range.end))?;
    let start_dt = resolve_local_date(start)?;
    let end_dt = resolve_local_date(end.succ_opt().unwrap_or(end))?;
    Ok((start_dt.timestamp_millis(), end_dt.timestamp_millis()))
}

fn resolve_local_date(date: NaiveDate) -> Result<chrono::DateTime<Local>> {
    let naive = date.and_hms_opt(0, 0, 0).expect("midnight");
    match Local.from_local_datetime(&naive) {
        LocalResult::Single(value) => Ok(value),
        LocalResult::Ambiguous(first, _) => Ok(first),
        LocalResult::None => anyhow::bail!("could not resolve local midnight for {}", date),
    }
}

fn tokenize_query_terms(query: &str) -> Vec<String> {
    let stop_words = ["the", "and", "for", "with", "from", "what", "when", "where", "how"];
    query
        .split(|ch: char| !ch.is_alphanumeric() && !is_cjk_like(ch))
        .filter_map(|token| {
            let token = token.trim().to_lowercase();
            if token.is_empty() || stop_words.contains(&token.as_str()) {
                None
            } else {
                Some(token)
            }
        })
        .collect()
}

fn query_token_set(query: &str) -> HashSet<String> {
    tokenize_query_terms(query).into_iter().collect()
}

fn jaccard(left: &HashSet<String>, right: &HashSet<String>) -> f32 {
    if left.is_empty() || right.is_empty() {
        return 0.0;
    }
    let intersection = left.intersection(right).count() as f32;
    let union = left.union(right).count() as f32;
    if union == 0.0 { 0.0 } else { intersection / union }
}

fn is_cjk_like(ch: char) -> bool {
    matches!(
        ch as u32,
        0x3400..=0x4DBF | 0x4E00..=0x9FFF | 0x3040..=0x30FF | 0xAC00..=0xD7AF
    )
}

pub fn get_sessions(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    request: &PagedDateRangeRequest,
) -> Result<SessionListResult> {
    let connection = open_intelligence_connection(paths, config, key)?;
    ensure_core_intelligence_schema(&connection)?;
    let (start_ms, end_ms) = date_range_bounds(&request.date_range)?;
    let total: i64 = connection.query_row(
        "SELECT COUNT(*)
         FROM sessions
         WHERE (?1 IS NULL OR profile_id = ?1)
           AND last_visit_ms >= ?2
           AND first_visit_ms < ?3",
        params![request.profile_id.as_deref(), start_ms, end_ms],
        |row| row.get(0),
    )?;
    let offset = request.page.saturating_mul(request.page_size.max(1)) as i64;
    let mut statement = connection.prepare(
        "SELECT session_id, first_visit_ms, last_visit_ms, visit_count, search_count, domain_count, is_deep_dive, auto_title
         FROM sessions
         WHERE (?1 IS NULL OR profile_id = ?1)
           AND last_visit_ms >= ?2
           AND first_visit_ms < ?3
         ORDER BY first_visit_ms DESC, session_id DESC
         LIMIT ?4 OFFSET ?5",
    )?;
    let sessions = statement
        .query_map(
            params![
                request.profile_id.as_deref(),
                start_ms,
                end_ms,
                request.page_size.max(1) as i64,
                offset
            ],
            |row| {
                Ok(SessionSummary {
                    session_id: row.get(0)?,
                    first_visit_ms: row.get(1)?,
                    last_visit_ms: row.get(2)?,
                    visit_count: row.get(3)?,
                    search_count: row.get(4)?,
                    domain_count: row.get(5)?,
                    is_deep_dive: row.get::<_, i64>(6)? != 0,
                    auto_title: row.get(7)?,
                })
            },
        )?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(SessionListResult { sessions, total, page: request.page, page_size: request.page_size })
}

pub fn get_session_detail(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    session_id: &str,
) -> Result<SessionDetail> {
    let connection = open_intelligence_connection(paths, config, key)?;
    ensure_core_intelligence_schema(&connection)?;
    let session = connection
        .query_row(
            "SELECT session_id, first_visit_ms, last_visit_ms, visit_count, search_count, domain_count, is_deep_dive, auto_title
             FROM sessions WHERE session_id = ?1",
            [session_id],
            |row| {
                Ok(SessionSummary {
                    session_id: row.get(0)?,
                    first_visit_ms: row.get(1)?,
                    last_visit_ms: row.get(2)?,
                    visit_count: row.get(3)?,
                    search_count: row.get(4)?,
                    domain_count: row.get(5)?,
                    is_deep_dive: row.get::<_, i64>(6)? != 0,
                    auto_title: row.get(7)?,
                })
            },
        )
        .optional()?
        .with_context(|| format!("session {session_id} was not found"))?;
    let visits = load_session_visits(&connection, session_id)?;
    let trails = load_session_trails(&connection, session_id)?;
    Ok(SessionDetail { session, visits, trails })
}

pub fn get_search_trails(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    request: &SearchTrailQueryRequest,
) -> Result<TrailListResult> {
    let connection = open_intelligence_connection(paths, config, key)?;
    ensure_core_intelligence_schema(&connection)?;
    let (start_ms, end_ms) = date_range_bounds(&request.date_range)?;
    let total: i64 = connection.query_row(
        "SELECT COUNT(*)
         FROM search_trails
         WHERE (?1 IS NULL OR profile_id = ?1)
           AND (?2 IS NULL OR search_engine = ?2)
           AND last_visit_ms >= ?3
           AND first_visit_ms < ?4",
        params![request.profile_id.as_deref(), request.engine.as_deref(), start_ms, end_ms],
        |row| row.get(0),
    )?;
    let offset = request.page.saturating_mul(request.page_size.max(1)) as i64;
    let mut statement = connection.prepare(
        "SELECT trail_id, session_id, initial_query, search_engine, reformulation_count, visit_count,
                landing_url, landing_domain, first_visit_ms, last_visit_ms, max_depth, queries_json
         FROM search_trails
         WHERE (?1 IS NULL OR profile_id = ?1)
           AND (?2 IS NULL OR search_engine = ?2)
           AND last_visit_ms >= ?3
           AND first_visit_ms < ?4
         ORDER BY first_visit_ms DESC, trail_id DESC
         LIMIT ?5 OFFSET ?6",
    )?;
    let trails = statement
        .query_map(
            params![
                request.profile_id.as_deref(),
                request.engine.as_deref(),
                start_ms,
                end_ms,
                request.page_size.max(1) as i64,
                offset
            ],
            trail_summary_from_row,
        )?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(TrailListResult { trails, total, page: request.page, page_size: request.page_size })
}

pub fn get_trail_detail(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    trail_id: &str,
) -> Result<TrailDetail> {
    let connection = open_intelligence_connection(paths, config, key)?;
    ensure_core_intelligence_schema(&connection)?;
    let trail = connection
        .query_row(
            "SELECT trail_id, session_id, initial_query, search_engine, reformulation_count, visit_count,
                    landing_url, landing_domain, first_visit_ms, last_visit_ms, max_depth, queries_json
             FROM search_trails
             WHERE trail_id = ?1",
            [trail_id],
            trail_summary_from_row,
        )
        .optional()?
        .with_context(|| format!("trail {trail_id} was not found"))?;
    let mut statement = connection.prepare(
        "SELECT search_trail_members.trail_id, search_trail_members.visit_id, search_trail_members.ordinal,
                search_trail_members.role, urls.url, urls.title, visits.visit_time_ms,
                visit_derived_facts.search_query
         FROM search_trail_members
         JOIN archive.visits AS visits ON visits.id = search_trail_members.visit_id
         JOIN archive.urls AS urls ON urls.id = visits.url_id
         LEFT JOIN visit_derived_facts ON visit_derived_facts.visit_id = visits.id
         WHERE search_trail_members.trail_id = ?1
         ORDER BY search_trail_members.ordinal ASC",
    )?;
    let members = statement
        .query_map([trail_id], |row| {
            Ok(TrailMember {
                trail_id: row.get(0)?,
                visit_id: row.get(1)?,
                ordinal: row.get(2)?,
                role: row.get(3)?,
                url: row.get(4)?,
                title: row.get(5)?,
                visit_time_ms: row.get(6)?,
                search_query: row.get(7)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(TrailDetail { trail, members })
}

pub fn get_navigation_path(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    visit_id: i64,
) -> Result<NavigationPath> {
    let connection = open_intelligence_connection(paths, config, key)?;
    ensure_core_intelligence_schema(&connection)?;
    let mut steps = Vec::<NavigationPathStep>::new();
    let mut current = load_navigation_visit(&connection, visit_id)?
        .with_context(|| format!("visit {visit_id} was not found"))?;
    let source_profile_id = current.source_profile_id;
    steps.push(NavigationPathStep {
        visit_id: current.visit_id,
        url: current.url.clone(),
        title: current.title.clone(),
        visit_time_ms: current.visit_time_ms,
        depth: 0,
    });
    let mut depth = 1_i64;
    let mut seen = HashSet::<i64>::from([current.visit_id]);
    while let Some(parent_source_visit_id) = current.from_visit {
        let Some(parent) = load_navigation_visit_by_source(
            &connection,
            source_profile_id,
            parent_source_visit_id,
        )?
        else {
            break;
        };
        if !seen.insert(parent.visit_id) {
            break;
        }
        steps.push(NavigationPathStep {
            visit_id: parent.visit_id,
            url: parent.url.clone(),
            title: parent.title.clone(),
            visit_time_ms: parent.visit_time_ms,
            depth,
        });
        current = parent;
        depth += 1;
    }
    steps.reverse();
    for (index, step) in steps.iter_mut().enumerate() {
        step.depth = index as i64;
    }
    Ok(NavigationPath { target_visit_id: visit_id, steps })
}

pub fn get_hub_pages(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    request: &TopSitesRequest,
) -> Result<Vec<HubPage>> {
    let connection = open_intelligence_connection(paths, config, key)?;
    ensure_core_intelligence_schema(&connection)?;
    let (start_ms, end_ms) = date_range_bounds(&request.date_range)?;
    let mut statement = connection.prepare(
        "SELECT visit_derived_facts.canonical_url, MAX(urls.title), visit_derived_facts.registrable_domain,
                COUNT(DISTINCT search_trail_members.trail_id)
         FROM search_trail_members
         JOIN archive.visits AS visits ON visits.id = search_trail_members.visit_id
         JOIN archive.urls AS urls ON urls.id = visits.url_id
         JOIN visit_derived_facts ON visit_derived_facts.visit_id = visits.id
         WHERE (?1 IS NULL OR visit_derived_facts.profile_id = ?1)
           AND visits.visit_time_ms >= ?2
           AND visits.visit_time_ms < ?3
         GROUP BY visit_derived_facts.canonical_url, visit_derived_facts.registrable_domain
         ORDER BY COUNT(DISTINCT search_trail_members.trail_id) DESC, visit_derived_facts.canonical_url ASC
         LIMIT ?4",
    )?;
    statement
        .query_map(
            params![
                request.profile_id.as_deref(),
                start_ms,
                end_ms,
                request.limit.unwrap_or(10).max(1) as i64
            ],
            |row| {
                Ok(HubPage {
                    url: row.get(0)?,
                    title: row.get(1)?,
                    registrable_domain: row.get(2)?,
                    trail_reference_count: row.get(3)?,
                })
            },
        )?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(Into::into)
}

pub fn get_search_engine_ranking(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    request: &ScopedDateRangeRequest,
) -> Result<Vec<EngineRanking>> {
    let connection = open_intelligence_connection(paths, config, key)?;
    ensure_core_intelligence_schema(&connection)?;
    let mut statement = connection.prepare(
        "SELECT search_engine, SUM(search_count)
         FROM engine_daily_rollups
         WHERE (?1 IS NULL OR profile_id = ?1)
           AND date_key >= ?2
           AND date_key <= ?3
         GROUP BY search_engine
         ORDER BY SUM(search_count) DESC, search_engine ASC",
    )?;
    statement
        .query_map(
            params![
                request.profile_id.as_deref(),
                request.date_range.start,
                request.date_range.end
            ],
            |row| {
                let engine: String = row.get(0)?;
                Ok(EngineRanking {
                    display_name: display_name_for_search_engine(&engine),
                    search_engine: engine,
                    search_count: row.get(1)?,
                })
            },
        )?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(Into::into)
}

pub fn get_top_search_concepts(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    request: &TopSearchConceptsRequest,
) -> Result<Vec<SearchConcept>> {
    let connection = open_intelligence_connection(paths, config, key)?;
    ensure_core_intelligence_schema(&connection)?;
    let (start_ms, end_ms) = date_range_bounds(&request.date_range)?;
    let mut statement = connection.prepare(
        "SELECT search_event_terms.term,
                COUNT(*) AS frequency,
                GROUP_CONCAT(DISTINCT search_events.search_engine)
         FROM search_event_terms
         JOIN search_events ON search_events.visit_id = search_event_terms.visit_id
         JOIN archive.visits AS visits ON visits.id = search_events.visit_id
         WHERE (?1 IS NULL OR search_events.profile_id = ?1)
           AND visits.visit_time_ms >= ?2
           AND visits.visit_time_ms < ?3
         GROUP BY search_event_terms.term
         ORDER BY frequency DESC, search_event_terms.term ASC
         LIMIT ?4",
    )?;
    statement
        .query_map(
            params![
                request.profile_id.as_deref(),
                start_ms,
                end_ms,
                request.limit.unwrap_or(20).max(1) as i64
            ],
            |row| {
                let engines = row
                    .get::<_, Option<String>>(2)?
                    .unwrap_or_default()
                    .split(',')
                    .filter(|value| !value.is_empty())
                    .map(|value| value.to_string())
                    .collect::<Vec<_>>();
                Ok(SearchConcept { term: row.get(0)?, frequency: row.get(1)?, engines })
            },
        )?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(Into::into)
}

pub fn get_query_families(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    request: &PagedDateRangeRequest,
) -> Result<QueryFamilyResult> {
    let connection = open_intelligence_connection(paths, config, key)?;
    ensure_core_intelligence_schema(&connection)?;
    let (start_ms, end_ms) = date_range_bounds(&request.date_range)?;
    let total: i64 = connection.query_row(
        "SELECT COUNT(*)
         FROM query_families
         WHERE (?1 IS NULL OR profile_id = ?1)
           AND last_seen_ms >= ?2
           AND first_seen_ms < ?3",
        params![request.profile_id.as_deref(), start_ms, end_ms],
        |row| row.get(0),
    )?;
    let offset = request.page.saturating_mul(request.page_size.max(1)) as i64;
    let mut statement = connection.prepare(
        "SELECT family_id, anchor_query, member_count, search_engine, first_seen_ms, last_seen_ms, queries_json
         FROM query_families
         WHERE (?1 IS NULL OR profile_id = ?1)
           AND last_seen_ms >= ?2
           AND first_seen_ms < ?3
         ORDER BY last_seen_ms DESC, family_id DESC
         LIMIT ?4 OFFSET ?5",
    )?;
    let families = statement
        .query_map(
            params![
                request.profile_id.as_deref(),
                start_ms,
                end_ms,
                request.page_size.max(1) as i64,
                offset
            ],
            |row| {
                Ok(QueryFamily {
                    family_id: row.get(0)?,
                    anchor_query: row.get(1)?,
                    member_count: row.get(2)?,
                    search_engine: row.get(3)?,
                    first_seen_at: rfc3339_from_millis(row.get(4)?),
                    last_seen_at: rfc3339_from_millis(row.get(5)?),
                    queries: serde_json::from_str(&row.get::<_, String>(6)?).unwrap_or_default(),
                })
            },
        )?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(QueryFamilyResult { families, total, page: request.page, page_size: request.page_size })
}

pub fn get_top_sites(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    request: &TopSitesRequest,
) -> Result<Vec<TopSite>> {
    let connection = open_intelligence_connection(paths, config, key)?;
    ensure_core_intelligence_schema(&connection)?;
    let order_clause = match request.sort_by.as_deref() {
        Some("unique_days") => {
            "unique_days DESC, average_daily_visits DESC, registrable_domain ASC"
        }
        Some("average_daily_visits") => {
            "average_daily_visits DESC, visit_count DESC, registrable_domain ASC"
        }
        _ => "visit_count DESC, unique_days DESC, registrable_domain ASC",
    };
    let sql = format!(
        "SELECT registrable_domain,
                MIN(domain_category),
                SUM(visit_count) AS visit_count,
                COUNT(DISTINCT date_key) AS unique_days,
                CAST(SUM(visit_count) AS REAL) / COUNT(DISTINCT date_key) AS average_daily_visits,
                SUM(unique_urls) AS unique_urls
         FROM domain_daily_rollups
         WHERE (?1 IS NULL OR profile_id = ?1)
           AND date_key >= ?2
           AND date_key <= ?3
         GROUP BY registrable_domain
         ORDER BY {order_clause}
         LIMIT ?4"
    );
    let mut statement = connection.prepare(&sql)?;
    statement
        .query_map(
            params![
                request.profile_id.as_deref(),
                request.date_range.start,
                request.date_range.end,
                request.limit.unwrap_or(20).max(1) as i64
            ],
            |row| {
                let domain: String = row.get(0)?;
                Ok(TopSite {
                    display_name: display_name_for_domain(&domain),
                    registrable_domain: domain,
                    domain_category: row.get(1)?,
                    visit_count: row.get(2)?,
                    unique_days: row.get(3)?,
                    average_daily_visits: row.get::<_, f32>(4)?,
                    unique_urls: row.get(5)?,
                })
            },
        )?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(Into::into)
}

pub fn get_domain_trend(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    request: &DomainTrendRequest,
) -> Result<DomainTrend> {
    let connection = open_intelligence_connection(paths, config, key)?;
    ensure_core_intelligence_schema(&connection)?;
    let mut statement = connection.prepare(
        "SELECT date_key, SUM(visit_count)
         FROM domain_daily_rollups
         WHERE registrable_domain = ?1
           AND date_key >= ?2
           AND date_key <= ?3
         GROUP BY date_key
         ORDER BY date_key ASC",
    )?;
    let points = statement
        .query_map(
            params![request.registrable_domain, request.date_range.start, request.date_range.end],
            |row| Ok(DomainTrendPoint { date_key: row.get(0)?, visit_count: row.get(1)? }),
        )?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(DomainTrend { registrable_domain: request.registrable_domain.clone(), points })
}

pub fn get_refind_pages(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    request: &RefindPagesRequest,
) -> Result<Vec<RefindPage>> {
    let connection = open_intelligence_connection(paths, config, key)?;
    ensure_core_intelligence_schema(&connection)?;
    let (start_ms, end_ms) = date_range_bounds(&request.date_range)?;
    let mut statement = connection.prepare(
        "SELECT canonical_url, url, title, registrable_domain, cross_day_count, trail_count,
                search_arrival_count, typed_revisit_count, refind_score, first_seen_ms, last_seen_ms
         FROM refind_pages
         WHERE (?1 IS NULL OR profile_id = ?1)
           AND last_seen_ms >= ?2
           AND first_seen_ms < ?3
         ORDER BY refind_score DESC, last_seen_ms DESC
         LIMIT ?4",
    )?;
    statement
        .query_map(
            params![
                request.profile_id.as_deref(),
                start_ms,
                end_ms,
                request.limit.unwrap_or(20).max(1) as i64
            ],
            |row| {
                Ok(RefindPage {
                    canonical_url: row.get(0)?,
                    url: row.get(1)?,
                    title: row.get(2)?,
                    registrable_domain: row.get(3)?,
                    cross_day_count: row.get(4)?,
                    trail_count: row.get(5)?,
                    search_arrival_count: row.get(6)?,
                    typed_revisit_count: row.get(7)?,
                    refind_score: row.get(8)?,
                    first_seen_at: rfc3339_from_millis(row.get(9)?),
                    last_seen_at: rfc3339_from_millis(row.get(10)?),
                })
            },
        )?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(Into::into)
}

pub fn explain_refind(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    request: &ExplainRefindRequest,
) -> Result<RefindExplanation> {
    let connection = open_intelligence_connection(paths, config, key)?;
    ensure_core_intelligence_schema(&connection)?;
    let (canonical_url, refind_score, evidence_json) = connection
        .query_row(
            "SELECT canonical_url, refind_score, evidence_json
             FROM refind_pages
             WHERE canonical_url = ?1
             ORDER BY refind_score DESC
             LIMIT 1",
            [request.canonical_url.as_str()],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, f32>(1)?, row.get::<_, String>(2)?)),
        )
        .optional()?
        .with_context(|| format!("refind page {} was not found", request.canonical_url))?;
    let evidence: serde_json::Value = serde_json::from_str(&evidence_json).unwrap_or_default();
    let factors = evidence
        .get("factors")
        .and_then(|value| value.as_array())
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .filter_map(|factor| {
            Some(RefindScoreFactor {
                signal: factor.get("signal")?.as_str()?.to_string(),
                raw_value: factor.get("rawValue")?.as_f64().unwrap_or_default() as f32,
                weight: factor.get("weight")?.as_f64().unwrap_or_default() as f32,
                contribution: factor.get("contribution")?.as_f64().unwrap_or_default() as f32,
            })
        })
        .collect::<Vec<_>>();
    let visit_ids = evidence
        .get("visitIds")
        .and_then(|value| value.as_array())
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .filter_map(|value| value.as_i64())
        .collect::<Vec<_>>();
    Ok(RefindExplanation { canonical_url, refind_score, factors, visit_ids })
}

pub fn get_activity_mix(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    request: &ScopedDateRangeRequest,
) -> Result<ActivityMix> {
    let connection = open_intelligence_connection(paths, config, key)?;
    ensure_core_intelligence_schema(&connection)?;
    let current =
        load_category_shares(&connection, &request.date_range, request.profile_id.as_deref())?;
    let previous = load_category_shares(
        &connection,
        &previous_date_range(&request.date_range)?,
        request.profile_id.as_deref(),
    )?;
    let current_map = current
        .iter()
        .map(|entry| (entry.domain_category.clone(), entry.clone()))
        .collect::<HashMap<_, _>>();
    let previous_map = previous
        .iter()
        .map(|entry| (entry.domain_category.clone(), entry.clone()))
        .collect::<HashMap<_, _>>();
    let categories = current;
    let change_vs_previous = categories
        .iter()
        .map(|entry| {
            let previous_share = previous_map
                .get(&entry.domain_category)
                .map(|value| value.share)
                .unwrap_or_default();
            CategoryChangeEntry {
                domain_category: entry.domain_category.clone(),
                current_share: entry.share,
                previous_share,
                change_points: entry.share - previous_share,
            }
        })
        .collect::<Vec<_>>();
    let _ = current_map;
    Ok(ActivityMix { categories, change_vs_previous })
}

pub fn get_activity_mix_trend(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    request: &GranularityDateRangeRequest,
) -> Result<ActivityMixTrend> {
    let connection = open_intelligence_connection(paths, config, key)?;
    ensure_core_intelligence_schema(&connection)?;
    let mut statement = connection.prepare(
        "SELECT date_key, domain_category, SUM(visit_count)
         FROM category_daily_rollups
         WHERE (?1 IS NULL OR profile_id = ?1)
           AND date_key >= ?2
           AND date_key <= ?3
         GROUP BY date_key, domain_category
         ORDER BY date_key ASC, SUM(visit_count) DESC",
    )?;
    let rows = statement
        .query_map(
            params![
                request.profile_id.as_deref(),
                request.date_range.start,
                request.date_range.end
            ],
            |row| {
                Ok((
                    collapse_date_key(&row.get::<_, String>(0)?, &request.granularity),
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                ))
            },
        )?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let mut grouped = BTreeMap::<String, Vec<(String, i64)>>::new();
    for (date_key, category, visit_count) in rows {
        grouped.entry(date_key).or_default().push((category, visit_count));
    }
    let points = grouped
        .into_iter()
        .map(|(date_key, categories)| {
            let total = categories.iter().map(|(_, count)| *count).sum::<i64>().max(1) as f32;
            ActivityMixTrendPoint {
                date_key,
                categories: categories
                    .into_iter()
                    .map(|(domain_category, visit_count)| CategoryMixEntry {
                        share: visit_count as f32 / total,
                        domain_category,
                        visit_count,
                    })
                    .collect(),
            }
        })
        .collect();
    Ok(ActivityMixTrend { points })
}

pub fn get_digest_summary(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    request: &ScopedDateRangeRequest,
) -> Result<DigestSummary> {
    let connection = open_intelligence_connection(paths, config, key)?;
    ensure_core_intelligence_schema(&connection)?;
    let current =
        load_summary_totals(&connection, &request.date_range, request.profile_id.as_deref())?;
    let previous_range = previous_date_range(&request.date_range)?;
    let previous =
        load_summary_totals(&connection, &previous_range, request.profile_id.as_deref())?;
    let current_deep =
        count_deep_dive_sessions(&connection, &request.date_range, request.profile_id.as_deref())?;
    let previous_deep =
        count_deep_dive_sessions(&connection, &previous_range, request.profile_id.as_deref())?;
    let current_refind = count_refind_pages_in_range(
        &connection,
        &request.date_range,
        request.profile_id.as_deref(),
    )?;
    let previous_refind =
        count_refind_pages_in_range(&connection, &previous_range, request.profile_id.as_deref())?;
    Ok(DigestSummary {
        date_range: request.date_range.clone(),
        total_visits: build_kpi(current.total_visits, previous.total_visits),
        total_searches: build_kpi(current.total_searches, previous.total_searches),
        new_domains: build_kpi(current.new_domains, previous.new_domains),
        deep_read_pages: build_kpi(current_deep, previous_deep),
        refind_pages: build_kpi(current_refind, previous_refind),
    })
}

pub fn get_stable_sources(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    request: &ScopedDateRangeRequest,
) -> Result<Vec<StableSource>> {
    let connection = open_intelligence_connection(paths, config, key)?;
    ensure_core_intelligence_schema(&connection)?;
    let (start_ms, end_ms) = date_range_bounds(&request.date_range)?;
    let mut statement = connection.prepare(
        "SELECT registrable_domain, source_role, trail_count, stable_landing_count, effectiveness_score
         FROM source_effectiveness
         WHERE (?1 IS NULL OR profile_id = ?1)
           AND last_seen_ms >= ?2
           AND first_seen_ms < ?3
         ORDER BY effectiveness_score DESC, registrable_domain ASC
         LIMIT ?4",
    )?;
    statement
        .query_map(params![request.profile_id.as_deref(), start_ms, end_ms, 10_i64], |row| {
            let domain: String = row.get(0)?;
            Ok(StableSource {
                display_name: display_name_for_domain(&domain),
                registrable_domain: domain,
                source_role: row.get(1)?,
                trail_count: row.get(2)?,
                stable_landing_count: row.get(3)?,
                effectiveness_score: row.get(4)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(Into::into)
}

pub fn get_search_effectiveness(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    request: &SearchEffectivenessRequest,
) -> Result<SearchEffectiveness> {
    let connection = open_intelligence_connection(paths, config, key)?;
    ensure_core_intelligence_schema(&connection)?;
    let (start_ms, end_ms) = date_range_bounds(&request.date_range)?;
    let mut statement = connection.prepare(
        "SELECT search_engine,
                AVG(reformulation_count),
                COUNT(*),
                AVG(max_depth)
         FROM search_trails
         WHERE (?1 IS NULL OR profile_id = ?1)
           AND (?2 IS NULL OR search_engine = ?2)
           AND last_visit_ms >= ?3
           AND first_visit_ms < ?4
         GROUP BY search_engine
         ORDER BY COUNT(*) DESC, search_engine ASC",
    )?;
    let engine_stats = statement
        .query_map(
            params![request.profile_id.as_deref(), request.engine.as_deref(), start_ms, end_ms],
            |row| {
                let engine: String = row.get(0)?;
                Ok(EngineEffectiveness {
                    display_name: display_name_for_search_engine(&engine),
                    search_engine: engine,
                    avg_reformulations: row.get::<_, f32>(1)?,
                    total_trails: row.get(2)?,
                    avg_depth: row.get::<_, f32>(3)?,
                })
            },
        )?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let top_resolving_sources = get_stable_sources(
        paths,
        config,
        key,
        &ScopedDateRangeRequest {
            date_range: request.date_range.clone(),
            profile_id: request.profile_id.clone(),
        },
    )?;
    let mut family_statement = connection.prepare(
        "SELECT anchor_query, member_count, first_seen_ms, last_seen_ms
         FROM query_families
         WHERE (?1 IS NULL OR profile_id = ?1)
           AND (?2 IS NULL OR search_engine = ?2)
           AND last_seen_ms >= ?3
           AND first_seen_ms < ?4
         ORDER BY member_count DESC, last_seen_ms DESC
         LIMIT 5",
    )?;
    let hardest_topics = family_statement
        .query_map(
            params![request.profile_id.as_deref(), request.engine.as_deref(), start_ms, end_ms],
            |row| {
                let first_seen: i64 = row.get(2)?;
                let last_seen: i64 = row.get(3)?;
                let lag_days = ((last_seen - first_seen) as f32 / 86_400_000.0).max(0.0);
                Ok(HardTopic {
                    query_family: row.get(0)?,
                    reformulation_count: row.get(1)?,
                    re_search_lag_days: lag_days,
                })
            },
        )?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(SearchEffectiveness { engine_stats, top_resolving_sources, hardest_topics })
}

pub fn get_friction_signals(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    request: &ScopedDateRangeRequest,
) -> Result<Vec<FrictionSignal>> {
    let connection = open_intelligence_connection(paths, config, key)?;
    ensure_core_intelligence_schema(&connection)?;
    let (start_ms, end_ms) = date_range_bounds(&request.date_range)?;
    let mut statement = connection.prepare(
        "SELECT initial_query, landing_domain, reformulation_count, visit_count
         FROM search_trails
         WHERE (?1 IS NULL OR profile_id = ?1)
           AND last_visit_ms >= ?2
           AND first_visit_ms < ?3
           AND (reformulation_count >= 2 OR (landing_domain IS NULL AND visit_count >= 3))
         ORDER BY reformulation_count DESC, visit_count DESC
         LIMIT ?4",
    )?;
    statement
        .query_map(params![request.profile_id.as_deref(), start_ms, end_ms, 10_i64], |row| {
            let landing_domain: Option<String> = row.get(1)?;
            let reformulation_count: i64 = row.get(2)?;
            let visit_count: i64 = row.get(3)?;
            let signal_kind =
                if reformulation_count >= 2 { "excessive_reformulation" } else { "bounce_pattern" };
            let description = if reformulation_count >= 2 {
                format!("Repeated search reformulation after query '{}'.", row.get::<_, String>(0)?)
            } else {
                "Search trail did not settle on a stable landing page.".to_string()
            };
            Ok(FrictionSignal {
                registrable_domain: landing_domain,
                url: None,
                evidence_type: "weak".to_string(),
                signal_kind: signal_kind.to_string(),
                occurrence_count: visit_count.max(reformulation_count),
                description,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(Into::into)
}

pub fn get_reopened_investigations(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    request: &ScopedDateRangeRequest,
) -> Result<Vec<ReopenedInvestigation>> {
    let connection = open_intelligence_connection(paths, config, key)?;
    ensure_core_intelligence_schema(&connection)?;
    let (start_ms, end_ms) = date_range_bounds(&request.date_range)?;
    let mut statement = connection.prepare(
        "SELECT investigation_id, anchor_type, anchor_id, anchor_label, occurrence_count, distinct_days,
                first_seen_ms, last_seen_ms
         FROM reopened_investigations
         WHERE (?1 IS NULL OR profile_id = ?1)
           AND last_seen_ms >= ?2
           AND first_seen_ms < ?3
         ORDER BY last_seen_ms DESC, occurrence_count DESC
         LIMIT ?4",
    )?;
    statement
        .query_map(params![request.profile_id.as_deref(), start_ms, end_ms, 10_i64], |row| {
            Ok(ReopenedInvestigation {
                investigation_id: row.get(0)?,
                anchor_type: row.get(1)?,
                anchor_id: row.get(2)?,
                anchor_label: row.get(3)?,
                occurrence_count: row.get(4)?,
                distinct_days: row.get(5)?,
                first_seen_at: rfc3339_from_millis(row.get(6)?),
                last_seen_at: rfc3339_from_millis(row.get(7)?),
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(Into::into)
}

pub fn get_domain_deep_dive(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    request: &DomainDeepDiveRequest,
) -> Result<DomainDeepDive> {
    let connection = open_intelligence_connection(paths, config, key)?;
    ensure_core_intelligence_schema(&connection)?;
    let (start_ms, end_ms) = date_range_bounds(&request.date_range)?;
    let visits = load_domain_visits(
        &connection,
        &request.registrable_domain,
        request.profile_id.as_deref(),
        start_ms,
        end_ms,
    )?;
    let total_visits = visits.len() as i64;
    let active_days = visits
        .iter()
        .map(|visit| local_date_key(visit.visit_time_ms))
        .collect::<HashSet<_>>()
        .len() as i64;
    let trail_count =
        visits.iter().filter_map(|visit| visit.trail_id.clone()).collect::<HashSet<_>>().len()
            as i64;
    let arrival_breakdown = ArrivalBreakdown {
        search: visits.iter().filter(|visit| visit.trail_id.is_some()).count() as i64,
        link: visits.iter().filter(|visit| visit.from_visit.is_some()).count() as i64,
        typed: visits
            .iter()
            .filter(|visit| visit.from_visit.is_none() && visit.trail_id.is_none())
            .count() as i64,
        other: 0,
    };
    let top_pages = visits
        .iter()
        .fold(HashMap::<String, i64>::new(), |mut acc, visit| {
            *acc.entry(path_from_url(&visit.url)).or_default() += 1;
            acc
        })
        .into_iter()
        .map(|(path, visit_count)| DomainPageStat { path, visit_count })
        .collect::<Vec<_>>();
    let mut top_pages = top_pages;
    top_pages.sort_by(|left, right| {
        right.visit_count.cmp(&left.visit_count).then_with(|| left.path.cmp(&right.path))
    });
    top_pages.truncate(10);
    let (top_referrers, top_exits) = build_domain_flows(&visits);
    let visit_trend = get_domain_trend(
        paths,
        config,
        key,
        &DomainTrendRequest {
            registrable_domain: request.registrable_domain.clone(),
            date_range: request.date_range.clone(),
        },
    )?
    .points;
    let domain_category = visits
        .first()
        .map(|visit| visit.domain_category.clone())
        .unwrap_or_else(|| "unknown".to_string());
    Ok(DomainDeepDive {
        registrable_domain: request.registrable_domain.clone(),
        display_name: display_name_for_domain(&request.registrable_domain),
        domain_category,
        total_visits,
        active_days,
        trail_count,
        arrival_breakdown,
        top_pages,
        top_referrers,
        top_exits,
        visit_trend,
    })
}

pub fn get_browsing_rhythm(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    request: &CategoryFilteredDateRangeRequest,
) -> Result<RhythmHeatmap> {
    let connection = open_intelligence_connection(paths, config, key)?;
    ensure_core_intelligence_schema(&connection)?;
    let (start_ms, end_ms) = date_range_bounds(&request.date_range)?;
    let mut statement = connection.prepare(
        "SELECT visits.visit_time_ms
         FROM visit_derived_facts
         JOIN archive.visits AS visits ON visits.id = visit_derived_facts.visit_id
         WHERE (?1 IS NULL OR visit_derived_facts.profile_id = ?1)
           AND (?2 IS NULL OR visit_derived_facts.domain_category = ?2)
           AND visits.visit_time_ms >= ?3
           AND visits.visit_time_ms < ?4",
    )?;
    let mut buckets = HashMap::<(i64, i64), i64>::new();
    let rows = statement.query_map(
        params![request.profile_id.as_deref(), request.category.as_deref(), start_ms, end_ms],
        |row| row.get::<_, i64>(0),
    )?;
    for row in rows {
        let visit_time_ms = row?;
        let local = local_datetime_from_millis(visit_time_ms);
        let dow = local.weekday().num_days_from_sunday() as i64;
        let hour = local.hour() as i64;
        *buckets.entry((dow, hour)).or_default() += 1;
    }
    let max_count = buckets.values().copied().max().unwrap_or_default();
    let cells = buckets
        .into_iter()
        .map(|((dow, hour), visit_count)| RhythmHeatmapCell { dow, hour, visit_count })
        .collect::<Vec<_>>();
    Ok(RhythmHeatmap { cells, max_count })
}

pub fn get_discovery_trend(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    request: &GranularityDateRangeRequest,
) -> Result<DiscoveryTrend> {
    let connection = open_intelligence_connection(paths, config, key)?;
    ensure_core_intelligence_schema(&connection)?;
    let mut statement = connection.prepare(
        "SELECT date_key, SUM(new_domains), SUM(total_visits)
         FROM daily_summary_rollups
         WHERE (?1 IS NULL OR profile_id = ?1)
           AND date_key >= ?2
           AND date_key <= ?3
         GROUP BY date_key
         ORDER BY date_key ASC",
    )?;
    let rows = statement
        .query_map(
            params![
                request.profile_id.as_deref(),
                request.date_range.start,
                request.date_range.end
            ],
            |row| {
                Ok((
                    collapse_date_key(&row.get::<_, String>(0)?, &request.granularity),
                    row.get::<_, i64>(1)?,
                    row.get::<_, i64>(2)?,
                ))
            },
        )?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let mut grouped = BTreeMap::<String, (i64, i64)>::new();
    for (date_key, new_domain_count, total_visits) in rows {
        let entry = grouped.entry(date_key).or_insert((0, 0));
        entry.0 += new_domain_count;
        entry.1 += total_visits;
    }
    Ok(DiscoveryTrend {
        points: grouped
            .into_iter()
            .map(|(date_key, (new_domain_count, total_visits))| DiscoveryTrendPoint {
                discovery_rate: if total_visits == 0 {
                    0.0
                } else {
                    new_domain_count as f32 / total_visits as f32
                },
                date_key,
                new_domain_count,
                total_visits,
            })
            .collect(),
    })
}

pub fn get_on_this_day(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    profile_id: Option<&str>,
) -> Result<Vec<OnThisDayEntry>> {
    let connection = open_intelligence_connection(paths, config, key)?;
    ensure_core_intelligence_schema(&connection)?;
    let today = Local::now();
    let month_day = today.format("%m-%d").to_string();
    let current_year = today.year();
    let mut statement = connection.prepare(
        "SELECT visits.visit_time_ms, visit_derived_facts.registrable_domain, sessions.is_deep_dive
         FROM visit_derived_facts
         JOIN archive.visits AS visits ON visits.id = visit_derived_facts.visit_id
         LEFT JOIN sessions ON sessions.session_id = visit_derived_facts.session_id
         WHERE (?1 IS NULL OR visit_derived_facts.profile_id = ?1)
           AND strftime('%m-%d', datetime(visits.visit_time_ms / 1000, 'unixepoch', 'localtime')) = ?2
           AND CAST(strftime('%Y', datetime(visits.visit_time_ms / 1000, 'unixepoch', 'localtime')) AS INTEGER) <> ?3",
    )?;
    let rows = statement.query_map(params![profile_id, month_day, current_year], |row| {
        Ok((
            row.get::<_, i64>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, Option<i64>>(2)?.unwrap_or(0) != 0,
        ))
    })?;
    let mut grouped = BTreeMap::<i32, Vec<(String, bool)>>::new();
    for row in rows {
        let (visit_time_ms, domain, is_deep_dive) = row?;
        grouped
            .entry(local_datetime_from_millis(visit_time_ms).year())
            .or_default()
            .push((domain, is_deep_dive));
    }
    Ok(grouped
        .into_iter()
        .rev()
        .map(|(year, items)| {
            let total_visits = items.len() as i64;
            let deep_dive_sessions = items.iter().filter(|(_, is_deep)| *is_deep).count() as i64;
            let mut top_domains = items
                .iter()
                .fold(HashMap::<String, i64>::new(), |mut acc, (domain, _)| {
                    *acc.entry(domain.clone()).or_default() += 1;
                    acc
                })
                .into_iter()
                .collect::<Vec<_>>();
            top_domains
                .sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(&right.0)));
            let top_domains =
                top_domains.into_iter().take(3).map(|(domain, _)| domain).collect::<Vec<_>>();
            OnThisDayEntry {
                year,
                date: format!("{year}-{}", Local::now().format("%m-%d")),
                total_visits,
                top_domains: top_domains.clone(),
                summary: top_domains.first().map(|domain| format!("Mostly browsing {domain}")),
                deep_dive_sessions,
            }
        })
        .collect())
}

fn load_session_visits(connection: &Connection, session_id: &str) -> Result<Vec<SessionVisit>> {
    let mut statement = connection.prepare(
        "SELECT visits.id, urls.url, urls.title, visit_derived_facts.registrable_domain, visits.visit_time_ms,
                visit_derived_facts.is_search_event, visit_derived_facts.search_query,
                visit_derived_facts.search_engine, visit_derived_facts.trail_id, visits.transition_type
         FROM visit_derived_facts
         JOIN archive.visits AS visits ON visits.id = visit_derived_facts.visit_id
         JOIN archive.urls AS urls ON urls.id = visits.url_id
         WHERE visit_derived_facts.session_id = ?1
         ORDER BY visits.visit_time_ms ASC, visits.id ASC",
    )?;
    statement
        .query_map([session_id], |row| {
            Ok(SessionVisit {
                visit_id: row.get(0)?,
                url: row.get(1)?,
                title: row.get(2)?,
                registrable_domain: row.get(3)?,
                visit_time_ms: row.get(4)?,
                is_search_event: row.get::<_, i64>(5)? != 0,
                search_query: row.get(6)?,
                search_engine: row.get(7)?,
                trail_id: row.get(8)?,
                transition_type: row.get::<_, Option<i64>>(9)?.map(|value| value.to_string()),
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(Into::into)
}

fn load_session_trails(connection: &Connection, session_id: &str) -> Result<Vec<TrailSummary>> {
    let mut statement = connection.prepare(
        "SELECT trail_id, session_id, initial_query, search_engine, reformulation_count, visit_count,
                landing_url, landing_domain, first_visit_ms, last_visit_ms, max_depth, queries_json
         FROM search_trails
         WHERE session_id = ?1
         ORDER BY first_visit_ms ASC, trail_id ASC",
    )?;
    statement
        .query_map([session_id], trail_summary_from_row)?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(Into::into)
}

fn trail_summary_from_row(row: &Row<'_>) -> rusqlite::Result<TrailSummary> {
    Ok(TrailSummary {
        trail_id: row.get(0)?,
        session_id: row.get(1)?,
        initial_query: row.get(2)?,
        search_engine: row.get(3)?,
        reformulation_count: row.get(4)?,
        visit_count: row.get(5)?,
        landing_url: row.get(6)?,
        landing_domain: row.get(7)?,
        first_visit_ms: row.get(8)?,
        last_visit_ms: row.get(9)?,
        max_depth: row.get(10)?,
        queries: serde_json::from_str(&row.get::<_, String>(11)?).unwrap_or_default(),
    })
}

fn load_navigation_visit(connection: &Connection, visit_id: i64) -> Result<Option<VisitRecord>> {
    connection
        .query_row(
            "SELECT visits.id, source_profiles.profile_key, visits.source_profile_id, CAST(visits.source_visit_id AS INTEGER),
                    urls.id, urls.url, urls.title, visits.visit_time_ms, visits.from_visit, visits.transition_type,
                    visits.external_referrer_url
             FROM archive.visits AS visits
             JOIN archive.urls AS urls ON urls.id = visits.url_id
             JOIN archive.source_profiles AS source_profiles ON source_profiles.id = visits.source_profile_id
             WHERE visits.id = ?1",
            [visit_id],
            visit_from_row,
        )
        .optional()
        .map_err(Into::into)
}

fn load_navigation_visit_by_source(
    connection: &Connection,
    source_profile_id: i64,
    source_visit_id: i64,
) -> Result<Option<VisitRecord>> {
    connection
        .query_row(
            "SELECT visits.id, source_profiles.profile_key, visits.source_profile_id, CAST(visits.source_visit_id AS INTEGER),
                    urls.id, urls.url, urls.title, visits.visit_time_ms, visits.from_visit, visits.transition_type,
                    visits.external_referrer_url
             FROM archive.visits AS visits
             JOIN archive.urls AS urls ON urls.id = visits.url_id
             JOIN archive.source_profiles AS source_profiles ON source_profiles.id = visits.source_profile_id
             WHERE visits.source_profile_id = ?1
               AND CAST(visits.source_visit_id AS INTEGER) = ?2
             LIMIT 1",
            params![source_profile_id, source_visit_id],
            visit_from_row,
        )
        .optional()
        .map_err(Into::into)
}

fn load_category_shares(
    connection: &Connection,
    range: &DateRange,
    profile_id: Option<&str>,
) -> Result<Vec<CategoryMixEntry>> {
    let mut statement = connection.prepare(
        "SELECT domain_category, SUM(visit_count)
         FROM category_daily_rollups
         WHERE (?1 IS NULL OR profile_id = ?1)
           AND date_key >= ?2
           AND date_key <= ?3
         GROUP BY domain_category
         ORDER BY SUM(visit_count) DESC, domain_category ASC",
    )?;
    let rows = statement
        .query_map(params![profile_id, range.start, range.end], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let total = rows.iter().map(|(_, count)| *count).sum::<i64>().max(1) as f32;
    Ok(rows
        .into_iter()
        .map(|(domain_category, visit_count)| CategoryMixEntry {
            share: visit_count as f32 / total,
            domain_category,
            visit_count,
        })
        .collect())
}

fn previous_date_range(range: &DateRange) -> Result<DateRange> {
    let start = NaiveDate::parse_from_str(&range.start, "%Y-%m-%d")?;
    let end = NaiveDate::parse_from_str(&range.end, "%Y-%m-%d")?;
    let days = (end - start).num_days().max(0) + 1;
    let prev_end = start - Duration::days(1);
    let prev_start = prev_end - Duration::days(days - 1);
    Ok(DateRange {
        start: prev_start.format("%Y-%m-%d").to_string(),
        end: prev_end.format("%Y-%m-%d").to_string(),
    })
}

#[derive(Default)]
struct SummaryTotals {
    total_visits: i64,
    total_searches: i64,
    new_domains: i64,
}

fn load_summary_totals(
    connection: &Connection,
    range: &DateRange,
    profile_id: Option<&str>,
) -> Result<SummaryTotals> {
    connection
        .query_row(
            "SELECT SUM(total_visits), SUM(total_searches), SUM(new_domains)
             FROM daily_summary_rollups
             WHERE (?1 IS NULL OR profile_id = ?1)
               AND date_key >= ?2
               AND date_key <= ?3",
            params![profile_id, range.start, range.end],
            |row| {
                Ok(SummaryTotals {
                    total_visits: row.get::<_, Option<i64>>(0)?.unwrap_or_default(),
                    total_searches: row.get::<_, Option<i64>>(1)?.unwrap_or_default(),
                    new_domains: row.get::<_, Option<i64>>(2)?.unwrap_or_default(),
                })
            },
        )
        .map_err(Into::into)
}

fn count_deep_dive_sessions(
    connection: &Connection,
    range: &DateRange,
    profile_id: Option<&str>,
) -> Result<i64> {
    let (start_ms, end_ms) = date_range_bounds(range)?;
    connection
        .query_row(
            "SELECT COUNT(*)
             FROM sessions
             WHERE (?1 IS NULL OR profile_id = ?1)
               AND is_deep_dive = 1
               AND first_visit_ms >= ?2
               AND first_visit_ms < ?3",
            params![profile_id, start_ms, end_ms],
            |row| row.get(0),
        )
        .map_err(Into::into)
}

fn count_refind_pages_in_range(
    connection: &Connection,
    range: &DateRange,
    profile_id: Option<&str>,
) -> Result<i64> {
    let (start_ms, end_ms) = date_range_bounds(range)?;
    connection
        .query_row(
            "SELECT COUNT(*)
             FROM refind_pages
             WHERE (?1 IS NULL OR profile_id = ?1)
               AND last_seen_ms >= ?2
               AND first_seen_ms < ?3",
            params![profile_id, start_ms, end_ms],
            |row| row.get(0),
        )
        .map_err(Into::into)
}

fn build_kpi(current: i64, previous: i64) -> KpiMetric {
    let trend = if current > previous {
        "up"
    } else if current < previous {
        "down"
    } else {
        "flat"
    };
    let change_percent = if previous == 0 {
        None
    } else {
        Some(((current - previous) as f32 / previous as f32) * 100.0)
    };
    KpiMetric {
        value: current,
        previous_value: Some(previous),
        change_percent,
        trend: trend.to_string(),
    }
}

fn collapse_date_key(date_key: &str, granularity: &str) -> String {
    match granularity {
        "week" => {
            if let Ok(date) = NaiveDate::parse_from_str(date_key, "%Y-%m-%d") {
                let iso = date.iso_week();
                format!("{}-W{:02}", iso.year(), iso.week())
            } else {
                date_key.to_string()
            }
        }
        "month" => date_key.get(0..7).unwrap_or(date_key).to_string(),
        "year" => date_key.get(0..4).unwrap_or(date_key).to_string(),
        _ => date_key.to_string(),
    }
}

fn load_domain_visits(
    connection: &Connection,
    domain: &str,
    profile_id: Option<&str>,
    start_ms: i64,
    end_ms: i64,
) -> Result<Vec<VisitRecord>> {
    let mut statement = connection.prepare(
        "SELECT visits.id, visit_derived_facts.profile_id, visits.source_profile_id, CAST(visits.source_visit_id AS INTEGER),
                urls.id, urls.url, urls.title, visits.visit_time_ms, visits.from_visit, visits.transition_type,
                visits.external_referrer_url, visit_derived_facts.canonical_url, visit_derived_facts.registrable_domain,
                visit_derived_facts.domain_category, visit_derived_facts.page_category, visit_derived_facts.search_engine,
                visit_derived_facts.search_query, visit_derived_facts.is_new_domain, visit_derived_facts.is_search_event,
                visit_derived_facts.evidence_tier, visit_derived_facts.taxonomy_source, visit_derived_facts.taxonomy_pack,
                visit_derived_facts.taxonomy_version, visit_derived_facts.session_id, visit_derived_facts.trail_id
         FROM visit_derived_facts
         JOIN archive.visits AS visits ON visits.id = visit_derived_facts.visit_id
         JOIN archive.urls AS urls ON urls.id = visits.url_id
         WHERE visit_derived_facts.registrable_domain = ?1
           AND (?2 IS NULL OR visit_derived_facts.profile_id = ?2)
           AND visits.visit_time_ms >= ?3
           AND visits.visit_time_ms < ?4
         ORDER BY visits.visit_time_ms ASC, visits.id ASC",
    )?;
    statement
        .query_map(params![domain, profile_id, start_ms, end_ms], |row| {
            Ok(VisitRecord {
                visit_id: row.get(0)?,
                profile_id: row.get(1)?,
                source_profile_id: row.get(2)?,
                source_visit_id: row.get(3)?,
                source_url_id: row.get(4)?,
                url: row.get(5)?,
                title: row.get(6)?,
                visit_time_ms: row.get(7)?,
                from_visit: row.get(8)?,
                transition_type: row.get(9)?,
                external_referrer_url: row.get(10)?,
                canonical_url: row.get(11)?,
                registrable_domain: row.get(12)?,
                domain_category: row.get(13)?,
                page_category: row.get(14)?,
                search_engine: row.get(15)?,
                search_query: row.get(16)?,
                is_new_domain: row.get::<_, i64>(17)? != 0,
                is_search_event: row.get::<_, i64>(18)? != 0,
                evidence_tier: row.get(19)?,
                taxonomy_source: row.get(20)?,
                taxonomy_pack: row.get(21)?,
                taxonomy_version: row.get(22)?,
                display_name: display_name_for_domain(domain),
                session_id: row.get(23)?,
                trail_id: row.get(24)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(Into::into)
}

fn build_domain_flows(visits: &[VisitRecord]) -> (Vec<DomainFlowStat>, Vec<DomainFlowStat>) {
    let mut referrers = HashMap::<String, i64>::new();
    let mut exits = HashMap::<String, i64>::new();
    for pair in visits.windows(2) {
        let left = &pair[0];
        let right = &pair[1];
        if left.session_id != right.session_id {
            continue;
        }
        if left.registrable_domain != right.registrable_domain {
            *referrers.entry(left.registrable_domain.clone()).or_default() += 1;
            *exits.entry(right.registrable_domain.clone()).or_default() += 1;
        }
    }
    let map_to_stats = |input: HashMap<String, i64>| {
        let mut stats = input
            .into_iter()
            .map(|(domain, count)| DomainFlowStat {
                display_name: display_name_for_domain(&domain),
                domain,
                count,
            })
            .collect::<Vec<_>>();
        stats.sort_by(|left, right| {
            right.count.cmp(&left.count).then_with(|| left.domain.cmp(&right.domain))
        });
        stats.truncate(10);
        stats
    };
    (map_to_stats(referrers), map_to_stats(exits))
}

fn path_from_url(url: &str) -> String {
    url.split("://")
        .nth(1)
        .and_then(|value| value.split_once('/'))
        .map(|(_, path)| format!("/{}", path))
        .unwrap_or_else(|| "/".to_string())
}

#[cfg(test)]
mod tests {
    use super::{build_kpi, collapse_date_key, normalize_query};

    #[test]
    fn collapse_date_key_supports_week_and_month() {
        assert_eq!(collapse_date_key("2026-04-14", "month"), "2026-04");
        assert!(collapse_date_key("2026-04-14", "week").starts_with("2026-W"));
    }

    #[test]
    fn build_kpi_reports_flat_when_values_match() {
        let metric = build_kpi(10, 10);
        assert_eq!(metric.trend, "flat");
        assert_eq!(metric.change_percent, Some(0.0));
    }

    #[test]
    fn normalize_query_trims_and_lowercases() {
        assert_eq!(normalize_query("  WAL   Checkpoint "), "wal checkpoint");
    }
}
