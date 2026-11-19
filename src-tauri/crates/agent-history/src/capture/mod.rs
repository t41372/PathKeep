//! Responsibilities: coordinate explicit source capture, interpretation and publication.
//! Not responsible for: scheduling, UI consent, cloud history, or modifying native sources.
//!
//! Capture can succeed while interpretation is partial. Destination failures abort
//! the operation; source limitations retain earlier evidence and produce diagnostics.
mod defaults;
mod raw;
use raw::capture_raw;
#[cfg(test)]
mod git_io_tests;
mod roots;
use roots::normalize_sources;
pub mod files;
pub(crate) mod git;
mod observations;
mod plan;
mod projection;
mod readiness;
#[cfg(test)]
mod readiness_tests;
mod reference_capture;
mod references;
mod reparse;
mod unreadable;
mod writer;

pub use defaults::default_sources;
pub use plan::{BackupPlan, PlannedArtifact, preview, source_identity};

use crate::{
    Archive, Artifact, ArtifactKind, Diagnostic, Error, Result, SourceRoot,
    adapters::{adapter, common},
};
use serde::Serialize;
use std::sync::Arc;
use vault_store::CancelToken;

/// Explicit budgets and cancellation keep a single enormous transcript bounded.
pub struct BackupOptions {
    pub max_record_bytes: usize,
    pub read_git: bool,
    pub max_decoded_bytes: u64,
    pub cancel: CancelToken,
    pub progress: Option<Arc<dyn Fn(BackupProgress) + Send + Sync>>,
}

impl Default for BackupOptions {
    fn default() -> Self {
        Self {
            read_git: true,
            max_record_bytes: 16 * 1024 * 1024,
            max_decoded_bytes: 8 * 1024 * 1024 * 1024,
            cancel: CancelToken::new(),
            progress: None,
        }
    }
}

/// A durable boundary can drive a responsive UI without exposing conversation text.
#[derive(Clone, Debug, Serialize)]
pub struct BackupProgress {
    pub phase: String,
    pub source_id: String,
    pub revision_id: Option<i64>,
    pub bytes: u64,
}

/// Counts refer to observed artifacts; diagnostics explain incomplete source coverage.
#[derive(Clone, Debug, Serialize)]
pub struct BackupReport {
    pub status: String,
    pub captured: u64,
    pub unchanged: u64,
    pub bytes: u64,
    pub run_id: i64,
    pub issues: Vec<Diagnostic>,
}

impl Archive {
    /// Executes an explicitly selected set of roots. The caller owns consent;
    /// `preview` provides the matching read-only inventory before this write.
    pub fn backup(&self, sources: &[SourceRoot], options: &BackupOptions) -> Result<BackupReport> {
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
        let selected = normalize_sources(self, sources)?;
        let run_id = self.start_backup_run()?;
        let mut report = BackupReport {
            status: "complete".into(),
            captured: 0,
            unchanged: 0,
            bytes: 0,
            run_id,
            issues: Vec::new(),
        };
        let outcome = execute(self, &selected, sources, options, &mut report);
        match outcome {
            Ok(()) => {
                if !report.issues.is_empty() {
                    report.status = "partial".into();
                }
                self.end_backup_run(run_id, &report.status, &serde_json::to_value(&report)?)?;
                Ok(report)
            }
            Err(error) => {
                report.status = if matches!(error, Error::Cancelled) {
                    "interrupted".into()
                } else {
                    "failed".into()
                };
                // Preserve the original destination/cancellation error if writing its
                // terminal audit record also fails. Recovery marks it interrupted.
                if let Ok(mut saved) = serde_json::to_value(&report) {
                    saved["error_code"] = serde_json::json!(error.code());
                    let _ = self.end_backup_run(run_id, &report.status, &saved);
                }
                Err(error)
            }
        }
    }
}

fn execute(
    archive: &Archive,
    sources: &[SourceRoot],
    declared_roots: &[SourceRoot],
    options: &BackupOptions,
    report: &mut BackupReport,
) -> Result<()> {
    for source in sources {
        check_cancel(options)?;
        let root_key = plan::root_identity(source)?;
        let readable = files::root_readable(&source.root);
        if !readable {
            issue(&mut report.issues, "root_unreadable", source.root.to_str());
            archive.set_coverage(&root_key, false)?;
            continue;
        }
        let mut discovery_issues = Vec::new();
        let mut callback_error = None;
        let discovered = adapter(source.harness).discover(
            source,
            &mut |artifact| match process_artifact(
                archive,
                source,
                &artifact,
                // Preserve explicitly approved aliases (such as /var on macOS)
                // for lexical reference authorization. Storage identities still
                // use canonical roots; unapproved targets are never probed.
                declared_roots,
                options,
                report,
            ) {
                Ok(()) => Ok(()),
                Err(error) => {
                    callback_error = Some(error);
                    Err(Error::Integrity("capture callback failed".into()))
                }
            },
            &mut discovery_issues,
        );
        if let Some(error) = callback_error {
            return Err(error);
        }
        if discovered.is_err() {
            issue(&mut discovery_issues, "source_discovery_failed", source.root.to_str());
        }
        archive.observe_missing(report.run_id, source)?;
        archive.set_coverage(&root_key, discovery_issues.is_empty())?;
        for diagnostic in discovery_issues {
            common::push_diagnostic(&mut report.issues, diagnostic);
        }
    }
    check_cancel(options)?;
    observations::folders(archive, options, report)?;
    archive.append_lineage_issues(&mut report.issues)?;
    archive.synchronize_search()?;
    maintenance(archive, options)?;
    // Unreferenced slices stay queued until a collection transaction succeeds.
    if archive.reclaim_slices().is_err() {
        issue(&mut report.issues, "slice_reclamation_deferred", None);
    }
    Ok(())
}

fn process_artifact(
    archive: &Archive,
    source: &SourceRoot,
    artifact: &Artifact,
    roots: &[SourceRoot],
    options: &BackupOptions,
    report: &mut BackupReport,
) -> Result<()> {
    check_cancel(options)?;
    let source_id = match source_identity(source, artifact) {
        Ok(identity) => identity,
        Err(_) => {
            issue(&mut report.issues, "source_path_unrepresentable", artifact.path.to_str());
            return Ok(());
        }
    };
    let Some(captured) = capture_raw(archive, &source_id, source, artifact, options, report, None)?
    else {
        return Ok(());
    };
    if artifact.kind != ArtifactKind::Transcript
        || (!artifact.current && !adapter(source.harness).current_source_only())
    {
        // Discovery may replace a fallback (for example Cowork audit) with a
        // primary transcript. Its bytes remain evidence, but its old view must
        // no longer duplicate the primary conversation in search.
        archive.retire_projection(&source_id)?;
        archive.set_coverage(&source_id, captured.complete)?;
        return Ok(());
    }
    let prepared = match projection::prepare(
        archive,
        source,
        artifact,
        &source_id,
        captured.revision_id,
        options,
        &mut report.issues,
        false,
    )? {
        projection::Preparation::Ready(prepared) => prepared,
        projection::Preparation::Writing => return Ok(()),
        projection::Preparation::Unreadable => {
            unreadable::publish(
                archive,
                source,
                artifact,
                &source_id,
                captured.revision_id,
                options,
            )?;
            archive.set_coverage(&source_id, false)?;
            return Ok(());
        }
    };
    if prepared.report.session.as_ref().is_some_and(|session| session.native_id.is_empty())
        && unreadable::publish(
            archive,
            source,
            artifact,
            &source_id,
            captured.revision_id,
            options,
        )?
    {
        archive.set_coverage(&source_id, false)?;
        return Ok(());
    }
    let complete = prepared.report.complete;
    let mut reference_changes = false;
    if !complete && prepared.report.diagnostics.is_empty() {
        issue(&mut report.issues, "source_interpretation_partial", Some(&artifact.relative_path));
    }
    for diagnostic in &prepared.report.diagnostics {
        if diagnostic.detail.as_deref() != Some("remote") {
            common::push_diagnostic(&mut report.issues, diagnostic.clone());
        }
    }
    for reference in &prepared.report.references {
        check_cancel(options)?;
        reference_changes |= reference_capture::capture_reference(
            archive,
            source,
            roots,
            captured.revision_id,
            reference,
            options,
            report,
        )?;
    }
    {
        let metadata = prepared.report.session.as_ref().expect("prepared metadata");
        if let Some(attempt) = &prepared.attempt {
            progress(options, "projection_staged", &source_id, &captured);
            check_cancel(options)?;
            archive.publish_projection_attempt_with_coverage(
                attempt,
                metadata,
                prepared.report.complete,
                complete,
            )?;
            archive.save_versioned_parse_report(
                captured.revision_id,
                &prepared.parser_version,
                &prepared.report,
            )?;
            progress(options, "projection_published", &source_id, &captured);
        } else if let Some(projection_id) = archive.current_projection_id(&source_id)? {
            archive.update_projection_coverage(
                &source_id,
                projection_id,
                prepared.report.complete,
                complete,
            )?;
            if reference_changes {
                archive.refresh_attachment_counter(&source_id, projection_id)?;
            }
        }
    }
    Ok(())
}

pub(super) fn check_cancel(options: &BackupOptions) -> Result<()> {
    options.cancel.check().map_err(|_| Error::Cancelled)
}

/// Spend a fixed amount of foreground work on disposable staging/projections.
/// Embedders can continue the same bounded archive APIs during idle time; this
/// cannot delete a completed raw revision or a chunk still referenced by one.
pub(super) fn maintenance(archive: &Archive, options: &BackupOptions) -> Result<()> {
    for _ in 0..4 {
        check_cancel(options)?;
        let projections = archive.prune_obsolete_projection_batch(256)?;
        let captures = archive.prune_abandoned_capture_batch(256)?;
        let counters = archive.prune_attachment_counts()?;
        if archive.prune_reader_pages(256)? == 0
            && projections == 0
            && captures == 0
            && counters == 0
        {
            break;
        }
    }
    Ok(())
}

fn progress(options: &BackupOptions, phase: &str, source_id: &str, captured: &writer::Captured) {
    if let Some(observer) = &options.progress {
        observer(BackupProgress {
            phase: phase.into(),
            source_id: source_id.into(),
            revision_id: Some(captured.revision_id),
            bytes: captured.byte_len,
        });
    }
}

fn issue(issues: &mut Vec<Diagnostic>, code: &str, path: Option<&str>) {
    common::push_diagnostic(
        issues,
        Diagnostic {
            code: code.into(),
            path: path.map(str::to_owned),
            ordinal: None,
            detail: None,
        },
    );
}
