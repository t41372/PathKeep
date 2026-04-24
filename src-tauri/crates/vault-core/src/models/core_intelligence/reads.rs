//! Core Intelligence route-level read model DTOs.
//!
//! ## Responsibilities
//! - Define rows and result wrappers returned by session, trail, query, domain,
//!   top-site, and refind read endpoints.
//! - Keep read-model payloads transport-only and serde-stable.
//! - Share common row shapes across overview and detail endpoints.
//!
//! ## Not responsible for
//! - Higher-level overview batching envelopes.
//! - Advanced analytics payloads such as path flows or browser diffs.
//! - SQL, scoring, pagination, or enrichment lookups.
//!
//! ## Dependencies
//! - `serde` for command transport.
//!
//! ## Performance notes
//! - Result wrappers include pagination metadata where rows may scale with
//!   archive size. Callers must keep vectors bounded before constructing them.

use serde::{Deserialize, Serialize};

/// One ranked domain in top-site and digest-adjacent surfaces.
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

/// One point in a domain trend time series.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct DomainTrendPoint {
    pub date_key: String,
    pub visit_count: i64,
}

/// Date-keyed visit trend for one registrable domain.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct DomainTrend {
    pub registrable_domain: String,
    pub points: Vec<DomainTrendPoint>,
}

/// Search-engine aggregate row used by ranking and public snapshots.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct EngineRanking {
    pub search_engine: String,
    pub display_name: Option<String>,
    pub search_count: i64,
}

/// Search term aggregate used by top-concept surfaces.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SearchConcept {
    pub term: String,
    pub frequency: i64,
    pub engines: Vec<String>,
}

/// One normalized search-query row for paginated query history.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SearchQueryRow {
    pub visit_id: i64,
    pub profile_id: String,
    pub browser_kind: String,
    pub search_engine: String,
    pub display_name: Option<String>,
    pub raw_query: String,
    pub normalized_query: String,
    pub searched_at: String,
    pub searched_at_ms: i64,
    pub exact_repeat_count: i64,
    pub family_count: i64,
    pub family_id: Option<String>,
    pub trail_id: Option<String>,
    pub trail_initial_query: Option<String>,
    pub trail_reformulation_count: Option<i64>,
}

/// Paginated search-query result set.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SearchQueryListResult {
    pub rows: Vec<SearchQueryRow>,
    pub total: i64,
    pub page: u32,
    pub page_size: u32,
}

/// Settings-facing merged search-engine rule row.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SearchEngineRule {
    pub rule_id: String,
    pub engine_id: String,
    pub display_name: String,
    pub host_pattern: String,
    pub path_prefix: Option<String>,
    pub query_param_key: String,
    pub enabled: bool,
    pub note: Option<String>,
    pub example_url: Option<String>,
    pub built_in: bool,
}

/// Query-family summary produced by structural search aggregation.
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

/// Paginated query-family result set.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct QueryFamilyResult {
    pub families: Vec<QueryFamily>,
    pub total: i64,
    pub page: u32,
    pub page_size: u32,
}

/// Query-family detail payload with related search trails.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct QueryFamilyDetail {
    pub family: QueryFamily,
    pub related_trails: Vec<TrailSummary>,
}

/// Refind-page row summarizing cross-day reuse of one canonical URL.
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

/// One factor contributing to a refind score explanation.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RefindScoreFactor {
    pub signal: String,
    pub raw_value: f32,
    pub weight: f32,
    pub contribution: f32,
}

/// Explainability payload for one refind page.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RefindExplanation {
    pub canonical_url: String,
    pub refind_score: f32,
    pub factors: Vec<RefindScoreFactor>,
    pub visit_ids: Vec<i64>,
}

/// Detail payload for one refind page.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RefindPageDetail {
    pub page: RefindPage,
    pub explanation: RefindExplanation,
    pub recent_days: Vec<String>,
    pub related_trails: Vec<TrailSummary>,
}

/// Session summary row used by session lists and related-detail surfaces.
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

/// Paginated session list result.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SessionListResult {
    pub sessions: Vec<SessionSummary>,
    pub total: i64,
    pub page: u32,
    pub page_size: u32,
}

/// Visit row included in a session detail payload.
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

/// Search trail summary row.
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

/// Full session detail with visits and trails.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SessionDetail {
    pub session: SessionSummary,
    pub visits: Vec<SessionVisit>,
    pub trails: Vec<TrailSummary>,
}

/// Paginated trail list result.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct TrailListResult {
    pub trails: Vec<TrailSummary>,
    pub total: i64,
    pub page: u32,
    pub page_size: u32,
}

/// Visit membership row within one search trail.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct TrailMember {
    pub trail_id: String,
    pub visit_id: i64,
    pub ordinal: i64,
    pub role: String,
    pub url: String,
    pub canonical_url: Option<String>,
    pub title: Option<String>,
    pub registrable_domain: Option<String>,
    pub visit_time_ms: i64,
    pub search_query: Option<String>,
}

/// Full trail detail with ordered members.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct TrailDetail {
    pub trail: TrailSummary,
    pub members: Vec<TrailMember>,
}

/// One navigation step in a traced path to a target visit.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct NavigationPathStep {
    pub visit_id: i64,
    pub url: String,
    pub title: Option<String>,
    pub visit_time_ms: i64,
    pub depth: i64,
}

/// Navigation path from a source visit to a target visit.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct NavigationPath {
    pub target_visit_id: i64,
    pub steps: Vec<NavigationPathStep>,
}

/// Hub page row summarizing repeated trail landings.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct HubPage {
    pub url: String,
    pub title: Option<String>,
    pub registrable_domain: String,
    pub trail_reference_count: i64,
}
