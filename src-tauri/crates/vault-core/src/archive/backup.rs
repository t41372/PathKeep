//! Archive backup execution.
//!
//! ## Responsibilities
//! - Orchestrate one canonical backup run across the selected readable browser
//!   profiles.
//! - Keep progress reporting, run-ledger updates, manifest writing, and
//!   source-evidence follow-up in one place.
//! - Preserve the accepted manual-vs-scheduled backup semantics without
//!   leaking backup-specific control flow back into `archive/mod.rs`.
//!
//! ## Not responsible for
//! - Parser-side row extraction or watermark persistence details.
//! - Read-model queries for Dashboard, Explorer, or Audit.
//! - Retention, restore, rekey, or doctor flows.
//!
//! ## Dependencies
//! - `archive::ingest` for staged profile processing and deferred
//!   source-evidence plans.
//! - `archive::run_support` for run-ledger, manifest, and due-window helpers.
//! - `crate::chrome` for live browser profile discovery before staging starts.
//!
//! ## Performance notes
//! - This is on the hottest local backup path. It stages and processes one
//!   profile at a time so large archives do not require loading every selected
//!   profile into memory at once.
//! - Canonical row writes stay inside a single transaction per run to avoid
//!   partial-commit drift between the archive and the run ledger.

use super::{
    SnapshotArtifact,
    ingest::{
        ArchiveIngestProgress, collect_skipped_profiles, persist_source_evidence_plans,
        process_profile_snapshot_with_progress, select_supported_profiles, snapshot_source_hashes,
    },
    run_support::{
        BackupManifest, archive_row_counts, backup_due_skip_reason, backup_run_summary,
        current_timezone_name, finalize_failed_run, finalize_successful_run, latest_manifest_row,
        persist_manifest_row, write_manifest_artifact,
    },
};
use crate::{
    chrome::discover_profiles,
    config::{ProjectPaths, ensure_paths},
    git_audit,
    models::{AppConfig, BackupProgressEvent, BackupReport, BackupRunOverview, BrowserProfile},
    utils::{now_rfc3339, sha256_hex},
};
use anyhow::{Context, Result};
use rusqlite::params;
use std::collections::BTreeMap;

/// Runs one backup without exposing the internal progress stream.
///
/// The app uses this wrapper when it only needs the final report and wants the
/// backup path to behave exactly like the progress-aware variant.
pub fn run_backup(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    due_only: bool,
) -> Result<BackupReport> {
    run_backup_with_progress(paths, config, key, due_only, |_| {})
}

/// Runs one canonical backup and emits progress events as each phase advances.
///
/// `due_only` preserves the scheduler contract: PathKeep may decline to run if
/// the most recent successful backup is still fresh enough. The callback sees
/// phase-local progress only; the returned report remains the single source of
/// truth for persisted run state, warnings, and manifest paths.
pub fn run_backup_with_progress<F>(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    due_only: bool,
    mut report_progress: F,
) -> Result<BackupReport>
where
    F: FnMut(BackupProgressEvent),
{
    ensure_paths(paths)?;
    if !config.initialized {
        anyhow::bail!("archive has not been initialized");
    }

    let mut connection = super::open_archive_connection(paths, config, key)?;
    let mut source_evidence = super::open_source_evidence_connection(paths, config, key)?;

    if due_only && let Some(reason) = backup_due_skip_reason(&connection, config)? {
        return Ok(BackupReport {
            due_skipped: true,
            reason: Some(reason),
            ..BackupReport::default()
        });
    }

    let discovered = discover_profiles()?;
    if config.selected_profile_ids.is_empty() {
        anyhow::bail!("select at least one readable browser profile before running a backup")
    }
    let selected_profiles = select_supported_profiles(&discovered, &config.selected_profile_ids);
    if selected_profiles.is_empty() {
        anyhow::bail!(
            "the selected profiles are not readable yet; choose at least one detected profile with a readable history database"
        )
    }
    let skipped_profiles = collect_skipped_profiles(&discovered, &config.selected_profile_ids);
    let started_at = now_rfc3339();
    let timezone = current_timezone_name();
    let trigger = if due_only { "schedule" } else { "manual" };
    let total_profiles = selected_profiles.len();

    report_progress(BackupProgressEvent {
        phase: "prepare".to_string(),
        label: "Inspect selected browser profiles".to_string(),
        detail: format!(
            "Queued {total_profiles} readable profile(s) for the canonical backup run."
        ),
        step: 0,
        total_steps: 3,
        completed_profiles: 0,
        total_profiles,
        profile_id: None,
        progress_current: Some(0),
        progress_total: Some(total_profiles),
        progress_percent: Some(0.0),
        log_lines: vec![format!("Queued {total_profiles} readable profile(s) for backup.")],
        ..BackupProgressEvent::default()
    });

    connection.execute(
        "INSERT INTO runs (run_type, trigger, started_at, timezone, status, profile_scope_json, warnings_json, stats_json, due_only)
         VALUES ('backup', ?1, ?2, ?3, 'running', ?4, '[]', '{}', ?5)",
        params![
            trigger,
            started_at,
            timezone,
            serde_json::to_string(
                &selected_profiles
                    .iter()
                    .map(|profile| profile.profile_id.clone())
                    .collect::<Vec<_>>()
            )?,
            due_only as i64,
        ],
    )?;
    let run_id = connection.last_insert_rowid();
    let parent_manifest = latest_manifest_row(&connection)?;

    let mut profile_summaries = Vec::new();
    let mut source_hashes = BTreeMap::<String, BTreeMap<String, String>>::new();
    let mut snapshot_artifacts = Vec::<SnapshotArtifact>::new();
    let mut source_evidence_plans = Vec::new();
    let mut warnings = skipped_profiles;

    let backup_result = (|| -> Result<()> {
        let transaction = connection.transaction()?;
        for (index, profile) in selected_profiles.iter().enumerate() {
            report_progress(BackupProgressEvent {
                phase: "stage-profile".to_string(),
                label: "Stage source profile".to_string(),
                detail: format!(
                    "Copying {} into the staging area ({}/{total_profiles}).",
                    profile.profile_id,
                    index + 1,
                ),
                step: 1,
                total_steps: 3,
                completed_profiles: index,
                total_profiles,
                profile_id: Some(profile.profile_id.clone()),
                progress_current: Some(index),
                progress_total: Some(total_profiles),
                progress_percent: if total_profiles == 0 {
                    None
                } else {
                    Some((index as f32 / total_profiles as f32) * 100.0)
                },
                log_lines: vec![format!("Staging {}.", profile.profile_id)],
                source_label: Some(format!("{} / {}", profile.browser_name, profile.profile_name)),
                ..BackupProgressEvent::default()
            });
            let snapshot = match crate::chrome::stage_profile_snapshot(paths, profile) {
                Ok(snapshot) => snapshot,
                Err(error) if is_skippable_staging_access_error(profile, &error) => {
                    let warning = staging_access_skip_warning(profile);
                    warnings.push(warning.clone());
                    report_progress(BackupProgressEvent {
                        phase: "stage-profile".to_string(),
                        label: "Skip unreadable profile".to_string(),
                        detail: warning.clone(),
                        step: 1,
                        total_steps: 3,
                        completed_profiles: index + 1,
                        total_profiles,
                        profile_id: Some(profile.profile_id.clone()),
                        progress_current: Some(index + 1),
                        progress_total: Some(total_profiles),
                        progress_percent: if total_profiles == 0 {
                            None
                        } else {
                            Some((((index + 1) as f32) / total_profiles as f32) * 100.0)
                        },
                        log_lines: vec![warning],
                        source_label: Some(format!(
                            "{} / {}",
                            profile.browser_name, profile.profile_name
                        )),
                        ..BackupProgressEvent::default()
                    });
                    continue;
                }
                Err(error) => {
                    return Err(error)
                        .with_context(|| format!("staging profile {}", profile.profile_id));
                }
            };
            report_progress(BackupProgressEvent {
                phase: "ingest-profile".to_string(),
                label: "Write canonical archive facts".to_string(),
                detail: format!(
                    "Processing {} and writing archive rows ({}/{total_profiles}).",
                    profile.profile_id,
                    index + 1,
                ),
                step: 1,
                total_steps: 3,
                completed_profiles: index,
                total_profiles,
                profile_id: Some(profile.profile_id.clone()),
                progress_current: Some(index + 1),
                progress_total: Some(total_profiles),
                progress_percent: if total_profiles == 0 {
                    None
                } else {
                    Some((((index + 1) as f32) / total_profiles as f32) * 100.0)
                },
                log_lines: vec![format!("Writing canonical facts for {}.", profile.profile_id)],
                source_label: Some(format!("{} / {}", profile.browser_name, profile.profile_name)),
                ..BackupProgressEvent::default()
            });
            let mut last_processed_records = 0usize;
            let report_profile_progress = |progress: ArchiveIngestProgress| {
                if progress.processed_records == last_processed_records {
                    return;
                }
                last_processed_records = progress.processed_records;
                report_progress(BackupProgressEvent {
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
                    log_lines: vec![format!(
                        "{} ({}/{total_profiles})",
                        profile.profile_id,
                        index + 1
                    )],
                    source_label: Some(format!(
                        "{} / {}",
                        profile.browser_name, profile.profile_name
                    )),
                    processed_records: Some(progress.processed_records),
                    total_records: None,
                    imported_records: Some(progress.imported_records),
                    duplicate_records: Some(progress.duplicate_records),
                    skipped_records: Some(progress.skipped_records),
                });
            };
            let profile_summary = process_profile_snapshot_with_progress(
                &transaction,
                run_id,
                paths,
                config,
                &snapshot,
                &mut snapshot_artifacts,
                &mut source_evidence_plans,
                true,
                true,
                Some(Box::new(report_profile_progress)),
            )
            .with_context(|| format!("processing profile {}", profile.profile_id))?;
            source_hashes.insert(profile.profile_id.clone(), snapshot_source_hashes(&snapshot));
            warnings.extend(profile_summary.notes.clone());
            profile_summaries.push(profile_summary);
        }
        report_progress(BackupProgressEvent {
            phase: "finalize".to_string(),
            label: "Finalize manifest and cached totals".to_string(),
            detail: format!(
                "Committing run artifacts after {total_profiles} processed profile(s)."
            ),
            step: 2,
            total_steps: 3,
            completed_profiles: total_profiles,
            total_profiles,
            profile_id: None,
            progress_current: Some(total_profiles),
            progress_total: Some(total_profiles),
            progress_percent: Some(100.0),
            log_lines: vec!["Refreshing run ledger and cached summaries.".to_string()],
            ..BackupProgressEvent::default()
        });
        transaction.commit()?;
        Ok(())
    })();

    if let Err(error) = backup_result {
        finalize_failed_run(&connection, run_id, &profile_summaries, &warnings, &error)?;
        return Err(error);
    }

    if let Err(error) =
        persist_source_evidence_plans(&mut source_evidence, &connection, &source_evidence_plans)
    {
        warnings.push(format!(
            "Canonical backup completed, but the source-evidence archive needs a rebuild: {error}"
        ));
    }

    let finished_at = now_rfc3339();
    let summary = backup_run_summary(
        "backup",
        run_id,
        &started_at,
        &finished_at,
        trigger,
        &selected_profiles.iter().map(|profile| profile.profile_id.clone()).collect::<Vec<_>>(),
        &profile_summaries,
    );
    let row_counts = archive_row_counts(&connection)?;
    let manifest = BackupManifest {
        created_at: finished_at.clone(),
        run_id,
        timezone: timezone.clone(),
        due_only,
        database_path: paths.archive_database_path.display().to_string(),
        summary: summary.clone(),
        profiles: profile_summaries.clone(),
        warnings: warnings.clone(),
        source_hashes,
        snapshots: snapshot_artifacts,
        row_counts: row_counts.clone(),
        parent_manifest_hash: parent_manifest.as_ref().map(|row| row.hash.clone()),
    };
    let manifest_json = serde_json::to_string_pretty(&manifest)?;
    let manifest_hash = sha256_hex(manifest_json.as_bytes());
    let manifest_path =
        write_manifest_artifact(paths, run_id, &finished_at, &manifest_hash, &manifest_json)?;
    persist_manifest_row(
        &connection,
        run_id,
        parent_manifest.as_ref(),
        &manifest_hash,
        &manifest_path,
        &finished_at,
        &row_counts,
    )?;
    finalize_successful_run(
        &connection,
        run_id,
        &finished_at,
        &summary,
        &warnings,
        &manifest_hash,
    )?;

    if let Err(error) = super::rebuild_search_projection(paths, config, key) {
        warnings.push(format!(
            "Canonical backup completed, but the keyword-recall projection needs a rebuild: {error}"
        ));
    }

    let git_commit = if config.git_enabled {
        git_audit::ensure_repo(&paths.audit_repo_path)?;
        git_audit::commit_all(&paths.audit_repo_path, &format!("backup run {run_id}"))?
    } else {
        None
    };

    Ok(BackupReport {
        due_skipped: false,
        reason: None,
        run: Some(BackupRunOverview { manifest_hash: Some(manifest_hash), ..summary }),
        profiles: profile_summaries,
        manifest_path: Some(manifest_path.display().to_string()),
        git_commit,
        warnings,
        remote_backup: None,
    })
}

fn is_skippable_staging_access_error(profile: &BrowserProfile, error: &anyhow::Error) -> bool {
    profile.browser_family == "safari"
        && format!("{error:#}").contains("Safari History.db is not readable yet")
}

fn staging_access_skip_warning(profile: &BrowserProfile) -> String {
    format!(
        "Skipped `{}` because Safari History.db is not readable yet. On macOS, grant Full Disk Access before the next backup.",
        profile.profile_id
    )
}
