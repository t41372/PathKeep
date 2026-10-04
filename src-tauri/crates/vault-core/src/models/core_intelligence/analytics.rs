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

/// One page on a site inside a domain deep dive: a URL string and its
/// visits in the window, across every selected profile.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct DomainPageStat {
    pub url: String,
    pub title: Option<String>,
    /// The URL without scheme and host, for display.
    pub path: String,
    pub visit_count: i64,
}

/// A search whose trail ended on the site, with how often it did in the
/// window.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct DomainSearchStat {
    pub query: String,
    pub count: i64,
}

/// Full read model for one registrable-domain deep dive.
///
/// Totals, active days and the trend come from `domain_daily_rollups`, the
/// same rows Top sites reads, so the numbers agree with it.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct DomainDeepDive {
    pub registrable_domain: String,
    pub display_name: Option<String>,
    pub domain_category: String,
    pub total_visits: i64,
    pub active_days: i64,
    /// Distinct URLs on the site visited in the window.
    pub page_count: i64,
    /// Searches whose trail landed on this site in the window.
    pub landing_search_count: i64,
    /// Most visited URLs, at most 10.
    pub top_pages: Vec<DomainPageStat>,
    /// Searches that most often landed here, at most 8.
    pub landing_searches: Vec<DomainSearchStat>,
    /// Visits per local day; days without visits are left out.
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
///
/// `trigger_rule` is English DIAGNOSTIC prose. The user-facing surface reads `trigger_rule_code` plus
/// the structured params below instead: the Explainability Panel used to reverse-engineer this prose
/// with regexes to recover the numbers it needed for localization, which meant the front end had to
/// recite backend sentences to stay translated. The code + params make that round trip unnecessary.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Explanation {
    pub entity_type: String,
    pub entity_id: String,
    pub trigger_rule: String,
    /// Stable, locale-independent CODE for the rule that produced this entity.
    ///
    /// One of the `TRIGGER_RULE_*` constants on this type. Additive: `None` on any payload whose rule
    /// has not been coded yet (and on every pre-existing persisted/mocked payload), which the front end
    /// answers by falling back to its legacy prose matching. The paired params below are populated
    /// only for the codes that need them, so the front end never parses a sentence to get a number.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trigger_rule_code: Option<String>,
    /// Score threshold the entity crossed — set with [`Explanation::TRIGGER_RULE_REFIND_SCORE`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trigger_rule_score: Option<f32>,
    /// Anchor query the rule keyed on — set with [`Explanation::TRIGGER_RULE_SEARCH_TRAIL`] and
    /// [`Explanation::TRIGGER_RULE_QUERY_FAMILY`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trigger_rule_query: Option<String>,
    /// Habit-cadence TYPE code (`daily_habit` / `weekly_habit` / `periodic_reference`) — set with the
    /// two [`Explanation::TRIGGER_RULE_HABIT_PATTERN`] variants.
    ///
    /// The raw persisted code, not a rendered name: the front end already owns the habit-type →
    /// localized-name table, so handing it the code lets it skip scraping the name out of the prose.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trigger_rule_habit_type: Option<String>,
    /// Session-grouping gap threshold in MINUTES — set with [`Explanation::TRIGGER_RULE_SESSION_GAP`].
    ///
    /// Derived from the one `SESSION_GAP_MS` constant the grouper actually uses, so the localized copy
    /// can interpolate the real threshold instead of hard-coding "30 minutes" in three catalogs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trigger_rule_gap_minutes: Option<i64>,
    pub factors: Vec<ExplainabilityFactor>,
    pub participating_visit_ids: Vec<i64>,
}

impl Explanation {
    /// A refind page crossed its refind-score threshold (param: [`Self::trigger_rule_score`]).
    pub const TRIGGER_RULE_REFIND_SCORE: &'static str = "refind-score";
    /// A session crossed the deep-dive thresholds (no params).
    pub const TRIGGER_RULE_SESSION_DEEP_DIVE: &'static str = "session-deep-dive";
    /// Visits grouped into one session by adjacent-gap proximity (param:
    /// [`Self::trigger_rule_gap_minutes`]).
    pub const TRIGGER_RULE_SESSION_GAP: &'static str = "session-gap";
    /// A search trail anchored on a query and extended through navigation ancestry (param:
    /// [`Self::trigger_rule_query`]).
    pub const TRIGGER_RULE_SEARCH_TRAIL: &'static str = "search-trail";
    /// Queries merged into one family by similarity to an anchor (param: [`Self::trigger_rule_query`]).
    pub const TRIGGER_RULE_QUERY_FAMILY: &'static str = "query-family";
    /// An investigation reopened because its anchor reappeared across days (no params).
    pub const TRIGGER_RULE_REOPENED_INVESTIGATION: &'static str = "reopened-investigation";
    /// A habit cadence was detected from repeated cross-day visits (param:
    /// [`Self::trigger_rule_habit_type`]).
    pub const TRIGGER_RULE_HABIT_PATTERN: &'static str = "habit-pattern";
    /// A detected habit cadence later crossed its interruption threshold (param:
    /// [`Self::trigger_rule_habit_type`]).
    pub const TRIGGER_RULE_HABIT_PATTERN_INTERRUPTED: &'static str = "habit-pattern-interrupted";
    /// A flow pattern recurs across session-local domain n-grams (no params).
    pub const TRIGGER_RULE_PATH_FLOW: &'static str = "path-flow";
    /// A compare set alternated between comparable pages inside one trail (no params).
    pub const TRIGGER_RULE_COMPARE_SET: &'static str = "compare-set";
}
