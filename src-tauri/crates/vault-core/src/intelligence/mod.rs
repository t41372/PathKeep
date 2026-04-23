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

use self::incremental::{
    ProfileSourceWatermark, StageCheckpoint, StageExecutionMode, delete_stage_checkpoints,
    ensure_core_intelligence_stage_checkpoint_schema, list_core_intelligence_profiles,
    load_profile_source_watermark, load_stage_checkpoint, save_stage_checkpoint, stage_name,
    stage_version, watermark_regressed,
};
use self::intelligence_core_persist::persist_core_state_for_job_kind;
use self::intelligence_daily_rollup_state::{build_daily_rollups, merge_rollups};
#[cfg(test)]
use self::intelligence_daily_rollups::load_profile_derived_visits;
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
#[cfg(test)]
use self::{
    intelligence_rebuild::merge_stage_run_result,
    intelligence_schema::{
        count_core_intelligence_job_triggers, count_core_intelligence_jobs, sum_table_row_counts,
        table_row_count,
    },
};
#[cfg(test)]
use self::{
    intelligence_structural_aggregates::{
        build_habit_patterns, build_path_flows, build_refind_pages, build_source_effectiveness,
        build_structural_profile_aggregates_from_batches,
    },
    intelligence_structural_build::{
        build_query_families, build_query_families_from_batches, load_profile_search_events,
    },
    intelligence_structural_stage::{
        build_source_effectiveness_from_database, load_profile_trails,
    },
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
