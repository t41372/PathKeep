//! Core Intelligence command request DTOs.
//!
//! ## Responsibilities
//! - Group all request payloads accepted by Core Intelligence Tauri commands,
//!   worker calls, and dev IPC bridge helpers.
//! - Preserve existing camelCase fields and aliases.
//! - Keep pagination/profile/date-window knobs explicit at the transport edge.
//!
//! ## Not responsible for
//! - Executing validation, defaulting, or database access.
//! - Returning read-model rows.
//! - Defining trusted local-host output payloads beyond their request shape.
//!
//! ## Dependencies
//! - Shared `DateRange` DTO.
//! - `serde` for IPC transport.
//!
//! ## Performance notes
//! - List-style requests expose `page`, `page_size`, or `limit`; callers should
//!   enforce bounded reads before materializing response DTOs.

use super::shared::DateRange;
use serde::{Deserialize, Serialize};

/// Common request shape for paginated Core Intelligence list queries.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PagedDateRangeRequest {
    pub date_range: DateRange,
    pub profile_id: Option<String>,
    #[serde(default)]
    pub cursor: Option<String>,
    pub page_size: u32,
}

/// Common request shape for non-paginated, profile-scoped Core Intelligence reads.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ScopedDateRangeRequest {
    pub date_range: DateRange,
    pub profile_id: Option<String>,
}

/// A date/profile list whose limit bounds one page and whose cursor reaches the remainder.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ScopedPageRequest {
    pub date_range: DateRange,
    pub profile_id: Option<String>,
    #[serde(default)]
    pub limit: Option<u32>,
    #[serde(default)]
    pub cursor: Option<String>,
}
impl From<&ScopedDateRangeRequest> for ScopedPageRequest {
    fn from(scope: &ScopedDateRangeRequest) -> Self {
        Self {
            date_range: scope.date_range.clone(),
            profile_id: scope.profile_id.clone(),
            ..Default::default()
        }
    }
}

/// Common request shape for profile-scoped reads that do not need a date window.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ProfileScopedRequest {
    pub profile_id: Option<String>,
}

/// Request shape for engine-scoped or filtered search-trail reads.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SearchTrailQueryRequest {
    pub date_range: DateRange,
    pub profile_id: Option<String>,
    pub engine: Option<String>,
    #[serde(default)]
    pub cursor: Option<String>,
    pub page_size: u32,
}

/// Request shape for top-sites queries.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct TopSitesRequest {
    #[serde(default)]
    pub cursor: Option<String>,
    pub date_range: DateRange,
    pub profile_id: Option<String>,
    /// Several profiles summed together (one browser can have several); takes
    /// precedence over `profile_id`. Empty with no `profile_id` means every profile.
    #[serde(default)]
    pub profile_ids: Vec<String>,
    pub sort_by: Option<String>,
    pub limit: Option<u32>,
}

/// Request shape for one domain trend series.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct DomainTrendRequest {
    #[serde(alias = "domain")]
    pub registrable_domain: String,
    pub date_range: DateRange,
}

/// Request shape for the most frequent searches in a date range.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct FrequentSearchesRequest {
    #[serde(default)]
    pub cursor: Option<String>,
    pub date_range: DateRange,
    pub profile_id: Option<String>,
    /// Page size; continuation remains available after the first 100 queries.
    pub limit: Option<u32>,
}

/// Request shape for top search concepts.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct TopSearchConceptsRequest {
    #[serde(default)]
    pub cursor: Option<String>,
    pub date_range: DateRange,
    pub profile_id: Option<String>,
    pub limit: Option<u32>,
}

/// Request shape for paginated search-query history reads.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SearchQueryListRequest {
    pub date_range: DateRange,
    pub profile_id: Option<String>,
    pub browser_kind: Option<String>,
    pub engine: Option<String>,
    pub domain: Option<String>,
    pub query: Option<String>,
    pub sort: Option<String>,
    #[serde(default)]
    pub cursor: Option<String>,
    pub page_size: u32,
}

/// Editable payload for one custom search-engine rule.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SearchEngineRuleInput {
    pub rule_id: Option<String>,
    pub engine_id: String,
    pub display_name: String,
    pub host_pattern: String,
    pub path_prefix: Option<String>,
    pub query_param_key: String,
    pub enabled: bool,
    pub note: Option<String>,
    pub example_url: Option<String>,
}

/// Request shape for refind-page queries.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RefindPagesRequest {
    #[serde(default)]
    pub cursor: Option<String>,
    pub date_range: DateRange,
    pub profile_id: Option<String>,
    pub limit: Option<u32>,
}

/// Request shape for one refind explanation lookup.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ExplainRefindRequest {
    #[serde(default)]
    pub profile_id: Option<String>,
    #[serde(default)]
    pub cursor: Option<String>,
    #[serde(default)]
    pub page_size: Option<u32>,

    pub canonical_url: String,
}

/// Request shape for granularity-based trend queries.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct GranularityDateRangeRequest {
    pub date_range: DateRange,
    pub profile_id: Option<String>,
    pub granularity: String,
}

/// Request shape for optional category filters on rhythm/activity queries.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct CategoryFilteredDateRangeRequest {
    pub date_range: DateRange,
    pub profile_id: Option<String>,
    pub category: Option<String>,
}

/// Request shape for one search effectiveness read.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SearchEffectivenessRequest {
    #[serde(default)]
    pub topics_cursor: Option<String>,
    #[serde(default)]
    pub topics_limit: Option<u32>,
    pub date_range: DateRange,
    pub profile_id: Option<String>,
    pub engine: Option<String>,
}

/// Request shape for one domain deep-dive read.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct DomainDeepDiveRequest {
    #[serde(default)]
    pub pages_cursor: Option<String>,
    #[serde(default)]
    pub searches_cursor: Option<String>,
    #[serde(default)]
    pub page_size: Option<u32>,
    #[serde(alias = "domain")]
    pub registrable_domain: String,
    pub date_range: DateRange,
    pub profile_id: Option<String>,
}

/// Every query family that holds one search, across all time.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SearchFamiliesRequest {
    /// The search as typed; matched ignoring case and surrounding spaces.
    pub query: String,
    pub profile_id: Option<String>,
}

/// Request shape for one query-family detail read.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct QueryFamilyDetailRequest {
    #[serde(default)]
    pub cursor: Option<String>,
    #[serde(default)]
    pub page_size: Option<u32>,
    pub family_id: String,
    pub date_range: DateRange,
    pub profile_id: Option<String>,
}

/// Request shape for one refind-page detail read.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RefindPageDetailRequest {
    #[serde(default)]
    pub visits_cursor: Option<String>,
    #[serde(default)]
    pub trails_cursor: Option<String>,
    #[serde(default)]
    pub page_size: Option<u32>,

    pub canonical_url: String,
    pub date_range: DateRange,
    pub profile_id: Option<String>,
}

/// Request shape for one local-calendar-day insights read.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct DayInsightsRequest {
    pub date: String,
    pub profile_id: Option<String>,
}

/// Request shape for path-flow reads.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PathFlowRequest {
    #[serde(default)]
    pub cursor: Option<String>,
    pub date_range: DateRange,
    pub profile_id: Option<String>,
    pub step_count: u32,
    pub limit: Option<u32>,
}

/// Request shape for one compare-set detail read.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct CompareSetDetailRequest {
    pub compare_set_id: String,
    pub date_range: DateRange,
    pub profile_id: Option<String>,
}

/// Request shape for shareable/embed-oriented intelligence payloads.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct IntelligenceEmbedCardsRequest {
    #[serde(default)]
    pub cursor: Option<String>,
    pub date_range: DateRange,
    pub profile_id: Option<String>,
    pub limit: Option<u32>,
}

/// Request shape for generic explainability lookups.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct EntityExplanationRequest {
    #[serde(default)]
    pub cursor: Option<String>,
    #[serde(default)]
    pub page_size: Option<u32>,

    pub entity_type: String,
    pub entity_id: String,
}

/// Request shape for one deterministic local-host artifact preview/build.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct IntelligenceLocalHostRequest {
    pub date_range: DateRange,
    pub profile_id: Option<String>,
    pub locale: String,
}

/// Navigation detail with independent visit and related-trail continuation.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SessionDetailRequest {
    pub session_id: String,
    pub visits_cursor: Option<String>,
    pub trails_cursor: Option<String>,
    pub page_size: Option<u32>,
}
/// A trail's members remain reachable regardless of its length.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct TrailDetailRequest {
    pub trail_id: String,
    pub cursor: Option<String>,
    pub page_size: Option<u32>,
}
