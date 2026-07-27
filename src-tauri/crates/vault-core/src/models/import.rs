//! Takeout and import-batch read models.

use super::ProgressLogEvent;
use serde::{Deserialize, Serialize};

/// Request payload for inspecting or importing a Takeout source.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TakeoutRequest {
    pub source_path: String,
    pub dry_run: bool,
}

/// Request payload for inspecting or importing one browser history database.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserHistoryImportRequest {
    pub source_path: String,
    pub dry_run: bool,
    pub browser_family: Option<String>,
    pub profile_id: Option<String>,
    pub browser_name: Option<String>,
    pub profile_name: Option<String>,
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

/// One inspection/import note carried as a stable code plus typed params.
///
/// Why this exists: the plain `notes: Vec<String>` channel ships English prose
/// straight into the review UI, so a `zh` user reads English. `code` lets the
/// front-end resolve localized copy from its catalog and interpolate the typed
/// params, while `message` stays as the English fallback for unknown codes and
/// `diagnostic` carries the untranslatable evidence (error chains) that must be
/// shown verbatim.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TakeoutNote {
    /// Stable kebab-case slug, e.g. `skipped-missing-timestamp`.
    pub code: String,
    /// English sentence the backend would have shown before code-ification.
    pub message: String,
    /// Record/row count referenced by the note, when it has one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub count: Option<u64>,
    /// Source file path or source label the note is about.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    /// RFC3339 timestamp for notes that record when an action happened.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub at: Option<String>,
    /// Run id for notes that point at a rollback/restore ledger row.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run_id: Option<i64>,
    /// Raw error chain or other diagnostic text that stays untranslated.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub diagnostic: Option<String>,
}

impl TakeoutNote {
    /// Builds a note whose whole meaning is carried by its code.
    pub fn new(code: &str, message: impl Into<String>) -> Self {
        Self { code: code.to_string(), message: message.into(), ..Self::default() }
    }

    /// Attaches the record/row count a localized sentence interpolates.
    pub fn with_count(mut self, count: usize) -> Self {
        self.count = Some(count as u64);
        self
    }

    /// Attaches the source path/label a localized sentence interpolates.
    pub fn with_source(mut self, source: impl Into<String>) -> Self {
        self.source = Some(source.into());
        self
    }

    /// Attaches the RFC3339 timestamp a localized sentence interpolates.
    pub fn with_at(mut self, at: impl Into<String>) -> Self {
        self.at = Some(at.into());
        self
    }

    /// Attaches the ledger run id a localized sentence interpolates.
    pub fn with_run_id(mut self, run_id: i64) -> Self {
        self.run_id = Some(run_id);
        self
    }

    /// Attaches verbatim diagnostic evidence that must never be translated.
    pub fn with_diagnostic(mut self, diagnostic: impl Into<String>) -> Self {
        self.diagnostic = Some(diagnostic.into());
        self
    }
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
    /// Coded mirror of `notes`; kept alongside the legacy string channel so the
    /// review UI can localize while older payloads still render.
    #[serde(default)]
    pub note_details: Vec<TakeoutNote>,
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
    /// Coded mirror of `notes`; kept alongside the legacy string channel so the
    /// wizard can localize while older payloads still render.
    #[serde(default)]
    pub note_details: Vec<TakeoutNote>,
    pub detected_locale: Option<String>,
    pub preview_range_start: Option<String>,
    pub preview_range_end: Option<String>,
}

impl TakeoutInspection {
    /// Records one note in both the legacy string channel and the coded channel.
    ///
    /// Producers call this instead of pushing to `notes` directly so the two
    /// channels can never drift — a drifted pair would make the UI localize a
    /// note the audit artifact does not contain.
    pub fn push_note(&mut self, note: TakeoutNote) {
        self.notes.push(note.message.clone());
        self.note_details.push(note);
    }

    /// Records a batch of notes while keeping both note channels aligned.
    pub fn extend_notes(&mut self, notes: impl IntoIterator<Item = TakeoutNote>) {
        for note in notes {
            self.push_note(note);
        }
    }

    /// Replaces both note channels with the persisted batch notes.
    ///
    /// Used after an import round-trips through the batch summary so the
    /// returned payload matches exactly what the audit artifact recorded.
    pub fn replace_notes(&mut self, notes: Vec<String>, note_details: Vec<TakeoutNote>) {
        self.notes = notes;
        self.note_details = note_details;
    }
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
    pub source_label: Option<String>,
    pub processed_records: Option<usize>,
    pub total_records: Option<usize>,
    pub imported_records: Option<usize>,
    pub duplicate_records: Option<usize>,
    pub skipped_records: Option<usize>,
    pub log_events: Vec<ProgressLogEvent>,
}

impl ImportProgressEvent {
    /// Attaches one structured log event using the event's current counters.
    pub fn with_log_event(mut self, level: &str, code: &str) -> Self {
        self.log_events = vec![ProgressLogEvent {
            level: level.to_string(),
            code: code.to_string(),
            message: self.detail.clone(),
            source_label: self.source_label.clone().or_else(|| self.source_path.clone()),
            diagnostic: None,
            processed_records: self.processed_records,
            total_records: self.total_records,
            imported_records: self.imported_records,
            duplicate_records: self.duplicate_records,
            skipped_records: self.skipped_records,
        }];
        self
    }
}
