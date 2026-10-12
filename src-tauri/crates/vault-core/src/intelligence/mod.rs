//! Failure modes: a public entry point bypasses Vault or reads an unpublished generation.
//! Responsibilities: expose deterministic intelligence reads and resumable rebuilds.
//! Not responsible for: canonical archive writes, provider work or database ownership.
//! Core Intelligence backend.
//!
//! This module owns the deterministic, non-LLM Core Intelligence read/write
//! plane introduced by the 2026-04-15 hard reset. The intelligence Turso
//! plane stays rebuildable: canonical archive facts remain in
//! `archive/history-vault.sqlite`, while this module materializes sessions,
//! search trails, refind pages, rollups, and related analytics in
//! `derived/history-intelligence.sqlite`.

mod build_append;
mod build_pipeline;
mod build_reclassify;
mod build_refind;
mod build_rows;
mod build_stages;
mod build_state;
mod compare_sets;
mod day_insights;
mod host_artifacts;
mod incremental;
mod insights_zone;
#[cfg(test)]
mod intelligence_core_persist;
mod intelligence_daily_rollup_state;
#[cfg(test)]
mod intelligence_daily_rollups;
mod intelligence_domain;
mod intelligence_explain;
mod intelligence_explain_helpers;
mod intelligence_navigation;
mod intelligence_outputs;
mod intelligence_overview;
mod intelligence_overview_snapshot;
mod intelligence_rebuild;
mod intelligence_refind;
mod intelligence_schema;
mod intelligence_search_metrics;
mod intelligence_search_queries;
mod intelligence_sessions;
mod intelligence_shared;
mod intelligence_structural_aggregates;
mod intelligence_structural_build;
mod intelligence_structural_state;
mod intelligence_structural_stream;
mod intelligence_summary;
mod intelligence_visit_records;
mod paging;
mod path_flows;
mod phase_four;
mod phase_three;
mod refind_days;
mod refind_evidence;
mod search_queries;
mod session_details;
mod site_days;
mod site_dictionary;

#[cfg(test)]
mod tests;

use self::incremental::{
    ProfileSourceWatermark, StageCheckpoint, save_stage_checkpoint, stage_name, stage_version,
};
#[cfg(test)]
use self::intelligence_daily_rollups::load_profile_derived_visit_batch;

#[cfg(test)]
use self::intelligence_shared::local_date_key;
use self::intelligence_shared::{
    build_kpi, classify_search_query_kind, collapse_date_key, count_refind_pages_in_range,
    date_range_bounds, jaccard, new_sites_by_day, previous_date_range, query_token_set,
    rfc3339_from_millis, search_words,
};
use self::intelligence_visit_records::visit_from_row;
use self::site_dictionary::{
    classify_visit, delete_search_engine_rule, display_name_for_domain,
    display_name_for_search_engine, display_name_for_search_engine_with_map,
    list_search_engine_rules, load_enabled_search_engine_rules, load_search_engine_display_names,
    load_search_engine_sites, load_site_dictionary_overrides, normalize_query,
    upsert_search_engine_rule,
};
use crate::{
    intelligence_catalog::RebuildMode,
    models::{CoreIntelligenceStageTimings, DerivedRuntimeNote},
};

pub(crate) use self::build_pipeline::built_with_other_rules;
pub use self::compare_sets::get_compare_sets;
pub use self::day_insights::get_day_insights;
pub use self::host_artifacts::{build_intelligence_local_host, preview_intelligence_local_host};
pub(crate) use self::insights_zone::InsightsZone;
pub use self::insights_zone::{insights_time_zone, insights_today, is_insights_time_zone};
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
    warm_recent_overviews_if_stale,
};
pub use self::intelligence_rebuild::{
    run_core_intelligence, run_core_intelligence_job_type_with_progress,
    run_core_intelligence_with_progress,
};
pub use self::intelligence_refind::{
    explain_refind, get_refind_page_detail, get_refind_pages, get_sites_in_category, get_top_sites,
};

/// Sets the category of a whole site from its Insights page, or with `None` gives it back to the
/// rules. Insights follow once they are built again: the next build sorts only that site's pages
/// (`build_reclassify`). A category PathKeep does not know is refused.
pub fn set_site_category(
    vault: &crate::Vault,
    site: &str,
    category: Option<&str>,
) -> anyhow::Result<()> {
    let (site, category) = (site.trim().to_lowercase(), category.map(str::to_owned));
    anyhow::ensure!(!site.is_empty(), "invalid-site: empty");
    vault.write(vault_store::Db::Intelligence, vault_store::Priority::Interactive, move |s| {
        site_dictionary::set_site_category(s, &site, category.as_deref())
    })
}
pub use self::intelligence_schema::{
    clear_derived_intelligence_state, intelligence_status, max_intelligence_schema_version,
    preview_derived_intelligence_clear,
};
pub use self::intelligence_search_metrics::{
    delete_search_engine_rule_for_settings, get_frequent_searches, get_search_engine_ranking,
    get_top_search_concepts, list_search_engine_rules_for_settings,
    upsert_search_engine_rule_for_settings,
};
pub use self::intelligence_search_queries::{
    get_query_families, get_query_family_detail, get_search_families, get_search_queries,
};
pub use self::intelligence_sessions::{
    get_search_trails, get_session_detail, get_sessions, get_trail_detail,
};
pub use self::intelligence_summary::{
    fill_section_evidence, get_activity_mix, get_activity_mix_trend, get_digest_summary,
    get_friction_signals, get_reopened_investigations, get_search_effectiveness,
    get_stable_sources,
};
pub use self::phase_four::{get_compare_set_detail, get_multi_browser_diff};
pub use self::phase_three::{
    get_breadth_index, get_habit_patterns, get_interrupted_habits, get_observed_interactions,
    get_path_flows,
};
pub(crate) use self::site_days::SiteDayCache;

const SESSION_GAP_MS: i64 = 30 * 60 * 1_000;
const TRAIL_GAP_MS: i64 = 15 * 60 * 1_000;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CoreIntelligenceProgress {
    pub phase: String,
    pub detail: String,
    pub processed_items: Option<usize>,
    pub total_items: Option<usize>,
    pub progress_percent: Option<f32>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
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

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
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

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
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

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
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
    /// The pages the search opened: its landing and the pages after it, not its rewordings
    /// (`visit_count` counts those too).
    max_depth: i64,
    queries: Vec<String>,
    members: Vec<TrailMemberRecord>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct TrailMemberRecord {
    trail_id: String,
    profile_id: String,
    visit_id: i64,
    ordinal: i64,
    role: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct StructuralVisitRecord {
    visit_id: i64,
    /// The browser's own id for the visit; another visit's `from_visit` names it.
    source_visit_id: i64,
    profile_id: String,
    url: String,
    title: Option<String>,
    visit_time_ms: i64,
    from_visit: Option<i64>,
    registrable_domain: String,
    search_engine: Option<String>,
    search_query: Option<String>,
    is_new_domain: bool,
    is_search_event: bool,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
struct QueryFamilyRecord {
    family_id: String,
    profile_id: String,
    anchor_query: String,
    member_count: i64,
    search_engine: String,
    first_seen_ms: i64,
    last_seen_ms: i64,
    queries: Vec<String>,
    /// The local days with a search in the family, so a thread can say it came back on N days.
    active_days: i64,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
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
    /// Local days (the build's zone) this record adds the page to in `refind_day_pages_rows`, the
    /// days it was opened on: all of them when the record is built from all its visits, only the
    /// new ones when an append adds visits to a stored row.
    #[serde(default)]
    days: Vec<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
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

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct PathFlowRecord {
    profile_id: String,
    flow_pattern: String,
    step_count: i64,
    occurrence_count: i64,
    last_seen_ms: i64,
}

/// One site's visits on one day, in one browser profile.
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
struct DomainDayRow {
    date_key: String,
    profile_id: String,
    registrable_domain: String,
    domain_category: String,
    visit_count: i64,
    search_count: i64,
    /// Visits that were the site's first in this profile.
    new_domain_visits: i64,
    unique_urls: i64,
    /// The day's last visit to the site, so habits know when it was last visited without
    /// looking through the site's visits. Rows saved before it existed read as 0.
    #[serde(default)]
    last_visit_ms: i64,
}

/// One day's visits in one browser profile.
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
struct DaySummaryRow {
    date_key: String,
    profile_id: String,
    total_visits: i64,
    total_searches: i64,
    new_domains: i64,
    unique_domains: i64,
    hhi_score: f32,
    discovery_rate: f32,
    /// Visits in each local hour of the day, 0 to 23, for the rhythm map; empty on rows saved
    /// before it existed.
    #[serde(default)]
    hours: Vec<i64>,
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
struct DailyRollupBundle {
    domain_rows: Vec<DomainDayRow>,
    category_rows: Vec<(String, String, String, i64, i64)>,
    engine_rows: Vec<(String, String, String, i64)>,
    summary_rows: Vec<DaySummaryRow>,
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
struct StageRunResult {
    processed_visits: usize,
    visit_derived_facts: usize,
    sessions: usize,
    search_trails: usize,
    query_families: usize,
    refind_pages: usize,
    reopened_investigations: usize,
    execution_mode: Option<String>,
    affected_profiles: Vec<String>,
    dirty_visit_count: Option<usize>,
    dirty_date_keys: Vec<String>,
    fallback_reason: Option<String>,
    /// Stable `REBUILD_FALLBACK_*` code paired with `fallback_reason`.
    fallback_reason_code: Option<String>,
    stage_timings_ms: Option<CoreIntelligenceStageTimings>,
    /// Coded notes carried all the way to the persisted module runtime so the
    /// shell never has to recover meaning from English prose.
    notes: Vec<DerivedRuntimeNote>,
}

// These batch sizes stay well below the low-RAM envelope we benchmark against,
// but they cut a large amount of repeated SQLite scan/statement overhead on
// multi-million-row rebuilds.
#[cfg(test)]
const STRUCTURAL_AGGREGATE_BATCH_SIZE: usize = 8_192;
#[cfg(test)]
const STRUCTURAL_TAIL_STREAM_BATCH_SIZE: usize = 8_192;
#[cfg(test)]
const VISIT_DERIVE_FALLBACK_BATCH_SIZE: usize = 8_192;
#[cfg(test)]
const DAILY_ROLLUP_FALLBACK_BATCH_SIZE: usize = 8_192;

#[cfg(test)]
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
struct DerivedVisitBatchCursor {
    visit_time_ms: i64,
    visit_id: i64,
}

#[cfg(test)]
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
struct SearchEventBatchCursor {
    visit_id: i64,
}

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
struct VisibleVisitBatchCursor {
    visit_time_ms: i64,
    visit_id: i64,
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
struct StructuralTailStreamReport {
    processed_visits: usize,
    sessions: usize,
    trails: usize,
    member_count: usize,
    term_count: usize,
    first_visit_ms: Option<i64>,
}
