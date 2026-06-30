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

/// Tags a browser-profile access failure as a Full Disk Access problem when the root cause is an OS
/// permission denial (macOS TCC / `EACCES`/`EPERM`), so the recorded reason and the UI can guide the
/// user to grant access instead of surfacing a bare "Operation not permitted". Non-permission errors
/// pass through unchanged. The "Full Disk Access" marker is the stable signal the front end keys on
/// to render its localized, actionable guidance.
pub(super) fn classify_browser_access_error(error: anyhow::Error) -> anyhow::Error {
    let permission_denied = error.chain().any(|cause| {
        cause
            .downcast_ref::<std::io::Error>()
            .is_some_and(|io| io.kind() == std::io::ErrorKind::PermissionDenied)
    }) || format!("{error:#}").contains("Operation not permitted");
    if permission_denied {
        error.context(
            "PathKeep needs Full Disk Access to read your browser history. Grant it in System Settings \u{2192} Privacy & Security \u{2192} Full Disk Access, then run the backup again.",
        )
    } else {
        error
    }
}

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

    // WRITE SERIALIZATION (data-integrity audit CRITICAL-1 / CRITICAL-5): hold BOTH
    // serialization layers for the WHOLE backup. The in-process top-level
    // [`ArchiveOpGate`] (taken FIRST) excludes a SECOND destructive op dispatched in THIS
    // process — e.g. the user toggling encryption (rekey) / triggering an import from
    // Settings while this run's canonical write transaction is open — which the reentrant
    // flock alone cannot exclude (CRIT-5's same-process trigger). The cross-process
    // [`ArchiveWriteLock`] excludes the OUT-OF-PROCESS scheduled backup. Either way a
    // foreign `fs::rename` of `history-vault.sqlite` out from under the open transaction
    // (committing into a now-unlinked inode, reporting success, silently losing the rows,
    // and orphaning a `-wal` that bricks the next open) is prevented. A manual run MUST
    // complete, so it BLOCKS until both layers free; a scheduled/automatic (`due_only`)
    // run DEFERS if EITHER layer is held — it steps aside this tick and the next due
    // window retries, surfaced through the existing `due_skipped` channel (no failure
    // toast, no banner). The flock is process-reentrant, so a backup that later calls
    // reconcile never self-deadlocks; the gate is taken ONLY here at the top level. The
    // guards are held (lock released first, then gate) until this function returns.
    let _write_lock = match acquire_backup_write_lock(paths, due_only)? {
        Some(guard) => guard,
        None => {
            return Ok(BackupReport {
                due_skipped: true,
                reason: Some(BACKUP_DEFERRED_FOR_WRITE_LOCK.to_string()),
                ..BackupReport::default()
            });
        }
    };

    // Heal a whole-app import whose commit phase was cut by a crash BEFORE this backup
    // opens the archive (data-integrity audit, HIGH residual). A crashed GUI import
    // frees its `flock` when the process dies; the SEPARATE scheduled-backup process can
    // then win the lock and would otherwise open — and record SUCCESS for — a silently
    // half-applied (e.g. new history-vault + old source-evidence) archive, never hitting
    // the unlock-path reconcile. Recovering here reverts to the consistent pre-import
    // state first. The acquire inside is process-reentrant (we already hold the lock) and
    // gated on a cheap marker `stat`, so this is a no-op when no import was interrupted.
    // Placed AFTER the lock acquire so a scheduled run still DEFERS (above) rather than
    // blocking when another process holds the lock mid-import.
    crate::migration::recover_interrupted_import(paths)?;

    let mut connection = super::open_archive_connection(paths, config, key)?;
    // Self-heal a drifted at-rest mode (e.g. an encrypted config left over a
    // plaintext source-evidence by an archive-only rekey) BEFORE opening
    // source-evidence — otherwise `PRAGMA key` on the plaintext file makes
    // SQLCipher decode the plaintext header as ciphertext and abort with
    // SQLITE_NOMEM ("out of memory") on every backup. No-op when consistent.
    super::reconcile_source_evidence_with_archive(&connection, paths, config, key)?;
    let mut source_evidence = super::open_source_evidence_connection(paths, config, key)?;

    if due_only && let Some(reason) = backup_due_skip_reason(&connection, config)? {
        return Ok(BackupReport {
            due_skipped: true,
            reason: Some(reason),
            ..BackupReport::default()
        });
    }

    let started_at = now_rfc3339();
    let timezone = current_timezone_name();
    let trigger = if due_only { "schedule" } else { "manual" };

    // Open the run ledger BEFORE any work that can fail (profile discovery, selection, staging).
    // A backup tool must never fail silently: if PathKeep can't even read the browser profiles
    // (e.g. macOS denies Full Disk Access), there must still be a `failed` run on record with the
    // reason — not a phantom no-op that the user only notices after they've cleared their history.
    // The scope records the user's *intent* (the selected ids), which is the honest scope for a run
    // that failed before it could resolve which of them are actually readable.
    connection.execute(
        "INSERT INTO runs (run_type, trigger, started_at, timezone, status, profile_scope_json, warnings_json, stats_json, due_only)
         VALUES ('backup', ?1, ?2, ?3, 'running', ?4, '[]', '{}', ?5)",
        params![
            trigger,
            started_at,
            timezone,
            serde_json::to_string(&config.selected_profile_ids)?,
            due_only as i64,
        ],
    )?;
    let run_id = connection.last_insert_rowid();
    let parent_manifest = latest_manifest_row(&connection)?;

    // From here, EVERY failure must finalize this run as `failed` with its reason recorded — never
    // an early return that leaves a phantom `running` row or an invisible no-op. Discovery, profile
    // selection, AND staging all run inside ONE guard whose single error path finalizes the run, so
    // "PathKeep couldn't read your browser history" is always a visible failed run, not silence.
    let mut profile_summaries = Vec::new();
    let mut source_hashes = BTreeMap::<String, BTreeMap<String, String>>::new();
    let mut snapshot_artifacts = Vec::<SnapshotArtifact>::new();
    let mut source_evidence_plans = Vec::new();
    let mut warnings: Vec<String> = Vec::new();

    let backup_result = (|| -> Result<Vec<String>> {
        let discovered = discover_profiles().map_err(classify_browser_access_error)?;
        if config.selected_profile_ids.is_empty() {
            anyhow::bail!("select at least one readable browser profile before running a backup");
        }
        let selected_profiles =
            select_supported_profiles(&discovered, &config.selected_profile_ids);
        if selected_profiles.is_empty() {
            anyhow::bail!(
                "the selected profiles are not readable yet; choose at least one detected profile with a readable history database"
            );
        }
        warnings.extend(collect_skipped_profiles(&discovered, &config.selected_profile_ids));
        let total_profiles = selected_profiles.len();

        report_progress(
            BackupProgressEvent {
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
            }
            .with_log_event("info", "backup.prepare"),
        );

        let transaction = connection.transaction()?;
        for (index, profile) in selected_profiles.iter().enumerate() {
            report_progress(
                BackupProgressEvent {
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
                    progress_percent: Some((index as f32 / total_profiles as f32) * 100.0),
                    log_lines: vec![format!("Staging {}.", profile.profile_id)],
                    source_label: Some(format!(
                        "{} / {}",
                        profile.browser_name, profile.profile_name
                    )),
                    ..BackupProgressEvent::default()
                }
                .with_log_event("info", "backup.stage-profile"),
            );
            let snapshot = match crate::chrome::stage_profile_snapshot(paths, profile) {
                Ok(staged) => {
                    // Record any degraded-staging notes (e.g. an online snapshot
                    // that fell back to a recovered raw copy because the browser
                    // was busy) on the run so the reason is visible, not silent.
                    for warning in &staged.warnings {
                        warnings.push(warning.clone());
                        report_progress(
                            BackupProgressEvent {
                                phase: "stage-profile".to_string(),
                                label: "Recovered a busy database".to_string(),
                                detail: warning.clone(),
                                step: 1,
                                total_steps: 3,
                                completed_profiles: index,
                                total_profiles,
                                profile_id: Some(profile.profile_id.clone()),
                                progress_current: Some(index + 1),
                                progress_total: Some(total_profiles),
                                progress_percent: Some(
                                    (((index + 1) as f32) / total_profiles as f32) * 100.0,
                                ),
                                log_lines: vec![warning.clone()],
                                source_label: Some(format!(
                                    "{} / {}",
                                    profile.browser_name, profile.profile_name
                                )),
                                ..BackupProgressEvent::default()
                            }
                            .with_log_event("warning", "backup.stage-profile.fallback"),
                        );
                    }
                    staged.snapshot
                }
                Err(error) if is_skippable_staging_access_error(profile, &error) => {
                    let warning = staging_access_skip_warning(profile);
                    warnings.push(warning.clone());
                    report_progress(
                        BackupProgressEvent {
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
                            progress_percent: Some(
                                (((index + 1) as f32) / total_profiles as f32) * 100.0,
                            ),
                            log_lines: vec![warning],
                            source_label: Some(format!(
                                "{} / {}",
                                profile.browser_name, profile.profile_name
                            )),
                            ..BackupProgressEvent::default()
                        }
                        .with_log_event("warning", "backup.stage-profile.skip"),
                    );
                    continue;
                }
                Err(error) => {
                    return Err(error)
                        .with_context(|| format!("staging profile {}", profile.profile_id));
                }
            };
            report_progress(
                BackupProgressEvent {
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
                    progress_percent: Some((((index + 1) as f32) / total_profiles as f32) * 100.0),
                    log_lines: vec![format!("Writing canonical facts for {}.", profile.profile_id)],
                    source_label: Some(format!(
                        "{} / {}",
                        profile.browser_name, profile.profile_name
                    )),
                    ..BackupProgressEvent::default()
                }
                .with_log_event("info", "backup.ingest-profile"),
            );
            let mut last_processed_records = 0usize;
            let report_profile_progress = |progress: ArchiveIngestProgress| {
                emit_backup_ingest_progress_if_changed(
                    &mut report_progress,
                    &mut last_processed_records,
                    index,
                    total_profiles,
                    profile,
                    progress,
                );
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
        report_progress(
            BackupProgressEvent {
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
            }
            .with_log_event("info", "backup.finalize"),
        );
        // Crash window: every staged profile's rows are in the OPEN transaction but not
        // yet committed. A kill/power-loss HERE must roll the transaction back so the
        // on-disk archive stays at its pre-backup state — never a torn half-write. (No-op
        // in production; armed only by the crash-window regression tests.)
        crate::fault_inject::checkpoint("backup.before_canonical_commit")?;
        transaction.commit()?;
        Ok(selected_profiles.iter().map(|profile| profile.profile_id.clone()).collect())
    })();

    let selected_profile_ids: Vec<String> = match backup_result {
        Ok(selected_profile_ids) => selected_profile_ids,
        Err(error) => {
            finalize_failed_run(&connection, run_id, &profile_summaries, &warnings, &error)?;
            return Err(error);
        }
    };

    // Crash window: the canonical history-vault rows are COMMITTED and durable, but
    // source-evidence, the manifest, and the run finalize have not run yet. A
    // kill/power-loss HERE must leave the archive fully consistent at the
    // newly-committed state — the canonical facts are never torn; the follow-up
    // artifacts are recoverable on a subsequent run. (No-op in production.)
    crate::fault_inject::checkpoint("backup.after_canonical_commit")?;

    warnings.extend(
        persist_source_evidence_plans(&mut source_evidence, &connection, &source_evidence_plans)
            .err()
            .map(source_evidence_rebuild_warning),
    );

    let finished_at = now_rfc3339();
    let summary = backup_run_summary(
        "backup",
        run_id,
        &started_at,
        &finished_at,
        trigger,
        &selected_profile_ids,
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
    let git_commit = if config.git_enabled {
        let (git_commit, git_warning) =
            git_audit::commit_all_optional(&paths.audit_repo_path, &format!("backup run {run_id}"));
        warnings.extend(git_warning);
        git_commit
    } else {
        None
    };
    finalize_successful_run(
        &connection,
        run_id,
        &finished_at,
        &summary,
        &warnings,
        &manifest_hash,
    )?;

    warnings.extend(
        super::rebuild_search_projection(paths, config, key)
            .err()
            .map(keyword_recall_rebuild_warning),
    );

    Ok(BackupReport {
        due_skipped: false,
        reason: None,
        run: Some(BackupRunOverview { manifest_hash: Some(manifest_hash), ..summary }),
        profiles: profile_summaries,
        manifest_path: Some(manifest_path.display().to_string()),
        git_commit,
        warnings,
    })
}

/// Reason recorded when a scheduled (`due_only`) backup steps aside because another
/// process already holds the archive write lock (e.g. a foreground rekey or import).
///
/// Surfaced through the normal `due_skipped` channel so the deferral is visible for PME
/// transparency without raising a failure toast — a deferred tick is a no-op that the
/// next due window retries, not a failed run.
const BACKUP_DEFERRED_FOR_WRITE_LOCK: &str = "Another archive operation is in progress; deferring this scheduled backup until the next due window.";

/// Acquires BOTH write-serialization layers for the duration of a backup run: the
/// in-process top-level [`super::ArchiveOpGate`] (taken FIRST) and the cross-process
/// [`super::ArchiveWriteLock`].
///
/// A manual run (`due_only == false`) MUST execute, so it BLOCKS until both layers free.
/// A scheduled run (`due_only == true`) instead tries once and DEFERS (`Ok(None)`) when
/// EITHER layer is held — a foreground top-level op in THIS process (gate) or a
/// destructive op in ANOTHER OS process (flock) — so the caller can skip-with-reason
/// instead of racing the rekey/import swap. The flock is process-reentrant, so this never
/// self-deadlocks against a nested archive helper running in THIS process; the gate is
/// non-reentrant and taken ONLY here (no nested backup helper re-takes it).
///
/// The guards are returned as `(lock, gate)` so they RELEASE in that order — the reverse
/// of acquisition — letting a parked same-process waiter wake to a gate it can take with
/// the flock already free.
fn acquire_backup_write_lock(
    paths: &ProjectPaths,
    due_only: bool,
) -> Result<Option<(super::ArchiveWriteLock, super::ArchiveOpGate)>> {
    if due_only {
        // Probe the in-process gate first; if a foreground top-level op holds it, defer
        // WITHOUT touching the flock. The gate guard drops on the early return, releasing
        // it for that in-flight op.
        let Some(gate) = super::ArchiveOpGate::try_acquire(paths) else {
            return Ok(None);
        };
        match super::ArchiveWriteLock::try_acquire(paths)? {
            Some(lock) => Ok(Some((lock, gate))),
            None => Ok(None),
        }
    } else {
        let gate = super::ArchiveOpGate::acquire(paths);
        let lock = super::ArchiveWriteLock::acquire(paths)?;
        Ok(Some((lock, gate)))
    }
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

pub(super) fn source_evidence_rebuild_warning(error: anyhow::Error) -> String {
    format!("Canonical backup completed, but the source-evidence archive needs a rebuild: {error}")
}

pub(super) fn keyword_recall_rebuild_warning(error: anyhow::Error) -> String {
    format!(
        "Canonical backup completed, but the keyword-recall projection needs a rebuild: {error}"
    )
}
