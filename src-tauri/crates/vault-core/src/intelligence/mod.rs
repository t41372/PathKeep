//! Core Intelligence backend.
//!
//! This module owns the deterministic, non-LLM Core Intelligence read/write
//! plane introduced by the 2026-04-15 hard reset. The intelligence SQLite
//! plane stays rebuildable: canonical archive facts remain in
//! `archive/history-vault.sqlite`, while this module materializes sessions,
//! search trails, refind pages, rollups, and related analytics in
//! `derived/history-intelligence.sqlite`.

mod day_insights;
mod host_artifacts;
mod incremental;
mod intelligence_domain;
mod intelligence_explain;
mod intelligence_explain_helpers;
mod intelligence_outputs;
mod intelligence_overview;
mod intelligence_refind;
mod intelligence_summary;
mod phase_four;
mod phase_three;
mod site_dictionary;

use self::incremental::{
    ProfileSourceWatermark, StageCheckpoint, StageExecutionMode, delete_stage_checkpoints,
    ensure_core_intelligence_stage_checkpoint_schema, list_core_intelligence_profiles,
    load_profile_source_watermark, load_stage_checkpoint, save_stage_checkpoint, stage_name,
    stage_version, watermark_regressed,
};
use self::site_dictionary::{
    SiteDictionaryEntry, classify_visit, delete_search_engine_rule, display_name_for_domain,
    display_name_for_search_engine, display_name_for_search_engine_with_map,
    ensure_search_engine_rule_schema, ensure_site_dictionary_override_schema,
    list_search_engine_rules, load_enabled_search_engine_rules, load_search_engine_display_names,
    load_site_dictionary_overrides, normalize_query, upsert_search_engine_rule,
};
use crate::{
    archive::open_intelligence_connection,
    config::ProjectPaths,
    enrichment::ensure_visit_content_enrichment_schema,
    intelligence_catalog::RebuildMode,
    intelligence_runtime::{
        DeterministicModuleRuntimeUpdate, persist_deterministic_module_runtime_updates,
    },
    models::{
        AppConfig, ClearDerivedIntelligenceReport, CoreIntelligenceRebuildReport,
        CoreIntelligenceRebuildRequest, CoreIntelligenceStageTimings, DateRange, DomainFlowStat,
        DomainTrend, DomainTrendPoint, DomainTrendRequest, EngineRanking, HubPage,
        IntelligenceStatus, NavigationPath, NavigationPathStep, PagedDateRangeRequest, QueryFamily,
        QueryFamilyDetail, QueryFamilyDetailRequest, QueryFamilyResult, ScopedDateRangeRequest,
        SearchConcept, SearchEngineRule, SearchEngineRuleInput, SearchQueryListRequest,
        SearchQueryListResult, SearchQueryRow, SearchTrailQueryRequest, SessionDetail,
        SessionListResult, SessionSummary, SessionVisit, TopSearchConceptsRequest, TopSitesRequest,
        TrailDetail, TrailListResult, TrailMember, TrailSummary,
    },
    utils::now_rfc3339,
};
use anyhow::{Context, Result};
use chrono::{Datelike, Duration, Local, LocalResult, NaiveDate, TimeZone, Utc};
use reqwest::Url;
use rusqlite::{Connection, OptionalExtension, Row, params};
use serde_json::json;
use std::{
    collections::{BTreeMap, BTreeSet, HashMap, HashSet},
    time::Instant,
};

pub use self::day_insights::get_day_insights;
pub use self::host_artifacts::{build_intelligence_local_host, preview_intelligence_local_host};
pub use self::intelligence_domain::{
    get_browsing_rhythm, get_discovery_trend, get_domain_deep_dive, get_on_this_day,
};
pub use self::intelligence_explain::explain_entity;
pub use self::intelligence_outputs::{
    get_intelligence_embed_cards, get_intelligence_public_snapshot,
    get_intelligence_widget_snapshot,
};
pub use self::intelligence_overview::{
    get_intelligence_primary_overview, get_intelligence_secondary_overview,
};
pub use self::intelligence_refind::{
    explain_refind, get_refind_page_detail, get_refind_pages, get_top_sites,
};
pub use self::intelligence_summary::{
    get_activity_mix, get_activity_mix_trend, get_digest_summary, get_friction_signals,
    get_reopened_investigations, get_search_effectiveness, get_stable_sources,
};
pub use self::phase_four::{get_compare_set_detail, get_compare_sets, get_multi_browser_diff};
pub use self::phase_three::{
    get_breadth_index, get_habit_patterns, get_interrupted_habits, get_observed_interactions,
    get_path_flows,
};

const CORE_INTELLIGENCE_SCHEMA_SQL: &str = r#"
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
CREATE INDEX IF NOT EXISTS idx_vdf_profile_visit_id ON visit_derived_facts(profile_id, visit_id);

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
CREATE INDEX IF NOT EXISTS idx_search_trails_profile_time_trail ON search_trails(profile_id, first_visit_ms ASC, trail_id ASC);

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
  query_kind         TEXT NOT NULL DEFAULT 'keyword',
  trail_id           TEXT,
  computed_at        TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_search_events_profile_engine ON search_events(profile_id, search_engine);
CREATE INDEX IF NOT EXISTS idx_search_events_profile_visit ON search_events(profile_id, visit_id);
CREATE INDEX IF NOT EXISTS idx_search_events_profile_kind ON search_events(profile_id, query_kind);

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

const INTELLIGENCE_SCHEMA_MIGRATIONS_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS intelligence_schema_migrations (
  version     INTEGER PRIMARY KEY,
  name        TEXT NOT NULL,
  applied_at  TEXT NOT NULL
);
"#;

#[derive(Clone, Copy)]
struct IntelligenceMigrationSpec {
    version: i64,
    name: &'static str,
    apply: fn(&Connection) -> Result<()>,
}

const INTELLIGENCE_MIGRATIONS: &[IntelligenceMigrationSpec] = &[
    IntelligenceMigrationSpec {
        version: 1,
        name: "core-intelligence-baseline",
        apply: apply_core_intelligence_baseline_migration,
    },
    IntelligenceMigrationSpec {
        version: 2,
        name: "site-dictionary-overrides",
        apply: apply_site_dictionary_override_migration,
    },
    IntelligenceMigrationSpec {
        version: 3,
        name: "stage-checkpoints",
        apply: apply_core_intelligence_stage_checkpoint_migration,
    },
    IntelligenceMigrationSpec {
        version: 4,
        name: "batch-read-indexes",
        apply: apply_core_intelligence_batch_index_migration,
    },
    IntelligenceMigrationSpec {
        version: 5,
        name: "search-engine-rules",
        apply: apply_search_engine_rule_migration,
    },
    IntelligenceMigrationSpec {
        version: 6,
        name: "search-query-kind",
        apply: apply_search_query_kind_migration,
    },
];

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
}

#[derive(Debug, Clone)]
struct SearchEventRecord {
    visit_id: i64,
    profile_id: String,
    search_engine: String,
    raw_query: String,
    normalized_query: String,
    query_kind: SearchQueryKind,
    trail_id: Option<String>,
    visit_time_ms: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SearchQueryKind {
    Keyword,
    Navigational,
}

impl SearchQueryKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::Keyword => "keyword",
            Self::Navigational => "navigational",
        }
    }

    fn is_keyword(self) -> bool {
        matches!(self, Self::Keyword)
    }
}

fn parse_search_query_kind(value: &str) -> SearchQueryKind {
    match value {
        "navigational" => SearchQueryKind::Navigational,
        _ => SearchQueryKind::Keyword,
    }
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
struct StructuralVisitRecord {
    visit_id: i64,
    profile_id: String,
    url: String,
    visit_time_ms: i64,
    from_visit: Option<i64>,
    registrable_domain: String,
    search_engine: Option<String>,
    search_query: Option<String>,
    is_new_domain: bool,
    is_search_event: bool,
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

#[derive(Debug, Clone, Default)]
struct StageRunResult {
    processed_visits: usize,
    visit_derived_facts: usize,
    sessions: usize,
    search_trails: usize,
    query_families: usize,
    refind_pages: usize,
    source_effectiveness: usize,
    reopened_investigations: usize,
    execution_mode: Option<String>,
    affected_profiles: Vec<String>,
    dirty_visit_count: Option<usize>,
    dirty_date_keys: Vec<String>,
    fallback_reason: Option<String>,
    stage_timings_ms: Option<CoreIntelligenceStageTimings>,
    notes: Vec<String>,
}

// These batch sizes stay well below the low-RAM envelope we benchmark against,
// but they cut a large amount of repeated SQLite scan/statement overhead on
// multi-million-row rebuilds.
const STRUCTURAL_AGGREGATE_BATCH_SIZE: usize = 8_192;
const STRUCTURAL_TAIL_STREAM_BATCH_SIZE: usize = 8_192;
const VISIT_DERIVE_FALLBACK_BATCH_SIZE: usize = 8_192;
const DAILY_ROLLUP_FALLBACK_BATCH_SIZE: usize = 8_192;

#[derive(Debug, Clone, Copy)]
struct DerivedVisitBatchCursor {
    visit_time_ms: i64,
    visit_id: i64,
}

#[derive(Debug, Clone, Copy)]
struct SearchEventBatchCursor {
    visit_id: i64,
}

#[derive(Debug, Clone)]
struct TrailBatchCursor {
    first_visit_ms: i64,
    trail_id: String,
}

#[derive(Debug, Clone, Copy)]
struct VisibleVisitBatchCursor {
    visit_time_ms: i64,
    visit_id: i64,
}

#[derive(Debug, Clone, Copy)]
struct StructuralVisitBatchCursor {
    visit_time_ms: i64,
    visit_id: i64,
}

#[derive(Debug, Default)]
struct StructuralDeltaSummary {
    delta_count: usize,
    dirty_date_keys: Vec<String>,
    dirty_from_visit_ms: Option<i64>,
}

#[derive(Debug, Default)]
struct StructuralTailStreamReport {
    processed_visits: usize,
    sessions: usize,
    trails: usize,
    first_visit_ms: Option<i64>,
}

type DailyDomainKey = (String, String, String);
type DailyDomainValue = (String, i64, i64, i64, HashSet<String>);
type DailyCategoryKey = (String, String, String);
type DailyCategoryValue = (i64, HashSet<String>);
type DailyEngineKey = (String, String, String);
type DailySummaryKey = (String, String);
type DailySummaryValue = (i64, i64, HashSet<String>, HashSet<String>, HashMap<String, i64>);

#[derive(Debug, Default)]
struct DailyRollupAccumulator {
    domains: HashMap<DailyDomainKey, DailyDomainValue>,
    categories: HashMap<DailyCategoryKey, DailyCategoryValue>,
    engines: HashMap<DailyEngineKey, i64>,
    summaries: HashMap<DailySummaryKey, DailySummaryValue>,
}

impl DailyRollupAccumulator {
    fn add_visit(&mut self, visit: &VisitRecord) {
        let date_key = local_date_key(visit.visit_time_ms);
        let domain_key =
            (date_key.clone(), visit.profile_id.clone(), visit.registrable_domain.clone());
        let domain_entry = self.domains.entry(domain_key).or_insert((
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
        let category_entry = self.categories.entry(category_key).or_insert((0, HashSet::new()));
        category_entry.0 += 1;
        category_entry.1.insert(visit.registrable_domain.clone());

        if let Some(engine) = &visit.search_engine {
            *self
                .engines
                .entry((date_key.clone(), visit.profile_id.clone(), engine.clone()))
                .or_default() += 1;
        }

        let summary_key = (date_key, visit.profile_id.clone());
        let summary_entry = self.summaries.entry(summary_key).or_insert((
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

    fn extend<'a>(&mut self, visits: impl IntoIterator<Item = &'a VisitRecord>) {
        for visit in visits {
            self.add_visit(visit);
        }
    }

    fn finish(self) -> DailyRollupBundle {
        DailyRollupBundle {
            domain_rows: self
                .domains
                .into_iter()
                .map(
                    |(
                        (date_key, profile_id, registrable_domain),
                        (
                            domain_category,
                            visit_count,
                            search_count,
                            new_domain_visits,
                            unique_urls,
                        ),
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
            category_rows: self
                .categories
                .into_iter()
                .map(|((date_key, profile_id, domain_category), (visit_count, unique_domains))| {
                    (
                        date_key,
                        profile_id,
                        domain_category,
                        visit_count,
                        unique_domains.len() as i64,
                    )
                })
                .collect(),
            engine_rows: self
                .engines
                .into_iter()
                .map(|((date_key, profile_id, search_engine), search_count)| {
                    (date_key, profile_id, search_engine, search_count)
                })
                .collect(),
            summary_rows: self
                .summaries
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
}

#[derive(Debug, Default)]
struct QueryFamilyAccumulator {
    families: Vec<QueryFamilyRecord>,
}

#[derive(Debug, Clone, Default)]
struct RefindAccumulatorEntry {
    profile_id: String,
    canonical_url: String,
    url: String,
    title: Option<String>,
    registrable_domain: String,
    distinct_days: HashSet<String>,
    trail_ids: HashSet<String>,
    search_arrival_count: i64,
    typed_revisit_count: i64,
    first_seen_ms: i64,
    last_seen_ms: i64,
    visit_ids: Vec<i64>,
}

#[derive(Debug, Default)]
struct StructuralAggregateAccumulator {
    profile_id: Option<String>,
    refind_pages: HashMap<String, RefindAccumulatorEntry>,
    flow_counts: HashMap<(String, String, i64), (i64, i64)>,
    current_session_id: Option<String>,
    current_profile_id: Option<String>,
    current_sequence: Vec<(String, i64)>,
    habit_days: HashMap<String, BTreeSet<NaiveDate>>,
    last_visit_ms: HashMap<String, i64>,
}

fn apply_core_intelligence_baseline_migration(connection: &Connection) -> Result<()> {
    ensure_visit_content_enrichment_schema(connection)?;
    connection.execute_batch(CORE_INTELLIGENCE_SCHEMA_SQL)?;
    Ok(())
}

fn apply_site_dictionary_override_migration(connection: &Connection) -> Result<()> {
    ensure_site_dictionary_override_schema(connection)
}

fn apply_core_intelligence_stage_checkpoint_migration(connection: &Connection) -> Result<()> {
    ensure_core_intelligence_stage_checkpoint_schema(connection)
}

fn apply_search_engine_rule_migration(connection: &Connection) -> Result<()> {
    ensure_search_engine_rule_schema(connection)
}

fn apply_search_query_kind_migration(connection: &Connection) -> Result<()> {
    if !table_has_column(connection, "search_events", "query_kind")? {
        connection.execute(
            "ALTER TABLE search_events
             ADD COLUMN query_kind TEXT NOT NULL DEFAULT 'keyword'",
            [],
        )?;
    }
    connection.execute(
        "CREATE INDEX IF NOT EXISTS idx_search_events_profile_kind
         ON search_events(profile_id, query_kind)",
        [],
    )?;
    backfill_search_event_query_kinds(connection)
}

fn apply_core_intelligence_batch_index_migration(connection: &Connection) -> Result<()> {
    connection.execute_batch(
        "CREATE INDEX IF NOT EXISTS idx_vdf_profile_visit_id
           ON visit_derived_facts(profile_id, visit_id);
         CREATE INDEX IF NOT EXISTS idx_search_trails_profile_time_trail
           ON search_trails(profile_id, first_visit_ms ASC, trail_id ASC);
         CREATE INDEX IF NOT EXISTS idx_search_events_profile_visit
           ON search_events(profile_id, visit_id);",
    )?;
    Ok(())
}

fn table_has_column(connection: &Connection, table: &str, column: &str) -> Result<bool> {
    let pragma = format!("PRAGMA table_info({table})");
    let mut statement = connection.prepare(&pragma)?;
    let columns = statement
        .query_map([], |row| row.get::<_, String>(1))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(columns.iter().any(|candidate| candidate == column))
}

fn backfill_search_event_query_kinds(connection: &Connection) -> Result<()> {
    let mut statement = connection.prepare(
        "SELECT search_events.visit_id,
                search_events.raw_query,
                search_events.normalized_query,
                search_trails.landing_domain
         FROM search_events
         LEFT JOIN search_trails ON search_trails.trail_id = search_events.trail_id",
    )?;
    let rows = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, Option<String>>(3)?,
            ))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    drop(statement);
    let tx = connection.unchecked_transaction()?;
    let mut update = tx.prepare("UPDATE search_events SET query_kind = ?2 WHERE visit_id = ?1")?;
    for (visit_id, raw_query, normalized_query, landing_domain) in rows {
        let query_kind =
            classify_search_query_kind(&raw_query, &normalized_query, landing_domain.as_deref());
        update.execute(params![visit_id, query_kind.as_str()])?;
    }
    drop(update);
    tx.commit()?;
    Ok(())
}

fn load_applied_intelligence_migrations(connection: &Connection) -> Result<BTreeSet<i64>> {
    connection.execute_batch(INTELLIGENCE_SCHEMA_MIGRATIONS_SQL)?;
    let mut statement = connection.prepare(
        "SELECT version
         FROM intelligence_schema_migrations
         ORDER BY version ASC",
    )?;
    statement
        .query_map([], |row| row.get::<_, i64>(0))?
        .collect::<rusqlite::Result<BTreeSet<_>>>()
        .map_err(Into::into)
}

fn run_core_intelligence_migrations(connection: &Connection) -> Result<()> {
    let applied = load_applied_intelligence_migrations(connection)?;
    for migration in INTELLIGENCE_MIGRATIONS {
        if applied.contains(&migration.version) {
            continue;
        }
        (migration.apply)(connection)?;
        connection.execute(
            "INSERT INTO intelligence_schema_migrations (version, name, applied_at)
             VALUES (?1, ?2, ?3)",
            params![migration.version, migration.name, now_rfc3339()],
        )?;
    }
    Ok(())
}

pub(crate) fn ensure_core_intelligence_schema(connection: &Connection) -> Result<()> {
    run_core_intelligence_migrations(connection)?;
    drop_legacy_insight_tables(connection)?;
    Ok(())
}

pub fn intelligence_status(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
) -> Result<IntelligenceStatus> {
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
    Ok(IntelligenceStatus {
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
    let cleared_runtime_rows = table_row_count(&connection, "deterministic_module_runtime")?
        + table_row_count(&connection, "core_intelligence_stage_checkpoints")?
        + count_core_intelligence_job_triggers(&connection)?
        + count_core_intelligence_jobs(&connection)?;
    let report = ClearDerivedIntelligenceReport {
        cleared_visit_derived_fact_rows: table_row_count(&connection, "visit_derived_facts")?,
        cleared_daily_rollup_rows: sum_table_row_counts(
            &connection,
            &[
                "domain_daily_rollups",
                "category_daily_rollups",
                "engine_daily_rollups",
                "daily_summary_rollups",
            ],
        )?,
        cleared_structural_rows: sum_table_row_counts(
            &connection,
            &[
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
        )?,
        cleared_runtime_rows,
        notes: vec![
            "Cleared Core Intelligence derived rows, checkpoints, and runtime traces without touching canonical archive facts."
                .to_string(),
        ],
    };
    clear_core_tables(&connection, None)?;
    delete_stage_checkpoints(&connection, None)?;
    connection.execute(
        "DELETE FROM intelligence_jobs
         WHERE job_type IN ('visit-derive', 'daily-rollup', 'structural-rebuild', 'full-rebuild')",
        [],
    )?;
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
        RebuildMode::FullRebuild,
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
        RebuildMode::FullRebuild,
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
        RebuildMode::from_job_type(job_type)?,
        request,
        on_progress,
    )
}

fn run_core_intelligence_job_with_progress<F>(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    job_kind: RebuildMode,
    request: &CoreIntelligenceRebuildRequest,
    mut on_progress: F,
) -> Result<CoreIntelligenceRebuildReport>
where
    F: FnMut(CoreIntelligenceProgress) -> Result<()>,
{
    if request.limit.is_some() {
        return run_core_intelligence_legacy_job_with_progress(
            paths,
            config,
            key,
            job_kind,
            request,
            on_progress,
        );
    }
    let connection = open_intelligence_connection(paths, config, key)?;
    ensure_core_intelligence_schema(&connection)?;
    let run_id = Utc::now().timestamp_millis();
    let computed_at = now_rfc3339();
    let profile_ids = list_core_intelligence_profiles(&connection, request.profile_id.as_deref())?;
    if profile_ids.is_empty() {
        clear_core_tables_for_job_kind(&connection, request.profile_id.as_deref(), job_kind)?;
        delete_stage_checkpoints(&connection, request.profile_id.as_deref())?;
        let notes = vec!["No visible visits matched the requested rebuild scope.".to_string()];
        persist_ready_module_updates(
            &connection,
            run_id,
            Some(computed_at.clone()),
            &job_kind.module_ids(),
            &notes,
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
            execution_mode: Some(StageExecutionMode::Noop.as_str().to_string()),
            affected_profiles: Some(Vec::new()),
            dirty_visit_count: Some(0),
            dirty_date_keys: Some(Vec::new()),
            fallback_reason: None,
            stage_timings_ms: None,
            notes,
            last_run_at: computed_at,
        });
    }

    let mut aggregate = StageRunResult {
        execution_mode: Some(StageExecutionMode::Noop.as_str().to_string()),
        ..StageRunResult::default()
    };
    let profile_total = profile_ids.len();
    for (profile_index, profile_id) in profile_ids.iter().enumerate() {
        let watermark = load_profile_source_watermark(&connection, profile_id)?;
        on_progress(CoreIntelligenceProgress {
            phase: "profile-scan".to_string(),
            detail: format!("Preparing {} for {}.", job_kind.label(), profile_id),
            processed_items: Some(profile_index),
            total_items: Some(profile_total),
            progress_percent: Some((profile_index as f32 / profile_total.max(1) as f32) * 100.0),
        })?;
        let result = match job_kind {
            RebuildMode::VisitDerive => execute_visit_derive_stage(
                &connection,
                profile_id,
                &watermark,
                request.full_rebuild,
                run_id,
                &computed_at,
            )?,
            RebuildMode::DailyRollup => execute_daily_rollup_stage(
                &connection,
                profile_id,
                &watermark,
                request.full_rebuild,
                run_id,
                &computed_at,
            )?,
            RebuildMode::StructuralRebuild => execute_structural_stage(
                &connection,
                profile_id,
                &watermark,
                request.full_rebuild,
                run_id,
                &computed_at,
            )?,
            RebuildMode::FullRebuild => execute_full_rebuild_stages(
                &connection,
                profile_id,
                &watermark,
                run_id,
                &computed_at,
            )?,
        };
        merge_stage_run_result(&mut aggregate, result, job_kind);
        on_progress(CoreIntelligenceProgress {
            phase: "profile-build".to_string(),
            detail: format!("Updated {} for profile {}.", job_kind.label(), profile_id),
            processed_items: Some(profile_index + 1),
            total_items: Some(profile_total),
            progress_percent: Some(
                ((profile_index + 1) as f32 / profile_total.max(1) as f32) * 100.0,
            ),
        })?;
    }

    if aggregate.notes.is_empty() {
        aggregate.notes.push(format!("Completed a {}.", job_kind.label()));
    }
    persist_ready_module_updates(
        &connection,
        run_id,
        Some(computed_at.clone()),
        &job_kind.module_ids(),
        &aggregate.notes,
    )?;
    Ok(CoreIntelligenceRebuildReport {
        run_id,
        processed_visits: aggregate.processed_visits,
        visit_derived_facts: aggregate.visit_derived_facts,
        sessions: aggregate.sessions,
        search_trails: aggregate.search_trails,
        query_families: aggregate.query_families,
        refind_pages: aggregate.refind_pages,
        source_effectiveness: aggregate.source_effectiveness,
        reopened_investigations: aggregate.reopened_investigations,
        execution_mode: aggregate.execution_mode,
        affected_profiles: Some(aggregate.affected_profiles),
        dirty_visit_count: aggregate.dirty_visit_count,
        dirty_date_keys: Some(aggregate.dirty_date_keys),
        fallback_reason: aggregate.fallback_reason,
        stage_timings_ms: aggregate.stage_timings_ms,
        notes: aggregate.notes,
        last_run_at: computed_at,
    })
}

fn run_core_intelligence_legacy_job_with_progress<F>(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    job_kind: RebuildMode,
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
    let notes =
        vec![format!("Completed a {} through the scoped debug fallback path.", job_kind.label())];
    let visits = load_visible_visits(&connection, request.profile_id.as_deref(), request.limit)?;
    if visits.is_empty() {
        clear_core_tables_for_job_kind(&connection, request.profile_id.as_deref(), job_kind)?;
        persist_ready_module_updates(
            &connection,
            run_id,
            Some(computed_at.clone()),
            &job_kind.module_ids(),
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
            execution_mode: Some(StageExecutionMode::FallbackFull.as_str().to_string()),
            affected_profiles: request.profile_id.clone().map(|profile_id| vec![profile_id]),
            dirty_visit_count: Some(0),
            dirty_date_keys: Some(Vec::new()),
            fallback_reason: Some(
                "Scoped debug rebuilds use the legacy full recompute path and do not advance incremental checkpoints."
                    .to_string(),
            ),
            stage_timings_ms: None,
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
    let mut affected_profiles = Vec::new();
    for (profile_index, (profile_id, mut profile_visits)) in by_profile.into_iter().enumerate() {
        affected_profiles.push(profile_id.clone());
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
        &job_kind.module_ids(),
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
        execution_mode: Some(StageExecutionMode::FallbackFull.as_str().to_string()),
        affected_profiles: Some(affected_profiles),
        dirty_visit_count: Some(total_visible_visits),
        dirty_date_keys: Some(Vec::new()),
        fallback_reason: Some(
            "Scoped debug rebuilds use the legacy full recompute path and do not advance incremental checkpoints."
                .to_string(),
        ),
        stage_timings_ms: None,
        notes,
        last_run_at: computed_at,
    })
}

fn merge_stage_run_result(
    aggregate: &mut StageRunResult,
    next: StageRunResult,
    job_kind: RebuildMode,
) {
    let execution_mode = next.execution_mode.clone();
    aggregate.visit_derived_facts += next.visit_derived_facts;
    aggregate.sessions += next.sessions;
    aggregate.search_trails += next.search_trails;
    aggregate.query_families += next.query_families;
    aggregate.refind_pages += next.refind_pages;
    aggregate.source_effectiveness += next.source_effectiveness;
    aggregate.reopened_investigations += next.reopened_investigations;
    if matches!(job_kind, RebuildMode::VisitDerive | RebuildMode::FullRebuild) {
        aggregate.processed_visits += next.processed_visits;
    } else {
        aggregate.processed_visits = aggregate.processed_visits.max(next.processed_visits);
    }
    if matches!(execution_mode.as_deref(), Some("fallback-full"))
        || aggregate.execution_mode.is_none()
        || matches!(aggregate.execution_mode.as_deref(), Some("noop"))
    {
        aggregate.execution_mode = execution_mode;
    }
    aggregate.affected_profiles.extend(next.affected_profiles);
    aggregate.affected_profiles.sort();
    aggregate.affected_profiles.dedup();
    if let Some(value) = next.dirty_visit_count {
        aggregate.dirty_visit_count = Some(aggregate.dirty_visit_count.unwrap_or(0) + value);
    }
    aggregate.dirty_date_keys.extend(next.dirty_date_keys);
    aggregate.dirty_date_keys.sort();
    aggregate.dirty_date_keys.dedup();
    if aggregate.fallback_reason.is_none() {
        aggregate.fallback_reason = next.fallback_reason;
    }
    match (&mut aggregate.stage_timings_ms, next.stage_timings_ms) {
        (Some(current), Some(next)) => {
            current.visit_derive_ms += next.visit_derive_ms;
            current.daily_rollup_ms += next.daily_rollup_ms;
            current.structural_rebuild_ms += next.structural_rebuild_ms;
            current.total_ms += next.total_ms;
        }
        (None, Some(next)) => {
            aggregate.stage_timings_ms = Some(next);
        }
        _ => {}
    }
    aggregate.notes.extend(next.notes);
}

fn execute_full_rebuild_stages(
    connection: &Connection,
    profile_id: &str,
    watermark: &ProfileSourceWatermark,
    run_id: i64,
    computed_at: &str,
) -> Result<StageRunResult> {
    let visit_started = Instant::now();
    let mut combined =
        execute_visit_derive_stage(connection, profile_id, watermark, true, run_id, computed_at)?;
    let visit_derive_ms = visit_started.elapsed().as_millis() as u64;
    let daily_started = Instant::now();
    let daily =
        execute_daily_rollup_stage(connection, profile_id, watermark, true, run_id, computed_at)?;
    let daily_rollup_ms = daily_started.elapsed().as_millis() as u64;
    let structural_started = Instant::now();
    let structural =
        execute_structural_stage(connection, profile_id, watermark, true, run_id, computed_at)?;
    let structural_rebuild_ms = structural_started.elapsed().as_millis() as u64;
    merge_stage_run_result(&mut combined, daily, RebuildMode::DailyRollup);
    merge_stage_run_result(&mut combined, structural, RebuildMode::StructuralRebuild);
    combined.execution_mode = Some(StageExecutionMode::FallbackFull.as_str().to_string());
    combined.dirty_visit_count = Some(watermark.visible_visit_count.max(0) as usize);
    combined.stage_timings_ms = Some(CoreIntelligenceStageTimings {
        visit_derive_ms,
        daily_rollup_ms,
        structural_rebuild_ms,
        total_ms: visit_derive_ms + daily_rollup_ms + structural_rebuild_ms,
    });
    combined.notes.push(format!("Performed a full Core Intelligence rebuild for {}.", profile_id));
    Ok(combined)
}

fn execute_visit_derive_stage(
    connection: &Connection,
    profile_id: &str,
    watermark: &ProfileSourceWatermark,
    force_full: bool,
    run_id: i64,
    computed_at: &str,
) -> Result<StageRunResult> {
    let current_version = stage_version(connection, RebuildMode::VisitDerive)?;
    let checkpoint = load_stage_checkpoint(connection, profile_id, RebuildMode::VisitDerive)?;
    if watermark.visible_visit_count == 0 {
        clear_core_tables_for_job_kind(connection, Some(profile_id), RebuildMode::VisitDerive)?;
        save_stage_checkpoint(
            connection,
            &StageCheckpoint {
                profile_id: profile_id.to_string(),
                stage: stage_name(RebuildMode::VisitDerive).to_string(),
                stage_version: current_version,
                source_watermark: watermark.clone(),
                last_processed_visit_id: 0,
                last_run_id: Some(run_id),
                updated_at: computed_at.to_string(),
                ..StageCheckpoint::default()
            },
        )?;
        return Ok(StageRunResult {
            execution_mode: Some(StageExecutionMode::Noop.as_str().to_string()),
            affected_profiles: vec![profile_id.to_string()],
            notes: vec![format!(
                "No visible visits remained for {profile_id}; cleared visit-derived facts."
            )],
            ..StageRunResult::default()
        });
    }

    let mut fallback_reason = if force_full {
        Some("Manual full rebuild requested for visit-derived facts.".to_string())
    } else {
        None
    };
    if !force_full {
        match checkpoint.as_ref() {
            None => fallback_reason =
                Some("No visit-derived checkpoint was recorded for this profile yet.".to_string()),
            Some(checkpoint) if checkpoint.stage_version != current_version => {
                fallback_reason =
                    Some("Visit-derived rules changed since the last successful rebuild.".to_string())
            }
            Some(checkpoint) if watermark_regressed(watermark, &checkpoint.source_watermark) => {
                fallback_reason = Some(
                    "Archive visibility regressed or source counters moved backwards for visit-derived facts."
                        .to_string(),
                )
            }
            _ => {}
        }
    }

    if fallback_reason.is_none()
        && checkpoint.as_ref().is_some_and(|checkpoint| checkpoint.source_watermark == *watermark)
    {
        save_stage_checkpoint(
            connection,
            &StageCheckpoint {
                profile_id: profile_id.to_string(),
                stage: stage_name(RebuildMode::VisitDerive).to_string(),
                stage_version: current_version,
                source_watermark: watermark.clone(),
                last_processed_visit_id: checkpoint
                    .as_ref()
                    .map(|value| value.last_processed_visit_id)
                    .unwrap_or_default(),
                last_run_id: Some(run_id),
                updated_at: computed_at.to_string(),
                ..StageCheckpoint::default()
            },
        )?;
        return Ok(StageRunResult {
            execution_mode: Some(StageExecutionMode::Noop.as_str().to_string()),
            affected_profiles: vec![profile_id.to_string()],
            notes: vec![format!("Visit-derived facts for {profile_id} were already up to date.")],
            ..StageRunResult::default()
        });
    }

    let (visits, execution_mode, dirty_visit_count, dirty_date_keys, dirty_from_visit_ms) =
        if let Some(_reason) = fallback_reason.clone() {
            clear_core_tables_for_job_kind(connection, Some(profile_id), RebuildMode::VisitDerive)?;
            let fallback_summary = rebuild_visit_derived_facts_in_batches(
                connection,
                profile_id,
                computed_at,
                VISIT_DERIVE_FALLBACK_BATCH_SIZE,
            )?;
            (
                Vec::new(),
                StageExecutionMode::FallbackFull,
                fallback_summary.processed_visits,
                fallback_summary.dirty_date_keys,
                fallback_summary.dirty_from_visit_ms,
            )
        } else {
            let last_processed_visit_id =
                checkpoint.as_ref().map(|value| value.last_processed_visit_id).unwrap_or_default();
            let source_profile_id = load_archive_source_profile_id(connection, profile_id)?;
            let mut visits = load_visible_visits_after_id(
                connection,
                profile_id,
                source_profile_id,
                last_processed_visit_id,
            )?;
            let expected_delta = (watermark.visible_visit_count
                - checkpoint
                    .as_ref()
                    .map(|value| value.source_watermark.visible_visit_count)
                    .unwrap_or_default())
            .max(0) as usize;
            if visits.is_empty() || visits.len() != expected_delta {
                fallback_reason = Some(
                    "Visit-derived delta rows no longer matched the current archive watermark."
                        .to_string(),
                );
                clear_core_tables_for_job_kind(
                    connection,
                    Some(profile_id),
                    RebuildMode::VisitDerive,
                )?;
                let fallback_summary = rebuild_visit_derived_facts_in_batches(
                    connection,
                    profile_id,
                    computed_at,
                    VISIT_DERIVE_FALLBACK_BATCH_SIZE,
                )?;
                (
                    Vec::new(),
                    StageExecutionMode::FallbackFull,
                    fallback_summary.processed_visits,
                    fallback_summary.dirty_date_keys,
                    fallback_summary.dirty_from_visit_ms,
                )
            } else {
                let mut seen_domains = load_seen_domains(connection, profile_id)?;
                compute_is_new_domain_with_seen(&mut visits, &mut seen_domains);
                let dirty_date_keys = unique_date_keys(&visits);
                let dirty_from_visit_ms = visits.first().map(|visit| visit.visit_time_ms);
                (
                    visits,
                    StageExecutionMode::Incremental,
                    expected_delta,
                    dirty_date_keys,
                    dirty_from_visit_ms,
                )
            }
        };

    if execution_mode != StageExecutionMode::FallbackFull {
        persist_visit_derived_facts(connection, &visits, computed_at)?;
    }
    save_stage_checkpoint(
        connection,
        &StageCheckpoint {
            profile_id: profile_id.to_string(),
            stage: stage_name(RebuildMode::VisitDerive).to_string(),
            stage_version: current_version,
            source_watermark: watermark.clone(),
            last_processed_visit_id: watermark.max_visit_id,
            dirty_from_visit_ms,
            dirty_date_key: dirty_date_keys.first().cloned(),
            last_run_id: Some(run_id),
            fallback_reason: fallback_reason.clone(),
            updated_at: computed_at.to_string(),
        },
    )?;
    Ok(StageRunResult {
        processed_visits: dirty_visit_count,
        visit_derived_facts: dirty_visit_count,
        execution_mode: Some(execution_mode.as_str().to_string()),
        affected_profiles: vec![profile_id.to_string()],
        dirty_visit_count: Some(dirty_visit_count),
        dirty_date_keys,
        fallback_reason: fallback_reason.clone(),
        notes: vec![match execution_mode {
            StageExecutionMode::Incremental => {
                format!("Incrementally refreshed visit-derived facts for {profile_id}.")
            }
            StageExecutionMode::FallbackFull => {
                format!("Rebuilt visit-derived facts for {profile_id} with a scoped full refresh.")
            }
            StageExecutionMode::Noop => unreachable!(),
        }],
        ..StageRunResult::default()
    })
}

fn execute_daily_rollup_stage(
    connection: &Connection,
    profile_id: &str,
    watermark: &ProfileSourceWatermark,
    force_full: bool,
    run_id: i64,
    computed_at: &str,
) -> Result<StageRunResult> {
    let current_version = stage_version(connection, RebuildMode::DailyRollup)?;
    let checkpoint = load_stage_checkpoint(connection, profile_id, RebuildMode::DailyRollup)?;
    if watermark.visible_visit_count == 0 {
        clear_core_tables_for_job_kind(connection, Some(profile_id), RebuildMode::DailyRollup)?;
        save_stage_checkpoint(
            connection,
            &StageCheckpoint {
                profile_id: profile_id.to_string(),
                stage: stage_name(RebuildMode::DailyRollup).to_string(),
                stage_version: current_version,
                source_watermark: watermark.clone(),
                last_processed_visit_id: 0,
                last_run_id: Some(run_id),
                updated_at: computed_at.to_string(),
                ..StageCheckpoint::default()
            },
        )?;
        return Ok(StageRunResult {
            execution_mode: Some(StageExecutionMode::Noop.as_str().to_string()),
            affected_profiles: vec![profile_id.to_string()],
            notes: vec![format!(
                "No visible visits remained for {profile_id}; cleared daily rollups."
            )],
            ..StageRunResult::default()
        });
    }

    let mut fallback_reason = if force_full {
        Some("Manual full rebuild requested for daily rollups.".to_string())
    } else {
        None
    };
    if !force_full {
        match checkpoint.as_ref() {
            None => fallback_reason =
                Some("No daily-rollup checkpoint was recorded for this profile yet.".to_string()),
            Some(checkpoint) if checkpoint.stage_version != current_version => {
                fallback_reason = Some("Daily rollup logic changed since the last successful rebuild.".to_string())
            }
            Some(checkpoint) if watermark_regressed(watermark, &checkpoint.source_watermark) => {
                fallback_reason = Some(
                    "Archive visibility regressed or source counters moved backwards for daily rollups."
                        .to_string(),
                )
            }
            _ => {}
        }
    }

    if fallback_reason.is_none()
        && checkpoint.as_ref().is_some_and(|checkpoint| checkpoint.source_watermark == *watermark)
    {
        save_stage_checkpoint(
            connection,
            &StageCheckpoint {
                profile_id: profile_id.to_string(),
                stage: stage_name(RebuildMode::DailyRollup).to_string(),
                stage_version: current_version,
                source_watermark: watermark.clone(),
                last_processed_visit_id: checkpoint
                    .as_ref()
                    .map(|value| value.last_processed_visit_id)
                    .unwrap_or_default(),
                last_run_id: Some(run_id),
                updated_at: computed_at.to_string(),
                ..StageCheckpoint::default()
            },
        )?;
        return Ok(StageRunResult {
            execution_mode: Some(StageExecutionMode::Noop.as_str().to_string()),
            affected_profiles: vec![profile_id.to_string()],
            notes: vec![format!("Daily rollups for {profile_id} were already up to date.")],
            ..StageRunResult::default()
        });
    }

    let (_visits, rollups, execution_mode, dirty_visit_count, dirty_date_keys, dirty_from_visit_ms) =
        if let Some(_reason) = fallback_reason.clone() {
            let fallback_rollups = build_daily_rollups_for_profile_in_batches(
                connection,
                profile_id,
                DAILY_ROLLUP_FALLBACK_BATCH_SIZE,
            )?;
            (
                Vec::new(),
                fallback_rollups.rollups,
                StageExecutionMode::FallbackFull,
                fallback_rollups.processed_visits,
                fallback_rollups.dirty_date_keys,
                fallback_rollups.dirty_from_visit_ms,
            )
        } else {
            let last_processed_visit_id =
                checkpoint.as_ref().map(|value| value.last_processed_visit_id).unwrap_or_default();
            let delta_visits = load_profile_derived_visits(
                connection,
                profile_id,
                None,
                Some(last_processed_visit_id),
            )?;
            let expected_delta = (watermark.visible_visit_count
                - checkpoint
                    .as_ref()
                    .map(|value| value.source_watermark.visible_visit_count)
                    .unwrap_or_default())
            .max(0) as usize;
            if delta_visits.is_empty() || delta_visits.len() != expected_delta {
                fallback_reason = Some(
                    "Daily rollup delta rows no longer matched the current archive watermark."
                        .to_string(),
                );
                let fallback_rollups = build_daily_rollups_for_profile_in_batches(
                    connection,
                    profile_id,
                    DAILY_ROLLUP_FALLBACK_BATCH_SIZE,
                )?;
                (
                    Vec::new(),
                    fallback_rollups.rollups,
                    StageExecutionMode::FallbackFull,
                    fallback_rollups.processed_visits,
                    fallback_rollups.dirty_date_keys,
                    fallback_rollups.dirty_from_visit_ms,
                )
            } else {
                let dirty_date_keys = unique_date_keys(&delta_visits);
                let visits = load_profile_derived_visits_for_date_keys(
                    connection,
                    profile_id,
                    &dirty_date_keys,
                )?;
                let dirty_from_visit_ms = visits.first().map(|visit| visit.visit_time_ms);
                let rollups = build_daily_rollups(&visits);
                (
                    visits,
                    rollups,
                    StageExecutionMode::Incremental,
                    expected_delta,
                    dirty_date_keys,
                    dirty_from_visit_ms,
                )
            }
        };

    replace_daily_rollups(
        connection,
        profile_id,
        if execution_mode == StageExecutionMode::FallbackFull {
            None
        } else {
            Some(&dirty_date_keys)
        },
        &rollups,
    )?;
    save_stage_checkpoint(
        connection,
        &StageCheckpoint {
            profile_id: profile_id.to_string(),
            stage: stage_name(RebuildMode::DailyRollup).to_string(),
            stage_version: current_version,
            source_watermark: watermark.clone(),
            last_processed_visit_id: watermark.max_visit_id,
            dirty_from_visit_ms,
            dirty_date_key: dirty_date_keys.first().cloned(),
            last_run_id: Some(run_id),
            fallback_reason: fallback_reason.clone(),
            updated_at: computed_at.to_string(),
        },
    )?;
    Ok(StageRunResult {
        processed_visits: dirty_visit_count,
        execution_mode: Some(execution_mode.as_str().to_string()),
        affected_profiles: vec![profile_id.to_string()],
        dirty_visit_count: Some(dirty_visit_count),
        dirty_date_keys,
        fallback_reason: fallback_reason.clone(),
        notes: vec![match execution_mode {
            StageExecutionMode::Incremental => {
                format!("Refreshed dirty daily rollups for {profile_id}.")
            }
            StageExecutionMode::FallbackFull => {
                format!("Rebuilt all daily rollups for {profile_id}.")
            }
            StageExecutionMode::Noop => unreachable!(),
        }],
        ..StageRunResult::default()
    })
}

fn execute_structural_stage(
    connection: &Connection,
    profile_id: &str,
    watermark: &ProfileSourceWatermark,
    force_full: bool,
    run_id: i64,
    computed_at: &str,
) -> Result<StageRunResult> {
    let current_version = stage_version(connection, RebuildMode::StructuralRebuild)?;
    let checkpoint = load_stage_checkpoint(connection, profile_id, RebuildMode::StructuralRebuild)?;
    if watermark.visible_visit_count == 0 {
        clear_core_tables_for_job_kind(
            connection,
            Some(profile_id),
            RebuildMode::StructuralRebuild,
        )?;
        save_stage_checkpoint(
            connection,
            &StageCheckpoint {
                profile_id: profile_id.to_string(),
                stage: stage_name(RebuildMode::StructuralRebuild).to_string(),
                stage_version: current_version,
                source_watermark: watermark.clone(),
                last_processed_visit_id: 0,
                last_run_id: Some(run_id),
                updated_at: computed_at.to_string(),
                ..StageCheckpoint::default()
            },
        )?;
        return Ok(StageRunResult {
            execution_mode: Some(StageExecutionMode::Noop.as_str().to_string()),
            affected_profiles: vec![profile_id.to_string()],
            notes: vec![format!(
                "No visible visits remained for {profile_id}; cleared structural entities."
            )],
            ..StageRunResult::default()
        });
    }

    let mut fallback_reason = if force_full {
        Some("Manual full rebuild requested for structural entities.".to_string())
    } else {
        None
    };
    if !force_full {
        match checkpoint.as_ref() {
            None => fallback_reason = Some(
                "No structural checkpoint was recorded for this profile yet.".to_string(),
            ),
            Some(checkpoint) if checkpoint.stage_version != current_version => {
                fallback_reason =
                    Some("Structural rebuild logic changed since the last successful rebuild.".to_string())
            }
            Some(checkpoint) if watermark_regressed(watermark, &checkpoint.source_watermark) => {
                fallback_reason = Some(
                    "Archive visibility regressed or source counters moved backwards for structural entities."
                        .to_string(),
                )
            }
            _ => {}
        }
    }

    if fallback_reason.is_none()
        && checkpoint.as_ref().is_some_and(|checkpoint| checkpoint.source_watermark == *watermark)
    {
        save_stage_checkpoint(
            connection,
            &StageCheckpoint {
                profile_id: profile_id.to_string(),
                stage: stage_name(RebuildMode::StructuralRebuild).to_string(),
                stage_version: current_version,
                source_watermark: watermark.clone(),
                last_processed_visit_id: checkpoint
                    .as_ref()
                    .map(|value| value.last_processed_visit_id)
                    .unwrap_or_default(),
                last_run_id: Some(run_id),
                updated_at: computed_at.to_string(),
                ..StageCheckpoint::default()
            },
        )?;
        return Ok(StageRunResult {
            execution_mode: Some(StageExecutionMode::Noop.as_str().to_string()),
            affected_profiles: vec![profile_id.to_string()],
            notes: vec![format!("Structural entities for {profile_id} were already up to date.")],
            ..StageRunResult::default()
        });
    }

    let (execution_mode, dirty_visit_count, dirty_date_keys, structural_start_ms) =
        if let Some(_reason) = fallback_reason.clone() {
            (
                StageExecutionMode::FallbackFull,
                watermark.visible_visit_count as usize,
                load_profile_dirty_date_keys(connection, profile_id, None, None)?,
                None,
            )
        } else {
            let last_processed_visit_id =
                checkpoint.as_ref().map(|value| value.last_processed_visit_id).unwrap_or_default();
            let delta_summary =
                load_structural_delta_summary(connection, profile_id, last_processed_visit_id)?;
            let expected_delta = (watermark.visible_visit_count
                - checkpoint
                    .as_ref()
                    .map(|value| value.source_watermark.visible_visit_count)
                    .unwrap_or_default())
            .max(0) as usize;
            if delta_summary.delta_count == 0 || delta_summary.delta_count != expected_delta {
                fallback_reason = Some(
                    "Structural delta rows no longer matched the current archive watermark."
                        .to_string(),
                );
                (
                    StageExecutionMode::FallbackFull,
                    watermark.visible_visit_count as usize,
                    load_profile_dirty_date_keys(connection, profile_id, None, None)?,
                    None,
                )
            } else {
                let dirty_from_visit_ms = delta_summary.dirty_from_visit_ms.unwrap_or_default();
                (
                    StageExecutionMode::Incremental,
                    expected_delta,
                    delta_summary.dirty_date_keys,
                    Some(expand_structural_rebuild_start(
                        connection,
                        profile_id,
                        dirty_from_visit_ms,
                    )?),
                )
            }
        };

    let tail_report = rebuild_structural_tail_state(
        connection,
        profile_id,
        structural_start_ms,
        computed_at,
        STRUCTURAL_TAIL_STREAM_BATCH_SIZE,
    )?;

    let query_families = build_query_families_from_batches(connection, profile_id)?;
    replace_query_families(connection, profile_id, &query_families, computed_at)?;

    let (refind_pages, path_flows, habits) =
        build_structural_profile_aggregates_from_batches(connection, profile_id)?;
    let source_effectiveness =
        build_source_effectiveness_from_database(connection, profile_id, &refind_pages)?;
    let reopened = build_reopened_investigations(&query_families, &refind_pages);
    replace_structural_profile_aggregates(
        connection,
        profile_id,
        &refind_pages,
        &source_effectiveness,
        &habits,
        &reopened,
        &path_flows,
        computed_at,
    )?;

    save_stage_checkpoint(
        connection,
        &StageCheckpoint {
            profile_id: profile_id.to_string(),
            stage: stage_name(RebuildMode::StructuralRebuild).to_string(),
            stage_version: current_version,
            source_watermark: watermark.clone(),
            last_processed_visit_id: watermark.max_visit_id,
            dirty_from_visit_ms: structural_start_ms
                .or_else(|| {
                    load_profile_first_visible_visit_ms(connection, profile_id).ok().flatten()
                })
                .or(tail_report.first_visit_ms),
            dirty_date_key: dirty_date_keys.first().cloned(),
            last_run_id: Some(run_id),
            fallback_reason: fallback_reason.clone(),
            updated_at: computed_at.to_string(),
        },
    )?;

    Ok(StageRunResult {
        processed_visits: if execution_mode == StageExecutionMode::FallbackFull {
            watermark.visible_visit_count.max(0) as usize
        } else {
            tail_report.processed_visits
        },
        sessions: tail_report.sessions,
        search_trails: tail_report.trails,
        query_families: query_families.len(),
        refind_pages: refind_pages.len(),
        source_effectiveness: source_effectiveness.len(),
        reopened_investigations: reopened.len(),
        execution_mode: Some(execution_mode.as_str().to_string()),
        affected_profiles: vec![profile_id.to_string()],
        dirty_visit_count: Some(dirty_visit_count),
        dirty_date_keys,
        fallback_reason: fallback_reason.clone(),
        notes: vec![match execution_mode {
            StageExecutionMode::Incremental => {
                format!("Rebuilt structural tail entities for {profile_id}.")
            }
            StageExecutionMode::FallbackFull => {
                format!("Rebuilt all structural entities for {profile_id}.")
            }
            StageExecutionMode::Noop => unreachable!(),
        }],
        ..StageRunResult::default()
    })
}

fn load_visible_visits_after_id(
    connection: &Connection,
    profile_id: &str,
    source_profile_id: i64,
    last_processed_visit_id: i64,
) -> Result<Vec<VisitRecord>> {
    let mut statement = connection.prepare(
        "SELECT visits.id,
                ?1,
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
         WHERE visits.reverted_at IS NULL
           AND visits.source_profile_id = ?2
           AND visits.id > ?3
         ORDER BY visits.visit_time_ms ASC, visits.id ASC",
    )?;
    let rows = statement.query_map(
        params![profile_id, source_profile_id, last_processed_visit_id],
        visit_from_row,
    )?;
    let mut visits = rows.collect::<rusqlite::Result<Vec<_>>>()?;
    hydrate_search_terms(connection, &mut visits)?;
    Ok(visits)
}

fn load_visible_visit_batch(
    connection: &Connection,
    profile_id: &str,
    source_profile_id: i64,
    after: Option<VisibleVisitBatchCursor>,
    limit: usize,
) -> Result<Vec<VisitRecord>> {
    let mut statement = connection.prepare(
        "SELECT visits.id,
                ?1,
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
         WHERE visits.reverted_at IS NULL
           AND visits.source_profile_id = ?2
           AND (
             ?3 IS NULL
             OR visits.visit_time_ms > ?3
             OR (visits.visit_time_ms = ?3 AND visits.id > ?4)
           )
         ORDER BY visits.visit_time_ms ASC, visits.id ASC
         LIMIT ?5",
    )?;
    let rows = statement.query_map(
        params![
            profile_id,
            source_profile_id,
            after.map(|cursor| cursor.visit_time_ms),
            after.map(|cursor| cursor.visit_id),
            limit.max(1) as i64,
        ],
        visit_from_row,
    )?;
    let mut visits = rows.collect::<rusqlite::Result<Vec<_>>>()?;
    hydrate_search_terms(connection, &mut visits)?;
    Ok(visits)
}

fn load_profile_derived_visits(
    connection: &Connection,
    profile_id: &str,
    start_ms: Option<i64>,
    last_processed_visit_id: Option<i64>,
) -> Result<Vec<VisitRecord>> {
    let mut statement = connection.prepare(
        "SELECT visits.id,
                visit_derived_facts.profile_id,
                visits.source_profile_id,
                CAST(visits.source_visit_id AS INTEGER),
                urls.id,
                urls.url,
                urls.title,
                visits.visit_time_ms,
                visits.from_visit,
                visits.transition_type,
                visits.external_referrer_url,
                visit_derived_facts.canonical_url,
                visit_derived_facts.registrable_domain,
                visit_derived_facts.domain_category,
                visit_derived_facts.page_category,
                visit_derived_facts.search_engine,
                visit_derived_facts.search_query,
                visit_derived_facts.is_new_domain,
                visit_derived_facts.is_search_event,
                visit_derived_facts.evidence_tier,
                visit_derived_facts.taxonomy_source,
                visit_derived_facts.taxonomy_pack,
                visit_derived_facts.taxonomy_version,
                visit_derived_facts.session_id,
                visit_derived_facts.trail_id
         FROM visit_derived_facts
         JOIN archive.visits AS visits ON visits.id = visit_derived_facts.visit_id
         JOIN archive.urls AS urls ON urls.id = visits.url_id
         WHERE visit_derived_facts.profile_id = ?1
           AND visits.reverted_at IS NULL
           AND (?2 IS NULL OR visits.visit_time_ms >= ?2)
           AND (?3 IS NULL OR visit_derived_facts.visit_id > ?3)
         ORDER BY visits.visit_time_ms ASC, visits.id ASC",
    )?;
    statement
        .query_map(params![profile_id, start_ms, last_processed_visit_id], |row| {
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
                display_name: display_name_for_domain(&row.get::<_, String>(12)?),
                session_id: row.get(23)?,
                trail_id: row.get(24)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(Into::into)
}

fn load_profile_derived_visit_batch(
    connection: &Connection,
    profile_id: &str,
    after: Option<DerivedVisitBatchCursor>,
    limit: usize,
) -> Result<Vec<VisitRecord>> {
    let mut statement = connection.prepare(
        "SELECT visits.id,
                visit_derived_facts.profile_id,
                visits.source_profile_id,
                CAST(visits.source_visit_id AS INTEGER),
                urls.id,
                urls.url,
                urls.title,
                visits.visit_time_ms,
                visits.from_visit,
                visits.transition_type,
                visits.external_referrer_url,
                visit_derived_facts.canonical_url,
                visit_derived_facts.registrable_domain,
                visit_derived_facts.domain_category,
                visit_derived_facts.page_category,
                visit_derived_facts.search_engine,
                visit_derived_facts.search_query,
                visit_derived_facts.is_new_domain,
                visit_derived_facts.is_search_event,
                visit_derived_facts.evidence_tier,
                visit_derived_facts.taxonomy_source,
                visit_derived_facts.taxonomy_pack,
                visit_derived_facts.taxonomy_version,
                visit_derived_facts.session_id,
                visit_derived_facts.trail_id
         FROM visit_derived_facts
         JOIN archive.visits AS visits ON visits.id = visit_derived_facts.visit_id
         JOIN archive.urls AS urls ON urls.id = visits.url_id
         WHERE visit_derived_facts.profile_id = ?1
           AND visits.reverted_at IS NULL
           AND (
             ?2 IS NULL
             OR visits.visit_time_ms > ?2
             OR (visits.visit_time_ms = ?2 AND visit_derived_facts.visit_id > ?3)
           )
         ORDER BY visits.visit_time_ms ASC, visits.id ASC
         LIMIT ?4",
    )?;
    statement
        .query_map(
            params![
                profile_id,
                after.map(|cursor| cursor.visit_time_ms),
                after.map(|cursor| cursor.visit_id),
                limit.max(1) as i64,
            ],
            |row| {
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
                    display_name: display_name_for_domain(&row.get::<_, String>(12)?),
                    session_id: row.get(23)?,
                    trail_id: row.get(24)?,
                })
            },
        )?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(Into::into)
}

fn load_structural_visit_batch(
    connection: &Connection,
    profile_id: &str,
    source_profile_id: i64,
    start_ms: Option<i64>,
    after: Option<StructuralVisitBatchCursor>,
    limit: usize,
) -> Result<Vec<StructuralVisitRecord>> {
    let mut statement = connection.prepare(
        "SELECT visit_derived_facts.visit_id,
                ?1,
                urls.url,
                visits.visit_time_ms,
                visits.from_visit,
                visit_derived_facts.registrable_domain,
                visit_derived_facts.search_engine,
                visit_derived_facts.search_query,
                visit_derived_facts.is_new_domain,
                visit_derived_facts.is_search_event
         FROM archive.visits AS visits
         JOIN visit_derived_facts ON visit_derived_facts.visit_id = visits.id
         JOIN archive.urls AS urls ON urls.id = visits.url_id
         WHERE visits.reverted_at IS NULL
           AND visits.source_profile_id = ?2
           AND (?3 IS NULL OR visits.visit_time_ms >= ?3)
           AND (
             ?4 IS NULL
             OR visits.visit_time_ms > ?4
             OR (visits.visit_time_ms = ?4 AND visits.id > ?5)
           )
         ORDER BY visits.visit_time_ms ASC, visits.id ASC
         LIMIT ?6",
    )?;
    statement
        .query_map(
            params![
                profile_id,
                source_profile_id,
                start_ms,
                after.map(|cursor| cursor.visit_time_ms),
                after.map(|cursor| cursor.visit_id),
                limit.max(1) as i64,
            ],
            |row| {
                Ok(StructuralVisitRecord {
                    visit_id: row.get(0)?,
                    profile_id: row.get(1)?,
                    url: row.get(2)?,
                    visit_time_ms: row.get(3)?,
                    from_visit: row.get(4)?,
                    registrable_domain: row.get(5)?,
                    search_engine: row.get(6)?,
                    search_query: row.get(7)?,
                    is_new_domain: row.get::<_, i64>(8)? != 0,
                    is_search_event: row.get::<_, i64>(9)? != 0,
                })
            },
        )?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(Into::into)
}

fn load_profile_dirty_date_keys(
    connection: &Connection,
    profile_id: &str,
    start_ms: Option<i64>,
    last_processed_visit_id: Option<i64>,
) -> Result<Vec<String>> {
    connection
        .prepare(
            "SELECT DISTINCT strftime('%Y-%m-%d', archive.visits.visit_time_ms / 1000.0, 'unixepoch', 'localtime')
             FROM visit_derived_facts
             JOIN archive.visits ON archive.visits.id = visit_derived_facts.visit_id
             WHERE visit_derived_facts.profile_id = ?1
               AND archive.visits.reverted_at IS NULL
               AND (?2 IS NULL OR archive.visits.visit_time_ms >= ?2)
               AND (?3 IS NULL OR visit_derived_facts.visit_id > ?3)
             ORDER BY 1 ASC",
        )?
        .query_map(params![profile_id, start_ms, last_processed_visit_id], |row| {
            row.get::<_, String>(0)
        })?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(Into::into)
}

fn load_structural_delta_summary(
    connection: &Connection,
    profile_id: &str,
    last_processed_visit_id: i64,
) -> Result<StructuralDeltaSummary> {
    let (delta_count, dirty_from_visit_ms) = connection.query_row(
        "SELECT COUNT(*), MIN(archive.visits.visit_time_ms)
         FROM visit_derived_facts
         JOIN archive.visits ON archive.visits.id = visit_derived_facts.visit_id
         WHERE visit_derived_facts.profile_id = ?1
           AND archive.visits.reverted_at IS NULL
           AND visit_derived_facts.visit_id > ?2",
        params![profile_id, last_processed_visit_id],
        |row| Ok((row.get::<_, i64>(0)?.max(0) as usize, row.get::<_, Option<i64>>(1)?)),
    )?;
    Ok(StructuralDeltaSummary {
        delta_count,
        dirty_from_visit_ms,
        dirty_date_keys: load_profile_dirty_date_keys(
            connection,
            profile_id,
            None,
            Some(last_processed_visit_id),
        )?,
    })
}

fn load_profile_first_visible_visit_ms(
    connection: &Connection,
    profile_id: &str,
) -> Result<Option<i64>> {
    connection
        .query_row(
            "SELECT MIN(archive.visits.visit_time_ms)
             FROM visit_derived_facts
             JOIN archive.visits ON archive.visits.id = visit_derived_facts.visit_id
             WHERE visit_derived_facts.profile_id = ?1
               AND archive.visits.reverted_at IS NULL",
            [profile_id],
            |row| row.get::<_, Option<i64>>(0),
        )
        .optional()
        .map(|value| value.flatten())
        .map_err(Into::into)
}

fn load_profile_derived_visits_for_date_keys(
    connection: &Connection,
    profile_id: &str,
    date_keys: &[String],
) -> Result<Vec<VisitRecord>> {
    if date_keys.is_empty() {
        return Ok(Vec::new());
    }
    let start_ms = local_day_start_ms(
        date_keys.iter().min().context("missing minimum dirty date key for daily rollup")?,
    )?;
    let end_ms = local_day_end_exclusive_ms(
        date_keys.iter().max().context("missing maximum dirty date key for daily rollup")?,
    )?;
    Ok(load_profile_derived_visits(connection, profile_id, Some(start_ms), None)?
        .into_iter()
        .filter(|visit| {
            let date_key = local_date_key(visit.visit_time_ms);
            date_keys.iter().any(|candidate| candidate == &date_key) && visit.visit_time_ms < end_ms
        })
        .collect())
}

#[derive(Debug, Default)]
struct VisitDeriveFallbackSummary {
    processed_visits: usize,
    dirty_date_keys: Vec<String>,
    dirty_from_visit_ms: Option<i64>,
}

fn rebuild_visit_derived_facts_in_batches(
    connection: &Connection,
    profile_id: &str,
    computed_at: &str,
    batch_size: usize,
) -> Result<VisitDeriveFallbackSummary> {
    let source_profile_id = load_archive_source_profile_id(connection, profile_id)?;
    let mut cursor = None;
    let mut seen_domains = HashSet::<String>::new();
    let mut dirty_date_keys = BTreeSet::<String>::new();
    let mut dirty_from_visit_ms = None;
    let mut processed_visits = 0usize;

    loop {
        let mut batch = load_visible_visit_batch(
            connection,
            profile_id,
            source_profile_id,
            cursor,
            batch_size,
        )?;
        if batch.is_empty() {
            break;
        }
        compute_is_new_domain_with_seen(&mut batch, &mut seen_domains);
        if dirty_from_visit_ms.is_none() {
            dirty_from_visit_ms = batch.first().map(|visit| visit.visit_time_ms);
        }
        dirty_date_keys.extend(batch.iter().map(|visit| local_date_key(visit.visit_time_ms)));
        processed_visits += batch.len();
        persist_visit_derived_facts(connection, &batch, computed_at)?;
        cursor = batch.last().map(|visit| VisibleVisitBatchCursor {
            visit_time_ms: visit.visit_time_ms,
            visit_id: visit.visit_id,
        });
    }

    Ok(VisitDeriveFallbackSummary {
        processed_visits,
        dirty_date_keys: dirty_date_keys.into_iter().collect(),
        dirty_from_visit_ms,
    })
}

#[derive(Debug, Default)]
struct DailyRollupFallbackSummary {
    processed_visits: usize,
    rollups: DailyRollupBundle,
    dirty_date_keys: Vec<String>,
    dirty_from_visit_ms: Option<i64>,
}

fn build_daily_rollups_for_profile_in_batches(
    connection: &Connection,
    profile_id: &str,
    _batch_size: usize,
) -> Result<DailyRollupFallbackSummary> {
    let (processed_visits, dirty_from_visit_ms) = connection.query_row(
        "SELECT COUNT(*), MIN(archive.visits.visit_time_ms)
         FROM visit_derived_facts
         JOIN archive.visits ON archive.visits.id = visit_derived_facts.visit_id
         WHERE visit_derived_facts.profile_id = ?1
           AND archive.visits.reverted_at IS NULL",
        [profile_id],
        |row| Ok((row.get::<_, i64>(0)?.max(0) as usize, row.get::<_, Option<i64>>(1)?)),
    )?;
    if processed_visits == 0 {
        return Ok(DailyRollupFallbackSummary::default());
    }

    let mut domain_statement = connection.prepare(
        "WITH category_counts AS (
             SELECT strftime('%Y-%m-%d', archive.visits.visit_time_ms / 1000.0, 'unixepoch', 'localtime') AS date_key,
                    visit_derived_facts.profile_id AS profile_id,
                    visit_derived_facts.registrable_domain AS registrable_domain,
                    visit_derived_facts.domain_category AS domain_category,
                    COUNT(*) AS category_visits
             FROM visit_derived_facts
             JOIN archive.visits ON archive.visits.id = visit_derived_facts.visit_id
             WHERE visit_derived_facts.profile_id = ?1
               AND archive.visits.reverted_at IS NULL
             GROUP BY date_key, profile_id, registrable_domain, domain_category
         ),
         ranked_categories AS (
             SELECT date_key,
                    profile_id,
                    registrable_domain,
                    domain_category,
                    ROW_NUMBER() OVER (
                        PARTITION BY date_key, profile_id, registrable_domain
                        ORDER BY category_visits DESC,
                                 CASE WHEN domain_category = 'unknown' THEN 1 ELSE 0 END ASC,
                                 domain_category ASC
                    ) AS category_rank
             FROM category_counts
         ),
         domain_totals AS (
             SELECT strftime('%Y-%m-%d', archive.visits.visit_time_ms / 1000.0, 'unixepoch', 'localtime') AS date_key,
                    visit_derived_facts.profile_id AS profile_id,
                    visit_derived_facts.registrable_domain AS registrable_domain,
                    COUNT(*) AS visit_count,
                    SUM(visit_derived_facts.is_search_event) AS search_count,
                    SUM(visit_derived_facts.is_new_domain) AS new_domain_visits,
                    COUNT(DISTINCT visit_derived_facts.canonical_url) AS unique_urls
             FROM visit_derived_facts
             JOIN archive.visits ON archive.visits.id = visit_derived_facts.visit_id
             WHERE visit_derived_facts.profile_id = ?1
               AND archive.visits.reverted_at IS NULL
             GROUP BY date_key, profile_id, registrable_domain
         )
         SELECT domain_totals.date_key,
                domain_totals.profile_id,
                domain_totals.registrable_domain,
                COALESCE(ranked_categories.domain_category, 'unknown') AS domain_category,
                domain_totals.visit_count,
                domain_totals.search_count,
                domain_totals.new_domain_visits,
                domain_totals.unique_urls
         FROM domain_totals
         LEFT JOIN ranked_categories
           ON ranked_categories.date_key = domain_totals.date_key
          AND ranked_categories.profile_id = domain_totals.profile_id
          AND ranked_categories.registrable_domain = domain_totals.registrable_domain
          AND ranked_categories.category_rank = 1
         ORDER BY domain_totals.date_key ASC, domain_totals.registrable_domain ASC",
    )?;
    let domain_rows = domain_statement
        .query_map([profile_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, i64>(4)?,
                row.get::<_, i64>(5)?,
                row.get::<_, i64>(6)?,
                row.get::<_, i64>(7)?,
            ))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    let mut engine_statement = connection.prepare(
        "SELECT strftime('%Y-%m-%d', archive.visits.visit_time_ms / 1000.0, 'unixepoch', 'localtime') AS date_key,
                visit_derived_facts.profile_id,
                visit_derived_facts.search_engine,
                COUNT(*)
         FROM visit_derived_facts
         JOIN archive.visits ON archive.visits.id = visit_derived_facts.visit_id
         WHERE visit_derived_facts.profile_id = ?1
           AND archive.visits.reverted_at IS NULL
           AND visit_derived_facts.search_engine IS NOT NULL
         GROUP BY date_key, visit_derived_facts.profile_id, visit_derived_facts.search_engine
         ORDER BY date_key ASC, visit_derived_facts.search_engine ASC",
    )?;
    let engine_rows = engine_statement
        .query_map([profile_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, i64>(3)?,
            ))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    let mut category_map = BTreeMap::<(String, String, String), (i64, i64)>::new();
    let mut summary_map = BTreeMap::<(String, String), (i64, i64, i64, i64, f32)>::new();
    let mut dirty_date_keys = BTreeSet::<String>::new();

    for (
        date_key,
        profile_id,
        _,
        domain_category,
        visit_count,
        search_count,
        new_domain_visits,
        _,
    ) in &domain_rows
    {
        dirty_date_keys.insert(date_key.clone());

        let category_entry = category_map
            .entry((date_key.clone(), profile_id.clone(), domain_category.clone()))
            .or_insert((0, 0));
        category_entry.0 += *visit_count;
        category_entry.1 += 1;

        let summary_entry =
            summary_map.entry((date_key.clone(), profile_id.clone())).or_insert((0, 0, 0, 0, 0.0));
        summary_entry.0 += *visit_count;
        summary_entry.1 += *search_count;
        summary_entry.2 += i64::from(*new_domain_visits > 0);
        summary_entry.3 += 1;
        summary_entry.4 += (*visit_count * *visit_count) as f32;
    }

    let category_rows = category_map
        .into_iter()
        .map(|((date_key, profile_id, domain_category), (visit_count, unique_domains))| {
            (date_key, profile_id, domain_category, visit_count, unique_domains)
        })
        .collect::<Vec<_>>();
    let summary_rows = summary_map
        .into_iter()
        .map(
            |(
                (date_key, profile_id),
                (total_visits, total_searches, new_domains, unique_domains, sumsq_domain_visits),
            )| {
                let hhi_score = if total_visits == 0 {
                    0.0
                } else {
                    sumsq_domain_visits / (total_visits * total_visits) as f32
                };
                let discovery_rate =
                    if total_visits == 0 { 0.0 } else { new_domains as f32 / total_visits as f32 };
                (
                    date_key,
                    profile_id,
                    total_visits,
                    total_searches,
                    new_domains,
                    unique_domains,
                    hhi_score,
                    discovery_rate,
                )
            },
        )
        .collect::<Vec<_>>();

    Ok(DailyRollupFallbackSummary {
        processed_visits,
        rollups: DailyRollupBundle { domain_rows, category_rows, engine_rows, summary_rows },
        dirty_date_keys: dirty_date_keys.into_iter().collect(),
        dirty_from_visit_ms,
    })
}

#[cfg(test)]
fn load_profile_search_events(
    connection: &Connection,
    profile_id: &str,
) -> Result<Vec<SearchEventRecord>> {
    let mut statement = connection.prepare(
        "SELECT search_events.visit_id,
                search_events.profile_id,
                search_events.search_engine,
                search_events.raw_query,
                search_events.normalized_query,
                search_events.query_kind,
                search_events.trail_id,
                archive.visits.visit_time_ms
         FROM search_events
         JOIN archive.visits ON archive.visits.id = search_events.visit_id
         WHERE profile_id = ?1
         ORDER BY visit_id ASC",
    )?;
    statement
        .query_map([profile_id], |row| {
            Ok(SearchEventRecord {
                visit_id: row.get(0)?,
                profile_id: row.get(1)?,
                search_engine: row.get(2)?,
                raw_query: row.get(3)?,
                normalized_query: row.get(4)?,
                query_kind: parse_search_query_kind(&row.get::<_, String>(5)?),
                trail_id: row.get(6)?,
                visit_time_ms: row.get(7)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(Into::into)
}

fn load_profile_search_event_batch(
    connection: &Connection,
    profile_id: &str,
    after: Option<SearchEventBatchCursor>,
    limit: usize,
) -> Result<Vec<SearchEventRecord>> {
    let mut statement = connection.prepare(
        "SELECT search_events.visit_id,
                search_events.profile_id,
                search_events.search_engine,
                search_events.raw_query,
                search_events.normalized_query,
                search_events.query_kind,
                search_events.trail_id,
                archive.visits.visit_time_ms
         FROM search_events
         JOIN archive.visits ON archive.visits.id = search_events.visit_id
         WHERE search_events.profile_id = ?1
           AND (?2 IS NULL OR search_events.visit_id > ?2)
         ORDER BY search_events.visit_id ASC
         LIMIT ?3",
    )?;
    statement
        .query_map(
            params![profile_id, after.map(|cursor| cursor.visit_id), limit.max(1) as i64],
            |row| {
                Ok(SearchEventRecord {
                    visit_id: row.get(0)?,
                    profile_id: row.get(1)?,
                    search_engine: row.get(2)?,
                    raw_query: row.get(3)?,
                    normalized_query: row.get(4)?,
                    query_kind: parse_search_query_kind(&row.get::<_, String>(5)?),
                    trail_id: row.get(6)?,
                    visit_time_ms: row.get(7)?,
                })
            },
        )?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(Into::into)
}

fn load_profile_trail_batch(
    connection: &Connection,
    profile_id: &str,
    after: Option<&TrailBatchCursor>,
    limit: usize,
) -> Result<Vec<TrailRecord>> {
    let mut statement = connection.prepare(
        "SELECT trail_id, profile_id, session_id, initial_query, search_engine, reformulation_count,
                visit_count, landing_url, landing_domain, first_visit_ms, last_visit_ms, max_depth, queries_json
         FROM search_trails
         WHERE profile_id = ?1
           AND (
             ?2 IS NULL
             OR first_visit_ms > ?2
             OR (first_visit_ms = ?2 AND trail_id > ?3)
           )
         ORDER BY first_visit_ms ASC, trail_id ASC
         LIMIT ?4",
    )?;
    statement
        .query_map(
            params![
                profile_id,
                after.map(|cursor| cursor.first_visit_ms),
                after.map(|cursor| cursor.trail_id.as_str()),
                limit.max(1) as i64
            ],
            |row| {
                let queries_json: String = row.get(12)?;
                Ok(TrailRecord {
                    trail_id: row.get(0)?,
                    profile_id: row.get(1)?,
                    session_id: row.get(2)?,
                    initial_query: row.get(3)?,
                    search_engine: row.get(4)?,
                    reformulation_count: row.get(5)?,
                    visit_count: row.get(6)?,
                    landing_url: row.get(7)?,
                    landing_domain: row.get(8)?,
                    first_visit_ms: row.get(9)?,
                    last_visit_ms: row.get(10)?,
                    max_depth: row.get(11)?,
                    queries: serde_json::from_str(&queries_json).unwrap_or_default(),
                    members: Vec::new(),
                })
            },
        )?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(Into::into)
}

#[cfg(test)]
fn load_profile_trails(connection: &Connection, profile_id: &str) -> Result<Vec<TrailRecord>> {
    let mut cursor = None::<TrailBatchCursor>;
    let mut trails = Vec::new();
    loop {
        let batch = load_profile_trail_batch(
            connection,
            profile_id,
            cursor.as_ref(),
            STRUCTURAL_AGGREGATE_BATCH_SIZE,
        )?;
        if batch.is_empty() {
            break;
        }
        cursor = batch.last().map(|trail| TrailBatchCursor {
            first_visit_ms: trail.first_visit_ms,
            trail_id: trail.trail_id.clone(),
        });
        trails.extend(batch);
    }
    Ok(trails)
}

fn build_query_families_from_batches(
    connection: &Connection,
    profile_id: &str,
) -> Result<Vec<QueryFamilyRecord>> {
    let mut accumulator = QueryFamilyAccumulator::default();
    let mut cursor = None;
    loop {
        let batch = load_profile_search_event_batch(
            connection,
            profile_id,
            cursor,
            STRUCTURAL_AGGREGATE_BATCH_SIZE,
        )?;
        if batch.is_empty() {
            break;
        }
        for event in &batch {
            accumulator.add_event(event);
        }
        cursor = batch.last().map(|event| SearchEventBatchCursor { visit_id: event.visit_id });
    }
    Ok(accumulator.finish())
}

fn build_structural_profile_aggregates_from_batches(
    connection: &Connection,
    profile_id: &str,
) -> Result<(Vec<RefindPageRecord>, Vec<PathFlowRecord>, Vec<HabitPatternRecord>)> {
    let mut accumulator = StructuralAggregateAccumulator::default();
    let mut cursor = None;
    loop {
        let batch = load_profile_derived_visit_batch(
            connection,
            profile_id,
            cursor,
            STRUCTURAL_AGGREGATE_BATCH_SIZE,
        )?;
        if batch.is_empty() {
            break;
        }
        for visit in &batch {
            accumulator.add_visit(visit);
        }
        cursor = batch.last().map(|visit| DerivedVisitBatchCursor {
            visit_time_ms: visit.visit_time_ms,
            visit_id: visit.visit_id,
        });
    }
    Ok(accumulator.finish())
}

fn load_seen_domains(connection: &Connection, profile_id: &str) -> Result<HashSet<String>> {
    let mut statement = connection.prepare(
        "SELECT registrable_domain
         FROM visit_derived_facts
         WHERE profile_id = ?1",
    )?;
    statement
        .query_map([profile_id], |row| row.get::<_, String>(0))?
        .collect::<rusqlite::Result<HashSet<_>>>()
        .map_err(Into::into)
}

fn compute_is_new_domain_with_seen(visits: &mut [VisitRecord], seen_domains: &mut HashSet<String>) {
    for visit in visits {
        visit.is_new_domain = seen_domains.insert(visit.registrable_domain.clone());
    }
}

fn unique_date_keys(visits: &[VisitRecord]) -> Vec<String> {
    visits
        .iter()
        .map(|visit| local_date_key(visit.visit_time_ms))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn persist_visit_derived_facts(
    connection: &Connection,
    visits: &[VisitRecord],
    computed_at: &str,
) -> Result<()> {
    let tx = connection.unchecked_transaction()?;
    let mut statement = tx.prepare(
        "INSERT INTO visit_derived_facts (
           visit_id, profile_id, session_id, trail_id, registrable_domain, canonical_url,
           domain_category, page_category, search_engine, search_query, is_new_domain,
           is_search_event, evidence_tier, taxonomy_source, taxonomy_pack, taxonomy_version,
           computed_at
         )
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17)
         ON CONFLICT(visit_id) DO UPDATE SET
           profile_id = excluded.profile_id,
           session_id = excluded.session_id,
           trail_id = excluded.trail_id,
           registrable_domain = excluded.registrable_domain,
           canonical_url = excluded.canonical_url,
           domain_category = excluded.domain_category,
           page_category = excluded.page_category,
           search_engine = excluded.search_engine,
           search_query = excluded.search_query,
           is_new_domain = excluded.is_new_domain,
           is_search_event = excluded.is_search_event,
           evidence_tier = excluded.evidence_tier,
           taxonomy_source = excluded.taxonomy_source,
           taxonomy_pack = excluded.taxonomy_pack,
           taxonomy_version = excluded.taxonomy_version,
           computed_at = excluded.computed_at",
    )?;
    for visit in visits {
        statement.execute(params![
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
        ])?;
    }
    drop(statement);
    tx.commit()?;
    Ok(())
}

fn replace_daily_rollups(
    connection: &Connection,
    profile_id: &str,
    dirty_date_keys: Option<&[String]>,
    rollups: &DailyRollupBundle,
) -> Result<()> {
    let tx = connection.unchecked_transaction()?;
    if let Some(date_keys) = dirty_date_keys {
        for date_key in date_keys {
            tx.execute(
                "DELETE FROM domain_daily_rollups WHERE profile_id = ?1 AND date_key = ?2",
                params![profile_id, date_key],
            )?;
            tx.execute(
                "DELETE FROM category_daily_rollups WHERE profile_id = ?1 AND date_key = ?2",
                params![profile_id, date_key],
            )?;
            tx.execute(
                "DELETE FROM engine_daily_rollups WHERE profile_id = ?1 AND date_key = ?2",
                params![profile_id, date_key],
            )?;
            tx.execute(
                "DELETE FROM daily_summary_rollups WHERE profile_id = ?1 AND date_key = ?2",
                params![profile_id, date_key],
            )?;
        }
    } else {
        tx.execute("DELETE FROM domain_daily_rollups WHERE profile_id = ?1", [profile_id])?;
        tx.execute("DELETE FROM category_daily_rollups WHERE profile_id = ?1", [profile_id])?;
        tx.execute("DELETE FROM engine_daily_rollups WHERE profile_id = ?1", [profile_id])?;
        tx.execute("DELETE FROM daily_summary_rollups WHERE profile_id = ?1", [profile_id])?;
    }

    ensure_unique_domain_rollup_rows(&rollups.domain_rows)?;
    for row in &rollups.domain_rows {
        tx.execute(
            "INSERT INTO domain_daily_rollups
             (date_key, profile_id, registrable_domain, domain_category, visit_count, search_count, new_domain_visits, unique_urls)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![row.0, row.1, row.2, row.3, row.4, row.5, row.6, row.7],
        )?;
    }
    for row in &rollups.category_rows {
        tx.execute(
            "INSERT INTO category_daily_rollups
             (date_key, profile_id, domain_category, visit_count, unique_domains)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![row.0, row.1, row.2, row.3, row.4],
        )?;
    }
    for row in &rollups.engine_rows {
        tx.execute(
            "INSERT INTO engine_daily_rollups
             (date_key, profile_id, search_engine, search_count)
             VALUES (?1, ?2, ?3, ?4)",
            params![row.0, row.1, row.2, row.3],
        )?;
    }
    for row in &rollups.summary_rows {
        tx.execute(
            "INSERT INTO daily_summary_rollups
             (date_key, profile_id, total_visits, total_searches, new_domains, unique_domains, hhi_score, discovery_rate)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![row.0, row.1, row.2, row.3, row.4, row.5, row.6, row.7],
        )?;
    }
    tx.commit()?;
    Ok(())
}

fn ensure_unique_domain_rollup_rows(
    domain_rows: &[(String, String, String, String, i64, i64, i64, i64)],
) -> Result<()> {
    let mut seen = HashSet::<(String, String, String)>::new();
    for (date_key, profile_id, registrable_domain, _, _, _, _, _) in domain_rows {
        let key = (date_key.clone(), profile_id.clone(), registrable_domain.clone());
        if !seen.insert(key.clone()) {
            anyhow::bail!(
                "duplicate domain daily rollup row prepared for {} / {} / {}",
                key.0,
                key.1,
                key.2
            );
        }
    }
    Ok(())
}

#[derive(Debug)]
struct SessionBuildState {
    record: SessionRecord,
    domain_counts: HashMap<String, usize>,
    first_search_query: Option<String>,
    navigation_chain_depth: i64,
    new_domain_count: i64,
}

impl SessionBuildState {
    fn new(visit: &StructuralVisitRecord) -> Self {
        Self {
            record: SessionRecord {
                session_id: format!("session:{}:{}", visit.profile_id, visit.visit_id),
                profile_id: visit.profile_id.clone(),
                first_visit_ms: visit.visit_time_ms,
                last_visit_ms: visit.visit_time_ms,
                visit_count: 0,
                search_count: 0,
                domain_count: 0,
                is_deep_dive: false,
                auto_title: None,
            },
            domain_counts: HashMap::new(),
            first_search_query: None,
            navigation_chain_depth: 0,
            new_domain_count: 0,
        }
    }

    fn push_visit(&mut self, visit: &StructuralVisitRecord) {
        self.record.last_visit_ms = visit.visit_time_ms;
        self.record.visit_count += 1;
        self.record.search_count += i64::from(visit.is_search_event);
        *self.domain_counts.entry(visit.registrable_domain.clone()).or_default() += 1;
        if self.first_search_query.is_none() {
            self.first_search_query = visit.search_query.clone();
        }
        self.navigation_chain_depth += i64::from(visit.from_visit.is_some());
        self.new_domain_count += i64::from(visit.is_new_domain);
    }

    fn finish(mut self) -> SessionRecord {
        self.record.domain_count = self.domain_counts.len() as i64;
        self.record.auto_title = build_session_title_from_summary(
            &self.domain_counts,
            self.first_search_query.as_deref(),
        );
        self.record.is_deep_dive = self.navigation_chain_depth >= 4
            && self.record.domain_count >= 5
            && self.record.visit_count >= 8
            && self.new_domain_count >= 1;
        self.record
    }
}

#[derive(Debug)]
struct TrailBuildState {
    record: TrailRecord,
    next_ordinal: i64,
}

impl TrailBuildState {
    fn new(visit: &StructuralVisitRecord, session_id: &str) -> Self {
        let query = visit.search_query.clone().unwrap_or_else(|| "search".to_string());
        let trail_id = format!("trail:{}:{}", visit.profile_id, visit.visit_id);
        Self {
            record: TrailRecord {
                trail_id,
                profile_id: visit.profile_id.clone(),
                session_id: session_id.to_string(),
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
                members: Vec::new(),
            },
            next_ordinal: 1,
        }
    }

    fn search_event_member(&self, visit_id: i64) -> TrailMemberRecord {
        TrailMemberRecord {
            trail_id: self.record.trail_id.clone(),
            profile_id: self.record.profile_id.clone(),
            visit_id,
            ordinal: 0,
            role: "search_event".to_string(),
        }
    }

    fn append_visit(&mut self, visit: &StructuralVisitRecord) -> TrailMemberRecord {
        let ordinal = self.next_ordinal;
        self.next_ordinal += 1;
        let role = if self.record.landing_url.is_none() { "landing" } else { "click" };
        self.record.visit_count += 1;
        self.record.last_visit_ms = visit.visit_time_ms;
        self.record.max_depth = self.record.max_depth.max(ordinal);
        self.record.landing_url.get_or_insert_with(|| visit.url.clone());
        self.record.landing_domain.get_or_insert_with(|| visit.registrable_domain.clone());
        TrailMemberRecord {
            trail_id: self.record.trail_id.clone(),
            profile_id: self.record.profile_id.clone(),
            visit_id: visit.visit_id,
            ordinal,
            role: role.to_string(),
        }
    }

    fn finish(self) -> TrailRecord {
        self.record
    }
}

struct StructuralTailPersist<'tx> {
    assignment_statement: rusqlite::Statement<'tx>,
    session_statement: rusqlite::Statement<'tx>,
    trail_statement: rusqlite::Statement<'tx>,
    trail_member_statement: rusqlite::Statement<'tx>,
    search_event_statement: rusqlite::Statement<'tx>,
    search_event_kind_statement: rusqlite::Statement<'tx>,
    search_event_term_delete_statement: rusqlite::Statement<'tx>,
    search_term_statement: rusqlite::Statement<'tx>,
}

impl<'tx> StructuralTailPersist<'tx> {
    fn new(tx: &'tx rusqlite::Transaction<'tx>) -> Result<Self> {
        Ok(Self {
            assignment_statement: tx.prepare(
                "UPDATE visit_derived_facts
                 SET session_id = ?2, trail_id = ?3, computed_at = ?4
                 WHERE visit_id = ?1",
            )?,
            session_statement: tx.prepare(
                "INSERT INTO sessions
                 (session_id, profile_id, first_visit_ms, last_visit_ms, visit_count, search_count, domain_count, is_deep_dive, auto_title, computed_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            )?,
            trail_statement: tx.prepare(
                "INSERT INTO search_trails
                 (trail_id, profile_id, session_id, initial_query, search_engine, reformulation_count, visit_count,
                  landing_url, landing_domain, first_visit_ms, last_visit_ms, max_depth, queries_json, computed_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
            )?,
            trail_member_statement: tx.prepare(
                "INSERT INTO search_trail_members (trail_id, profile_id, visit_id, ordinal, role)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
            )?,
            search_event_statement: tx.prepare(
                "INSERT INTO search_events
                 (visit_id, profile_id, search_engine, raw_query, normalized_query, query_kind, trail_id, computed_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            )?,
            search_event_kind_statement: tx.prepare(
                "UPDATE search_events SET query_kind = ?2 WHERE visit_id = ?1",
            )?,
            search_event_term_delete_statement: tx.prepare(
                "DELETE FROM search_event_terms WHERE visit_id = ?1",
            )?,
            search_term_statement: tx.prepare(
                "INSERT INTO search_event_terms (visit_id, profile_id, term)
                 VALUES (?1, ?2, ?3)",
            )?,
        })
    }

    fn persist_assignment(
        &mut self,
        visit_id: i64,
        session_id: &str,
        trail_id: Option<&str>,
        computed_at: &str,
    ) -> Result<()> {
        self.assignment_statement.execute(params![visit_id, session_id, trail_id, computed_at])?;
        Ok(())
    }

    fn persist_session(&mut self, session: &SessionRecord, computed_at: &str) -> Result<()> {
        self.session_statement.execute(params![
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
        ])?;
        Ok(())
    }

    fn persist_trail(&mut self, trail: &TrailRecord, computed_at: &str) -> Result<()> {
        self.trail_statement.execute(params![
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
        ])?;
        Ok(())
    }

    fn persist_trail_member(&mut self, member: &TrailMemberRecord) -> Result<()> {
        self.trail_member_statement.execute(params![
            member.trail_id,
            member.profile_id,
            member.visit_id,
            member.ordinal,
            member.role,
        ])?;
        Ok(())
    }

    fn persist_search_event(&mut self, event: &SearchEventRecord, computed_at: &str) -> Result<()> {
        self.search_event_statement.execute(params![
            event.visit_id,
            event.profile_id,
            event.search_engine,
            event.raw_query,
            event.normalized_query,
            event.query_kind.as_str(),
            event.trail_id,
            computed_at,
        ])?;
        if event.query_kind.is_keyword() {
            for term in
                tokenize_query_terms(&event.normalized_query).into_iter().collect::<BTreeSet<_>>()
            {
                self.search_term_statement.execute(params![
                    event.visit_id,
                    event.profile_id,
                    term
                ])?;
            }
        }
        Ok(())
    }

    fn update_search_event_kind(
        &mut self,
        visit_id: i64,
        profile_id: &str,
        normalized_query: &str,
        query_kind: SearchQueryKind,
    ) -> Result<()> {
        self.search_event_kind_statement.execute(params![visit_id, query_kind.as_str()])?;
        self.search_event_term_delete_statement.execute([visit_id])?;
        if query_kind.is_keyword() {
            for term in tokenize_query_terms(normalized_query).into_iter().collect::<BTreeSet<_>>()
            {
                self.search_term_statement.execute(params![visit_id, profile_id, term])?;
            }
        }
        Ok(())
    }
}

#[derive(Default)]
struct StructuralTailStreamState {
    current_session: Option<SessionBuildState>,
    current_trail: Option<TrailBuildState>,
    report: StructuralTailStreamReport,
}

impl StructuralTailStreamState {
    fn process_visit(
        &mut self,
        visit: StructuralVisitRecord,
        computed_at: &str,
        persist: &mut StructuralTailPersist<'_>,
    ) -> Result<()> {
        self.report.processed_visits += 1;
        self.report.first_visit_ms.get_or_insert(visit.visit_time_ms);

        let starts_new_session = self.current_session.as_ref().is_none_or(|session| {
            visit.visit_time_ms - session.record.last_visit_ms > SESSION_GAP_MS
        });
        if starts_new_session {
            self.finish_trail(computed_at, persist)?;
            self.finish_session(computed_at, persist)?;
            self.current_session = Some(SessionBuildState::new(&visit));
        }

        let session = self.current_session.as_mut().expect("structural session");
        session.push_visit(&visit);
        let session_id = session.record.session_id.clone();
        let mut trail_id = None::<String>;

        if visit.is_search_event {
            self.finish_trail(computed_at, persist)?;
            let trail = TrailBuildState::new(&visit, &session_id);
            let raw_query = visit.search_query.clone().unwrap_or_default();
            let normalized_query = normalize_query(&raw_query);
            let event = SearchEventRecord {
                visit_id: visit.visit_id,
                profile_id: visit.profile_id.clone(),
                search_engine: trail.record.search_engine.clone(),
                raw_query: raw_query.clone(),
                normalized_query: normalized_query.clone(),
                query_kind: classify_search_query_kind(&raw_query, &normalized_query, None),
                trail_id: Some(trail.record.trail_id.clone()),
                visit_time_ms: visit.visit_time_ms,
            };
            persist.persist_search_event(&event, computed_at)?;
            persist.persist_trail_member(&trail.search_event_member(visit.visit_id))?;
            trail_id = Some(trail.record.trail_id.clone());
            self.current_trail = Some(trail);
        } else if let Some(trail) = self.current_trail.as_mut() {
            if trail.record.session_id != session_id
                || visit.visit_time_ms - trail.record.last_visit_ms > TRAIL_GAP_MS
            {
                self.finish_trail(computed_at, persist)?;
            } else {
                let member = trail.append_visit(&visit);
                trail_id = Some(trail.record.trail_id.clone());
                persist.persist_trail_member(&member)?;
            }
        }

        persist.persist_assignment(
            visit.visit_id,
            &session_id,
            trail_id.as_deref(),
            computed_at,
        )?;
        Ok(())
    }

    fn finish_trail(
        &mut self,
        computed_at: &str,
        persist: &mut StructuralTailPersist<'_>,
    ) -> Result<()> {
        if let Some(trail) = self.current_trail.take() {
            if let Some(search_visit_id) =
                trail.record.members.first().map(|member| member.visit_id)
            {
                let normalized_query = normalize_query(&trail.record.initial_query);
                let query_kind = classify_search_query_kind(
                    &trail.record.initial_query,
                    &normalized_query,
                    trail.record.landing_domain.as_deref(),
                );
                persist.update_search_event_kind(
                    search_visit_id,
                    &trail.record.profile_id,
                    &normalized_query,
                    query_kind,
                )?;
            }
            persist.persist_trail(&trail.finish(), computed_at)?;
            self.report.trails += 1;
        }
        Ok(())
    }

    fn finish_session(
        &mut self,
        computed_at: &str,
        persist: &mut StructuralTailPersist<'_>,
    ) -> Result<()> {
        if let Some(session) = self.current_session.take() {
            persist.persist_session(&session.finish(), computed_at)?;
            self.report.sessions += 1;
        }
        Ok(())
    }

    fn finish(&mut self, computed_at: &str, persist: &mut StructuralTailPersist<'_>) -> Result<()> {
        self.finish_trail(computed_at, persist)?;
        self.finish_session(computed_at, persist)
    }
}

fn clear_structural_tail_state(
    tx: &rusqlite::Transaction<'_>,
    profile_id: &str,
    start_ms: Option<i64>,
) -> Result<()> {
    if let Some(start_ms) = start_ms {
        tx.execute(
            "DELETE FROM sessions WHERE profile_id = ?1 AND last_visit_ms >= ?2",
            params![profile_id, start_ms],
        )?;
        tx.execute(
            "DELETE FROM search_trails WHERE profile_id = ?1 AND last_visit_ms >= ?2",
            params![profile_id, start_ms],
        )?;
        delete_structural_memberships_in_range(tx, profile_id, start_ms)?;
    } else {
        tx.execute("DELETE FROM sessions WHERE profile_id = ?1", [profile_id])?;
        tx.execute("DELETE FROM search_trails WHERE profile_id = ?1", [profile_id])?;
        tx.execute("DELETE FROM search_trail_members WHERE profile_id = ?1", [profile_id])?;
        tx.execute("DELETE FROM search_event_terms WHERE profile_id = ?1", [profile_id])?;
        tx.execute("DELETE FROM search_events WHERE profile_id = ?1", [profile_id])?;
    }
    Ok(())
}

fn rebuild_structural_tail_state(
    connection: &Connection,
    profile_id: &str,
    start_ms: Option<i64>,
    computed_at: &str,
    batch_size: usize,
) -> Result<StructuralTailStreamReport> {
    let source_profile_id = load_archive_source_profile_id(connection, profile_id)?;
    let tx = connection.unchecked_transaction()?;
    clear_structural_tail_state(&tx, profile_id, start_ms)?;
    let mut persist = StructuralTailPersist::new(&tx)?;
    let mut state = StructuralTailStreamState::default();
    let mut cursor = None::<StructuralVisitBatchCursor>;

    loop {
        let batch = load_structural_visit_batch(
            &tx,
            profile_id,
            source_profile_id,
            start_ms,
            cursor,
            batch_size,
        )?;
        if batch.is_empty() {
            break;
        }
        let next_cursor = batch.last().map(|visit| StructuralVisitBatchCursor {
            visit_time_ms: visit.visit_time_ms,
            visit_id: visit.visit_id,
        });
        for visit in batch {
            state.process_visit(visit, computed_at, &mut persist)?;
        }
        cursor = next_cursor;
    }

    state.finish(computed_at, &mut persist)?;
    drop(persist);
    tx.commit()?;
    Ok(state.report)
}

fn replace_query_families(
    connection: &Connection,
    profile_id: &str,
    query_families: &[QueryFamilyRecord],
    computed_at: &str,
) -> Result<()> {
    let tx = connection.unchecked_transaction()?;
    tx.execute("DELETE FROM query_families WHERE profile_id = ?1", [profile_id])?;
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
    tx.commit()?;
    Ok(())
}

fn replace_structural_profile_aggregates(
    connection: &Connection,
    profile_id: &str,
    refind_pages: &[RefindPageRecord],
    source_effectiveness: &[SourceEffectivenessRecord],
    habits: &[HabitPatternRecord],
    reopened: &[ReopenedInvestigationRecord],
    path_flows: &[PathFlowRecord],
    computed_at: &str,
) -> Result<()> {
    let tx = connection.unchecked_transaction()?;
    tx.execute("DELETE FROM refind_pages WHERE profile_id = ?1", [profile_id])?;
    tx.execute("DELETE FROM source_effectiveness WHERE profile_id = ?1", [profile_id])?;
    tx.execute("DELETE FROM habit_patterns WHERE profile_id = ?1", [profile_id])?;
    tx.execute("DELETE FROM reopened_investigations WHERE profile_id = ?1", [profile_id])?;
    tx.execute("DELETE FROM path_flows WHERE profile_id = ?1", [profile_id])?;

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
                flow.last_seen_ms,
            ],
        )?;
    }
    tx.commit()?;
    Ok(())
}

fn delete_structural_memberships_in_range(
    tx: &rusqlite::Transaction<'_>,
    profile_id: &str,
    start_ms: i64,
) -> Result<()> {
    let membership_filter = "SELECT visit_derived_facts.visit_id
         FROM visit_derived_facts
         JOIN archive.visits ON archive.visits.id = visit_derived_facts.visit_id
         WHERE visit_derived_facts.profile_id = ?1
           AND archive.visits.reverted_at IS NULL
           AND archive.visits.visit_time_ms >= ?2";
    tx.execute(
        &format!(
            "DELETE FROM search_trail_members
             WHERE profile_id = ?1
               AND visit_id IN ({membership_filter})"
        ),
        params![profile_id, start_ms],
    )?;
    tx.execute(
        &format!(
            "DELETE FROM search_event_terms
             WHERE profile_id = ?1
               AND visit_id IN ({membership_filter})"
        ),
        params![profile_id, start_ms],
    )?;
    tx.execute(
        &format!(
            "DELETE FROM search_events
             WHERE profile_id = ?1
               AND visit_id IN ({membership_filter})"
        ),
        params![profile_id, start_ms],
    )?;
    Ok(())
}

fn expand_structural_rebuild_start(
    connection: &Connection,
    profile_id: &str,
    dirty_from_visit_ms: i64,
) -> Result<i64> {
    let session_start = connection
        .query_row(
            "SELECT MIN(first_visit_ms)
             FROM sessions
             WHERE profile_id = ?1
               AND last_visit_ms >= ?2",
            params![profile_id, dirty_from_visit_ms - SESSION_GAP_MS],
            |row| row.get::<_, Option<i64>>(0),
        )
        .optional()?
        .flatten();
    let trail_start = connection
        .query_row(
            "SELECT MIN(first_visit_ms)
             FROM search_trails
             WHERE profile_id = ?1
               AND last_visit_ms >= ?2",
            params![profile_id, dirty_from_visit_ms - TRAIL_GAP_MS],
            |row| row.get::<_, Option<i64>>(0),
        )
        .optional()?
        .flatten();
    Ok(session_start.into_iter().chain(trail_start).min().unwrap_or(dirty_from_visit_ms))
}

fn local_day_start_ms(date_key: &str) -> Result<i64> {
    let date = NaiveDate::parse_from_str(date_key, "%Y-%m-%d")
        .with_context(|| format!("parsing local date key '{date_key}'"))?;
    let start = date.and_hms_opt(0, 0, 0).context("building local day start")?;
    match Local.from_local_datetime(&start) {
        LocalResult::Single(value) => Ok(value.timestamp_millis()),
        LocalResult::Ambiguous(first, _) => Ok(first.timestamp_millis()),
        LocalResult::None => {
            Err(anyhow::anyhow!("Local timezone could not represent day start for {date_key}."))
        }
    }
}

fn local_day_end_exclusive_ms(date_key: &str) -> Result<i64> {
    let date = NaiveDate::parse_from_str(date_key, "%Y-%m-%d")
        .with_context(|| format!("parsing local date key '{date_key}'"))?;
    let next = date.succ_opt().context("computing next local day for dirty rollup range")?;
    let start = next.and_hms_opt(0, 0, 0).context("building local day end")?;
    match Local.from_local_datetime(&start) {
        LocalResult::Single(value) => Ok(value.timestamp_millis()),
        LocalResult::Ambiguous(first, _) => Ok(first.timestamp_millis()),
        LocalResult::None => Err(anyhow::anyhow!(
            "Local timezone could not represent next day start for {date_key}."
        )),
    }
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

fn load_archive_source_profile_id(connection: &Connection, profile_id: &str) -> Result<i64> {
    connection
        .query_row(
            "SELECT id
             FROM archive.source_profiles
             WHERE profile_key = ?1
             LIMIT 1",
            [profile_id],
            |row| row.get(0),
        )
        .with_context(|| format!("loading source_profile_id for {profile_id}"))
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
    let overrides = load_site_dictionary_overrides(connection)?;
    let search_rules = load_enabled_search_engine_rules(connection)?;
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
            &overrides,
            &search_rules,
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
    let mut current: Option<SessionBuildState> = None;
    for visit in visits.iter_mut() {
        let start_new = current.as_ref().is_none_or(|session| {
            visit.visit_time_ms - session.record.last_visit_ms > SESSION_GAP_MS
        });
        if start_new {
            if let Some(session) = current.take() {
                sessions.push(session.finish());
            }
            current = Some(SessionBuildState::new(&StructuralVisitRecord {
                visit_id: visit.visit_id,
                profile_id: visit.profile_id.clone(),
                url: visit.url.clone(),
                visit_time_ms: visit.visit_time_ms,
                from_visit: visit.from_visit,
                registrable_domain: visit.registrable_domain.clone(),
                search_engine: visit.search_engine.clone(),
                search_query: visit.search_query.clone(),
                is_new_domain: visit.is_new_domain,
                is_search_event: visit.is_search_event,
            }));
        }
        let session = current.as_mut().expect("current session");
        session.push_visit(&StructuralVisitRecord {
            visit_id: visit.visit_id,
            profile_id: visit.profile_id.clone(),
            url: visit.url.clone(),
            visit_time_ms: visit.visit_time_ms,
            from_visit: visit.from_visit,
            registrable_domain: visit.registrable_domain.clone(),
            search_engine: visit.search_engine.clone(),
            search_query: visit.search_query.clone(),
            is_new_domain: visit.is_new_domain,
            is_search_event: visit.is_search_event,
        });
        visit.session_id = Some(session.record.session_id.clone());
    }
    if let Some(session) = current.take() {
        sessions.push(session.finish());
    }
    sessions
}

fn build_session_title_from_summary(
    domain_counts: &HashMap<String, usize>,
    first_search_query: Option<&str>,
) -> Option<String> {
    let top_domain = domain_counts
        .iter()
        .max_by(|left, right| left.1.cmp(right.1).then_with(|| right.0.cmp(left.0)))
        .map(|(domain, _)| display_name_for_domain(domain).unwrap_or_else(|| domain.clone()));
    match (top_domain, first_search_query) {
        (Some(domain), Some(query)) => Some(format!("{domain} · {query}")),
        (Some(domain), None) => Some(domain),
        (None, Some(query)) => Some(query.to_string()),
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
                query_kind: SearchQueryKind::Keyword,
                trail_id: None,
                visit_time_ms: visit.visit_time_ms,
            });
            if let Some(trail) = current.take() {
                trails.push(trail);
            }
            let query = visit_query.unwrap_or_else(|| "search".to_string());
            let trail_id = format!("trail:{}:{}", visit.profile_id, visit.visit_id);
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

    let landing_domains = trails
        .iter()
        .map(|trail| (trail.trail_id.clone(), trail.landing_domain.clone()))
        .collect::<HashMap<_, _>>();
    for event in &mut search_events {
        event.query_kind = classify_search_query_kind(
            &event.raw_query,
            &event.normalized_query,
            event
                .trail_id
                .as_ref()
                .and_then(|trail_id| landing_domains.get(trail_id))
                .and_then(|domain| domain.as_deref()),
        );
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
    for event in events.iter().filter(|event| event.query_kind.is_keyword()) {
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

impl QueryFamilyAccumulator {
    fn add_event(&mut self, event: &SearchEventRecord) {
        let tokens = query_token_set(&event.normalized_query);
        if tokens.is_empty() {
            return;
        }
        for family in &mut self.families {
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
                return;
            }
        }
        self.families.push(QueryFamilyRecord {
            family_id: format!("family:{}:{:04}", event.profile_id, self.families.len() + 1),
            profile_id: event.profile_id.clone(),
            anchor_query: event.raw_query.clone(),
            member_count: 1,
            search_engine: event.search_engine.clone(),
            first_seen_ms: event.visit_time_ms,
            last_seen_ms: event.visit_time_ms,
            queries: vec![event.raw_query.clone()],
        });
    }

    fn finish(self) -> Vec<QueryFamilyRecord> {
        self.families
    }
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
        if let Some(domain) = &trail.landing_domain {
            trail_counts.entry(domain.clone()).or_default().insert(trail.trail_id.clone());
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

fn build_source_effectiveness_from_database(
    connection: &Connection,
    profile_id: &str,
    refind_pages: &[RefindPageRecord],
) -> Result<Vec<SourceEffectivenessRecord>> {
    let mut landing_counts = HashMap::<String, i64>::new();
    let mut trail_counts = HashMap::<String, i64>::new();
    let mut first_seen = HashMap::<String, i64>::new();
    let mut last_seen = HashMap::<String, i64>::new();
    let mut cursor = None::<TrailBatchCursor>;

    loop {
        let batch = load_profile_trail_batch(
            connection,
            profile_id,
            cursor.as_ref(),
            STRUCTURAL_AGGREGATE_BATCH_SIZE,
        )?;
        if batch.is_empty() {
            break;
        }
        for trail in &batch {
            if let Some(domain) = &trail.landing_domain {
                *landing_counts.entry(domain.clone()).or_default() += 1;
                *trail_counts.entry(domain.clone()).or_default() += 1;
                first_seen
                    .entry(domain.clone())
                    .and_modify(|value| *value = (*value).min(trail.first_visit_ms))
                    .or_insert(trail.first_visit_ms);
                last_seen
                    .entry(domain.clone())
                    .and_modify(|value| *value = (*value).max(trail.last_visit_ms))
                    .or_insert(trail.last_visit_ms);
            }
        }
        cursor = batch.last().map(|trail| TrailBatchCursor {
            first_visit_ms: trail.first_visit_ms,
            trail_id: trail.trail_id.clone(),
        });
    }

    let reference_counts =
        refind_pages.iter().fold(HashMap::<String, i64>::new(), |mut acc, page| {
            *acc.entry(page.registrable_domain.clone()).or_default() += 1;
            acc
        });
    let domains =
        landing_counts.keys().chain(reference_counts.keys()).cloned().collect::<BTreeSet<_>>();

    Ok(domains
        .into_iter()
        .map(|domain| {
            let stable_landing_count = *landing_counts.get(&domain).unwrap_or(&0);
            let reference_count = *reference_counts.get(&domain).unwrap_or(&0);
            let trail_count = *trail_counts.get(&domain).unwrap_or(&0);
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
                profile_id: profile_id.to_string(),
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
        .collect())
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
        for step_count in [2_usize, 3_usize, 4_usize] {
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

impl StructuralAggregateAccumulator {
    fn add_visit(&mut self, visit: &VisitRecord) {
        self.profile_id.get_or_insert_with(|| visit.profile_id.clone());
        self.record_refind_page(visit);
        self.record_path_flow(visit);
        self.record_habit_day(visit);
    }

    fn finish(mut self) -> (Vec<RefindPageRecord>, Vec<PathFlowRecord>, Vec<HabitPatternRecord>) {
        if let (Some(profile_id), false) =
            (self.current_profile_id.as_deref(), self.current_sequence.is_empty())
        {
            flush_path_flow_sequence(&mut self.flow_counts, profile_id, &self.current_sequence);
        }
        let StructuralAggregateAccumulator {
            profile_id,
            refind_pages,
            flow_counts,
            current_session_id: _,
            current_profile_id: _,
            current_sequence: _,
            habit_days,
            last_visit_ms,
        } = self;
        (
            finish_refind_pages(refind_pages),
            finish_path_flows(flow_counts),
            finish_habits(profile_id, habit_days, last_visit_ms),
        )
    }

    fn record_refind_page(&mut self, visit: &VisitRecord) {
        if visit.is_search_event {
            return;
        }
        let entry = self.refind_pages.entry(visit.canonical_url.clone()).or_insert_with(|| {
            RefindAccumulatorEntry {
                profile_id: visit.profile_id.clone(),
                canonical_url: visit.canonical_url.clone(),
                url: visit.url.clone(),
                title: visit.title.clone(),
                registrable_domain: visit.registrable_domain.clone(),
                first_seen_ms: visit.visit_time_ms,
                last_seen_ms: visit.visit_time_ms,
                ..RefindAccumulatorEntry::default()
            }
        });
        entry.distinct_days.insert(local_date_key(visit.visit_time_ms));
        if let Some(trail_id) = &visit.trail_id {
            entry.trail_ids.insert(trail_id.clone());
            entry.search_arrival_count += 1;
        }
        if visit.from_visit.is_none() {
            entry.typed_revisit_count += 1;
        }
        entry.first_seen_ms = entry.first_seen_ms.min(visit.visit_time_ms);
        entry.last_seen_ms = entry.last_seen_ms.max(visit.visit_time_ms);
        entry.visit_ids.push(visit.visit_id);
    }

    fn record_path_flow(&mut self, visit: &VisitRecord) {
        let session_id = visit
            .session_id
            .clone()
            .unwrap_or_else(|| format!("sessionless:{}:{}", visit.profile_id, visit.visit_id));
        if self.current_session_id.as_deref() != Some(session_id.as_str()) {
            if let (Some(profile_id), false) =
                (self.current_profile_id.as_deref(), self.current_sequence.is_empty())
            {
                flush_path_flow_sequence(&mut self.flow_counts, profile_id, &self.current_sequence);
            }
            self.current_session_id = Some(session_id);
            self.current_profile_id = Some(visit.profile_id.clone());
            self.current_sequence.clear();
        }
        if self
            .current_sequence
            .last()
            .is_none_or(|(last_domain, _)| *last_domain != visit.registrable_domain)
        {
            self.current_sequence.push((visit.registrable_domain.clone(), visit.visit_time_ms));
        } else if let Some((_, last_seen_ms)) = self.current_sequence.last_mut() {
            *last_seen_ms = visit.visit_time_ms;
        }
    }

    fn record_habit_day(&mut self, visit: &VisitRecord) {
        self.habit_days
            .entry(visit.registrable_domain.clone())
            .or_default()
            .insert(local_datetime_from_millis(visit.visit_time_ms).date_naive());
        self.last_visit_ms
            .entry(visit.registrable_domain.clone())
            .and_modify(|value| *value = (*value).max(visit.visit_time_ms))
            .or_insert(visit.visit_time_ms);
    }
}

fn finish_refind_pages(
    refind_pages: HashMap<String, RefindAccumulatorEntry>,
) -> Vec<RefindPageRecord> {
    refind_pages
            .into_values()
            .filter_map(|entry| {
                let cross_day_count = entry.distinct_days.len() as i64;
                let trail_count = entry.trail_ids.len() as i64;
                let score = (cross_day_count as f32 * 2.0)
                    + (trail_count as f32 * 1.5)
                    + entry.search_arrival_count as f32
                    + (entry.typed_revisit_count as f32 * 1.2);
                if cross_day_count < 2 && trail_count < 2 && entry.typed_revisit_count < 2 {
                    return None;
                }
                let evidence_json = json!({
                    "factors": [
                        { "signal": "cross_day_count", "rawValue": cross_day_count, "weight": 2.0, "contribution": cross_day_count as f32 * 2.0 },
                        { "signal": "trail_count", "rawValue": trail_count, "weight": 1.5, "contribution": trail_count as f32 * 1.5 },
                        { "signal": "search_arrival_count", "rawValue": entry.search_arrival_count, "weight": 1.0, "contribution": entry.search_arrival_count as f32 },
                        { "signal": "typed_revisit_count", "rawValue": entry.typed_revisit_count, "weight": 1.2, "contribution": entry.typed_revisit_count as f32 * 1.2 }
                    ],
                    "visitIds": entry.visit_ids
                })
                .to_string();
                Some(RefindPageRecord {
                    profile_id: entry.profile_id,
                    canonical_url: entry.canonical_url,
                    url: entry.url,
                    title: entry.title,
                    registrable_domain: entry.registrable_domain,
                    cross_day_count,
                    trail_count,
                    search_arrival_count: entry.search_arrival_count,
                    typed_revisit_count: entry.typed_revisit_count,
                    refind_score: score,
                    evidence_json,
                    first_seen_ms: entry.first_seen_ms,
                    last_seen_ms: entry.last_seen_ms,
                })
            })
            .collect()
}

fn finish_path_flows(
    flow_counts: HashMap<(String, String, i64), (i64, i64)>,
) -> Vec<PathFlowRecord> {
    flow_counts
        .into_iter()
        .map(|((profile_id, flow_pattern, step_count), (occurrence_count, last_seen_ms))| {
            PathFlowRecord { profile_id, flow_pattern, step_count, occurrence_count, last_seen_ms }
        })
        .collect()
}

fn finish_habits(
    profile_id: Option<String>,
    habit_days: HashMap<String, BTreeSet<NaiveDate>>,
    last_visit_ms: HashMap<String, i64>,
) -> Vec<HabitPatternRecord> {
    let Some(profile_id) = profile_id else {
        return Vec::new();
    };
    habit_days
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
            let last_visited_ms = *last_visit_ms.get(&domain).unwrap_or(&0);
            let days_since_last =
                ((Utc::now().timestamp_millis() - last_visited_ms) as f32 / 86_400_000.0).max(0.0);
            Some(HabitPatternRecord {
                profile_id: profile_id.clone(),
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

fn flush_path_flow_sequence(
    flows: &mut HashMap<(String, String, i64), (i64, i64)>,
    profile_id: &str,
    sequence: &[(String, i64)],
) {
    for step_count in [2_usize, 3_usize, 4_usize] {
        if sequence.len() < step_count {
            continue;
        }
        for window in sequence.windows(step_count) {
            let flow_pattern =
                window.iter().map(|(domain, _)| domain.as_str()).collect::<Vec<_>>().join(" → ");
            let last_seen_ms =
                window.iter().map(|(_, visit_time_ms)| *visit_time_ms).max().unwrap_or(0);
            let key = (profile_id.to_string(), flow_pattern, step_count as i64);
            let entry = flows.entry(key).or_insert((0, 0));
            entry.0 += 1;
            entry.1 = entry.1.max(last_seen_ms);
        }
    }
}

fn build_daily_rollups(visits: &[VisitRecord]) -> DailyRollupBundle {
    let mut accumulator = DailyRollupAccumulator::default();
    accumulator.extend(visits.iter());
    accumulator.finish()
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
    job_kind: RebuildMode,
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
    ensure_unique_domain_rollup_rows(&rollups.domain_rows)?;

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
             (visit_id, profile_id, search_engine, raw_query, normalized_query, query_kind, trail_id, computed_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                event.visit_id,
                event.profile_id,
                event.search_engine,
                event.raw_query,
                event.normalized_query,
                event.query_kind.as_str(),
                event.trail_id,
                computed_at,
            ],
        )?;
        if event.query_kind.is_keyword() {
            for term in
                tokenize_query_terms(&event.normalized_query).into_iter().collect::<BTreeSet<_>>()
            {
                tx.execute(
                    "INSERT INTO search_event_terms (visit_id, profile_id, term) VALUES (?1, ?2, ?3)",
                    params![event.visit_id, event.profile_id, term],
                )?;
            }
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
    clear_core_tables_for_job_kind(connection, profile_id, RebuildMode::FullRebuild)
}

fn clear_core_tables_for_job_kind(
    connection: &Connection,
    profile_id: Option<&str>,
    job_kind: RebuildMode,
) -> Result<()> {
    let tables: &[&str] = match job_kind {
        RebuildMode::VisitDerive => &["visit_derived_facts"],
        RebuildMode::DailyRollup => &[
            "domain_daily_rollups",
            "category_daily_rollups",
            "engine_daily_rollups",
            "daily_summary_rollups",
        ],
        RebuildMode::StructuralRebuild => &[
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
        RebuildMode::FullRebuild => &[
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

fn sum_table_row_counts(connection: &Connection, tables: &[&str]) -> Result<usize> {
    tables.iter().try_fold(0_usize, |count, table| {
        table_row_count(connection, table).map(|table_count| count + table_count)
    })
}

fn count_core_intelligence_jobs(connection: &Connection) -> Result<usize> {
    connection
        .query_row(
            "SELECT COUNT(*)
             FROM intelligence_jobs
             WHERE job_type IN ('visit-derive', 'daily-rollup', 'structural-rebuild', 'full-rebuild')",
            [],
            |row| row.get::<_, i64>(0),
        )
        .map(|value| value.max(0) as usize)
        .map_err(Into::into)
}

fn count_core_intelligence_job_triggers(connection: &Connection) -> Result<usize> {
    if !table_exists(connection, "intelligence_job_triggers")? {
        return Ok(0);
    }
    connection
        .query_row(
            "SELECT COUNT(*)
             FROM intelligence_job_triggers
             WHERE job_id IN (
               SELECT id
               FROM intelligence_jobs
               WHERE job_type IN ('visit-derive', 'daily-rollup', 'structural-rebuild', 'full-rebuild')
             )",
            [],
            |row| row.get::<_, i64>(0),
        )
        .map(|value| value.max(0) as usize)
        .map_err(Into::into)
}

pub(super) fn local_date_key(visit_time_ms: i64) -> String {
    local_datetime_from_millis(visit_time_ms).format("%Y-%m-%d").to_string()
}

pub(super) fn rfc3339_from_millis(visit_time_ms: i64) -> String {
    local_datetime_from_millis(visit_time_ms).with_timezone(&Utc).to_rfc3339()
}

fn search_query_sort_sql(sort: Option<&str>) -> &'static str {
    match sort.unwrap_or("newest") {
        "exact-frequency" | "exact_frequency" => {
            "exact_repeat_count DESC, visit_time_ms DESC, search_engine ASC, normalized_query ASC"
        }
        "family-frequency" | "family_frequency" => {
            "exact_repeat_count DESC, visit_time_ms DESC, search_engine ASC, normalized_query ASC"
        }
        "alphabetical" => "normalized_query ASC, visit_time_ms DESC, search_engine ASC",
        _ => "visit_time_ms DESC, search_engine ASC, normalized_query ASC",
    }
}

fn load_trail_context_map(
    connection: &Connection,
    trail_ids: &[String],
) -> Result<HashMap<String, (String, i64)>> {
    if trail_ids.is_empty() {
        return Ok(HashMap::new());
    }
    let unique_ids =
        trail_ids.iter().cloned().collect::<BTreeSet<_>>().into_iter().collect::<Vec<_>>();
    let placeholders = std::iter::repeat_n("?", unique_ids.len()).collect::<Vec<_>>().join(", ");
    let sql = format!(
        "SELECT trail_id, initial_query, reformulation_count
         FROM search_trails
         WHERE trail_id IN ({placeholders})"
    );
    let mut statement = connection.prepare(&sql)?;
    let rows = statement.query_map(rusqlite::params_from_iter(unique_ids.iter()), |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, i64>(2)?))
    })?;
    let mut map = HashMap::new();
    for row in rows {
        let (trail_id, initial_query, reformulation_count) = row?;
        map.insert(trail_id, (initial_query, reformulation_count));
    }
    Ok(map)
}

fn load_query_family_summary_map(
    connection: &Connection,
    profile_id: Option<&str>,
    search_engine: Option<&str>,
    start_ms: i64,
    end_ms: i64,
) -> Result<HashMap<(String, String), (String, i64)>> {
    let mut statement = connection.prepare(
        "SELECT family_id, search_engine, member_count, queries_json
         FROM query_families
         WHERE (?1 IS NULL OR profile_id = ?1)
           AND (?2 IS NULL OR search_engine = ?2)
           AND last_seen_ms >= ?3
           AND first_seen_ms < ?4",
    )?;
    let rows =
        statement.query_map(params![profile_id, search_engine, start_ms, end_ms], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, String>(3)?,
            ))
        })?;
    let mut counts = HashMap::<(String, String), (String, i64)>::new();
    for row in rows {
        let (family_id, engine, member_count, queries_json) = row?;
        let queries = serde_json::from_str::<Vec<String>>(&queries_json).unwrap_or_default();
        for query in queries {
            let normalized = normalize_query(&query);
            counts
                .entry((engine.clone(), normalized))
                .and_modify(|current| {
                    if member_count > current.1 {
                        *current = (family_id.clone(), member_count);
                    }
                })
                .or_insert((family_id.clone(), member_count.max(1)));
        }
    }
    Ok(counts)
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
    let stop_words = [
        "a", "an", "and", "com", "edu", "for", "from", "how", "html", "http", "https", "in", "is",
        "net", "of", "on", "org", "the", "to", "what", "when", "where", "with", "www",
    ];
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

fn classify_search_query_kind(
    raw_query: &str,
    normalized_query: &str,
    landing_domain: Option<&str>,
) -> SearchQueryKind {
    if normalized_query.trim().is_empty() {
        return SearchQueryKind::Navigational;
    }

    if let Some(candidate_domain) =
        query_domain_candidate(raw_query).or_else(|| query_domain_candidate(normalized_query))
    {
        let landing_matches = landing_domain.is_none_or(|domain| {
            domain == candidate_domain
                || landing_domain_matches_candidate(domain, &candidate_domain)
        });
        if landing_matches {
            return SearchQueryKind::Navigational;
        }
    }

    SearchQueryKind::Keyword
}

fn landing_domain_matches_candidate(landing_domain: &str, candidate_domain: &str) -> bool {
    landing_domain == candidate_domain
        || landing_domain.ends_with(&format!(".{candidate_domain}"))
        || candidate_domain.ends_with(&format!(".{landing_domain}"))
}

fn query_domain_candidate(query: &str) -> Option<String> {
    let trimmed = query.trim().trim_matches('"').trim_matches('\'');
    if trimmed.is_empty() || trimmed.contains(char::is_whitespace) {
        return None;
    }

    let parsed = Url::parse(trimmed).ok().or_else(|| {
        if trimmed.contains('.') { Url::parse(&format!("https://{trimmed}")).ok() } else { None }
    })?;
    let host = parsed.host_str()?.trim_end_matches('.').to_ascii_lowercase();
    if host.is_empty() {
        return None;
    }

    Some(crate::deterministic::registrable_domain_for_host(&host))
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
                search_trail_members.role, urls.url, visit_derived_facts.canonical_url, urls.title,
                visit_derived_facts.registrable_domain, visits.visit_time_ms, visit_derived_facts.search_query
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
                canonical_url: row.get(5)?,
                title: row.get(6)?,
                registrable_domain: row.get(7)?,
                visit_time_ms: row.get(8)?,
                search_query: row.get(9)?,
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
    get_search_engine_ranking_with_connection(&connection, request)
}

fn get_search_engine_ranking_with_connection(
    connection: &Connection,
    request: &ScopedDateRangeRequest,
) -> Result<Vec<EngineRanking>> {
    let display_names = load_search_engine_display_names(connection)?;
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
                    display_name: display_name_for_search_engine_with_map(&engine, &display_names),
                    search_engine: engine,
                    search_count: row.get(1)?,
                })
            },
        )?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(Into::into)
}

pub fn list_search_engine_rules_for_settings(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
) -> Result<Vec<SearchEngineRule>> {
    let connection = open_intelligence_connection(paths, config, key)?;
    ensure_core_intelligence_schema(&connection)?;
    list_search_engine_rules(&connection)
}

pub fn upsert_search_engine_rule_for_settings(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    input: &SearchEngineRuleInput,
) -> Result<Vec<SearchEngineRule>> {
    let connection = open_intelligence_connection(paths, config, key)?;
    ensure_core_intelligence_schema(&connection)?;
    upsert_search_engine_rule(&connection, input)
}

pub fn delete_search_engine_rule_for_settings(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    rule_id: &str,
) -> Result<Vec<SearchEngineRule>> {
    let connection = open_intelligence_connection(paths, config, key)?;
    ensure_core_intelligence_schema(&connection)?;
    delete_search_engine_rule(&connection, rule_id)
}

pub fn get_top_search_concepts(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    request: &TopSearchConceptsRequest,
) -> Result<Vec<SearchConcept>> {
    let connection = open_intelligence_connection(paths, config, key)?;
    ensure_core_intelligence_schema(&connection)?;
    get_top_search_concepts_with_connection(&connection, request)
}

fn get_top_search_concepts_with_connection(
    connection: &Connection,
    request: &TopSearchConceptsRequest,
) -> Result<Vec<SearchConcept>> {
    let (start_ms, end_ms) = date_range_bounds(&request.date_range)?;
    let mut statement = connection.prepare(
        "SELECT search_event_terms.term,
                COUNT(*) AS frequency,
                GROUP_CONCAT(DISTINCT search_events.search_engine)
         FROM search_event_terms
         JOIN search_events ON search_events.visit_id = search_event_terms.visit_id
         JOIN archive.visits AS visits ON visits.id = search_events.visit_id
         WHERE (?1 IS NULL OR search_events.profile_id = ?1)
           AND search_events.query_kind = 'keyword'
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

pub fn get_search_queries(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    request: &SearchQueryListRequest,
) -> Result<SearchQueryListResult> {
    let connection = open_intelligence_connection(paths, config, key)?;
    ensure_core_intelligence_schema(&connection)?;
    let display_names = load_search_engine_display_names(&connection)?;
    let (start_ms, end_ms) = date_range_bounds(&request.date_range)?;
    let sort = request.sort.as_deref().unwrap_or("newest");
    let sort_in_rust = matches!(sort, "family-frequency" | "family_frequency");
    let query_filter =
        request.query.as_deref().map(normalize_query).filter(|value| !value.is_empty());
    let total: i64 = connection.query_row(
        "WITH filtered AS (
            SELECT search_events.visit_id,
                   search_events.profile_id,
                   source_profiles.browser_kind,
                   search_events.search_engine,
                   search_events.raw_query,
                   search_events.normalized_query,
                   search_events.trail_id,
                   visits.visit_time_ms
            FROM search_events
            JOIN archive.visits AS visits ON visits.id = search_events.visit_id
            JOIN archive.source_profiles AS source_profiles
              ON source_profiles.profile_key = search_events.profile_id
            JOIN visit_derived_facts ON visit_derived_facts.visit_id = search_events.visit_id
            WHERE (?1 IS NULL OR search_events.profile_id = ?1)
              AND (?2 IS NULL OR source_profiles.browser_kind = ?2)
              AND (?3 IS NULL OR search_events.search_engine = ?3)
              AND (?4 IS NULL OR visit_derived_facts.registrable_domain = ?4)
              AND (?5 IS NULL
                   OR search_events.normalized_query LIKE '%' || ?5 || '%'
                   OR LOWER(search_events.raw_query) LIKE '%' || ?5 || '%')
              AND search_events.query_kind = 'keyword'
              AND visits.visit_time_ms >= ?6
              AND visits.visit_time_ms < ?7
         ),
         ranked AS (
            SELECT filtered.*,
                   ROW_NUMBER() OVER (
                     PARTITION BY filtered.search_engine, filtered.normalized_query
                     ORDER BY filtered.visit_time_ms DESC, filtered.visit_id DESC
                   ) AS row_rank
            FROM filtered
         )
         SELECT COUNT(*) FROM ranked WHERE row_rank = 1",
        params![
            request.profile_id.as_deref(),
            request.browser_kind.as_deref(),
            request.engine.as_deref(),
            request.domain.as_deref(),
            query_filter.as_deref(),
            start_ms,
            end_ms,
        ],
        |row| row.get(0),
    )?;
    let requested_page_size = request.page_size.max(1);
    let offset =
        if sort_in_rust { 0 } else { request.page.saturating_mul(requested_page_size) as i64 };
    let fetch_limit = if sort_in_rust { total.max(1) } else { requested_page_size as i64 };
    let order_by = search_query_sort_sql(request.sort.as_deref());
    let sql = format!(
        "WITH filtered AS (
            SELECT search_events.visit_id,
                   search_events.profile_id,
                   source_profiles.browser_kind,
                   search_events.search_engine,
                   search_events.raw_query,
                   search_events.normalized_query,
                   search_events.trail_id,
                   visits.visit_time_ms
            FROM search_events
            JOIN archive.visits AS visits ON visits.id = search_events.visit_id
            JOIN archive.source_profiles AS source_profiles
              ON source_profiles.profile_key = search_events.profile_id
            JOIN visit_derived_facts ON visit_derived_facts.visit_id = search_events.visit_id
            WHERE (?1 IS NULL OR search_events.profile_id = ?1)
              AND (?2 IS NULL OR source_profiles.browser_kind = ?2)
              AND (?3 IS NULL OR search_events.search_engine = ?3)
              AND (?4 IS NULL OR visit_derived_facts.registrable_domain = ?4)
              AND (?5 IS NULL
                   OR search_events.normalized_query LIKE '%' || ?5 || '%'
                   OR LOWER(search_events.raw_query) LIKE '%' || ?5 || '%')
              AND search_events.query_kind = 'keyword'
              AND visits.visit_time_ms >= ?6
              AND visits.visit_time_ms < ?7
         ),
         ranked AS (
            SELECT filtered.*,
                   COUNT(*) OVER (
                     PARTITION BY filtered.search_engine, filtered.normalized_query
                   ) AS exact_repeat_count,
                   ROW_NUMBER() OVER (
                     PARTITION BY filtered.search_engine, filtered.normalized_query
                     ORDER BY filtered.visit_time_ms DESC, filtered.visit_id DESC
                   ) AS row_rank
            FROM filtered
         )
         SELECT visit_id,
                profile_id,
                browser_kind,
                search_engine,
                raw_query,
                normalized_query,
                trail_id,
                visit_time_ms,
                exact_repeat_count
         FROM ranked
         WHERE row_rank = 1
         ORDER BY {order_by}
         LIMIT ?8 OFFSET ?9"
    );
    let rows = connection
        .prepare(&sql)?
        .query_map(
            params![
                request.profile_id.as_deref(),
                request.browser_kind.as_deref(),
                request.engine.as_deref(),
                request.domain.as_deref(),
                query_filter.as_deref(),
                start_ms,
                end_ms,
                fetch_limit,
                offset,
            ],
            |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, Option<String>>(6)?,
                    row.get::<_, i64>(7)?,
                    row.get::<_, i64>(8)?,
                ))
            },
        )?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let trail_ids = rows
        .iter()
        .filter_map(|(_, _, _, _, _, _, trail_id, _, _)| trail_id.clone())
        .collect::<Vec<_>>();
    let trail_context = load_trail_context_map(&connection, &trail_ids)?;
    let family_summaries = load_query_family_summary_map(
        &connection,
        request.profile_id.as_deref(),
        request.engine.as_deref(),
        start_ms,
        end_ms,
    )?;
    let mut results = rows
        .into_iter()
        .map(
            |(
                visit_id,
                profile_id,
                browser_kind,
                search_engine,
                raw_query,
                normalized_query,
                trail_id,
                visit_time_ms,
                exact_repeat_count,
            )| {
                let trail = trail_id.as_ref().and_then(|value| trail_context.get(value));
                let family_summary =
                    family_summaries.get(&(search_engine.clone(), normalized_query.clone()));
                SearchQueryRow {
                    visit_id,
                    profile_id,
                    browser_kind,
                    display_name: display_name_for_search_engine_with_map(
                        &search_engine,
                        &display_names,
                    ),
                    search_engine: search_engine.clone(),
                    raw_query,
                    normalized_query: normalized_query.clone(),
                    searched_at: rfc3339_from_millis(visit_time_ms),
                    searched_at_ms: visit_time_ms,
                    exact_repeat_count,
                    family_count: family_summary
                        .map(|(_, count)| *count)
                        .unwrap_or(exact_repeat_count.max(1)),
                    family_id: family_summary.map(|(family_id, _)| family_id.clone()),
                    trail_id,
                    trail_initial_query: trail.map(|value| value.0.clone()),
                    trail_reformulation_count: trail.map(|value| value.1),
                }
            },
        )
        .collect::<Vec<_>>();
    if sort_in_rust {
        results.sort_by(|left, right| {
            right
                .family_count
                .cmp(&left.family_count)
                .then_with(|| right.exact_repeat_count.cmp(&left.exact_repeat_count))
                .then_with(|| right.searched_at_ms.cmp(&left.searched_at_ms))
                .then_with(|| left.search_engine.cmp(&right.search_engine))
                .then_with(|| left.normalized_query.cmp(&right.normalized_query))
        });
        let start = request.page.saturating_mul(requested_page_size) as usize;
        results =
            results.into_iter().skip(start).take(requested_page_size as usize).collect::<Vec<_>>();
    }
    Ok(SearchQueryListResult {
        rows: results,
        total,
        page: request.page,
        page_size: requested_page_size,
    })
}

pub fn get_query_families(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    request: &PagedDateRangeRequest,
) -> Result<QueryFamilyResult> {
    let connection = open_intelligence_connection(paths, config, key)?;
    ensure_core_intelligence_schema(&connection)?;
    get_query_families_with_connection(&connection, request)
}

fn get_query_families_with_connection(
    connection: &Connection,
    request: &PagedDateRangeRequest,
) -> Result<QueryFamilyResult> {
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
            query_family_from_row,
        )?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(QueryFamilyResult { families, total, page: request.page, page_size: request.page_size })
}

pub fn get_query_family_detail(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    request: &QueryFamilyDetailRequest,
) -> Result<QueryFamilyDetail> {
    let connection = open_intelligence_connection(paths, config, key)?;
    ensure_core_intelligence_schema(&connection)?;
    let (start_ms, end_ms) = date_range_bounds(&request.date_range)?;
    let (profile_id, family) = load_query_family_detail_row(&connection, request)?;
    let normalized_queries =
        family.queries.iter().map(|query| normalize_query(query)).collect::<HashSet<_>>();
    let mut statement = connection.prepare(
        "SELECT trail_id, session_id, initial_query, search_engine, reformulation_count, visit_count,
                landing_url, landing_domain, first_visit_ms, last_visit_ms, max_depth, queries_json
         FROM search_trails
         WHERE profile_id = ?1
           AND last_visit_ms >= ?2
           AND first_visit_ms < ?3
         ORDER BY last_visit_ms DESC, trail_id DESC",
    )?;
    let related_trails = statement
        .query_map(params![profile_id, start_ms, end_ms], trail_summary_from_row)?
        .collect::<rusqlite::Result<Vec<_>>>()?
        .into_iter()
        .filter(|trail| {
            trail.queries.iter().any(|query| normalized_queries.contains(&normalize_query(query)))
                || normalized_queries.contains(&normalize_query(&trail.initial_query))
        })
        .take(8)
        .collect::<Vec<_>>();
    Ok(QueryFamilyDetail { family, related_trails })
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

fn load_query_family_detail_row(
    connection: &Connection,
    request: &QueryFamilyDetailRequest,
) -> Result<(String, QueryFamily)> {
    connection
        .query_row(
            "SELECT profile_id, family_id, anchor_query, member_count, search_engine, first_seen_ms, last_seen_ms, queries_json
             FROM query_families
             WHERE family_id = ?1
               AND (?2 IS NULL OR profile_id = ?2)",
            params![request.family_id, request.profile_id.as_deref()],
            |row| Ok((row.get::<_, String>(0)?, query_family_from_row_with_offset(row, 1)?)),
        )
        .optional()?
        .with_context(|| format!("query family {} was not found", request.family_id))
}

fn query_family_from_row(row: &Row<'_>) -> rusqlite::Result<QueryFamily> {
    query_family_from_row_with_offset(row, 0)
}

fn query_family_from_row_with_offset(
    row: &Row<'_>,
    offset: usize,
) -> rusqlite::Result<QueryFamily> {
    Ok(QueryFamily {
        family_id: row.get(offset)?,
        anchor_query: row.get(offset + 1)?,
        member_count: row.get(offset + 2)?,
        search_engine: row.get(offset + 3)?,
        first_seen_at: rfc3339_from_millis(row.get(offset + 4)?),
        last_seen_at: rfc3339_from_millis(row.get(offset + 5)?),
        queries: serde_json::from_str(&row.get::<_, String>(offset + 6)?).unwrap_or_default(),
    })
}

pub(super) fn trail_summary_from_row(row: &Row<'_>) -> rusqlite::Result<TrailSummary> {
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

#[cfg(test)]
fn build_kpi(current: i64, previous: i64) -> crate::models::KpiMetric {
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
    crate::models::KpiMetric {
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
           AND visits.reverted_at IS NULL
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
    use super::{
        DAILY_ROLLUP_FALLBACK_BATCH_SIZE, QueryFamilyRecord, STRUCTURAL_TAIL_STREAM_BATCH_SIZE,
        SearchQueryKind, StageRunResult, VISIT_DERIVE_FALLBACK_BATCH_SIZE, build_habit_patterns,
        build_kpi, build_path_flows, build_query_families, build_query_families_from_batches,
        build_refind_pages, build_source_effectiveness, build_source_effectiveness_from_database,
        build_structural_profile_aggregates_from_batches, classify_search_query_kind,
        collapse_date_key, ensure_core_intelligence_schema, explain_entity, get_day_insights,
        get_discovery_trend, get_domain_deep_dive, get_intelligence_embed_cards,
        get_intelligence_primary_overview, get_intelligence_public_snapshot,
        get_intelligence_secondary_overview, get_intelligence_widget_snapshot, get_path_flows,
        get_search_queries, get_top_search_concepts, load_profile_derived_visits,
        load_profile_search_events, load_profile_trails, local_date_key, merge_stage_run_result,
        normalize_query, run_core_intelligence, run_core_intelligence_job_type_with_progress,
    };
    use crate::{
        archive::{
            open_archive_connection, open_intelligence_connection,
            open_intelligence_connection_call_count, open_intelligence_connection_call_sites,
            reset_open_intelligence_connection_call_count,
        },
        config::project_paths_with_root,
        intelligence_catalog::RebuildMode,
        intelligence_runtime::{
            FULL_REBUILD_JOB_TYPE, ensure_intelligence_runtime_schema,
            load_intelligence_runtime_from_connection_call_count,
            reset_load_intelligence_runtime_from_connection_call_count,
        },
        models::{
            AppConfig, ArchiveMode, CoreIntelligenceRebuildRequest, CoreIntelligenceStageTimings,
            DateRange, DayInsightsRequest, DomainDeepDiveRequest, EntityExplanationRequest,
            GranularityDateRangeRequest, IntelligenceEmbedCardsRequest, PathFlowRequest,
            ScopedDateRangeRequest, SearchQueryListRequest, TopSearchConceptsRequest,
        },
        utils::now_rfc3339,
    };
    use rusqlite::{Connection, OptionalExtension, params};
    use std::collections::{BTreeMap, HashSet};

    fn has_index(connection: &Connection, index_name: &str) -> bool {
        connection
            .query_row(
                "SELECT 1
                 FROM sqlite_master
                 WHERE type = 'index'
                   AND name = ?1
                 LIMIT 1",
                [index_name],
                |_| Ok(()),
            )
            .optional()
            .expect("index lookup")
            .is_some()
    }

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

    #[test]
    fn ensure_core_intelligence_schema_records_versioned_migrations() {
        let connection = Connection::open_in_memory().expect("in memory sqlite");
        ensure_core_intelligence_schema(&connection).expect("ensure intelligence schema");
        ensure_core_intelligence_schema(&connection).expect("ensure intelligence schema twice");

        let migration_count: i64 = connection
            .query_row("SELECT COUNT(*) FROM intelligence_schema_migrations", [], |row| row.get(0))
            .expect("migration count");
        assert_eq!(migration_count, 6);
        assert!(has_index(&connection, "idx_vdf_profile_visit_id"));
        assert!(has_index(&connection, "idx_search_trails_profile_time_trail"));
        assert!(has_index(&connection, "idx_search_events_profile_visit"));
        assert!(has_index(&connection, "idx_search_events_profile_kind"));
    }

    #[test]
    fn classifies_url_like_search_queries_as_navigational_noise() {
        assert_eq!(
            classify_search_query_kind("https://asu.edu", "https://asu.edu", Some("asu.edu")),
            SearchQueryKind::Navigational
        );
        assert_eq!(
            classify_search_query_kind("asu.edu", "asu.edu", Some("asu.edu")),
            SearchQueryKind::Navigational
        );
        assert_eq!(
            classify_search_query_kind("pathkeep sqlite", "pathkeep sqlite", Some("github.com")),
            SearchQueryKind::Keyword
        );
    }

    #[test]
    fn primary_overview_reuses_single_connection_and_runtime_snapshot() {
        let root = tempfile::tempdir().expect("tempdir");
        let paths = project_paths_with_root(root.path());
        let config = AppConfig {
            initialized: true,
            archive_mode: ArchiveMode::Plaintext,
            ..AppConfig::default()
        };
        let archive = open_archive_connection(&paths, &config, None).expect("archive");
        seed_core_intelligence_fixture(&archive);
        drop(archive);

        run_core_intelligence(&paths, &config, None, &CoreIntelligenceRebuildRequest::default())
            .expect("full rebuild");

        reset_open_intelligence_connection_call_count();
        reset_load_intelligence_runtime_from_connection_call_count();

        let overview = get_intelligence_primary_overview(
            &paths,
            &config,
            None,
            &ScopedDateRangeRequest {
                date_range: DateRange {
                    start: "2024-04-01".to_string(),
                    end: "2024-04-30".to_string(),
                },
                profile_id: Some("chrome:Default".to_string()),
            },
        )
        .expect("primary overview");

        assert_eq!(
            open_intelligence_connection_call_count(),
            1,
            "{:?}",
            open_intelligence_connection_call_sites()
        );
        assert_eq!(load_intelligence_runtime_from_connection_call_count(), 1);
        assert_eq!(overview.timings.len(), 11);
        assert_eq!(overview.digest_summary.meta.section_id, "digest-summary");
        assert_eq!(overview.search_engine_ranking.meta.section_id, "search-activity");
        assert!(overview.total_duration_ms >= overview.timings[0].duration_ms);
    }

    #[test]
    fn secondary_overview_reuses_single_connection_and_runtime_snapshot() {
        let root = tempfile::tempdir().expect("tempdir");
        let paths = project_paths_with_root(root.path());
        let config = AppConfig {
            initialized: true,
            archive_mode: ArchiveMode::Plaintext,
            ..AppConfig::default()
        };
        let archive = open_archive_connection(&paths, &config, None).expect("archive");
        seed_core_intelligence_fixture(&archive);
        drop(archive);

        run_core_intelligence(&paths, &config, None, &CoreIntelligenceRebuildRequest::default())
            .expect("full rebuild");

        reset_open_intelligence_connection_call_count();
        reset_load_intelligence_runtime_from_connection_call_count();

        let overview = get_intelligence_secondary_overview(
            &paths,
            &config,
            None,
            &ScopedDateRangeRequest {
                date_range: DateRange {
                    start: "2024-04-01".to_string(),
                    end: "2024-04-30".to_string(),
                },
                profile_id: Some("chrome:Default".to_string()),
            },
        )
        .expect("secondary overview");

        assert_eq!(
            open_intelligence_connection_call_count(),
            1,
            "{:?}",
            open_intelligence_connection_call_sites()
        );
        assert_eq!(load_intelligence_runtime_from_connection_call_count(), 1);
        assert_eq!(overview.timings.len(), 10);
        assert_eq!(overview.stable_sources.meta.section_id, "stable-sources");
        assert_eq!(overview.path_flows.meta.section_id, "path-flows");
        assert!(overview.total_duration_ms >= overview.timings[0].duration_ms);
    }

    #[test]
    fn discovery_trend_reports_available_years_and_respects_profile_scope() {
        let root = tempfile::tempdir().expect("tempdir");
        let paths = project_paths_with_root(root.path());
        let config = AppConfig {
            initialized: true,
            archive_mode: ArchiveMode::Plaintext,
            ..AppConfig::default()
        };
        let intelligence = open_intelligence_connection(&paths, &config, None).expect("runtime");
        ensure_core_intelligence_schema(&intelligence).expect("ensure intelligence schema");

        for (date_key, profile_id, total_visits, new_domains) in [
            ("2024-04-18", "chrome:Default", 3_i64, 1_i64),
            ("2025-04-18", "chrome:Default", 8_i64, 2_i64),
            ("2026-04-18", "firefox:Default", 5_i64, 1_i64),
        ] {
            intelligence
                .execute(
                    "INSERT INTO daily_summary_rollups
                     (date_key, profile_id, total_visits, total_searches, new_domains, unique_domains, hhi_score, discovery_rate)
                     VALUES (?1, ?2, ?3, 0, ?4, ?4, 0.0, 0.0)",
                    params![date_key, profile_id, total_visits, new_domains],
                )
                .expect("insert daily summary rollup");
        }
        drop(intelligence);

        let archive_wide = get_discovery_trend(
            &paths,
            &config,
            None,
            &GranularityDateRangeRequest {
                date_range: DateRange {
                    start: "2025-01-01".to_string(),
                    end: "2025-12-31".to_string(),
                },
                profile_id: None,
                granularity: "day".to_string(),
            },
        )
        .expect("archive-wide discovery trend");
        assert_eq!(archive_wide.available_years, vec![2026, 2025, 2024]);
        assert_eq!(archive_wide.points.len(), 1);
        assert_eq!(archive_wide.points[0].date_key, "2025-04-18");

        let scoped = get_discovery_trend(
            &paths,
            &config,
            None,
            &GranularityDateRangeRequest {
                date_range: DateRange {
                    start: "2025-01-01".to_string(),
                    end: "2025-12-31".to_string(),
                },
                profile_id: Some("chrome:Default".to_string()),
                granularity: "day".to_string(),
            },
        )
        .expect("scoped discovery trend");
        assert_eq!(scoped.available_years, vec![2025, 2024]);
        assert_eq!(scoped.points.len(), 1);
        assert_eq!(scoped.points[0].date_key, "2025-04-18");
    }

    #[test]
    fn day_insights_compose_exact_day_entities_and_drilldown_metadata() {
        let root = tempfile::tempdir().expect("tempdir");
        let paths = project_paths_with_root(root.path());
        let config = AppConfig {
            initialized: true,
            archive_mode: ArchiveMode::Plaintext,
            ..AppConfig::default()
        };
        let archive = open_archive_connection(&paths, &config, None).expect("archive");
        seed_core_intelligence_fixture(&archive);
        drop(archive);

        run_core_intelligence(&paths, &config, None, &CoreIntelligenceRebuildRequest::default())
            .expect("rebuild intelligence");

        let first_day = local_date_key(1711929600000);
        let insights = get_day_insights(
            &paths,
            &config,
            None,
            &DayInsightsRequest {
                date: first_day.clone(),
                profile_id: Some("chrome:Default".to_string()),
            },
        )
        .expect("day insights");

        assert_eq!(insights.date, first_day);
        assert_eq!(insights.digest_summary.total_visits.value, 2);
        assert_eq!(insights.drilldown.explorer_date_range.start, insights.date);
        assert_eq!(insights.drilldown.explorer_date_range.end, insights.date);
        assert_eq!(insights.hourly_activity.len(), 24);
        assert!(insights.top_sites.iter().any(|site| site.registrable_domain == "github.com"));
        assert!(!insights.query_families.families.is_empty());
        assert!(!insights.refind_pages.is_empty());
    }

    #[test]
    fn domain_deep_dive_keeps_day_scoped_trend_consistent_with_exact_day_insights() {
        let root = tempfile::tempdir().expect("tempdir");
        let paths = project_paths_with_root(root.path());
        let config = AppConfig {
            initialized: true,
            archive_mode: ArchiveMode::Plaintext,
            ..AppConfig::default()
        };
        let archive = open_archive_connection(&paths, &config, None).expect("archive");
        seed_core_intelligence_fixture(&archive);
        drop(archive);

        run_core_intelligence(&paths, &config, None, &CoreIntelligenceRebuildRequest::default())
            .expect("rebuild intelligence");

        let first_day = local_date_key(1711929600000);
        let day = get_day_insights(
            &paths,
            &config,
            None,
            &DayInsightsRequest {
                date: first_day.clone(),
                profile_id: Some("chrome:Default".to_string()),
            },
        )
        .expect("day insights");
        let domain = get_domain_deep_dive(
            &paths,
            &config,
            None,
            &DomainDeepDiveRequest {
                registrable_domain: "github.com".to_string(),
                date_range: DateRange { start: first_day.clone(), end: first_day.clone() },
                profile_id: Some("chrome:Default".to_string()),
            },
        )
        .expect("domain deep dive");

        assert_eq!(domain.registrable_domain, "github.com");
        assert_eq!(domain.total_visits, 1);
        assert_eq!(domain.active_days, 1);
        assert_eq!(domain.visit_trend.len(), 1);
        assert_eq!(domain.visit_trend[0].date_key, first_day);
        assert!(
            day.top_sites.iter().any(|site| site.registrable_domain == domain.registrable_domain)
        );
    }

    #[test]
    fn search_queries_reuse_family_and_trail_identity() {
        let root = tempfile::tempdir().expect("tempdir");
        let paths = project_paths_with_root(root.path());
        let config = AppConfig {
            initialized: true,
            archive_mode: ArchiveMode::Plaintext,
            ..AppConfig::default()
        };
        let archive = open_archive_connection(&paths, &config, None).expect("archive");
        seed_core_intelligence_fixture(&archive);
        drop(archive);

        run_core_intelligence(&paths, &config, None, &CoreIntelligenceRebuildRequest::default())
            .expect("rebuild intelligence");

        let queries = get_search_queries(
            &paths,
            &config,
            None,
            &SearchQueryListRequest {
                date_range: DateRange {
                    start: local_date_key(1711929600000),
                    end: local_date_key(1711929600000),
                },
                profile_id: Some("chrome:Default".to_string()),
                browser_kind: Some("chrome".to_string()),
                engine: Some("google".to_string()),
                domain: None,
                query: Some("sqlite".to_string()),
                sort: Some("family-frequency".to_string()),
                page: 0,
                page_size: 10,
            },
        )
        .expect("search queries");

        assert!(!queries.rows.is_empty());
        assert!(queries.rows.iter().any(|row| row.family_id.is_some() && row.trail_id.is_some()));
        let top_row = &queries.rows[0];
        assert!(top_row.family_count >= top_row.exact_repeat_count);
        assert_eq!(top_row.display_name.as_deref(), Some("Google"));
    }

    #[test]
    fn keyword_surfaces_filter_navigational_noise_and_support_domain_reads() {
        let root = tempfile::tempdir().expect("tempdir");
        let paths = project_paths_with_root(root.path());
        let config = AppConfig {
            initialized: true,
            archive_mode: ArchiveMode::Plaintext,
            ..AppConfig::default()
        };
        let archive = open_archive_connection(&paths, &config, None).expect("archive");
        seed_core_intelligence_fixture(&archive);
        seed_search_keyword_noise_fixture(&archive);
        drop(archive);

        run_core_intelligence(&paths, &config, None, &CoreIntelligenceRebuildRequest::default())
            .expect("rebuild intelligence");

        let intelligence = open_intelligence_connection(&paths, &config, None).expect("runtime");
        let search_events =
            load_profile_search_events(&intelligence, "chrome:Default").expect("search events");
        let navigation_event = search_events
            .iter()
            .find(|event| event.raw_query == "https://asu.edu")
            .expect("navigational event");
        assert_eq!(navigation_event.query_kind, SearchQueryKind::Navigational);
        drop(intelligence);
        let fixture_day = local_date_key(1711929600000);

        let google_queries = get_search_queries(
            &paths,
            &config,
            None,
            &SearchQueryListRequest {
                date_range: DateRange { start: fixture_day.clone(), end: "2024-04-30".to_string() },
                profile_id: Some("chrome:Default".to_string()),
                browser_kind: Some("chrome".to_string()),
                engine: None,
                domain: Some("google.com".to_string()),
                query: None,
                sort: Some("newest".to_string()),
                page: 0,
                page_size: 20,
            },
        )
        .expect("google queries");
        assert_eq!(google_queries.total, 1);
        assert_eq!(google_queries.rows[0].raw_query, "sqlite wal");

        let github_queries = get_search_queries(
            &paths,
            &config,
            None,
            &SearchQueryListRequest {
                date_range: DateRange { start: fixture_day.clone(), end: "2024-04-30".to_string() },
                profile_id: Some("chrome:Default".to_string()),
                browser_kind: Some("chrome".to_string()),
                engine: None,
                domain: Some("github.com".to_string()),
                query: None,
                sort: Some("newest".to_string()),
                page: 0,
                page_size: 20,
            },
        )
        .expect("github queries");
        assert_eq!(github_queries.total, 1);
        assert_eq!(github_queries.rows[0].raw_query, "pathkeep sqlite");

        let concepts = get_top_search_concepts(
            &paths,
            &config,
            None,
            &TopSearchConceptsRequest {
                date_range: DateRange { start: fixture_day, end: "2024-04-30".to_string() },
                profile_id: Some("chrome:Default".to_string()),
                limit: Some(20),
            },
        )
        .expect("top concepts");
        let terms = concepts.into_iter().map(|concept| concept.term).collect::<HashSet<_>>();
        assert!(terms.contains("sqlite"));
        assert!(terms.contains("pathkeep"));
        assert!(!terms.contains("https"));
        assert!(!terms.contains("asu"));
        assert!(!terms.contains("edu"));
    }

    #[test]
    fn clear_derived_intelligence_state_reports_canonical_group_counts() {
        let root = tempfile::tempdir().expect("tempdir");
        let paths = project_paths_with_root(root.path());
        let config = AppConfig {
            initialized: true,
            archive_mode: ArchiveMode::Plaintext,
            ..AppConfig::default()
        };
        let archive = open_archive_connection(&paths, &config, None).expect("archive");
        seed_core_intelligence_fixture(&archive);
        drop(archive);

        run_core_intelligence(&paths, &config, None, &CoreIntelligenceRebuildRequest::default())
            .expect("full rebuild");

        let intelligence = open_intelligence_connection(&paths, &config, None).expect("runtime");
        ensure_intelligence_runtime_schema(&intelligence).expect("runtime schema");
        let now = now_rfc3339();
        intelligence
            .execute(
                "INSERT INTO intelligence_jobs
                 (job_type, plugin_id, run_id, state, priority, attempt, dedupe_key, payload_json,
                  artifact_json, created_at, scheduled_at, started_at, updated_at)
                 VALUES (?1, NULL, NULL, 'running', 50, 1, 'clear-test:full', '{}', '{}',
                         ?2, ?2, ?2, ?2)",
                params![FULL_REBUILD_JOB_TYPE, now],
            )
            .expect("insert runtime job");
        let job_id = intelligence.last_insert_rowid();
        intelligence
            .execute(
                "INSERT INTO intelligence_job_triggers (job_id, run_id, reason, requested_at)
                 VALUES (?1, NULL, 'clear test', ?2)",
                params![job_id, now_rfc3339()],
            )
            .expect("insert runtime trigger");

        let expected_visit_derived =
            super::table_row_count(&intelligence, "visit_derived_facts").expect("visit facts");
        let expected_daily_rollups = super::sum_table_row_counts(
            &intelligence,
            &[
                "domain_daily_rollups",
                "category_daily_rollups",
                "engine_daily_rollups",
                "daily_summary_rollups",
            ],
        )
        .expect("daily rollups");
        let expected_structural = super::sum_table_row_counts(
            &intelligence,
            &[
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
        )
        .expect("structural");
        let expected_runtime =
            super::table_row_count(&intelligence, "deterministic_module_runtime")
                .expect("module runtime")
                + super::table_row_count(&intelligence, "core_intelligence_stage_checkpoints")
                    .expect("stage checkpoints")
                + super::count_core_intelligence_jobs(&intelligence).expect("runtime jobs")
                + super::count_core_intelligence_job_triggers(&intelligence)
                    .expect("runtime triggers");
        drop(intelligence);

        let report = super::clear_derived_intelligence_state(&paths, &config, None)
            .expect("clear derived state");
        assert_eq!(report.cleared_visit_derived_fact_rows, expected_visit_derived);
        assert_eq!(report.cleared_daily_rollup_rows, expected_daily_rollups);
        assert_eq!(report.cleared_structural_rows, expected_structural);
        assert_eq!(report.cleared_runtime_rows, expected_runtime);

        let intelligence =
            open_intelligence_connection(&paths, &config, None).expect("runtime after clear");
        assert_eq!(
            super::table_row_count(&intelligence, "visit_derived_facts").expect("visit facts"),
            0
        );
        assert_eq!(
            super::table_row_count(&intelligence, "daily_summary_rollups")
                .expect("daily summaries"),
            0
        );
        assert_eq!(
            super::table_row_count(&intelligence, "search_trails").expect("search trails"),
            0
        );
        assert_eq!(
            super::table_row_count(&intelligence, "core_intelligence_stage_checkpoints")
                .expect("stage checkpoints"),
            0
        );
        assert_eq!(
            super::table_row_count(&intelligence, "deterministic_module_runtime")
                .expect("module runtime"),
            0
        );
        assert_eq!(super::count_core_intelligence_jobs(&intelligence).expect("runtime jobs"), 0);
        assert_eq!(
            super::count_core_intelligence_job_triggers(&intelligence).expect("runtime triggers"),
            0
        );
    }

    #[test]
    fn merge_stage_run_result_sums_stage_timings_across_profiles() {
        let mut aggregate = StageRunResult {
            stage_timings_ms: Some(CoreIntelligenceStageTimings {
                visit_derive_ms: 10,
                daily_rollup_ms: 20,
                structural_rebuild_ms: 30,
                total_ms: 60,
            }),
            ..StageRunResult::default()
        };
        let next = StageRunResult {
            stage_timings_ms: Some(CoreIntelligenceStageTimings {
                visit_derive_ms: 1,
                daily_rollup_ms: 2,
                structural_rebuild_ms: 3,
                total_ms: 6,
            }),
            ..StageRunResult::default()
        };

        merge_stage_run_result(&mut aggregate, next, RebuildMode::FullRebuild);

        let timings = aggregate.stage_timings_ms.expect("stage timings");
        assert_eq!(timings.visit_derive_ms, 11);
        assert_eq!(timings.daily_rollup_ms, 22);
        assert_eq!(timings.structural_rebuild_ms, 33);
        assert_eq!(timings.total_ms, 66);
    }

    #[test]
    fn explain_entity_and_provider_snapshots_build_from_core_intelligence_tables() {
        let root = tempfile::tempdir().expect("tempdir");
        let paths = project_paths_with_root(root.path());
        let config = AppConfig {
            initialized: true,
            archive_mode: ArchiveMode::Plaintext,
            ..AppConfig::default()
        };
        let archive = open_archive_connection(&paths, &config, None).expect("archive");
        seed_core_intelligence_fixture(&archive);
        drop(archive);

        let rebuild = run_core_intelligence(
            &paths,
            &config,
            None,
            &CoreIntelligenceRebuildRequest::default(),
        )
        .expect("run core intelligence");
        assert!(rebuild.processed_visits >= 3);
        let stage_timings = rebuild.stage_timings_ms.as_ref().expect("full rebuild stage timings");
        assert!(stage_timings.total_ms >= stage_timings.visit_derive_ms);
        assert!(stage_timings.total_ms >= stage_timings.daily_rollup_ms);
        assert!(stage_timings.total_ms >= stage_timings.structural_rebuild_ms);

        let session_explanation = explain_entity(
            &paths,
            &config,
            None,
            &EntityExplanationRequest {
                entity_type: "session".to_string(),
                entity_id: "session:chrome:Default:1".to_string(),
            },
        )
        .expect("session explanation");
        assert_eq!(session_explanation.entity_type, "session");
        assert!(!session_explanation.participating_visit_ids.is_empty());

        let refind_explanation = explain_entity(
            &paths,
            &config,
            None,
            &EntityExplanationRequest {
                entity_type: "refind_page".to_string(),
                entity_id: "https://github.com/example/repo/issues/42".to_string(),
            },
        )
        .expect("refind explanation");
        assert_eq!(refind_explanation.entity_type, "refind_page");
        assert!(refind_explanation.factors.iter().any(|factor| factor.label == "cross_day_count"));

        let embed_cards = get_intelligence_embed_cards(
            &paths,
            &config,
            None,
            &IntelligenceEmbedCardsRequest {
                date_range: DateRange {
                    start: "2024-04-01".to_string(),
                    end: "2024-04-30".to_string(),
                },
                profile_id: Some("chrome:Default".to_string()),
                limit: Some(4),
            },
        )
        .expect("embed cards");
        assert!(!embed_cards.is_empty());
        assert!(embed_cards.iter().any(|card| card.card_type == "digest"));

        let public_snapshot = get_intelligence_public_snapshot(
            &paths,
            &config,
            None,
            &ScopedDateRangeRequest {
                date_range: DateRange {
                    start: "2024-04-01".to_string(),
                    end: "2024-04-30".to_string(),
                },
                profile_id: Some("chrome:Default".to_string()),
            },
        )
        .expect("public snapshot");
        assert!(!public_snapshot.top_domains.is_empty());
        assert!(
            public_snapshot.notes.iter().any(|note| note.contains("omit visit-level identifiers"))
        );
        let public_snapshot_json =
            serde_json::to_string(&public_snapshot).expect("serialize public snapshot");
        assert!(!public_snapshot_json.contains("https://"));
        assert!(!public_snapshot_json.contains("visitId"));

        let widget_snapshot = get_intelligence_widget_snapshot(
            &paths,
            &config,
            None,
            &IntelligenceEmbedCardsRequest {
                date_range: DateRange {
                    start: "2024-04-01".to_string(),
                    end: "2024-04-30".to_string(),
                },
                profile_id: Some("chrome:Default".to_string()),
                limit: Some(8),
            },
        )
        .expect("widget snapshot");
        assert!(widget_snapshot.highlights.len() <= 4);
        assert!(widget_snapshot.notes.iter().any(|note| note.contains("internal_only")));

        let intelligence =
            open_intelligence_connection(&paths, &config, None).expect("intelligence");
        let migration_count: i64 = intelligence
            .query_row("SELECT COUNT(*) FROM intelligence_schema_migrations", [], |row| row.get(0))
            .expect("migration count");
        assert_eq!(migration_count, 6);
    }

    #[test]
    fn visit_derive_stage_processes_only_new_visible_visits() {
        let root = tempfile::tempdir().expect("tempdir");
        let paths = project_paths_with_root(root.path());
        let config = AppConfig {
            initialized: true,
            archive_mode: ArchiveMode::Plaintext,
            ..AppConfig::default()
        };
        let archive = open_archive_connection(&paths, &config, None).expect("archive");
        seed_core_intelligence_fixture(&archive);
        drop(archive);

        run_core_intelligence(&paths, &config, None, &CoreIntelligenceRebuildRequest::default())
            .expect("full rebuild");

        let archive = open_archive_connection(&paths, &config, None).expect("archive reopen");
        append_fixture_visit(
            &archive,
            4,
            "https://docs.example.com/sqlite/wal-checkpoint",
            "SQLite WAL Checkpoint",
            1712102400000,
            None,
            None,
        );
        drop(archive);

        let report = run_core_intelligence_job_type_with_progress(
            &paths,
            &config,
            None,
            "visit-derive",
            &CoreIntelligenceRebuildRequest {
                profile_id: Some("chrome:Default".to_string()),
                ..CoreIntelligenceRebuildRequest::default()
            },
            |_progress| Ok(()),
        )
        .expect("visit derive stage");
        assert_eq!(report.execution_mode.as_deref(), Some("incremental"));
        assert_eq!(report.dirty_visit_count, Some(1));
        assert_eq!(report.visit_derived_facts, 1);

        let intelligence =
            open_intelligence_connection(&paths, &config, None).expect("intelligence");
        let checkpoint = intelligence
            .query_row(
                "SELECT last_processed_visit_id
                 FROM core_intelligence_stage_checkpoints
                 WHERE profile_id = 'chrome:Default' AND stage = 'visit-derive'",
                [],
                |row| row.get::<_, i64>(0),
            )
            .expect("stage checkpoint");
        assert_eq!(checkpoint, 4);
    }

    #[test]
    fn daily_rollup_stage_recomputes_only_dirty_days() {
        let root = tempfile::tempdir().expect("tempdir");
        let paths = project_paths_with_root(root.path());
        let config = AppConfig {
            initialized: true,
            archive_mode: ArchiveMode::Plaintext,
            ..AppConfig::default()
        };
        let archive = open_archive_connection(&paths, &config, None).expect("archive");
        seed_core_intelligence_fixture(&archive);
        drop(archive);

        run_core_intelligence(&paths, &config, None, &CoreIntelligenceRebuildRequest::default())
            .expect("full rebuild");

        let archive = open_archive_connection(&paths, &config, None).expect("archive reopen");
        append_fixture_visit(
            &archive,
            4,
            "https://docs.example.com/sqlite/wal-checkpoint",
            "SQLite WAL Checkpoint",
            1712059200000,
            None,
            None,
        );
        append_fixture_visit(
            &archive,
            5,
            "https://example.com/deep-dive",
            "Deep Dive",
            1712145600000,
            None,
            None,
        );
        drop(archive);

        run_core_intelligence_job_type_with_progress(
            &paths,
            &config,
            None,
            "visit-derive",
            &CoreIntelligenceRebuildRequest {
                profile_id: Some("chrome:Default".to_string()),
                ..CoreIntelligenceRebuildRequest::default()
            },
            |_progress| Ok(()),
        )
        .expect("visit derive stage");
        let report = run_core_intelligence_job_type_with_progress(
            &paths,
            &config,
            None,
            "daily-rollup",
            &CoreIntelligenceRebuildRequest {
                profile_id: Some("chrome:Default".to_string()),
                ..CoreIntelligenceRebuildRequest::default()
            },
            |_progress| Ok(()),
        )
        .expect("daily rollup stage");
        let expected_dirty_dates =
            vec![local_date_key(1712059200000), local_date_key(1712145600000)];
        let expected_totals = [
            1711929600000_i64,
            1711929660000_i64,
            1712016000000_i64,
            1712059200000_i64,
            1712145600000_i64,
        ]
        .into_iter()
        .fold(BTreeMap::<String, i64>::new(), |mut acc, timestamp| {
            *acc.entry(local_date_key(timestamp)).or_default() += 1;
            acc
        });
        assert_eq!(report.execution_mode.as_deref(), Some("incremental"));
        assert_eq!(report.dirty_date_keys.as_deref(), Some(expected_dirty_dates.as_slice()));

        let intelligence =
            open_intelligence_connection(&paths, &config, None).expect("intelligence");
        let first_dirty_total: i64 = intelligence
            .query_row(
                "SELECT total_visits
                 FROM daily_summary_rollups
                 WHERE profile_id = 'chrome:Default' AND date_key = ?1",
                [expected_dirty_dates[0].as_str()],
                |row| row.get(0),
            )
            .expect("first dirty rollup");
        let second_dirty_total: i64 = intelligence
            .query_row(
                "SELECT total_visits
                 FROM daily_summary_rollups
                 WHERE profile_id = 'chrome:Default' AND date_key = ?1",
                [expected_dirty_dates[1].as_str()],
                |row| row.get(0),
            )
            .expect("second dirty rollup");
        let summary_row_count: i64 = intelligence
            .query_row(
                "SELECT COUNT(*)
                 FROM daily_summary_rollups
                 WHERE profile_id = 'chrome:Default'",
                [],
                |row| row.get(0),
            )
            .expect("summary row count");
        assert_eq!(first_dirty_total, *expected_totals.get(&expected_dirty_dates[0]).unwrap_or(&0));
        assert_eq!(
            second_dirty_total,
            *expected_totals.get(&expected_dirty_dates[1]).unwrap_or(&0)
        );
        assert_eq!(summary_row_count, expected_totals.len() as i64);
    }

    #[test]
    fn daily_rollup_fallback_collapses_conflicting_categories_into_one_domain_row() {
        let root = tempfile::tempdir().expect("tempdir");
        let paths = project_paths_with_root(root.path());
        let config = AppConfig {
            initialized: true,
            archive_mode: ArchiveMode::Plaintext,
            ..AppConfig::default()
        };
        let archive = open_archive_connection(&paths, &config, None).expect("archive");
        seed_core_intelligence_fixture(&archive);
        append_fixture_visit(
            &archive,
            4,
            "https://github.com/example/repo/pulls/7",
            "Pull Request 7",
            1711929720000,
            Some(2),
            None,
        );
        drop(archive);

        run_core_intelligence(&paths, &config, None, &CoreIntelligenceRebuildRequest::default())
            .expect("full rebuild");

        let intelligence =
            open_intelligence_connection(&paths, &config, None).expect("intelligence");
        intelligence
            .execute(
                "UPDATE visit_derived_facts
                 SET domain_category = CASE
                     WHEN visit_id = 2 THEN 'docs'
                     WHEN visit_id = 4 THEN 'developer'
                     ELSE domain_category
                 END
                 WHERE visit_id IN (2, 4)",
                [],
            )
            .expect("inject conflicting categories");
        intelligence
            .execute(
                "DELETE FROM core_intelligence_stage_checkpoints
                 WHERE profile_id = 'chrome:Default' AND stage = 'daily-rollup'",
                [],
            )
            .expect("clear daily rollup checkpoint");
        drop(intelligence);

        let report = run_core_intelligence_job_type_with_progress(
            &paths,
            &config,
            None,
            "daily-rollup",
            &CoreIntelligenceRebuildRequest {
                profile_id: Some("chrome:Default".to_string()),
                ..CoreIntelligenceRebuildRequest::default()
            },
            |_progress| Ok(()),
        )
        .expect("daily rollup fallback");
        assert_eq!(report.execution_mode.as_deref(), Some("fallback-full"));

        let intelligence =
            open_intelligence_connection(&paths, &config, None).expect("intelligence reopen");
        let row_count: i64 = intelligence
            .query_row(
                "SELECT COUNT(*)
                 FROM domain_daily_rollups
                 WHERE profile_id = 'chrome:Default'
                   AND date_key = ?1
                   AND registrable_domain = 'github.com'",
                [local_date_key(1711929600000)],
                |row| row.get(0),
            )
            .expect("domain row count");
        let category: String = intelligence
            .query_row(
                "SELECT domain_category
                 FROM domain_daily_rollups
                 WHERE profile_id = 'chrome:Default'
                   AND date_key = ?1
                   AND registrable_domain = 'github.com'",
                [local_date_key(1711929600000)],
                |row| row.get(0),
            )
            .expect("selected domain category");
        assert_eq!(row_count, 1);
        assert_eq!(category, "developer");
    }

    #[test]
    fn structural_stage_updates_tail_assignments_incrementally() {
        let root = tempfile::tempdir().expect("tempdir");
        let paths = project_paths_with_root(root.path());
        let config = AppConfig {
            initialized: true,
            archive_mode: ArchiveMode::Plaintext,
            ..AppConfig::default()
        };
        let archive = open_archive_connection(&paths, &config, None).expect("archive");
        seed_core_intelligence_fixture(&archive);
        drop(archive);

        run_core_intelligence(&paths, &config, None, &CoreIntelligenceRebuildRequest::default())
            .expect("full rebuild");

        let archive = open_archive_connection(&paths, &config, None).expect("archive reopen");
        append_fixture_visit(
            &archive,
            4,
            "https://github.com/example/repo/pulls/7",
            "Pull Request 7",
            1711929900000,
            Some(2),
            None,
        );
        drop(archive);

        run_core_intelligence_job_type_with_progress(
            &paths,
            &config,
            None,
            "visit-derive",
            &CoreIntelligenceRebuildRequest {
                profile_id: Some("chrome:Default".to_string()),
                ..CoreIntelligenceRebuildRequest::default()
            },
            |_progress| Ok(()),
        )
        .expect("visit derive stage");
        let report = run_core_intelligence_job_type_with_progress(
            &paths,
            &config,
            None,
            "structural-rebuild",
            &CoreIntelligenceRebuildRequest {
                profile_id: Some("chrome:Default".to_string()),
                ..CoreIntelligenceRebuildRequest::default()
            },
            |_progress| Ok(()),
        )
        .expect("structural stage");
        assert_eq!(report.execution_mode.as_deref(), Some("incremental"));
        assert_eq!(report.dirty_visit_count, Some(1));

        let intelligence =
            open_intelligence_connection(&paths, &config, None).expect("intelligence");
        let assignments = intelligence
            .prepare(
                "SELECT visit_id, session_id, trail_id
                 FROM visit_derived_facts
                 WHERE visit_id IN (1, 2, 4)
                 ORDER BY visit_id ASC",
            )
            .expect("prepare assignments")
            .query_map([], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, Option<String>>(2)?,
                ))
            })
            .expect("query assignments")
            .collect::<rusqlite::Result<Vec<_>>>()
            .expect("collect assignments");
        assert_eq!(assignments[0].1.as_deref(), Some("session:chrome:Default:1"));
        assert_eq!(assignments[1].1.as_deref(), Some("session:chrome:Default:1"));
        assert_eq!(assignments[2].1.as_deref(), Some("session:chrome:Default:1"));
        assert_eq!(assignments[0].2.as_deref(), Some("trail:chrome:Default:1"));
        assert_eq!(assignments[1].2.as_deref(), Some("trail:chrome:Default:1"));
        assert_eq!(assignments[2].2.as_deref(), Some("trail:chrome:Default:1"));
    }

    #[test]
    fn structural_stream_keeps_assignments_stable_across_batch_boundaries() {
        let root = tempfile::tempdir().expect("tempdir");
        let paths = project_paths_with_root(root.path());
        let config = AppConfig {
            initialized: true,
            archive_mode: ArchiveMode::Plaintext,
            ..AppConfig::default()
        };
        let archive = open_archive_connection(&paths, &config, None).expect("archive");
        seed_core_intelligence_fixture(&archive);
        drop(archive);

        run_core_intelligence(&paths, &config, None, &CoreIntelligenceRebuildRequest::default())
            .expect("full rebuild");

        let archive = open_archive_connection(&paths, &config, None).expect("archive reopen");
        append_fixture_chain_visits(
            &archive,
            4,
            STRUCTURAL_TAIL_STREAM_BATCH_SIZE + 5,
            1711929720000,
            2,
        );
        drop(archive);

        run_core_intelligence_job_type_with_progress(
            &paths,
            &config,
            None,
            "visit-derive",
            &CoreIntelligenceRebuildRequest {
                profile_id: Some("chrome:Default".to_string()),
                ..CoreIntelligenceRebuildRequest::default()
            },
            |_progress| Ok(()),
        )
        .expect("visit derive stage");
        let report = run_core_intelligence_job_type_with_progress(
            &paths,
            &config,
            None,
            "structural-rebuild",
            &CoreIntelligenceRebuildRequest {
                profile_id: Some("chrome:Default".to_string()),
                ..CoreIntelligenceRebuildRequest::default()
            },
            |_progress| Ok(()),
        )
        .expect("structural stage");
        assert_eq!(report.execution_mode.as_deref(), Some("incremental"));
        assert!(report.processed_visits > STRUCTURAL_TAIL_STREAM_BATCH_SIZE);

        let boundary_visit_id = 4 + STRUCTURAL_TAIL_STREAM_BATCH_SIZE as i64 - 1;
        let trailing_visit_id = 4 + STRUCTURAL_TAIL_STREAM_BATCH_SIZE as i64 + 4;
        let intelligence =
            open_intelligence_connection(&paths, &config, None).expect("intelligence");
        let assignments = intelligence
            .prepare(
                "SELECT visit_id, session_id, trail_id
                 FROM visit_derived_facts
                 WHERE visit_id IN (1, 2, 4, ?1, ?2)
                 ORDER BY visit_id ASC",
            )
            .expect("prepare assignments")
            .query_map(params![boundary_visit_id, trailing_visit_id], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, Option<String>>(2)?,
                ))
            })
            .expect("query assignments")
            .collect::<rusqlite::Result<Vec<_>>>()
            .expect("collect assignments");
        assert_eq!(assignments.len(), 5);
        for (_, session_id, trail_id) in assignments {
            assert_eq!(session_id.as_deref(), Some("session:chrome:Default:1"));
            assert_eq!(trail_id.as_deref(), Some("trail:chrome:Default:1"));
        }
    }

    #[test]
    fn structural_range_delete_preserves_unaffected_rows_before_start_ms() {
        let root = tempfile::tempdir().expect("tempdir");
        let paths = project_paths_with_root(root.path());
        let config = AppConfig {
            initialized: true,
            archive_mode: ArchiveMode::Plaintext,
            ..AppConfig::default()
        };
        let archive = open_archive_connection(&paths, &config, None).expect("archive");
        seed_core_intelligence_fixture(&archive);
        append_fixture_visit(
            &archive,
            40,
            "https://www.google.com/search?q=earlier+investigation",
            "Earlier Search",
            1711843200000,
            None,
            Some("earlier investigation"),
        );
        append_fixture_visit(
            &archive,
            41,
            "https://docs.example.com/older-trail",
            "Older Trail",
            1711843260000,
            Some(40),
            None,
        );
        drop(archive);

        run_core_intelligence(&paths, &config, None, &CoreIntelligenceRebuildRequest::default())
            .expect("full rebuild");

        let archive = open_archive_connection(&paths, &config, None).expect("archive reopen");
        append_fixture_chain_visits(&archive, 4, 12, 1711929720000, 2);
        drop(archive);

        run_core_intelligence_job_type_with_progress(
            &paths,
            &config,
            None,
            "visit-derive",
            &CoreIntelligenceRebuildRequest {
                profile_id: Some("chrome:Default".to_string()),
                ..CoreIntelligenceRebuildRequest::default()
            },
            |_progress| Ok(()),
        )
        .expect("visit derive stage");
        run_core_intelligence_job_type_with_progress(
            &paths,
            &config,
            None,
            "structural-rebuild",
            &CoreIntelligenceRebuildRequest {
                profile_id: Some("chrome:Default".to_string()),
                ..CoreIntelligenceRebuildRequest::default()
            },
            |_progress| Ok(()),
        )
        .expect("structural stage");

        let intelligence =
            open_intelligence_connection(&paths, &config, None).expect("intelligence");
        let preserved_session = intelligence
            .query_row(
                "SELECT session_id FROM visit_derived_facts WHERE visit_id = 40",
                [],
                |row| row.get::<_, Option<String>>(0),
            )
            .expect("preserved session");
        let preserved_trail = intelligence
            .query_row("SELECT trail_id FROM visit_derived_facts WHERE visit_id = 41", [], |row| {
                row.get::<_, Option<String>>(0)
            })
            .expect("preserved trail");
        let trail_rows: i64 = intelligence
            .query_row(
                "SELECT COUNT(*) FROM search_trails WHERE trail_id = 'trail:chrome:Default:40'",
                [],
                |row| row.get(0),
            )
            .expect("trail row count");
        let event_rows: i64 = intelligence
            .query_row("SELECT COUNT(*) FROM search_events WHERE visit_id = 40", [], |row| {
                row.get(0)
            })
            .expect("event row count");
        assert_eq!(preserved_session.as_deref(), Some("session:chrome:Default:40"));
        assert_eq!(preserved_trail.as_deref(), Some("trail:chrome:Default:40"));
        assert_eq!(trail_rows, 1);
        assert_eq!(event_rows, 1);
    }

    #[test]
    fn batched_query_family_builder_matches_in_memory_builder() {
        let root = tempfile::tempdir().expect("tempdir");
        let paths = project_paths_with_root(root.path());
        let config = AppConfig {
            initialized: true,
            archive_mode: ArchiveMode::Plaintext,
            ..AppConfig::default()
        };
        let archive = open_archive_connection(&paths, &config, None).expect("archive");
        seed_core_intelligence_fixture(&archive);
        for visit_id in 4..=24 {
            let timestamp = 1712145600000 + ((visit_id - 4) * 60_000);
            let query = if visit_id % 2 == 0 {
                format!("sqlite wal checkpoint {}", visit_id % 3)
            } else {
                format!("tauri ipc bridge {}", visit_id % 4)
            };
            append_fixture_visit(
                &archive,
                visit_id,
                &format!("https://www.google.com/search?q={}", query.replace(' ', "+")),
                &format!("Search {query}"),
                timestamp,
                None,
                Some(&query),
            );
        }
        drop(archive);

        run_core_intelligence(&paths, &config, None, &CoreIntelligenceRebuildRequest::default())
            .expect("full rebuild");

        let intelligence =
            open_intelligence_connection(&paths, &config, None).expect("intelligence");
        let all_events =
            load_profile_search_events(&intelligence, "chrome:Default").expect("search events");
        let in_memory = build_query_families(&all_events);
        let batched = build_query_families_from_batches(&intelligence, "chrome:Default")
            .expect("batched query families");

        let to_summary = |families: Vec<QueryFamilyRecord>| {
            let mut summary = families
                .into_iter()
                .map(|family| {
                    (
                        normalize_query(&family.anchor_query),
                        family.member_count,
                        family.search_engine,
                        family.queries.len(),
                    )
                })
                .collect::<Vec<_>>();
            summary.sort();
            summary
        };
        assert_eq!(to_summary(batched), to_summary(in_memory));
    }

    #[test]
    fn batched_structural_aggregates_match_in_memory_builders() {
        let root = tempfile::tempdir().expect("tempdir");
        let paths = project_paths_with_root(root.path());
        let config = AppConfig {
            initialized: true,
            archive_mode: ArchiveMode::Plaintext,
            ..AppConfig::default()
        };
        let archive = open_archive_connection(&paths, &config, None).expect("archive");
        seed_core_intelligence_fixture(&archive);
        append_fixture_visit(
            &archive,
            4,
            "https://docs.example.com/sqlite/wal-checkpoint",
            "SQLite WAL Checkpoint",
            1712102400000,
            None,
            None,
        );
        append_fixture_visit(
            &archive,
            5,
            "https://alpha-docs.dev/guide",
            "Guide",
            1712145600000,
            None,
            None,
        );
        append_fixture_visit(
            &archive,
            6,
            "https://beta-community.dev/thread",
            "Thread",
            1712145660000,
            Some(5),
            None,
        );
        append_fixture_visit(
            &archive,
            7,
            "https://gamma-news.dev/article",
            "Article",
            1712145720000,
            Some(6),
            None,
        );
        append_fixture_visit(
            &archive,
            8,
            "https://delta-shop.dev/item",
            "Item",
            1712145780000,
            Some(7),
            None,
        );
        drop(archive);

        run_core_intelligence(&paths, &config, None, &CoreIntelligenceRebuildRequest::default())
            .expect("full rebuild");

        let intelligence =
            open_intelligence_connection(&paths, &config, None).expect("intelligence");
        let visits = load_profile_derived_visits(&intelligence, "chrome:Default", None, None)
            .expect("derived visits");
        let mut in_memory_refind = build_refind_pages(&visits)
            .into_iter()
            .map(|page| (page.canonical_url, page.refind_score, page.cross_day_count))
            .collect::<Vec<_>>();
        let mut in_memory_flows = build_path_flows(&visits)
            .into_iter()
            .map(|flow| (flow.flow_pattern, flow.step_count, flow.occurrence_count))
            .collect::<Vec<_>>();
        let mut in_memory_habits = build_habit_patterns(&visits)
            .into_iter()
            .map(|habit| (habit.registrable_domain, habit.habit_type, habit.visit_count))
            .collect::<Vec<_>>();

        let (batched_refind, batched_flows, batched_habits) =
            build_structural_profile_aggregates_from_batches(&intelligence, "chrome:Default")
                .expect("batched aggregates");
        let mut batched_refind = batched_refind
            .into_iter()
            .map(|page| (page.canonical_url, page.refind_score, page.cross_day_count))
            .collect::<Vec<_>>();
        let mut batched_flows = batched_flows
            .into_iter()
            .map(|flow| (flow.flow_pattern, flow.step_count, flow.occurrence_count))
            .collect::<Vec<_>>();
        let mut batched_habits = batched_habits
            .into_iter()
            .map(|habit| (habit.registrable_domain, habit.habit_type, habit.visit_count))
            .collect::<Vec<_>>();

        in_memory_refind.sort_by(|left, right| {
            left.0
                .cmp(&right.0)
                .then_with(|| left.1.total_cmp(&right.1))
                .then_with(|| left.2.cmp(&right.2))
        });
        in_memory_flows.sort();
        in_memory_habits.sort();
        batched_refind.sort_by(|left, right| {
            left.0
                .cmp(&right.0)
                .then_with(|| left.1.total_cmp(&right.1))
                .then_with(|| left.2.cmp(&right.2))
        });
        batched_flows.sort();
        batched_habits.sort();

        assert_eq!(batched_refind, in_memory_refind);
        assert_eq!(batched_flows, in_memory_flows);
        assert_eq!(batched_habits, in_memory_habits);
    }

    #[test]
    fn batched_source_effectiveness_matches_in_memory_builder() {
        let root = tempfile::tempdir().expect("tempdir");
        let paths = project_paths_with_root(root.path());
        let config = AppConfig {
            initialized: true,
            archive_mode: ArchiveMode::Plaintext,
            ..AppConfig::default()
        };
        let archive = open_archive_connection(&paths, &config, None).expect("archive");
        seed_core_intelligence_fixture(&archive);
        append_fixture_visit(
            &archive,
            4,
            "https://docs.example.com/sqlite/wal-checkpoint",
            "SQLite WAL Checkpoint",
            1712102400000,
            None,
            None,
        );
        append_fixture_visit(
            &archive,
            5,
            "https://alpha-docs.dev/guide",
            "Guide",
            1712145600000,
            None,
            None,
        );
        append_fixture_visit(
            &archive,
            6,
            "https://beta-community.dev/thread",
            "Thread",
            1712145660000,
            Some(5),
            None,
        );
        drop(archive);

        run_core_intelligence(&paths, &config, None, &CoreIntelligenceRebuildRequest::default())
            .expect("full rebuild");

        let intelligence =
            open_intelligence_connection(&paths, &config, None).expect("intelligence");
        let trails = load_profile_trails(&intelligence, "chrome:Default").expect("trails");
        let (refind_pages, _, _) =
            build_structural_profile_aggregates_from_batches(&intelligence, "chrome:Default")
                .expect("aggregates");
        let mut in_memory = build_source_effectiveness(&trails, &refind_pages)
            .into_iter()
            .map(|record| {
                (
                    record.registrable_domain,
                    record.source_role,
                    record.trail_count,
                    record.stable_landing_count,
                    record.effectiveness_score,
                    record.first_seen_ms,
                    record.last_seen_ms,
                )
            })
            .collect::<Vec<_>>();
        let mut batched = build_source_effectiveness_from_database(
            &intelligence,
            "chrome:Default",
            &refind_pages,
        )
        .expect("batched source effectiveness")
        .into_iter()
        .map(|record| {
            (
                record.registrable_domain,
                record.source_role,
                record.trail_count,
                record.stable_landing_count,
                record.effectiveness_score,
                record.first_seen_ms,
                record.last_seen_ms,
            )
        })
        .collect::<Vec<_>>();
        in_memory.sort_by(|left, right| {
            left.0
                .cmp(&right.0)
                .then_with(|| left.1.cmp(&right.1))
                .then_with(|| left.2.cmp(&right.2))
                .then_with(|| left.3.cmp(&right.3))
                .then_with(|| left.4.total_cmp(&right.4))
                .then_with(|| left.5.cmp(&right.5))
                .then_with(|| left.6.cmp(&right.6))
        });
        batched.sort_by(|left, right| {
            left.0
                .cmp(&right.0)
                .then_with(|| left.1.cmp(&right.1))
                .then_with(|| left.2.cmp(&right.2))
                .then_with(|| left.3.cmp(&right.3))
                .then_with(|| left.4.total_cmp(&right.4))
                .then_with(|| left.5.cmp(&right.5))
                .then_with(|| left.6.cmp(&right.6))
        });
        assert_eq!(batched, in_memory);
    }

    #[test]
    fn visit_derive_stage_falls_back_full_after_visibility_regression() {
        let root = tempfile::tempdir().expect("tempdir");
        let paths = project_paths_with_root(root.path());
        let config = AppConfig {
            initialized: true,
            archive_mode: ArchiveMode::Plaintext,
            ..AppConfig::default()
        };
        let archive = open_archive_connection(&paths, &config, None).expect("archive");
        seed_core_intelligence_fixture(&archive);
        drop(archive);

        run_core_intelligence(&paths, &config, None, &CoreIntelligenceRebuildRequest::default())
            .expect("full rebuild");

        let archive = open_archive_connection(&paths, &config, None).expect("archive reopen");
        archive
            .execute("UPDATE visits SET reverted_at = '2026-04-16T00:00:00Z' WHERE id = 2", [])
            .expect("revert visit");
        drop(archive);

        let report = run_core_intelligence_job_type_with_progress(
            &paths,
            &config,
            None,
            "visit-derive",
            &CoreIntelligenceRebuildRequest {
                profile_id: Some("chrome:Default".to_string()),
                ..CoreIntelligenceRebuildRequest::default()
            },
            |_progress| Ok(()),
        )
        .expect("visit derive fallback");
        assert_eq!(report.execution_mode.as_deref(), Some("fallback-full"));
        assert!(
            report
                .fallback_reason
                .as_deref()
                .is_some_and(|reason| reason.contains("visibility regressed"))
        );

        let intelligence =
            open_intelligence_connection(&paths, &config, None).expect("intelligence");
        let row_count: i64 = intelligence
            .query_row("SELECT COUNT(*) FROM visit_derived_facts", [], |row| row.get(0))
            .expect("row count");
        assert_eq!(row_count, 2);
    }

    #[test]
    fn visit_derive_fallback_matches_clean_full_rebuild_across_batches() {
        let fallback_root = tempfile::tempdir().expect("fallback tempdir");
        let clean_root = tempfile::tempdir().expect("clean tempdir");
        let fallback_paths = project_paths_with_root(fallback_root.path());
        let clean_paths = project_paths_with_root(clean_root.path());
        let config = AppConfig {
            initialized: true,
            archive_mode: ArchiveMode::Plaintext,
            ..AppConfig::default()
        };
        let reverted_visit_id = 1200_i64;

        let fallback_archive =
            open_archive_connection(&fallback_paths, &config, None).expect("fallback archive");
        seed_core_intelligence_fixture(&fallback_archive);
        append_many_fixture_visits(
            &fallback_archive,
            4,
            VISIT_DERIVE_FALLBACK_BATCH_SIZE + 17,
            1712145600000,
        );
        drop(fallback_archive);

        run_core_intelligence(
            &fallback_paths,
            &config,
            None,
            &CoreIntelligenceRebuildRequest::default(),
        )
        .expect("fallback full rebuild");

        let fallback_archive =
            open_archive_connection(&fallback_paths, &config, None).expect("fallback archive");
        fallback_archive
            .execute(
                "UPDATE visits SET reverted_at = '2026-04-16T00:00:00Z' WHERE id = ?1",
                [reverted_visit_id],
            )
            .expect("revert fallback visit");
        drop(fallback_archive);

        let fallback_report = run_core_intelligence_job_type_with_progress(
            &fallback_paths,
            &config,
            None,
            "visit-derive",
            &CoreIntelligenceRebuildRequest {
                profile_id: Some("chrome:Default".to_string()),
                ..CoreIntelligenceRebuildRequest::default()
            },
            |_progress| Ok(()),
        )
        .expect("visit derive fallback");
        assert_eq!(fallback_report.execution_mode.as_deref(), Some("fallback-full"));
        assert!(fallback_report.visit_derived_facts > VISIT_DERIVE_FALLBACK_BATCH_SIZE);

        let clean_archive =
            open_archive_connection(&clean_paths, &config, None).expect("clean archive");
        seed_core_intelligence_fixture(&clean_archive);
        append_many_fixture_visits(
            &clean_archive,
            4,
            VISIT_DERIVE_FALLBACK_BATCH_SIZE + 17,
            1712145600000,
        );
        clean_archive
            .execute(
                "UPDATE visits SET reverted_at = '2026-04-16T00:00:00Z' WHERE id = ?1",
                [reverted_visit_id],
            )
            .expect("revert clean visit");
        drop(clean_archive);

        run_core_intelligence(
            &clean_paths,
            &config,
            None,
            &CoreIntelligenceRebuildRequest::default(),
        )
        .expect("clean full rebuild");

        let fallback_intelligence = open_intelligence_connection(&fallback_paths, &config, None)
            .expect("fallback intelligence");
        let clean_intelligence =
            open_intelligence_connection(&clean_paths, &config, None).expect("clean intelligence");
        assert_eq!(
            load_visit_derived_fact_rows(&fallback_intelligence),
            load_visit_derived_fact_rows(&clean_intelligence)
        );
    }

    #[test]
    fn daily_rollup_fallback_matches_clean_full_rebuild_across_batches() {
        let fallback_root = tempfile::tempdir().expect("fallback tempdir");
        let clean_root = tempfile::tempdir().expect("clean tempdir");
        let fallback_paths = project_paths_with_root(fallback_root.path());
        let clean_paths = project_paths_with_root(clean_root.path());
        let config = AppConfig {
            initialized: true,
            archive_mode: ArchiveMode::Plaintext,
            ..AppConfig::default()
        };
        let reverted_visit_id = 1200_i64;

        let fallback_archive =
            open_archive_connection(&fallback_paths, &config, None).expect("fallback archive");
        seed_core_intelligence_fixture(&fallback_archive);
        append_many_fixture_visits(
            &fallback_archive,
            4,
            DAILY_ROLLUP_FALLBACK_BATCH_SIZE + 17,
            1712145600000,
        );
        drop(fallback_archive);

        run_core_intelligence(
            &fallback_paths,
            &config,
            None,
            &CoreIntelligenceRebuildRequest::default(),
        )
        .expect("fallback full rebuild");

        let fallback_archive =
            open_archive_connection(&fallback_paths, &config, None).expect("fallback archive");
        fallback_archive
            .execute(
                "UPDATE visits SET reverted_at = '2026-04-16T00:00:00Z' WHERE id = ?1",
                [reverted_visit_id],
            )
            .expect("revert fallback visit");
        drop(fallback_archive);

        run_core_intelligence_job_type_with_progress(
            &fallback_paths,
            &config,
            None,
            "visit-derive",
            &CoreIntelligenceRebuildRequest {
                profile_id: Some("chrome:Default".to_string()),
                ..CoreIntelligenceRebuildRequest::default()
            },
            |_progress| Ok(()),
        )
        .expect("visit derive fallback");
        let fallback_report = run_core_intelligence_job_type_with_progress(
            &fallback_paths,
            &config,
            None,
            "daily-rollup",
            &CoreIntelligenceRebuildRequest {
                profile_id: Some("chrome:Default".to_string()),
                ..CoreIntelligenceRebuildRequest::default()
            },
            |_progress| Ok(()),
        )
        .expect("daily rollup fallback");
        assert_eq!(fallback_report.execution_mode.as_deref(), Some("fallback-full"));
        assert!(fallback_report.processed_visits > DAILY_ROLLUP_FALLBACK_BATCH_SIZE);

        let clean_archive =
            open_archive_connection(&clean_paths, &config, None).expect("clean archive");
        seed_core_intelligence_fixture(&clean_archive);
        append_many_fixture_visits(
            &clean_archive,
            4,
            DAILY_ROLLUP_FALLBACK_BATCH_SIZE + 17,
            1712145600000,
        );
        clean_archive
            .execute(
                "UPDATE visits SET reverted_at = '2026-04-16T00:00:00Z' WHERE id = ?1",
                [reverted_visit_id],
            )
            .expect("revert clean visit");
        drop(clean_archive);

        run_core_intelligence(
            &clean_paths,
            &config,
            None,
            &CoreIntelligenceRebuildRequest::default(),
        )
        .expect("clean full rebuild");

        let fallback_intelligence = open_intelligence_connection(&fallback_paths, &config, None)
            .expect("fallback intelligence");
        let clean_intelligence =
            open_intelligence_connection(&clean_paths, &config, None).expect("clean intelligence");
        assert_eq!(
            load_daily_rollup_rows(&fallback_intelligence, "domain_daily_rollups"),
            load_daily_rollup_rows(&clean_intelligence, "domain_daily_rollups")
        );
        assert_eq!(
            load_daily_rollup_rows(&fallback_intelligence, "category_daily_rollups"),
            load_daily_rollup_rows(&clean_intelligence, "category_daily_rollups")
        );
        assert_eq!(
            load_daily_rollup_rows(&fallback_intelligence, "engine_daily_rollups"),
            load_daily_rollup_rows(&clean_intelligence, "engine_daily_rollups")
        );
        assert_eq!(
            load_daily_rollup_rows(&fallback_intelligence, "daily_summary_rollups"),
            load_daily_rollup_rows(&clean_intelligence, "daily_summary_rollups")
        );
    }

    #[test]
    fn path_flows_support_four_step_queries_and_explanations() {
        let root = tempfile::tempdir().expect("tempdir");
        let paths = project_paths_with_root(root.path());
        let config = AppConfig {
            initialized: true,
            archive_mode: ArchiveMode::Plaintext,
            ..AppConfig::default()
        };
        let archive = open_archive_connection(&paths, &config, None).expect("archive");
        seed_core_intelligence_fixture(&archive);
        append_fixture_visit(
            &archive,
            4,
            "https://alpha-docs.dev/guide",
            "Guide",
            1712145600000,
            None,
            None,
        );
        append_fixture_visit(
            &archive,
            5,
            "https://beta-community.dev/thread",
            "Thread",
            1712145660000,
            Some(4),
            None,
        );
        append_fixture_visit(
            &archive,
            6,
            "https://gamma-news.dev/article",
            "Article",
            1712145720000,
            Some(5),
            None,
        );
        append_fixture_visit(
            &archive,
            7,
            "https://delta-shop.dev/item",
            "Item",
            1712145780000,
            Some(6),
            None,
        );
        drop(archive);

        run_core_intelligence(&paths, &config, None, &CoreIntelligenceRebuildRequest::default())
            .expect("full rebuild");

        let flows = get_path_flows(
            &paths,
            &config,
            None,
            &PathFlowRequest {
                date_range: DateRange {
                    start: "2024-03-30".to_string(),
                    end: "2024-04-10".to_string(),
                },
                profile_id: Some("chrome:Default".to_string()),
                step_count: 4,
                limit: Some(10),
            },
        )
        .expect("path flows");
        let flow = flows.iter().find(|entry| entry.step_count == 4).expect("four-step flow");
        let explanation = explain_entity(
            &paths,
            &config,
            None,
            &EntityExplanationRequest {
                entity_type: "path_flow".to_string(),
                entity_id: format!("chrome:Default::4::{}", flow.flow_pattern),
            },
        )
        .expect("path flow explanation");
        assert!(
            explanation
                .factors
                .iter()
                .any(|factor| factor.label == "step_count" && factor.raw_value == 4.0)
        );
    }

    fn append_fixture_visit(
        connection: &Connection,
        visit_id: i64,
        url: &str,
        title: &str,
        visit_time_ms: i64,
        from_visit: Option<i64>,
        normalized_search_term: Option<&str>,
    ) {
        let url_id = visit_id + 10;
        let visit_time_iso =
            chrono::DateTime::from_timestamp_millis(visit_time_ms).expect("timestamp millis");
        connection
            .execute(
                "INSERT INTO urls (
                    id, url, title, visit_count, typed_count, first_visit_ms, first_visit_iso, last_visit_ms, last_visit_iso,
                    source_profile_id, created_by_run_id, source_url_id, hidden, payload_hash, recorded_at
                 ) VALUES (?1, ?2, ?3, 1, 0, ?4, ?5, ?4, ?5, 1, 1, ?6, 0, ?7, '2026-04-14T00:00:00Z')",
                rusqlite::params![
                    url_id,
                    url,
                    title,
                    visit_time_ms,
                    visit_time_iso.to_rfc3339(),
                    url_id + 100,
                    format!("hash-{visit_id}")
                ],
            )
            .expect("insert url");
        connection
            .execute(
                "INSERT INTO visits (
                    id, url_id, source_visit_id, visit_time_ms, visit_time_iso, transition_type, visit_duration_ms,
                    source_profile_id, created_by_run_id, from_visit, is_known_to_sync, event_fingerprint, payload_hash, recorded_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5, 1, 0, 1, 1, ?6, 0, ?7, ?8, '2026-04-14T00:00:00Z')",
                rusqlite::params![
                    visit_id,
                    url_id,
                    visit_id.to_string(),
                    visit_time_ms,
                    visit_time_iso.to_rfc3339(),
                    from_visit,
                    format!("fingerprint-{visit_id}"),
                    format!("visit-hash-{visit_id}")
                ],
            )
            .expect("insert visit");
        if let Some(normalized_search_term) = normalized_search_term {
            connection
                .execute(
                    "INSERT INTO search_terms (
                        id, url_id, term, normalized_term, source_profile_id, created_by_run_id, profile_id
                     ) VALUES (?1, ?2, ?3, ?3, 1, 1, 'chrome:Default')",
                    rusqlite::params![visit_id + 1000, url_id, normalized_search_term],
                )
                .expect("insert search term");
        }
    }

    fn append_many_fixture_visits(
        connection: &Connection,
        start_visit_id: i64,
        count: usize,
        start_time_ms: i64,
    ) {
        for offset in 0..count {
            let visit_id = start_visit_id + offset as i64;
            let query = format!("incremental topic {}", offset % 9);
            let (url, title, from_visit, normalized_search_term) = match offset % 3 {
                0 => (
                    format!("https://www.google.com/search?q={}", query.replace(' ', "+")),
                    format!("Search {query}"),
                    None,
                    Some(query),
                ),
                1 => (
                    format!("https://docs.incremental-{}.dev/guide/{}", offset % 7, offset),
                    format!("Guide {offset}"),
                    Some(visit_id - 1),
                    None,
                ),
                _ => (
                    format!("https://reference.incremental-{}.dev/page/{}", offset % 5, offset),
                    format!("Reference {offset}"),
                    Some(visit_id - 1),
                    None,
                ),
            };
            append_fixture_visit(
                connection,
                visit_id,
                &url,
                &title,
                start_time_ms + (offset as i64 * 60_000),
                from_visit,
                normalized_search_term.as_deref(),
            );
        }
    }

    fn append_fixture_chain_visits(
        connection: &Connection,
        start_visit_id: i64,
        count: usize,
        start_time_ms: i64,
        from_visit_seed: i64,
    ) {
        let mut previous_visit_id = from_visit_seed;
        for offset in 0..count {
            let visit_id = start_visit_id + offset as i64;
            append_fixture_visit(
                connection,
                visit_id,
                &format!("https://github.com/example/repo/pulls/{visit_id}"),
                &format!("Pull Request {visit_id}"),
                start_time_ms + (offset as i64 * 60_000),
                Some(previous_visit_id),
                None,
            );
            previous_visit_id = visit_id;
        }
    }

    fn load_visit_derived_fact_rows(connection: &Connection) -> Vec<(i64, String, String, i64)> {
        connection
            .prepare(
                "SELECT visit_id, registrable_domain, canonical_url, is_new_domain
                 FROM visit_derived_facts
                 ORDER BY visit_id ASC",
            )
            .expect("prepare visit-derived facts")
            .query_map([], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, i64>(3)?,
                ))
            })
            .expect("query visit-derived facts")
            .collect::<rusqlite::Result<Vec<_>>>()
            .expect("collect visit-derived facts")
    }

    fn load_daily_rollup_rows(connection: &Connection, table: &str) -> Vec<String> {
        let sql = format!("SELECT * FROM {table} ORDER BY 1, 2, 3");
        let mut statement = connection.prepare(&sql).expect("prepare rollup rows");
        statement
            .query_map([], |row| {
                let mut values = Vec::with_capacity(row.as_ref().column_count());
                for index in 0..row.as_ref().column_count() {
                    let value = row.get_ref(index)?;
                    let normalized = match value {
                        rusqlite::types::ValueRef::Null => "NULL".to_string(),
                        rusqlite::types::ValueRef::Integer(inner) => inner.to_string(),
                        rusqlite::types::ValueRef::Real(inner) => format!("{inner:.6}"),
                        rusqlite::types::ValueRef::Text(inner) => {
                            String::from_utf8_lossy(inner).into_owned()
                        }
                        rusqlite::types::ValueRef::Blob(inner) => format!("blob:{}", inner.len()),
                    };
                    values.push(normalized);
                }
                Ok(values.join("|"))
            })
            .expect("query rollup rows")
            .collect::<rusqlite::Result<Vec<_>>>()
            .expect("collect rollup rows")
    }

    fn seed_core_intelligence_fixture(connection: &Connection) {
        connection
            .execute(
                "INSERT INTO runs (
                    id, run_type, trigger, started_at, timezone, status, profile_scope_json, warnings_json, stats_json, due_only
                 ) VALUES (
                    1, 'backup', 'manual', '2026-04-14T00:00:00Z', 'UTC', 'success', '[]', '[]', '{}', 0
                 )",
                [],
            )
            .expect("run");
        connection
            .execute(
                "INSERT INTO source_profiles (
                    id, browser_kind, browser_version, profile_name, profile_path, discovered_at, enabled, profile_key, updated_at
                 ) VALUES (
                    1, 'chrome', '1', 'Default', '/tmp/profile', '2026-04-14T00:00:00Z', 1, 'chrome:Default', '2026-04-14T00:00:00Z'
                 )",
                [],
            )
            .expect("profile");
        connection
            .execute(
                "INSERT INTO urls (
                    id, url, title, visit_count, typed_count, first_visit_ms, first_visit_iso, last_visit_ms, last_visit_iso,
                    source_profile_id, created_by_run_id, source_url_id, hidden, payload_hash, recorded_at
                 ) VALUES
                 (1, 'https://www.google.com/search?q=sqlite+wal', 'sqlite wal - Google Search', 1, 0, 1, '1970-01-01T00:00:00Z', 1, '1970-01-01T00:00:00Z', 1, 1, 11, 0, 'hash-1', '2026-04-14T00:00:00Z'),
                 (2, 'https://github.com/example/repo/issues/42', 'Issue 42', 2, 1, 2, '1970-01-01T00:00:02Z', 86400002, '1970-01-02T00:00:00Z', 1, 1, 12, 0, 'hash-2', '2026-04-14T00:00:00Z')",
                [],
            )
            .expect("urls");
        connection
            .execute(
                "INSERT INTO visits (
                    id, url_id, source_visit_id, visit_time_ms, visit_time_iso, transition_type, visit_duration_ms,
                    source_profile_id, created_by_run_id, from_visit, is_known_to_sync, event_fingerprint, payload_hash, recorded_at
                 ) VALUES
                 (1, 1, '1', 1711929600000, '2024-04-01T00:00:00Z', 1, 0, 1, 1, NULL, 0, 'fingerprint-1', 'visit-hash-1', '2026-04-14T00:00:00Z'),
                 (2, 2, '2', 1711929660000, '2024-04-01T00:01:00Z', 1, 0, 1, 1, 1, 0, 'fingerprint-2', 'visit-hash-2', '2026-04-14T00:00:00Z'),
                 (3, 2, '3', 1712016000000, '2024-04-02T00:00:00Z', 1, 0, 1, 1, NULL, 0, 'fingerprint-3', 'visit-hash-3', '2026-04-14T00:00:00Z')",
                [],
            )
            .expect("visits");
        connection
            .execute(
                "INSERT INTO search_terms (
                    id, url_id, term, normalized_term, source_profile_id, created_by_run_id, profile_id
                 ) VALUES (
                    1, 1, 'sqlite wal', 'sqlite wal', 1, 1, 'chrome:Default'
                 )",
                [],
            )
            .expect("search term");
    }

    fn seed_search_keyword_noise_fixture(connection: &Connection) {
        connection
            .execute(
                "INSERT INTO urls (
                    id, url, title, visit_count, typed_count, first_visit_ms, first_visit_iso, last_visit_ms, last_visit_iso,
                    source_profile_id, created_by_run_id, source_url_id, hidden, payload_hash, recorded_at
                 ) VALUES
                 (3, 'https://www.google.com/search?q=https%3A%2F%2Fasu.edu', 'asu.edu - Google Search', 1, 0, 1711929720000, '2024-04-01T00:02:00Z', 1711929720000, '2024-04-01T00:02:00Z', 1, 1, 13, 0, 'hash-3', '2026-04-14T00:00:00Z'),
                 (4, 'https://asu.edu/', 'Arizona State University', 1, 0, 1711929780000, '2024-04-01T00:03:00Z', 1711929780000, '2024-04-01T00:03:00Z', 1, 1, 14, 0, 'hash-4', '2026-04-14T00:00:00Z'),
                 (5, 'https://github.com/search?q=pathkeep+sqlite', 'Repository search results', 1, 0, 1711929840000, '2024-04-01T00:04:00Z', 1711929840000, '2024-04-01T00:04:00Z', 1, 1, 15, 0, 'hash-5', '2026-04-14T00:00:00Z'),
                 (6, 'https://github.com/example/pathkeep', 'PathKeep repo', 1, 0, 1711929900000, '2024-04-01T00:05:00Z', 1711929900000, '2024-04-01T00:05:00Z', 1, 1, 16, 0, 'hash-6', '2026-04-14T00:00:00Z')",
                [],
            )
            .expect("extra urls");
        connection
            .execute(
                "INSERT INTO visits (
                    id, url_id, source_visit_id, visit_time_ms, visit_time_iso, transition_type, visit_duration_ms,
                    source_profile_id, created_by_run_id, from_visit, is_known_to_sync, event_fingerprint, payload_hash, recorded_at
                 ) VALUES
                 (4, 3, '4', 1711929720000, '2024-04-01T00:02:00Z', 1, 0, 1, 1, NULL, 0, 'fingerprint-4', 'visit-hash-4', '2026-04-14T00:00:00Z'),
                 (5, 4, '5', 1711929780000, '2024-04-01T00:03:00Z', 1, 0, 1, 1, 4, 0, 'fingerprint-5', 'visit-hash-5', '2026-04-14T00:00:00Z'),
                 (6, 5, '6', 1711929840000, '2024-04-01T00:04:00Z', 1, 0, 1, 1, NULL, 0, 'fingerprint-6', 'visit-hash-6', '2026-04-14T00:00:00Z'),
                 (7, 6, '7', 1711929900000, '2024-04-01T00:05:00Z', 1, 0, 1, 1, 6, 0, 'fingerprint-7', 'visit-hash-7', '2026-04-14T00:00:00Z')",
                [],
            )
            .expect("extra visits");
    }
}
