use anyhow::{Context, Result};
use chrono::{Duration, Utc};
use rusqlite::{Connection, params};
use serde_json::json;
use std::{env, fs, path::PathBuf, time::Instant};
use tempfile::tempdir;
use vault_core::{
    archive::{open_archive_connection, open_intelligence_connection},
    config::project_paths_with_root,
    get_digest_summary, get_query_families, get_refind_pages, get_search_trails, get_sessions,
    get_top_search_concepts, get_top_sites,
    intelligence::run_core_intelligence_job_type_with_progress,
    intelligence_runtime::load_intelligence_runtime,
    models::{
        AppConfig, ArchiveMode, CoreIntelligenceRebuildRequest, DateRange, PagedDateRangeRequest,
        RefindPagesRequest, ScopedDateRangeRequest, SearchTrailQueryRequest,
        TopSearchConceptsRequest, TopSitesRequest,
    },
    run_core_intelligence_with_progress,
};

#[derive(Debug, Clone)]
struct Options {
    visits: usize,
    window_days: u32,
    horizon_days: u32,
    scenario: Scenario,
    output: Option<PathBuf>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Scenario {
    Full,
    AppendDelta,
    ExpiredLeaseRecovery,
    VisibilityRegressionFallback,
}

impl Scenario {
    fn as_str(self) -> &'static str {
        match self {
            Self::Full => "full",
            Self::AppendDelta => "append-delta",
            Self::ExpiredLeaseRecovery => "expired-lease-recovery",
            Self::VisibilityRegressionFallback => "visibility-regression-fallback",
        }
    }
}

fn main() -> Result<()> {
    let options = parse_args()?;
    let root = tempdir().context("creating temporary benchmark root")?;
    let paths = project_paths_with_root(root.path());
    let config = AppConfig {
        initialized: true,
        archive_mode: ArchiveMode::Plaintext,
        ..AppConfig::default()
    };
    let mut connection = open_archive_connection(&paths, &config, None)?;
    seed_synthetic_archive(
        &mut connection,
        options.visits,
        options.window_days,
        options.horizon_days,
    )?;
    drop(connection);
    let corpus = collect_corpus_stats(
        &paths,
        &config,
        options.visits,
        options.window_days,
        options.horizon_days,
    )?;

    let rebuild_request =
        CoreIntelligenceRebuildRequest { profile_id: None, full_rebuild: true, limit: None };
    let query_range = DateRange {
        start: (Utc::now() - Duration::days(options.window_days as i64)).date_naive().to_string(),
        end: Utc::now().date_naive().to_string(),
    };
    let paged_request = PagedDateRangeRequest {
        date_range: query_range.clone(),
        profile_id: None,
        page: 0,
        page_size: 20,
    };

    let rebuild_started = Instant::now();
    let baseline_rebuild =
        run_core_intelligence_with_progress(&paths, &config, None, &rebuild_request, |_| Ok(()))?;
    let baseline_rebuild_elapsed_ms = rebuild_started.elapsed().as_millis();
    let baseline_memory = memory_snapshot_json();

    let follow_up = match options.scenario {
        Scenario::Full => None,
        Scenario::AppendDelta => Some(run_append_delta_scenario(&paths, &config)?),
        Scenario::ExpiredLeaseRecovery => {
            Some(run_expired_lease_recovery_scenario(&paths, &config)?)
        }
        Scenario::VisibilityRegressionFallback => {
            Some(run_visibility_regression_scenario(&paths, &config)?)
        }
    };

    let query_started = Instant::now();
    let sessions = get_sessions(&paths, &config, None, &paged_request)?;
    let trails = get_search_trails(
        &paths,
        &config,
        None,
        &SearchTrailQueryRequest {
            date_range: query_range.clone(),
            profile_id: None,
            engine: None,
            page: 0,
            page_size: 20,
        },
    )?;
    let digest = get_digest_summary(
        &paths,
        &config,
        None,
        &ScopedDateRangeRequest { date_range: query_range.clone(), profile_id: None },
    )?;
    let query_families = get_query_families(&paths, &config, None, &paged_request)?;
    let top_sites = get_top_sites(
        &paths,
        &config,
        None,
        &TopSitesRequest {
            date_range: query_range.clone(),
            profile_id: None,
            sort_by: Some("visit_count".to_string()),
            limit: Some(10),
        },
    )?;
    let refind_pages = get_refind_pages(
        &paths,
        &config,
        None,
        &RefindPagesRequest { date_range: query_range.clone(), profile_id: None, limit: Some(10) },
    )?;
    let top_concepts = get_top_search_concepts(
        &paths,
        &config,
        None,
        &TopSearchConceptsRequest { date_range: query_range, profile_id: None, limit: Some(15) },
    )?;
    let query_elapsed_ms = query_started.elapsed().as_millis();

    let payload = json!({
        "corpus": corpus,
        "scenario": options.scenario.as_str(),
        "timings": {
            "baselineRunCoreIntelligenceMs": baseline_rebuild_elapsed_ms,
            "querySurfacesMs": query_elapsed_ms,
        },
        "memory": {
            "baseline": baseline_memory,
            "afterQueries": memory_snapshot_json(),
        },
        "baselineReport": baseline_rebuild,
        "followUp": follow_up,
        "surfaces": {
            "sessions": sessions.sessions.len(),
            "sessionTotal": sessions.total,
            "searchTrails": trails.trails.len(),
            "searchTrailTotal": trails.total,
            "queryFamilies": query_families.families.len(),
            "queryFamilyTotal": query_families.total,
            "topSites": top_sites.len(),
            "refindPages": refind_pages.len(),
            "topConcepts": top_concepts.len(),
            "digest": {
                "totalVisits": digest.total_visits.value,
                "totalSearches": digest.total_searches.value,
                "newDomains": digest.new_domains.value,
                "deepReadPages": digest.deep_read_pages.value,
                "refindPages": digest.refind_pages.value,
            }
        }
    });

    if let Some(output) = options.output {
        if let Some(parent) = output.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("creating benchmark output dir {}", parent.display()))?;
        }
        fs::write(&output, serde_json::to_string_pretty(&payload)?)
            .with_context(|| format!("writing benchmark artifact {}", output.display()))?;
    } else {
        println!("{}", serde_json::to_string_pretty(&payload)?);
    }

    Ok(())
}

fn parse_args() -> Result<Options> {
    let mut visits = 100_000usize;
    let mut window_days = 365u32;
    let mut horizon_days = 730u32;
    let mut scenario = Scenario::Full;
    let mut output = None;
    let mut args = env::args().skip(1);
    while let Some(argument) = args.next() {
        match argument.as_str() {
            "--visits" => {
                let value = args.next().context("--visits requires a value")?;
                visits = value.parse().context("parsing --visits")?;
            }
            "--window-days" => {
                let value = args.next().context("--window-days requires a value")?;
                window_days = value.parse().context("parsing --window-days")?;
            }
            "--horizon-days" => {
                let value = args.next().context("--horizon-days requires a value")?;
                horizon_days = value.parse().context("parsing --horizon-days")?;
            }
            "--scenario" => {
                let value = args.next().context("--scenario requires a value")?;
                scenario = match value.as_str() {
                    "full" => Scenario::Full,
                    "append-delta" => Scenario::AppendDelta,
                    "expired-lease-recovery" => Scenario::ExpiredLeaseRecovery,
                    "visibility-regression-fallback" => Scenario::VisibilityRegressionFallback,
                    _ => anyhow::bail!(
                        "unknown --scenario {value}; expected full, append-delta, expired-lease-recovery, or visibility-regression-fallback"
                    ),
                };
            }
            "--output" => {
                output = Some(PathBuf::from(args.next().context("--output requires a value")?));
            }
            flag => anyhow::bail!("unknown flag {flag}"),
        }
    }
    Ok(Options {
        visits,
        window_days: window_days.clamp(7, 365),
        horizon_days: horizon_days.max(window_days.clamp(7, 365) * 2),
        scenario,
        output,
    })
}

fn collect_corpus_stats(
    paths: &vault_core::ProjectPaths,
    config: &AppConfig,
    seeded_visits: usize,
    window_days: u32,
    horizon_days: u32,
) -> Result<serde_json::Value> {
    let connection = open_archive_connection(paths, config, None)?;
    let archive_bytes = fs::metadata(&paths.archive_database_path)
        .map(|metadata| metadata.len())
        .unwrap_or_default();
    let visit_count: i64 =
        connection.query_row("SELECT COUNT(*) FROM visits", [], |row| row.get(0))?;
    let profile_count: i64 =
        connection.query_row("SELECT COUNT(*) FROM source_profiles", [], |row| row.get(0))?;
    let search_term_count: i64 =
        connection.query_row("SELECT COUNT(*) FROM search_terms", [], |row| row.get(0))?;
    let url_stats = connection.query_row(
        "SELECT
            COALESCE(SUM(LENGTH(url)), 0),
            COALESCE(SUM(LENGTH(title)), 0),
            COALESCE(MAX(LENGTH(url)), 0),
            COALESCE(MAX(LENGTH(title)), 0)
         FROM urls",
        [],
        |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, i64>(3)?,
            ))
        },
    )?;
    Ok(json!({
        "seededVisits": seeded_visits,
        "windowDays": window_days,
        "horizonDays": horizon_days,
        "archiveBytes": archive_bytes,
        "profileCount": profile_count,
        "visitCount": visit_count,
        "searchTermCount": search_term_count,
        "urlCharTotal": url_stats.0,
        "titleCharTotal": url_stats.1,
        "maxUrlChars": url_stats.2,
        "maxTitleChars": url_stats.3,
    }))
}

fn memory_snapshot_json() -> serde_json::Value {
    let peak_rss_bytes = peak_rss_bytes();
    json!({
        "peakRssBytes": peak_rss_bytes,
        "peakRssMiB": peak_rss_bytes.map(|bytes| bytes as f64 / (1024.0 * 1024.0)),
    })
}

fn peak_rss_bytes() -> Option<u64> {
    #[cfg(unix)]
    unsafe {
        let mut usage = std::mem::zeroed::<libc::rusage>();
        if libc::getrusage(libc::RUSAGE_SELF, &mut usage) != 0 {
            return None;
        }
        #[cfg(any(target_os = "macos", target_os = "ios"))]
        {
            Some(usage.ru_maxrss as u64)
        }
        #[cfg(not(any(target_os = "macos", target_os = "ios")))]
        {
            Some((usage.ru_maxrss as u64) * 1024)
        }
    }
    #[cfg(not(unix))]
    {
        None
    }
}

fn run_append_delta_scenario(
    paths: &vault_core::ProjectPaths,
    config: &AppConfig,
) -> Result<serde_json::Value> {
    let mut archive = open_archive_connection(paths, config, None)?;
    let appended = append_synthetic_delta_visits(
        &mut archive,
        "chrome:Default",
        Utc::now().timestamp_millis(),
        64,
    )?;
    drop(archive);

    let request = CoreIntelligenceRebuildRequest {
        profile_id: Some("chrome:Default".to_string()),
        ..CoreIntelligenceRebuildRequest::default()
    };
    let visit_started = Instant::now();
    let visit_derive = run_core_intelligence_job_type_with_progress(
        paths,
        config,
        None,
        "visit-derive",
        &request,
        |_| Ok(()),
    )?;
    let visit_elapsed_ms = visit_started.elapsed().as_millis();
    let rollup_started = Instant::now();
    let daily_rollup = run_core_intelligence_job_type_with_progress(
        paths,
        config,
        None,
        "daily-rollup",
        &request,
        |_| Ok(()),
    )?;
    let rollup_elapsed_ms = rollup_started.elapsed().as_millis();
    let structural_started = Instant::now();
    let structural = run_core_intelligence_job_type_with_progress(
        paths,
        config,
        None,
        "structural-rebuild",
        &request,
        |_| Ok(()),
    )?;
    let structural_elapsed_ms = structural_started.elapsed().as_millis();

    Ok(json!({
        "scenario": Scenario::AppendDelta.as_str(),
        "appendedVisits": appended,
        "timings": {
            "visitDeriveMs": visit_elapsed_ms,
            "dailyRollupMs": rollup_elapsed_ms,
            "structuralRebuildMs": structural_elapsed_ms,
        },
        "memory": memory_snapshot_json(),
        "reports": {
            "visitDerive": visit_derive,
            "dailyRollup": daily_rollup,
            "structuralRebuild": structural,
        }
    }))
}

fn run_expired_lease_recovery_scenario(
    paths: &vault_core::ProjectPaths,
    config: &AppConfig,
) -> Result<serde_json::Value> {
    let connection = open_intelligence_connection(paths, config, None)?;
    let expired_at = (Utc::now() - Duration::minutes(10)).to_rfc3339();
    connection.execute(
        "INSERT INTO intelligence_jobs
         (job_type, plugin_id, run_id, state, priority, attempt, dedupe_key, payload_json,
          artifact_json, created_at, scheduled_at, started_at, heartbeat_at, lease_owner,
          lease_expires_at, updated_at, last_error, cancellation_reason, stop_requested)
         VALUES
            ('structural-rebuild', NULL, NULL, 'running', 40, 1, 'expired-lease-queued',
             ?1, '{}', ?2, ?2, ?2, ?2, 'benchmark-worker', ?3, ?2, NULL, NULL, 0),
            ('daily-rollup', NULL, NULL, 'running', 30, 1, 'expired-lease-cancelled',
             ?4, '{}', ?2, ?2, ?2, ?2, 'benchmark-worker', ?3, ?2, NULL, 'cancelled from UI', 1)",
        params![
            serde_json::to_string(&json!({
                "jobType": "structural-rebuild",
                "request": {
                    "profileId": "chrome:Default",
                    "fullRebuild": false,
                    "limit": null
                },
                "reason": "Benchmark expired lease recovery scenario"
            }))?,
            Utc::now().to_rfc3339(),
            expired_at,
            serde_json::to_string(&json!({
                "jobType": "daily-rollup",
                "request": {
                    "profileId": "chrome:Default",
                    "fullRebuild": false,
                    "limit": null
                },
                "reason": "Benchmark expired lease recovery scenario"
            }))?,
        ],
    )?;

    let started = Instant::now();
    let snapshot = load_intelligence_runtime(paths, config, None)?;
    let elapsed_ms = started.elapsed().as_millis();

    let recovered = connection
        .prepare(
            "SELECT dedupe_key, state, last_error, cancellation_reason
             FROM intelligence_jobs
             WHERE dedupe_key IN ('expired-lease-queued', 'expired-lease-cancelled')
             ORDER BY dedupe_key ASC",
        )?
        .query_map([], |row| {
            Ok(json!({
                "dedupeKey": row.get::<_, String>(0)?,
                "state": row.get::<_, String>(1)?,
                "lastError": row.get::<_, Option<String>>(2)?,
                "cancellationReason": row.get::<_, Option<String>>(3)?,
            }))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    Ok(json!({
        "scenario": Scenario::ExpiredLeaseRecovery.as_str(),
        "timings": {
            "runtimeRecoveryMs": elapsed_ms,
        },
        "memory": memory_snapshot_json(),
        "runtimeNotes": snapshot.notes,
        "recoveredJobs": recovered,
    }))
}

fn run_visibility_regression_scenario(
    paths: &vault_core::ProjectPaths,
    config: &AppConfig,
) -> Result<serde_json::Value> {
    let archive = open_archive_connection(paths, config, None)?;
    let reverted = archive.execute(
        "UPDATE visits
         SET reverted_at = ?1
         WHERE id IN (
             SELECT visits.id
             FROM visits
             JOIN source_profiles ON source_profiles.id = visits.source_profile_id
             WHERE source_profiles.profile_key = 'chrome:Default'
               AND visits.reverted_at IS NULL
             ORDER BY visits.id DESC
             LIMIT 24
         )",
        params![Utc::now().to_rfc3339()],
    )?;
    drop(archive);

    let request = CoreIntelligenceRebuildRequest {
        profile_id: Some("chrome:Default".to_string()),
        ..CoreIntelligenceRebuildRequest::default()
    };
    let started = Instant::now();
    let visit_derive = run_core_intelligence_job_type_with_progress(
        paths,
        config,
        None,
        "visit-derive",
        &request,
        |_| Ok(()),
    )?;
    let elapsed_ms = started.elapsed().as_millis();

    Ok(json!({
        "scenario": Scenario::VisibilityRegressionFallback.as_str(),
        "revertedVisits": reverted,
        "timings": {
            "visitDeriveMs": elapsed_ms,
        },
        "memory": memory_snapshot_json(),
        "reports": {
            "visitDerive": visit_derive,
        }
    }))
}

fn append_synthetic_delta_visits(
    connection: &mut Connection,
    profile_id: &str,
    start_time_ms: i64,
    count: usize,
) -> Result<usize> {
    let profile_row_id: i64 = connection.query_row(
        "SELECT id FROM source_profiles WHERE profile_key = ?1",
        [profile_id],
        |row| row.get(0),
    )?;
    let next_id: i64 =
        connection
            .query_row("SELECT COALESCE(MAX(id), 0) + 1 FROM visits", [], |row| row.get(0))?;
    let transaction = connection.unchecked_transaction()?;
    let mut url_statement = transaction.prepare(
        "INSERT INTO urls
         (id, url, title, visit_count, typed_count, first_visit_ms, first_visit_iso, last_visit_ms, last_visit_iso, source_profile_id, created_by_run_id, source_url_id, hidden, payload_hash, recorded_at)
         VALUES (?1, ?2, ?3, 1, 0, ?4, ?5, ?4, ?5, ?6, 1, ?1, 0, ?7, ?5)",
    )?;
    let mut visit_statement = transaction.prepare(
        "INSERT INTO visits
         (id, url_id, source_visit_id, visit_time_ms, visit_time_iso, transition_type, visit_duration_ms, source_profile_id, created_by_run_id, from_visit, is_known_to_sync, external_referrer_url, event_fingerprint, payload_hash, recorded_at)
         VALUES (?1, ?1, ?2, ?3, ?4, 1, 0, ?5, 1, ?6, 1, NULL, ?7, ?8, ?4)",
    )?;
    let mut search_statement = transaction.prepare(
        "INSERT INTO search_terms
         (url_id, term, normalized_term, source_profile_id, created_by_run_id, profile_id, keyword_id, recorded_at)
         VALUES (?1, ?2, ?3, ?4, 1, ?5, ?6, ?7)",
    )?;
    for index in 0..count {
        let history_id = next_id + index as i64;
        let visited_at_ms = start_time_ms + (index as i64 * 60_000);
        let visited_at = chrono::DateTime::from_timestamp_millis(visited_at_ms)
            .context("building append-delta timestamp")?;
        let query = format!("incremental topic {}", index % 9);
        let (url, title, from_visit) = match index % 3 {
            0 => (
                format!("https://www.google.com/search?q={}", query.replace(' ', "+")),
                format!("Search {query}"),
                None,
            ),
            1 => (
                format!("https://docs.incremental-{}.dev/guide/{}", index % 7, index),
                format!("Guide {index}"),
                Some(history_id - 1),
            ),
            _ => (
                format!("https://reference.incremental-{}.dev/page/{}", index % 5, index),
                format!("Reference {index}"),
                Some(history_id - 1),
            ),
        };
        url_statement.execute(params![
            history_id,
            url,
            title,
            visited_at_ms,
            visited_at.to_rfc3339(),
            profile_row_id,
            format!("append-url-hash-{history_id}")
        ])?;
        visit_statement.execute(params![
            history_id,
            history_id.to_string(),
            visited_at_ms,
            visited_at.to_rfc3339(),
            profile_row_id,
            from_visit.filter(|_| index % 3 != 0),
            format!("append-fingerprint-{history_id}"),
            format!("append-visit-hash-{history_id}")
        ])?;
        if index % 3 == 0 {
            search_statement.execute(params![
                history_id,
                query,
                query.to_lowercase(),
                profile_row_id,
                profile_id,
                history_id,
                visited_at.to_rfc3339()
            ])?;
        }
    }
    drop(search_statement);
    drop(visit_statement);
    drop(url_statement);
    transaction.commit()?;
    Ok(count)
}

fn seed_synthetic_archive(
    connection: &mut Connection,
    visits: usize,
    _window_days: u32,
    horizon_days: u32,
) -> Result<()> {
    let now = Utc::now();
    let horizon_start = now - Duration::days(horizon_days as i64);
    let transaction = connection.unchecked_transaction()?;
    transaction.execute(
        "INSERT INTO runs (id, run_type, trigger, started_at, timezone, status, profile_scope_json, warnings_json, stats_json, due_only)
         VALUES (1, 'backup', 'benchmark', ?1, 'UTC', 'success', '[]', '[]', '{}', 0)",
        params![now.to_rfc3339()],
    )?;
    for (id, profile_id) in
        [(1_i64, "chrome:Default"), (2, "chrome:Work"), (3, "edge:Default"), (4, "brave:Default")]
    {
        let browser_kind = profile_id.split(':').next().unwrap_or("legacy");
        transaction.execute(
            "INSERT INTO source_profiles (id, browser_kind, browser_version, profile_name, profile_path, discovered_at, enabled, profile_key, updated_at)
             VALUES (?1, ?2, 'benchmark', ?3, ?4, ?5, 1, ?3, ?5)",
            params![id, browser_kind, profile_id, format!("/tmp/{profile_id}"), now.to_rfc3339()],
        )?;
    }
    let mut url_statement = transaction.prepare(
        "INSERT INTO urls
         (id, url, title, visit_count, typed_count, first_visit_ms, first_visit_iso, last_visit_ms, last_visit_iso, source_profile_id, created_by_run_id, source_url_id, hidden, payload_hash, recorded_at)
         VALUES (?1, ?2, ?3, 1, 0, ?4, ?5, ?4, ?5, ?6, 1, ?1, 0, ?7, ?5)",
    )?;
    let mut visit_statement = transaction.prepare(
        "INSERT INTO visits
         (id, url_id, source_visit_id, visit_time_ms, visit_time_iso, transition_type, visit_duration_ms, source_profile_id, created_by_run_id, from_visit, is_known_to_sync, external_referrer_url, event_fingerprint, payload_hash, recorded_at)
         VALUES (?1, ?1, ?2, ?3, ?4, 805306368, ?5, ?6, 1, ?7, 1, ?8, ?9, ?10, ?4)",
    )?;
    let mut search_statement = transaction.prepare(
        "INSERT INTO search_terms
         (url_id, term, normalized_term, source_profile_id, created_by_run_id, profile_id,
          keyword_id, recorded_at)
         VALUES (?1, ?2, ?3, (SELECT id FROM source_profiles WHERE profile_key = ?4), 0, ?4, ?5, ?6)",
    )?;
    for index in 0..visits {
        let history_id = index as i64 + 1;
        let profile_id = match index % 4 {
            0 => "chrome:Default",
            1 => "chrome:Work",
            2 => "edge:Default",
            _ => "brave:Default",
        };
        let profile_row_id = match profile_id {
            "chrome:Default" => 1,
            "chrome:Work" => 2,
            "edge:Default" => 3,
            _ => 4,
        };
        let day_offset = (index as i64 % horizon_days as i64).max(1);
        let minute_offset = (index as i64 % 1_440).max(1);
        let visited_at =
            horizon_start + Duration::days(day_offset) + Duration::minutes(minute_offset);
        let visit_time_ms = visited_at.timestamp_millis();
        let query_term = format!("topic {} pattern {}", index % 400, index % 13);
        let (url, title, referrer) = match index % 6 {
            0 => (
                format!("https://www.google.com/search?q={}", query_term.replace(' ', "+")),
                format!("Search {}", index % 400),
                None,
            ),
            1 => (
                format!("https://docs.example.com/topic/{}/guide/{}", index % 200, index % 17),
                format!("Guide {}", index % 200),
                Some("https://www.google.com"),
            ),
            2 => (
                format!("https://github.com/example/repo/issues/{}", index % 5_000),
                format!("Issue {}", index % 5_000),
                Some("https://docs.example.com"),
            ),
            3 => (
                format!("https://community.example.com/t/thread-{}", index % 3_000),
                format!("Forum {}", index % 3_000),
                Some("https://www.google.com"),
            ),
            4 => (
                format!("https://reference.example.com/article/{}", index % 1_000),
                format!("Reference {}", index % 1_000),
                Some("https://docs.example.com"),
            ),
            _ => (
                format!("https://example.com/pricing/plan-{}", index % 40),
                format!("Plan {}", index % 40),
                None,
            ),
        };
        url_statement.execute(params![
            history_id,
            url,
            title,
            visit_time_ms,
            visited_at.to_rfc3339(),
            profile_row_id,
            format!("payload-{history_id}"),
        ])?;
        visit_statement.execute(params![
            history_id,
            history_id.to_string(),
            visit_time_ms,
            visited_at.to_rfc3339(),
            5_000 + (index % 20) as i64 * 400,
            profile_row_id,
            (index > 0).then_some(history_id - 1),
            referrer,
            format!("fp-{history_id}"),
            format!("payload-{history_id}"),
        ])?;
        if index % 6 == 0 {
            search_statement.execute(params![
                history_id,
                &query_term,
                &query_term,
                profile_id,
                history_id,
                visited_at.to_rfc3339(),
            ])?;
        }
    }
    drop(search_statement);
    drop(url_statement);
    drop(visit_statement);
    transaction.commit()?;
    Ok(())
}
