//! Responsibilities: account for raw captures and redact reference-only capture failures.
//! Not responsible for: reference authority or publishing parsed conversations.
use super::{BackupOptions, BackupReport, check_cancel, issue, progress, writer};
use crate::{Archive, Artifact, Error, Result, SourceRoot, adapters::common};

pub(super) fn capture_raw(
    archive: &Archive,
    source_id: &str,
    source: &SourceRoot,
    artifact: &Artifact,
    options: &BackupOptions,
    report: &mut BackupReport,
    reference: Option<&mut Option<String>>,
) -> Result<Option<writer::Captured>> {
    archive.register_source(source_id, source, artifact)?;
    archive.observe_source(report.run_id, source_id, true)?;
    match writer::capture(archive, source_id, source, artifact, options) {
        Ok(captured) => {
            if captured.unchanged {
                report.unchanged = report.unchanged.saturating_add(1);
            } else {
                report.captured = report.captured.saturating_add(1);
                report.bytes = report.bytes.saturating_add(captured.byte_len);
                progress(options, "raw_committed", source_id, &captured);
            }
            for diagnostic in &captured.diagnostics {
                common::push_diagnostic(&mut report.issues, diagnostic.clone());
            }
            if reference.is_some() {
                archive.reference_coverage_scope(source_id)?;
            }
            Ok(Some(captured))
        }
        Err(writer::CaptureFailure::Destination(error)) => Err(error),
        Err(writer::CaptureFailure::Source(error)) => {
            check_cancel(options)?;
            if matches!(error, Error::Cancelled) {
                return Err(error);
            }
            archive.set_coverage(source_id, false)?;
            if reference.is_some() {
                archive.reference_coverage_scope(source_id)?;
            }
            let code = if matches!(error, Error::SourceChanged(_)) {
                "source_changed_during_capture"
            } else {
                "source_capture_unavailable"
            };
            if let Some(reason) = reference {
                *reason = Some(
                    match &error {
                        Error::Io(error) if error.kind() == std::io::ErrorKind::NotFound => {
                            "missing"
                        }
                        Error::InvalidInput(_) | Error::Unsupported(_) => "unsafe_path",
                        _ => "authority_unavailable",
                    }
                    .into(),
                );
            } else {
                issue(&mut report.issues, code, Some(&artifact.relative_path));
            }
            Ok(None)
        }
    }
}
