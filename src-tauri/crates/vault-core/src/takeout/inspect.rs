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

    for file in files {
        let Some(kind) = recognize_takeout_file(&file.path) else {
            inspection.quarantined_files.push(TakeoutFileReport {
                path: file.path,
                kind: "unknown".to_string(),
                status: "quarantine".to_string(),
                records: 0,
            });
            continue;
        };

        let mut report = TakeoutFileReport {
            path: file.path.clone(),
            kind: kind.to_string(),
            status: "recognized".to_string(),
            records: 0,
        };

        if kind == KIND_INDEX {
            inspection.recognized_files.push(report);
            continue;
        }

        let bytes = if file.from_zip {
            read_zip_entry(Path::new(&request.source_path), &file.path)?
        } else {
            fs::read(&file.path)?
        };

        match collect_records_from_payload(&file.path, &kind, &bytes) {
            Ok(payload) => {
                report.records = payload.report.record_count;
                report.status = if payload.skipped_missing_visit_time > 0 {
                    "previewed-with-skips".to_string()
                } else {
                    "previewed".to_string()
                };
                inspection.candidate_items += payload.records.len();
                if payload.skipped_missing_visit_time > 0 {
                    inspection.notes.push(format!(
                        "Skipped {} records from {} because they were missing a visit timestamp.",
                        payload.skipped_missing_visit_time, file.path
                    ));
                }
                for record in payload.records {
                    if inspection.preview_entries.len() < PREVIEW_LIMIT {
                        inspection.preview_entries.push(preview_entry(&record, "candidate"));
                    }
                }
            }
            Err(error) => {
                report.status = "parse-error".to_string();
                inspection.notes.push(format!("Could not parse {}: {}", file.path, error));
            }
        }

        inspection.recognized_files.push(report);
    }

    if inspection.recognized_files.is_empty() {
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
pub(super) fn collect_records_from_payload(
    source_path: &str,
    kind: &str,
    bytes: &[u8],
) -> Result<CollectedPayload> {
    let report = parse_payload_report(source_path, kind, bytes)?;
    let records = preview_records_from_report(source_path, &report);
    Ok(CollectedPayload {
        skipped_missing_visit_time: report.skipped_missing_visit_time,
        records,
        report,
    })
}

/// Parses one recognized Takeout payload without allocating inspection preview rows.
pub(super) fn parse_payload_report(
    source_path: &str,
    kind: &str,
    bytes: &[u8],
) -> Result<TakeoutPayloadReport> {
    parse_takeout_payload(source_path, kind, bytes)
        .with_context(|| format!("parsing Takeout payload {source_path} ({kind})"))
}

/// Builds the lightweight preview rows used by inspect/review surfaces from a parser report.
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

/// Enumerates all file candidates contained in a Takeout directory or zip source.
pub(super) fn gather_takeout_files(source: &Path) -> Result<Vec<TakeoutFile>> {
    if source.is_dir() {
        return Ok(WalkDir::new(source)
            .into_iter()
            .filter_map(Result::ok)
            .filter(|entry| entry.file_type().is_file())
            .map(|entry| TakeoutFile { path: entry.path().display().to_string(), from_zip: false })
            .collect());
    }

    let file = fs::File::open(source)?;
    let mut archive = ZipArchive::new(file)?;
    let mut files = Vec::new();
    for index in 0..archive.len() {
        let entry = archive.by_index(index)?;
        if entry.is_file() {
            files.push(TakeoutFile { path: entry.name().to_string(), from_zip: true });
        }
    }
    Ok(files)
}

/// Classifies a file path into the supported Takeout payload family, if any.
pub(super) fn recognize_takeout_file(path: &str) -> Option<String> {
    recognize_takeout_payload(path).map(ToString::to_string)
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
