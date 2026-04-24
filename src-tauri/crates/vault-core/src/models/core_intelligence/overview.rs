//! Batched Core Intelligence overview DTOs.
//!
//! ## Responsibilities
//! - Define digest, primary overview, secondary overview, day insights, and
//!   "On This Day" payload shapes.
//! - Keep batched section responses separate from the individual row DTOs they
//!   embed.
//! - Preserve route-facing overview contracts while implementation owners move.
//!
//! ## Not responsible for
//! - Computing section state metadata.
//! - Owning individual analytics/read row definitions.
//! - Managing frontend rendering order.
//!
//! ## Dependencies
//! - Shared section envelopes.
//! - Read-row and analytics DTO families.
//!
//! ## Performance notes
//! - Overview payloads bundle multiple sections to reduce IPC fan-out, but each
//!   embedded vector must still be bounded by the underlying section loader.

use super::analytics::DiscoveryTrend;
use super::{
    analytics::{
        ActivityMix, BreadthIndex, BrowserDiff, CompareSet, FrictionSignal, HabitPattern,
        InterruptedHabit, ObservedInteraction, PathFlow, ReopenedInvestigation,
        SearchEffectiveness, StableSource,
    },
    reads::{EngineRanking, QueryFamilyResult, RefindPage, SearchConcept, TopSite},
    shared::{CoreIntelligenceSectionResult, CoreIntelligenceSectionTiming, DateRange, KpiMetric},
};
use serde::{Deserialize, Serialize};

/// Top-line Core Intelligence digest card payload.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct DigestSummary {
    pub date_range: DateRange,
    pub total_visits: KpiMetric,
    pub total_searches: KpiMetric,
    pub new_domains: KpiMetric,
    pub deep_read_pages: KpiMetric,
    pub refind_pages: KpiMetric,
}

/// Batched first-band `/intelligence` payload used to avoid foreground IPC fan-out.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CoreIntelligencePrimaryOverview {
    pub digest_summary: CoreIntelligenceSectionResult<DigestSummary>,
    pub on_this_day: CoreIntelligenceSectionResult<Vec<OnThisDayEntry>>,
    pub top_sites: CoreIntelligenceSectionResult<Vec<TopSite>>,
    pub refind_pages: CoreIntelligenceSectionResult<Vec<RefindPage>>,
    pub search_engine_ranking: CoreIntelligenceSectionResult<Vec<EngineRanking>>,
    pub top_search_concepts: CoreIntelligenceSectionResult<Vec<SearchConcept>>,
    pub query_families: CoreIntelligenceSectionResult<QueryFamilyResult>,
    pub activity_mix: CoreIntelligenceSectionResult<ActivityMix>,
    pub discovery_trend_day: CoreIntelligenceSectionResult<DiscoveryTrend>,
    pub habit_patterns: CoreIntelligenceSectionResult<Vec<HabitPattern>>,
    pub interrupted_habits: CoreIntelligenceSectionResult<Vec<InterruptedHabit>>,
    pub timings: Vec<CoreIntelligenceSectionTiming>,
    pub total_duration_ms: u64,
}

/// Batched deferred `/intelligence` payload for below-the-fold sections.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CoreIntelligenceSecondaryOverview {
    pub stable_sources: CoreIntelligenceSectionResult<Vec<StableSource>>,
    pub search_effectiveness: CoreIntelligenceSectionResult<SearchEffectiveness>,
    pub friction_signals: CoreIntelligenceSectionResult<Vec<FrictionSignal>>,
    pub reopened_investigations: CoreIntelligenceSectionResult<Vec<ReopenedInvestigation>>,
    pub discovery_trend_week: CoreIntelligenceSectionResult<DiscoveryTrend>,
    pub breadth_index: CoreIntelligenceSectionResult<BreadthIndex>,
    pub path_flows: CoreIntelligenceSectionResult<Vec<PathFlow>>,
    pub compare_sets: CoreIntelligenceSectionResult<Vec<CompareSet>>,
    pub multi_browser_diff: CoreIntelligenceSectionResult<BrowserDiff>,
    pub observed_interactions: CoreIntelligenceSectionResult<Vec<ObservedInteraction>>,
    pub timings: Vec<CoreIntelligenceSectionTiming>,
    pub total_duration_ms: u64,
}

/// Explorer drilldown metadata emitted with one day insights payload.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct DayInsightsDrilldown {
    pub explorer_date_range: DateRange,
}

/// One hourly activity bucket inside a day-level insights payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct DayInsightsHourlyBucket {
    pub hour: i64,
    pub visit_count: i64,
}

/// Full deterministic read model for one exact local calendar day.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct DayInsights {
    pub date: String,
    pub digest_summary: DigestSummary,
    pub top_sites: Vec<TopSite>,
    pub activity_mix: ActivityMix,
    pub refind_pages: Vec<RefindPage>,
    pub query_families: QueryFamilyResult,
    pub hourly_activity: Vec<DayInsightsHourlyBucket>,
    pub drilldown: DayInsightsDrilldown,
}

/// One historical "On This Day" entry.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct OnThisDayEntry {
    pub year: i32,
    pub date: String,
    pub total_visits: i64,
    pub top_domains: Vec<String>,
    pub summary: Option<String>,
    pub deep_dive_sessions: i64,
}
