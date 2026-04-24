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
mod intelligence_core_persist;
mod intelligence_daily_rollup_state;
mod intelligence_daily_rollups;
mod intelligence_domain;
mod intelligence_explain;
mod intelligence_explain_helpers;
mod intelligence_navigation;
mod intelligence_outputs;
mod intelligence_overview;
mod intelligence_rebuild;
mod intelligence_refind;
mod intelligence_schema;
mod intelligence_schema_sql;
mod intelligence_search_metrics;
mod intelligence_search_queries;
mod intelligence_sessions;
mod intelligence_shared;
mod intelligence_structural_aggregates;
mod intelligence_structural_build;
mod intelligence_structural_persist;
mod intelligence_structural_stage;
mod intelligence_structural_state;
mod intelligence_structural_stream;
mod intelligence_summary;
mod intelligence_visit_derive;
mod intelligence_visit_records;
mod phase_four;
mod phase_three;
mod site_dictionary;

#[cfg(test)]
mod tests;

use self::incremental::{
    ProfileSourceWatermark, StageCheckpoint, StageExecutionMode, delete_stage_checkpoints,
    ensure_core_intelligence_stage_checkpoint_schema, list_core_intelligence_profiles,
    load_profile_source_watermark, load_stage_checkpoint, save_stage_checkpoint, stage_name,
    stage_version, watermark_regressed,
};
use self::intelligence_core_persist::persist_core_state_for_job_kind;
use self::intelligence_daily_rollup_state::{build_daily_rollups, merge_rollups};
use self::intelligence_daily_rollups::{
    ensure_unique_domain_rollup_rows, execute_daily_rollup_stage, load_profile_derived_visit_batch,
};
pub(crate) use self::intelligence_schema::ensure_core_intelligence_schema;
use self::intelligence_shared::{
    build_kpi, classify_search_query_kind, collapse_date_key, count_refind_pages_in_range,
    date_range_bounds, jaccard, local_date_key, local_datetime_from_millis, previous_date_range,
    query_token_set, rfc3339_from_millis, tokenize_query_terms,
};
use self::intelligence_visit_derive::execute_visit_derive_stage;
use self::intelligence_visit_records::{
    build_profile_state, compute_is_new_domain, compute_is_new_domain_with_seen,
    hydrate_search_terms, load_seen_domains, load_visible_visits, persist_visit_derived_facts,
    unique_date_keys, visit_from_row,
};
use self::site_dictionary::{
    SiteDictionaryEntry, classify_visit, delete_search_engine_rule, display_name_for_domain,
    display_name_for_search_engine, display_name_for_search_engine_with_map,
    ensure_search_engine_rule_schema, ensure_site_dictionary_override_schema,
    list_search_engine_rules, load_enabled_search_engine_rules, load_search_engine_display_names,
    load_site_dictionary_overrides, normalize_query, upsert_search_engine_rule,
};
use self::{
    intelligence_rebuild::load_archive_source_profile_id,
    intelligence_schema::clear_core_tables_for_job_kind,
};
use crate::{intelligence_catalog::RebuildMode, models::CoreIntelligenceStageTimings};

pub use self::day_insights::get_day_insights;
pub use self::host_artifacts::{build_intelligence_local_host, preview_intelligence_local_host};
pub use self::intelligence_domain::{
    get_browsing_rhythm, get_discovery_trend, get_domain_deep_dive, get_domain_trend,
    get_on_this_day,
};
pub use self::intelligence_explain::explain_entity;
pub use self::intelligence_navigation::{get_hub_pages, get_navigation_path};
pub use self::intelligence_outputs::{
    get_intelligence_embed_cards, get_intelligence_public_snapshot,
    get_intelligence_widget_snapshot,
};
pub use self::intelligence_overview::{
    get_intelligence_primary_overview, get_intelligence_secondary_overview,
};
pub use self::intelligence_rebuild::{
    run_core_intelligence, run_core_intelligence_job_type_with_progress,
    run_core_intelligence_with_progress,
};
pub use self::intelligence_refind::{
    explain_refind, get_refind_page_detail, get_refind_pages, get_top_sites,
};
pub use self::intelligence_schema::{clear_derived_intelligence_state, intelligence_status};
pub use self::intelligence_search_metrics::{
    delete_search_engine_rule_for_settings, get_search_engine_ranking, get_top_search_concepts,
    list_search_engine_rules_for_settings, upsert_search_engine_rule_for_settings,
};
pub use self::intelligence_search_queries::{
    get_query_families, get_query_family_detail, get_search_queries,
};
pub use self::intelligence_sessions::{
    get_search_trails, get_session_detail, get_sessions, get_trail_detail,
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

#[derive(Debug, Clone)]
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
