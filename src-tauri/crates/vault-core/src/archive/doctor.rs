//! Archive doctor and repair flows.
//!
//! The doctor surface verifies recoverability/trust invariants that sit above
//! ordinary backup success:
//!
//! - manifest chain integrity
//! - snapshot artifact presence
//! - import audit artifact presence
//! - rollback visibility references
//! - derived-state freshness
//!
//! Repair is intentionally conservative. It can regenerate review artifacts,
//! relink broken visibility pointers, and invalidate stale derived state, but it
//! does not rewrite canonical history facts.

use super::*;

/// Runs the doctor checks for the current archive/config state.
pub fn doctor(paths: &ProjectPaths, config: &AppConfig, key: Option<&str>) -> Result<HealthReport> {
    ensure_paths(paths)?;
    let discovered_profiles = discover_profiles().unwrap_or_default();
    let status = archive_status(paths, config, key)?;
    let archive = if status.initialized && status.unlocked {
        Some(open_archive_connection(paths, config, key)?)
    } else {
        None
    };
    let intelligence = if status.initialized && status.unlocked {
        Some(open_intelligence_connection(paths, config, key)?)
    } else {
        None
    };

    let mut checks = Vec::new();
    checks.push(HealthCheck {
        code: "config".to_string(),
        name: "Config".to_string(),
        ok: paths.config_path.exists(),
        detail: paths.config_path.display().to_string(),
    });
    checks.push(HealthCheck {
        code: "browser-sources".to_string(),
        name: "Browser sources".to_string(),
        ok: !discovered_profiles.is_empty(),
        detail: if discovered_profiles.is_empty() {
            "No supported browser profiles were detected in the known source locations.".to_string()
        } else {
            format!(
                "{} supported browser profiles detected across local data roots.",
                discovered_profiles.len()
            )
        },
    });
    checks.push(HealthCheck {
        code: "archive-db".to_string(),
        name: "Archive DB".to_string(),
        ok: paths.archive_database_path.exists(),
        detail: paths.archive_database_path.display().to_string(),
    });
    checks.push(HealthCheck {
        code: "archive-unlock".to_string(),
        name: "Archive Unlock".to_string(),
        ok: status.unlocked,
        detail: if matches!(config.archive_mode, ArchiveMode::Encrypted) {
            "Encrypted archive requires an active session key".to_string()
        } else {
            "Plaintext archive".to_string()
        },
    });

    if let Some(connection) = archive.as_ref() {
        create_schema(connection)?;
        checks.push(HealthCheck {
            code: "schema-version".to_string(),
            name: "Schema version".to_string(),
            ok: current_version(connection)? >= 2,
            detail: format!("current canonical schema version is {}", current_version(connection)?),
        });
        checks.push(check_manifest_chain(connection)?);
        checks.push(check_snapshot_files(connection)?);
        checks.push(check_import_audit_artifacts(connection)?);
        checks.push(check_broken_visibility(connection)?);
    }
    if let Some(connection) = intelligence.as_ref() {
        checks.push(check_stale_derived_state(connection)?);
    }

    Ok(HealthReport { generated_at: now_rfc3339(), checks })
}

/// Repairs the subset of doctor findings that are explicitly recoverable in-place.
pub fn repair_health_issues(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
) -> Result<HealthRepairReport> {
    ensure_paths(paths)?;
    let archive = open_archive_connection(paths, config, key)?;
    let intelligence = open_intelligence_connection(paths, config, key)?;

    let missing_import_audits = missing_import_audit_batches(&archive)?;
    let broken_visibility_rows: usize = archive
        .query_row(
            "SELECT COUNT(*)
             FROM visits
             LEFT JOIN runs
               ON runs.id = visits.reverted_by_run_id
             WHERE visits.reverted_at IS NOT NULL
               AND (visits.reverted_by_run_id IS NULL OR runs.id IS NULL)",
            [],
            |row| row.get::<_, i64>(0),
        )?
        .max(0) as usize;
    let stale_ai_embeddings = count_stale_ai_embeddings(&intelligence)?;
    let stale_insight_state =
        count_rows_without_visible_visit(&intelligence, "search_trail_members", "visit_id")?
            + count_rows_without_visible_visit(&intelligence, "visit_derived_facts", "visit_id")?;

    if missing_import_audits.is_empty()
        && broken_visibility_rows == 0
        && stale_ai_embeddings == 0
        && stale_insight_state == 0
    {
        return Ok(HealthRepairReport {
            run_id: None,
            notes: vec!["Doctor repair found no actionable damage.".to_string()],
            ..HealthRepairReport::default()
        });
    }

    let started_at = now_rfc3339();
    let timezone = current_timezone_name();
    archive.execute(
        "INSERT INTO runs (run_type, trigger, started_at, timezone, status, profile_scope_json, warnings_json, stats_json, due_only)
         VALUES ('doctor', 'manual', ?1, ?2, 'running', '[]', '[]', '{}', 0)",
        params![started_at, timezone],
    )?;
    let run_id = archive.last_insert_rowid();

    let repair_result = (|| -> Result<HealthRepairReport> {
        let mut notes = Vec::new();
        let repaired_audit_paths =
            rewrite_import_audit_artifacts(paths, config, key, &missing_import_audits)?;
        let repaired_import_audits = repaired_audit_paths.len();
        for (batch_id, audit_path) in &repaired_audit_paths {
            archive.execute(
                "UPDATE import_batches SET audit_path = ?1 WHERE id = ?2",
                params![audit_path, batch_id],
            )?;
        }
        if repaired_import_audits > 0 {
            notes.push(format!(
                "Rebuilt {} missing import audit artifact(s).",
                repaired_import_audits
            ));
        }

        let repaired_visibility_rows = archive.execute(
            "UPDATE visits
             SET reverted_by_run_id = ?1
             WHERE reverted_at IS NOT NULL
               AND (
                 reverted_by_run_id IS NULL
                 OR reverted_by_run_id NOT IN (SELECT id FROM runs)
               )",
            [run_id],
        )?;
        if repaired_visibility_rows > 0 {
            notes.push(format!(
                "Re-linked {} reverted visit rows to doctor repair run #{}.",
                repaired_visibility_rows, run_id
            ));
        }

        let cleared_ai_embeddings = clear_stale_ai_embeddings(&intelligence)?;
        if cleared_ai_embeddings > 0 {
            notes.push(format!(
                "Removed {} stale AI embedding rows that pointed at hidden or missing visits.",
                cleared_ai_embeddings
            ));
        }

        let cleared_insight_rows =
            if stale_insight_state > 0 { invalidate_insight_state(&intelligence)? } else { 0 };
        if cleared_insight_rows > 0 {
            notes.push(format!(
                "Cleared {} stale Core Intelligence rows so the next rebuild runs from visible history only.",
                cleared_insight_rows
            ));
        }

        let cleared_derived_rows = cleared_ai_embeddings + cleared_insight_rows;
        let git_commit = if config.git_enabled && repaired_import_audits > 0 {
            let (git_commit, git_warning) = git_audit::commit_all_optional(
                &paths.audit_repo_path,
                "doctor repair import audit artifacts",
            );
            notes.extend(git_warning.map(|warning| warning.message));
            git_commit
        } else {
            None
        };
        if let Some(git_commit) = git_commit {
            for batch_id in &missing_import_audits {
                archive.execute(
                    "UPDATE import_batches SET git_commit = ?1 WHERE id = ?2",
                    params![git_commit, batch_id],
                )?;
            }
            notes.push(format!(
                "Recorded repaired import artifacts in audit commit {}.",
                git_commit
            ));
        }

        Ok(HealthRepairReport {
            run_id: Some(run_id),
            repaired_import_audits,
            repaired_visibility_rows,
            cleared_derived_rows,
            notes,
        })
    })();

    match repair_result {
        Ok(report) => {
            archive.execute(
                "UPDATE runs
                 SET finished_at = ?1,
                     status = 'success',
                     stats_json = ?2,
                     warnings_json = ?3
                 WHERE id = ?4",
                params![
                    now_rfc3339(),
                    serde_json::to_string(&json!({
                        "repairedImportAudits": report.repaired_import_audits,
                        "repairedVisibilityRows": report.repaired_visibility_rows,
                        "clearedDerivedRows": report.cleared_derived_rows,
                    }))?,
                    serde_json::to_string(&report.notes)?,
                    run_id,
                ],
            )?;
            Ok(report)
        }
        Err(error) => {
            archive.execute(
                "UPDATE runs
                 SET finished_at = ?1,
                     status = 'failed',
                     error_message = ?2
                 WHERE id = ?3",
                params![now_rfc3339(), error.to_string(), run_id],
            )?;
            Err(error)
        }
    }
}

/// Validates the manifest hash chain and artifact contents.
fn check_manifest_chain(connection: &Connection) -> Result<HealthCheck> {
    let mut statement = connection.prepare(
        "SELECT id, parent_manifest_id, content_hash, file_path
         FROM manifests
         ORDER BY id ASC",
    )?;
    let rows = statement.query_map([], |row| {
        Ok((
            row.get::<_, i64>(0)?,
            row.get::<_, Option<i64>>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, Option<String>>(3)?,
        ))
    })?;

    let mut previous_id = None;
    let mut previous_hash = None::<String>;
    for row in rows {
        let (id, parent_id, hash, file_path) = row?;
        if previous_id.is_some() && parent_id != previous_id {
            return Ok(HealthCheck {
                code: "manifest-chain".to_string(),
                name: "Manifest chain".to_string(),
                ok: false,
                detail: format!("manifest {id} does not point to the previous manifest id"),
            });
        }
        if let Some(path) = file_path {
            let content = fs::read_to_string(&path)
                .with_context(|| format!("reading manifest artifact {}", path))?;
            let recalculated = sha256_hex(content.as_bytes());
            if recalculated != hash {
                return Ok(HealthCheck {
                    code: "manifest-chain".to_string(),
                    name: "Manifest chain".to_string(),
                    ok: false,
                    detail: format!("manifest hash mismatch at run artifact {}", path),
                });
            }
        }
        previous_id = Some(id);
        previous_hash = Some(hash);
    }

    Ok(HealthCheck {
        code: "manifest-chain".to_string(),
        name: "Manifest chain".to_string(),
        ok: true,
        detail: previous_hash.unwrap_or_else(|| "No manifest artifacts recorded yet.".to_string()),
    })
}

/// Verifies that recorded snapshot artifacts still exist on disk.
fn check_snapshot_files(connection: &Connection) -> Result<HealthCheck> {
    let missing = connection
        .query_row(
            "SELECT file_path
             FROM snapshots
             WHERE file_path IS NOT NULL
             ORDER BY id DESC",
            [],
            |row| row.get::<_, String>(0),
        )
        .optional()?
        .filter(|path| !Path::new(path).exists());

    Ok(match missing {
        Some(path) => HealthCheck {
            code: "snapshot-artifacts".to_string(),
            name: "Snapshot artifacts".to_string(),
            ok: false,
            detail: format!("missing snapshot artifact {}", path),
        },
        None => HealthCheck {
            code: "snapshot-artifacts".to_string(),
            name: "Snapshot artifacts".to_string(),
            ok: true,
            detail: "All recorded snapshot artifacts are present.".to_string(),
        },
    })
}

/// Verifies that every import batch still has a readable review artifact.
fn check_import_audit_artifacts(connection: &Connection) -> Result<HealthCheck> {
    let mut statement = connection.prepare(
        "SELECT id, audit_path
         FROM import_batches
         ORDER BY id DESC",
    )?;
    let mut rows = statement.query([])?;
    let mut missing = None;
    while let Some(row) = rows.next()? {
        let batch_id = row.get::<_, i64>(0)?;
        let audit_path = row.get::<_, Option<String>>(1)?;
        match audit_path {
            Some(path) if !path.is_empty() && Path::new(&path).exists() => continue,
            other => {
                missing = Some((batch_id, other));
                break;
            }
        }
    }

    Ok(match missing {
        Some((batch_id, Some(path))) => HealthCheck {
            code: "import-audit-artifacts".to_string(),
            name: "Import audit artifacts".to_string(),
            ok: false,
            detail: format!("import batch {batch_id} points to a missing audit artifact at {path}"),
        },
        Some((batch_id, None)) => HealthCheck {
            code: "import-audit-artifacts".to_string(),
            name: "Import audit artifacts".to_string(),
            ok: false,
            detail: format!("import batch {batch_id} does not have an audit artifact yet"),
        },
        None => HealthCheck {
            code: "import-audit-artifacts".to_string(),
            name: "Import audit artifacts".to_string(),
            ok: true,
            detail: "All recorded import batches have readable audit artifacts.".to_string(),
        },
    })
}

/// Verifies that hidden visit rows still point at a valid rollback/repair run.
fn check_broken_visibility(connection: &Connection) -> Result<HealthCheck> {
    let broken_visibility: i64 = connection.query_row(
        "SELECT COUNT(*)
         FROM visits
         LEFT JOIN runs
           ON runs.id = visits.reverted_by_run_id
         WHERE visits.reverted_at IS NOT NULL
           AND (visits.reverted_by_run_id IS NULL OR runs.id IS NULL)",
        [],
        |row| row.get(0),
    )?;

    Ok(if broken_visibility > 0 {
        HealthCheck {
            code: "broken-visibility-references".to_string(),
            name: "Broken visibility references".to_string(),
            ok: false,
            detail: format!(
                "{broken_visibility} reverted visit rows are missing the rollback run that should explain their hidden state"
            ),
        }
    } else {
        HealthCheck {
            code: "broken-visibility-references".to_string(),
            name: "Broken visibility references".to_string(),
            ok: true,
            detail: "All hidden visit rows still point at a valid rollback run.".to_string(),
        }
    })
}

/// Verifies that derived AI/insight tables only reference currently visible visits.
fn check_stale_derived_state(connection: &Connection) -> Result<HealthCheck> {
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
fn count_rows_without_visible_visit(
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

fn count_stale_ai_embeddings(intelligence: &Connection) -> Result<usize> {
    count_rows_without_visible_visit(intelligence, "ai_embeddings", "history_id")
}

fn clear_stale_ai_embeddings(intelligence: &Connection) -> Result<usize> {
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

/// Lists import batches whose review artifacts need to be rebuilt.
fn missing_import_audit_batches(connection: &Connection) -> Result<Vec<i64>> {
    let mut statement = connection.prepare(
        "SELECT id, audit_path
         FROM import_batches
         ORDER BY id ASC",
    )?;
    let rows = statement
        .query_map([], |row| Ok((row.get::<_, i64>(0)?, row.get::<_, Option<String>>(1)?)))?;

    let mut batch_ids = Vec::new();
    for row in rows {
        let (batch_id, audit_path) = row?;
        match audit_path {
            Some(path) if !path.is_empty() && Path::new(&path).exists() => {}
            _ => batch_ids.push(batch_id),
        }
    }
    Ok(batch_ids)
}

/// Rebuilds missing import audit artifacts from persisted import batch facts.
fn rewrite_import_audit_artifacts(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    batch_ids: &[i64],
) -> Result<Vec<(i64, String)>> {
    if batch_ids.is_empty() {
        return Ok(Vec::new());
    }

    let mut repair_config = config.clone();
    repair_config.git_enabled = false;
    let mut rewritten = Vec::new();
    for batch_id in batch_ids {
        let (audit_path, _) = crate::takeout::ensure_import_batch_audit_artifact(
            paths,
            &repair_config,
            key,
            *batch_id,
            None,
        )?;
        if let Some(audit_path) = audit_path {
            rewritten.push((*batch_id, audit_path));
        }
    }
    Ok(rewritten)
}

/// Clears stale derived intelligence state and marks runtime modules stale for rebuild.
fn invalidate_insight_state(connection: &Connection) -> Result<usize> {
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
}
