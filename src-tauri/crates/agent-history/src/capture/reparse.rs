//! Responsibilities: rebuild one interpretation exclusively from retained archive evidence.
//! Not responsible for: source discovery, filesystem reference resolution or native restore.
//! Reparse creates a new projection generation; it never changes a raw revision.

use super::{BackupOptions, BackupProgress, BackupReport, check_cancel, projection};
use crate::{
    Archive, Artifact, ArtifactKind, Diagnostic, Error, Harness, Reference, Result, SourceRoot,
    adapters::{adapter, common},
    archive::RevisionInfo,
};
use std::{io::Read, path::PathBuf};

impl Archive {
    /// Reinterprets the most recently retained source bytes after a parser or
    /// resource-budget change, even when the native source no longer exists.
    /// Only evidence already bound during capture can satisfy an attachment.
    pub fn reparse_source(&self, source_id: &str, options: &BackupOptions) -> Result<BackupReport> {
        check_cancel(options)?;
        if options.max_record_bytes == 0
            || options.max_record_bytes > 64 * 1024 * 1024
            || options.max_decoded_bytes == 0
        {
            return Err(Error::InvalidInput(
                "capture byte budgets are outside supported bounds".into(),
            ));
        }
        let _job = self.jobs.enter(Some(&options.cancel))?;
        let provenance = self.source(source_id)?;
        if provenance.kind != "transcript" {
            return Err(Error::Unsupported(
                "only native transcript evidence can be reparsed".into(),
            ));
        }
        let raw = self
            .latest_revision(source_id)?
            .ok_or_else(|| Error::NotFound("retained source revision".into()))?;
        if raw.capture_format != "native" {
            return Err(Error::Unsupported(
                "logical database snapshots are not transcript streams".into(),
            ));
        }
        let source = SourceRoot {
            harness: provenance.harness.parse()?,
            root: PathBuf::from(provenance.root),
        };
        // These paths are provenance only. Neither this operation nor the parser
        // checks their existence, canonicalizes them, or opens their contents.
        let artifact = Artifact {
            path: source.root.join(&provenance.relative_path),
            relative_path: provenance.relative_path,
            kind: ArtifactKind::Transcript,
            project_hint: None,
            generation: None,
            current: true,
        };
        let run_id = self.start_backup_run()?;
        let mut report = BackupReport {
            status: "complete".into(),
            captured: 0,
            unchanged: 1,
            bytes: 0,
            run_id,
            issues: Vec::new(),
            warning_sources: Default::default(),
        };
        let outcome = reparse(self, &source, &artifact, &raw, options, &mut report);
        finish(self, source_id, report, outcome)
    }
}

fn reparse(
    archive: &Archive,
    source: &SourceRoot,
    artifact: &Artifact,
    raw: &RevisionInfo,
    options: &BackupOptions,
    report: &mut BackupReport,
) -> Result<()> {
    archive.set_coverage(&raw.source_id, false)?;
    let prepared = match projection::prepare(
        archive,
        source,
        artifact,
        &raw.source_id,
        raw.id,
        options,
        &mut report.issues,
        true,
    )? {
        projection::Preparation::Ready(prepared) => prepared,
        projection::Preparation::Writing => return Ok(()),
        projection::Preparation::Unreadable => {
            super::unreadable::publish(archive, source, artifact, &raw.source_id, raw.id, options)?;
            archive.synchronize_search()?;
            return Ok(());
        }
    };
    for diagnostic in &prepared.report.diagnostics {
        if diagnostic.detail.as_deref() != Some("remote") {
            common::push_diagnostic(&mut report.issues, diagnostic.clone());
        }
    }
    let complete = prepared.report.complete;
    for reference in &prepared.report.references {
        check_cancel(options)?;
        retained_reference(
            archive,
            source.harness,
            raw.id,
            reference,
            options,
            &mut report.issues,
        )?;
    }
    if let Some(metadata) = &prepared.report.session {
        let attempt = prepared.attempt.as_ref().ok_or_else(|| {
            Error::Integrity("forced reparse did not reserve a generation".into())
        })?;
        progress(options, "projection_staged", raw);
        check_cancel(options)?;
        archive.publish_projection_attempt_with_coverage(
            attempt,
            metadata,
            prepared.report.complete,
            complete,
        )?;
        archive.save_versioned_parse_report(raw.id, &prepared.parser_version, &prepared.report)?;
        progress(options, "projection_published", raw);
    }
    check_cancel(options)?;
    archive.append_lineage_issues(&mut report.issues)?;
    archive.synchronize_search()?;
    super::maintenance(archive, options)?;
    Ok(())
}

fn retained_reference(
    archive: &Archive,
    harness: Harness,
    parent_revision: i64,
    reference: &Reference,
    options: &BackupOptions,
    issues: &mut Vec<Diagnostic>,
) -> Result<bool> {
    if reference.kind == "lineage" && !adapter(harness).lineage_is_path() {
        return Ok(true);
    }
    if reference.kind == "attachment" && reference.target.starts_with("data:") {
        return Ok(true);
    }
    let binding = archive.bound_reference(parent_revision, reference)?;
    if let Some(retained) = &binding
        && retained.bytes_retained
    {
        let mut reader = archive.raw_reader(retained.id)?;
        let mut buffer = vec![0u8; 64 * 1024];
        loop {
            check_cancel(options)?;
            if reader.read(&mut buffer)? == 0 {
                break;
            }
        }
        archive.record_reference_outcome(parent_revision, reference, None)?;
        return Ok(true);
    }
    if reference.kind == "lineage" {
        common::push_diagnostic(
            issues,
            Diagnostic {
                code: "lineage_not_retained".into(),
                path: None,
                ordinal: None,
                detail: Some("provenance_only".into()),
            },
        );
        return Ok(true);
    }
    let reason = if binding.is_some() {
        // The binding proves authority. Only the bound revision's bytes are gone.
        "missing".to_owned()
    } else if reference.kind == "remote" {
        "remote".to_owned()
    } else {
        archive
            .reference_reason(parent_revision, reference)?
            .flatten()
            .unwrap_or_else(|| "authority_unavailable".into())
    };
    super::reference_capture::attachment_issue(issues, &reason);
    archive.record_reference_outcome(parent_revision, reference, Some(&reason))?;
    Ok(false)
}

fn progress(options: &BackupOptions, phase: &str, raw: &RevisionInfo) {
    if let Some(observer) = &options.progress {
        observer(BackupProgress {
            phase: phase.into(),
            harness: None,
            source_id: raw.source_id.clone(),
            revision_id: Some(raw.id),
            bytes: raw.byte_len,
        });
    }
}

fn finish(
    archive: &Archive,
    source_id: &str,
    mut report: BackupReport,
    outcome: Result<()>,
) -> Result<BackupReport> {
    match outcome {
        Ok(()) => {
            if !report.issues.is_empty() {
                report.status = "partial".into();
            }
            let mut saved = serde_json::to_value(&report)?;
            saved["operation"] = serde_json::json!("reparse");
            saved["source_id"] = serde_json::json!(source_id);
            archive.end_backup_run(report.run_id, &report.status, &saved)?;
            Ok(report)
        }
        Err(error) => {
            report.status =
                if matches!(error, Error::Cancelled) { "interrupted" } else { "failed" }.into();
            if let Ok(mut saved) = serde_json::to_value(&report) {
                saved["operation"] = serde_json::json!("reparse");
                saved["source_id"] = serde_json::json!(source_id);
                saved["error_code"] = serde_json::json!(error.code());
                // A second persistence failure must not replace the original error.
                let _ = archive.end_backup_run(report.run_id, &report.status, &saved);
            }
            Err(error)
        }
    }
}
