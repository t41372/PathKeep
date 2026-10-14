//! Responsibilities: adapt native bootstrap, rekey, backup and restore to the desktop session.
//! Not responsible for: conversion internals, encryption primitives or Tauri event registration.
//! Failure modes: legacy detection before journal recovery, stale credentials or lost maintenance
//! progress.
//!
//! Worker-bridge helpers for archive, backup, export, and repair flows.

use crate::command_error::CommandError;
use crate::session::{SessionState, session_key, update_session_key};
use vault_core::{AppConfig, ExportRequest, HistoryQuery};
use vault_worker::RekeyRequest;

use super::worker_result;

/// Initializes the archive (forwarding first-run upgrade progress to the UI
/// callback) and synchronizes the session key with the chosen key.
///
/// The command layer wraps `report_progress` around
/// `AppHandle::emit("pathkeep://archive-upgrade")` so a large first launch drives
/// the "Upgrading your archive…" screen; the dev bridge and tests pass a no-op
/// reporter (they cannot deliver Tauri events), and the upgrade work still runs.
pub(crate) fn initialize_archive_with_progress_impl(
    mut config: AppConfig,
    database_key: Option<String>,
    state: &SessionState,
    mut report_progress: impl FnMut(vault_core::ArchiveUpgradeProgress),
) -> Result<vault_core::AppSnapshot, CommandError> {
    let paths = worker_result(vault_core::project_paths())?;
    if state.recover_pending_archive(&paths)? {
        config = worker_result(vault_core::load_config(&paths))?;
    }
    if paths.app_root.join("archive/history-vault.db").exists() {
        let stored = worker_result(vault_core::load_config(&paths))?;
        if stored.archive_mode != config.archive_mode {
            return Err(
                "encryption-mode-change-requires-rekey: use rekey_archive to change encryption"
                    .to_owned()
                    .into(),
            );
        }
        update_session_key(state, database_key)?;
        state.background_vault()?;
        report_progress(vault_core::ArchiveUpgradeProgress::finished());
        return super::app_snapshot_impl(state);
    }
    if paths.archive_database_path.exists()
        && !paths.app_root.join("archive/history-vault.db").exists()
    {
        return worker_result(vault_worker::convert_archive_with_progress(
            state,
            &config,
            database_key.as_deref(),
            report_progress,
        ));
    }
    worker_result(vault_worker::initialize_archive_database_with_progress(
        state,
        &config,
        database_key.as_deref(),
        &mut report_progress,
    ))?;
    super::app_snapshot_impl(state)
}

/// Runs the cheap first-run upgrade pre-check the shell calls at bootstrap to
/// decide whether to show the upgrade screen at all.
pub(crate) fn assess_archive_upgrade_impl(
    state: &SessionState,
    session_database_key: Option<&str>,
) -> Result<vault_core::ArchiveUpgradeAssessment, CommandError> {
    let paths = worker_result(vault_core::project_paths())?;
    state.recover_pending_archive(&paths)?;
    worker_result(vault_worker::assess_archive_upgrade(session_database_key))
}

/// Rekeys the archive and updates the cached session key to the new key.
///
/// The session key is deliberately not passed down: the worker opens the
/// archive with the request's current password once it has proved that
/// password opens the file, so holding an unlocked session is not enough to
/// change or remove the archive's encryption.
pub(crate) fn rekey_archive_impl(
    request: RekeyRequest,
    state: &SessionState,
) -> Result<vault_core::AppSnapshot, CommandError> {
    worker_result(vault_worker::rekey_archive_database(state, &request))
}

/// Self-heals a drifted encryption-at-rest state (e.g. an encrypted config left
/// over a plaintext source-evidence by an archive-only rekey) using the unlocked
/// session key. Safe no-op when already consistent.
pub(crate) fn reconcile_archive_encryption_impl(
    state: &SessionState,
) -> Result<vault_core::ReconcileReport, CommandError> {
    worker_result(vault_worker::reconcile_archive_encryption(&state.vault()?))
}

/// Previews the impact of an archive rekey/mode switch, refusing a wrong
/// current password the same way the rekey would.
pub(crate) fn preview_rekey_archive_impl(
    request: RekeyRequest,
) -> Result<vault_core::RekeyPreview, CommandError> {
    worker_result(vault_worker::preview_rekey_archive(&request))
}

#[cfg_attr(test, allow(dead_code))]
/// Previews a checkpoint restore against the current archive state.
pub(crate) fn preview_snapshot_restore_impl(
    request: vault_core::SnapshotRestoreRequest,
    state: &SessionState,
) -> Result<vault_core::SnapshotRestorePreview, CommandError> {
    worker_result(vault_worker::preview_snapshot_restore_plan(&state.vault()?, &request))
}

#[cfg_attr(test, allow(dead_code))]
/// Executes a checkpoint restore and returns the resulting backup-style report.
pub(crate) fn run_snapshot_restore_impl(
    request: vault_core::SnapshotRestoreRequest,
    state: &SessionState,
) -> Result<vault_core::BackupReport, CommandError> {
    worker_result(vault_worker::run_snapshot_restore_plan(&state.vault()?, &request))
}

#[cfg_attr(test, allow(dead_code))]
/// Lists verified full-archive safety snapshots for the recovery GUI (keyless FS scan).
pub(crate) fn list_recovery_snapshots_impl()
-> Result<Vec<vault_core::RecoverySnapshot>, CommandError> {
    worker_result(vault_worker::list_recovery_snapshots())
}

#[cfg_attr(test, allow(dead_code))]
/// Runs the one-click full-archive restore from a verified safety snapshot.
///
/// Prefers an explicit user-entered archive key (the recovery/unlock escape hatch holds no
/// ambient session key), falling back to the session key when none is supplied. The key is
/// never logged or echoed back in the report.
pub(crate) fn run_full_archive_restore_impl(
    request: vault_core::SnapshotRestoreRequest,
    key: Option<String>,
    state: &SessionState,
) -> Result<vault_core::FullArchiveRestoreReport, CommandError> {
    let effective_key = key.or(session_key(state)?);
    worker_result(vault_worker::run_full_archive_restore(state, effective_key.as_deref(), &request))
}

#[cfg_attr(test, allow(dead_code))]
/// Previews the current retention-prune plan.
pub(crate) fn preview_retention_prune_impl(
    state: &SessionState,
) -> Result<vault_core::RetentionPreview, CommandError> {
    worker_result(vault_worker::preview_retention_plan(state.current_vault().as_ref()))
}

#[cfg_attr(test, allow(dead_code))]
/// Executes the selected retention-prune plan.
pub(crate) fn run_retention_prune_impl(
    request: vault_core::RetentionPruneRequest,
    state: &SessionState,
) -> Result<vault_core::RetentionPruneResult, CommandError> {
    worker_result(vault_worker::run_retention_plan(&state.vault()?, &request))
}

/// Runs a backup immediately and forwards progress to the UI callback.
pub(crate) fn run_backup_now_impl(
    due_only: bool,
    state: &crate::session::SessionState,
    report_progress: impl FnMut(vault_core::BackupProgressEvent),
) -> Result<vault_core::BackupReport, CommandError> {
    let vault = state.vault()?;
    let key = state.typed_password();
    let session_database_key = key.as_deref();
    worker_result(vault_worker::run_backup_now_with_progress(
        &vault,
        session_database_key,
        due_only,
        report_progress,
    ))
}

/// Executes one history query against visible archive facts.
pub(crate) fn query_history_impl(
    query: HistoryQuery,
    state: &SessionState,
) -> Result<vault_core::HistoryQueryResponse, CommandError> {
    let key = if query.q.as_ref().is_some_and(|q| !q.trim().is_empty()) {
        state.get_key()?
    } else {
        None
    };
    worker_result(vault_worker::query_history(&state.vault()?, key.as_deref(), query))
}

#[cfg_attr(test, allow(dead_code))]
/// Loads the icons of the sites a screen lists.
pub(crate) fn load_site_favicons_impl(
    domains: Vec<String>,
    state: &SessionState,
) -> Result<Vec<vault_core::SiteFavicon>, CommandError> {
    worker_result(vault_worker::load_site_favicons(&state.vault()?, domains))
}

#[cfg_attr(test, allow(dead_code))]
/// Loads favicon payloads for already-visible Explorer rows after the primary
/// history query has painted.
pub(crate) fn load_history_favicons_impl(
    entries: Vec<vault_core::HistoryFaviconLookupEntry>,
    state: &SessionState,
) -> Result<Vec<vault_core::HistoryFaviconLookupResult>, CommandError> {
    worker_result(vault_worker::load_history_favicons(&state.vault()?, entries))
}

#[cfg_attr(test, allow(dead_code))]
/// Loads cached og:image payloads for already-visible card-mode rows.
pub(crate) fn load_history_og_images_impl(
    entries: Vec<vault_core::HistoryOgImageLookupEntry>,
    state: &crate::session::SessionState,
) -> Result<Vec<vault_core::HistoryOgImageLookupResult>, CommandError> {
    worker_result(vault_worker::load_history_og_images(&state.vault()?, entries))
}

#[cfg_attr(test, allow(dead_code))]
/// Bumps `last_shown_at` for the supplied page URLs (LRU eviction signal).
pub(crate) fn mark_og_images_shown_impl(
    urls: Vec<String>,
    state: &crate::session::SessionState,
) -> Result<(), CommandError> {
    worker_result(vault_worker::mark_og_images_shown(&state.vault()?, urls))
}

#[cfg_attr(test, allow(dead_code))]
/// Fetches og:image previews for the given URLs and persists each outcome.
///
/// This is the IPC entrypoint hit by the implicit on-demand path (the
/// `useExplorerOgImages` hook). It must short-circuit when the user has
/// chosen `OgImageFetchMode::Off` so the data-sovereignty promise on the
/// Settings → Link previews copy ("Off — No fetching anywhere.") holds
/// regardless of what the frontend does.
pub(crate) fn refetch_og_images_impl(
    urls: Vec<String>,
    state: &crate::session::SessionState,
) -> Result<u32, CommandError> {
    match vault_worker::effective_og_image_fetch_mode() {
        Ok(vault_core::OgImageFetchMode::Off) => return Ok(0),
        Ok(_) => {}
        Err(error) => return Err(CommandError::classified(format!("{error:#}"))),
    }
    worker_result(vault_worker::refetch_og_images(&state.vault()?, urls))
}

#[cfg_attr(test, allow(dead_code))]
/// Reports the current og:image cache footprint to Settings → Storage.
pub(crate) fn og_image_storage_stats_impl(
    state: &crate::session::SessionState,
) -> Result<vault_core::OgImageStorageStats, CommandError> {
    worker_result(vault_worker::og_image_storage_stats(&state.vault()?))
}

#[cfg_attr(test, allow(dead_code))]
/// Reports og:image coverage (share of web pages with a preview) to Settings.
pub(crate) fn og_image_coverage_stats_impl(
    state: &crate::session::SessionState,
) -> Result<vault_core::OgImageCoverageStats, CommandError> {
    worker_result(vault_worker::og_image_coverage_stats(&state.vault()?))
}

#[cfg_attr(test, allow(dead_code))]
/// Empties both og:image cache tables (behind the Settings confirm dialog).
pub(crate) fn clear_og_image_cache_impl(
    state: &crate::session::SessionState,
) -> Result<vault_core::OgImageCleanupReport, CommandError> {
    worker_result(vault_worker::clear_og_image_cache(&state.vault()?))
}

#[cfg_attr(test, allow(dead_code))]
/// Runs one eviction pass using the user's configured cleanup mode.
pub(crate) fn run_og_image_cleanup_impl(
    state: &crate::session::SessionState,
) -> Result<vault_core::OgImageCleanupReport, CommandError> {
    worker_result(vault_worker::run_og_image_cleanup(&state.vault()?))
}

#[cfg_attr(test, allow(dead_code))]
/// Loads the dashboard snapshot for the current archive.
pub(crate) fn dashboard_snapshot_impl(
    state: &SessionState,
) -> Result<vault_core::DashboardSnapshot, CommandError> {
    worker_result(vault_worker::dashboard_snapshot(&state.vault()?))
}

#[cfg_attr(test, allow(dead_code))]
/// Loads per-source visit counts and date spans for Home and History.
pub(crate) fn source_stats_impl(
    state: &SessionState,
) -> Result<Vec<vault_core::SourceStats>, CommandError> {
    worker_result(vault_worker::source_stats(&state.vault()?))
}

#[cfg_attr(test, allow(dead_code))]
/// Loads the History detail panel data for one exact URL.
pub(crate) fn url_detail_impl(
    url: &str,
    state: &SessionState,
) -> Result<vault_core::UrlDetail, CommandError> {
    worker_result(vault_worker::url_detail(&state.vault()?, url))
}

#[cfg_attr(test, allow(dead_code))]
/// Loads the full audit detail for one run ID.
pub(crate) fn audit_run_detail_impl(
    run_id: i64,
    state: &SessionState,
) -> Result<vault_core::AuditRunDetail, CommandError> {
    worker_result(vault_worker::audit_run_detail(&state.vault()?, run_id))
}

/// Exports history to the file the person chooses in the system's save dialog; `None` when they
/// close it without choosing. The interface sends the format, never a path. Blocks on the dialog:
/// call it off the main thread.
pub(crate) fn export_history_impl<R: tauri::Runtime>(
    app: Option<&tauri::AppHandle<R>>,
    request: ExportRequest,
    state: &SessionState,
) -> Result<Option<vault_core::ExportResult>, CommandError> {
    let vault = match state.vault() {
        Ok(vault) => vault,
        Err(error) => {
            let error = anyhow::Error::msg(error);
            vault_core::record_export_failure(request.export_id.as_deref(), &error);
            return worker_result(Err(error));
        }
    };
    let name = vault_core::suggested_export_name(&request.format);
    let kind = match request.format {
        vault_core::ExportFormat::Html => "HTML",
        vault_core::ExportFormat::Markdown => "Markdown",
        vault_core::ExportFormat::Text => "Text",
        vault_core::ExportFormat::Jsonl => "JSON Lines",
    };
    let Some(target) = super::save_path(app, &name, (kind, &[request.format.extension()]))? else {
        return Ok(None);
    };
    let result = vault_worker::export_query(&vault, request, &target);
    if let Err(error) = &result {
        if error.downcast_ref::<vault_core::ExportCancelled>().is_some() {
            return Err(CommandError {
                message: format!("{error:#}"),
                code: Some("export-cancelled".to_string()),
                action_hint: None,
                retry_hint: None,
            });
        }
    }
    worker_result(result).map(Some)
}

/// Polls process-local export state without archive I/O.
#[cfg_attr(all(test, not(feature = "devtools-bridge")), allow(dead_code))]
pub(crate) fn get_export_progress_impl(export_id: &str) -> Option<vault_core::ExportProgress> {
    vault_core::get_export_progress(export_id)
}

/// Requests cancellation through the same registry the in-process worker uses.
#[cfg_attr(all(test, not(feature = "devtools-bridge")), allow(dead_code))]
pub(crate) fn cancel_export_impl(export_id: &str) -> bool {
    vault_core::cancel_export(export_id)
}

/// Runs the archive doctor read path through the worker.
pub(crate) fn doctor_report_impl(
    state: &SessionState,
) -> Result<vault_core::HealthReport, CommandError> {
    worker_result(vault_worker::doctor_report(&state.vault()?))
}

#[cfg_attr(test, allow(dead_code))]
/// Applies conservative repair steps for doctor-detected health issues.
pub(crate) fn repair_health_impl(
    state: &crate::session::SessionState,
) -> Result<vault_core::HealthRepairReport, CommandError> {
    worker_result(vault_worker::repair_health(&state.vault()?))
}

/// Continues the run ledger beyond the snapshot's first page.
pub(crate) fn load_recent_runs_impl(
    cursor: Option<i64>,
    limit: Option<usize>,
    state: &SessionState,
) -> Result<vault_core::RunPage, CommandError> {
    worker_result(vault_core::load_recent_runs(&state.vault()?, cursor, limit.unwrap_or(12)))
}

/// What outside apps asked through the MCP server, a page at a time (Settings → AI).
pub(crate) fn load_mcp_queries_impl(
    cursor: Option<i64>,
    limit: Option<usize>,
    state: &SessionState,
) -> Result<vault_core::McpQueryPage, CommandError> {
    worker_result(vault_core::load_mcp_queries(&state.vault()?, cursor, limit.unwrap_or(50)))
}

/// Continues import history beyond the snapshot's first page.
pub(crate) fn load_import_batches_impl(
    cursor: Option<String>,
    limit: Option<usize>,
    state: &SessionState,
) -> Result<vault_core::ImportBatchPage, CommandError> {
    worker_result(vault_core::load_import_batches(&state.vault()?, cursor, limit.unwrap_or(16)))
}
