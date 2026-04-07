use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HistoryDatabaseSet {
    pub history_path: PathBuf,
    pub favicons_path: Option<PathBuf>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChromiumReadCursor {
    pub after_visit_id: i64,
    pub after_url_last_visit_time: i64,
    pub after_download_id: i64,
    pub after_favicon_last_updated: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParserWarning {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DatabaseInspection {
    pub table_names: Vec<String>,
    pub warnings: Vec<ParserWarning>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParsedUrl {
    pub source_url_id: i64,
    pub url: String,
    pub title: Option<String>,
    pub visit_count: i64,
    pub typed_count: i64,
    pub last_visit_ms: i64,
    pub last_visit_iso: String,
    pub hidden: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParsedVisit {
    pub source_visit_id: i64,
    pub source_url_id: i64,
    pub url: String,
    pub title: Option<String>,
    pub visit_time_ms: i64,
    pub visit_time_iso: String,
    pub from_visit: Option<i64>,
    pub transition: Option<i64>,
    pub visit_duration_ms: Option<i64>,
    pub is_known_to_sync: bool,
    pub visited_link_id: Option<i64>,
    pub external_referrer_url: Option<String>,
    pub app_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParsedDownload {
    pub source_download_id: i64,
    pub guid: Option<String>,
    pub current_path: Option<String>,
    pub target_path: Option<String>,
    pub start_time_ms: Option<i64>,
    pub start_time_iso: Option<String>,
    pub received_bytes: Option<i64>,
    pub total_bytes: Option<i64>,
    pub state: Option<i64>,
    pub mime_type: Option<String>,
    pub original_mime_type: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParsedSearchTerm {
    pub keyword_id: i64,
    pub url_id: i64,
    pub term: String,
    pub normalized_term: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParsedFavicon {
    pub page_url: String,
    pub icon_url: String,
    pub icon_type: Option<i64>,
    pub width: i64,
    pub height: i64,
    pub last_updated_ms: i64,
    pub last_updated_iso: String,
    pub image_data: Option<Vec<u8>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChromiumHistory {
    pub inspection: DatabaseInspection,
    pub urls: Vec<ParsedUrl>,
    pub visits: Vec<ParsedVisit>,
    pub downloads: Vec<ParsedDownload>,
    pub search_terms: Vec<ParsedSearchTerm>,
    pub favicons: Vec<ParsedFavicon>,
    pub warnings: Vec<ParserWarning>,
}
