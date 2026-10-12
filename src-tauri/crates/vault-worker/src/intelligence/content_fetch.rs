//! Failure modes: stale consent permits a drain, parked work is never reoffered, or a page size truncates candidate coverage.
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
use crate::job_runtime::{WorkerPool, maybe_spawn_worker_pool};
use anyhow::Result;
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
/// Coverage drives the lane inline; the shared shutdown counter remains zero there.
pub(super) static CONTENT_FETCH_WORKERS: WorkerPool = WorkerPool::new();

/// Default bulk-enqueue cap for the working-set-prioritized content fetch (bounded, 06 §5).
const DEFAULT_WORKING_SET_PAGE_SIZE: usize = 2_000;

/// Hard cap on how long the lane sleeps waiting for a deferred (rate-limited) job.
///
/// GitHub's 60/hr refill ETA is ≈ 60s, so capping the wait at 60s keeps the lane responsive (a config
/// change / pause is re-checked at least every minute) while still letting a deferred fetch complete
/// without a user action.
const MAX_DEFERRED_SLEEP_SECS: u64 = 60;

/// Returns the Settings-facing content-fetch consent + status surface.
///
/// Like the other queue status reads (`load_ai_queue`, `load_intelligence_runtime_snapshot`), it also
/// restarts the drain when queued work is waiting: the lane exits while the queue is paused, and
/// resuming only saves the config, so this read is what gets parked fetches moving again.
pub fn content_fetch_settings(vault: &vault_core::Vault) -> Result<ContentFetchSettings> {
    let _paths = vault.paths();
    let config = load_unlocked_config(vault.paths())?;
    let settings = core_content_fetch_settings(&vault.with_config(config.clone()))?;
    if settings.queued_jobs > 0 {
        maybe_spawn_content_fetch_drain(vault, &config);
    }
    Ok(settings)
}

/// Persists the content-fetch consent settings (master switch + per-extractor + per-domain) and, when
/// turning the master switch ON, kicks the working-set enqueue + drain so fetching starts immediately.
///
/// Returns a fresh [`AppSnapshot`] so the shell sees the updated config. Writing the config is the
/// consent gate: nothing is fetched until `enabled` is true here (and persisted).
pub fn set_content_fetch_settings(
    vault: &vault_core::Vault,
    settings: &ContentFetchSettings,
) -> Result<AppSnapshot> {
    let _paths = vault.paths();
    let mut config = load_unlocked_config(vault.paths())?;
    config.ai.content_fetch_enabled = settings.enabled;
    config.ai.content_fetch_extractors = settings.extractors.clone();
    config.ai.content_fetch_domains = settings.domains.clone();
    crate::app::save_user_config_with_vault(Some(vault), &config, None, None)?;

    // On enabling, prime the queue from the prioritized working set + start the drain so the user sees
    // enrichment appear without a manual trigger. Best-effort: a priming failure must not fail the
    // settings save (the consent flag is already persisted).
    if settings.enabled {
        let _ = enqueue_content_fetch_working_set(
            &vault.with_config(config.clone()),
            DEFAULT_WORKING_SET_PAGE_SIZE,
        );
        maybe_spawn_content_fetch_drain(vault, &config);
    }
    crate::app::app_snapshot_with_vault(Some(vault), None)
}

/// Lists the stored content enrichment for one visit/history id (the detail-panel read, 06 §6).
pub fn list_visit_enrichment(
    vault: &vault_core::Vault,
    history_id: i64,
) -> Result<Vec<VisitEnrichmentRecord>> {
    let _paths = vault.paths();
    let _config = load_unlocked_config(vault.paths())?;
    core_list_visit_enrichment(vault, history_id)
}

/// Triggers the manual "fetch now" PME for one URL, then starts the drain so it runs promptly.
pub fn content_fetch_now(
    vault: &vault_core::Vault,
    request: &ContentFetchNowRequest,
) -> Result<ContentFetchNowResult> {
    let _paths = vault.paths();
    let config = load_unlocked_config(vault.paths())?;
    let result = core_enqueue_content_fetch_now(&vault.with_config(config.clone()), request)?;
    if result.state == "queued" {
        maybe_spawn_content_fetch_drain(vault, &config);
    }
    Ok(result)
}

/// Enqueues the prioritized working set for content fetch (the 06 §5 bulk hook) + starts the drain.
///
/// Returns the number of jobs enqueued. A no-op (returns 0) when the master switch is off.
pub fn enqueue_content_fetch_working_set(vault: &vault_core::Vault, limit: usize) -> Result<usize> {
    let _paths = vault.paths();
    let config = load_unlocked_config(vault.paths())?;
    let enqueued =
        core_enqueue_content_fetch_working_set(&vault.with_config(config.clone()), limit)?;
    if enqueued > 0 {
        maybe_spawn_content_fetch_drain(vault, &config);
    }
    Ok(enqueued)
}

/// Spawns the single content-fetch drain worker when fetching is enabled + not paused (06 §5).
///
/// LOW concurrency (one worker): network-bound, rate-limited work must not fan out on a 4-core host.
/// Off the backup/import critical path — only Settings / manual-trigger / post-enable flows start it.
pub(crate) fn maybe_spawn_content_fetch_drain(vault: &vault_core::Vault, config: &AppConfig) {
    if !config.ai.content_fetch_enabled || config.ai.job_queue_paused {
        return;
    }
    spawn_content_fetch_drain(&vault.with_config(config.clone()));
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
fn spawn_content_fetch_drain(vault: &vault_core::Vault) {
    let vault = vault.clone();
    maybe_spawn_worker_pool("pathkeep-content-fetch", &CONTENT_FETCH_WORKERS, 1, move || {
        run_content_fetch_lane(
            || drain_content_fetch_lane_step(&vault),
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
fn spawn_content_fetch_drain(vault: &vault_core::Vault) {
    let slept = std::cell::Cell::new(false);
    run_content_fetch_lane(
        || {
            if slept.get() {
                return LaneStep::Stop;
            }
            drain_content_fetch_lane_step(vault)
        },
        |_secs| slept.set(true),
    );
}

/// Runs the content-fetch drain loop, dispatching each [`LaneStep`] (the deferred-job sleep cap
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
            // so the deferred work completes without a user action. Cap the wait so a paused /
            // disabled config is observed within the cap on the next iteration.
            LaneStep::Idle { sleep_secs: Some(secs) } => sleep(secs.min(MAX_DEFERRED_SLEEP_SECS)),
            // Truly idle (no due + no deferred work) or stopped → exit the lane.
            LaneStep::Idle { sleep_secs: None } | LaneStep::Stop => break,
        }
    }
}

/// Outcome of one content-fetch lane iteration (testable PURE-of-spawn).
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
fn drain_content_fetch_lane_step(vault: &vault_core::Vault) -> LaneStep {
    let config = match load_unlocked_config(vault.paths()) {
        Ok(config) => config,
        Err(_) => return LaneStep::Stop,
    };
    if !config.initialized || !config.ai.content_fetch_enabled || config.ai.job_queue_paused {
        return LaneStep::Stop;
    }
    match drain_one_content_fetch_job(&vault.with_config(config.clone())) {
        Ok(true) => LaneStep::Drained,
        Ok(false) => {
            // Nothing due NOW. If a job was deferred (rate-limited) with a future schedule, report its
            // ETA so the lane sleeps until then instead of exiting; otherwise the queue is empty.
            let sleep_secs =
                core_content_fetch_schedule_eta_secs(&vault.with_config(config.clone()))
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
    use vault_core::{
        ArchiveMode, ContentFetchNowRequest, Vault, test_support::native::TestDatabase,
    };
    use vault_store::{Db, Priority, params_owned as params};

    struct Fixture {
        vault: Vault,
        previous: Option<std::ffi::OsString>,
        _guard: std::sync::MutexGuard<'static, ()>,
    }
    impl Fixture {
        fn new(config: AppConfig) -> Self {
            let guard = lock_env();
            let vault = Vault::open_temp().unwrap().with_config(config);
            vault_core::save_config(vault.paths(), vault.config()).unwrap();
            let previous = std::env::var_os(PROJECT_ROOT_OVERRIDE_ENV);
            unsafe {
                std::env::set_var(PROJECT_ROOT_OVERRIDE_ENV, &vault.paths().app_root);
            }
            Self { vault, previous, _guard: guard }
        }
        fn db(&self) -> TestDatabase {
            TestDatabase::new(&self.vault, Db::Intelligence)
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            restore_env_var(PROJECT_ROOT_OVERRIDE_ENV, self.previous.as_deref());
        }
    }
    fn consenting_config() -> AppConfig {
        let mut config = AppConfig {
            initialized: true,
            archive_mode: ArchiveMode::Plaintext,
            ..Default::default()
        };
        config.ai.content_fetch_enabled = true;
        config
    }
    fn request(url: &str) -> ContentFetchNowRequest {
        ContentFetchNowRequest { history_id: None, profile_id: None, url: url.into(), title: None }
    }
    fn seed_page(vault: &Vault, url: &str) {
        let url = url.to_owned();
        vault.store().write(Db::Archive,Priority::Batch,move |s| {
            s.execute("INSERT INTO pages(id,url,canonical_url,registrable_domain) VALUES(1,?1,?1,'example.com')",[url.clone()])?;
            s.execute("INSERT INTO search_documents(page_id,url,title,search_terms,normalized_url,normalized_title,normalized_search_terms,cjk_grams,compact_text,updated_at) VALUES(1,?1,'','',?1,'','','','','')",[url.clone()])?;
            s.execute("INSERT INTO star(entity_kind,entity_key,starred_at) VALUES('url',?1,'')",[url])?;Ok(())
        }).unwrap();
    }

    #[test]
    fn worker_content_fetch_entrypoints_cover_consent_read_write_and_lane() {
        let mut config = consenting_config();
        config.ai.content_fetch_enabled = false;
        config.ai.job_queue_paused = true;
        let fixture = Fixture::new(config);
        let vault = &fixture.vault;
        let off = content_fetch_settings(vault).unwrap();
        assert!(!off.enabled);
        assert_eq!(off.queued_jobs, 0);
        assert_eq!(
            content_fetch_now(vault, &request("https://github.com/o/r")).unwrap().state,
            "disabled"
        );
        assert_eq!(enqueue_content_fetch_working_set(vault, 10).unwrap(), 0);
        let mut on = off;
        on.enabled = true;
        let snapshot = set_content_fetch_settings(vault, &on).unwrap();
        assert!(snapshot.config.ai.content_fetch_enabled);
        assert!(content_fetch_settings(vault).unwrap().enabled);
        assert_eq!(
            content_fetch_now(vault, &request("https://github.com/o/r")).unwrap().state,
            "queued"
        );
        assert!(list_visit_enrichment(vault, 99).unwrap().is_empty());
        assert_eq!(drain_content_fetch_lane_step(vault), LaneStep::Stop);
        fixture.db().execute("DELETE FROM intelligence_jobs", ()).unwrap();
        let config = consenting_config();
        vault_core::save_config(vault.paths(), &config).unwrap();
        let url = format!("https://worker-drain-{}.example/post", std::process::id());
        seed_page(vault, &url);
        vault_core::enqueue_content_fetch_now(&vault.with_config(config), &request(&url)).unwrap();
        assert_eq!(drain_content_fetch_lane_step(vault), LaneStep::Drained);
    }

    #[test]
    fn drain_spawn_guards_and_working_set_kick_and_config_error_lane() {
        let mut config = consenting_config();
        config.ai.job_queue_paused = true;
        let fixture = Fixture::new(config.clone());
        let vault = &fixture.vault;
        let mut off = config.clone();
        off.ai.content_fetch_enabled = false;
        maybe_spawn_content_fetch_drain(vault, &off);
        maybe_spawn_content_fetch_drain(vault, &config);
        seed_page(vault, "https://github.com/o/r");
        assert_eq!(enqueue_content_fetch_working_set(vault, 100).unwrap(), 1);
        fixture.db().execute("DELETE FROM intelligence_jobs", ()).unwrap();
        std::fs::write(&vault.paths().config_path, "{ not valid json").unwrap();
        assert_eq!(drain_content_fetch_lane_step(vault), LaneStep::Stop);
    }

    #[test]
    fn run_content_fetch_lane_caps_deferred_sleeps_and_exits_on_idle_none() {
        let steps = std::cell::RefCell::new(vec![
            LaneStep::Idle { sleep_secs: None },
            LaneStep::Idle { sleep_secs: Some(10) },
            LaneStep::Idle { sleep_secs: Some(999) },
            LaneStep::Drained,
        ]);
        let slept = std::cell::RefCell::new(Vec::new());
        run_content_fetch_lane(
            || steps.borrow_mut().pop().unwrap(),
            |secs| slept.borrow_mut().push(secs),
        );
        assert_eq!(slept.into_inner(), vec![MAX_DEFERRED_SLEEP_SECS, 10]);
        assert!(steps.borrow().is_empty());
    }
    #[test]
    fn run_content_fetch_lane_exits_on_stop() {
        let mut slept = Vec::new();
        run_content_fetch_lane(|| LaneStep::Stop, |secs| slept.push(secs));
        assert!(slept.is_empty());
    }

    fn deferred(fixture: &Fixture) -> i64 {
        let vault = &fixture.vault;
        let job = vault_core::enqueue_content_fetch_now(
            &vault.with_config(consenting_config()),
            &request("https://github.com/o/r"),
        )
        .unwrap();
        let future = (chrono::Utc::now() + chrono::Duration::seconds(30))
            .to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
        fixture
            .db()
            .execute(
                "UPDATE intelligence_jobs SET scheduled_at=?1 WHERE id=?2",
                params![future, job.job_id],
            )
            .unwrap();
        job.job_id
    }
    #[cfg(coverage)]
    #[test]
    fn coverage_drain_stub_sleeps_once_for_a_deferred_job_then_stops() {
        let fixture = Fixture::new(consenting_config());
        let id = deferred(&fixture);
        spawn_content_fetch_drain(&fixture.vault);
        assert_eq!(
            fixture
                .db()
                .query_one("SELECT state FROM intelligence_jobs WHERE id=?1", [id], |r| Ok(
                    r.get::<String>(0)?
                ))
                .unwrap(),
            "queued"
        );
    }
    #[test]
    fn lane_step_reports_idle_with_deferred_eta_then_empty() {
        let fixture = Fixture::new(consenting_config());
        deferred(&fixture);
        match drain_content_fetch_lane_step(&fixture.vault) {
            LaneStep::Idle { sleep_secs: Some(secs) } => assert!((1..=31).contains(&secs)),
            other => panic!("expected deferred idle, got {other:?}"),
        }
        fixture.db().execute("DELETE FROM intelligence_jobs", ()).unwrap();
        assert_eq!(
            drain_content_fetch_lane_step(&fixture.vault),
            LaneStep::Idle { sleep_secs: None }
        );
    }
}
