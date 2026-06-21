//! Worker-side content-fetch (W-ENRICH-1) orchestration.
//!
//! ## Responsibilities
//! - Bridge the Tauri command façade to the vault-core content-fetch plane: read/write the consent
//!   settings, list a visit's stored enrichment, trigger a manual PME "fetch now", and run the bulk
//!   working-set-prioritized enqueue.
//! - Own the low-concurrency content-fetch DRAIN lane (a SINGLE worker, off the backup/import critical
//!   path) so fetches are polite (06 §5: LOW concurrency, never block search/explorer).
//!
//! ## Not responsible for
//! - The fetch/extract/store itself (vault-core `content_fetch_api`), the SSRF/rate-limit/negative-
//!   cache gates (vault-core), or the desktop command naming (the Tauri façade).
//!
//! ## Why a separate lane from the enrichment lane
//! The offline enrichment lane (title normalization) is local + fast; content fetch is network-bound +
//! rate-limited. Giving content fetch its OWN single-worker lane keeps a slow/throttled host from
//! starving the local enrichment work and bounds egress concurrency to one (06 §5).

use crate::context::load_unlocked_config;
#[cfg(not(coverage))]
use crate::job_runtime::maybe_spawn_worker_pool;
use anyhow::Result;
#[cfg(not(coverage))]
use std::sync::atomic::AtomicUsize;
use vault_core::{
    AppConfig, AppSnapshot, ContentFetchNowRequest, ContentFetchNowResult, ContentFetchSettings,
    VisitEnrichmentRecord, content_fetch_schedule_eta_secs as core_content_fetch_schedule_eta_secs,
    content_fetch_settings as core_content_fetch_settings, drain_one_content_fetch_job,
    enqueue_content_fetch_now as core_enqueue_content_fetch_now,
    enqueue_content_fetch_working_set as core_enqueue_content_fetch_working_set,
    list_visit_enrichment as core_list_visit_enrichment,
};

/// Single-worker content-fetch lane counter (LOW concurrency, 06 §5).
///
/// Only the production [`spawn_content_fetch_drain`] (the real worker pool) needs it; the coverage
/// build drives the lane inline (no pool), so it is `#[cfg(not(coverage))]` to stay dead-code clean.
#[cfg(not(coverage))]
static CONTENT_FETCH_WORKERS: AtomicUsize = AtomicUsize::new(0);

/// Default bulk-enqueue cap for the working-set-prioritized content fetch (bounded, 06 §5).
const DEFAULT_WORKING_SET_ENQUEUE_LIMIT: usize = 2_000;

/// Hard cap on how long the lane sleeps waiting for a deferred (rate-limited) job (SEC-2).
///
/// GitHub's 60/hr refill ETA is ≈ 60s, so capping the wait at 60s keeps the lane responsive (a config
/// change / pause is re-checked at least every minute) while still letting a deferred fetch complete
/// without a user action.
const MAX_DEFERRED_SLEEP_SECS: u64 = 60;

/// Returns the Settings-facing content-fetch consent + status surface.
pub fn content_fetch_settings(session_database_key: Option<&str>) -> Result<ContentFetchSettings> {
    let paths = vault_core::project_paths()?;
    let config = load_unlocked_config(&paths)?;
    core_content_fetch_settings(&paths, &config, session_database_key)
}

/// Persists the content-fetch consent settings (master switch + per-extractor + per-domain) and, when
/// turning the master switch ON, kicks the working-set enqueue + drain so fetching starts immediately.
///
/// Returns a fresh [`AppSnapshot`] so the shell sees the updated config. Writing the config is the
/// consent gate: nothing is fetched until `enabled` is true here (and persisted).
pub fn set_content_fetch_settings(
    settings: &ContentFetchSettings,
    session_database_key: Option<&str>,
) -> Result<AppSnapshot> {
    let paths = vault_core::project_paths()?;
    let mut config = load_unlocked_config(&paths)?;
    config.ai.content_fetch_enabled = settings.enabled;
    config.ai.content_fetch_extractors = settings.extractors.clone();
    config.ai.content_fetch_domains = settings.domains.clone();
    crate::app::save_user_config(&config, session_database_key)?;

    // On enabling, prime the queue from the prioritized working set + start the drain so the user sees
    // enrichment appear without a manual trigger. Best-effort: a priming failure must not fail the
    // settings save (the consent flag is already persisted).
    if settings.enabled {
        let _ = enqueue_content_fetch_working_set(
            DEFAULT_WORKING_SET_ENQUEUE_LIMIT,
            session_database_key,
        );
        maybe_spawn_content_fetch_drain(&paths, &config, session_database_key);
    }
    crate::app::app_snapshot(session_database_key)
}

/// Lists the stored content enrichment for one visit/history id (the detail-panel read, 06 §6).
pub fn list_visit_enrichment(
    history_id: i64,
    session_database_key: Option<&str>,
) -> Result<Vec<VisitEnrichmentRecord>> {
    let paths = vault_core::project_paths()?;
    let config = load_unlocked_config(&paths)?;
    core_list_visit_enrichment(&paths, &config, session_database_key, history_id)
}

/// Triggers the manual "fetch now" PME for one URL, then starts the drain so it runs promptly.
pub fn content_fetch_now(
    request: &ContentFetchNowRequest,
    session_database_key: Option<&str>,
) -> Result<ContentFetchNowResult> {
    let paths = vault_core::project_paths()?;
    let config = load_unlocked_config(&paths)?;
    let result = core_enqueue_content_fetch_now(&paths, &config, session_database_key, request)?;
    if result.state == "queued" {
        maybe_spawn_content_fetch_drain(&paths, &config, session_database_key);
    }
    Ok(result)
}

/// Enqueues the prioritized working set for content fetch (the 06 §5 bulk hook) + starts the drain.
///
/// Returns the number of jobs enqueued. A no-op (returns 0) when the master switch is off.
pub fn enqueue_content_fetch_working_set(
    limit: usize,
    session_database_key: Option<&str>,
) -> Result<usize> {
    let paths = vault_core::project_paths()?;
    let config = load_unlocked_config(&paths)?;
    let enqueued =
        core_enqueue_content_fetch_working_set(&paths, &config, session_database_key, limit)?;
    if enqueued > 0 {
        maybe_spawn_content_fetch_drain(&paths, &config, session_database_key);
    }
    Ok(enqueued)
}

/// Spawns the single content-fetch drain worker when fetching is enabled + not paused (06 §5).
///
/// LOW concurrency (one worker): network-bound, rate-limited work must not fan out on a 4-core host.
/// Off the backup/import critical path — only Settings / manual-trigger / post-enable flows start it.
pub(crate) fn maybe_spawn_content_fetch_drain(
    paths: &vault_core::ProjectPaths,
    config: &AppConfig,
    session_database_key: Option<&str>,
) {
    if !config.ai.content_fetch_enabled || config.ai.job_queue_paused {
        return;
    }
    let paths = paths.clone();
    let session_database_key = session_database_key.map(ToOwned::to_owned);
    spawn_content_fetch_drain(paths, session_database_key);
}

/// Production driver: runs the drain lane on the bounded worker pool with the REAL per-iteration step
/// and a REAL sleep between deferred jobs.
///
/// Split from [`maybe_spawn_content_fetch_drain`] so the only irreducible real-thread/real-sleep code
/// is these two thunks. The loop body itself (the cap + arm dispatch) lives in [`run_content_fetch_lane`]
/// so it is unit-tested without a thread; this thunk wiring is `#[cfg(not(coverage))]` (mirroring
/// `chat.rs`/`model_download.rs`: the real spawn is compiled out of the coverage build, and the
/// `#[cfg(coverage)]` twin below drives the SAME seam inline so the 100% gate is deterministic).
#[cfg(not(coverage))]
fn spawn_content_fetch_drain(
    paths: vault_core::ProjectPaths,
    session_database_key: Option<String>,
) {
    maybe_spawn_worker_pool("pathkeep-content-fetch", &CONTENT_FETCH_WORKERS, 1, move || {
        run_content_fetch_lane(
            || drain_content_fetch_lane_step(&paths, session_database_key.as_deref()),
            |secs| std::thread::sleep(std::time::Duration::from_secs(secs)),
        );
    });
}

/// Coverage twin of [`spawn_content_fetch_drain`]: drives the lane INLINE (no background thread, no real
/// sleep) so the driver wiring — the real step thunk, the sleep thunk, and the [`run_content_fetch_lane`]
/// call — is exercised deterministically at 100% coverage without depending on thread scheduling.
///
/// The step is wrapped so that after the lane sleeps ONCE it reports [`LaneStep::Stop`], which bounds the
/// drive to a single deferred-sleep pass. That keeps a persistently rate-limited job from spinning the
/// inline drive forever while still flowing through the SAME seam + cap the production thread uses.
#[cfg(coverage)]
fn spawn_content_fetch_drain(
    paths: vault_core::ProjectPaths,
    session_database_key: Option<String>,
) {
    let slept = std::cell::Cell::new(false);
    run_content_fetch_lane(
        || {
            if slept.get() {
                return LaneStep::Stop;
            }
            drain_content_fetch_lane_step(&paths, session_database_key.as_deref())
        },
        |_secs| slept.set(true),
    );
}

/// Runs the content-fetch drain loop, dispatching each [`LaneStep`] (the SEC-2 deferred-job sleep cap
/// lives HERE so it is unit-tested without a thread).
///
/// Injectable seam: `step` produces one lane decision per iteration (the production driver calls
/// [`drain_content_fetch_lane_step`]) and `sleep` performs the deferred wait (the production driver calls
/// [`std::thread::sleep`]). A `Drained` step loops immediately; an `Idle { Some(secs) }` sleeps for the
/// deferred ETA CAPPED at [`MAX_DEFERRED_SLEEP_SECS`] (so a config change / pause is re-checked at least
/// every minute) then loops; an `Idle { None }` or `Stop` exits the lane.
pub(crate) fn run_content_fetch_lane(
    mut step: impl FnMut() -> LaneStep,
    mut sleep: impl FnMut(u64),
) {
    loop {
        match step() {
            // A job ran → loop immediately for the next due one.
            LaneStep::Drained => continue,
            // Nothing due now but a job is deferred (rate-limited) → sleep until it is due, then loop
            // so the deferred work completes without a user action (SEC-2). Cap the wait so a paused /
            // disabled config is observed within the cap on the next iteration.
            LaneStep::Idle { sleep_secs: Some(secs) } => sleep(secs.min(MAX_DEFERRED_SLEEP_SECS)),
            // Truly idle (no due + no deferred work) or stopped → exit the lane.
            LaneStep::Idle { sleep_secs: None } | LaneStep::Stop => break,
        }
    }
}

/// Outcome of one content-fetch lane iteration (testable PURE-of-spawn, SEC-2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LaneStep {
    /// A job was drained — loop immediately for the next.
    Drained,
    /// No job is due right now. `sleep_secs` is `Some` when a deferred job is pending (sleep then
    /// loop) and `None` when the queue is genuinely empty (exit the lane).
    Idle { sleep_secs: Option<u64> },
    /// Config could not load, or fetching was disabled/paused → stop the lane.
    Stop,
}

/// One iteration of the content-fetch drain loop (config reload + pause/consent recheck + drain).
///
/// Extracted PURE-of-spawn so the loop body is unit-tested directly without standing up a worker thread
/// (mirrors `runtime.rs`'s `drain_one_*` seam). Returns [`LaneStep::Stop`] when config can't load or
/// fetching was disabled/paused; [`LaneStep::Drained`] when a job ran; [`LaneStep::Idle`] otherwise,
/// carrying the deferred-job sleep ETA (`Some` = a rate-limited job is waiting, sleep then loop; `None`
/// = the queue is empty, exit). A drain error is logged + treated as idle-empty (stop) so a transient
/// fault never spins the lane.
fn drain_content_fetch_lane_step(
    paths: &vault_core::ProjectPaths,
    session_database_key: Option<&str>,
) -> LaneStep {
    let config = match load_unlocked_config(paths) {
        Ok(config) => config,
        Err(_) => return LaneStep::Stop,
    };
    if !config.initialized || !config.ai.content_fetch_enabled || config.ai.job_queue_paused {
        return LaneStep::Stop;
    }
    match drain_one_content_fetch_job(paths, &config, session_database_key) {
        Ok(true) => LaneStep::Drained,
        Ok(false) => {
            // Nothing due NOW. If a job was deferred (rate-limited) with a future schedule, report its
            // ETA so the lane sleeps until then instead of exiting; otherwise the queue is empty.
            let sleep_secs =
                core_content_fetch_schedule_eta_secs(paths, &config, session_database_key)
                    .unwrap_or(None);
            LaneStep::Idle { sleep_secs }
        }
        Err(error) => {
            eprintln!("PathKeep could not drain content-fetch work: {error:#}");
            LaneStep::Idle { sleep_secs: None }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::{PROJECT_ROOT_OVERRIDE_ENV, lock_env, restore_env_var};
    use vault_core::{ArchiveMode, ContentFetchNowRequest};

    fn consenting_config() -> AppConfig {
        let mut config = AppConfig {
            initialized: true,
            archive_mode: ArchiveMode::Plaintext,
            ..AppConfig::default()
        };
        config.ai.content_fetch_enabled = true;
        config
    }

    /// Drives the worker content-fetch entry points against an isolated, initialized project root so
    /// the thin delegation (paths resolution + config read/write + DTO shaping + lane step) is covered
    /// without a desktop harness. Uses the crate-wide env lock to avoid racing on `CHB_PROJECT_ROOT`.
    #[test]
    fn worker_content_fetch_entrypoints_cover_consent_read_write_and_lane() {
        let dir = tempfile::tempdir().expect("tempdir");
        let _guard = lock_env();
        let original = std::env::var_os(PROJECT_ROOT_OVERRIDE_ENV);
        unsafe {
            std::env::set_var(PROJECT_ROOT_OVERRIDE_ENV, dir.path());
        }

        // Bootstrap a plaintext archive so the intelligence connection can attach it. The bootstrap
        // config leaves content fetch OFF (the hard default) so the first read reflects disabled.
        let bootstrap = AppConfig {
            initialized: true,
            archive_mode: ArchiveMode::Plaintext,
            ..AppConfig::default()
        };
        crate::app::initialize_archive_database(&bootstrap, None).expect("initialize archive");

        // Default (master OFF until persisted): the read surface reports disabled + zero jobs.
        let off_settings = content_fetch_settings(None).expect("settings off");
        assert!(!off_settings.enabled);
        assert_eq!(off_settings.queued_jobs, 0);

        // A manual fetch-now while disabled is parked (no job).
        let disabled = content_fetch_now(
            &ContentFetchNowRequest {
                history_id: 1,
                profile_id: "chrome:Default".to_string(),
                url: "https://github.com/o/r".to_string(),
                title: None,
            },
            None,
        )
        .expect("fetch now disabled");
        assert_eq!(disabled.state, "disabled");

        // The working-set enqueue is a no-op while disabled.
        assert_eq!(enqueue_content_fetch_working_set(10, None).expect("ws disabled"), 0);

        // Persist consent ON via the settings writer (the consent gate), then the read surface flips.
        let mut on = off_settings.clone();
        on.enabled = true;
        let snapshot = set_content_fetch_settings(&on, None).expect("enable");
        assert!(snapshot.config.ai.content_fetch_enabled);
        let on_settings = content_fetch_settings(None).expect("settings on");
        assert!(on_settings.enabled);

        // A manual fetch-now now enqueues a job (covers the enabled branch + drain spawn kick).
        let queued = content_fetch_now(
            &ContentFetchNowRequest {
                history_id: 2,
                profile_id: "chrome:Default".to_string(),
                url: "https://github.com/o/r".to_string(),
                title: Some("o/r".to_string()),
            },
            None,
        )
        .expect("fetch now enabled");
        assert_eq!(queued.state, "queued");

        // The detail-panel read returns an empty list for an unfetched visit (never blocks on net).
        assert!(list_visit_enrichment(99, None).expect("list").is_empty());

        let paths = vault_core::project_paths().expect("paths");

        // The spawn-kicking calls above drove the drain lane (the production build on a background
        // thread; the coverage build inline, deterministically), so it may have already claimed the
        // queued job. PAUSE the queue so any production-build background lane Stops on its next
        // iteration, then clear the queue, giving the explicit lane-step assertion below a deterministic,
        // uncontended starting state (no racing background drain).
        let mut paused = consenting_config();
        paused.ai.job_queue_paused = true;
        crate::app::save_user_config(&paused, None).expect("save paused");
        // The paused lane reports Stop (covers the disabled/paused early-return branch) and, once any
        // background lane observes the pause + exits, the queue is ours to control.
        assert_eq!(
            drain_content_fetch_lane_step(&paths, None),
            LaneStep::Stop,
            "paused lane stops"
        );
        let connection =
            rusqlite::Connection::open(&paths.intelligence_database_path).expect("intelligence db");
        // Let any in-flight background drain (production build) settle, then clear the queue
        // deterministically. Under coverage the lane already ran inline, so this clears immediately.
        for _ in 0..50 {
            std::thread::sleep(std::time::Duration::from_millis(10));
            connection
                .execute("DELETE FROM intelligence_jobs WHERE job_type = 'content-fetch'", [])
                .expect("clear queue");
            let running: i64 = connection
                .query_row(
                    "SELECT COUNT(*) FROM intelligence_jobs WHERE job_type = 'content-fetch' AND state = 'running'",
                    [],
                    |row| row.get(0),
                )
                .expect("count running");
            if running == 0 {
                break;
            }
        }

        // Re-enable (unpaused) WITHOUT spawning a lane (write the config directly), enqueue exactly one
        // DUE job, and assert the explicit lane step finds + RUNS it → `Drained`. With no background lane
        // competing, this is deterministic. The unique host keeps the job off the shared github bucket.
        let mut on_cfg = consenting_config();
        on_cfg.ai.job_queue_paused = false;
        vault_core::save_config(&paths, &on_cfg).expect("persist unpaused consent");
        let host = format!("worker-drain-{}.example", std::process::id());
        vault_core::enqueue_content_fetch_now(
            &paths,
            &on_cfg,
            None,
            &ContentFetchNowRequest {
                history_id: 3,
                profile_id: "chrome:Default".to_string(),
                url: format!("https://{host}/post"),
                title: None,
            },
        )
        .expect("enqueue due job");
        assert_eq!(
            drain_content_fetch_lane_step(&paths, None),
            LaneStep::Drained,
            "a queued, un-throttled job is drained (ran)"
        );

        restore_env_var(PROJECT_ROOT_OVERRIDE_ENV, original.as_deref());
    }

    /// Covers `maybe_spawn_content_fetch_drain`'s disabled/paused early-return + the working-set enqueue
    /// drain-spawn kick (enqueued > 0) + the lane step's config-load-error Stop branch.
    #[test]
    fn drain_spawn_guards_and_working_set_kick_and_config_error_lane() {
        use rusqlite::Connection;

        let dir = tempfile::tempdir().expect("tempdir");
        let _guard = lock_env();
        let original = std::env::var_os(PROJECT_ROOT_OVERRIDE_ENV);
        unsafe {
            std::env::set_var(PROJECT_ROOT_OVERRIDE_ENV, dir.path());
        }

        let config = consenting_config();
        crate::app::initialize_archive_database(&config, None).expect("initialize archive");
        let paths = vault_core::project_paths().expect("paths");

        // maybe_spawn early-returns when fetching is disabled (covers the guard) — no thread spawned.
        let mut off = config.clone();
        off.ai.content_fetch_enabled = false;
        maybe_spawn_content_fetch_drain(&paths, &off, None);
        // …and when the queue is paused.
        let mut paused = config.clone();
        paused.ai.job_queue_paused = true;
        maybe_spawn_content_fetch_drain(&paths, &paused, None);

        // Seed a starred, due github URL so the working-set enqueue returns > 0 and kicks the drain
        // spawn (covers the `enqueued > 0 => maybe_spawn` line).
        {
            let archive = vault_core::archive::open_archive_connection(&paths, &config, None)
                .expect("archive");
            archive
                .execute(
                    "INSERT OR IGNORE INTO runs (id, run_type, trigger, started_at, timezone, status, profile_scope_json, warnings_json, stats_json, due_only)
                     VALUES (1, 'backup', 'test', '2026-01-01', 'UTC', 'success', '[]', '[]', '{}', 0)",
                    [],
                )
                .expect("run");
            archive
                .execute(
                    "INSERT OR IGNORE INTO source_profiles (id, browser_kind, browser_version, profile_name, profile_path, discovered_at, enabled, profile_key, updated_at)
                     VALUES (1, 'chrome', 'x', 'p', '/p', '2026-01-01', 1, 'chrome:Default', '2026-01-01')",
                    [],
                )
                .expect("profile");
            archive
                .execute(
                    "INSERT INTO urls (id, url, title, visit_count, typed_count, first_visit_ms, first_visit_iso, last_visit_ms, last_visit_iso, source_profile_id, created_by_run_id)
                     VALUES (1, 'https://github.com/o/r', 'o/r', 1, 0, 1, '', 1, '', 1, 1)",
                    [],
                )
                .expect("url");
            archive
                .execute(
                    "INSERT INTO visits (id, url_id, source_profile_id, visit_time_ms, visit_time_iso, created_by_run_id)
                     VALUES (1, 1, 1, 1, '2026-01-01T00:00:00Z', 1)",
                    [],
                )
                .expect("visit");
            archive
                .execute(
                    "INSERT INTO star (entity_kind, entity_key, starred_at) VALUES ('url', 'https://github.com/o/r', '2026-01-01')",
                    [],
                )
                .expect("star");
        }
        // Persist consent (master ON) by writing the config JSON directly (the snapshot-rebuilding
        // `save_user_config` is irrelevant here and brittle on a fresh queue), and PAUSE the queue so
        // the drain spawn the enqueue kicks immediately Stops (no CPU-spinning lane under the test
        // stub). `enqueue_content_fetch_working_set` enqueues regardless of the pause.
        let mut on_cfg = config.clone();
        on_cfg.ai.content_fetch_enabled = true;
        on_cfg.ai.job_queue_paused = true;
        vault_core::save_config(&paths, &on_cfg).expect("persist consent + pause");
        let enqueued = enqueue_content_fetch_working_set(100, None).expect("ws enqueue");
        assert_eq!(enqueued, 1, "a starred due URL enqueues, kicking the drain spawn");
        // Drop the queued job so no spinning thread can churn on it under the test stub.
        {
            let connection =
                Connection::open(&paths.intelligence_database_path).expect("intelligence db");
            connection
                .execute("DELETE FROM intelligence_jobs WHERE job_type = 'content-fetch'", [])
                .expect("clear queue");
        }

        // Lane-step Stop when config cannot LOAD: point at a fresh root + write a CORRUPT config so
        // `load_unlocked_config` returns Err (a missing config would parse to an uninitialized default,
        // which Stops at a different branch). This exercises the `Err(_) => Stop` arm.
        let bad = tempfile::tempdir().expect("bad root");
        unsafe {
            std::env::set_var(PROJECT_ROOT_OVERRIDE_ENV, bad.path());
        }
        let bad_paths = vault_core::project_paths().expect("bad paths");
        std::fs::create_dir_all(bad_paths.config_path.parent().unwrap()).expect("config dir");
        std::fs::write(&bad_paths.config_path, "{ not valid json").expect("write corrupt config");
        assert_eq!(
            drain_content_fetch_lane_step(&bad_paths, None),
            LaneStep::Stop,
            "a lane step whose config cannot load stops"
        );

        restore_env_var(PROJECT_ROOT_OVERRIDE_ENV, original.as_deref());
    }

    /// Covers [`run_content_fetch_lane`]'s loop body WITHOUT a thread (SEC-2): a `Drained` step loops,
    /// an `Idle { Some(secs) }` sleeps for `secs` CAPPED at [`MAX_DEFERRED_SLEEP_SECS`], and the loop
    /// exits on `Idle { None }`. Drives a deterministic scripted step sequence + a recording sleep so
    /// the former in-thread cap + sleep arm (the flaky line) is now exercised deterministically.
    #[test]
    fn run_content_fetch_lane_caps_deferred_sleeps_and_exits_on_idle_none() {
        let steps = std::cell::RefCell::new(vec![
            // empty -> exit (popped last)
            LaneStep::Idle { sleep_secs: None },
            // below the cap -> sleeps 10
            LaneStep::Idle { sleep_secs: Some(10) },
            // above the cap -> sleeps MAX_DEFERRED_SLEEP_SECS (60)
            LaneStep::Idle { sleep_secs: Some(999) },
            // a job ran -> loops immediately
            LaneStep::Drained,
        ]);
        let slept: std::cell::RefCell<Vec<u64>> = std::cell::RefCell::new(Vec::new());

        run_content_fetch_lane(
            || steps.borrow_mut().pop().expect("step sequence underran"),
            |secs| slept.borrow_mut().push(secs),
        );

        // The 999s ETA was capped to 60 and the 10s ETA passed through; the empty step exited the loop.
        assert_eq!(slept.into_inner(), vec![MAX_DEFERRED_SLEEP_SECS, 10]);
        assert!(steps.borrow().is_empty(), "the whole sequence was consumed (Drained looped)");
    }

    /// Covers the coverage-build [`spawn_content_fetch_drain`] stub's single deferred-sleep pass: with a
    /// deferred (future-scheduled) job present, the inline drive sleeps ONCE (the no-op sleep flips the
    /// stop flag) then the wrapped step reports `Stop`, so the drive returns instead of spinning. This is
    /// `#[cfg(coverage)]`-gated because the stub only exists in the coverage build; under a normal build
    /// the production thread variant runs and this branch is compiled out.
    #[cfg(coverage)]
    #[test]
    fn coverage_drain_stub_sleeps_once_for_a_deferred_job_then_stops() {
        use rusqlite::Connection;

        let dir = tempfile::tempdir().expect("tempdir");
        let _guard = lock_env();
        let original = std::env::var_os(PROJECT_ROOT_OVERRIDE_ENV);
        unsafe {
            std::env::set_var(PROJECT_ROOT_OVERRIDE_ENV, dir.path());
        }

        let config = consenting_config();
        crate::app::initialize_archive_database(&config, None).expect("initialize archive");
        let paths = vault_core::project_paths().expect("paths");
        vault_core::save_config(&paths, &config).expect("persist consent");

        // Enqueue a job and push its `scheduled_at` into the future so the lane finds nothing DUE but a
        // DEFERRED job pending → the inline drive's first step reports `Idle { Some(_) }`, exercising the
        // stub's sleep thunk + the post-sleep `Stop` arm.
        vault_core::enqueue_content_fetch_now(
            &paths,
            &config,
            None,
            &ContentFetchNowRequest {
                history_id: 1,
                profile_id: "chrome:Default".to_string(),
                url: "https://github.com/o/r".to_string(),
                title: None,
            },
        )
        .expect("enqueue deferred job");
        let future = (chrono::Utc::now() + chrono::Duration::seconds(30))
            .to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
        {
            let connection =
                Connection::open(&paths.intelligence_database_path).expect("intelligence db");
            connection
                .execute(
                    "UPDATE intelligence_jobs SET scheduled_at = ?1 WHERE job_type = 'content-fetch'",
                    [&future],
                )
                .expect("defer the job");
        }

        // The inline drive must RETURN (the bounded single sleep pass keeps a deferred job from spinning
        // the coverage stub forever). The job stays deferred (untouched) afterwards.
        spawn_content_fetch_drain(paths.clone(), None);

        restore_env_var(PROJECT_ROOT_OVERRIDE_ENV, original.as_deref());
    }

    /// Covers [`run_content_fetch_lane`]'s `Stop` exit arm (config gone / paused) without a thread.
    #[test]
    fn run_content_fetch_lane_exits_on_stop() {
        let mut slept = Vec::new();
        run_content_fetch_lane(|| LaneStep::Stop, |secs| slept.push(secs));
        assert!(slept.is_empty(), "a Stop step exits the lane without sleeping");
    }

    /// Covers the lane's IDLE arms (SEC-2): a future-scheduled (deferred) job makes the step report
    /// `Idle { sleep_secs: Some(_) }` (sleep then loop); an empty queue reports `Idle { None }` (exit).
    #[test]
    fn lane_step_reports_idle_with_deferred_eta_then_empty() {
        use rusqlite::Connection;

        let dir = tempfile::tempdir().expect("tempdir");
        let _guard = lock_env();
        let original = std::env::var_os(PROJECT_ROOT_OVERRIDE_ENV);
        unsafe {
            std::env::set_var(PROJECT_ROOT_OVERRIDE_ENV, dir.path());
        }

        crate::app::initialize_archive_database(&consenting_config(), None)
            .expect("initialize archive");

        // Persist consent ON via the settings writer so the lane step sees fetching enabled.
        let mut on = content_fetch_settings(None).expect("settings");
        on.enabled = true;
        set_content_fetch_settings(&on, None).expect("enable");

        let paths = vault_core::project_paths().expect("paths");

        // Enqueue one content-fetch job, then push its `scheduled_at` into the future so it is NOT due
        // (mirrors a rate-limit requeue): the drain finds nothing due, and the schedule-ETA query
        // surfaces it → Idle with a Some(_) sleep. Enqueue via the CORE function (not the worker's
        // `content_fetch_now`, which kicks the drain) so the job is not consumed before we defer it —
        // under the coverage build that kick drives the lane inline and would otherwise drain the job.
        let on_cfg = consenting_config();
        let queued = vault_core::enqueue_content_fetch_now(
            &paths,
            &on_cfg,
            None,
            &ContentFetchNowRequest {
                history_id: 1,
                profile_id: "chrome:Default".to_string(),
                url: "https://github.com/o/r".to_string(),
                title: None,
            },
        )
        .expect("fetch now");
        assert_eq!(queued.state, "queued");

        let future = (chrono::Utc::now() + chrono::Duration::seconds(30))
            .to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
        {
            let connection =
                Connection::open(&paths.intelligence_database_path).expect("intelligence db");
            connection
                .execute(
                    "UPDATE intelligence_jobs SET scheduled_at = ?1 WHERE job_type = 'content-fetch'",
                    [&future],
                )
                .expect("defer the job");
        }

        match drain_content_fetch_lane_step(&paths, None) {
            LaneStep::Idle { sleep_secs: Some(secs) } => {
                assert!((1..=31).contains(&secs), "deferred ETA should be ~30s, got {secs}");
            }
            other => panic!("expected Idle with a deferred ETA, got {other:?}"),
        }

        // Drop the deferred job → the queue is empty → Idle with no sleep (the lane exits).
        {
            let connection =
                Connection::open(&paths.intelligence_database_path).expect("intelligence db");
            connection
                .execute("DELETE FROM intelligence_jobs WHERE job_type = 'content-fetch'", [])
                .expect("clear queue");
        }
        assert_eq!(
            drain_content_fetch_lane_step(&paths, None),
            LaneStep::Idle { sleep_secs: None },
            "an empty queue is idle with nothing to wait on"
        );

        restore_env_var(PROJECT_ROOT_OVERRIDE_ENV, original.as_deref());
    }
}
