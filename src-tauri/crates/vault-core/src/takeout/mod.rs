//! Google Takeout import boundary.
//!
//! ## Responsibilities
//! - Own the archive-facing Takeout flow: inspect, quarantine, import, batch
//!   review, and reversible visibility changes.
//! - Keep the import transport honest by preserving preview/manual/execute
//!   semantics and import-batch audit history.
//! - Preserve source-evidence provenance for recognized Takeout payloads.
//!
//! ## Not responsible for
//! - Defining the parser contract for non-Takeout browser sources.
//! - Owning canonical archive schema bootstrap outside the import-specific
//!   tables it depends on.
//! - Rewriting frontend or Tauri command contracts.
//!
//! ## Dependencies
//! - Canonical/archive helpers from `crate::archive`.
//! - Audit artifact persistence from `crate::git_audit`.
//! - Source payload parsing from `browser_history_parser::takeout`.
//!
//! ## Performance notes
//! - This boundary still runs on potentially large import sources, so import
//!   execution and review helpers must avoid hidden extra passes and keep
//!   per-file work explicit.

mod batch_review;
mod batches;
mod import_flow;
mod inspect;
mod payload_import;

#[cfg(test)]
mod tests;

use crate::{
    archive::{
        DeferredSourceEvidencePayload, SourceBatchInput, SourceEvidencePayload,
        coverage_stats_json_from_parts, create_schema, defer_source_evidence_payload,
        open_archive_connection, open_source_evidence_connection, rebuild_search_projection,
        record_schema_observation, stats_with_archive_totals, upsert_source_batch,
        visit_event_fingerprint,
    },
    config::{ProjectPaths, ensure_paths},
    git_audit,
    models::{
        AppConfig, ImportBatchDetail, ImportBatchOverview, ImportProgressEvent, TakeoutFileReport,
        TakeoutInspection, TakeoutPreviewEntry, TakeoutRequest,
    },
    utils::{now_rfc3339, sha256_hex},
};
use anyhow::{Context, Result};
use browser_history_parser::takeout::{
    KIND_INDEX, TakeoutPayloadReport, TakeoutPayloadStreamReport,
    parse_payload as parse_takeout_payload, recognize_payload as recognize_takeout_payload,
    stream_payload as stream_takeout_payload,
};
use rusqlite::{Connection, OptionalExtension, Row, Transaction, params};
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
    batches::{restore_import_batch, revert_import_batch},
    import_flow::{import_takeout, import_takeout_with_progress},
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
    source_evidence_plan: TakeoutSourceEvidencePlan,
}

/// Holds parsed payload rows together with the parser-side report for one file.
#[derive(Debug)]
struct CollectedPayload {
    report: TakeoutPayloadReport,
    records: Vec<ParsedTakeoutRecord>,
    skipped_missing_visit_time: usize,
}

/// Stages the source-evidence writes required after canonical import commits.
#[derive(Debug)]
struct TakeoutSourceEvidencePlan {
    source_batch: SourceBatchInput,
    schema_observation: browser_history_parser::SchemaObservation,
    source_evidence_payload: DeferredSourceEvidencePayload,
}

/// Represents one persisted import batch plus its review metadata.
#[derive(Debug, Clone)]
struct ImportBatchRecord {
    overview: ImportBatchOverview,
    recognized_files: Vec<TakeoutFileReport>,
    quarantined_files: Vec<TakeoutFileReport>,
    notes: Vec<String>,
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
