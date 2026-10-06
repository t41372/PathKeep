//! Failure modes: stale derived rows survive repair, missing optional tables fail, or visible rows are removed. Tests: retained intelligence health regressions.
//! Responsibilities: retain the existing intelligence health algorithms for area 4.
//! Not responsible for: native archive health checks.
use super::*;
/// Verifies that derived AI/insight tables only reference currently visible visits.
pub(crate) fn check_stale_derived_state(connection: &Connection) -> Result<HealthCheck> {
    let mut stale_details = Vec::new();

    for (table, column, label) in [
        ("ai_embeddings", "history_id", "stale AI embeddings"),
        ("search_trail_members", "visit_id", "stale search trail members"),
        ("visit_derived_facts", "visit_id", "stale visit-derived-fact rows"),
    ] {
        let stale = count_rows_without_visible_visit(connection, table, column)?;
        if stale > 0 {
            stale_details.push(format!("{stale} {label}"));
        }
    }

    Ok(if stale_details.is_empty() {
        HealthCheck {
            code: "derived-state-freshness".to_string(),
            name: "Derived state freshness".to_string(),
            ok: true,
            detail: "Derived AI and Core Intelligence tables match the visible visit set."
                .to_string(),
        }
    } else {
        HealthCheck {
            code: "derived-state-freshness".to_string(),
            name: "Derived state freshness".to_string(),
            ok: false,
            detail: stale_details.join(", "),
        }
    })
}

/// Checks whether a table exists in the current archive schema.
fn table_exists(connection: &Connection, table_name: &str) -> Result<bool> {
    let table_count = connection.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = ?1",
        [table_name],
        |row| row.get::<_, i64>(0),
    )?;
    Ok(table_count > 0)
}

/// Counts rows of a derived `table` whose `column` no longer points at a visible visit (the
/// visit was rolled back or is gone). A missing table counts as clean: AI and intelligence tables
/// only exist once those features have run.
///
/// The check is a correlated `NOT EXISTS`, so SQLite looks each row's visit up by primary key.
/// The `NOT IN (SELECT id FROM archive.visits WHERE reverted_at IS NULL)` it replaced first
/// copied every visible visit id into a temporary index on each call: 14.4M ids at the product
/// target. Numbers in `docs/architecture/ipc-performance.md`.
pub(crate) fn count_rows_without_visible_visit(
    connection: &Connection,
    table: &str,
    column: &str,
) -> Result<usize> {
    if !table_exists(connection, table)? {
        return Ok(0);
    }
    let count: i64 =
        connection
            .query_row(&rows_without_visible_visit_sql(table, column), [], |row| row.get(0))?;
    Ok(count.max(0) as usize)
}

fn rows_without_visible_visit_sql(table: &str, column: &str) -> String {
    format!(
        "SELECT COUNT(*)
         FROM {table}
         WHERE NOT EXISTS (
           SELECT 1 FROM archive.visits AS visits
           WHERE visits.id = {table}.{column} AND visits.reverted_at IS NULL
         )"
    )
}

pub(crate) fn count_stale_ai_embeddings(intelligence: &Connection) -> Result<usize> {
    count_rows_without_visible_visit(intelligence, "ai_embeddings", "history_id")
}

pub(crate) fn clear_stale_ai_embeddings(intelligence: &Connection) -> Result<usize> {
    if table_exists(intelligence, "ai_embeddings")? {
        intelligence
            .execute(
                "DELETE FROM ai_embeddings
                 WHERE NOT EXISTS (
                   SELECT 1 FROM archive.visits AS visits
                   WHERE visits.id = ai_embeddings.history_id AND visits.reverted_at IS NULL
                 )",
                [],
            )
            .map_err(Into::into)
    } else {
        Ok(0)
    }
}

/// Clears stale derived intelligence state and marks runtime modules stale for rebuild.
pub(crate) fn invalidate_insight_state(connection: &Connection) -> Result<usize> {
    let mut cleared_rows = 0usize;
    for table_name in [
        "path_flows",
        "reopened_investigations",
        "habit_patterns",
        "source_effectiveness",
        "refind_pages",
        "query_families",
        "search_event_terms",
        "search_events",
        "search_trail_members",
        "search_trails",
        "sessions",
        "daily_summary_rollups",
        "engine_daily_rollups",
        "category_daily_rollups",
        "domain_daily_rollups",
        "visit_derived_facts",
        // Repair rewrites derived tables without moving the archive watermark,
        // so the all-time overview snapshot must be dropped explicitly or it
        // would keep serving pre-repair aggregates.
        "intelligence_overview_snapshots",
    ] {
        if table_exists(connection, table_name)? {
            cleared_rows += connection
                .execute(&format!("DELETE FROM {table_name}"), [])
                .with_context(|| format!("clearing stale derived table {table_name}"))?;
        }
    }
    crate::intelligence_runtime::ensure_intelligence_runtime_schema(connection)?;
    crate::intelligence_runtime::mark_all_deterministic_modules_stale(
        connection,
        crate::models::DERIVED_STALE_VISIBILITY_OR_ROLLBACK_CHANGED,
        "Archive visibility or rollback state changed after the last Core Intelligence rebuild.",
    )?;
    Ok(cleared_rows)
}

/// Keeps area's existing intelligence checks until its store is ported.
pub fn legacy_derived_health(
    vault: &crate::Vault,
    key: Option<&str>,
) -> Result<Option<HealthCheck>> {
    let paths = crate::project_paths_with_root(&vault.paths().app_root);
    if !paths.intelligence_database_path.exists()
        || (vault.config().archive_mode == ArchiveMode::Encrypted && key.is_none())
    {
        return Ok(None);
    }
    let connection = open_intelligence_connection(&paths, vault.config(), key)?;
    Ok(Some(check_stale_derived_state(&connection)?))
}

/// Calls the existing intelligence repair without making it an archive repair dependency.
pub(crate) fn repair_legacy_derived(vault: &crate::Vault, key: Option<&str>) -> Result<usize> {
    let paths = crate::project_paths_with_root(&vault.paths().app_root);
    if !paths.intelligence_database_path.exists()
        || (vault.config().archive_mode == ArchiveMode::Encrypted && key.is_none())
    {
        return Ok(0);
    }
    let connection = open_intelligence_connection(&paths, vault.config(), key)?;
    let stale = count_rows_without_visible_visit(&connection, "search_trail_members", "visit_id")?
        + count_rows_without_visible_visit(&connection, "visit_derived_facts", "visit_id")?;
    let embeddings = if count_stale_ai_embeddings(&connection)? > 0 {
        clear_stale_ai_embeddings(&connection)?
    } else {
        0
    };
    Ok(embeddings + if stale > 0 { invalidate_insight_state(&connection)? } else { 0 })
}

#[cfg(test)]
mod tests {
    use super::{
        clear_stale_ai_embeddings, count_rows_without_visible_visit, count_stale_ai_embeddings,
        rows_without_visible_visit_sql, table_exists,
    };
    use crate::{
        archive::{open_archive_connection, open_intelligence_connection},
        config::project_paths_with_root,
        models::AppConfig,
    };
    use rusqlite::Connection;

    /// A derived row is stale when its visit is hidden (rolled back) or gone. A visible visit
    /// must never count: repair clears every intelligence table when anything is stale.
    #[test]
    fn stale_rows_are_the_ones_whose_visit_is_hidden_or_missing() {
        let connection = Connection::open_in_memory().expect("sqlite");
        connection
            .execute_batch(
                "ATTACH DATABASE ':memory:' AS archive;
                 CREATE TABLE archive.visits (id INTEGER PRIMARY KEY, reverted_at TEXT);
                 INSERT INTO archive.visits (id, reverted_at) VALUES (1, NULL), (2, '2026-01-01');
                 CREATE TABLE visit_derived_facts (visit_id INTEGER PRIMARY KEY);
                 INSERT INTO visit_derived_facts (visit_id) VALUES (1), (2), (3);
                 CREATE TABLE ai_embeddings (history_id INTEGER, provider_id TEXT);
                 INSERT INTO ai_embeddings (history_id, provider_id) VALUES (1, 'p'), (2, 'p'), (3, 'p');",
            )
            .expect("fixture");

        assert_eq!(
            count_rows_without_visible_visit(&connection, "visit_derived_facts", "visit_id")
                .expect("count"),
            2
        );
        assert_eq!(count_stale_ai_embeddings(&connection).expect("count embeddings"), 2);
        assert_eq!(clear_stale_ai_embeddings(&connection).expect("clear"), 2);
        let kept: i64 = connection
            .query_row("SELECT history_id FROM ai_embeddings", [], |row| row.get(0))
            .expect("the visible visit's embedding is kept");
        assert_eq!(kept, 1);
    }

    /// At 14.4M visits the old `NOT IN (SELECT id FROM archive.visits WHERE reverted_at IS NULL)`
    /// built a temporary index of every visible id on each call. The check must look visits up
    /// by primary key instead.
    #[test]
    fn stale_row_checks_look_visits_up_by_primary_key() {
        let root = tempfile::tempdir().expect("tempdir");
        let paths = project_paths_with_root(root.path());
        let config = AppConfig::default();
        drop(open_archive_connection(&paths, &config, None).expect("archive"));
        let intelligence = open_intelligence_connection(&paths, &config, None).expect("open");
        for (table, column) in [
            ("ai_embeddings", "history_id"),
            ("search_trail_members", "visit_id"),
            ("visit_derived_facts", "visit_id"),
        ] {
            let plan = intelligence
                .prepare(&format!(
                    "EXPLAIN QUERY PLAN {}",
                    rows_without_visible_visit_sql(table, column)
                ))
                .expect("plan")
                .query_map([], |row| row.get::<_, String>(3))
                .expect("plan rows")
                .collect::<rusqlite::Result<Vec<_>>>()
                .expect("plan text")
                .join("\n");
            assert!(
                plan.contains("SEARCH visits USING INTEGER PRIMARY KEY")
                    && !plan.contains("LIST SUBQUERY"),
                "{table} must probe visits by id, got:\n{plan}"
            );
        }
    }

    #[test]
    fn stale_ai_embedding_helpers_treat_missing_optional_table_as_clean() {
        let connection = Connection::open_in_memory().expect("sqlite");

        assert!(!table_exists(&connection, "ai_embeddings").expect("table exists"));
        connection
            .execute("CREATE TABLE ai_embeddings (history_id INTEGER)", [])
            .expect("create optional embeddings table");
        assert!(table_exists(&connection, "ai_embeddings").expect("table exists"));
        connection.execute("DROP TABLE ai_embeddings", []).expect("drop optional embeddings table");
        assert_eq!(count_stale_ai_embeddings(&connection).expect("count stale embeddings"), 0);
        assert_eq!(clear_stale_ai_embeddings(&connection).expect("clear stale embeddings"), 0);
    }
    #[test]
    fn legacy_repairs_tolerate_missing_optional_intelligence_tables() {
        for tables in [
            vec!["ai_embeddings", "search_trail_members"],
            vec!["visit_derived_facts"],
            vec!["search_trail_members", "visit_derived_facts"],
        ] {
            let vault = crate::Vault::open_temp().unwrap();
            let paths = project_paths_with_root(&vault.paths().app_root);
            drop(open_archive_connection(&paths, vault.config(), None).unwrap());
            let connection = open_intelligence_connection(&paths, vault.config(), None).unwrap();
            for table in tables {
                connection.execute(&format!("DROP TABLE {table}"), []).unwrap();
            }
            assert_eq!(super::repair_legacy_derived(&vault, None).unwrap(), 0);
        }
    }
}
