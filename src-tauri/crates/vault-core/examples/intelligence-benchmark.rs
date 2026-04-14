use anyhow::{Context, Result};
use chrono::{Duration, Utc};
use rusqlite::{Connection, params};
use serde_json::json;
use std::{env, fs, path::PathBuf, time::Instant};
use tempfile::tempdir;
use vault_core::{
    archive::open_archive_connection,
    config::project_paths_with_root,
    load_insights,
    models::{AppConfig, ArchiveMode, RunInsightsRequest},
    run_insights_with_progress,
};

#[derive(Debug, Clone)]
struct Options {
    visits: usize,
    window_days: u32,
    horizon_days: u32,
    output: Option<PathBuf>,
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

    let request = RunInsightsRequest {
        profile_id: None,
        window_days: Some(options.window_days),
        full_rebuild: true,
        limit: None,
    };

    let rebuild_started = Instant::now();
    let rebuild = run_insights_with_progress(&paths, &config, None, None, &request, |_| Ok(()))?;
    let rebuild_elapsed_ms = rebuild_started.elapsed().as_millis();

    let snapshot_started = Instant::now();
    let snapshot = load_insights(&paths, &config, None, &request)?;
    let snapshot_elapsed_ms = snapshot_started.elapsed().as_millis();

    let archive_bytes = fs::metadata(&paths.archive_database_path)
        .map(|metadata| metadata.len())
        .unwrap_or_default();
    let payload = json!({
        "corpus": {
            "visits": options.visits,
            "windowDays": options.window_days,
            "horizonDays": options.horizon_days,
            "archiveBytes": archive_bytes,
        },
        "timings": {
            "runInsightsMs": rebuild_elapsed_ms,
            "loadInsightsMs": snapshot_elapsed_ms,
        },
        "report": rebuild,
        "snapshot": {
            "cards": snapshot.cards.len(),
            "queryGroups": snapshot.query_groups.len(),
            "threads": snapshot.threads.len(),
            "referencePages": snapshot.reference_pages.len(),
            "sourceEffectiveness": snapshot.source_effectiveness.len(),
            "templateSummaries": snapshot.template_summaries.len(),
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
        output,
    })
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
