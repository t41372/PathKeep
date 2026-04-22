//! Takeout inspection and quarantine helpers.
//!
//! ## Responsibilities
//! - Inspect candidate Takeout sources without mutating the archive.
//! - Parse recognized payload files into preview rows.
//! - Quarantine unsupported files so the user can review what was skipped.
//!
//! ## Not responsible for
//! - Writing canonical archive rows.
//! - Updating import batches or audit artifacts.
//! - Managing post-import source-evidence persistence.
//!
//! ## Dependencies
//! - Filesystem helpers from `std::fs`.
//! - Takeout payload recognition/parsing from `browser_history_parser::takeout`.
//!
//! ## Performance notes
//! - Inspection intentionally stops at preview data and avoids any archive
//!   writes, but zipped sources are still read entry-by-entry into memory.

use super::*;
use browser_history_parser::{
    HistoryBatchConsumer, ParsedUrl, ParsedVisit, StreamHistoryError,
    takeout::{TakeoutStreamOptions, stream_payload_with_options},
};

/// Inspects a Takeout source and builds a preview-only import report.
pub fn inspect_takeout(
    _paths: &ProjectPaths,
    request: &TakeoutRequest,
) -> Result<TakeoutInspection> {
    let files = gather_takeout_files(Path::new(&request.source_path))?;
    let mut inspection = TakeoutInspection {
        source_path: request.source_path.clone(),
        dry_run: request.dry_run,
        ..TakeoutInspection::default()
    };
    let mut found_importable_payload = false;

    let source_root = Path::new(&request.source_path);
    for file in files {
        let classified_file = classify_takeout_file(source_root, &file)?;
        merge_detected_locale(&mut inspection.detected_locale, classified_file.path_match.locale);

        if classified_file.path_match.disposition == TakeoutPathDisposition::NeedsReview {
            inspection.quarantined_files.push(file_report_from_match(
                classified_file,
                "needs-review",
                0,
                None,
            ));
            continue;
        }

        let Some(kind) = classified_file.path_match.recognized_kind else {
            inspection.recognized_files.push(file_report_from_match(
                classified_file,
                "ignored",
                0,
                None,
            ));
            continue;
        };

        if kind == KIND_INDEX {
            inspection.recognized_files.push(file_report_from_match(
                classified_file,
                "ignored",
                0,
                None,
            ));
            continue;
        }

        if classified_file.path_match.disposition == TakeoutPathDisposition::WillImport {
            found_importable_payload = true;
        }

        let bytes = if file.from_zip {
            read_zip_entry(Path::new(&request.source_path), &file.path)?
        } else {
            fs::read(&file.path)?
        };

        match preview_payload_stream(
            &file.path,
            kind,
            &bytes,
            PREVIEW_LIMIT.saturating_sub(inspection.preview_entries.len()),
        ) {
            Ok(payload) => {
                let status = if payload.skipped_missing_visit_time > 0 {
                    "previewed-with-skips"
                } else {
                    "previewed"
                };
                let report =
                    file_report_from_match(classified_file, status, payload.record_count, None);
                inspection.candidate_items += payload.candidate_items;
                let mut preview_range = PreviewRangeSummary {
                    start: inspection.preview_range_start.take(),
                    end: inspection.preview_range_end.take(),
                };
                merge_preview_range(
                    &mut preview_range,
                    payload.earliest_visit_iso.as_deref(),
                    payload.latest_visit_iso.as_deref(),
                );
                inspection.preview_range_start = preview_range.start;
                inspection.preview_range_end = preview_range.end;
                if payload.skipped_missing_visit_time > 0 {
                    inspection.notes.push(format!(
                        "Skipped {} records from {} because they were missing a visit timestamp.",
                        payload.skipped_missing_visit_time, file.path
                    ));
                }
                for record in payload.records {
                    inspection.preview_entries.push(preview_entry(&record, "candidate"));
                }
                inspection.recognized_files.push(report);
            }
            Err(error) => {
                let mut report = file_report_from_match(
                    classified_file,
                    "parse-error",
                    0,
                    Some(error.to_string()),
                );
                report.classification = "parse-error".to_string();
                report.reason_code = Some("parse-error".to_string());
                inspection.notes.push(format!("Could not parse {}: {}", file.path, error));
                inspection.recognized_files.push(report);
            }
        }
    }

    if !found_importable_payload {
        inspection.notes.push(
            "No directly importable history files were detected. Dry-run still captured the archive structure."
                .to_string(),
        );
    }

    Ok(inspection)
}

/// Builds one preview entry from parser output during test-only fixture coverage.
#[cfg(test)]
pub(super) fn preview_entry_from_payload(
    source_path: &str,
    kind: &str,
    payload_json: &[u8],
) -> Result<TakeoutPreviewEntry> {
    let payload = collect_records_from_payload(source_path, kind, payload_json)?;
    let Some(record) = payload.records.first() else {
        anyhow::bail!("payload did not include a usable history record")
    };
    Ok(preview_entry(record, "imported"))
}

/// Converts one parsed record into the shared preview/read-model shape.
fn preview_entry(record: &ParsedTakeoutRecord, status: &str) -> TakeoutPreviewEntry {
    TakeoutPreviewEntry {
        source_path: record.source_path.clone(),
        url: record.url.clone(),
        title: record.title.clone(),
        visited_at: record.visited_at.clone(),
        source_visit_id: record.source_visit_id,
        status: status.to_string(),
    }
}

/// Parses one recognized Takeout payload into preview rows plus parser provenance.
#[cfg(test)]
pub(super) fn collect_records_from_payload(
    source_path: &str,
    kind: &str,
    bytes: &[u8],
) -> Result<CollectedPayload> {
    let report = parse_payload_report(source_path, kind, bytes)?;
    let records = preview_records_from_report(source_path, &report);
    Ok(CollectedPayload { skipped_missing_visit_time: report.skipped_missing_visit_time, records })
}

/// Streams one payload into capped preview rows without accumulating source evidence.
fn preview_payload_stream(
    source_path: &str,
    kind: &str,
    bytes: &[u8],
    preview_limit: usize,
) -> Result<PreviewPayload> {
    let mut consumer = TakeoutPreviewCollector {
        source_path: source_path.to_string(),
        preview_limit,
        records: Vec::new(),
        candidate_items: 0,
        earliest_visit_iso: None,
        latest_visit_iso: None,
    };
    let report = stream_payload_with_options(
        source_path,
        kind,
        bytes,
        PREVIEW_LIMIT,
        TakeoutStreamOptions {
            collect_source_evidence: false,
            retain_report_source_evidence: false,
        },
        &mut consumer,
    )
    .map_err(|error| match error {
        StreamHistoryError::Parse(error) => anyhow::Error::new(error),
        StreamHistoryError::Consumer(error) => match error {},
    })
    .with_context(|| format!("parsing Takeout payload {source_path} ({kind})"))?;
    Ok(PreviewPayload {
        records: consumer.records,
        candidate_items: consumer.candidate_items,
        record_count: report.record_count,
        skipped_missing_visit_time: report.skipped_missing_visit_time,
        earliest_visit_iso: report.earliest_visit_iso,
        latest_visit_iso: report.latest_visit_iso,
    })
}

/// Parses one recognized Takeout payload without allocating inspection preview rows.
#[cfg(test)]
pub(super) fn parse_payload_report(
    source_path: &str,
    kind: &str,
    bytes: &[u8],
) -> Result<TakeoutPayloadReport> {
    parse_takeout_payload(source_path, kind, bytes)
        .with_context(|| format!("parsing Takeout payload {source_path} ({kind})"))
}

/// Builds the lightweight preview rows used by inspect/review surfaces from a parser report.
#[cfg(test)]
fn preview_records_from_report(
    source_path: &str,
    report: &TakeoutPayloadReport,
) -> Vec<ParsedTakeoutRecord> {
    report
        .history
        .visits
        .iter()
        .map(|visit| ParsedTakeoutRecord {
            source_path: source_path.to_string(),
            url: visit.url.clone(),
            title: visit.title.clone(),
            visited_at: visit.visit_time_iso.clone(),
            source_visit_id: visit.source_visit_id,
        })
        .collect()
}

#[derive(Debug)]
struct PreviewPayload {
    records: Vec<ParsedTakeoutRecord>,
    candidate_items: usize,
    record_count: usize,
    skipped_missing_visit_time: usize,
    earliest_visit_iso: Option<String>,
    latest_visit_iso: Option<String>,
}

#[derive(Debug)]
struct TakeoutPreviewCollector {
    source_path: String,
    preview_limit: usize,
    records: Vec<ParsedTakeoutRecord>,
    candidate_items: usize,
    earliest_visit_iso: Option<String>,
    latest_visit_iso: Option<String>,
}

impl HistoryBatchConsumer for TakeoutPreviewCollector {
    type Error = std::convert::Infallible;

    fn urls(&mut self, _batch: Vec<ParsedUrl>) -> Result<(), Self::Error> {
        Ok(())
    }

    fn visits(&mut self, batch: Vec<ParsedVisit>) -> Result<(), Self::Error> {
        self.candidate_items += batch.len();
        for visit in batch {
            let should_update_start = self
                .earliest_visit_iso
                .as_deref()
                .is_none_or(|current| visit.visit_time_iso.as_str() < current);
            if should_update_start {
                self.earliest_visit_iso = Some(visit.visit_time_iso.clone());
            }
            let should_update_end = self
                .latest_visit_iso
                .as_deref()
                .is_none_or(|current| visit.visit_time_iso.as_str() > current);
            if should_update_end {
                self.latest_visit_iso = Some(visit.visit_time_iso.clone());
            }
            if self.records.len() < self.preview_limit {
                self.records.push(ParsedTakeoutRecord {
                    source_path: self.source_path.clone(),
                    url: visit.url,
                    title: visit.title,
                    visited_at: visit.visit_time_iso,
                    source_visit_id: visit.source_visit_id,
                });
            }
        }
        Ok(())
    }
}

/// Enumerates all file candidates contained in a Takeout directory or zip source.
pub(super) fn gather_takeout_files(source: &Path) -> Result<Vec<TakeoutFile>> {
    if source.is_dir() {
        return Ok(WalkDir::new(source)
            .into_iter()
            .filter_map(Result::ok)
            .filter(|entry| entry.file_type().is_file())
            .filter(|entry| !should_skip_takeout_file(entry.path().to_string_lossy().as_ref()))
            .map(|entry| TakeoutFile { path: entry.path().display().to_string(), from_zip: false })
            .collect());
    }

    if source.is_file() && !is_zip_source(source) {
        if should_skip_takeout_file(source.to_string_lossy().as_ref()) {
            return Ok(Vec::new());
        }
        return Ok(vec![TakeoutFile { path: source.display().to_string(), from_zip: false }]);
    }

    let file = fs::File::open(source)?;
    let mut archive = ZipArchive::new(file)?;
    let mut files = Vec::new();
    for index in 0..archive.len() {
        let entry = archive.by_index(index)?;
        if entry.is_file() && !should_skip_takeout_file(entry.name()) {
            files.push(TakeoutFile { path: entry.name().to_string(), from_zip: true });
        }
    }
    Ok(files)
}

/// Classifies a file path into the supported Takeout payload family, if any.
#[cfg(test)]
pub(super) fn recognize_takeout_file(path: &str) -> Option<String> {
    browser_history_parser::takeout::recognize_payload(path).map(ToString::to_string)
}

pub(super) fn classify_takeout_file<'a>(
    source_root: &Path,
    file: &'a TakeoutFile,
) -> Result<ClassifiedTakeoutFile<'a>> {
    Ok(ClassifiedTakeoutFile {
        file,
        path_match: classify_takeout_payload_with_sniff(source_root, &file.path, file.from_zip)?,
    })
}

/// Reads one zip entry into memory so inspection/import can parse it.
pub(super) fn read_zip_entry(source_zip: &Path, entry_name: &str) -> Result<Vec<u8>> {
    let file = fs::File::open(source_zip)?;
    let mut archive = ZipArchive::new(file)?;
    let mut entry = archive.by_name(entry_name)?;
    let mut bytes = Vec::new();
    entry.read_to_end(&mut bytes)?;
    Ok(bytes)
}

/// Copies one unsupported Takeout file into the quarantine directory.
pub(super) fn quarantine_takeout_file(
    paths: &ProjectPaths,
    source_root: &Path,
    file: &TakeoutFile,
) -> Result<()> {
    if file.from_zip {
        let bytes = read_zip_entry(source_root, &file.path)?;
        quarantine_bytes(paths, source_root, &file.path, &bytes)?;
    } else {
        quarantine_file(paths, source_root, &file.path)?;
    }
    Ok(())
}

/// Copies one unsupported on-disk file into quarantine if the source still exists.
pub(super) fn quarantine_file(paths: &ProjectPaths, source_root: &Path, path: &str) -> Result<()> {
    let destination = quarantine_destination(paths, source_root, path);
    ensure_parent_dir(&destination)?;
    copy_if_exists(path, &destination)?;
    Ok(())
}

/// Persists an unsupported zip entry into the quarantine directory.
fn quarantine_bytes(
    paths: &ProjectPaths,
    source_root: &Path,
    path: &str,
    bytes: &[u8],
) -> Result<()> {
    let destination = quarantine_destination(paths, source_root, path);
    ensure_parent_dir(&destination)?;
    fs::write(&destination, bytes).with_context(|| format!("writing {}", destination.display()))?;
    Ok(())
}

/// Computes the destination path for one quarantined payload.
fn quarantine_destination(paths: &ProjectPaths, source_root: &Path, path: &str) -> PathBuf {
    paths
        .quarantine_dir
        .join(quarantine_source_name(source_root))
        .join(quarantine_relative_path(source_root, path))
}

/// Normalizes the root folder name used under the quarantine directory.
fn quarantine_source_name(source_root: &Path) -> &str {
    source_root.file_stem().and_then(|name| name.to_str()).unwrap_or("takeout")
}

/// Produces a safe relative path for a quarantined payload.
fn quarantine_relative_path(source_root: &Path, path: &str) -> PathBuf {
    if source_root.is_file() && !is_zip_source(source_root) {
        return Path::new(path)
            .file_name()
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("unknown"));
    }

    let candidate = Path::new(path)
        .strip_prefix(source_root)
        .map(Path::to_path_buf)
        .unwrap_or_else(|_| PathBuf::from(path));
    let sanitized = sanitized_relative_path(&candidate);
    if sanitized.as_os_str().is_empty() { PathBuf::from("unknown") } else { sanitized }
}

/// Removes non-normal path components so quarantine writes cannot escape their root.
fn sanitized_relative_path(path: &Path) -> PathBuf {
    let mut sanitized = PathBuf::new();
    for component in path.components() {
        if let std::path::Component::Normal(part) = component {
            sanitized.push(part);
        }
    }
    sanitized
}

/// Ensures the parent directory exists before a quarantine write.
fn ensure_parent_dir(path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    Ok(())
}

/// Copies a file into quarantine when the source still exists on disk.
fn copy_if_exists(source: &str, destination: &Path) -> Result<()> {
    if Path::new(source).exists() {
        fs::copy(source, destination)?;
    }
    Ok(())
}

fn should_skip_takeout_file(path: &str) -> bool {
    let normalized = path.replace('\\', "/").to_lowercase();
    let file_name = normalized.rsplit('/').next().unwrap_or(normalized.as_str());
    file_name.starts_with('.') || normalized.split('/').any(|segment| segment == "__macosx")
}

fn is_zip_source(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("zip"))
}
