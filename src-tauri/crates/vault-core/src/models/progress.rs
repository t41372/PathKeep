//! Shared progress-log transport models.
//!
//! ## Responsibilities
//! - Carry structured log events for long-running archive work.
//! - Keep progress counters and diagnostics machine-readable for shell task UI.
//! - Preserve additive compatibility with legacy progress text payloads.
//!
//! ## Not responsible for
//! - Translating diagnostic codes into localized UI copy.
//! - Owning progress throttling, task persistence, or notification policy.
//!
//! ## Dependencies
//! - Serde camelCase transport used by Tauri event payloads.
//!
//! ## Performance notes
//! - Entries are intentionally compact and cloneable; emitters cap event tails
//!   before sending payloads to avoid large-import UI churn.

use serde::{Deserialize, Serialize};

/// Structured progress log entry emitted alongside legacy progress text.
///
/// The shell uses this event before falling back to raw log lines so record
/// counters, source labels, and diagnostics stay machine-readable during large
/// imports and backups.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProgressLogEvent {
    pub level: String,
    pub code: String,
    pub message: String,
    pub source_label: Option<String>,
    pub diagnostic: Option<String>,
    pub processed_records: Option<usize>,
    pub total_records: Option<usize>,
    pub imported_records: Option<usize>,
    pub duplicate_records: Option<usize>,
    pub skipped_records: Option<usize>,
}
