//! Responsibilities: orchestrate archive reads, backup, native restore and retention through their
//! owners.
//! Not responsible for: database copying, file journals or UI transport.
//! Failure modes: dropped progress, lost restore reports, scheduled backup blocked by App Lock or
//! hidden follow-up failures.
//!
//! Failure modes for scheduled progress: dropping events or changing the App Lock
//! exception. Tests: owner_requests, scheduled backup regressions and owner_process.
//! Archive-facing worker flows.
//!
//! This module owns the worker entrypoints that talk to the canonical archive:
//! backup, query/export, snapshot restore, retention, import/rollback, doctor,
//! and remote backup upload/verify.
//!
//! The worker layer is allowed to chain follow-up actions such as remote backup
//! upload or AI queue work, but it must keep those steps honest. In practice
//! that means archive mutations still finish first, follow-up failures are
//! surfaced as warnings instead of being hidden, and the explicit/manual
//! boundaries described in the design docs remain visible in the returned
//! payloads.

use crate::{
    context::{load_unlocked_config, selected_embedding_provider_runtime},
    intelligence::{maybe_spawn_ai_queue_drain, maybe_spawn_intelligence_queue_drain},
};
use anyhow::{Context, Result};
use vault_core::Vault;
use vault_core::{
    AiIndexRequest, AiQueueJob, BackupProgressEvent, BackupReport, BackupWarning,
    BrowserHistoryImportRequest, ClearDerivedIntelligenceReport, CoreIntelligenceRebuildRequest,
    DashboardSnapshot, ExportRequest, HealthRepairReport, HealthReport, HistoryQuery,
    HistoryQueryResponse, ImportBatchDetail, ImportProgressEvent, TakeoutInspection,
    TakeoutRequest, ai_queue, clear_derived_intelligence_state, doctor, export_history,
    inspect_browser_history, inspect_takeout,
    intelligence_runtime::{
        DAILY_ROLLUP_JOB_TYPE, STRUCTURAL_REBUILD_JOB_TYPE, VISIT_DERIVE_JOB_TYPE,
        enqueue_core_intelligence_job, mark_all_deterministic_modules_stale,
    },
    list_history, load_audit_run_detail, load_dashboard_snapshot,
    preview_derived_intelligence_clear, preview_import_batch, repair_health_issues,
    restore_import_batch, revert_import_batch, run_backup_with_progress,
};

use std::collections::HashMap;
use std::ops::ControlFlow;
use std::sync::mpsc::Sender;
use std::sync::{Mutex, MutexGuard};
use std::time::{Duration, Instant};
use vault_core::og_images_fetch::{FetchClient, FetchedOgImage};

/// Previews replaying a saved browser-source checkpoint into the canonical archive.
pub fn preview_snapshot_restore_plan(
    vault: &Vault,
    request: &vault_core::SnapshotRestoreRequest,
) -> Result<vault_core::SnapshotRestorePreview> {
    vault_core::preview_snapshot_restore(vault, request)
}

/// Executes the saved-checkpoint replay flow and returns the resulting backup report.
pub fn run_snapshot_restore_plan(
    vault: &Vault,
    request: &vault_core::SnapshotRestoreRequest,
) -> Result<vault_core::BackupReport> {
    vault_core::run_snapshot_restore(vault, request)
}

/// Lists the safety snapshots the recovery screen offers, native and old-format, newest first.
/// A keyless filesystem scan: no archive open, no key.
pub fn list_recovery_snapshots() -> Result<Vec<vault_core::RecoverySnapshot>> {
    let paths = vault_core::project_paths()?;
    let mut snapshots = vault_core::maintenance::list_snapshots(&paths)?;
    snapshots.extend(vault_convert_sqlcipher::list_legacy_snapshots(&paths)?);
    snapshots.sort_by(|a, b| b.created_at.cmp(&a.created_at).then_with(|| b.path.cmp(&a.path)));
    Ok(snapshots)
}

/// Runs the one-click full-archive restore (D1): quarantine the broken canonical files, install +
/// verify the chosen snapshot, reconcile config, rebuild an empty source-evidence.
///
/// The canonical archive may be un-openable (that is the whole point of this recovery), so the
/// config falls back to default rather than failing the restore on a `load_unlocked_config` error.
pub fn run_full_archive_restore(
    session: &crate::VaultSession,
    password: Option<&str>,
    request: &vault_core::SnapshotRestoreRequest,
) -> Result<vault_core::FullArchiveRestoreReport> {
    let paths = vault_core::project_paths()?;
    if let Ok(config) = vault_core::load_config(&paths) {
        vault_core::ensure_app_lock_unlocked(&paths, &config)?;
    }
    let source = std::path::Path::new(&request.snapshot_path);
    // A restore replaces the archive with whatever it is given, so it takes only a snapshot
    // PathKeep made and listed; any other file is refused before anything changes.
    let listed = list_recovery_snapshots()?.into_iter().any(|snapshot| {
        std::fs::canonicalize(&snapshot.path).ok() == std::fs::canonicalize(source).ok()
    });
    anyhow::ensure!(
        listed && source.exists(),
        "snapshot-not-listed: restore only from a safety snapshot PathKeep lists"
    );
    let current = session.current_vault();
    let legacy = source.is_file();
    let installed =
        crate::maintenance_jobs::run(&paths.app_root, "restoring", |cancel, progress| {
            if legacy {
                return vault_convert_sqlcipher::restore_snapshot(
                    &paths,
                    current.as_ref(),
                    session.owner(),
                    source,
                    password,
                    cancel,
                    progress,
                );
            }
            vault_core::maintenance::restore_snapshot_to(
                &paths,
                current.as_ref(),
                session.owner(),
                source,
                password,
                cancel,
            )
        })?;
    let audit = vault_core::maintenance::record_restore(&installed.vault, &request.snapshot_path);
    let (run_id, warnings) = match audit {
        Ok(id) => (Some(id), Vec::new()),
        Err(error) => (
            None,
            vec![format!(
                "The archive was restored, but its review run could not be saved: {error}"
            )],
        ),
    };
    let report = vault_core::FullArchiveRestoreReport {
        run_id,
        restored_snapshot_path: request.snapshot_path.clone(),
        restored_mode: installed.vault.config().archive_mode.clone(),
        quarantine_dir: installed.previous_files.display().to_string(),
        source_evidence_rebuilt: legacy,
        warnings,
    };
    session.install_archive(installed, password.map(str::to_owned))?;
    Ok(report)
}

/// Builds the manual-first retention preview for local rebuildable artifacts.
pub fn preview_retention_plan(vault: Option<&Vault>) -> Result<vault_core::RetentionPreview> {
    let paths = vault_core::project_paths()?;
    load_unlocked_config(&paths)?;
    vault_core::maintenance::preview_retention(&paths, vault)
}

/// Prunes artifacts through the same native owner used by backups.
pub fn run_retention_plan(
    vault: &Vault,
    request: &vault_core::RetentionPruneRequest,
) -> Result<vault_core::RetentionPruneResult> {
    let keep = list_recovery_snapshots()?
        .into_iter()
        .find(|snapshot| snapshot.verified_openable)
        .map(|snapshot| std::path::PathBuf::from(snapshot.path));
    crate::maintenance_jobs::run(&vault.paths().app_root, "pruning", |cancel, _| {
        vault_core::maintenance::prune(vault, request, keep.as_deref(), &cancel)
    })
}

/// Runs a backup without exposing per-profile progress.
pub fn run_backup_now(
    vault: &Vault,
    session_database_key: Option<&str>,
    due_only: bool,
) -> Result<vault_core::BackupReport> {
    run_backup_now_with_progress(vault, session_database_key, due_only, |_| {})
}

/// Runs a backup and optionally executes configured post-backup follow-ups.
///
/// Archive ingestion completes first. Remote backup upload and AI auto-index are
/// best-effort follow-ups whose failures are returned as warnings so the caller
/// can distinguish “archive write succeeded” from “secondary automation also
/// succeeded”.
pub fn run_backup_now_with_progress<F>(
    vault: &Vault,
    session_database_key: Option<&str>,
    due_only: bool,
    report_progress: F,
) -> Result<vault_core::BackupReport>
where
    F: FnMut(BackupProgressEvent),
{
    let paths = vault_core::project_paths()?;
    let config = load_unlocked_config(&paths)?;
    run_backup_with_loaded_config(
        vault,
        &paths,
        &config,
        session_database_key,
        due_only,
        report_progress,
    )
}

/// Keeps scheduled progress on the same backup path and App Lock exception.
pub(crate) fn run_scheduled_backup_with_progress(
    vault: &Vault,
    paths: &vault_core::ProjectPaths,
    config: &vault_core::AppConfig,
    database_key: Option<&str>,
    progress: impl FnMut(BackupProgressEvent),
) -> Result<vault_core::BackupReport> {
    run_backup_with_loaded_config(vault, paths, config, database_key, true, progress)
}

fn run_backup_with_loaded_config<F>(
    vault: &Vault,
    paths: &vault_core::ProjectPaths,
    config: &vault_core::AppConfig,
    session_database_key: Option<&str>,
    due_only: bool,
    mut report_progress: F,
) -> Result<vault_core::BackupReport>
where
    F: FnMut(BackupProgressEvent),
{
    let mut report =
        run_backup_with_progress(&vault.with_config(config.clone()), due_only, |event| {
            report_progress(event);
        })?;
    // The run record holds the warnings up to here; what the follow-ups add is saved at the end.
    let recorded = report.warning_details.len();
    if !report.due_skipped
        && config.ai.enabled
        && config.ai.semantic_index_enabled
        && config.ai.auto_index_after_backup
    {
        match selected_embedding_provider_runtime(config, None) {
            Ok(provider) => {
                let auto_index_request = AiIndexRequest {
                    provider_id: Some(provider.config.id),
                    ..AiIndexRequest::default()
                };
                if append_ai_auto_index_enqueue_result(
                    &mut report,
                    ai_queue::enqueue_index_job(
                        vault,
                        &auto_index_request,
                        config.ai.job_queue_paused,
                    ),
                    config.ai.job_queue_paused,
                ) {
                    maybe_spawn_ai_queue_drain(vault, paths, config, session_database_key, 1);
                }
            }
            Err(error) => append_ai_auto_index_provider_warning(&mut report, error),
        }
    }
    if !report.due_skipped && backup_changed_archive(report.run.as_ref()) {
        // Sources whose copies of a visit a backed-up visit now stands in for count fewer
        // visits, so their Insights are built again too.
        let mut dirty_profiles = report
            .profiles
            .iter()
            .filter(|profile| {
                profile.new_visits > 0 || profile.new_urls > 0 || profile.new_downloads > 0
            })
            .map(|profile| profile.profile_id.clone())
            .chain(report.profiles.iter().flat_map(|profile| profile.replaced_profiles.clone()))
            .collect::<Vec<_>>();
        dirty_profiles.sort();
        dirty_profiles.dedup();
        append_core_refresh_backup_result(
            &mut report,
            enqueue_and_spawn_deterministic_refresh(vault, &dirty_profiles),
        );
        if config.ai.content_fetch_enabled {
            match vault_core::reoffer_archived_candidates(vault) {
                Ok(count) if count > 0 => {
                    crate::intelligence::maybe_spawn_content_fetch_drain(vault, config)
                }
                Ok(_) => {}
                Err(error) => report.push_warning(vault_core::BackupWarning::new(
                    "enrichment-reoffer-failed",
                    format!("{error:#}"),
                )),
            }
        }
    }
    // Daily og:image cache hygiene. Runs even when the user-selected
    // eviction mode is `Off` because vault_core::og_images::run_cleanup
    // always GCs orphan blobs (rows pointing at no-longer-referenced
    // blob hashes) — that GC is the safety floor that keeps the cache
    // honest. Failures here are non-fatal: surface as a backup warning
    // so the user can see "archive write succeeded, cleanup hiccupped"
    // without losing the backup result. Skipped when the backup itself
    // was skipped (due_skipped) because the OS scheduler is the one
    // driving daily cadence and a manual run shouldn't double-fire.
    if !report.due_skipped {
        append_og_image_cleanup_result(&mut report, run_og_image_cleanup(vault));
        // Negative-cache auto-refetch: try again for any URL whose
        // `refetch_after` has elapsed. Bounded by NEGATIVE_CACHE_DAILY_BUDGET
        // so a single overnight backlog can't burst-fire hundreds of
        // outbound requests; remaining URLs roll into the next daily
        // tick. Same warning channel as the cleanup pass.
        // Load config once and reuse — both budget reads + the wrapped
        // worker calls would otherwise hit disk three times per backup
        // tick. `load_unlocked_config` does parse the on-disk JSON, so
        // the saving is non-trivial on a hot post-backup path.
        // The two helpers will still re-load internally (they need it
        // for `effective_mode`); the dedupe here is for the budget
        // lookups + future readers that want to extend this block.
        let (refetch_budget, prefetch_budget) = vault_core::project_paths()
            .ok()
            .and_then(|paths| load_unlocked_config(&paths).ok())
            .map(|config| {
                (
                    clamp_budget(config.og_image.daily_refetch_budget),
                    clamp_budget(config.og_image.new_visit_prefetch_budget),
                )
            })
            .unwrap_or((clamp_budget(50), clamp_budget(100)));
        append_og_image_refetch_due_result(
            &mut report,
            try_refetch_due_og_images(vault, refetch_budget),
        );
        // Background-mode prefetch: warm the cache for URLs the user
        // visited but the worker hasn't seen yet. OnDemand / Off
        // short-circuit inside `try_prefetch_new_visit_og_images`.
        append_og_image_prefetch_result(
            &mut report,
            try_prefetch_new_visit_og_images(vault, prefetch_budget),
        );
    }
    if let Some(run) = report.run.as_ref()
        && report.warning_details.len() > recorded
        && let Err(error) =
            vault_core::append_run_warnings(vault, run.id, &report.warning_details[recorded..])
    {
        eprintln!("PathKeep could not save the backup's follow-up warnings: {error:#}");
    }
    Ok(report)
}

/// Hard ceiling on each per-tick worker pass — a safety net against a
/// misconfigured user-supplied budget (e.g. someone sets the per-backup
/// new-visit budget to 1,000,000 hoping to rebuild overnight). The
/// per-host rate limit + worker pool still apply, but we refuse to even
/// enqueue more than this number in one shot so a single backup cannot
/// monopolise the worker for hours.
const PER_TICK_BUDGET_HARD_CAP: u32 = 5000;

fn clamp_budget(configured: u32) -> usize {
    configured.min(PER_TICK_BUDGET_HARD_CAP) as usize
}

/// Returns the user's effective og:image fetch policy, folding the
/// legacy `fetch_enabled` kill switch into the modern `fetch_mode`
/// tri-state.
///
/// Surfaced to the worker bridge so the implicit on-demand IPC path
/// (`trigger_og_image_refetch`, called from the Browse `useExplorerOgImages`
/// hook) can refuse to enqueue network fetches when the user has chosen
/// `OgImageFetchMode::Off`.
pub fn effective_og_image_fetch_mode() -> Result<vault_core::OgImageFetchMode> {
    let paths = vault_core::project_paths()?;
    let config = load_unlocked_config(&paths)?;
    Ok(config.og_image.effective_mode())
}

/// Looks up URLs whose `refetch_after` window has elapsed and hands
/// them back to `refetch_og_images` for a second attempt. Returns the
/// (count_due, count_succeeded) pair so the caller can surface both in
/// the backup warning channel.
fn try_refetch_due_og_images(vault: &Vault, budget: usize) -> Result<(usize, u32)> {
    let config = vault.config();
    use vault_core::OgImageFetchMode;
    // `effective_mode` folds in the legacy `fetch_enabled` kill switch;
    // OnDemand intentionally also runs the daily refetch because the
    // negative-cache cooldown is what keeps the on-demand path from
    // re-asking flaky hosts every viewport scroll. Off skips entirely.
    if matches!(config.og_image.effective_mode(), OgImageFetchMode::Off) {
        return Ok((0, 0));
    }
    let due_urls = { vault_core::og_images::list_urls_due_for_refetch(vault, budget, None)?.items };
    if due_urls.is_empty() {
        return Ok((0, 0));
    }
    let due_count = due_urls.len();
    let successful = refetch_og_images(vault, due_urls)?;
    Ok((due_count, successful))
}

/// Per-backup "warm new visits" pass. Picks URLs whose page has been
/// visited but never had an og:image fetch attempt, ordered by most-
/// recent visit, and runs them through the same `refetch_og_images`
/// worker pool. Only runs when `OgImageFetchMode::Background` is the
/// effective mode — `OnDemand` and `Off` short-circuit.
///
/// Returns the (count_enqueued, count_succeeded) pair so the caller
/// can surface it in the backup warnings channel alongside the
/// existing refetch + cleanup outcomes.
fn try_prefetch_new_visit_og_images(vault: &Vault, budget: usize) -> Result<(usize, u32)> {
    let config = vault.config();
    use vault_core::OgImageFetchMode;
    if !matches!(config.og_image.effective_mode(), OgImageFetchMode::Background) {
        return Ok((0, 0));
    }
    if budget == 0 {
        return Ok((0, 0));
    }
    let urls = { vault_core::og_images::list_urls_for_prefetch(vault, budget, None)?.items };
    if urls.is_empty() {
        return Ok((0, 0));
    }
    let enqueued = urls.len();
    let successful = refetch_og_images(vault, urls)?;
    Ok((enqueued, successful))
}

/// Pushes a non-fatal refetch-due result onto a backup report's warning
/// list. Same shape as `append_og_image_cleanup_result` so the formatting
/// stays auditable without spinning up a real worker.
fn append_og_image_refetch_due_result(report: &mut BackupReport, result: Result<(usize, u32)>) {
    match result {
        Ok((0, _)) => {}
        Ok((due, successful)) => report.push_warning(
            BackupWarning::new(
                "og-refetch-summary",
                format!(
                    "Link previews negative-cache retry: re-attempted {due} URLs, {successful} succeeded.",
                ),
            )
            .with_count(due)
            .with_succeeded(successful),
        ),
        Err(error) => report.push_warning(
            BackupWarning::new(
                "og-refetch-failed",
                format!("Link previews negative-cache retry failed: {error:#}"),
            )
            .with_diagnostic(format!("{error:#}")),
        ),
    }
}

/// Pushes a non-fatal prefetch outcome onto a backup report's warning
/// list. Mirrors `append_og_image_refetch_due_result` so the backup
/// audit log treats both passes the same way.
fn append_og_image_prefetch_result(report: &mut BackupReport, result: Result<(usize, u32)>) {
    match result {
        Ok((0, _)) => {}
        Ok((enqueued, successful)) => report.push_warning(
            BackupWarning::new(
                "og-prefetch-summary",
                format!(
                    "Link previews prefetch: enqueued {enqueued} new-visit URLs, {successful} succeeded.",
                ),
            )
            .with_count(enqueued)
            .with_succeeded(successful),
        ),
        Err(error) => report.push_warning(
            BackupWarning::new(
                "og-prefetch-failed",
                format!("Link previews prefetch failed: {error:#}"),
            )
            .with_diagnostic(format!("{error:#}")),
        ),
    }
}

/// Pushes a non-fatal cleanup result onto a backup report's warning list.
///
/// Exposed at module scope so the success vs. failure formatting can be
/// unit-tested directly, without spinning a full backup run.
fn append_og_image_cleanup_result(
    report: &mut BackupReport,
    result: Result<vault_core::OgImageCleanupReport>,
) {
    match result {
        Ok(cleanup) => {
            if cleanup.deleted_rows > 0 || cleanup.deleted_blobs > 0 || cleanup.reclaimed_bytes > 0
            {
                report.push_warning(
                    BackupWarning::new(
                        "og-cleanup-summary",
                        format!(
                            "Link previews cache hygiene: removed {} rows, {} orphan blobs, reclaimed {} bytes.",
                            cleanup.deleted_rows, cleanup.deleted_blobs, cleanup.reclaimed_bytes,
                        ),
                    )
                    .with_count(cleanup.deleted_rows.max(0) as usize)
                    .with_reclaimed(
                        cleanup.deleted_blobs.max(0) as usize,
                        cleanup.reclaimed_bytes.max(0) as u64,
                    ),
                );
            }
        }
        Err(error) => {
            report.push_warning(
                BackupWarning::new(
                    "og-cleanup-failed",
                    format!("Link previews cache hygiene failed: {error:#}"),
                )
                .with_diagnostic(format!("{error:#}")),
            );
        }
    }
}

/// Queries visible archive history rows with the canonical filter contract.
pub fn query_history(
    vault: &Vault,
    _session_database_key: Option<&str>,
    query: HistoryQuery,
) -> Result<HistoryQueryResponse> {
    list_history(vault, query)
}

/// Loads favicon payloads for already-visible Explorer rows without blocking the
/// main history query path on image bytes.
pub fn load_history_favicons(
    vault: &Vault,
    entries: Vec<vault_core::HistoryFaviconLookupEntry>,
) -> Result<Vec<vault_core::HistoryFaviconLookupResult>> {
    vault_core::load_history_favicons(vault, entries)
}

/// Loads each listed site's own icon, for screens that list sites (Insights).
pub fn load_site_favicons(
    vault: &Vault,
    domains: Vec<String>,
) -> Result<Vec<vault_core::SiteFavicon>> {
    vault_core::load_site_favicons(vault, domains)
}

/// Loads cached og:image payloads for already-visible card-mode rows.
pub fn load_history_og_images(
    vault: &Vault,
    entries: Vec<vault_core::HistoryOgImageLookupEntry>,
) -> Result<Vec<vault_core::HistoryOgImageLookupResult>> {
    vault_core::og_images::load_og_images(vault, entries)
}

/// Bumps `last_shown_at` for the given page URLs. Frontend calls this in
/// a debounced batch after cards settle into view so user-configured
/// LRU eviction has a real signal.
pub fn mark_og_images_shown(vault: &Vault, urls: Vec<String>) -> Result<()> {
    vault_core::og_images::mark_og_images_shown(vault, &urls)
}

/// Reports cache footprint to the Settings → Storage panel.
pub fn og_image_storage_stats(vault: &Vault) -> Result<vault_core::OgImageStorageStats> {
    vault_core::og_images::storage_stats(vault)
}

/// Reports og:image coverage (share of web pages with a preview image) for
/// Settings → Link previews. On-demand only — the eligible-page count scans the
/// `urls` table, so this never runs on a hot path.
pub fn og_image_coverage_stats(vault: &Vault) -> Result<vault_core::OgImageCoverageStats> {
    vault_core::og_images::coverage_stats(vault)
}

/// Drops every og:image cache row and its blob bytes. Behind the
/// Settings → "Clear all link previews" confirm dialog.
pub fn clear_og_image_cache(vault: &Vault) -> Result<vault_core::OgImageCleanupReport> {
    vault_core::og_images::clear_cache(vault)
}

/// Runs one cleanup pass using the user's configured eviction mode.
/// Settings exposes this as "Run cleanup now"; the daily schedule tick
/// will also call this in C5.
pub fn run_og_image_cleanup(vault: &Vault) -> Result<vault_core::OgImageCleanupReport> {
    let config = vault.config();
    vault_core::og_images::run_cleanup(vault, config.og_image.cleanup)
}

/// Fetches og:image previews for the given URLs synchronously, one at
/// a time, and persists each outcome. Skips URLs whose host is on the
/// user's blocklist and short-circuits the whole call when the user
/// has disabled fetching globally.
///
/// This is the entry point Tauri commands hand to `spawn_blocking` so
/// the UI thread never sees a network stall. Fetches run on a small
/// thread pool (currently 2 workers) so a long-tail of slow hosts can't
/// block faster ones, with a per-host throttle that enforces at least
/// 500 ms between requests targeting the same hostname — friendly to the
/// upstream and a hard upper bound on our own RPS regardless of how
/// many URLs the UI hands in.
///
/// SQLite isn't safe to share across threads, so the workers send
/// `FetchedOgImage` outcomes back to the main thread via an mpsc
/// channel and only the main thread writes to the archive. The Rust
/// reqwest client is `Send + Sync` and is cloned via Arc to the
/// workers.
pub fn refetch_og_images(vault: &Vault, urls: Vec<String>) -> Result<u32> {
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex, mpsc};
    use std::time::{Duration, Instant};

    const WORKER_POOL_SIZE: usize = 2;
    const RATE_LIMIT_PER_HOST: Duration = Duration::from_millis(500);

    let config = vault.config();
    if !config.og_image.fetch_enabled {
        return Ok(0);
    }
    if urls.is_empty() {
        return Ok(0);
    }
    let client = Arc::new(
        vault_core::og_images_fetch::build_fetch_client()
            .context("building og:image fetch client")?,
    );
    let blocked_hosts = Arc::new(config.og_image.blocked_hosts.clone());

    // Reverse + pop = FIFO without amortizing O(n) drain costs.
    let work: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(urls.into_iter().rev().collect()));
    // Maps `host` → the next Instant at which a fetch for that host is
    // allowed. Updated under lock by every worker before issuing a
    // request so two workers can't blow the budget by picking same-host
    // URLs at the same moment.
    let host_state: Arc<Mutex<HashMap<String, Instant>>> = Arc::new(Mutex::new(HashMap::new()));
    let (sender, receiver) =
        mpsc::channel::<(String, vault_core::og_images_fetch::FetchedOgImage)>();

    let mut handles = Vec::with_capacity(WORKER_POOL_SIZE);
    for _ in 0..WORKER_POOL_SIZE {
        let work = Arc::clone(&work);
        let client = Arc::clone(&client);
        let host_state = Arc::clone(&host_state);
        let blocked_hosts = Arc::clone(&blocked_hosts);
        let sender = sender.clone();
        handles.push(std::thread::spawn(move || {
            while drain_one_worker_url(
                &work,
                &host_state,
                client.as_ref(),
                blocked_hosts.as_ref(),
                &sender,
                RATE_LIMIT_PER_HOST,
            )
            .is_continue()
            {}
        }));
    }
    drop(sender);

    let mut successful = 0_u32;
    let mut last_persist_error: Option<anyhow::Error> = None;
    while let Ok((url, outcome)) = receiver.recv() {
        record_refetch_outcome(vault, &url, &outcome, &mut successful, &mut last_persist_error);
    }
    for handle in handles {
        let _ = handle.join();
    }
    finalize_refetch_run(successful, last_persist_error)
}

/// Surfaces the last persist error from the worker drain if there was
/// one, otherwise reports the success counter.
fn finalize_refetch_run(successful: u32, last_persist_error: Option<anyhow::Error>) -> Result<u32> {
    if let Some(error) = last_persist_error {
        return Err(error);
    }
    Ok(successful)
}

/// Walks one work item through the per-worker pipeline. Returns
/// `ControlFlow::Continue(())` while the worker should keep running, and
/// `Break(())` when the queue is drained or the receiver has hung up.
fn drain_one_worker_url(
    work: &Mutex<Vec<String>>,
    host_state: &Mutex<HashMap<String, Instant>>,
    client: &FetchClient,
    blocked_hosts: &[String],
    sender: &Sender<(String, FetchedOgImage)>,
    interval: Duration,
) -> ControlFlow<()> {
    let url = {
        let mut queue = lock_or_recover(work);
        match queue.pop() {
            Some(url) => url,
            None => return ControlFlow::Break(()),
        }
    };

    let wait = host_throttle_wait(host_state, &url, interval);
    if !wait.is_zero() {
        std::thread::sleep(wait);
    }

    let outcome = if vault_core::og_images_fetch::is_host_blocked(blocked_hosts, &url) {
        vault_core::og_images_fetch::blocked_outcome(&url)
    } else {
        vault_core::og_images_fetch::fetch_og_image_for(client, &url)
    };
    if sender.send((url, outcome)).is_err() {
        return ControlFlow::Break(());
    }
    ControlFlow::Continue(())
}

/// Receiver-side accumulator for one worker outcome. Bumps `successful`
/// when the fetch produced bytes, persists the row regardless (so
/// negative-cache outcomes still write their refetch_after window), and
/// captures the *last* persist error so the parent function can surface
/// it after the worker pool joins.
fn record_refetch_outcome(
    vault: &Vault,
    url: &str,
    outcome: &FetchedOgImage,
    successful: &mut u32,
    last_persist_error: &mut Option<anyhow::Error>,
) {
    if outcome.is_ok() {
        *successful += 1;
    }
    // Compute the negative-cache window from the outcome's terminal status
    // so transient HTTP / parse / size failures get a real retry slot
    // instead of going dormant forever with refetch_after = NULL.
    let refetch_after =
        vault_core::og_images_fetch::default_refetch_after_for_status(outcome.fetch_status());
    if let Err(error) = vault_core::og_images::upsert_og_image(
        vault,
        &outcome.as_insert(url, refetch_after.as_deref()),
    ) {
        *last_persist_error = Some(error);
    }
}

/// Locks a mutex and recovers the inner data when a previous holder
/// panicked while holding it. The recovery path matters because both
/// worker threads and the throttle book-keeping share `Arc<Mutex>` —
/// if one worker panics, the rest must still finish their queue rather
/// than poisoning the whole refetch run.
fn lock_or_recover<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    match mutex.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

/// Computes how long the worker should sleep before issuing a request
/// for `url`'s host, and records the next-allowed slot for that host.
fn host_throttle_wait(
    host_state: &Mutex<HashMap<String, Instant>>,
    url: &str,
    interval: Duration,
) -> Duration {
    let host = vault_core::utils::url_domain(url).to_ascii_lowercase();
    if host.is_empty() {
        return Duration::ZERO;
    }
    let mut state = lock_or_recover(host_state);
    let now = Instant::now();
    let next_allowed = state.get(&host).copied().unwrap_or(now);
    let my_slot = if next_allowed > now { next_allowed } else { now };
    state.insert(host, my_slot + interval);
    my_slot.saturating_duration_since(now)
}

/// Loads the dashboard snapshot read model for the current unlocked session.
pub fn dashboard_snapshot(vault: &Vault) -> Result<DashboardSnapshot> {
    load_dashboard_snapshot(vault)
}

/// Loads per-source visit counts and date spans. Discovery in the app snapshot only knows file
/// sizes, so Home and History ask for this separately.
pub fn source_stats(vault: &Vault) -> Result<Vec<vault_core::SourceStats>> {
    vault_core::load_source_stats(vault)
}

/// Whether the user visited `url`; the launcher opens a `file://` URL only if they did.
pub fn is_visited_url(vault: &Vault, url: &str) -> Result<bool> {
    vault_core::is_visited_url(vault, url)
}

/// Loads the History detail panel data for one exact URL.
pub fn url_detail(vault: &Vault, url: &str) -> Result<vault_core::UrlDetail> {
    vault_core::get_url_detail(vault, url)
}

/// Loads audit detail for a specific run id.
pub fn audit_run_detail(vault: &Vault, run_id: i64) -> Result<vault_core::AuditRunDetail> {
    load_audit_run_detail(vault, run_id)
}

/// Exports the currently visible history query in the requested format.
pub fn export_query(
    vault: &Vault,
    _session_database_key: Option<&str>,
    request: ExportRequest,
) -> Result<vault_core::ExportResult> {
    export_history(vault, request)
}

/// Counts what [`clear_derived_intelligence`] would remove, without removing anything.
pub fn preview_clear_derived_intelligence(vault: &Vault) -> Result<ClearDerivedIntelligenceReport> {
    preview_derived_intelligence_clear(vault)
}

/// Clears rebuildable intelligence state while leaving canonical archive facts intact.
pub fn clear_derived_intelligence(vault: &Vault) -> Result<ClearDerivedIntelligenceReport> {
    clear_derived_intelligence_state(vault)
}

/// Inspects a Takeout source without mutating the archive.
pub fn inspect_takeout_source(request: &TakeoutRequest) -> Result<TakeoutInspection> {
    let paths = vault_core::project_paths()?;
    inspect_takeout(&paths, request)
}

/// Inspects one local browser history database without mutating the archive.
///
/// Reads the archive (hence App Lock and the session key) only to count the
/// visits it already holds, so the preview can say how many are new.
pub fn inspect_browser_history_source(
    vault: &Vault,
    request: &BrowserHistoryImportRequest,
) -> Result<TakeoutInspection> {
    inspect_browser_history(vault, request)
}

/// Imports a Takeout source into the canonical archive.
pub fn import_takeout_source(
    vault: &Vault,
    session_database_key: Option<&str>,
    request: &TakeoutRequest,
) -> Result<TakeoutInspection> {
    import_takeout_source_with_progress(vault, session_database_key, request, None, |_| {})
}

/// Imports a Takeout source with progress events. Under `import_id`, `stop_import` can stop it
/// between batches of records; a stopped import returns what it saved, and its Insights are
/// refreshed like a finished one's.
pub fn import_takeout_source_with_progress<F>(
    vault: &Vault,
    _session_database_key: Option<&str>,
    request: &TakeoutRequest,
    import_id: Option<&str>,
    report_progress: F,
) -> Result<TakeoutInspection>
where
    F: FnMut(ImportProgressEvent),
{
    let paths = vault_core::project_paths()?;
    let config = load_unlocked_config(&paths)?;
    let job = vault_core::ImportJob::start(import_id)?;
    let mut inspection = vault_core::takeout::import_takeout_with_control(
        &vault.with_config(config.clone()),
        request,
        job.cancel_token(),
        report_progress,
    )?;
    if inspection.imported_items > 0 {
        let profiles = vault_core::takeout::takeout_profiles(&inspection);
        append_core_refresh_import_result(
            &mut inspection.notes,
            enqueue_and_spawn_deterministic_refresh(vault, &profiles),
        );
    }
    Ok(inspection)
}

/// Imports one local browser history database into the canonical archive.
pub fn import_browser_history_source(
    vault: &Vault,
    session_database_key: Option<&str>,
    request: &BrowserHistoryImportRequest,
) -> Result<TakeoutInspection> {
    import_browser_history_source_with_progress(vault, session_database_key, request, None, |_| {})
}

/// Imports a browser's history file with progress events; `import_id` lets `stop_import` stop
/// it, as for Takeout.
pub fn import_browser_history_source_with_progress<F>(
    vault: &Vault,
    _session_database_key: Option<&str>,
    request: &BrowserHistoryImportRequest,
    import_id: Option<&str>,
    report_progress: F,
) -> Result<TakeoutInspection>
where
    F: FnMut(ImportProgressEvent),
{
    let paths = vault_core::project_paths()?;
    let config = load_unlocked_config(&paths)?;
    let job = vault_core::ImportJob::start(import_id)?;
    let mut inspection = vault_core::takeout::import_browser_history_with_control(
        &vault.with_config(config.clone()),
        request,
        job.cancel_token(),
        report_progress,
    )?;
    if inspection.imported_items > 0 {
        if let Some(profile_id) =
            inspection.import_batch.as_ref().map(|batch| batch.profile_id.clone())
        {
            let mut profiles = vec![profile_id];
            profiles.extend(inspection.replaced_profiles.iter().cloned());
            append_core_refresh_import_result(
                &mut inspection.notes,
                enqueue_and_spawn_deterministic_refresh(vault, &profiles),
            );
        }
    }
    Ok(inspection)
}

/// Loads the preview/read-model detail for one import batch.
pub fn preview_import_batch_detail(vault: &Vault, batch_id: i64) -> Result<ImportBatchDetail> {
    preview_import_batch(vault, batch_id)
}

/// Soft-hides one imported batch and returns its updated detail.
pub fn revert_import_batch_detail(vault: &Vault, batch_id: i64) -> Result<ImportBatchDetail> {
    refresh_after_visibility_change(vault, revert_import_batch(vault, batch_id)?)
}

/// Restores one previously reverted import batch to visible status.
pub fn restore_import_batch_detail(vault: &Vault, batch_id: i64) -> Result<ImportBatchDetail> {
    refresh_after_visibility_change(vault, restore_import_batch(vault, batch_id)?)
}

/// Undoing or restoring an import changes counted visits as an import does, so Insights are
/// rebuilt the same way, for the profiles it changed. The undo itself is done either way; if
/// the rebuild cannot be queued, Background work shows the Insights as out of date.
fn refresh_after_visibility_change(
    vault: &Vault,
    detail: ImportBatchDetail,
) -> Result<ImportBatchDetail> {
    if !detail.changed_profiles.is_empty() {
        if let Err(error) = enqueue_and_spawn_deterministic_refresh(vault, &detail.changed_profiles)
        {
            eprintln!(
                "pathkeep: import batch {}: Insights could not be queued to rebuild: {error:#}",
                detail.batch.id
            );
        }
    }
    Ok(detail)
}

/// Runs doctor checks against the current archive.
pub fn doctor_report(vault: &Vault) -> Result<HealthReport> {
    let mut report = doctor(vault)?;
    report.checks.push(vault_core::derived_health(vault)?);
    Ok(report)
}

/// Repairs archive metadata and clears derived Insights rows that outlived their visits.
pub fn repair_health(vault: &Vault) -> Result<HealthRepairReport> {
    repair_health_issues(vault)
}

fn backup_changed_archive(run: Option<&vault_core::BackupRunOverview>) -> bool {
    run.is_some_and(|run| run.new_visits > 0 || run.new_urls > 0 || run.new_downloads > 0)
}

fn enqueue_and_spawn_deterministic_refresh(
    vault: &Vault,
    dirty_profiles: &[String],
) -> Result<Vec<i64>> {
    mark_all_deterministic_modules_stale(
        vault,
        vault_core::DERIVED_STALE_ARCHIVE_DATA_CHANGED,
        "Archive data changed and Core Intelligence refresh jobs were queued.",
    )?;
    let rebuild_scopes = core_refresh_rebuild_scopes(dirty_profiles);
    let mut job_ids = Vec::with_capacity(rebuild_scopes.len() * 3);
    for request in &rebuild_scopes {
        let scope_label = request.profile_id.as_deref().unwrap_or("all profiles").to_string();
        job_ids.push(enqueue_core_intelligence_job(
            vault,
            VISIT_DERIVE_JOB_TYPE,
            request,
            &format!(
                "Archive data changed and visit-derived facts need a refresh for {scope_label}."
            ),
        )?);
        job_ids.push(enqueue_core_intelligence_job(
            vault,
            DAILY_ROLLUP_JOB_TYPE,
            request,
            &format!("Archive data changed and daily rollups need a refresh for {scope_label}."),
        )?);
        job_ids.push(enqueue_core_intelligence_job(
            vault,
            STRUCTURAL_REBUILD_JOB_TYPE,
            request,
            &format!("Archive data changed and structural Core Intelligence entities need a refresh for {scope_label}."),
        )?);
    }
    if !vault.config().ai.job_queue_paused {
        maybe_spawn_intelligence_queue_drain(vault, job_ids.len());
    }
    Ok(job_ids)
}

fn append_core_refresh_backup_result(report: &mut BackupReport, result: Result<Vec<i64>>) {
    if let Err(error) = result {
        report.push_warning(
            BackupWarning::new(
                "intelligence-refresh-failed",
                format!("Core Intelligence could not refresh after backup: {error}"),
            )
            .with_diagnostic(format!("{error}")),
        );
    }
}

fn append_ai_auto_index_enqueue_result(
    report: &mut BackupReport,
    result: Result<AiQueueJob>,
    queue_paused: bool,
) -> bool {
    match result {
        Ok(job) if queue_paused => {
            report.push_warning(
                BackupWarning::new(
                    "ai-autoindex-queued-while-paused",
                    format!("AI auto-index queued job {} while the AI queue is paused.", job.id),
                )
                .with_job_id(job.id),
            );
            false
        }
        Ok(_) => true,
        Err(error) => {
            report.push_warning(
                BackupWarning::new(
                    "ai-autoindex-enqueue-failed",
                    format!("AI auto-index could not enqueue a follow-up job: {error}"),
                )
                .with_diagnostic(format!("{error}")),
            );
            false
        }
    }
}

fn append_ai_auto_index_provider_warning(report: &mut BackupReport, error: anyhow::Error) {
    report.push_warning(
        BackupWarning::new(
            "ai-autoindex-provider-not-ready",
            format!("AI auto-index is enabled, but the embedding provider is not ready: {error}"),
        )
        .with_diagnostic(format!("{error}")),
    );
}

fn append_core_refresh_import_result(notes: &mut Vec<String>, result: Result<Vec<i64>>) {
    match result {
        Ok(job_ids) => notes.push(core_refresh_import_note(&job_ids)),
        Err(error) => notes.push(format!(
            "Core Intelligence could not refresh automatically after import: {error}"
        )),
    }
}

fn core_refresh_import_note(job_ids: &[i64]) -> String {
    format!(
        "Core Intelligence refresh jobs {} were queued automatically after import and will finish in the background.",
        job_ids.iter().map(ToString::to_string).collect::<Vec<_>>().join(", ")
    )
}

fn core_refresh_rebuild_scopes(dirty_profiles: &[String]) -> Vec<CoreIntelligenceRebuildRequest> {
    if dirty_profiles.is_empty() {
        vec![CoreIntelligenceRebuildRequest::default()]
    } else {
        dirty_profiles
            .iter()
            .map(|profile_id| CoreIntelligenceRebuildRequest {
                profile_id: Some(profile_id.clone()),
                ..CoreIntelligenceRebuildRequest::default()
            })
            .collect::<Vec<_>>()
    }
}

#[cfg(test)]
mod tests {
    use super::try_prefetch_new_visit_og_images;
    use crate::tests::{
        PROJECT_ROOT_OVERRIDE_ENV, TEST_KEYRING_OVERRIDE_ENV, lock_env, restore_env_var,
    };
    use tempfile::tempdir;
    use vault_core::{AppConfig, ArchiveMode};

    fn write_test_config(paths: &vault_core::ProjectPaths, config: &AppConfig) {
        if let Some(parent) = paths.archive_database_path.parent() {
            std::fs::create_dir_all(parent).expect("archive db parent");
        }
        if let Some(parent) = paths.config_path.parent() {
            std::fs::create_dir_all(parent).expect("config parent");
        }
        std::fs::write(&paths.config_path, serde_json::to_string(config).expect("config json"))
            .expect("write config");
    }

    #[test]
    fn try_prefetch_new_visit_short_circuits_when_mode_is_not_background() {
        let vault = vault_core::Vault::open_temp().unwrap();
        let _guard = lock_env();
        let dir = tempdir().expect("tempdir");
        let original_project_root = std::env::var_os(PROJECT_ROOT_OVERRIDE_ENV);
        let original_keyring = std::env::var_os(TEST_KEYRING_OVERRIDE_ENV);
        unsafe {
            std::env::set_var(PROJECT_ROOT_OVERRIDE_ENV, dir.path());
            std::env::set_var(TEST_KEYRING_OVERRIDE_ENV, dir.path().join("keyring"));
        }
        let paths = vault_core::project_paths().expect("paths");

        let mut config = AppConfig {
            initialized: true,
            archive_mode: ArchiveMode::Plaintext,
            ..AppConfig::default()
        };
        // `fetch_enabled` is the kill switch; once it's off, `effective_mode`
        // returns `Off` regardless of `fetch_mode` and the prefetch pass
        // must short-circuit.
        config.og_image.fetch_enabled = false;
        write_test_config(&paths, &config);
        vault_core::config::ensure_paths(&paths).expect("paths");
        let result = try_prefetch_new_visit_og_images(&vault.with_config(config.clone()), 100);
        assert_eq!(result.expect("prefetch returns"), (0, 0));

        // Re-enable but pick OnDemand — prefetch should still skip
        // because the docstring restricts that pass to Background mode.
        let mut config = AppConfig {
            initialized: true,
            archive_mode: ArchiveMode::Plaintext,
            ..AppConfig::default()
        };
        config.og_image.fetch_mode = vault_core::OgImageFetchMode::OnDemand;
        write_test_config(&paths, &config);
        let result = try_prefetch_new_visit_og_images(&vault.with_config(config.clone()), 100);
        assert_eq!(result.expect("prefetch returns"), (0, 0));

        // Zero budget short-circuits even when mode is Background.
        let config = AppConfig {
            initialized: true,
            archive_mode: ArchiveMode::Plaintext,
            ..AppConfig::default()
        };
        write_test_config(&paths, &config);
        let result = try_prefetch_new_visit_og_images(&vault.with_config(config.clone()), 0);
        assert_eq!(result.expect("prefetch returns"), (0, 0));

        restore_env_var(PROJECT_ROOT_OVERRIDE_ENV, original_project_root.as_deref());
        restore_env_var(TEST_KEYRING_OVERRIDE_ENV, original_keyring.as_deref());
    }

    #[test]
    fn refetch_og_images_short_circuits_when_kill_switch_is_off() {
        let vault = vault_core::Vault::open_temp().unwrap();
        // Every external caller of
        // `refetch_og_images` pre-gates on `effective_mode != Off`. The
        // inner `if !config.og_image.fetch_enabled` check at the top of
        // this fn is the safety net for a future caller that forgets to
        // gate, so it is exercised directly here.
        let _guard = lock_env();
        let dir = tempdir().expect("tempdir");
        let original_project_root = std::env::var_os(PROJECT_ROOT_OVERRIDE_ENV);
        let original_keyring = std::env::var_os(TEST_KEYRING_OVERRIDE_ENV);
        unsafe {
            std::env::set_var(PROJECT_ROOT_OVERRIDE_ENV, dir.path());
            std::env::set_var(TEST_KEYRING_OVERRIDE_ENV, dir.path().join("keyring"));
        }
        let paths = vault_core::project_paths().expect("paths");

        let mut config = AppConfig {
            initialized: true,
            archive_mode: ArchiveMode::Plaintext,
            ..AppConfig::default()
        };
        config.og_image.fetch_enabled = false;
        write_test_config(&paths, &config);
        vault_core::config::ensure_paths(&paths).expect("paths");

        let result =
            super::refetch_og_images(&vault, vec!["https://example.com/never-reached".to_string()]);
        assert_eq!(result.expect("kill-switch short-circuits"), 0);

        restore_env_var(PROJECT_ROOT_OVERRIDE_ENV, original_project_root.as_deref());
        restore_env_var(TEST_KEYRING_OVERRIDE_ENV, original_keyring.as_deref());
    }
}
