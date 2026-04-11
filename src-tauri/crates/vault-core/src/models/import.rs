use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TakeoutRequest {
    pub source_path: String,
    pub dry_run: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct TakeoutFileReport {
    pub path: String,
    pub kind: String,
    pub status: String,
    pub records: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct TakeoutPreviewEntry {
    pub source_path: String,
    pub url: String,
    pub title: Option<String>,
    pub visited_at: String,
    pub source_visit_id: i64,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ImportBatchOverview {
    pub id: i64,
    pub source_kind: String,
    pub source_path: String,
    pub profile_id: String,
    pub created_at: String,
    pub imported_at: Option<String>,
    pub reverted_at: Option<String>,
    pub status: String,
    pub candidate_items: usize,
    pub imported_items: usize,
    pub duplicate_items: usize,
    pub visible_items: usize,
    pub audit_path: Option<String>,
    pub git_commit: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ImportBatchDetail {
    pub batch: ImportBatchOverview,
    pub preview_entries: Vec<TakeoutPreviewEntry>,
    pub recognized_files: Vec<TakeoutFileReport>,
    pub quarantined_files: Vec<TakeoutFileReport>,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct TakeoutInspection {
    pub source_path: String,
    pub dry_run: bool,
    pub recognized_files: Vec<TakeoutFileReport>,
    pub quarantined_files: Vec<TakeoutFileReport>,
    pub candidate_items: usize,
    pub imported_items: usize,
    pub duplicate_items: usize,
    pub preview_entries: Vec<TakeoutPreviewEntry>,
    pub import_batch: Option<ImportBatchOverview>,
    pub notes: Vec<String>,
}
