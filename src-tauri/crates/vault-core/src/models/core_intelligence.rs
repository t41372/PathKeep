//! Core Intelligence transport models.
//!
//! These DTOs mirror the backend/frontend contract for the deterministic,
//! non-LLM Core Intelligence reset. They intentionally stay focused on
//! read-model payloads and rebuild/query requests so worker and Tauri layers can
//! evolve without baking UI concerns into computation code.

use super::schedule::GeneratedFile;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
/// Inclusive local-date range used by Core Intelligence query commands.
pub struct DateRange {
    pub start: String,
    pub end: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
/// Period-over-period KPI metadata used by digest cards.
pub struct KpiMetric {
    pub value: i64,
    pub previous_value: Option<i64>,
    pub change_percent: Option<f32>,
    pub trend: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
/// Request payload for a full or scoped Core Intelligence rebuild.
pub struct CoreIntelligenceRebuildRequest {
    pub profile_id: Option<String>,
    pub full_rebuild: bool,
    pub limit: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
/// Stage-by-stage timing summary emitted for full Core Intelligence rebuilds.
pub struct CoreIntelligenceStageTimings {
    pub visit_derive_ms: u64,
    pub daily_rollup_ms: u64,
    pub structural_rebuild_ms: u64,
    pub total_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
/// Result payload returned after a Core Intelligence rebuild finishes.
pub struct CoreIntelligenceRebuildReport {
    pub run_id: i64,
    pub processed_visits: usize,
    pub visit_derived_facts: usize,
    pub sessions: usize,
    pub search_trails: usize,
    pub query_families: usize,
    pub refind_pages: usize,
    pub source_effectiveness: usize,
    pub reopened_investigations: usize,
    pub execution_mode: Option<String>,
    pub affected_profiles: Option<Vec<String>>,
    pub dirty_visit_count: Option<usize>,
    pub dirty_date_keys: Option<Vec<String>>,
    pub fallback_reason: Option<String>,
    pub stage_timings_ms: Option<CoreIntelligenceStageTimings>,
    pub notes: Vec<String>,
    pub last_run_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
/// Queue acknowledgement for a manual Core Intelligence rebuild request.
pub struct CoreIntelligenceQueueReport {
    pub job_id: i64,
    pub state: String,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "kebab-case")]
/// Structured window metadata for one Core Intelligence section response.
pub enum CoreIntelligenceSectionWindow {
    DateRange { date_range: DateRange },
    CalendarDayHistory { reference_date: String },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
/// Shared metadata emitted alongside one `/intelligence` section payload.
pub struct CoreIntelligenceSectionMeta {
    pub section_id: String,
    pub generated_at: Option<String>,
    pub window: CoreIntelligenceSectionWindow,
    pub module_ids: Vec<String>,
    pub source_tables: Vec<String>,
    pub includes_enrichment: bool,
    pub state: String,
    pub state_reason: Option<String>,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
/// Generic transport envelope for one Core Intelligence section payload.
pub struct CoreIntelligenceSectionResult<T> {
    pub data: T,
    pub meta: CoreIntelligenceSectionMeta,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
/// Common request shape for paginated Core Intelligence list queries.
pub struct PagedDateRangeRequest {
    pub date_range: DateRange,
    pub profile_id: Option<String>,
    pub page: u32,
    pub page_size: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
/// Common request shape for non-paginated, profile-scoped Core Intelligence reads.
pub struct ScopedDateRangeRequest {
    pub date_range: DateRange,
    pub profile_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
/// Common request shape for profile-scoped reads that do not need a date window.
pub struct ProfileScopedRequest {
    pub profile_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
/// Request shape for engine-scoped or filtered search-trail reads.
pub struct SearchTrailQueryRequest {
    pub date_range: DateRange,
    pub profile_id: Option<String>,
    pub engine: Option<String>,
    pub page: u32,
    pub page_size: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
/// Request shape for top-sites queries.
pub struct TopSitesRequest {
    pub date_range: DateRange,
    pub profile_id: Option<String>,
    pub sort_by: Option<String>,
    pub limit: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
/// Request shape for one domain trend series.
pub struct DomainTrendRequest {
    #[serde(alias = "domain")]
    pub registrable_domain: String,
    pub date_range: DateRange,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
/// Request shape for top search concepts.
pub struct TopSearchConceptsRequest {
    pub date_range: DateRange,
    pub profile_id: Option<String>,
    pub limit: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
/// Request shape for refind-page queries.
pub struct RefindPagesRequest {
    pub date_range: DateRange,
    pub profile_id: Option<String>,
    pub limit: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
/// Request shape for one refind explanation lookup.
pub struct ExplainRefindRequest {
    pub canonical_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
/// Request shape for granularity-based trend queries.
pub struct GranularityDateRangeRequest {
    pub date_range: DateRange,
    pub profile_id: Option<String>,
    pub granularity: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
/// Request shape for optional category filters on rhythm/activity queries.
pub struct CategoryFilteredDateRangeRequest {
    pub date_range: DateRange,
    pub profile_id: Option<String>,
    pub category: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
/// Request shape for one search effectiveness read.
pub struct SearchEffectivenessRequest {
    pub date_range: DateRange,
    pub profile_id: Option<String>,
    pub engine: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
/// Request shape for one domain deep-dive read.
pub struct DomainDeepDiveRequest {
    #[serde(alias = "domain")]
    pub registrable_domain: String,
    pub date_range: DateRange,
    pub profile_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
/// Request shape for path-flow reads.
pub struct PathFlowRequest {
    pub date_range: DateRange,
    pub profile_id: Option<String>,
    pub step_count: u32,
    pub limit: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
/// Request shape for shareable/embed-oriented intelligence payloads.
pub struct IntelligenceEmbedCardsRequest {
    pub date_range: DateRange,
    pub profile_id: Option<String>,
    pub limit: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
/// Request shape for generic explainability lookups.
pub struct EntityExplanationRequest {
    pub entity_type: String,
    pub entity_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
/// Top-line Core Intelligence digest card payload.
pub struct DigestSummary {
    pub date_range: DateRange,
    pub total_visits: KpiMetric,
    pub total_searches: KpiMetric,
    pub new_domains: KpiMetric,
    pub deep_read_pages: KpiMetric,
    pub refind_pages: KpiMetric,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
/// One historical "On This Day" entry.
pub struct OnThisDayEntry {
    pub year: i32,
    pub date: String,
    pub total_visits: i64,
    pub top_domains: Vec<String>,
    pub summary: Option<String>,
    pub deep_dive_sessions: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct TopSite {
    pub registrable_domain: String,
    pub display_name: Option<String>,
    pub domain_category: String,
    pub visit_count: i64,
    pub unique_days: i64,
    pub average_daily_visits: f32,
    pub unique_urls: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct DomainTrendPoint {
    pub date_key: String,
    pub visit_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct DomainTrend {
    pub registrable_domain: String,
    pub points: Vec<DomainTrendPoint>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct EngineRanking {
    pub search_engine: String,
    pub display_name: Option<String>,
    pub search_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SearchConcept {
    pub term: String,
    pub frequency: i64,
    pub engines: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct QueryFamily {
    pub family_id: String,
    pub anchor_query: String,
    pub member_count: i64,
    pub search_engine: String,
    pub queries: Vec<String>,
    pub first_seen_at: String,
    pub last_seen_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct QueryFamilyResult {
    pub families: Vec<QueryFamily>,
    pub total: i64,
    pub page: u32,
    pub page_size: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RefindPage {
    pub canonical_url: String,
    pub url: String,
    pub title: Option<String>,
    pub registrable_domain: String,
    pub cross_day_count: i64,
    pub trail_count: i64,
    pub search_arrival_count: i64,
    pub typed_revisit_count: i64,
    pub refind_score: f32,
    pub first_seen_at: String,
    pub last_seen_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RefindScoreFactor {
    pub signal: String,
    pub raw_value: f32,
    pub weight: f32,
    pub contribution: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RefindExplanation {
    pub canonical_url: String,
    pub refind_score: f32,
    pub factors: Vec<RefindScoreFactor>,
    pub visit_ids: Vec<i64>,
}

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

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct InterruptedHabit {
    #[serde(flatten)]
    pub habit: HabitPattern,
    pub days_since_last_visit: i64,
    pub interruption_threshold_days: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SessionSummary {
    pub session_id: String,
    pub first_visit_ms: i64,
    pub last_visit_ms: i64,
    pub visit_count: i64,
    pub search_count: i64,
    pub domain_count: i64,
    pub is_deep_dive: bool,
    pub auto_title: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SessionListResult {
    pub sessions: Vec<SessionSummary>,
    pub total: i64,
    pub page: u32,
    pub page_size: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SessionVisit {
    pub visit_id: i64,
    pub url: String,
    pub title: Option<String>,
    pub registrable_domain: String,
    pub visit_time_ms: i64,
    pub is_search_event: bool,
    pub search_query: Option<String>,
    pub search_engine: Option<String>,
    pub trail_id: Option<String>,
    pub transition_type: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct TrailSummary {
    pub trail_id: String,
    pub session_id: Option<String>,
    pub initial_query: String,
    pub search_engine: String,
    pub reformulation_count: i64,
    pub visit_count: i64,
    pub landing_url: Option<String>,
    pub landing_domain: Option<String>,
    pub first_visit_ms: i64,
    pub last_visit_ms: i64,
    pub max_depth: i64,
    pub queries: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SessionDetail {
    pub session: SessionSummary,
    pub visits: Vec<SessionVisit>,
    pub trails: Vec<TrailSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct TrailListResult {
    pub trails: Vec<TrailSummary>,
    pub total: i64,
    pub page: u32,
    pub page_size: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct TrailMember {
    pub trail_id: String,
    pub visit_id: i64,
    pub ordinal: i64,
    pub role: String,
    pub url: String,
    pub title: Option<String>,
    pub visit_time_ms: i64,
    pub search_query: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct TrailDetail {
    pub trail: TrailSummary,
    pub members: Vec<TrailMember>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct NavigationPathStep {
    pub visit_id: i64,
    pub url: String,
    pub title: Option<String>,
    pub visit_time_ms: i64,
    pub depth: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct NavigationPath {
    pub target_visit_id: i64,
    pub steps: Vec<NavigationPathStep>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct HubPage {
    pub url: String,
    pub title: Option<String>,
    pub registrable_domain: String,
    pub trail_reference_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ArrivalBreakdown {
    pub search: i64,
    pub link: i64,
    pub typed: i64,
    pub other: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct DomainPageStat {
    pub path: String,
    pub visit_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct DomainFlowStat {
    pub domain: String,
    pub display_name: Option<String>,
    pub count: i64,
}

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

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct EngineEffectiveness {
    pub search_engine: String,
    pub display_name: Option<String>,
    pub avg_reformulations: f32,
    pub total_trails: i64,
    pub avg_depth: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct HardTopic {
    pub query_family: String,
    pub reformulation_count: i64,
    pub re_search_lag_days: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SearchEffectiveness {
    pub engine_stats: Vec<EngineEffectiveness>,
    pub top_resolving_sources: Vec<StableSource>,
    pub hardest_topics: Vec<HardTopic>,
}

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

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RhythmHeatmapCell {
    pub dow: i64,
    pub hour: i64,
    pub visit_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RhythmHeatmap {
    pub cells: Vec<RhythmHeatmapCell>,
    pub max_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveryTrendPoint {
    pub date_key: String,
    pub discovery_rate: f32,
    pub new_domain_count: i64,
    pub total_visits: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveryTrend {
    pub points: Vec<DiscoveryTrendPoint>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct CategoryMixEntry {
    pub domain_category: String,
    pub visit_count: i64,
    pub share: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct CategoryChangeEntry {
    pub domain_category: String,
    pub current_share: f32,
    pub previous_share: f32,
    pub change_points: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ActivityMix {
    pub categories: Vec<CategoryMixEntry>,
    pub change_vs_previous: Vec<CategoryChangeEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ActivityMixTrendPoint {
    pub date_key: String,
    pub categories: Vec<CategoryMixEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ActivityMixTrend {
    pub points: Vec<ActivityMixTrendPoint>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct BreadthIndex {
    pub hhi: f32,
    pub breadth_score: f32,
    pub concentration_domain_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PathFlow {
    pub flow_pattern: String,
    pub step_count: i64,
    pub occurrence_count: i64,
    pub last_seen_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct CompareSetPage {
    pub url: String,
    pub title: Option<String>,
    pub registrable_domain: String,
    pub visit_count: i64,
    pub is_landing: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct CompareSet {
    pub compare_set_id: String,
    pub search_query: String,
    pub pages: Vec<CompareSetPage>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct BrowserProfileSummary {
    pub profile_id: String,
    pub profile_name: String,
    pub browser_family: String,
    pub domain_count: i64,
    pub visit_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ExclusiveDomainEntry {
    pub registrable_domain: String,
    pub profile_id: String,
    pub visit_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct BrowserCategoryDistribution {
    pub profile_id: String,
    pub profile_name: String,
    pub categories: Vec<CategoryMixEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct BrowserDiff {
    pub profiles: Vec<BrowserProfileSummary>,
    pub exclusive_domains: Vec<ExclusiveDomainEntry>,
    pub shared_domains: Vec<String>,
    pub category_distributions: Vec<BrowserCategoryDistribution>,
}

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

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ExplainabilityFactor {
    pub label: String,
    pub raw_value: f32,
    pub weight: f32,
    pub contribution: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Explanation {
    pub entity_type: String,
    pub entity_id: String,
    pub trigger_rule: String,
    pub factors: Vec<ExplainabilityFactor>,
    pub participating_visit_ids: Vec<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct IntelligenceEmbedCardPayload {
    pub card_id: String,
    pub card_type: String,
    pub title: String,
    pub eyebrow: Option<String>,
    pub body: String,
    pub metric_label: Option<String>,
    pub metric_value: Option<String>,
    pub href: Option<String>,
    pub internal_only: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct IntelligenceWidgetSnapshot {
    pub generated_at: String,
    pub date_range: DateRange,
    pub digest_summary: DigestSummary,
    pub highlights: Vec<IntelligenceEmbedCardPayload>,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct IntelligencePublicSnapshot {
    pub generated_at: String,
    pub date_range: DateRange,
    pub digest_summary: DigestSummary,
    pub top_domains: Vec<String>,
    pub search_engines: Vec<EngineRanking>,
    pub discovery_trend: DiscoveryTrend,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
/// Request shape for one deterministic local-host artifact preview/build.
pub struct IntelligenceLocalHostRequest {
    pub date_range: DateRange,
    pub profile_id: Option<String>,
    pub locale: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
/// Machine-readable bundle saved beside one local Core Intelligence host.
pub struct IntelligenceLocalHostBundle {
    pub bundle_version: String,
    pub host_id: String,
    pub generated_at: String,
    pub locale: String,
    pub date_range: DateRange,
    pub profile_id: Option<String>,
    pub embed_cards: Vec<IntelligenceEmbedCardPayload>,
    pub widget_snapshot: IntelligenceWidgetSnapshot,
    pub public_snapshot: IntelligencePublicSnapshot,
    pub trusted_only_card_ids: Vec<String>,
    pub trusted_only_card_count: usize,
    pub boundary_notes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
/// Existing installed local-host artifact discovered on disk for verify UX.
pub struct IntelligenceInstalledLocalHost {
    pub artifact_root: String,
    pub entry_file_path: String,
    pub bundle: IntelligenceLocalHostBundle,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
/// Preview payload for one deterministic local-host artifact without writing files.
pub struct IntelligenceLocalHostPreview {
    pub artifact_root: String,
    pub entry_file_path: String,
    pub generated_files: Vec<GeneratedFile>,
    pub bundle: IntelligenceLocalHostBundle,
    pub boundary_notes: Vec<String>,
    pub manual_steps: Vec<String>,
    pub warnings: Vec<String>,
    pub installed_host: Option<IntelligenceInstalledLocalHost>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
/// Result payload after writing one deterministic local-host artifact bundle.
pub struct IntelligenceLocalHostBuildResult {
    pub artifact_root: String,
    pub entry_file_path: String,
    pub generated_files: Vec<GeneratedFile>,
    pub bundle: IntelligenceLocalHostBundle,
    pub boundary_notes: Vec<String>,
    pub manual_steps: Vec<String>,
    pub warnings: Vec<String>,
    pub installed_host: Option<IntelligenceInstalledLocalHost>,
}
