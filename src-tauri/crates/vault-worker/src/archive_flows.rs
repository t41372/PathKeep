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
    context::{
        ai_archive_connection, load_unlocked_config, resolved_app_lock_status,
        selected_embedding_provider_runtime,
    },
    intelligence::{maybe_spawn_ai_queue_drain, maybe_spawn_intelligence_queue_drain},
};
use anyhow::{Context, Result};
use vault_core::{
    AiIndexRequest, AiQueueJob, BackupProgressEvent, BrowserHistoryImportRequest,
    ClearDerivedIntelligenceReport, CoreIntelligenceRebuildRequest, DashboardSnapshot,
    ExportRequest, HealthRepairReport, HealthReport, HistoryQuery, HistoryQueryResponse,
    ImportBatchDetail, ImportProgressEvent, RemoteBackupPreview, RemoteBackupResult,
    RemoteBackupVerification, TakeoutInspection, TakeoutRequest, ai_queue,
    clear_derived_intelligence_state, doctor, export_history, import_browser_history_with_progress,
    import_takeout_with_progress, inspect_browser_history, inspect_takeout,
    intelligence_runtime::{
        DAILY_ROLLUP_JOB_TYPE, STRUCTURAL_REBUILD_JOB_TYPE, VISIT_DERIVE_JOB_TYPE,
        enqueue_core_intelligence_job, mark_all_deterministic_modules_stale,
    },
    list_history, load_audit_run_detail, load_dashboard_snapshot, preview_import_batch,
    preview_remote_backup, repair_health_issues, restore_import_batch, revert_import_batch,
    run_backup_with_progress, run_remote_backup, verify_remote_backup,
};
use vault_platform::keyring_get_s3_credentials;

use std::collections::HashMap;
use std::ops::ControlFlow;
use std::sync::mpsc::Sender;
use std::sync::{Mutex, MutexGuard};
use std::time::{Duration, Instant};
use vault_core::og_images_fetch::{FetchClient, FetchedOgImage};

/// Previews replaying a saved browser-source checkpoint into the canonical archive.
pub fn preview_snapshot_restore_plan(
    session_database_key: Option<&str>,
    request: &vault_core::SnapshotRestoreRequest,
) -> Result<vault_core::SnapshotRestorePreview> {
    let paths = vault_core::project_paths()?;
    let config = load_unlocked_config(&paths)?;
    vault_core::preview_snapshot_restore(&paths, &config, session_database_key, request)
}

/// Executes the saved-checkpoint replay flow and returns the resulting backup report.
pub fn run_snapshot_restore_plan(
    session_database_key: Option<&str>,
    request: &vault_core::SnapshotRestoreRequest,
) -> Result<vault_core::BackupReport> {
    let paths = vault_core::project_paths()?;
    let config = load_unlocked_config(&paths)?;
    vault_core::run_snapshot_restore(&paths, &config, session_database_key, request)
}

/// Builds the manual-first retention preview for local rebuildable artifacts.
pub fn preview_retention_plan(
    session_database_key: Option<&str>,
) -> Result<vault_core::RetentionPreview> {
    let paths = vault_core::project_paths()?;
    let config = load_unlocked_config(&paths).unwrap_or_default();
    vault_core::preview_retention(&paths, &config, session_database_key)
}

/// Executes the explicit retention prune request.
pub fn run_retention_plan(
    session_database_key: Option<&str>,
    request: &vault_core::RetentionPruneRequest,
) -> Result<vault_core::RetentionPruneResult> {
    let paths = vault_core::project_paths()?;
    let config = load_unlocked_config(&paths)?;
    vault_core::run_retention_prune(&paths, &config, session_database_key, request)
}

/// Runs a backup without exposing per-profile progress.
pub fn run_backup_now(
    session_database_key: Option<&str>,
    due_only: bool,
) -> Result<vault_core::BackupReport> {
    run_backup_now_with_progress(session_database_key, due_only, |_| {})
}

/// Runs a backup and optionally executes configured post-backup follow-ups.
///
/// Archive ingestion completes first. Remote backup upload and AI auto-index are
/// best-effort follow-ups whose failures are returned as warnings so the caller
/// can distinguish “archive write succeeded” from “secondary automation also
/// succeeded”.
pub fn run_backup_now_with_progress<F>(
    session_database_key: Option<&str>,
    due_only: bool,
    mut report_progress: F,
) -> Result<vault_core::BackupReport>
where
    F: FnMut(BackupProgressEvent),
{
    let paths = vault_core::project_paths()?;
    let config = load_unlocked_config(&paths)?;
    let mut report =
        run_backup_with_progress(&paths, &config, session_database_key, due_only, |event| {
            report_progress(event);
        })?;
    if !report.due_skipped
        && config.remote_backup.enabled
        && config.remote_backup.upload_after_backup
    {
        match keyring_get_s3_credentials()? {
            Some(credentials) => {
                let remote = run_remote_backup(&paths, &config, session_database_key, &credentials)?;
                if remote.uploaded {
                    report.remote_backup = Some(remote);
                } else {
                    report.warnings.push(remote.message.clone());
                    report.remote_backup = Some(remote);
                }
            }
            None => report
                .warnings
                .push("Remote backup is enabled, but S3 credentials are not stored in the system keyring.".to_string()),
        }
    }
    if !report.due_skipped
        && config.ai.enabled
        && config.ai.semantic_index_enabled
        && config.ai.auto_index_after_backup
    {
        match selected_embedding_provider_runtime(&config, None) {
            Ok(provider) => {
                let auto_index_request = AiIndexRequest {
                    provider_id: Some(provider.config.id),
                    ..AiIndexRequest::default()
                };
                if append_ai_auto_index_archive_result(
                    &mut report.warnings,
                    ai_archive_connection(&paths, &config, session_database_key),
                    &auto_index_request,
                    config.ai.job_queue_paused,
                ) {
                    maybe_spawn_ai_queue_drain(&paths, &config, session_database_key, 1);
                }
            }
            Err(error) => append_ai_auto_index_provider_warning(&mut report.warnings, error),
        }
    }
    if !report.due_skipped && backup_changed_archive(report.run.as_ref()) {
        let dirty_profiles = report
            .profiles
            .iter()
            .filter(|profile| {
                profile.new_visits > 0 || profile.new_urls > 0 || profile.new_downloads > 0
            })
            .map(|profile| profile.profile_id.clone())
            .collect::<Vec<_>>();
        append_core_refresh_backup_result(
            &mut report.warnings,
            enqueue_and_spawn_deterministic_refresh(
                &paths,
                &config,
                session_database_key,
                &dirty_profiles,
            ),
        );
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
        append_og_image_cleanup_result(
            &mut report.warnings,
            run_og_image_cleanup(session_database_key),
        );
        // Negative-cache auto-refetch: try again for any URL whose
        // `refetch_after` has elapsed. Bounded by NEGATIVE_CACHE_DAILY_BUDGET
        // so a single overnight backlog can't burst-fire hundreds of
        // outbound requests; remaining URLs roll into the next daily
        // tick. Same warning channel as the cleanup pass.
        append_og_image_refetch_due_result(
            &mut report.warnings,
            try_refetch_due_og_images(session_database_key, NEGATIVE_CACHE_DAILY_BUDGET),
        );
    }
    Ok(report)
}

/// Per-day ceiling on negative-cache retries. The number lands between
/// "enough to cover a normal user's daily transient-failure backlog" and
/// "low enough that a single misconfigured backup can't spam an upstream
/// in concert with the per-host rate limit". 50 keeps both invariants
/// honest at ≤ 25s total wall-clock under the worst-case same-host
/// distribution (50 × 500ms / 2 workers).
const NEGATIVE_CACHE_DAILY_BUDGET: usize = 50;

/// Looks up URLs whose `refetch_after` window has elapsed and hands
/// them back to `refetch_og_images` for a second attempt. Returns the
/// (count_due, count_succeeded) pair so the caller can surface both in
/// the backup warning channel.
fn try_refetch_due_og_images(
    session_database_key: Option<&str>,
    budget: usize,
) -> Result<(usize, u32)> {
    let paths = vault_core::project_paths()?;
    let config = load_unlocked_config(&paths)?;
    if !config.og_image.fetch_enabled {
        return Ok((0, 0));
    }
    let due_urls = {
        let connection =
            vault_core::archive::open_archive_connection(&paths, &config, session_database_key)?;
        vault_core::og_images::list_urls_due_for_refetch(&connection, budget)?
    };
    if due_urls.is_empty() {
        return Ok((0, 0));
    }
    let due_count = due_urls.len();
    let successful = refetch_og_images(session_database_key, due_urls)?;
    Ok((due_count, successful))
}

/// Pushes a non-fatal refetch-due result onto a backup report's warning
/// list. Same shape as `append_og_image_cleanup_result` so the formatting
/// stays auditable without spinning up a real worker.
fn append_og_image_refetch_due_result(warnings: &mut Vec<String>, result: Result<(usize, u32)>) {
    match result {
        Ok((0, _)) => {}
        Ok((due, successful)) => warnings.push(format!(
            "Link previews negative-cache retry: re-attempted {due} URLs, {successful} succeeded.",
        )),
        Err(error) => {
            warnings.push(format!("Link previews negative-cache retry failed: {error:#}",))
        }
    }
}

/// Pushes a non-fatal cleanup result onto a backup report's warning list.
///
/// Exposed at module scope so the success vs. failure formatting can be
/// unit-tested directly, without spinning a full backup run.
fn append_og_image_cleanup_result(
    warnings: &mut Vec<String>,
    result: Result<vault_core::OgImageCleanupReport>,
) {
    match result {
        Ok(report) => {
            if report.deleted_rows > 0 || report.deleted_blobs > 0 || report.reclaimed_bytes > 0 {
                warnings.push(format!(
                    "Link previews cache hygiene: removed {} rows, {} orphan blobs, reclaimed {} bytes.",
                    report.deleted_rows,
                    report.deleted_blobs,
                    report.reclaimed_bytes,
                ));
            }
        }
        Err(error) => {
            warnings.push(format!("Link previews cache hygiene failed: {error:#}",));
        }
    }
}

/// Queries visible archive history rows with the canonical filter contract.
pub fn query_history(
    session_database_key: Option<&str>,
    query: HistoryQuery,
) -> Result<HistoryQueryResponse> {
    let paths = vault_core::project_paths()?;
    let config = load_unlocked_config(&paths)?;
    list_history(&paths, &config, session_database_key, query)
}

/// Loads favicon payloads for already-visible Explorer rows without blocking the
/// main history query path on image bytes.
pub fn load_history_favicons(
    session_database_key: Option<&str>,
    entries: Vec<vault_core::HistoryFaviconLookupEntry>,
) -> Result<Vec<vault_core::HistoryFaviconLookupResult>> {
    let paths = vault_core::project_paths()?;
    let config = load_unlocked_config(&paths)?;
    vault_core::load_history_favicons(&paths, &config, session_database_key, entries)
}

/// Loads cached og:image payloads for already-visible card-mode rows.
pub fn load_history_og_images(
    session_database_key: Option<&str>,
    entries: Vec<vault_core::HistoryOgImageLookupEntry>,
) -> Result<Vec<vault_core::HistoryOgImageLookupResult>> {
    let paths = vault_core::project_paths()?;
    let config = load_unlocked_config(&paths)?;
    vault_core::og_images::load_og_images(&paths, &config, session_database_key, entries)
}

/// Bumps `last_shown_at` for the given page URLs. Frontend calls this in
/// a debounced batch after cards settle into view so user-configured
/// LRU eviction has a real signal.
pub fn mark_og_images_shown(session_database_key: Option<&str>, urls: Vec<String>) -> Result<()> {
    let paths = vault_core::project_paths()?;
    let config = load_unlocked_config(&paths)?;
    let connection =
        vault_core::archive::open_archive_connection(&paths, &config, session_database_key)?;
    vault_core::og_images::mark_og_images_shown(&connection, &urls)
}

/// Reports cache footprint to the Settings → Storage panel.
pub fn og_image_storage_stats(
    session_database_key: Option<&str>,
) -> Result<vault_core::OgImageStorageStats> {
    let paths = vault_core::project_paths()?;
    let config = load_unlocked_config(&paths)?;
    let connection =
        vault_core::archive::open_archive_connection(&paths, &config, session_database_key)?;
    vault_core::og_images::storage_stats(&connection)
}

/// Drops every og:image cache row and its blob bytes. Behind the
/// Settings → "Clear all link previews" confirm dialog.
pub fn clear_og_image_cache(
    session_database_key: Option<&str>,
) -> Result<vault_core::OgImageCleanupReport> {
    let paths = vault_core::project_paths()?;
    let config = load_unlocked_config(&paths)?;
    let connection =
        vault_core::archive::open_archive_connection(&paths, &config, session_database_key)?;
    vault_core::og_images::clear_cache(&connection)
}

/// Runs one cleanup pass using the user's configured eviction mode.
/// Settings exposes this as "Run cleanup now"; the daily schedule tick
/// will also call this in C5.
pub fn run_og_image_cleanup(
    session_database_key: Option<&str>,
) -> Result<vault_core::OgImageCleanupReport> {
    let paths = vault_core::project_paths()?;
    let config = load_unlocked_config(&paths)?;
    let connection =
        vault_core::archive::open_archive_connection(&paths, &config, session_database_key)?;
    vault_core::og_images::run_cleanup(&connection, config.og_image.cleanup)
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
pub fn refetch_og_images(session_database_key: Option<&str>, urls: Vec<String>) -> Result<u32> {
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex, mpsc};
    use std::time::{Duration, Instant};

    const WORKER_POOL_SIZE: usize = 2;
    const RATE_LIMIT_PER_HOST: Duration = Duration::from_millis(500);

    let paths = vault_core::project_paths()?;
    let config = load_unlocked_config(&paths)?;
    if !config.og_image.fetch_enabled {
        return Ok(0);
    }
    if urls.is_empty() {
        return Ok(0);
    }
    let connection =
        vault_core::archive::open_archive_connection(&paths, &config, session_database_key)?;
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
        record_refetch_outcome(
            &connection,
            &url,
            &outcome,
            &mut successful,
            &mut last_persist_error,
        );
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
    connection: &rusqlite::Connection,
    url: &str,
    outcome: &FetchedOgImage,
    successful: &mut u32,
    last_persist_error: &mut Option<anyhow::Error>,
) {
    if outcome.is_ok() {
        *successful += 1;
    }
    if let Err(error) = vault_core::og_images::upsert_og_image(connection, &outcome.as_insert(url))
    {
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
pub fn dashboard_snapshot(session_database_key: Option<&str>) -> Result<DashboardSnapshot> {
    let paths = vault_core::project_paths()?;
    let config = load_unlocked_config(&paths)?;
    load_dashboard_snapshot(&paths, &config, session_database_key)
}

/// Loads audit detail for a specific run id.
pub fn audit_run_detail(
    session_database_key: Option<&str>,
    run_id: i64,
) -> Result<vault_core::AuditRunDetail> {
    let paths = vault_core::project_paths()?;
    let config = load_unlocked_config(&paths)?;
    load_audit_run_detail(&paths, &config, session_database_key, run_id)
}

/// Exports the currently visible history query in the requested format.
pub fn export_query(
    session_database_key: Option<&str>,
    request: ExportRequest,
) -> Result<vault_core::ExportResult> {
    let paths = vault_core::project_paths()?;
    let config = load_unlocked_config(&paths)?;
    export_history(&paths, &config, session_database_key, request)
}

/// Builds the remote-backup preview bundle metadata without uploading anything.
pub fn preview_remote_backup_bundle() -> Result<RemoteBackupPreview> {
    let paths = vault_core::project_paths()?;
    let config = load_unlocked_config(&paths)?;
    preview_remote_backup(&paths, &config)
}

/// Uploads the latest remote-backup bundle with the stored S3 credentials.
pub fn upload_remote_backup_bundle(
    session_database_key: Option<&str>,
) -> Result<RemoteBackupResult> {
    let paths = vault_core::project_paths()?;
    let config = load_unlocked_config(&paths)?;
    let credentials = keyring_get_s3_credentials()?
        .context("store S3 credentials in Settings before running a remote backup")?;
    run_remote_backup(&paths, &config, session_database_key, &credentials)
}

/// Verifies a built remote-backup bundle against the v1 restore contract.
pub fn verify_remote_backup_bundle(
    session_database_key: Option<&str>,
    bundle_path: &str,
) -> Result<RemoteBackupVerification> {
    let paths = vault_core::project_paths()?;
    let config = load_unlocked_config(&paths)?;
    let _ = resolved_app_lock_status(&paths, &config)?;
    verify_remote_backup(std::path::Path::new(bundle_path), session_database_key)
}

/// Clears rebuildable intelligence state while leaving canonical archive facts intact.
pub fn clear_derived_intelligence(
    session_database_key: Option<&str>,
) -> Result<ClearDerivedIntelligenceReport> {
    let paths = vault_core::project_paths()?;
    let config = load_unlocked_config(&paths)?;
    clear_derived_intelligence_state(&paths, &config, session_database_key)
}

/// Inspects a Takeout source without mutating the archive.
pub fn inspect_takeout_source(request: &TakeoutRequest) -> Result<TakeoutInspection> {
    let paths = vault_core::project_paths()?;
    inspect_takeout(&paths, request)
}

/// Inspects one local browser history database without mutating the archive.
pub fn inspect_browser_history_source(
    request: &BrowserHistoryImportRequest,
) -> Result<TakeoutInspection> {
    let paths = vault_core::project_paths()?;
    inspect_browser_history(&paths, request)
}

/// Imports a Takeout source into the canonical archive.
pub fn import_takeout_source(
    session_database_key: Option<&str>,
    request: &TakeoutRequest,
) -> Result<TakeoutInspection> {
    import_takeout_source_with_progress(session_database_key, request, |_| {})
}

pub fn import_takeout_source_with_progress<F>(
    session_database_key: Option<&str>,
    request: &TakeoutRequest,
    report_progress: F,
) -> Result<TakeoutInspection>
where
    F: FnMut(ImportProgressEvent),
{
    let paths = vault_core::project_paths()?;
    let config = load_unlocked_config(&paths)?;
    let mut inspection = import_takeout_with_progress(
        &paths,
        &config,
        session_database_key,
        request,
        report_progress,
    )?;
    if inspection.imported_items > 0 {
        append_core_refresh_import_result(
            &mut inspection.notes,
            enqueue_and_spawn_deterministic_refresh(
                &paths,
                &config,
                session_database_key,
                &["takeout::browser-history".to_string()],
            ),
        );
    }
    Ok(inspection)
}

/// Imports one local browser history database into the canonical archive.
pub fn import_browser_history_source(
    session_database_key: Option<&str>,
    request: &BrowserHistoryImportRequest,
) -> Result<TakeoutInspection> {
    import_browser_history_source_with_progress(session_database_key, request, |_| {})
}

pub fn import_browser_history_source_with_progress<F>(
    session_database_key: Option<&str>,
    request: &BrowserHistoryImportRequest,
    report_progress: F,
) -> Result<TakeoutInspection>
where
    F: FnMut(ImportProgressEvent),
{
    let paths = vault_core::project_paths()?;
    let config = load_unlocked_config(&paths)?;
    let mut inspection = import_browser_history_with_progress(
        &paths,
        &config,
        session_database_key,
        request,
        report_progress,
    )?;
    if inspection.imported_items > 0 {
        if let Some(profile_id) =
            inspection.import_batch.as_ref().map(|batch| batch.profile_id.clone())
        {
            append_core_refresh_import_result(
                &mut inspection.notes,
                enqueue_and_spawn_deterministic_refresh(
                    &paths,
                    &config,
                    session_database_key,
                    &[profile_id],
                ),
            );
        }
    }
    Ok(inspection)
}

/// Loads the preview/read-model detail for one import batch.
pub fn preview_import_batch_detail(
    session_database_key: Option<&str>,
    batch_id: i64,
) -> Result<ImportBatchDetail> {
    let paths = vault_core::project_paths()?;
    let config = load_unlocked_config(&paths)?;
    preview_import_batch(&paths, &config, session_database_key, batch_id)
}

/// Soft-hides one imported batch and returns its updated detail.
pub fn revert_import_batch_detail(
    session_database_key: Option<&str>,
    batch_id: i64,
) -> Result<ImportBatchDetail> {
    let paths = vault_core::project_paths()?;
    let config = load_unlocked_config(&paths)?;
    revert_import_batch(&paths, &config, session_database_key, batch_id)
}

/// Restores one previously reverted import batch to visible status.
pub fn restore_import_batch_detail(
    session_database_key: Option<&str>,
    batch_id: i64,
) -> Result<ImportBatchDetail> {
    let paths = vault_core::project_paths()?;
    let config = load_unlocked_config(&paths)?;
    restore_import_batch(&paths, &config, session_database_key, batch_id)
}

/// Runs doctor checks against the current archive.
pub fn doctor_report(session_database_key: Option<&str>) -> Result<HealthReport> {
    let paths = vault_core::project_paths()?;
    let config = load_unlocked_config(&paths)?;
    doctor(&paths, &config, session_database_key)
}

/// Runs the repair path for doctor-detected issues.
pub fn repair_health(session_database_key: Option<&str>) -> Result<HealthRepairReport> {
    let paths = vault_core::project_paths()?;
    let config = load_unlocked_config(&paths)?;
    repair_health_issues(&paths, &config, session_database_key)
}

fn backup_changed_archive(run: Option<&vault_core::BackupRunOverview>) -> bool {
    run.is_some_and(|run| run.new_visits > 0 || run.new_urls > 0 || run.new_downloads > 0)
}

fn enqueue_and_spawn_deterministic_refresh(
    paths: &vault_core::ProjectPaths,
    config: &vault_core::AppConfig,
    session_database_key: Option<&str>,
    dirty_profiles: &[String],
) -> Result<Vec<i64>> {
    let connection = ai_archive_connection(paths, config, session_database_key)?;
    mark_all_deterministic_modules_stale(
        &connection,
        "Archive data changed and Core Intelligence refresh jobs were queued.",
    )?;
    let rebuild_scopes = core_refresh_rebuild_scopes(dirty_profiles);
    let mut job_ids = Vec::with_capacity(rebuild_scopes.len() * 3);
    for request in &rebuild_scopes {
        let scope_label = request.profile_id.as_deref().unwrap_or("all profiles").to_string();
        job_ids.push(enqueue_core_intelligence_job(
            &connection,
            VISIT_DERIVE_JOB_TYPE,
            request,
            &format!(
                "Archive data changed and visit-derived facts need a refresh for {scope_label}."
            ),
        )?);
        job_ids.push(enqueue_core_intelligence_job(
            &connection,
            DAILY_ROLLUP_JOB_TYPE,
            request,
            &format!("Archive data changed and daily rollups need a refresh for {scope_label}."),
        )?);
        job_ids.push(enqueue_core_intelligence_job(
            &connection,
            STRUCTURAL_REBUILD_JOB_TYPE,
            request,
            &format!("Archive data changed and structural Core Intelligence entities need a refresh for {scope_label}."),
        )?);
    }
    if !config.ai.job_queue_paused {
        maybe_spawn_intelligence_queue_drain(paths, config, session_database_key, job_ids.len());
    }
    Ok(job_ids)
}

fn append_core_refresh_backup_result(warnings: &mut Vec<String>, result: Result<Vec<i64>>) {
    if let Err(error) = result {
        warnings.push(format!("Core Intelligence could not refresh after backup: {error}"));
    }
}

fn append_ai_auto_index_enqueue_result(
    warnings: &mut Vec<String>,
    result: Result<AiQueueJob>,
    queue_paused: bool,
) -> bool {
    match result {
        Ok(job) if queue_paused => {
            warnings
                .push(format!("AI auto-index queued job {} while the AI queue is paused.", job.id));
            false
        }
        Ok(_) => true,
        Err(error) => {
            warnings.push(format!("AI auto-index could not enqueue a follow-up job: {error}"));
            false
        }
    }
}

fn append_ai_auto_index_archive_result(
    warnings: &mut Vec<String>,
    connection: Result<rusqlite::Connection>,
    request: &AiIndexRequest,
    queue_paused: bool,
) -> bool {
    match connection {
        Ok(connection) => append_ai_auto_index_enqueue_result(
            warnings,
            ai_queue::enqueue_index_job(&connection, request, queue_paused),
            queue_paused,
        ),
        Err(error) => {
            append_ai_auto_index_provider_warning(warnings, error);
            false
        }
    }
}

fn append_ai_auto_index_provider_warning(warnings: &mut Vec<String>, error: anyhow::Error) {
    warnings.push(format!(
        "AI auto-index is enabled, but the embedding provider is not ready: {error}"
    ));
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
    use super::{
        append_ai_auto_index_archive_result, append_ai_auto_index_enqueue_result,
        append_ai_auto_index_provider_warning, append_core_refresh_backup_result,
        append_core_refresh_import_result, append_og_image_cleanup_result,
        append_og_image_refetch_due_result, core_refresh_import_note, core_refresh_rebuild_scopes,
        drain_one_worker_url, finalize_refetch_run, host_throttle_wait, lock_or_recover,
        try_refetch_due_og_images,
    };
    use crate::tests::{
        PROJECT_ROOT_OVERRIDE_ENV, TEST_KEYRING_OVERRIDE_ENV, lock_env, restore_env_var,
    };
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};
    use std::time::{Duration, Instant};
    use tempfile::tempdir;
    use vault_core::{AiQueueJob, AppConfig, ArchiveMode, OgImageCleanupReport};

    fn fresh_host_state() -> Arc<Mutex<HashMap<String, Instant>>> {
        Arc::new(Mutex::new(HashMap::new()))
    }

    #[test]
    fn append_og_image_cleanup_result_silent_when_no_work_done() {
        let mut warnings = Vec::new();
        append_og_image_cleanup_result(
            &mut warnings,
            Ok(OgImageCleanupReport { deleted_rows: 0, deleted_blobs: 0, reclaimed_bytes: 0 }),
        );
        assert!(warnings.is_empty(), "no-op cleanup should not add a warning");
    }

    #[test]
    fn append_og_image_cleanup_result_records_evicted_rows() {
        let mut warnings = Vec::new();
        append_og_image_cleanup_result(
            &mut warnings,
            Ok(OgImageCleanupReport { deleted_rows: 4, deleted_blobs: 2, reclaimed_bytes: 1_234 }),
        );
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("4 rows"));
        assert!(warnings[0].contains("2 orphan blobs"));
    }

    #[test]
    fn append_og_image_cleanup_result_surfaces_errors_as_warnings() {
        let mut warnings = Vec::new();
        append_og_image_cleanup_result(&mut warnings, Err(anyhow::anyhow!("archive locked")));
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("archive locked"));
        assert!(warnings[0].contains("cache hygiene failed"));
    }

    #[test]
    fn append_og_image_refetch_due_result_silent_when_no_due_rows() {
        let mut warnings = Vec::new();
        append_og_image_refetch_due_result(&mut warnings, Ok((0, 0)));
        assert!(warnings.is_empty(), "no due rows should not add a warning even when successful=0",);
    }

    #[test]
    fn append_og_image_refetch_due_result_annotates_retried_counts() {
        let mut warnings = Vec::new();
        append_og_image_refetch_due_result(&mut warnings, Ok((7, 4)));
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("7 URLs"));
        assert!(warnings[0].contains("4 succeeded"));
    }

    #[test]
    fn append_og_image_refetch_due_result_surfaces_errors_as_warnings() {
        let mut warnings = Vec::new();
        append_og_image_refetch_due_result(&mut warnings, Err(anyhow::anyhow!("dns hiccup")));
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("dns hiccup"));
        assert!(warnings[0].contains("negative-cache retry failed"));
    }

    #[test]
    fn host_throttle_wait_returns_zero_for_first_request_to_a_host() {
        let state = fresh_host_state();
        let wait =
            host_throttle_wait(&state, "https://github.com/foo/bar", Duration::from_millis(500));
        assert_eq!(wait, Duration::ZERO);
        assert!(state.lock().unwrap().contains_key("github.com"));
    }

    #[test]
    fn host_throttle_wait_returns_zero_when_url_has_no_host() {
        let state = fresh_host_state();
        // The empty-string input is the only canonical "no host" case that
        // url_domain produces an empty extraction for — malformed scheme
        // prefixes (`not a url`, `httpsx://x`) still produce a hostlike
        // bucket that the throttle is happy to track separately.
        let wait = host_throttle_wait(&state, "", Duration::from_millis(500));
        assert_eq!(wait, Duration::ZERO);
        assert!(state.lock().unwrap().is_empty());
    }

    #[test]
    fn host_throttle_wait_serializes_back_to_back_same_host_requests() {
        let state = fresh_host_state();
        // First request reserves its slot at "now" and writes next_allowed = now + 500ms.
        let first = host_throttle_wait(&state, "https://github.com/a", Duration::from_millis(500));
        assert_eq!(first, Duration::ZERO);
        // Second request has to wait roughly the full interval.
        let second = host_throttle_wait(&state, "https://github.com/b", Duration::from_millis(500));
        assert!(
            second >= Duration::from_millis(450) && second <= Duration::from_millis(500),
            "expected second host wait to be ~500ms, got {second:?}",
        );
    }

    #[test]
    fn host_throttle_wait_does_not_cross_pollinate_hosts() {
        let state = fresh_host_state();
        let _ = host_throttle_wait(&state, "https://github.com/a", Duration::from_millis(500));
        let other = host_throttle_wait(&state, "https://medium.com/a", Duration::from_millis(500));
        assert_eq!(other, Duration::ZERO);
    }

    #[test]
    fn host_throttle_wait_lowercases_host_and_unifies_case_variants() {
        let state = fresh_host_state();
        let _ = host_throttle_wait(&state, "https://EXAMPLE.com/a", Duration::from_millis(500));
        let second =
            host_throttle_wait(&state, "https://example.com/b", Duration::from_millis(500));
        assert!(second > Duration::ZERO, "case variants must share a host slot");
    }

    #[test]
    fn lock_or_recover_returns_inner_data_when_lock_is_clean() {
        let mutex = Mutex::new(vec![1_u8, 2, 3]);
        let guard = lock_or_recover(&mutex);
        assert_eq!(*guard, vec![1, 2, 3]);
    }

    #[test]
    fn lock_or_recover_recovers_poisoned_lock() {
        // Poison the mutex on purpose, then verify lock_or_recover still
        // hands the data back instead of bubbling the PoisonError. This is
        // the contract the og:image worker pool relies on so one panicked
        // worker can't take down the rest of the refetch run.
        let mutex = Arc::new(Mutex::new(vec![10_u8]));
        let mutex_clone = Arc::clone(&mutex);
        let join = std::thread::spawn(move || {
            let _guard = mutex_clone.lock().expect("first lock");
            panic!("poisoning the mutex on purpose");
        });
        let _ = join.join(); // we expect the thread to have panicked
        assert!(mutex.is_poisoned());
        let guard = lock_or_recover(&mutex);
        assert_eq!(*guard, vec![10_u8]);
    }

    #[test]
    fn record_refetch_outcome_increments_successful_for_ok_outcomes() {
        use crate::archive_flows::record_refetch_outcome as recorder;
        let connection = rusqlite::Connection::open_in_memory().expect("memory db");
        vault_core::archive::create_schema(&connection).expect("schema");

        let png_bytes: [u8; 9] = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 0xFF];
        let ok_outcome = vault_core::og_images_fetch::ok_outcome_for_test(
            "https://example.com/x",
            "https://example.com/og.png",
            &png_bytes,
            "image/png",
        );

        let mut successful = 0_u32;
        let mut last_error = None;
        recorder(
            &connection,
            "https://example.com/x",
            &ok_outcome,
            &mut successful,
            &mut last_error,
        );
        assert_eq!(successful, 1);
        assert!(last_error.is_none());
    }

    #[test]
    fn record_refetch_outcome_records_persist_error_when_schema_is_absent() {
        use crate::archive_flows::record_refetch_outcome as recorder;
        // A connection with no schema makes `upsert_og_image` fail with
        // "no such table: og_images". This drives the
        // `if let Err(error) = upsert_og_image(...) { last_persist_error = ... }`
        // branch end-to-end with no network or worker pool involvement.
        let connection = rusqlite::Connection::open_in_memory().expect("memory db");

        let outcome = vault_core::og_images_fetch::blocked_outcome("https://example.com/blocked");
        let mut successful = 0_u32;
        let mut last_error = None;
        recorder(
            &connection,
            "https://example.com/blocked",
            &outcome,
            &mut successful,
            &mut last_error,
        );
        // outcome.is_ok() == false for blocked → success counter stays put
        // and the persist call surfaces the missing-table error.
        assert_eq!(successful, 0);
        assert!(last_error.is_some(), "missing schema must surface as a persist error");
    }

    #[test]
    fn drain_one_worker_url_returns_break_when_queue_is_empty() {
        let work = Mutex::new(Vec::<String>::new());
        let host_state = fresh_host_state();
        let client = vault_core::og_images_fetch::build_fetch_client().expect("client");
        let blocked: Vec<String> = Vec::new();
        let (sender, _receiver) = std::sync::mpsc::channel();
        let flow = drain_one_worker_url(
            &work,
            &host_state,
            &client,
            &blocked,
            &sender,
            Duration::from_millis(0),
        );
        assert!(matches!(flow, std::ops::ControlFlow::Break(())));
    }

    #[test]
    fn drain_one_worker_url_breaks_when_receiver_is_dropped() {
        // Dropping the receiver causes `sender.send` to fail; the worker
        // must observe that and break instead of spinning. We use a
        // host-blocked URL so the inner branch returns a deterministic
        // outcome with no network call.
        let work = Mutex::new(vec!["https://blocked.test/a".to_string()]);
        let host_state = fresh_host_state();
        let client = vault_core::og_images_fetch::build_fetch_client().expect("client");
        let blocked: Vec<String> = vec!["blocked.test".to_string()];
        let (sender, receiver) = std::sync::mpsc::channel();
        drop(receiver);
        let flow = drain_one_worker_url(
            &work,
            &host_state,
            &client,
            &blocked,
            &sender,
            Duration::from_millis(0),
        );
        assert!(matches!(flow, std::ops::ControlFlow::Break(())));
    }

    #[test]
    fn drain_one_worker_url_processes_a_blocked_host_and_continues() {
        let work = Mutex::new(vec!["https://blocked.test/a".to_string()]);
        let host_state = fresh_host_state();
        let client = vault_core::og_images_fetch::build_fetch_client().expect("client");
        let blocked: Vec<String> = vec!["blocked.test".to_string()];
        let (sender, receiver) = std::sync::mpsc::channel();
        let flow = drain_one_worker_url(
            &work,
            &host_state,
            &client,
            &blocked,
            &sender,
            Duration::from_millis(0),
        );
        assert!(matches!(flow, std::ops::ControlFlow::Continue(())));
        let (url, outcome) = receiver.try_recv().expect("a forwarded outcome");
        assert_eq!(url, "https://blocked.test/a");
        assert!(!outcome.is_ok());
    }

    #[test]
    fn drain_one_worker_url_observes_host_throttle_sleep_path() {
        // Pre-poison the host_state so the throttle returns a non-zero
        // wait, ensuring the `if !wait.is_zero() { std::thread::sleep }`
        // arm fires inside the worker body. Use a 1 ms interval so the
        // test still runs fast.
        let host_state = fresh_host_state();
        // Seed the state by walking a previous URL through the throttle.
        let _ =
            host_throttle_wait(&host_state, "https://blocked.test/seed", Duration::from_millis(20));

        let work = Mutex::new(vec!["https://blocked.test/a".to_string()]);
        let client = vault_core::og_images_fetch::build_fetch_client().expect("client");
        let blocked: Vec<String> = vec!["blocked.test".to_string()];
        let (sender, receiver) = std::sync::mpsc::channel();
        let started = Instant::now();
        let flow = drain_one_worker_url(
            &work,
            &host_state,
            &client,
            &blocked,
            &sender,
            Duration::from_millis(20),
        );
        let elapsed = started.elapsed();
        assert!(matches!(flow, std::ops::ControlFlow::Continue(())));
        // Sleep arm ran — total elapsed should reflect the throttle wait.
        // Loose bound to keep the test stable on a busy CI host.
        assert!(elapsed >= Duration::from_millis(5), "throttle sleep should have fired");
        assert!(receiver.try_recv().is_ok());
    }

    #[test]
    fn drain_one_worker_url_calls_fetch_og_image_for_unblocked_hosts() {
        // Force the !is_host_blocked branch by leaving the blocklist
        // empty. Use an `http://127.0.0.1:0` URL so the production
        // `fetch_og_image_for` short-circuits at its https-only guard
        // without reaching the network. The arm we care about is the
        // `vault_core::og_images_fetch::fetch_og_image_for(client, url)`
        // call; the resulting outcome carries `parse_error` because
        // http:// is rejected, which is fine — we only assert that the
        // worker forwarded a message for the URL.
        let work = Mutex::new(vec!["http://127.0.0.1:1/test".to_string()]);
        let host_state = fresh_host_state();
        let client = vault_core::og_images_fetch::build_fetch_client().expect("client");
        let blocked: Vec<String> = Vec::new();
        let (sender, receiver) = std::sync::mpsc::channel();
        let flow = drain_one_worker_url(
            &work,
            &host_state,
            &client,
            &blocked,
            &sender,
            Duration::from_millis(0),
        );
        assert!(matches!(flow, std::ops::ControlFlow::Continue(())));
        let (url, _outcome) = receiver.try_recv().expect("a forwarded outcome");
        assert_eq!(url, "http://127.0.0.1:1/test");
    }

    #[test]
    fn finalize_refetch_run_returns_success_when_no_persist_error_recorded() {
        assert_eq!(finalize_refetch_run(3, None).unwrap(), 3);
    }

    #[test]
    fn finalize_refetch_run_surfaces_last_persist_error_when_present() {
        let result = finalize_refetch_run(2, Some(anyhow::anyhow!("write failed")));
        assert!(result.is_err());
        let error = result.unwrap_err();
        assert!(error.to_string().contains("write failed"));
    }

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
    fn try_refetch_due_og_images_short_circuits_when_fetch_disabled() {
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
        vault_core::ensure_archive_initialized(&paths, &config, None).expect("init archive");

        let result = try_refetch_due_og_images(None, 10);
        assert_eq!(result.expect("try_refetch returns"), (0, 0));

        restore_env_var(PROJECT_ROOT_OVERRIDE_ENV, original_project_root.as_deref());
        restore_env_var(TEST_KEYRING_OVERRIDE_ENV, original_keyring.as_deref());
    }

    #[test]
    fn try_refetch_due_og_images_returns_zero_when_no_due_rows_exist() {
        let _guard = lock_env();
        let dir = tempdir().expect("tempdir");
        let original_project_root = std::env::var_os(PROJECT_ROOT_OVERRIDE_ENV);
        let original_keyring = std::env::var_os(TEST_KEYRING_OVERRIDE_ENV);
        unsafe {
            std::env::set_var(PROJECT_ROOT_OVERRIDE_ENV, dir.path());
            std::env::set_var(TEST_KEYRING_OVERRIDE_ENV, dir.path().join("keyring"));
        }
        let paths = vault_core::project_paths().expect("paths");

        let config = AppConfig {
            initialized: true,
            archive_mode: ArchiveMode::Plaintext,
            ..AppConfig::default()
        };
        write_test_config(&paths, &config);
        vault_core::ensure_archive_initialized(&paths, &config, None).expect("init archive");

        // Archive has no og:image rows, so the due list is empty and
        // `try_refetch_due_og_images` returns (0, 0) without ever calling
        // `refetch_og_images`.
        let result = try_refetch_due_og_images(None, 10).expect("try_refetch returns");
        assert_eq!(result, (0, 0));

        restore_env_var(PROJECT_ROOT_OVERRIDE_ENV, original_project_root.as_deref());
        restore_env_var(TEST_KEYRING_OVERRIDE_ENV, original_keyring.as_deref());
    }

    #[test]
    fn try_refetch_due_og_images_drives_refetch_for_due_urls() {
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
        // Block every host we'd otherwise hit so the worker pool returns
        // a deterministic `blocked` outcome with no network touch and
        // the drained tally still flows through the success-counter and
        // upsert paths.
        config.og_image.blocked_hosts = vec!["blocked.test".to_string()];
        write_test_config(&paths, &config);
        vault_core::ensure_archive_initialized(&paths, &config, None).expect("init archive");

        // Pre-seed a row whose refetch_after window is already in the
        // past so `list_urls_due_for_refetch` returns it.
        let connection = vault_core::archive::open_archive_connection(&paths, &config, None)
            .expect("connection");
        let insert = vault_core::og_images::OgImageInsert {
            page_url: "https://blocked.test/page",
            page_host: Some("blocked.test"),
            source_og_url: None,
            image_bytes: None,
            mime: None,
            width: None,
            height: None,
            fetch_status: "missing",
            http_status: Some(503),
            refetch_after: Some("2000-01-01T00:00:00Z"),
            fetch_attempts: 1,
            created_by_run_id: None,
        };
        vault_core::og_images::upsert_og_image(&connection, &insert).expect("insert");
        drop(connection);

        let (due, _successful) = try_refetch_due_og_images(None, 10).expect("try_refetch");
        assert_eq!(due, 1, "the seeded row must surface as the one due URL");

        restore_env_var(PROJECT_ROOT_OVERRIDE_ENV, original_project_root.as_deref());
        restore_env_var(TEST_KEYRING_OVERRIDE_ENV, original_keyring.as_deref());
    }

    #[test]
    fn core_refresh_note_helpers_cover_success_error_and_scope_edges() {
        assert!(core_refresh_import_note(&[1, 2, 3]).contains("1, 2, 3"));

        let mut notes = Vec::new();
        append_core_refresh_import_result(&mut notes, Ok(vec![7, 8]));
        append_core_refresh_import_result(&mut notes, Err(anyhow::anyhow!("queue offline")));
        assert!(notes.iter().any(|note| note.contains("7, 8")));
        assert!(notes.iter().any(|note| note.contains("queue offline")));

        let mut warnings = Vec::new();
        append_core_refresh_backup_result(&mut warnings, Ok(vec![1]));
        append_core_refresh_backup_result(&mut warnings, Err(anyhow::anyhow!("archive locked")));
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("archive locked"));

        let mut ai_warnings = Vec::new();
        assert!(!append_ai_auto_index_enqueue_result(
            &mut ai_warnings,
            Ok(AiQueueJob { id: 42, ..AiQueueJob::default() }),
            true,
        ));
        assert!(ai_warnings[0].contains("queued job 42"));
        assert!(append_ai_auto_index_enqueue_result(
            &mut ai_warnings,
            Ok(AiQueueJob::default()),
            false,
        ));
        assert!(!append_ai_auto_index_enqueue_result(
            &mut ai_warnings,
            Err(anyhow::anyhow!("queue offline")),
            false,
        ));
        append_ai_auto_index_provider_warning(
            &mut ai_warnings,
            anyhow::anyhow!("provider missing"),
        );
        assert!(!append_ai_auto_index_archive_result(
            &mut ai_warnings,
            Err(anyhow::anyhow!("archive unavailable")),
            &vault_core::AiIndexRequest::default(),
            false,
        ));
        assert!(ai_warnings.iter().any(|warning| warning.contains("queue offline")));
        assert!(ai_warnings.iter().any(|warning| warning.contains("provider missing")));
        assert!(ai_warnings.iter().any(|warning| warning.contains("archive unavailable")));

        let all_profiles = core_refresh_rebuild_scopes(&[]);
        assert_eq!(all_profiles.len(), 1);
        assert!(all_profiles[0].profile_id.is_none());
        let scoped = core_refresh_rebuild_scopes(&["chrome:Default".to_string()]);
        assert_eq!(scoped[0].profile_id.as_deref(), Some("chrome:Default"));
    }
}
