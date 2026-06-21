//! Core Intelligence schema and derived-state ownership.
//!
//! ## Responsibilities
//! - Own the rebuildable intelligence-plane schema, versioned migrations, and
//!   legacy table cleanup.
//! - Report readiness and clear derived-state surfaces without touching
//!   canonical archive facts.
//! - Keep table-count and scoped clear helpers out of route read models and
//!   rebuild orchestration.
//!
//! ## Not responsible for
//! - Running rebuild stages or progress callbacks.
//! - Serving route-level `/intelligence` read models.
//! - Generating host artifacts.
//!
//! ## Dependencies
//! - Archive intelligence connections from `archive::open_intelligence_connection`.
//! - Site-dictionary and search-rule schema helpers from the parent module.
//! - Incremental checkpoint helpers for full derived-state clears.
//!
//! ## Performance notes
//! - Status and clear operations only scan the tables they report on; they do
//!   not recompute deterministic entities.
//! - Migration backfills run in a single transaction over persisted search
//!   events, so callers must keep them off the UI thread.

use super::intelligence_schema_sql::{
    CORE_INTELLIGENCE_SCHEMA_SQL, INTELLIGENCE_SCHEMA_MIGRATIONS_SQL, LEGACY_INSIGHT_TABLES,
};
use super::{
    classify_search_query_kind, delete_stage_checkpoints, ensure_search_engine_rule_schema,
    ensure_site_dictionary_override_schema,
};
use crate::{
    archive::open_intelligence_connection,
    config::ProjectPaths,
    enrichment::ensure_visit_content_enrichment_schema,
    intelligence_catalog::RebuildMode,
    models::{AppConfig, ClearDerivedIntelligenceReport, IntelligenceStatus},
    utils::now_rfc3339,
};
use anyhow::Result;
use rusqlite::{Connection, OptionalExtension, params};
use std::collections::BTreeSet;

/// Defines one ordered intelligence-plane schema migration.
#[derive(Clone, Copy)]
struct IntelligenceMigrationSpec {
    version: i64,
    name: &'static str,
    apply: fn(&Connection) -> Result<()>,
}

const INTELLIGENCE_MIGRATIONS: &[IntelligenceMigrationSpec] = &[
    IntelligenceMigrationSpec {
        version: 1,
        name: "core-intelligence-baseline",
        apply: apply_core_intelligence_baseline_migration,
    },
    IntelligenceMigrationSpec {
        version: 2,
        name: "site-dictionary-overrides",
        apply: apply_site_dictionary_override_migration,
    },
    IntelligenceMigrationSpec {
        version: 3,
        name: "stage-checkpoints",
        apply: apply_core_intelligence_stage_checkpoint_migration,
    },
    IntelligenceMigrationSpec {
        version: 4,
        name: "batch-read-indexes",
        apply: apply_core_intelligence_batch_index_migration,
    },
    IntelligenceMigrationSpec {
        version: 5,
        name: "search-engine-rules",
        apply: apply_search_engine_rule_migration,
    },
    IntelligenceMigrationSpec {
        version: 6,
        name: "search-query-kind",
        apply: apply_search_query_kind_migration,
    },
    IntelligenceMigrationSpec {
        version: 7,
        name: "overview-snapshots",
        apply: apply_overview_snapshot_migration,
    },
    IntelligenceMigrationSpec {
        version: 8,
        name: "content-enrichment-w-enrich",
        apply: apply_content_enrichment_w_enrich_migration,
    },
];

/// W-ENRICH-1 (migration "015" in the doc-06 naming, applied on the INTELLIGENCE plane where the
/// `visit_content_enrichments` table actually lives — see the module note below): adds
/// `extractor_version` + `enrichment_summary` + `refetch_after` + `http_status` to the enrichment
/// table so the capped summary feeds the dedup content_hash + FTS5 mirror and the negative-cache
/// cadence is persisted. A no-op on a fresh DB (the baseline schema already includes the columns).
fn apply_content_enrichment_w_enrich_migration(connection: &Connection) -> Result<()> {
    // The baseline (v1) created the table; on a DB that already has it, ALTER the new columns in. On a
    // fresh DB the baseline now creates them, so the ALTERs no-op (guarded inside the helper).
    crate::enrichment::add_visit_content_enrichment_w_enrich_columns(connection)
}

/// Installs the baseline intelligence schema on a freshly attached derived
/// database.
fn apply_core_intelligence_baseline_migration(connection: &Connection) -> Result<()> {
    ensure_visit_content_enrichment_schema(connection)?;
    connection.execute_batch(CORE_INTELLIGENCE_SCHEMA_SQL)?;
    Ok(())
}

/// Adds the accepted site-dictionary override surface used by deterministic
/// normalization.
fn apply_site_dictionary_override_migration(connection: &Connection) -> Result<()> {
    ensure_site_dictionary_override_schema(connection)
}

/// Installs incremental checkpoint tables used by staged rebuilds.
fn apply_core_intelligence_stage_checkpoint_migration(connection: &Connection) -> Result<()> {
    super::ensure_core_intelligence_stage_checkpoint_schema(connection)
}

/// Installs mutable search-engine rule tables used by Settings.
fn apply_search_engine_rule_migration(connection: &Connection) -> Result<()> {
    ensure_search_engine_rule_schema(connection)
}

/// Installs the persistent all-time overview snapshot cache table.
fn apply_overview_snapshot_migration(connection: &Connection) -> Result<()> {
    super::intelligence_overview_snapshot::ensure_overview_snapshot_schema(connection)
}

/// Backfills query-kind metadata so keyword surfaces can exclude navigational
/// noise without rebuilding archive facts.
fn apply_search_query_kind_migration(connection: &Connection) -> Result<()> {
    if !table_has_column(connection, "search_events", "query_kind")? {
        connection.execute(
            "ALTER TABLE search_events
             ADD COLUMN query_kind TEXT NOT NULL DEFAULT 'keyword'",
            [],
        )?;
    }
    connection.execute(
        "CREATE INDEX IF NOT EXISTS idx_search_events_profile_kind
         ON search_events(profile_id, query_kind)",
        [],
    )?;
    backfill_search_event_query_kinds(connection)
}

/// Adds the extra composite indexes required by streamed batch rebuilds.
fn apply_core_intelligence_batch_index_migration(connection: &Connection) -> Result<()> {
    connection.execute_batch(
        "CREATE INDEX IF NOT EXISTS idx_vdf_profile_visit_id
           ON visit_derived_facts(profile_id, visit_id);
         CREATE INDEX IF NOT EXISTS idx_search_trails_profile_time_trail
           ON search_trails(profile_id, first_visit_ms ASC, trail_id ASC);
         CREATE INDEX IF NOT EXISTS idx_search_events_profile_visit
           ON search_events(profile_id, visit_id);",
    )?;
    Ok(())
}

/// Detects whether a migration must add a column before issuing the `ALTER`.
fn table_has_column(connection: &Connection, table: &str, column: &str) -> Result<bool> {
    let pragma = format!("PRAGMA table_info({table})");
    let mut statement = connection.prepare(&pragma)?;
    let columns = statement
        .query_map([], |row| row.get::<_, String>(1))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(columns.iter().any(|candidate| candidate == column))
}

/// Reclassifies persisted search events with the current query-kind heuristic
/// without touching any canonical archive rows.
fn backfill_search_event_query_kinds(connection: &Connection) -> Result<()> {
    let mut statement = connection.prepare(
        "SELECT search_events.visit_id,
                search_events.raw_query,
                search_events.normalized_query,
                search_trails.landing_domain
         FROM search_events
         LEFT JOIN search_trails ON search_trails.trail_id = search_events.trail_id",
    )?;
    let rows = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, Option<String>>(3)?,
            ))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    drop(statement);
    let tx = connection.unchecked_transaction()?;
    let mut update = tx.prepare("UPDATE search_events SET query_kind = ?2 WHERE visit_id = ?1")?;
    for (visit_id, raw_query, normalized_query, landing_domain) in rows {
        let query_kind =
            classify_search_query_kind(&raw_query, &normalized_query, landing_domain.as_deref());
        update.execute(params![visit_id, query_kind.as_str()])?;
    }
    drop(update);
    tx.commit()?;
    Ok(())
}

/// Loads the set of already-applied intelligence schema migration versions.
fn load_applied_intelligence_migrations(connection: &Connection) -> Result<BTreeSet<i64>> {
    connection.execute_batch(INTELLIGENCE_SCHEMA_MIGRATIONS_SQL)?;
    let mut statement = connection.prepare(
        "SELECT version
         FROM intelligence_schema_migrations
         ORDER BY version ASC",
    )?;
    statement
        .query_map([], |row| row.get::<_, i64>(0))?
        .collect::<rusqlite::Result<BTreeSet<_>>>()
        .map_err(Into::into)
}

/// Applies every pending intelligence-plane migration in version order.
fn run_core_intelligence_migrations(connection: &Connection) -> Result<()> {
    let applied = load_applied_intelligence_migrations(connection)?;
    for migration in INTELLIGENCE_MIGRATIONS {
        if applied.contains(&migration.version) {
            continue;
        }
        (migration.apply)(connection)?;
        connection.execute(
            "INSERT INTO intelligence_schema_migrations (version, name, applied_at)
             VALUES (?1, ?2, ?3)",
            params![migration.version, migration.name, now_rfc3339()],
        )?;
    }
    Ok(())
}

/// Ensures the intelligence plane is ready for reads and rebuilds before any
/// route surface or background task opens the derived database.
pub(crate) fn ensure_core_intelligence_schema(connection: &Connection) -> Result<()> {
    run_core_intelligence_migrations(connection)?;
    drop_legacy_insight_tables(connection)?;
    Ok(())
}

/// Reports whether deterministic Core Intelligence has materialized enough
/// state to serve the top-level `/intelligence` route.
pub fn intelligence_status(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
) -> Result<IntelligenceStatus> {
    let connection = open_intelligence_connection(paths, config, key)?;
    ensure_core_intelligence_schema(&connection)?;
    let session_count = table_row_count(&connection, "sessions")?;
    let trail_count = table_row_count(&connection, "search_trails")?;
    let refind_count = table_row_count(&connection, "refind_pages")?;
    let last_run_at = connection
        .query_row(
            "SELECT MAX(updated_at) FROM intelligence_jobs
             WHERE job_type IN ('visit-derive', 'daily-rollup', 'structural-rebuild', 'full-rebuild')
               AND state = 'succeeded'",
            [],
            |row| row.get::<_, Option<String>>(0),
        )
        .optional()?
        .flatten();
    Ok(IntelligenceStatus {
        ready: session_count > 0 || trail_count > 0 || refind_count > 0,
        last_run_at,
        runs: 0,
        cards: session_count,
        topics: 0,
        threads: 0,
        query_groups: trail_count,
        reference_pages: refind_count,
        content_coverage: 0.0,
        warning: None,
    })
}

/// Clears only rebuildable intelligence state and runtime traces while leaving
/// the canonical archive untouched.
pub fn clear_derived_intelligence_state(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
) -> Result<ClearDerivedIntelligenceReport> {
    let connection = open_intelligence_connection(paths, config, key)?;
    ensure_core_intelligence_schema(&connection)?;
    let cleared_runtime_rows = table_row_count(&connection, "deterministic_module_runtime")?
        + table_row_count(&connection, "core_intelligence_stage_checkpoints")?
        + count_core_intelligence_job_triggers(&connection)?
        + count_core_intelligence_jobs(&connection)?;
    let report = ClearDerivedIntelligenceReport {
        cleared_visit_derived_fact_rows: table_row_count(&connection, "visit_derived_facts")?,
        cleared_daily_rollup_rows: sum_table_row_counts(
            &connection,
            &[
                "domain_daily_rollups",
                "category_daily_rollups",
                "engine_daily_rollups",
                "daily_summary_rollups",
            ],
        )?,
        cleared_structural_rows: sum_table_row_counts(
            &connection,
            &[
                "sessions",
                "search_trails",
                "search_trail_members",
                "search_events",
                "search_event_terms",
                "query_families",
                "refind_pages",
                "source_effectiveness",
                "habit_patterns",
                "reopened_investigations",
                "path_flows",
            ],
        )?,
        cleared_runtime_rows,
        notes: vec![
            "Cleared Core Intelligence derived rows, checkpoints, and runtime traces without touching canonical archive facts."
                .to_string(),
        ],
    };
    clear_core_tables(&connection, None)?;
    delete_stage_checkpoints(&connection, None)?;
    super::intelligence_overview_snapshot::clear_overview_snapshots(&connection)?;
    connection.execute(
        "DELETE FROM intelligence_jobs
         WHERE job_type IN ('visit-derive', 'daily-rollup', 'structural-rebuild', 'full-rebuild')",
        [],
    )?;
    connection.execute("DELETE FROM deterministic_module_runtime", [])?;
    Ok(report)
}

/// Clears every derived table touched by one rebuild mode across the requested
/// profile scope.
fn clear_core_tables(connection: &Connection, profile_id: Option<&str>) -> Result<()> {
    clear_core_tables_for_job_kind(connection, profile_id, RebuildMode::FullRebuild)
}

/// Clears only the derived tables owned by one rebuild mode so incremental and
/// scoped rebuilds do not wipe unrelated planes.
pub(super) fn clear_core_tables_for_job_kind(
    connection: &Connection,
    profile_id: Option<&str>,
    job_kind: RebuildMode,
) -> Result<()> {
    let tables: &[&str] = match job_kind {
        RebuildMode::VisitDerive => &["visit_derived_facts"],
        RebuildMode::DailyRollup => &[
            "domain_daily_rollups",
            "category_daily_rollups",
            "engine_daily_rollups",
            "daily_summary_rollups",
        ],
        RebuildMode::StructuralRebuild => &[
            "sessions",
            "search_trails",
            "search_trail_members",
            "search_events",
            "search_event_terms",
            "query_families",
            "refind_pages",
            "source_effectiveness",
            "habit_patterns",
            "reopened_investigations",
            "path_flows",
        ],
        RebuildMode::FullRebuild => &[
            "visit_derived_facts",
            "domain_daily_rollups",
            "category_daily_rollups",
            "engine_daily_rollups",
            "daily_summary_rollups",
            "sessions",
            "search_trails",
            "search_trail_members",
            "search_events",
            "search_event_terms",
            "query_families",
            "refind_pages",
            "source_effectiveness",
            "habit_patterns",
            "reopened_investigations",
            "path_flows",
        ],
    };

    if let Some(profile_id) = profile_id {
        for table in tables {
            if *table == "search_trail_members" {
                connection.execute(
                    "DELETE FROM search_trail_members WHERE profile_id = ?1",
                    [profile_id],
                )?;
            } else if *table == "search_event_terms" {
                connection.execute(
                    "DELETE FROM search_event_terms WHERE profile_id = ?1",
                    [profile_id],
                )?;
            } else {
                connection
                    .execute(&format!("DELETE FROM {table} WHERE profile_id = ?1"), [profile_id])?;
            }
        }
    } else {
        for table in tables {
            connection.execute(&format!("DELETE FROM {table}"), [])?;
        }
    }
    Ok(())
}

/// Drops pre-reset legacy insight tables after the new deterministic plane is
/// available so stale schema does not mislead diagnostics or audits.
fn drop_legacy_insight_tables(connection: &Connection) -> Result<()> {
    for table in LEGACY_INSIGHT_TABLES {
        if table_exists(connection, table)? {
            connection.execute(&format!("DROP TABLE IF EXISTS {table}"), [])?;
        }
    }
    Ok(())
}

/// Checks whether a named SQLite table exists before trying to count or drop
/// it.
fn table_exists(connection: &Connection, table: &str) -> Result<bool> {
    connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1)",
            [table],
            |row| row.get::<_, i64>(0),
        )
        .map(|value| value != 0)
        .map_err(Into::into)
}

/// Counts rows in one table while tolerating tables that may not exist yet on
/// a freshly initialized intelligence plane.
pub(super) fn table_row_count(connection: &Connection, table: &str) -> Result<usize> {
    if !table_exists(connection, table)? {
        return Ok(0);
    }
    connection
        .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| row.get::<_, i64>(0))
        .map(|value| value.max(0) as usize)
        .map_err(Into::into)
}

/// Sums row counts across a bounded list of related intelligence tables.
pub(super) fn sum_table_row_counts(connection: &Connection, tables: &[&str]) -> Result<usize> {
    tables.iter().try_fold(0_usize, |count, table| {
        table_row_count(connection, table).map(|table_count| count + table_count)
    })
}

/// Counts only deterministic rebuild jobs, excluding optional AI/runtime work.
pub(super) fn count_core_intelligence_jobs(connection: &Connection) -> Result<usize> {
    connection
        .query_row(
            "SELECT COUNT(*)
             FROM intelligence_jobs
             WHERE job_type IN ('visit-derive', 'daily-rollup', 'structural-rebuild', 'full-rebuild')",
            [],
            |row| row.get::<_, i64>(0),
        )
        .map(|value| value.max(0) as usize)
        .map_err(Into::into)
}

/// Counts trigger rows attached only to deterministic rebuild jobs.
pub(super) fn count_core_intelligence_job_triggers(connection: &Connection) -> Result<usize> {
    if !table_exists(connection, "intelligence_job_triggers")? {
        return Ok(0);
    }
    connection
        .query_row(
            "SELECT COUNT(*)
             FROM intelligence_job_triggers
             WHERE job_id IN (
               SELECT id
               FROM intelligence_jobs
               WHERE job_type IN ('visit-derive', 'daily-rollup', 'structural-rebuild', 'full-rebuild')
             )",
            [],
            |row| row.get::<_, i64>(0),
        )
        .map(|value| value.max(0) as usize)
        .map_err(Into::into)
}
