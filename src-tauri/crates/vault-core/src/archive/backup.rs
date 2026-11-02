//! Responsibilities: stage selected profiles, commit batches and finish the run ledger.
//! Not responsible for: SQL ownership, parser implementation or index rebuilding.
//! Failure modes: discovery/staging failures leave invisible runs, cancellation
//! loses a committed prefix, progress outruns data, interactive writes starve,
//! skipped sources disappear from audit, run completion precedes its last batch.
//! Tests: ingest::concurrency_tests, ingest::dedup_scenarios* and archive tests.

use super::{
    ingest::{self, ArchiveIngestProgress, process_profile_snapshot},
    write_runs,
};
use crate::{
    Vault,
    models::{
        BackupProfileSummary, BackupProgressEvent, BackupReport, BackupRunOverview, BrowserProfile,
        ProfileOutcome,
    },
};
use anyhow::{Context, Result};
use vault_store::{CancelToken, Db, Priority, params};

/// Backups use the same owner as interactive commands.
pub fn run_backup(vault: &Vault, due_only: bool) -> Result<BackupReport> {
    run_backup_with_progress(vault, due_only, |_| {})
}

/// Progress is emitted after each committed batch, on the requesting thread.
pub fn run_backup_with_progress(
    vault: &Vault,
    due_only: bool,
    progress: impl FnMut(BackupProgressEvent),
) -> Result<BackupReport> {
    run_backup_with_control(vault, due_only, &CancelToken::new(), progress)
}

/// Cancellation stops between batches; the committed prefix remains visible.
pub fn run_backup_with_control(
    vault: &Vault,
    due_only: bool,
    cancel: &CancelToken,
    progress: impl FnMut(BackupProgressEvent),
) -> Result<BackupReport> {
    execute(
        vault,
        due_only,
        cancel,
        || crate::chrome::discover_profiles().map_err(classify_browser_access_error),
        false,
        progress,
    )
}

#[cfg(test)]
pub(crate) fn run_backup_profiles(
    vault: &Vault,
    due_only: bool,
    cancel: &CancelToken,
    profiles: Vec<BrowserProfile>,
    progress: impl FnMut(BackupProgressEvent),
) -> Result<BackupReport> {
    execute(vault, due_only, cancel, || Ok(profiles), true, progress)
}

fn execute(
    vault: &Vault,
    due_only: bool,
    cancel: &CancelToken,
    profiles: impl FnOnce() -> Result<Vec<BrowserProfile>>,
    all_profiles: bool,
    mut report: impl FnMut(BackupProgressEvent),
) -> Result<BackupReport> {
    let _backup = vault.begin_backup()?;
    anyhow::ensure!(vault.config().initialized, "archive has not been initialized");
    crate::config::ensure_paths(vault.paths())?;
    if due_only && let Some(reason) = due_skip_reason(vault)? {
        return skipped(vault, reason);
    }
    let trigger = if due_only { "schedule" } else { "manual" };
    let run_id = write_runs::start(vault, "backup", trigger, &vault.config().selected_profile_ids)?;
    let mut warnings = Vec::new();
    let mut summaries = Vec::new();
    let result = (|| {
        cancel.check()?;
        anyhow::ensure!(
            all_profiles || !vault.config().selected_profile_ids.is_empty(),
            "select at least one readable browser profile before running a backup"
        );
        let (discovered, discovery_outcome) = match profiles() {
            Ok(profiles) => (profiles, None),
            Err(error) => {
                if let Some(source) = error.downcast_ref::<super::source_error::SourceError>() {
                    (Vec::new(), Some(source.outcome()))
                } else {
                    return Err(error);
                }
            }
        };
        let scope = if all_profiles && !discovered.is_empty() {
            discovered.iter().map(|profile| profile.profile_id.clone()).collect::<Vec<_>>()
        } else {
            vault.config().selected_profile_ids.clone()
        };
        // With no profile at all there is nothing a run could report on; "success" would claim
        // a backup that read nothing.
        anyhow::ensure!(
            !scope.is_empty(),
            "backup-no-profiles: PathKeep found no browser profile to back up on this computer"
        );
        let mut selected = Vec::new();
        for id in scope {
            let profile = discovered.iter().find(|profile| profile.profile_id == id);
            let code = profile.is_none().then_some("source-profile-not-discovered");
            if let Some(code) = code {
                summaries.push(BackupProfileSummary {
                    profile_id: id,
                    outcome: discovery_outcome
                        .clone()
                        .unwrap_or(ProfileOutcome::Unavailable { code: code.into() }),
                    ..Default::default()
                });
            } else if let Some(profile) = profile {
                selected.push(profile);
            }
        }
        report(
            BackupProgressEvent {
                phase: "prepare".into(),
                label: "Inspect selected browser profiles".into(),
                total_profiles: selected.len(),
                ..Default::default()
            }
            .with_log_event("info", "backup.prepare"),
        );
        let mut hashes = std::collections::BTreeMap::new();
        for (index, profile) in selected.iter().enumerate() {
            cancel.check()?;
            report(
                BackupProgressEvent {
                    phase: "stage-profile".into(),
                    label: "Stage source profile".into(),
                    profile_id: Some(profile.profile_id.clone()),
                    completed_profiles: index,
                    total_profiles: selected.len(),
                    ..BackupProgressEvent::default()
                }
                .with_log_event("info", "backup.stage-profile"),
            );
            let staged = match crate::chrome::stage_profile_snapshot(vault.paths(), profile) {
                Ok(staged) => staged,
                Err(error) => {
                    if let Some(source) = error.downcast_ref::<super::source_error::SourceError>() {
                        let outcome = source.outcome();
                        let code = outcome.code().expect("source errors have a code");
                        warnings.push(
                            crate::models::BackupWarning::new(code, code)
                                .with_profile_id(profile.profile_id.clone())
                                .with_diagnostic(format!("{error:#}")),
                        );
                        summaries.push(BackupProfileSummary {
                            profile_id: profile.profile_id.clone(),
                            outcome,
                            ..Default::default()
                        });
                        continue;
                    }
                    return Err(error);
                }
            };
            warnings.extend(staged.warnings);
            let mut previous = 0;
            let summary = process_profile_snapshot(
                vault,
                run_id,
                &staged.snapshot,
                true,
                true,
                cancel,
                Some(Box::new(|progress| {
                    emit_backup_ingest_progress_if_changed(
                        &mut report,
                        &mut previous,
                        index,
                        selected.len(),
                        profile,
                        progress,
                    )
                })),
            )
            .with_context(|| {
                format!(
                    "ingesting staged profile {} from {}",
                    profile.profile_id,
                    staged.snapshot.history_path.display()
                )
            });
            let summary = match summary {
                Ok(summary) => summary,
                Err(error) => {
                    cancel.check()?;
                    if let Some(source) = error.downcast_ref::<super::source_error::SourceError>() {
                        let outcome = source.outcome();
                        let code = outcome.code().expect("source errors have a code");
                        warnings.push(
                            crate::models::BackupWarning::new(code, code)
                                .with_profile_id(profile.profile_id.clone())
                                .with_diagnostic(format!("{error:#}")),
                        );
                        summaries.push(BackupProfileSummary {
                            profile_id: profile.profile_id.clone(),
                            outcome,
                            ..Default::default()
                        });
                        continue;
                    }
                    return Err(error);
                }
            };
            hashes.insert(
                profile.profile_id.clone(),
                ingest::snapshot_source_hashes(&staged.snapshot),
            );
            warnings.extend(summary.note_details.iter().cloned());
            summaries.push(summary);
        }
        cancel.check()?;
        report(
            BackupProgressEvent {
                phase: "finalize".into(),
                label: "Finalize manifest and cached totals".into(),
                total_profiles: selected.len(),
                completed_profiles: selected.len(),
                ..Default::default()
            }
            .with_log_event("info", "backup.finalize"),
        );
        super::backup_manifest::finish(vault, run_id, summaries.clone(), warnings.clone(), hashes)
    })();
    if let Err(error) = &result {
        write_runs::end_failed(
            vault,
            run_id,
            error,
            serde_json::json!({"profilesProcessed":summaries.len(),"profiles":summaries}),
            serde_json::to_value(&warnings).ok(),
        );
    }
    result
}

fn due_skip_reason(vault: &Vault) -> Result<Option<String>> {
    let latest=vault.store().read(Db::Archive,|s| {
        Ok(s.query("SELECT finished_at FROM runs WHERE run_type='backup' AND status IN ('success','partial') ORDER BY id DESC LIMIT 1",(),|r|Ok(r.get::<String>(0)?))?.pop())
    })?;
    Ok(latest.and_then(|text| chrono::DateTime::parse_from_rfc3339(&text).ok()).and_then(|at| {
        super::run_support::backup_due_skip_reason_at(
            at.with_timezone(&chrono::Utc),
            vault.config(),
            chrono::Utc::now(),
        )
    }))
}

fn skipped(vault: &Vault, reason: String) -> Result<BackupReport> {
    let detail = reason.clone();
    let scope = vault.config().selected_profile_ids.clone();
    let (id, at, scope) = vault.write(Db::Archive, Priority::Batch, move |s| {
        let id = write_runs::start_in(s, "backup", "schedule", &scope)?;
        let at = s.query_one("SELECT started_at FROM runs WHERE id=?1", [id], |r| {
            Ok(r.get::<String>(0)?)
        })?;
        s.execute(
            "UPDATE runs SET status='skipped',finished_at=started_at,stats_json=?2 WHERE id=?1",
            params![id, serde_json::json!({"reasonCode":"not-due","reason":detail}).to_string()],
        )?;
        Ok((id, at, scope))
    })?;
    Ok(BackupReport {
        due_skipped: true,
        reason_code: Some("not-due".into()),
        reason: Some(reason),
        run: Some(BackupRunOverview {
            id,
            started_at: at.clone(),
            finished_at: Some(at),
            status: "skipped".into(),
            run_type: "backup".into(),
            trigger: "schedule".into(),
            profile_scope: scope,
            ..Default::default()
        }),
        ..Default::default()
    })
}

pub(super) fn classify_browser_access_error(error: anyhow::Error) -> anyhow::Error {
    if crate::browser_access::is_permission_denied(&error) {
        error.context(super::source_error::SourceError::Unavailable("source-permission-denied"))
    } else if let Some(io) = error.chain().find_map(|cause| cause.downcast_ref::<std::io::Error>())
    {
        let marker = super::source_error::SourceError::access(io);
        error.context(marker)
    } else {
        error
    }
}

pub(super) fn emit_backup_ingest_progress_if_changed(
    report_progress: &mut impl FnMut(BackupProgressEvent),
    last_processed_records: &mut usize,
    index: usize,
    total_profiles: usize,
    profile: &BrowserProfile,
    progress: ArchiveIngestProgress,
) {
    if progress.processed_records == *last_processed_records {
        return;
    }
    *last_processed_records = progress.processed_records;
    report_progress(
        BackupProgressEvent {
            phase: "ingest-profile".to_string(),
            label: "Write canonical archive facts".to_string(),
            detail: format!("Processing {} and writing archive rows.", profile.profile_id),
            step: 1,
            total_steps: 3,
            completed_profiles: index,
            total_profiles,
            profile_id: Some(profile.profile_id.clone()),
            progress_current: Some(index + 1),
            progress_total: Some(total_profiles),
            progress_percent: None,
            log_lines: vec![format!("{} ({}/{total_profiles})", profile.profile_id, index + 1)],
            source_label: Some(format!("{} / {}", profile.browser_name, profile.profile_name)),
            processed_records: Some(progress.processed_records),
            total_records: None,
            imported_records: Some(progress.imported_records),
            duplicate_records: Some(progress.duplicate_records),
            skipped_records: Some(progress.skipped_records),
            log_events: Vec::new(),
        }
        .with_log_event("info", "backup.ingest-profile.records"),
    );
}
