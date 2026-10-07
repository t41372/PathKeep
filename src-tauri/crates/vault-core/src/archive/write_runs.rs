//! Responsibilities: keep backup/import run state and committed batch progress honest.
//! Not responsible for: parser loops, manifests or storage ownership.
//! Failure modes: errors leave running rows, partial batches lose progress, profile
//! changes reset run totals, completing one import completes another batch.
//! Tests: vault::tests and archive::ingest::concurrency_tests.

use crate::{Vault, utils::now_rfc3339};
use anyhow::Result;
use serde_json::{Value, json};
use vault_store::{Db, Priority, Session, params};

pub(crate) fn start(vault: &Vault, kind: &str, trigger: &str, profiles: &[String]) -> Result<i64> {
    let (kind, trigger, profiles) = (kind.to_owned(), trigger.to_owned(), profiles.to_vec());
    vault.write(Db::Archive, Priority::Batch, move |s| start_in(s, &kind, &trigger, &profiles))
}

pub(crate) fn start_in(
    s: &Session<'_>,
    kind: &str,
    trigger: &str,
    profiles: &[String],
) -> Result<i64> {
    Ok(s.query_one("INSERT INTO runs(run_type,trigger,started_at,timezone,status,profile_scope_json,stats_json,warnings_json,due_only)
        VALUES(?1,?2,?3,?4,'running',?5,'{}','[]',?6) RETURNING id",
        params![kind,trigger,now_rfc3339(),super::run_support::current_timezone_name(),serde_json::to_string(profiles)?,i64::from(trigger=="schedule")],|r|Ok(r.get(0)?))?)
}

pub(crate) fn record_progress(
    s: &Session<'_>,
    run_id: i64,
    counts: &[(&str, usize)],
) -> Result<()> {
    let text = s.query_one("SELECT stats_json FROM runs WHERE id=?1", [run_id], |r| {
        Ok(r.get::<Option<String>>(0)?)
    })?;
    let mut stats: Value = serde_json::from_str(text.as_deref().unwrap_or("{}"))?;
    for (key, delta) in counts {
        stats[*key] = json!(stats[*key].as_u64().unwrap_or(0) + *delta as u64);
    }
    s.execute("UPDATE runs SET stats_json=?2 WHERE id=?1", params![run_id, stats.to_string()])?;
    Ok(())
}

pub(crate) fn complete(
    vault: &Vault,
    run_id: i64,
    extra: Value,
    warnings: Value,
    finished_at: String,
) -> Result<()> {
    // Read counters and indexed bounds; no fact table recount.
    let extra = vault.read(Db::Archive, move |s| with_totals(s, extra))?;
    finish(vault, run_id, "success", extra, Some(warnings), None, finished_at)
}

pub(crate) fn stopped(
    vault: &Vault,
    run_id: i64,
    error: &anyhow::Error,
    extra: Value,
    warnings: Option<Value>,
) -> Result<()> {
    finish(
        vault,
        run_id,
        failure_status(error),
        extra,
        warnings,
        Some(format!("{error:#}")),
        now_rfc3339(),
    )
}

pub(crate) fn failure_status(error: &anyhow::Error) -> &'static str {
    if error.chain().any(|cause| {
        matches!(
            cause.downcast_ref::<vault_store::Error>(),
            Some(vault_store::Error::Cancelled { .. })
        )
    }) {
        "interrupted"
    } else {
        "failed"
    }
}

fn finish(
    vault: &Vault,
    run_id: i64,
    status: &'static str,
    extra: Value,
    warnings: Option<Value>,
    error: Option<String>,
    finished_at: String,
) -> Result<()> {
    vault.write(Db::Archive, Priority::Batch, move |s| {
        finish_in(s, run_id, status, extra, warnings, error, finished_at)
    })
}

pub(crate) fn complete_in(
    s: &Session<'_>,
    run_id: i64,
    extra: Value,
    warnings: Value,
) -> Result<()> {
    finish_in(s, run_id, "success", with_totals(s, extra)?, Some(warnings), None, now_rfc3339())
}

fn finish_in(
    s: &Session<'_>,
    run_id: i64,
    status: &str,
    extra: Value,
    warnings: Option<Value>,
    error: Option<String>,
    finished_at: String,
) -> Result<()> {
    let text = s.query_one("SELECT stats_json FROM runs WHERE id=?1", [run_id], |r| {
        Ok(r.get::<String>(0)?)
    })?;
    let mut stats: Value = serde_json::from_str(&text)?;
    if let Some(fields) = extra.as_object() {
        for (key, value) in fields {
            stats[key] = value.clone();
        }
    }
    s.execute("UPDATE runs SET status=?2,finished_at=?3,stats_json=?4,warnings_json=COALESCE(?5,warnings_json),error_message=?6 WHERE id=?1",
            params![run_id,status,finished_at,stats.to_string(),warnings.map(|value|value.to_string()),error])?;
    Ok(())
}

/// Persisted totals keep unported dashboard readers from recounting on each render.
pub(crate) fn with_totals(s: &Session<'_>, mut stats: Value) -> Result<Value> {
    let totals = super::totals::load_cached_archive_totals(s)?;
    for (key, source) in [
        ("totalProfiles", "sourceProfiles"),
        ("totalUrls", "urls"),
        ("totalVisits", "visits"),
        ("totalDownloads", "downloads"),
    ] {
        stats[key] = json!(totals[source]);
    }
    for (key, direction) in [("earliestVisitAt", "ASC"), ("latestVisitAt", "DESC")] {
        let time=s.query(&format!("SELECT visit_time_iso FROM visits WHERE reverted_at IS NULL ORDER BY visit_time_ms {direction},id {direction} LIMIT 1"),(),|r|Ok(r.get::<String>(0)?))?.pop();
        stats[key] = json!(time);
    }
    Ok(stats)
}
