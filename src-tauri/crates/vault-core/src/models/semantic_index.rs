//! Responsibilities: the desktop contract for semantic index selection and build control.
//! Not responsible for: queue policy, provider resolution or persistence.
use serde::{Deserialize, Serialize};

/// Keeps provider and model together wherever an index is selected.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EmbeddingIndexRef {
    pub provider_id: String,
    pub model: String,
}

/// Distinguishes pauses that require individual consent from a queue pause.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum IndexBuildPause {
    User,
    Service,
    Queue,
}

/// Exposes durable visit progress without counting archive or embedding rows on each poll.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IndexBuildJob {
    pub job_id: i64,
    pub state: String,
    pub paused_by: Option<IndexBuildPause>,
    pub pause_message: Option<String>,
    pub done: u64,
    pub total: u64,
}

/// Shows which model the user chose and which index can currently serve searches.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SemanticIndexState {
    pub chosen: Option<EmbeddingIndexRef>,
    pub serving: Option<EmbeddingIndexRef>,
    pub chosen_complete: bool,
    pub build: Option<IndexBuildJob>,
}
