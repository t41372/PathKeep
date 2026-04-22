//! Takeout and import-batch read models.

use serde::{Deserialize, Serialize};

/// Request payload for inspecting or importing a Takeout source.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TakeoutRequest {
    pub source_path: String,
    pub dry_run: bool,
}

/// Summary of one recognized or quarantined file inside a Takeout source.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct TakeoutFileReport {
    pub path: String,
    pub kind: String,
    pub status: String,
    pub records: usize,
    pub classification: String,
    pub reason_code: Option<String>,
    pub reason_detail: Option<String>,
    pub detected_locale: Option<String>,
}

/// One preview visit shown before or after a Takeout import.
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

/// Compact summary for one recorded import batch.
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

/// Detailed import-batch view with preview rows and file reports.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ImportBatchDetail {
    pub batch: ImportBatchOverview,
    pub preview_entries: Vec<TakeoutPreviewEntry>,
    pub recognized_files: Vec<TakeoutFileReport>,
    pub quarantined_files: Vec<TakeoutFileReport>,
    pub notes: Vec<String>,
    pub detected_locale: Option<String>,
    pub preview_range_start: Option<String>,
    pub preview_range_end: Option<String>,
}

/// Full inspection/import payload returned by the Takeout flow.
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
    pub detected_locale: Option<String>,
    pub preview_range_start: Option<String>,
    pub preview_range_end: Option<String>,
}

/// Progress event streamed while a Takeout import is running.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ImportProgressEvent {
    pub phase: String,
    pub label: String,
    pub detail: String,
    pub current: usize,
    pub total: usize,
    pub progress_percent: Option<f32>,
    pub log_lines: Vec<String>,
    pub source_path: Option<String>,
}
