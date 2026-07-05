//! Public façade for the W-ENRICH-1 content-fetch plane (worker + IPC command entrypoints).
//!
//! ## Responsibilities
//! - Expose the `(paths, config, key)`-shaped entrypoints the worker drain + Tauri commands call, so
//!   neither has to construct the intelligence connection or know the queue internals: the worker
//!   drain ([`drain_one_content_fetch_job`]), the working-set-fed bulk enqueue
//!   ([`enqueue_content_fetch_working_set`]), the manual PME "fetch now"
//!   ([`enqueue_content_fetch_now`]), the detail-panel read ([`list_visit_enrichment`]), and the
//!   Settings status ([`content_fetch_settings`]).
//!
//! ## Not responsible for
//! - The actual fetch/extract/store (that is [`super::content_fetch`]), the extractor parsing, or the
//!   consent DECISION (that is `models::content_fetch_allowed` / the runner's gate). This is the thin
//!   wiring layer that opens connections + reads/writes the queue.
//!
//! ## Working-set prioritization hook (06 §5)
//! [`enqueue_content_fetch_working_set`] is the seam doc 06 §5 calls for: it consumes the SAME
//! [`crate::ai::working_set::select_working_set`] ranking the heavy embedding tier uses (starred ≫
//! recent ∪ tagged/noted ∪ high-freq) and enqueues the top URLs that are DUE (no fresh row /
//! negative-cache cooled down). It is the production path that feeds the queue a prioritized,
//! unique-URL set rather than the whole corpus.

use super::content_fetch::{content_fetch_allowed, execute_content_fetch_job_by_id};
use super::extractors::resolve_extractor;
use crate::archive::open_intelligence_connection;
use crate::config::ProjectPaths;
use crate::intelligence_runtime::{
    EnrichmentJobPayload, content_fetch_job_due, enqueue_content_fetch_job,
    next_content_fetch_schedule_eta_secs, next_queued_content_fetch_job,
};
use crate::models::{
    AppConfig, ContentFetchNowRequest, ContentFetchNowResult, ContentFetchSettings,
    VisitEnrichmentRecord, content_fetch_domain_blocked,
};
use crate::utils::url_domain;
use anyhow::Result;
use rusqlite::{OptionalExtension, params};

/// Drains ONE due content-fetch job (the worker's low-concurrency lane, 06 §5).
///
/// Returns `Ok(true)` when a job RAN (a row stored / terminally cancelled, so the worker loops for the
/// next), `Ok(false)` when none was due OR the claimed job was DEFERRED (rate-limited → requeued, SEC-2)
/// so nothing more is due right now and the lane should idle + sleep to the deferred ETA rather than
/// busy-looping. A no-op when the master switch is off (no fetch ever happens without consent). NEVER on
/// the backup/import critical path — the worker spawns this on its own lane.
pub fn drain_one_content_fetch_job(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
) -> Result<bool> {
    if !config.ai.content_fetch_enabled {
        return Ok(false);
    }
    let connection = open_intelligence_connection(paths, config, key)?;
    let Some(job_id) = next_queued_content_fetch_job(&connection)? else {
        return Ok(false);
    };
    // Propagate the run/defer signal: `execute_content_fetch_job_by_id` maps Ran → Ok(true) and both
    // Deferred + NotClaimable → Ok(false) (BUG-3). A deferred (rate-limited) job MUST report false so
    // the worker lane idles + sleeps to its ETA instead of immediately re-looping on the parked row.
    execute_content_fetch_job_by_id(paths, &connection, config, job_id)
}

/// Seconds until the soonest DEFERRED content-fetch job becomes due, or `None` when none is pending.
///
/// The worker drain lane calls this when [`drain_one_content_fetch_job`] reports idle: a rate-limited
/// job is REQUEUED with a future `scheduled_at` (SEC-2), so the lane should sleep until then and loop
/// rather than exiting (else the deferred fetch only completes on a later user action). A no-op (`None`)
/// when the master switch is off — no fetching happens without consent, so there is nothing to wait on.
pub fn content_fetch_schedule_eta_secs(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
) -> Result<Option<u64>> {
    if !config.ai.content_fetch_enabled {
        return Ok(None);
    }
    let connection = open_intelligence_connection(paths, config, key)?;
    next_content_fetch_schedule_eta_secs(&connection)
}

/// Enqueues content-fetch jobs for the prioritized working set (06 §5 hook).
///
/// Pulls the top `limit` working-set candidates (starred ≫ recent ∪ tagged/noted ∪ high-freq) via the
/// SHARED selector, then enqueues each whose domain is consent-allowed AND whose content-fetch is DUE
/// (never fetched / negative-cache cooled / extractor bumped). Keyed by unique URL so each page is
/// fetched once and fanned out. Returns the number of jobs enqueued. A no-op when the master switch is
/// off. The caller (a Settings action or a post-backup hook) decides WHEN to run this; the selection +
/// gating live here so the prioritization is consistent with the heavy embedding tier.
pub fn enqueue_content_fetch_working_set(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    limit: usize,
) -> Result<usize> {
    if !config.ai.content_fetch_enabled {
        return Ok(0);
    }
    let connection = open_intelligence_connection(paths, config, key)?;
    // The working-set selector reads canonical signals off the attached `archive` schema; run it
    // against the archive connection so it sees `urls`/`star`/annotations. The intelligence connection
    // attaches archive as `archive`, but the selector queries unqualified table names, so use the
    // archive connection directly for selection, then enqueue on the intelligence connection.
    let archive = crate::archive::open_archive_connection(paths, config, key)?;
    let now_ms = chrome_now_ms();
    let candidates = crate::ai::select_working_set(
        &archive,
        &crate::ai::WorkingSetConfig::default(),
        now_ms,
        limit,
    )?;
    let mut enqueued = 0_usize;
    for candidate in candidates {
        if content_fetch_domain_blocked(config, &url_domain(&candidate.canonical_url)) {
            continue;
        }
        if !content_fetch_allowed(config, &candidate.canonical_url) {
            continue;
        }
        // Resolve the history_id by matching the RAW stored url (CORR-2). The CANONICAL url strips
        // tracking params / lowercases the host, so it may not equal any `urls.url`; seeking on the raw
        // variant the working set surfaced rides `idx_urls_url` and finds the live visit. Fall back to
        // the canonical URL when no raw variant was recorded (a URL-star-only candidate).
        let seek_url = candidate.raw_url.as_deref().unwrap_or(candidate.canonical_url.as_str());
        let Some((history_id, profile_id)) = resolve_visit_for_raw_url(&connection, seek_url)?
        else {
            continue;
        };
        // `content_fetch_allowed` above already proved an extractor resolves (it returns false on a
        // None), so this re-resolution is infallible — `expect` documents that invariant without an
        // unreachable `continue` branch the coverage gate would flag.
        let extractor = resolve_extractor(&candidate.canonical_url)
            .expect("content_fetch_allowed guarantees an extractor resolves for this URL");
        if !content_fetch_job_due(
            &connection,
            history_id,
            extractor.id(),
            extractor.version() as i64,
        )? {
            continue;
        }
        enqueue_content_fetch_job(
            &connection,
            &EnrichmentJobPayload {
                history_id,
                profile_id,
                url: candidate.canonical_url.clone(),
                title: candidate.title.clone(),
            },
        )?;
        enqueued += 1;
    }
    Ok(enqueued)
}

/// Enqueues (or refreshes) one content-fetch job for the manual "fetch now" PME trigger (06 §6).
///
/// Honest about consent: when the master switch is off (or the URL is per-extractor/domain blocked)
/// it returns a `disabled` result WITHOUT enqueuing anything (so the FE can prompt the user to enable
/// fetching rather than silently queueing a job that will never run). Otherwise it enqueues the job
/// and the worker drain picks it up.
pub fn enqueue_content_fetch_now(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    request: &ContentFetchNowRequest,
) -> Result<ContentFetchNowResult> {
    if !content_fetch_allowed(config, &request.url) {
        return Ok(ContentFetchNowResult {
            job_id: 0,
            state: "disabled".to_string(),
            note: "content_fetch.disabled_for_url".to_string(),
        });
    }
    let connection = open_intelligence_connection(paths, config, key)?;
    let job_id = enqueue_content_fetch_job(
        &connection,
        &EnrichmentJobPayload {
            history_id: request.history_id,
            profile_id: request.profile_id.clone(),
            url: request.url.clone(),
            title: request.title.clone(),
        },
    )?;
    let state = connection
        .query_row("SELECT state FROM intelligence_jobs WHERE id = ?1", [job_id], |row| {
            row.get::<_, String>(0)
        })
        .unwrap_or_else(|_| "queued".to_string());
    Ok(ContentFetchNowResult { job_id, state, note: "content_fetch.queued".to_string() })
}

/// Lists the stored enrichment rows for one visit/history id (the detail-panel read, 06 §6).
///
/// Reads from the intelligence DB; does NO network (the detail panel never blocks on a fetch — it
/// shows whatever has been stored). Returns the rows newest-first; the full body blob is intentionally
/// omitted (only the capped summary + structured metadata ride to the FE).
pub fn list_visit_enrichment(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    history_id: i64,
) -> Result<Vec<VisitEnrichmentRecord>> {
    let connection = open_intelligence_connection(paths, config, key)?;
    let mut statement = connection.prepare(
        "SELECT content_source, fetch_status, fetched_at, readable_title, enrichment_summary,
                extractor_version, extraction_json, final_url, http_status, refetch_after
         FROM visit_content_enrichments
         WHERE history_id = ?1
         ORDER BY fetched_at DESC",
    )?;
    let rows = statement.query_map(params![history_id], |row| {
        Ok(VisitEnrichmentRecord {
            content_source: row.get(0)?,
            fetch_status: row.get(1)?,
            fetched_at: row.get(2)?,
            readable_title: row.get(3)?,
            summary: row.get(4)?,
            extractor_version: row.get(5)?,
            metadata_json: row.get::<_, Option<String>>(6)?,
            final_url: row.get(7)?,
            http_status: row.get(8)?,
            refetch_after: row.get(9)?,
        })
    })?;
    rows.collect::<rusqlite::Result<Vec<_>>>().map_err(Into::into)
}

/// Returns the Settings-facing content-fetch consent + status surface (06 §6).
pub fn content_fetch_settings(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
) -> Result<ContentFetchSettings> {
    let connection = open_intelligence_connection(paths, config, key)?;
    let count_jobs = |state: &str| -> Result<usize> {
        connection
            .query_row(
                "SELECT COUNT(*) FROM intelligence_jobs WHERE job_type = 'content-fetch' AND state = ?1",
                [state],
                |row| row.get::<_, i64>(0),
            )
            .map(|value| value.max(0) as usize)
            .map_err(Into::into)
    };
    let stored_records = connection
        .query_row(
            "SELECT COUNT(*) FROM visit_content_enrichments
             WHERE content_source IN ('github-repo', 'generic-readable')",
            [],
            |row| row.get::<_, i64>(0),
        )
        .map(|value| value.max(0) as usize)
        .unwrap_or(0);
    Ok(ContentFetchSettings {
        enabled: config.ai.content_fetch_enabled,
        extractors: config.ai.content_fetch_extractors.clone(),
        domains: config.ai.content_fetch_domains.clone(),
        queued_jobs: count_jobs("queued")?,
        running_jobs: count_jobs("running")?,
        failed_jobs: count_jobs("failed")?,
        stored_records,
    })
}

/// Resolves one (history_id, profile_id) for a RAW stored url via the attached archive (newest visit).
///
/// The stored enrichment row keys by history_id; the working set surfaces a RAW url that actually
/// exists in `urls.url` (CORR-2). Matching that raw url with an equality seek rides `idx_urls_url`
/// (migration 014) and finds the live visit — whereas matching the CANONICAL url (which may strip
/// `utm_*`/`gclid` or lowercase the host) would miss the stored variant and silently drop the
/// candidate. We pick the most-recent matching visit so the stored row maps to a live, recent visit
/// (the store-time fan-out then spreads the enrichment across the page's sibling visits).
fn resolve_visit_for_raw_url(
    connection: &rusqlite::Connection,
    raw_url: &str,
) -> Result<Option<(i64, String)>> {
    // The intelligence connection attaches the canonical archive as `archive`.
    connection
        .query_row(
            "SELECT visits.id, source_profiles.profile_key
             FROM archive.visits AS visits
             JOIN archive.urls AS urls ON urls.id = visits.url_id
             JOIN archive.source_profiles AS source_profiles
               ON source_profiles.id = visits.source_profile_id
             WHERE visits.reverted_at IS NULL
               AND urls.url = ?1
             ORDER BY visits.visit_time_ms DESC
             LIMIT 1",
            params![raw_url],
            |row| Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?)),
        )
        .optional()
        .map_err(Into::into)
}

/// Current time in Chrome-epoch milliseconds (what the working-set recency window measures against).
fn chrome_now_ms() -> i64 {
    // `last_visit_ms` in `urls` is Chrome-epoch milliseconds; derive "now" in the same frame.
    const CHROME_EPOCH_OFFSET_MS: i64 = 11_644_473_600_000;
    let unix_ms = chrono::Utc::now().timestamp_millis();
    unix_ms + CHROME_EPOCH_OFFSET_MS
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{ensure_paths, project_paths_with_root};
    use crate::enrichment::rate_limit;
    use crate::models::{ContentFetchExtractorPreference, ContentFetchNowRequest};
    use tempfile::tempdir;

    fn consenting_config() -> AppConfig {
        let mut config = AppConfig { initialized: true, ..AppConfig::default() };
        config.ai.content_fetch_enabled = true;
        config
    }

    /// A unique per-test host so the process-global rate-limit registry never leaks between parallel
    /// drain tests (the shared `api.github.com` bucket in particular is off-limits here).
    fn unique_host(tag: &str) -> String {
        format!("drain-{tag}-{}.example", std::process::id())
    }

    #[test]
    fn enqueue_content_fetch_now_is_disabled_when_master_switch_off() {
        let root = tempdir().expect("tempdir");
        let paths = project_paths_with_root(root.path());
        ensure_paths(&paths).expect("paths");
        let config = AppConfig { initialized: true, ..AppConfig::default() }; // master OFF
        let result = enqueue_content_fetch_now(
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
        .expect("fetch now");
        assert_eq!(result.state, "disabled");
        assert_eq!(result.job_id, 0);
    }

    #[test]
    fn enqueue_content_fetch_now_queues_a_job_when_enabled() {
        let root = tempdir().expect("tempdir");
        let paths = project_paths_with_root(root.path());
        ensure_paths(&paths).expect("paths");
        let config = consenting_config();
        let result = enqueue_content_fetch_now(
            &paths,
            &config,
            None,
            &ContentFetchNowRequest {
                history_id: 1,
                profile_id: "chrome:Default".to_string(),
                url: "https://github.com/o/r".to_string(),
                title: Some("o/r".to_string()),
            },
        )
        .expect("fetch now");
        assert_eq!(result.state, "queued");
        assert!(result.job_id > 0);
    }

    #[test]
    fn content_fetch_settings_reports_consent_and_zero_jobs_initially() {
        let root = tempdir().expect("tempdir");
        let paths = project_paths_with_root(root.path());
        ensure_paths(&paths).expect("paths");
        let config = consenting_config();
        let settings = content_fetch_settings(&paths, &config, None).expect("settings");
        assert!(settings.enabled);
        assert_eq!(settings.queued_jobs, 0);
        assert_eq!(settings.stored_records, 0);
        // Both built-in extractors are represented.
        assert_eq!(settings.extractors.len(), 2);
    }

    #[test]
    fn drain_one_content_fetch_job_is_noop_without_consent() {
        let root = tempdir().expect("tempdir");
        let paths = project_paths_with_root(root.path());
        ensure_paths(&paths).expect("paths");
        let config = AppConfig { initialized: true, ..AppConfig::default() }; // master OFF
        assert!(!drain_one_content_fetch_job(&paths, &config, None).expect("drain"));
    }

    #[test]
    fn drain_one_content_fetch_job_picks_up_a_due_job_when_enabled() {
        let root = tempdir().expect("tempdir");
        let paths = project_paths_with_root(root.path());
        ensure_paths(&paths).expect("paths");
        let config = consenting_config();
        // No due job → drain returns false (the lane idles).
        assert!(!drain_one_content_fetch_job(&paths, &config, None).expect("idle drain"));

        // Enqueue one DUE job on a unique host with a FULL bucket, then the drain finds it + RUNS it
        // (returns true). Under the test build the execute entry drives the real pipeline through an
        // offline fetcher, so an un-throttled job stores a row (`Ran` → true) — proving the "found a due
        // job + dispatched + ran" path of the worker-facing drain without real egress. The unique host
        // keeps this off the shared `api.github.com` bucket the BUG-1 deferral test drains in parallel.
        let host = unique_host("due");
        rate_limit::reset_host_bucket_for_test(&host);
        enqueue_content_fetch_now(
            &paths,
            &config,
            None,
            &ContentFetchNowRequest {
                history_id: 1,
                profile_id: "chrome:Default".to_string(),
                url: format!("https://{host}/post"),
                title: None,
            },
        )
        .expect("enqueue");
        assert!(drain_one_content_fetch_job(&paths, &config, None).expect("drain picks up job"));
    }

    #[test]
    fn drain_one_content_fetch_job_returns_false_for_a_rate_limited_job() {
        // BUG-3: `drain_one_content_fetch_job` must PROPAGATE the run/defer signal — a rate-limited job
        // (egress bucket empty → requeued, SEC-2) maps to `Ok(false)` so the worker lane idles + sleeps
        // to the deferred ETA instead of busy-looping the parked row. We drain a unique host's bucket,
        // enqueue a due job on it, and assert the drain reports false (the job was deferred, not run).
        let root = tempdir().expect("tempdir");
        let paths = project_paths_with_root(root.path());
        ensure_paths(&paths).expect("paths");
        let config = consenting_config();

        let host = unique_host("rate-limited");
        let url = format!("https://{host}/post");
        // Empty the host bucket so the claimed job's egress is throttled → it defers.
        while rate_limit::acquire_host_token(&host) {}
        enqueue_content_fetch_now(
            &paths,
            &config,
            None,
            &ContentFetchNowRequest {
                history_id: 1,
                profile_id: "chrome:Default".to_string(),
                url: url.clone(),
                title: None,
            },
        )
        .expect("enqueue");

        assert!(
            !drain_one_content_fetch_job(&paths, &config, None).expect("drain"),
            "a deferred (rate-limited) job makes the drain report false (idle + sleep to ETA)"
        );
        // The job is back to `queued` with a future schedule (requeued, not run or cancelled).
        let connection = open_intelligence_connection(&paths, &config, None).expect("intelligence");
        let state: String = connection
            .query_row(
                "SELECT state FROM intelligence_jobs WHERE job_type = 'content-fetch'",
                [],
                |row| row.get(0),
            )
            .expect("state");
        assert_eq!(state, "queued", "the deferred job is requeued, awaiting its ETA");
        rate_limit::reset_host_bucket_for_test(&host);
    }

    #[test]
    fn list_visit_enrichment_is_empty_for_unknown_history_id() {
        let root = tempdir().expect("tempdir");
        let paths = project_paths_with_root(root.path());
        ensure_paths(&paths).expect("paths");
        let config = consenting_config();
        let records = list_visit_enrichment(&paths, &config, None, 999).expect("list");
        assert!(records.is_empty());
    }

    #[test]
    fn enqueue_working_set_is_noop_without_consent() {
        let root = tempdir().expect("tempdir");
        let paths = project_paths_with_root(root.path());
        ensure_paths(&paths).expect("paths");
        let config = AppConfig { initialized: true, ..AppConfig::default() }; // master OFF
        assert_eq!(enqueue_content_fetch_working_set(&paths, &config, None, 100).expect("ws"), 0);
    }

    #[test]
    fn enqueue_working_set_enqueues_a_starred_due_url() {
        // The 06 §5 prioritization hook: a STARRED URL with a live visit is selected by the shared
        // working-set ranking + enqueued for content fetch (keyed by its canonical URL).
        let root = tempdir().expect("tempdir");
        let paths = project_paths_with_root(root.path());
        ensure_paths(&paths).expect("paths");
        let config = AppConfig {
            initialized: true,
            archive_mode: crate::models::ArchiveMode::Plaintext,
            ai: {
                let mut ai = AppConfig::default().ai;
                ai.content_fetch_enabled = true;
                ai
            },
            ..AppConfig::default()
        };
        // Bootstrap the canonical archive + seed one starred github URL with a live visit.
        let archive =
            crate::archive::open_archive_connection(&paths, &config, None).expect("archive");
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
                "INSERT INTO star (entity_kind, entity_key, starred_at)
                 VALUES ('url', 'https://github.com/o/r', '2026-01-01')",
                [],
            )
            .expect("star");

        let enqueued =
            enqueue_content_fetch_working_set(&paths, &config, None, 100).expect("enqueue ws");
        assert_eq!(enqueued, 1, "the starred, due github URL must be enqueued once");

        // A second pass is a no-op: the job already exists (deduped) and re-enqueue refreshes it, so
        // the COUNT of content-fetch jobs stays 1.
        enqueue_content_fetch_working_set(&paths, &config, None, 100).expect("enqueue ws again");
        let connection = open_intelligence_connection(&paths, &config, None).expect("intelligence");
        let jobs: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM intelligence_jobs WHERE job_type = 'content-fetch'",
                [],
                |row| row.get(0),
            )
            .expect("count");
        assert_eq!(jobs, 1);
    }

    /// Builds a plaintext, content-fetch-enabled config + seeds runs/profile in a fresh archive.
    fn seeded_ws_config(root: &std::path::Path) -> (ProjectPaths, AppConfig) {
        let paths = project_paths_with_root(root);
        ensure_paths(&paths).expect("paths");
        let config = AppConfig {
            initialized: true,
            archive_mode: crate::models::ArchiveMode::Plaintext,
            ai: {
                let mut ai = AppConfig::default().ai;
                ai.content_fetch_enabled = true;
                ai
            },
            ..AppConfig::default()
        };
        let archive =
            crate::archive::open_archive_connection(&paths, &config, None).expect("archive");
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
        (paths, config)
    }

    #[test]
    fn enqueue_working_set_resolves_a_tracking_param_raw_url() {
        // CORR-2: a STARRED page whose stored raw url carries a tracking param (canonical ≠ raw) must
        // STILL resolve a live visit (via the RAW url, riding `idx_urls_url`) and enqueue — the old
        // canonical-keyed seek returned None and silently dropped it.
        let root = tempdir().expect("tempdir");
        let (paths, config) = seeded_ws_config(root.path());
        let archive =
            crate::archive::open_archive_connection(&paths, &config, None).expect("archive");
        // The stored raw url has a utm param; the star + working set key by the canonical url.
        archive
            .execute(
                "INSERT INTO urls (id, url, title, visit_count, typed_count, first_visit_ms, first_visit_iso, last_visit_ms, last_visit_iso, source_profile_id, created_by_run_id)
                 VALUES (1, 'https://github.com/o/r?utm_source=newsletter', 'o/r', 1, 0, 1, '', 1, '', 1, 1)",
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
                "INSERT INTO star (entity_kind, entity_key, starred_at)
                 VALUES ('url', 'https://github.com/o/r', '2026-01-01')",
                [],
            )
            .expect("star");
        drop(archive);

        let enqueued =
            enqueue_content_fetch_working_set(&paths, &config, None, 100).expect("enqueue ws");
        assert_eq!(enqueued, 1, "a tracking-param raw URL must still resolve a visit + enqueue");
    }

    #[test]
    fn enqueue_working_set_skips_blocked_and_undue_candidates() {
        // Covers the per-candidate continue guards: a per-domain-blocked candidate, and a candidate
        // that is NOT due (a fresh success row in the negative cache) are both skipped.
        let root = tempdir().expect("tempdir");
        let (paths, mut config) = seeded_ws_config(root.path());
        let archive =
            crate::archive::open_archive_connection(&paths, &config, None).expect("archive");
        // Two starred github repos with live visits.
        for (uid, vid, path) in [(1_i64, 1_i64, "blocked-co/r"), (2, 2, "fresh-co/r")] {
            archive
                .execute(
                    "INSERT INTO urls (id, url, title, visit_count, typed_count, first_visit_ms, first_visit_iso, last_visit_ms, last_visit_iso, source_profile_id, created_by_run_id)
                     VALUES (?1, ?2, 'r', 1, 0, 1, '', 1, '', 1, 1)",
                    params![uid, format!("https://github.com/{path}")],
                )
                .expect("url");
            archive
                .execute(
                    "INSERT INTO visits (id, url_id, source_profile_id, visit_time_ms, visit_time_iso, created_by_run_id)
                     VALUES (?1, ?2, 1, 1, '2026-01-01T00:00:00Z', 1)",
                    params![vid, uid],
                )
                .expect("visit");
            archive
                .execute(
                    "INSERT INTO star (entity_kind, entity_key, starred_at) VALUES ('url', ?1, '2026-01-01')",
                    params![format!("https://github.com/{path}")],
                )
                .expect("star");
        }
        drop(archive);

        // Block the first candidate's domain via a per-domain rule (covers the domain-block continue).
        config.ai.content_fetch_domains = vec![crate::models::ContentFetchDomainRule {
            domain: "github.com".to_string(),
            allowed: false,
        }];
        assert_eq!(
            enqueue_content_fetch_working_set(&paths, &config, None, 100).expect("ws blocked"),
            0,
            "a per-domain-blocked candidate is skipped"
        );

        // Unblock, but mark the SECOND repo as freshly fetched (not due) → it is skipped, only the
        // first remains due and enqueues.
        config.ai.content_fetch_domains.clear();
        let connection = open_intelligence_connection(&paths, &config, None).expect("intelligence");
        crate::enrichment::ensure_visit_content_enrichment_schema(&connection).expect("schema");
        connection
            .execute(
                "INSERT INTO visit_content_enrichments
                 (history_id, content_source, fetch_status, fetched_at, snippet_json, extraction_json,
                  pipeline_version, extractor_version, enrichment_summary)
                 VALUES (2, 'github-repo', 'success', '2026-06-21T00:00:00Z', '[]', '{}', 'v1', 1, 's')",
                [],
            )
            .expect("fresh row makes candidate 2 not due");
        let enqueued =
            enqueue_content_fetch_working_set(&paths, &config, None, 100).expect("ws not-due");
        assert_eq!(enqueued, 1, "only the still-due candidate enqueues; the fresh one is skipped");
    }

    #[test]
    fn enqueue_working_set_skips_extractor_disabled_and_visitless_candidates() {
        // Covers the per-extractor-disabled continue (`!content_fetch_allowed`) and the
        // no-live-visit-for-raw-url continue (`resolve_visit_for_raw_url` None).
        let root = tempdir().expect("tempdir");
        let (paths, mut config) = seeded_ws_config(root.path());
        let archive =
            crate::archive::open_archive_connection(&paths, &config, None).expect("archive");
        // Candidate A: a starred github repo WITH a live visit (but github extractor disabled below).
        archive
            .execute(
                "INSERT INTO urls (id, url, title, visit_count, typed_count, first_visit_ms, first_visit_iso, last_visit_ms, last_visit_iso, source_profile_id, created_by_run_id)
                 VALUES (1, 'https://github.com/a/r', 'r', 1, 0, 1, '', 1, '', 1, 1)",
                [],
            )
            .expect("url a");
        archive
            .execute(
                "INSERT INTO visits (id, url_id, source_profile_id, visit_time_ms, visit_time_iso, created_by_run_id)
                 VALUES (1, 1, 1, 1, '2026-01-01T00:00:00Z', 1)",
                [],
            )
            .expect("visit a");
        archive
            .execute(
                "INSERT INTO star (entity_kind, entity_key, starred_at) VALUES ('url', 'https://github.com/a/r', '2026-01-01')",
                [],
            )
            .expect("star a");
        // Candidate B: a STARRED generic url with NO visit row at all → resolve_visit_for_raw_url None.
        archive
            .execute(
                "INSERT INTO star (entity_kind, entity_key, starred_at) VALUES ('url', 'https://no-visit.example/page', '2026-01-01')",
                [],
            )
            .expect("star b");
        drop(archive);

        // Disable the github extractor so candidate A is skipped at `!content_fetch_allowed`.
        config.ai.content_fetch_extractors = vec![ContentFetchExtractorPreference {
            extractor_id: "github-repo".to_string(),
            enabled: false,
        }];
        let enqueued = enqueue_content_fetch_working_set(&paths, &config, None, 100).expect("ws");
        assert_eq!(enqueued, 0, "extractor-disabled + visitless candidates are both skipped");
    }

    #[test]
    fn list_visit_enrichment_returns_stored_rows() {
        // Covers the `list_visit_enrichment` row-mapping closure with an actual stored row.
        let root = tempdir().expect("tempdir");
        let paths = project_paths_with_root(root.path());
        ensure_paths(&paths).expect("paths");
        let config = consenting_config();
        let connection = open_intelligence_connection(&paths, &config, None).expect("intelligence");
        crate::enrichment::ensure_visit_content_enrichment_schema(&connection).expect("schema");
        connection
            .execute(
                "INSERT INTO visit_content_enrichments
                 (history_id, content_source, fetch_status, fetched_at, readable_title, snippet_json,
                  extraction_json, pipeline_version, extractor_version, enrichment_summary, final_url,
                  http_status, refetch_after)
                 VALUES (42, 'github-repo', 'success', '2026-06-21T00:00:00Z', 'o/r', '[]',
                         '{\"topics\":[\"wasm\"]}', 'v1', 1, 'A repo', 'https://github.com/o/r', 200, NULL)",
                [],
            )
            .expect("insert row");
        let records = list_visit_enrichment(&paths, &config, None, 42).expect("list");
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].content_source, "github-repo");
        assert_eq!(records[0].summary.as_deref(), Some("A repo"));
        assert_eq!(records[0].readable_title.as_deref(), Some("o/r"));
        assert_eq!(records[0].http_status, Some(200));
    }

    #[test]
    fn content_fetch_schedule_eta_is_none_without_consent_and_reports_deferral() {
        let root = tempdir().expect("tempdir");
        let paths = project_paths_with_root(root.path());
        ensure_paths(&paths).expect("paths");
        // Master OFF → None (no fetching, nothing to wait on).
        let off = AppConfig { initialized: true, ..AppConfig::default() };
        assert_eq!(content_fetch_schedule_eta_secs(&paths, &off, None).expect("off"), None);

        // Consent on, a job deferred into the future → its ETA is surfaced.
        let config = consenting_config();
        let connection = open_intelligence_connection(&paths, &config, None).expect("intelligence");
        let job_id = enqueue_content_fetch_job(
            &connection,
            &EnrichmentJobPayload {
                history_id: 1,
                profile_id: "chrome:Default".to_string(),
                url: "https://github.com/o/r".to_string(),
                title: None,
            },
        )
        .expect("enqueue");
        let future = (chrono::Utc::now() + chrono::Duration::seconds(20))
            .to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
        connection
            .execute(
                "UPDATE intelligence_jobs SET scheduled_at = ?1 WHERE id = ?2",
                params![future, job_id],
            )
            .expect("defer");
        let eta =
            content_fetch_schedule_eta_secs(&paths, &config, None).expect("eta").expect("some");
        assert!((1..=21).contains(&eta), "ETA should be ~20s, got {eta}");
    }

    #[test]
    fn chrome_now_ms_is_in_chrome_epoch_frame() {
        // Sanity: "now" in Chrome ms is far larger than the Unix ms (offset is ~11.6 trillion ms).
        // Sample the unix clock FIRST so the chrome value (taken after) is never earlier; allow a
        // small tolerance so the two non-atomic `Utc::now()` reads don't make the delta brittle.
        const CHROME_EPOCH_OFFSET_MS: i64 = 11_644_473_600_000;
        let unix = chrono::Utc::now().timestamp_millis();
        let chrome = chrome_now_ms();
        assert!(chrome > unix);
        // The delta is the offset plus the tiny time between the two reads, never less than the offset
        // minus a 1s safety margin.
        assert!((chrome - unix - CHROME_EPOCH_OFFSET_MS).abs() < 1_000);
    }
}
