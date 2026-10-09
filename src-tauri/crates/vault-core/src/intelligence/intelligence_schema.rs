//! Responsibilities: native readiness/counts and explicit derived-state clearing.
//! Not responsible for: schema installation, rebuilding facts or canonical mutations.
//! Failure modes: status scans millions of rows, clear touches canonical data, or
//! reads open/migrate a database. Store installs consolidated v1 before requests.

use super::build_state;
use crate::{
    Vault,
    config::ProjectPaths,
    models::{ClearDerivedIntelligenceReport, IntelligenceStatus},
};
use anyhow::Result;
use vault_store::{Db, Priority, Session};

pub fn max_intelligence_schema_version() -> i64 {
    1
}

pub fn intelligence_status(vault: &Vault) -> Result<IntelligenceStatus> {
    let paths = vault.paths().clone();
    vault.read(Db::Intelligence, move |s| intelligence_status_from(&paths, s))
}

pub(crate) fn intelligence_status_from(
    _paths: &ProjectPaths,
    s: &Session<'_>,
) -> Result<IntelligenceStatus> {
    let sessions = table_row_count(s, "sessions")?;
    let trails = table_row_count(s, "search_trails")?;
    let refind = table_row_count(s, "refind_pages")?;
    let last = s.query_one("SELECT MAX(updated_at) FROM intelligence_builds", (), |r| {
        Ok(r.get::<Option<String>>(0)?)
    })?;
    Ok(IntelligenceStatus {
        ready: sessions > 0
            || trails > 0
            || refind > 0
            || table_row_count(s, "visit_derived_facts")? > 0
            || table_row_count(s, "daily_summary_rollups")? > 0,
        last_run_at: last,
        runs: 0,
        cards: sessions,
        topics: 0,
        threads: 0,
        query_groups: trails,
        reference_pages: refind,
        content_coverage: 0.0,
        warning: None,
    })
}

pub fn preview_derived_intelligence_clear(vault: &Vault) -> Result<ClearDerivedIntelligenceReport> {
    vault.read(Db::Intelligence, count_derived_intelligence_state)
}

pub fn clear_derived_intelligence_state(vault: &Vault) -> Result<ClearDerivedIntelligenceReport> {
    let _guard = vault.intelligence_build();
    let report = preview_derived_intelligence_clear(vault)?;
    for (table, _) in build_state::TABLES {
        let sql = format!(
            "DELETE FROM {table}_rows WHERE rowid IN (SELECT rowid FROM {table}_rows LIMIT 1024)"
        );
        loop {
            let sql = sql.clone();
            let count = vault
                .write(Db::Intelligence, Priority::Batch, move |s| Ok(s.execute(&sql, ())?))?;
            if count == 0 {
                break;
            }
        }
    }
    for table in [
        "visit_assignments",
        "session_domains",
        "session_paths",
        "intelligence_seen_domains",
        "intelligence_day_counts",
        "intelligence_counts",
        "intelligence_generations",
        "intelligence_builds",
        "core_intelligence_stage_checkpoints",
        "deterministic_module_runtime",
    ] {
        let sql =
            format!("DELETE FROM {table} WHERE rowid IN (SELECT rowid FROM {table} LIMIT 1024)");
        loop {
            let sql = sql.clone();
            if vault.write(Db::Intelligence, Priority::Batch, move |s| Ok(s.execute(&sql, ())?))?
                == 0
            {
                break;
            }
        }
    }
    vault.write(Db::Intelligence,Priority::Batch,|s| {
        super::intelligence_overview_snapshot::clear_overview_snapshots(s)?;
        s.execute("DELETE FROM intelligence_jobs WHERE job_type IN ('visit-derive','daily-rollup','structural-rebuild','full-rebuild')",())?;Ok(())
    })?;
    Ok(report)
}

pub(super) fn table_row_count(s: &Session<'_>, table: &str) -> Result<usize> {
    if let Some((_, group)) = build_state::TABLES.iter().find(|(name, _)| *name == table) {
        return Ok(s.query_one(&format!("SELECT COALESCE(SUM(c.value),0) FROM intelligence_counts c JOIN intelligence_generations g ON g.profile_id=c.profile_id AND g.{group}=c.generation WHERE c.table_name=?1"),[table],|r|Ok(r.get::<i64>(0)?))? as usize);
    }
    Ok(s.query_one(&format!("SELECT COUNT(*) FROM {table}"), (), |r| Ok(r.get::<i64>(0)?))?
        as usize)
}
pub(super) fn sum_table_row_counts(s: &Session<'_>, tables: &[&str]) -> Result<usize> {
    tables.iter().try_fold(0, |total, table| Ok(total + table_row_count(s, table)?))
}

fn count_derived_intelligence_state(
    connection: &Session<'_>,
) -> Result<ClearDerivedIntelligenceReport> {
    // Every table the clear deletes from is counted in one of the groups, so the dialog's
    // numbers are what goes.
    let cleared_runtime_rows = sum_table_row_counts(
        connection,
        &[
            "deterministic_module_runtime",
            "core_intelligence_stage_checkpoints",
            "intelligence_counts",
            "intelligence_generations",
            "intelligence_builds",
            "intelligence_overview_snapshots",
        ],
    )? + count_core_intelligence_job_triggers(connection)?
        + count_core_intelligence_jobs(connection)?;
    Ok(ClearDerivedIntelligenceReport {
        cleared_visit_derived_fact_rows: table_row_count(connection, "visit_derived_facts")?,
        cleared_daily_rollup_rows: sum_table_row_counts(
            connection,
            &[
                "domain_daily_rollups",
                "category_daily_rollups",
                "engine_daily_rollups",
                "daily_summary_rollups",
                "intelligence_day_counts",
                "intelligence_seen_domains",
            ],
        )?,
        cleared_structural_rows: sum_table_row_counts(
            connection,
            &[
                "sessions",
                "search_trails",
                "search_trail_members",
                "search_events",
                "search_event_terms",
                "query_families",
                "refind_pages",
                "habit_patterns",
                "reopened_investigations",
                "path_flows",
                "compare_sets",
                "visit_assignments",
                "session_domains",
                "session_paths",
            ],
        )?,
        cleared_runtime_rows,
        notes: vec![
            "Cleared Core Intelligence derived rows, checkpoints, and runtime traces without touching canonical archive facts."
                .to_string(),
        ],
    })
}

pub(super) fn count_core_intelligence_jobs(connection: &Session<'_>) -> Result<usize> {
    connection
        .query_one(
            "SELECT COUNT(*)
             FROM intelligence_jobs
             WHERE job_type IN ('visit-derive', 'daily-rollup', 'structural-rebuild', 'full-rebuild')",
            (),
            |row| Ok(row.get::< i64>(0)?),
        )
        .map(|value| value.max(0) as usize)
        .map_err(Into::into)
}

/// Counts trigger rows attached only to deterministic rebuild jobs.
pub(super) fn count_core_intelligence_job_triggers(connection: &Session<'_>) -> Result<usize> {
    connection
        .query_one(
            "SELECT COUNT(*)
             FROM intelligence_job_triggers
             WHERE job_id IN (
               SELECT id
               FROM intelligence_jobs
               WHERE job_type IN ('visit-derive', 'daily-rollup', 'structural-rebuild', 'full-rebuild')
             )",
            (),
            |row| Ok(row.get::< i64>(0)?),
        )
        .map(|value| value.max(0) as usize)
        .map_err(Into::into)
}
