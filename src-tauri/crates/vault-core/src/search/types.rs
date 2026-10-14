//! Responsibilities: serializable search request and channel contract.
//! Not responsible for: scheduling, query execution or UI copy.
//! Failure modes: partial counts look exact; cursors escape their session; empty builds
//! hide missing coverage. Tests: search::tests and worker streaming contract tests.
use super::IndexerState;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SearchRequest {
    pub query: String,
    /// Internal optional-AI ranking identity; never accepted from an IPC payload.
    #[serde(skip)]
    pub ranking_key: Option<String>,
    pub sort: Option<String>,
    pub page_size: usize,
    pub profile_id: Option<String>,
    pub browser_kind: Option<String>,
    pub domain: Option<String>,
    pub start_time_ms: Option<i64>,
    pub end_time_ms: Option<i64>,
    pub regex: bool,
    pub fuzzy: bool,
    pub starred: bool,
}
impl Default for SearchRequest {
    fn default() -> Self {
        Self {
            query: String::new(),
            ranking_key: None,
            sort: None,
            page_size: 100,
            profile_id: None,
            browser_kind: None,
            domain: None,
            start_time_ms: None,
            end_time_ms: None,
            regex: false,
            fuzzy: false,
            starred: false,
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchHit {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    pub page_id: i64,
    pub score: f32,
    pub visit: crate::HistoryEntry,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchCount {
    pub value: u64,
    pub exact: bool,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchProgress {
    pub documents_processed: u64,
    pub segments_completed: usize,
    pub segments_total: usize,
    pub statistics_fields_processed: usize,
    pub statistics_fields_total: usize,
}
impl From<vault_search::Progress> for SearchProgress {
    fn from(p: vault_search::Progress) -> Self {
        Self {
            documents_processed: p.documents_processed,
            segments_completed: p.segments_completed,
            segments_total: p.segments_total,
            statistics_fields_processed: p.statistics_fields_processed,
            statistics_fields_total: p.statistics_fields_total,
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchStep {
    /// Identifies the output page while count progress continues on the same channel.
    pub page: u64,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub search_notes: Vec<crate::AiSearchNote>,
    pub items: Vec<SearchHit>,
    pub provisional: bool,
    pub progress: SearchProgress,
    pub count: SearchCount,
    pub visits: SearchCount,
    pub count_progress: SearchProgress,
    pub cursor: Option<String>,
    pub indexer: IndexerState,
    pub snapshot_seq: u64,
    pub coverage_complete: bool,
}

/// Distinguishes exact counts within a pinned reader from complete archive coverage.
#[derive(Clone, Debug, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SearchCoverage {
    pub indexer: IndexerState,
    pub snapshot_seq: u64,
    pub coverage_complete: bool,
}
impl SearchStep {
    pub fn coverage(&self) -> SearchCoverage {
        SearchCoverage {
            indexer: self.indexer.clone(),
            snapshot_seq: self.snapshot_seq,
            coverage_complete: self.coverage_complete,
        }
    }
}
