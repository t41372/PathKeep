//! Responsibilities: resolve permitted paths and record redacted reference outcomes.
//! Not responsible for: treating native IDs as paths or lowering conversation parse coverage.
use super::{BackupOptions, BackupReport, capture_raw, references, source_identity};
use crate::{
    Archive, ArtifactKind, Diagnostic, Reference, Result, SourceRoot,
    adapters::{adapter, common},
};

pub(super) fn capture_reference(
    archive: &Archive,
    source: &SourceRoot,
    roots: &[SourceRoot],
    parent_revision: i64,
    reference: &Reference,
    options: &BackupOptions,
    report: &mut BackupReport,
) -> Result<bool> {
    if reference.kind == "lineage" && !adapter(source.harness).lineage_is_path() {
        return Ok(false);
    }
    if reference.kind == "remote" {
        return outcome(archive, parent_revision, reference, Some("remote"), &mut report.issues);
    }
    match references::resolve(source, roots, reference)? {
        references::Resolution::Inline => {
            outcome(archive, parent_revision, reference, None, &mut report.issues)
        }
        references::Resolution::Unresolved(diagnostic) => {
            if reference.kind == "lineage" {
                common::push_diagnostic(&mut report.issues, diagnostic);
                return Ok(false);
            }
            outcome(
                archive,
                parent_revision,
                reference,
                diagnostic.detail.as_deref(),
                &mut report.issues,
            )
        }
        references::Resolution::Local { source, mut artifact } => {
            let identity = source_identity(&source, &artifact)?;
            if reference.kind == "lineage" {
                artifact.kind = ArtifactKind::Transcript;
            }
            if archive.latest_revision(&identity)?.is_some() {
                artifact.kind = match archive.source(&identity)?.kind.as_str() {
                    "transcript" => ArtifactKind::Transcript,
                    "metadata" => ArtifactKind::Metadata,
                    "sqlite" => ArtifactKind::Sqlite,
                    _ => ArtifactKind::Attachment,
                };
            }
            let mut failure = None;
            let captured = capture_raw(
                archive,
                &identity,
                &source,
                &artifact,
                options,
                report,
                Some(&mut failure),
            )?;
            if let Some(captured) = captured {
                archive.bind_reference(parent_revision, reference, captured.revision_id)?;
                if artifact.kind != ArtifactKind::Transcript {
                    archive.set_coverage(&identity, captured.complete)?;
                }
                if reference.kind == "lineage" {
                    return Ok(false);
                }
                outcome(archive, parent_revision, reference, None, &mut report.issues)
            } else if reference.kind == "lineage" {
                Ok(false)
            } else {
                outcome(
                    archive,
                    parent_revision,
                    reference,
                    Some(failure.as_deref().unwrap_or("authority_unavailable")),
                    &mut report.issues,
                )
            }
        }
    }
}

pub(super) fn attachment_issue(issues: &mut Vec<Diagnostic>, reason: &str) {
    common::push_diagnostic(
        issues,
        Diagnostic {
            code: "attachment_not_retained".into(),
            path: None,
            ordinal: None,
            detail: Some(reason.into()),
        },
    );
}

fn outcome(
    archive: &Archive,
    revision: i64,
    reference: &Reference,
    reason: Option<&str>,
    issues: &mut Vec<Diagnostic>,
) -> Result<bool> {
    // A file can disappear locally after it was successfully retained. Its
    // existing binding still supplies the file; that is not an unkept attachment.
    let retained = if reason.is_some() {
        archive.bound_reference(revision, reference)?.is_some_and(|info| info.bytes_retained)
    } else {
        false
    };
    let reason = if retained { None } else { reason };
    if let Some(reason) = reason {
        attachment_issue(issues, reason);
    }
    archive.record_reference_outcome(revision, reference, reason)
}
