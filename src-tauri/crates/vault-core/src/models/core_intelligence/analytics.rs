//! Core Intelligence analytics and explainability DTOs.
//!
//! ## Responsibilities
//! - Define activity, discovery, domain, source-effectiveness, path-flow,
//!   compare-set, browser-diff, and explanation payloads.
//! - Keep advanced read surfaces separate from basic session/query/read rows.
//! - Preserve transport shape for Phase 3/4 Core Intelligence commands.
//!
//! ## Not responsible for
//! - Calculating scores, trends, or path-flow membership.
//! - Owning section envelopes or overview batching.
//! - Defining local-host trusted output bundles.
//!
//! ## Dependencies
//! - Read-row DTOs for domain trend points, sessions, and trails.
//! - `serde` for command transport.
//!
//! ## Performance notes
//! - These payloads can summarize large archives, but every vector here should
//!   be backed by SQL limits or aggregate tables before serialization.

use super::reads::{DomainTrendPoint, SessionSummary, TrailSummary};
use serde::{Deserialize, Serialize};

/// Arrival-channel counts for one domain deep dive.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ArrivalBreakdown {
    pub search: i64,
    pub link: i64,
    pub typed: i64,
    pub other: i64,
}

/// Top page row inside a domain deep dive.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct DomainPageStat {
    pub path: String,
    pub visit_count: i64,
}

/// Inbound/outbound domain flow row inside a domain deep dive.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct DomainFlowStat {
    pub domain: String,
    pub display_name: Option<String>,
    pub count: i64,
}

/// Full read model for one registrable-domain deep dive.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct DomainDeepDive {
    pub registrable_domain: String,
    pub display_name: Option<String>,
    pub domain_category: String,
    pub total_visits: i64,
    pub active_days: i64,
    pub trail_count: i64,
    pub arrival_breakdown: ArrivalBreakdown,
    pub top_pages: Vec<DomainPageStat>,
    pub top_referrers: Vec<DomainFlowStat>,
    pub top_exits: Vec<DomainFlowStat>,
    pub visit_trend: Vec<DomainTrendPoint>,
}

/// Stable source row used by source-effectiveness and secondary overview reads.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct StableSource {
    pub registrable_domain: String,
    pub display_name: Option<String>,
    pub source_role: String,
    pub trail_count: i64,
    pub stable_landing_count: i64,
    pub effectiveness_score: f32,
}

/// Per-engine search effectiveness aggregate.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct EngineEffectiveness {
    pub search_engine: String,
    pub display_name: Option<String>,
    pub avg_reformulations: f32,
    pub total_trails: i64,
    pub avg_depth: f32,
}

/// Query-family row that needed repeated search effort.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct HardTopic {
    pub family_id: String,
    pub query_family: String,
    pub reformulation_count: i64,
    pub re_search_lag_days: f32,
}

/// Search effectiveness payload combining engine, source, and topic signals.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SearchEffectiveness {
    pub engine_stats: Vec<EngineEffectiveness>,
    pub top_resolving_sources: Vec<StableSource>,
    pub hardest_topics: Vec<HardTopic>,
}

/// Friction signal row surfaced by secondary Core Intelligence sections.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct FrictionSignal {
    pub registrable_domain: Option<String>,
    pub url: Option<String>,
    pub evidence_type: String,
    pub signal_kind: String,
    pub occurrence_count: i64,
    pub description: String,
}

/// Reopened investigation aggregate row.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ReopenedInvestigation {
    pub investigation_id: String,
    pub anchor_type: String,
    pub anchor_id: String,
    pub anchor_label: String,
    pub occurrence_count: i64,
    pub distinct_days: i64,
    pub first_seen_at: String,
    pub last_seen_at: String,
}

/// One day/hour activity bucket in a browsing rhythm heatmap.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RhythmHeatmapCell {
    pub dow: i64,
    pub hour: i64,
    pub visit_count: i64,
}

/// Browsing rhythm heatmap with max-count metadata for visualization scaling.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RhythmHeatmap {
    pub cells: Vec<RhythmHeatmapCell>,
    pub max_count: i64,
}

/// One discovery-rate trend point.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveryTrendPoint {
    pub date_key: String,
    pub discovery_rate: f32,
    pub new_domain_count: i64,
    pub total_visits: i64,
}

/// Discovery trend with the available archive years used by Dashboard rhythm.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveryTrend {
    pub points: Vec<DiscoveryTrendPoint>,
    pub available_years: Vec<i32>,
}

/// Category share row in activity mix payloads.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct CategoryMixEntry {
    pub domain_category: String,
    pub visit_count: i64,
    pub share: f32,
}

/// Period-over-period category share delta row.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct CategoryChangeEntry {
    pub domain_category: String,
    pub current_share: f32,
    pub previous_share: f32,
    pub change_points: f32,
}

/// Category mix payload for one date range.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ActivityMix {
    pub categories: Vec<CategoryMixEntry>,
    pub change_vs_previous: Vec<CategoryChangeEntry>,
}

/// One date-keyed activity mix point.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ActivityMixTrendPoint {
    pub date_key: String,
    pub categories: Vec<CategoryMixEntry>,
}

/// Activity mix trend payload for charted category shares.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ActivityMixTrend {
    pub points: Vec<ActivityMixTrendPoint>,
}

/// Concentration and breadth score payload.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct BreadthIndex {
    pub hhi: f32,
    pub breadth_score: f32,
    pub concentration_domain_count: i64,
}

/// Habit-pattern row derived from recurring domain visits.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct HabitPattern {
    pub registrable_domain: String,
    pub display_name: Option<String>,
    pub habit_type: String,
    pub mean_interval_days: f32,
    pub cv: f32,
    pub visit_count: i64,
    pub last_visited_at: String,
    pub is_interrupted: bool,
}

/// Habit row that has crossed its interruption threshold.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct InterruptedHabit {
    #[serde(flatten)]
    pub habit: HabitPattern,
    pub days_since_last_visit: i64,
    pub interruption_threshold_days: f32,
}

/// Repeated path-flow row.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PathFlow {
    pub flow_id: String,
    pub flow_pattern: String,
    pub step_count: i64,
    pub occurrence_count: i64,
    pub last_seen_at: String,
    pub steps: Vec<PathFlowStep>,
}

/// One step in a path-flow pattern.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PathFlowStep {
    pub index: i64,
    pub label: String,
    pub registrable_domain: Option<String>,
}

/// Candidate page row inside a compare set.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct CompareSetPage {
    pub canonical_url: String,
    pub url: String,
    pub title: Option<String>,
    pub registrable_domain: String,
    pub visit_count: i64,
    pub is_landing: bool,
}

/// Compare set summary for repeated evaluation journeys.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct CompareSet {
    pub compare_set_id: String,
    pub trail_id: String,
    pub search_query: String,
    pub page_category: String,
    pub pages: Vec<CompareSetPage>,
}

/// Compare set detail with its owning trail and optional session context.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct CompareSetDetail {
    pub compare_set: CompareSet,
    pub trail: TrailSummary,
    pub session: Option<SessionSummary>,
    pub recent_days: Vec<String>,
}

/// Per-profile browser summary row for multi-browser comparison.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct BrowserProfileSummary {
    pub profile_id: String,
    pub profile_name: String,
    pub browser_family: String,
    pub domain_count: i64,
    pub visit_count: i64,
}

/// Domain that appears in one profile but not others.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ExclusiveDomainEntry {
    pub registrable_domain: String,
    pub profile_id: String,
    pub visit_count: i64,
}

/// Category distribution row for one browser profile.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct BrowserCategoryDistribution {
    pub profile_id: String,
    pub profile_name: String,
    pub categories: Vec<CategoryMixEntry>,
}

/// Multi-browser difference payload.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct BrowserDiff {
    pub profiles: Vec<BrowserProfileSummary>,
    pub exclusive_domains: Vec<ExclusiveDomainEntry>,
    pub shared_domains: Vec<String>,
    pub category_distributions: Vec<BrowserCategoryDistribution>,
}

/// Browser-reported interaction metrics for one visit when available.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ObservedInteraction {
    pub visit_id: i64,
    pub url: String,
    pub title: Option<String>,
    pub browser_family: String,
    pub foreground_duration_ms: Option<i64>,
    pub scrolling_time_ms: Option<i64>,
    pub scrolling_distance: Option<i64>,
    pub key_presses: Option<i64>,
    pub typing_time_ms: Option<i64>,
    pub load_successful: Option<bool>,
    pub page_end_reason: Option<String>,
}

/// Weighted factor row in a generic explanation payload.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ExplainabilityFactor {
    pub label: String,
    pub raw_value: f32,
    pub weight: f32,
    pub contribution: f32,
}

/// Generic explanation payload for entity-focused Core Intelligence surfaces.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Explanation {
    pub entity_type: String,
    pub entity_id: String,
    pub trigger_rule: String,
    pub factors: Vec<ExplainabilityFactor>,
    pub participating_visit_ids: Vec<i64>,
}
