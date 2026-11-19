//! Responsibilities: coordinate explicit source capture, interpretation and publication.
//! Not responsible for: scheduling, UI consent, cloud history, or modifying native sources.
//!
//! Capture can succeed while interpretation is partial. Destination failures abort
//! the operation; source limitations retain earlier evidence and produce diagnostics.
mod debug_delay;
mod defaults;
mod diagnostics;
use diagnostics::RootOutcome;
pub use diagnostics::{CaptureOutcome, SaveWarning};
pub(crate) mod discovery;
pub use discovery::Discovery;
mod grant;
pub use grant::{directory_handle, directory_identity, directory_issue};
mod inventory;
pub use inventory::{
    InventoryCounts, InventoryObserver, RootInventory, inventory_root, inventory_root_bound,
    root_status,
};
pub use plan::{normalized_root, root_identity};
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
pub(crate) mod references;
mod reparse;
mod unreadable;
mod writer;

pub use defaults::{default_sources, known_sources};
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
    /// Desktop grants bind native directory identity across discovery and file opens.
    /// Standalone callers declare their source paths afresh on each invocation.
    pub approved_directories: Option<std::collections::BTreeMap<std::path::PathBuf, String>>,
    pub max_decoded_bytes: u64,
    pub cancel: CancelToken,
    pub progress: Option<Arc<dyn Fn(BackupProgress) + Send + Sync>>,
}

impl Default for BackupOptions {
    fn default() -> Self {
        Self {
            read_git: true,
            approved_directories: None,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub harness: Option<crate::Harness>,
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
    #[serde(rename = "captureOutcome", skip_serializing_if = "Option::is_none")]
    pub capture_outcome: Option<CaptureOutcome>,
    pub issues: Vec<Diagnostic>,
    /// Bounded indices into the native report, with the directory that produced the warning.
    #[serde(
        rename = "warningSources",
        skip_serializing_if = "std::collections::BTreeMap::is_empty"
    )]
    pub warning_sources: std::collections::BTreeMap<usize, WarningSource>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WarningSource {
    pub root_id: String,
    pub tool: crate::Harness,
}

impl Archive {
    /// Executes an explicitly selected set of roots. The caller owns consent;
    /// `preview` provides the matching read-only inventory before this write.
    pub fn backup(&self, sources: &[SourceRoot], options: &BackupOptions) -> Result<BackupReport> {
        options.validate()?;
        let _job = self.jobs.enter(Some(&options.cancel))?;
        let selected = normalize_sources(self, sources)?;
        check_cancel(options)?;
        let run_id = self.start_backup_run()?;
        self.execute_run(run_id, &selected, sources, options, Vec::new())
    }

    /// Keep the owner's generation and producer slot alive from consent publication
    /// until the terminal report. Maintenance can signal and drain this lease.
    pub fn reserve_backup(self: &Arc<Self>, options: BackupOptions) -> Result<ReservedBackup> {
        options.validate()?;
        let job = self.jobs.enter(Some(&options.cancel))?;
        Ok(ReservedBackup { archive: self.clone(), options, _job: job })
    }

    fn execute_run(
        &self,
        run_id: i64,
        selected: &[SourceRoot],
        declared: &[SourceRoot],
        options: &BackupOptions,
        issues: Vec<Diagnostic>,
    ) -> Result<BackupReport> {
        let mut capture_outcome = CaptureOutcome::default();
        // The owner rejected these selected directories before discovery could start.
        for issue in &issues {
            capture_outcome.warning(issue, None);
        }
        let mut report = BackupReport {
            status: "complete".into(),
            captured: 0,
            unchanged: 0,
            bytes: 0,
            run_id,
            capture_outcome: Some(capture_outcome),
            warning_sources: Default::default(),
            issues,
        };
        let outcome = execute(self, selected, declared, options, &mut report).and_then(|()| {
            if report.capture_outcome.as_ref().is_some_and(|o| o.warning_count > 0) {
                report.status = "partial".into();
            }
            self.end_backup_run(run_id, &report.status, &serde_json::to_value(&report)?)
        });
        if let Err(error) = outcome {
            report.status = if matches!(error, Error::Cancelled) {
                "interrupted"
            } else {
                let issue = Diagnostic {
                    code: "archive-publication-failed".into(),
                    path: None,
                    ordinal: None,
                    detail: Some(error.code().into()),
                };
                report.capture_outcome.as_mut().expect("backup outcome").warning(&issue, None);
                common::push_diagnostic(&mut report.issues, issue);
                if report.captured > 0 || report.unchanged > 0 { "partial" } else { "failed" }
            }
            .into();
            // Preserve the original error if the archive cannot even publish the
            // terminal receipt. Open will then recover the attempt as interrupted.
            if let Ok(mut saved) = serde_json::to_value(&report) {
                saved["error_code"] = serde_json::json!(error.code());
                let _ = self.end_backup_run(run_id, &report.status, &saved);
            }
            return Err(error);
        }
        Ok(report)
    }
}

/// The owner has reserved this slot before publishing an authorized request.
pub struct ReservedBackup {
    archive: Arc<Archive>,
    options: BackupOptions,
    _job: crate::archive::lifetime::Job,
}

impl ReservedBackup {
    /// The owner inserted this native run and its public receipt in one transaction.
    pub fn execute(
        &self,
        run_id: i64,
        sources: &[SourceRoot],
        unavailable: Vec<Diagnostic>,
    ) -> Result<BackupReport> {
        let selected = normalize_sources(&self.archive, sources)?;
        self.archive.execute_run(run_id, &selected, sources, &self.options, unavailable)
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
        let mut state = RootOutcome::default();
        let issues =
            capture_root(archive, source, &root_key, declared_roots, options, report, &mut state)?;
        state.include(&issues);
        record_diagnostics(archive, source, &root_key, None, report, issues)?;
        archive.root_outcome(report.run_id, &root_key, source, state.state())?;
    }
    check_cancel(options)?;
    for source in sources {
        let root = plan::root_identity(source)?;
        let earlier = std::mem::take(&mut report.issues);
        if matches!(
            source.harness,
            crate::Harness::ClaudeCode
                | crate::Harness::Codex
                | crate::Harness::Pi
                | crate::Harness::Deepseek
        ) && options.validate_root(&source.root).is_ok()
        {
            observations::folders(archive, source, options, report)?;
        }
        archive.lineage_issues_for(&mut report.issues, Some(source), &options.cancel)?;
        let issues = std::mem::replace(&mut report.issues, earlier);
        record_diagnostics(archive, source, &root, None, report, issues)?;
    }
    notify_phase(options, "index", None);
    archive.synchronize_search_with_cancel(&|| options.cancel.check().is_err())?;
    maintenance(archive, options)?;
    // Unreferenced slices stay queued until a collection transaction succeeds.
    check_cancel(options)?;
    if archive.reclaim_slices_with_cancel(&options.cancel).is_err() {
        check_cancel(options)?;
        let mut cleanup = Vec::new();
        issue(&mut cleanup, "slice_reclamation_deferred", None);
        if let Some(source) = sources.first() {
            record_diagnostics(
                archive,
                source,
                &plan::root_identity(source)?,
                None,
                report,
                cleanup,
            )?;
        } else {
            report.issues.extend(cleanup);
        }
    }
    check_cancel(options)?;
    notify_phase(options, "finishing", None);
    check_cancel(options)
}

fn capture_root(
    archive: &Archive,
    source: &SourceRoot,
    root_key: &str,
    declared_roots: &[SourceRoot],
    options: &BackupOptions,
    report: &mut BackupReport,
    state: &mut RootOutcome,
) -> Result<Vec<Diagnostic>> {
    let mut discovery_issues = Vec::new();
    let pin = match options.directory(&source.root)? {
        Some(expected) => match directory_handle(&source.root) {
            Ok(handle) if directory_identity(&handle).is_ok_and(|now| now == expected) => {
                Some((source.root.clone(), Arc::new(handle)))
            }
            _ => {
                issue(&mut discovery_issues, "root_changed", source.root.to_str());
                archive.set_coverage(root_key, false)?;
                return Ok(discovery_issues);
            }
        },
        None => None,
    };
    if !files::root_readable(&source.root) {
        issue(&mut discovery_issues, "root_unreadable", source.root.to_str());
        archive.set_coverage(root_key, false)?;
        return Ok(discovery_issues);
    }
    let discovery = Discovery::new(&options.cancel, pin);

    let mut callback_error = None;
    let delay = debug_delay::configured("PATHKEEP_TEST_AGENT_CAPTURE_FILE_DELAY_US");
    let discovered = adapter(source.harness).discover_cancellable(
        &discovery,
        source,
        &mut |artifact| {
            notify_phase(options, "file_found", Some(source.harness));
            debug_delay::wait(delay, &options.cancel)?;
            if !discovery.still_selected() {
                return Err(Error::SourceChanged("reviewed root changed".into()));
            }
            // Declared aliases constrain native references. A reference is never a new grant.
            let earlier = std::mem::take(&mut report.issues);
            let processed =
                process_artifact(archive, source, &artifact, declared_roots, options, report);
            let issues = std::mem::replace(&mut report.issues, earlier);
            state.include(&issues);
            let recorded = processed.and_then(|()| {
                record_diagnostics(
                    archive,
                    source,
                    root_key,
                    Some(&artifact.relative_path),
                    report,
                    issues,
                )
            });
            if let Err(error) = recorded {
                callback_error = Some(error);
                return Err(Error::Integrity("capture callback failed".into()));
            }
            if !discovery.still_selected() {
                return Err(Error::SourceChanged("reviewed root changed".into()));
            }
            Ok(())
        },
        &mut discovery_issues,
    );
    if let Some(error) = callback_error {
        return Err(error);
    }
    check_cancel(options)?;
    if discovered.is_err() {
        issue(&mut discovery_issues, "source_discovery_failed", source.root.to_str());
    }
    if options.validate_root(&source.root).is_err() {
        issue(&mut discovery_issues, "root_changed", source.root.to_str());
    }
    if discovery_issues.is_empty() && !archive.observe_missing(report.run_id, source, &discovery)? {
        issue(&mut discovery_issues, "root_changed", source.root.to_str());
    }
    archive.set_coverage(root_key, discovery_issues.is_empty())?;
    Ok(discovery_issues)
}

/// Keep the raw CLI samples bounded. Product status and note identities were
/// recorded at file/root scope before different files' samples are merged.
fn record_diagnostics(
    archive: &Archive,
    source: &SourceRoot,
    root: &str,
    file: Option<&str>,
    report: &mut BackupReport,
    issues: Vec<Diagnostic>,
) -> Result<()> {
    if issues.is_empty() {
        return Ok(());
    }
    report.capture_outcome.as_mut().expect("backup outcome").observe(
        archive,
        report.run_id,
        root,
        source.harness,
        file,
        &issues,
    )?;
    for diagnostic in issues {
        let sample =
            report.issues.iter().position(Diagnostic::omission).unwrap_or(report.issues.len());
        common::push_diagnostic(&mut report.issues, diagnostic);
        if report.issues.get(sample).is_some_and(|d| !d.omission()) {
            report
                .warning_sources
                .insert(sample, WarningSource { root_id: root.to_owned(), tool: source.harness });
        }
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
    if captured.byte_len == 0 && adapter(source.harness).needs_first_line_end() {
        issue(&mut report.issues, "empty_transcript", Some(&artifact.relative_path));
        archive.set_coverage(&source_id, false)?;
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
        projection::Preparation::Writing => {
            archive.file_problem(report.run_id, &source_id, "unstable")?;
            return Ok(());
        }
        projection::Preparation::Unreadable => {
            archive.file_problem(report.run_id, &source_id, "unknownFormat")?;
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
    for diagnostic in &prepared.report.diagnostics {
        if diagnostic.detail.as_deref() != Some("remote") {
            common::push_diagnostic(&mut report.issues, diagnostic.clone());
        }
    }
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
    if prepared.report.diagnostics.iter().any(|d| {
        d.code.contains("unsupported")
            || matches!(d.code.as_str(), "session_metadata_unavailable" | "deepseek-format-limited")
    }) {
        archive.file_problem(report.run_id, &source_id, "unknownFormat")?;
    }
    let prior = archive.published_identity(&source_id)?;
    let complete = prepared.report.complete;
    let mut reference_changes = false;
    if !complete && prepared.report.diagnostics.is_empty() {
        issue(&mut report.issues, "source_interpretation_partial", Some(&artifact.relative_path));
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
            progress(options, source.harness, "projection_staged", &source_id, &captured);
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
            progress(options, source.harness, "projection_published", &source_id, &captured);
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
    if prepared.report.session.is_some_and(|m| !m.native_id.is_empty()) {
        if let Some(unit) = archive.observed_identity(
            report.run_id,
            &source_id,
            captured.revision_id,
            !captured.unchanged,
            prior,
        )? {
            notify_phase(options, unit, Some(source.harness));
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
        if projections == 0 && captures == 0 && counters == 0 {
            break;
        }
    }
    Ok(())
}

fn progress(
    options: &BackupOptions,
    harness: crate::Harness,
    phase: &str,
    source_id: &str,
    captured: &writer::Captured,
) {
    if let Some(observer) = &options.progress {
        observer(BackupProgress {
            phase: phase.into(),
            harness: Some(harness),
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

fn notify_phase(options: &BackupOptions, phase: &str, harness: Option<crate::Harness>) {
    if let Some(observer) = &options.progress {
        observer(BackupProgress {
            phase: phase.into(),
            harness,
            source_id: String::new(),
            revision_id: None,
            bytes: 0,
        });
    }
}

impl BackupOptions {
    fn validate(&self) -> Result<()> {
        if !(1..=64 * 1024 * 1024).contains(&self.max_record_bytes) || self.max_decoded_bytes == 0 {
            return Err(Error::InvalidInput(
                "capture byte budgets are outside supported bounds".into(),
            ));
        }
        Ok(())
    }
    fn directory(&self, root: &std::path::Path) -> Result<Option<&str>> {
        self.approved_directories
            .as_ref()
            .map(|grants| {
                grants
                    .get(root)
                    .map(String::as_str)
                    .ok_or_else(|| Error::InvalidInput("source has no reviewed directory".into()))
            })
            .transpose()
    }
    fn validate_root(&self, path: &std::path::Path) -> Result<()> {
        if let Some(expected) = self.directory(path)?
            && directory_identity(&directory_handle(path)?)? != expected
        {
            return Err(Error::SourceChanged("approved directory was replaced".into()));
        }
        Ok(())
    }
}
