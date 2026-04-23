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
mod intelligence_structural_aggregates;
mod intelligence_structural_build;
mod intelligence_structural_persist;
mod intelligence_structural_stage;
mod intelligence_structural_state;
mod intelligence_structural_stream;
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
pub(crate) use self::intelligence_schema::ensure_core_intelligence_schema;
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
use crate::{
    intelligence_catalog::RebuildMode,
    models::{CoreIntelligenceStageTimings, DateRange},
};
use anyhow::{Context, Result};
use chrono::{Datelike, Duration, Local, LocalResult, NaiveDate, TimeZone, Utc};
use reqwest::Url;
use rusqlite::{Connection, Row, params};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

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
