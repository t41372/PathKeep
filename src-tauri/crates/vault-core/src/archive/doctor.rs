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
        name: "Config".to_string(),
        ok: paths.config_path.exists(),
        detail: paths.config_path.display().to_string(),
    });
    checks.push(HealthCheck {
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
        name: "Archive DB".to_string(),
        ok: paths.archive_database_path.exists(),
        detail: paths.archive_database_path.display().to_string(),
    });
    checks.push(HealthCheck {
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
    let stale_ai_embeddings = if table_exists(&intelligence, "ai_embeddings")? {
        intelligence
            .query_row(
                "SELECT COUNT(*)
                 FROM ai_embeddings
                 WHERE history_id NOT IN (
                   SELECT id FROM archive.visits WHERE reverted_at IS NULL
                 )",
                [],
                |row| row.get::<_, i64>(0),
            )?
            .max(0) as usize
    } else {
        0
    };
    let stale_insight_state = if table_exists(&intelligence, "insight_thread_members")?
        || table_exists(&intelligence, "visit_insight_features")?
    {
        let stale_members = if table_exists(&intelligence, "insight_thread_members")? {
            intelligence
                .query_row(
                    "SELECT COUNT(*)
                     FROM insight_thread_members
                     WHERE history_id NOT IN (
                       SELECT id FROM archive.visits WHERE reverted_at IS NULL
                     )",
                    [],
                    |row| row.get::<_, i64>(0),
                )?
                .max(0) as usize
        } else {
            0
        };
        let stale_features = if table_exists(&intelligence, "visit_insight_features")? {
            intelligence
                .query_row(
                    "SELECT COUNT(*)
                     FROM visit_insight_features
                     WHERE history_id NOT IN (
                       SELECT id FROM archive.visits WHERE reverted_at IS NULL
                     )",
                    [],
                    |row| row.get::<_, i64>(0),
                )?
                .max(0) as usize
        } else {
            0
        };
        stale_members + stale_features
    } else {
        0
    };

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

        let cleared_ai_embeddings = if table_exists(&intelligence, "ai_embeddings")? {
            intelligence.execute(
                "DELETE FROM ai_embeddings
                 WHERE history_id NOT IN (
                   SELECT id FROM archive.visits WHERE reverted_at IS NULL
                 )",
                [],
            )?
        } else {
            0
        };
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
                "Cleared {} stale insight rows so the next insight run rebuilds from visible history only.",
                cleared_insight_rows
            ));
        }

        let cleared_derived_rows = cleared_ai_embeddings + cleared_insight_rows;
        let git_commit = if config.git_enabled && repaired_import_audits > 0 {
            git_audit::commit_all(&paths.audit_repo_path, "doctor repair import audit artifacts")?
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
            name: "Snapshot artifacts".to_string(),
            ok: false,
            detail: format!("missing snapshot artifact {}", path),
        },
        None => HealthCheck {
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
            name: "Import audit artifacts".to_string(),
            ok: false,
            detail: format!("import batch {batch_id} points to a missing audit artifact at {path}"),
        },
        Some((batch_id, None)) => HealthCheck {
            name: "Import audit artifacts".to_string(),
            ok: false,
            detail: format!("import batch {batch_id} does not have an audit artifact yet"),
        },
        None => HealthCheck {
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
            name: "Broken visibility references".to_string(),
            ok: false,
            detail: format!(
                "{broken_visibility} reverted visit rows are missing the rollback run that should explain their hidden state"
            ),
        }
    } else {
        HealthCheck {
            name: "Broken visibility references".to_string(),
            ok: true,
            detail: "All hidden visit rows still point at a valid rollback run.".to_string(),
        }
    })
}

/// Verifies that derived AI/insight tables only reference currently visible visits.
fn check_stale_derived_state(connection: &Connection) -> Result<HealthCheck> {
    let mut stale_details = Vec::new();

    if table_exists(connection, "ai_embeddings")? {
        let stale_embeddings: i64 = connection.query_row(
            "SELECT COUNT(*)
             FROM ai_embeddings
             WHERE history_id NOT IN (
               SELECT id FROM archive.visits WHERE reverted_at IS NULL
             )",
            [],
            |row| row.get(0),
        )?;
        if stale_embeddings > 0 {
            stale_details.push(format!("{stale_embeddings} stale AI embeddings"));
        }
    }

    if table_exists(connection, "insight_thread_members")? {
        let stale_members: i64 = connection.query_row(
            "SELECT COUNT(*)
             FROM insight_thread_members
             WHERE history_id NOT IN (
               SELECT id FROM archive.visits WHERE reverted_at IS NULL
             )",
            [],
            |row| row.get(0),
        )?;
        if stale_members > 0 {
            stale_details.push(format!("{stale_members} stale insight thread members"));
        }
    }

    if table_exists(connection, "visit_insight_features")? {
        let stale_features: i64 = connection.query_row(
            "SELECT COUNT(*)
             FROM visit_insight_features
             WHERE history_id NOT IN (
               SELECT id FROM archive.visits WHERE reverted_at IS NULL
             )",
            [],
            |row| row.get(0),
        )?;
        if stale_features > 0 {
            stale_details.push(format!("{stale_features} stale insight feature rows"));
        }
    }

    Ok(if stale_details.is_empty() {
        HealthCheck {
            name: "Derived state freshness".to_string(),
            ok: true,
            detail: "Derived AI and insight tables match the visible visit set.".to_string(),
        }
    } else {
        HealthCheck {
            name: "Derived state freshness".to_string(),
            ok: false,
            detail: stale_details.join(", "),
        }
    })
}

/// Checks whether a table exists in the current archive schema.
fn table_exists(connection: &Connection, table_name: &str) -> Result<bool> {
    Ok(connection.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = ?1",
        [table_name],
        |row| row.get::<_, i64>(0),
    )? > 0)
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

    let mut rewritten = Vec::new();
    for batch_id in batch_ids {
        let (audit_path, _) = crate::takeout::ensure_import_batch_audit_artifact(
            paths, config, key, *batch_id, None,
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
        "insight_cards",
        "insight_reference_pages",
        "insight_source_effectiveness",
        "insight_query_group_members",
        "insight_query_groups",
        "insight_bursts",
        "insight_thread_members",
        "insight_threads",
        "insight_topics",
        "visit_insight_features",
        "insight_runs",
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
        "Archive visibility or rollback state changed after the last deterministic rebuild.",
    )?;
    Ok(cleared_rows)
}
