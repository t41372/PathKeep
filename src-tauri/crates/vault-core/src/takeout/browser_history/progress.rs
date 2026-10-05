//! Responsibilities: translate committed import counters into progress and warnings.
//! Not responsible for: SQL or source parsing.
//! Failure modes: repeated events, incorrect counter totals, missing warning codes.
//! Tests: browser_history::consumer_tests.
use super::*;
pub(super) fn should_emit_browser_import_progress(
    last_processed_records: &mut usize,
    current: usize,
) -> bool {
    if current == *last_processed_records {
        return false;
    }
    *last_processed_records = current;
    true
}

pub(super) fn emit_browser_import_progress_if_changed(
    report_progress: &mut impl FnMut(ImportProgressEvent),
    last_processed_records: &mut usize,
    source_label: &str,
    browser_name: &str,
    profile_name: &str,
    progress_log_lines: &[String],
    progress: BrowserImportProgress,
) {
    if !should_emit_browser_import_progress(last_processed_records, progress.processed_records) {
        return;
    }
    emit_browser_import_progress_with_records(
        report_progress,
        BrowserImportProgressEventInput {
            phase: "import-file",
            detail: format!("Processing {source_label}"),
            current: 1,
            total: 1,
            progress_percent: None,
            source_path: Some(source_label.to_string()),
            log_lines: progress_log_lines,
            record_state: BrowserImportProgressState {
                source_label: Some(format!("{browser_name} / {profile_name}")),
                processed_records: Some(progress.processed_records),
                total_records: None,
                imported_records: Some(progress.imported_records),
                duplicate_records: Some(progress.duplicate_records),
                skipped_records: Some(progress.skipped_records),
            },
        },
    );
}

/// Converts parser warnings into coded notes the review UI can localize.
///
/// The parser already emits a stable `code`, so it is namespaced rather than
/// re-derived; the raw parser sentence stays as fallback/diagnostic evidence
/// because it names concrete tables and files.
pub(super) fn parser_warning_notes(
    warnings: &[browser_history_parser::ParserWarning],
) -> Vec<TakeoutNote> {
    warnings
        .iter()
        .map(|warning| {
            TakeoutNote::new(&format!("parser-{}", warning.code), warning.message.clone())
                .with_diagnostic(warning.message.clone())
        })
        .collect()
}

pub(super) fn append_browser_import_skipped_note(
    inspection: &mut TakeoutInspection,
    skipped_items: usize,
) {
    if skipped_items > 0 {
        inspection.push_note(
            TakeoutNote::new(
                "browser-skipped-missing-url-row",
                format!(
                    "Skipped {} visit row(s) because their URL row was not present in the source.",
                    skipped_items
                ),
            )
            .with_count(skipped_items),
        );
    }
}

#[derive(Default)]
pub(super) struct BrowserImportProgressState {
    pub(super) source_label: Option<String>,
    pub(super) processed_records: Option<usize>,
    pub(super) total_records: Option<usize>,
    pub(super) imported_records: Option<usize>,
    pub(super) duplicate_records: Option<usize>,
    pub(super) skipped_records: Option<usize>,
}

pub(super) struct BrowserImportProgressEventInput<'a> {
    pub(super) phase: &'a str,
    pub(super) detail: String,
    pub(super) current: usize,
    pub(super) total: usize,
    pub(super) progress_percent: Option<f32>,
    pub(super) source_path: Option<String>,
    pub(super) log_lines: &'a [String],
    pub(super) record_state: BrowserImportProgressState,
}

pub(super) fn emit_browser_import_progress(
    report_progress: &mut impl FnMut(ImportProgressEvent),
    phase: &str,
    detail: String,
    current: usize,
    total: usize,
    progress_percent: Option<f32>,
    source_path: Option<String>,
    log_lines: &[String],
) {
    emit_browser_import_progress_with_records(
        report_progress,
        BrowserImportProgressEventInput {
            phase,
            detail,
            current,
            total,
            progress_percent,
            source_path,
            log_lines,
            record_state: BrowserImportProgressState::default(),
        },
    );
}

pub(super) fn emit_browser_import_progress_with_records(
    report_progress: &mut impl FnMut(ImportProgressEvent),
    input: BrowserImportProgressEventInput<'_>,
) {
    let BrowserImportProgressEventInput {
        phase,
        detail,
        current,
        total,
        progress_percent,
        source_path,
        log_lines,
        record_state,
    } = input;
    let label = match phase {
        "prepare" => "Preparing import",
        "import-file" => "Importing browser history",
        "finalize" => "Finalizing import",
        "complete" => "Import complete",
        _ => "Importing browser history",
    };
    report_progress(
        ImportProgressEvent {
            phase: phase.to_string(),
            label: label.to_string(),
            detail,
            current,
            total,
            progress_percent,
            log_lines: log_lines
                .iter()
                .rev()
                .take(4)
                .cloned()
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .collect(),
            source_path,
            source_label: record_state.source_label,
            processed_records: record_state.processed_records,
            total_records: record_state.total_records,
            imported_records: record_state.imported_records,
            duplicate_records: record_state.duplicate_records,
            skipped_records: record_state.skipped_records,
            log_events: Vec::new(),
        }
        .with_log_event(progress_log_level_for_browser_phase(phase), &format!("import.{phase}")),
    );
}

pub(super) fn progress_log_level_for_browser_phase(phase: &str) -> &'static str {
    match phase {
        "complete" => "success",
        _ => "info",
    }
}
