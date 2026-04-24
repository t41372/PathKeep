//! Shared Core Intelligence request and envelope primitives.
//!
//! ## Responsibilities
//! - Define date ranges, KPI deltas, rebuild reports, section envelopes, and
//!   entity references used by multiple payload families.
//! - Preserve shared serde tags and camelCase field names.
//! - Keep generic wrappers independent from route-specific payload structs.
//!
//! ## Not responsible for
//! - Defining concrete overview cards or analytics rows.
//! - Validating date ranges against archive contents.
//! - Describing worker queue internals beyond rebuild report DTOs.
//!
//! ## Dependencies
//! - `serde` for the backend/frontend transport contract.
//!
//! ## Performance notes
//! - Generic envelopes should wrap already-bounded payloads; they must not
//!   encourage callers to return unpaginated visit-scale vectors.

use serde::{Deserialize, Serialize};

/// Inclusive local-date range used by Core Intelligence query commands.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DateRange {
    pub start: String,
    pub end: String,
}

/// Period-over-period KPI metadata used by digest cards.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct KpiMetric {
    pub value: i64,
    pub previous_value: Option<i64>,
    pub change_percent: Option<f32>,
    pub trend: String,
}

/// Request payload for a full or scoped Core Intelligence rebuild.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct CoreIntelligenceRebuildRequest {
    pub profile_id: Option<String>,
    pub full_rebuild: bool,
    pub limit: Option<u32>,
}

/// Stage-by-stage timing summary emitted for full Core Intelligence rebuilds.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct CoreIntelligenceStageTimings {
    pub visit_derive_ms: u64,
    pub daily_rollup_ms: u64,
    pub structural_rebuild_ms: u64,
    pub total_ms: u64,
}

/// Result payload returned after a Core Intelligence rebuild finishes.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
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

/// Queue acknowledgement for a manual Core Intelligence rebuild request.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct CoreIntelligenceQueueReport {
    pub job_id: i64,
    pub state: String,
    pub notes: Vec<String>,
}

/// Structured window metadata for one Core Intelligence section response.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum CoreIntelligenceSectionWindow {
    DateRange {
        #[serde(rename = "dateRange")]
        date_range: DateRange,
    },
    CalendarDayHistory {
        #[serde(rename = "referenceDate")]
        reference_date: String,
    },
}

/// Shared metadata emitted alongside one `/intelligence` section payload.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
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

/// Generic transport envelope for one Core Intelligence section payload.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CoreIntelligenceSectionResult<T> {
    pub data: T,
    pub meta: CoreIntelligenceSectionMeta,
}

/// Per-section timing sample emitted by staged overview commands.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct CoreIntelligenceSectionTiming {
    pub section_id: String,
    pub duration_ms: u64,
}

/// Reusable shared-entity reference carried by trusted output payloads.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum InsightEntityReference {
    Day { date: String },
    Domain { domain: String },
    QueryFamily { family_id: String },
    RefindPage { canonical_url: String },
    Session { session_id: String },
    Trail { trail_id: String },
    CompareSet { compare_set_id: String },
}
