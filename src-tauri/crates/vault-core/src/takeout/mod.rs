//! Responsibilities: expose preview, import, review and reversible batch operations.
//! Not responsible for: database ownership, browser parsing or search indexing.
//! Failure modes: preview mutates facts, imports lose evidence, review loses
//! provenance, revert/restore diverges from search. Tests: takeout and batch_search_tests.

mod batch_review;
pub(crate) use self::batch_review::import_batches_from;
mod batches;
mod browser_history;
mod import_flow;
mod import_run;
mod inspect;
mod payload_import;
mod source_reader;
mod visibility;

#[cfg(test)]
mod tests;

use crate::{
    archive::{
        SourceBatchInput, record_schema_observation, upsert_source_batch, visit_event_fingerprint,
    },
    config::{ProjectPaths, ensure_paths},
    git_audit,
    models::{
        AppConfig, BrowserHistoryImportRequest, ImportBatchDetail, ImportBatchOverview,
        ImportProgressEvent, TakeoutFileReport, TakeoutInspection, TakeoutNote,
        TakeoutPreviewEntry, TakeoutRequest,
    },
    utils::{now_rfc3339, sha256_hex},
};
use anyhow::{Context, Result};
use browser_history_parser::takeout::{
    KIND_INDEX, TakeoutPathDisposition, TakeoutPathMatch,
    classify_payload_path_with_sniff as classify_takeout_payload_with_sniff,
};
#[cfg(test)]
use browser_history_parser::takeout::{
    TakeoutPayloadReport, parse_payload as parse_takeout_payload,
};
use rusqlite::{Connection, Row};
use serde_json::{Value, json};
use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
};
use walkdir::WalkDir;
use zip::ZipArchive;

pub use self::{
    batch_review::{load_import_batches, preview_import_batch},
    batches::{
        restore_import_batch, restore_import_batch_with_control, revert_import_batch,
        revert_import_batch_with_control,
    },
    browser_history::{
        import_browser_history, import_browser_history_with_control,
        import_browser_history_with_progress, inspect_browser_history,
    },
    import_flow::{import_takeout, import_takeout_with_control, import_takeout_with_progress},
    inspect::inspect_takeout,
};

pub(crate) use self::batches::ensure_import_batch_audit_artifact;

#[cfg(test)]
use self::{
    batch_review::load_import_batch_record,
    inspect::{
        collect_records_from_payload, gather_takeout_files, parse_payload_report,
        preview_entry_from_payload, quarantine_file, quarantine_takeout_file, read_zip_entry,
        recognize_takeout_file,
    },
};

/// Caps the number of preview entries returned for inspection and batch review.
const PREVIEW_LIMIT: usize = 24;

/// Describes one discovered Takeout payload file before parsing/import.
#[derive(Debug, Clone)]
struct TakeoutFile {
    path: String,
    from_zip: bool,
}

/// Captures one previewable history row extracted from a Takeout payload.
#[derive(Debug, Clone)]
struct ParsedTakeoutRecord {
    source_path: String,
    url: String,
    title: Option<String>,
    visited_at: String,
    source_visit_id: i64,
}

/// Aggregates per-import counters before they are folded into run/batch summary JSON.
#[derive(Debug, Default)]
struct ImportStats {
    imported_items: usize,
    duplicate_items: usize,
    skipped_items: usize,
}

/// Carries one file import result plus the source-evidence work it unlocked.
#[derive(Debug)]
struct ImportedPayload {
    stats: ImportStats,
    record_count: usize,
    recognized_file: TakeoutFileReport,
    earliest_visit_iso: Option<String>,
    latest_visit_iso: Option<String>,
}

/// Holds preview rows plus the minimal parser metadata needed by inspection tests.
#[cfg(test)]
#[derive(Debug)]
struct CollectedPayload {
    records: Vec<ParsedTakeoutRecord>,
    skipped_missing_visit_time: usize,
}

/// Represents one persisted import batch plus its review metadata.
#[derive(Debug, Clone)]
struct ImportBatchRecord {
    overview: ImportBatchOverview,
    recognized_files: Vec<TakeoutFileReport>,
    quarantined_files: Vec<TakeoutFileReport>,
    notes: Vec<String>,
    note_details: Vec<TakeoutNote>,
    detected_locale: Option<String>,
    preview_range_start: Option<String>,
    preview_range_end: Option<String>,
}

/// Describes the summary JSON mutation applied to one import batch row.
#[derive(Debug)]
struct BatchSummaryUpdate<'a> {
    batch_id: i64,
    status: &'a str,
    imported_items: usize,
    duplicate_items: usize,
    candidate_items: usize,
    recognized_files: &'a [TakeoutFileReport],
    quarantined_files: &'a [TakeoutFileReport],
    notes: &'a [String],
    note_details: &'a [TakeoutNote],
    detected_locale: Option<&'a str>,
    preview_range_start: Option<&'a str>,
    preview_range_end: Option<&'a str>,
    reverted_at: Option<String>,
}

/// Provides the SQL row fields needed to rebuild an `ImportBatchOverview`.
#[derive(Debug)]
struct ImportBatchOverviewRow {
    id: i64,
    source_kind: String,
    source_path: String,
    profile_id: String,
    created_at: String,
    imported_at: Option<String>,
    reverted_at: Option<String>,
    status: String,
    summary_json: String,
    visible_items: usize,
    audit_path: Option<String>,
    git_commit: Option<String>,
}

/// Tracks the earliest/latest previewable visit timestamps seen across Takeout files.
#[derive(Debug, Clone, Default)]
struct PreviewRangeSummary {
    start: Option<String>,
    end: Option<String>,
}

/// Combines a parser-side path classification with the file being reported to the UI.
#[derive(Debug, Clone, Copy)]
struct ClassifiedTakeoutFile<'a> {
    file: &'a TakeoutFile,
    path_match: TakeoutPathMatch,
}

fn merge_detected_locale(detected_locale: &mut Option<String>, next_locale: Option<&str>) {
    let Some(next_locale) = next_locale else {
        return;
    };
    match detected_locale.as_deref() {
        None => *detected_locale = Some(next_locale.to_string()),
        Some(current) if current == next_locale || current == "mixed" => {}
        Some(_) => *detected_locale = Some("mixed".to_string()),
    }
}

fn merge_preview_range(
    preview_range: &mut PreviewRangeSummary,
    next_start: Option<&str>,
    next_end: Option<&str>,
) {
    if let Some(next_start) = next_start {
        let should_replace =
            preview_range.start.as_deref().is_none_or(|current| next_start < current);
        if should_replace {
            preview_range.start = Some(next_start.to_string());
        }
    }
    if let Some(next_end) = next_end {
        let should_replace = preview_range.end.as_deref().is_none_or(|current| next_end > current);
        if should_replace {
            preview_range.end = Some(next_end.to_string());
        }
    }
}

fn file_report_kind(path_match: TakeoutPathMatch) -> String {
    path_match.recognized_kind.unwrap_or(path_match.family).to_string()
}

fn file_report_from_match(
    classified_file: ClassifiedTakeoutFile<'_>,
    status: &str,
    records: usize,
    reason_detail: Option<String>,
) -> TakeoutFileReport {
    TakeoutFileReport {
        path: classified_file.file.path.clone(),
        kind: file_report_kind(classified_file.path_match),
        status: status.to_string(),
        records,
        classification: match classified_file.path_match.disposition {
            TakeoutPathDisposition::WillImport => "will-import",
            TakeoutPathDisposition::KnownIgnored => "known-but-ignored",
            TakeoutPathDisposition::NeedsReview => "needs-review",
        }
        .to_string(),
        reason_code: Some(classified_file.path_match.reason_code.to_string()),
        reason_detail,
        detected_locale: classified_file.path_match.locale.map(ToString::to_string),
    }
}
