//! Responsibilities: inspect native files and repair recoverable archive metadata.
//! Not responsible for: file recovery, search index consumption or intelligence algorithms.
//! Failure modes: schema/integrity damage reported healthy, counters drift from facts,
//! broken outbox events, missing artifacts, hidden visits restored by repair, failed
//! repairs recorded as success. Tests: doctor_tests in plaintext and encrypted stores.
//! Turso 0.8.1 supports integrity_check and quick_check, but not foreign_key_check
//! or SQLite's progress-handler interruption. We check relationships explicitly;
//! long repairs check each keyset batch. Structural corruption is reported, never
//! repaired by inventing or deleting canonical facts. Integrity checking is not interruptible.
use super::doctor_facts::{
    BAD_BATCH_TOTAL, BROKEN_OUTBOX, BROKEN_VISIBILITY, DAMAGED_PAGES, incorrect_batch_count,
    incorrect_totals, orphan_facts, outbox_damage,
};
use super::*;
use crate::Vault;
use vault_store::{Db, Priority, Session};
#[cfg(test)]
#[path = "doctor_tests.rs"]
mod tests;

fn check(code: &str, name: &str, ok: bool, detail: impl ToString) -> HealthCheck {
    HealthCheck { code: code.into(), name: name.into(), ok, detail: detail.to_string() }
}
/// Checks all four durable Turso files and canonical metadata without opening another owner.
pub fn doctor(vault: &Vault) -> Result<HealthReport> {
    let profiles = crate::chrome::discover_profiles().unwrap_or_default();
    let mut checks = vec![
        check(
            "config",
            "Config",
            vault.paths().config_path.exists(),
            vault.paths().config_path.display(),
        ),
        check(
            "browser-sources",
            "Browser sources",
            !profiles.is_empty(),
            format!("{} supported browser profiles detected", profiles.len()),
        ),
        check(
            "archive-db",
            "Archive DB",
            vault.paths().archive_db.exists(),
            vault.paths().archive_db.display(),
        ),
        check("archive-unlock", "Archive Unlock", true, "The session owns an open archive"),
    ];
    let mut versions = Vec::new();
    for (db, name, label) in [
        (Db::Archive, "archive", "Archive integrity"),
        (Db::SourceEvidence, "source-evidence", "Source evidence integrity"),
        (Db::Intelligence, "intelligence", "Intelligence data integrity"),
        (Db::Agent, "agent", "Conversation data integrity"),
    ] {
        match vault.read(db, |s| Ok((s.schema_version()?, s.integrity_check()?))) {
            Ok((version, messages)) => {
                versions.push((name, version, crate::store_schema::schema_version(db)));
                checks.push(check(
                    &format!("{name}-integrity"),
                    label,
                    messages == ["ok"],
                    messages.join("; "),
                ));
            }
            Err(error) => {
                checks.push(check(&format!("{name}-integrity"), label, false, format!("{error:#}")))
            }
        }
    }
    checks.push(check(
        "schema-version",
        "Schema version",
        versions.len() == 4 && versions.iter().all(|(_, actual, expected)| actual == expected),
        format!("{versions:?}"),
    ));
    checks.extend(vault.read(Db::Archive, |s| {
        let visibility = s.query_one(
            &format!("SELECT COUNT(*) FROM visits WHERE {BROKEN_VISIBILITY}"),
            (),
            |r| Ok(r.get::<i64>(0)?),
        )?;
        let orphan = orphan_facts(s)?;
        let totals = incorrect_totals(s)?;
        let batch_totals = incorrect_batch_count(s)?;
        let outbox = outbox_damage(s)?;
        Ok(vec![
            check(
                "canonical-references",
                "Canonical references",
                orphan == 0,
                format!("{orphan} orphan references"),
            ),
            check(
                "broken-visibility-references",
                "Broken visibility references",
                visibility == 0,
                format!("{visibility} hidden visits lack a rollback run"),
            ),
            check(
                "cached-totals",
                "Cached totals",
                totals.is_empty() && batch_totals == 0,
                format!("{} counters differ from facts", totals.len() + batch_totals as usize),
            ),
            check(
                "search-outbox",
                "Search outbox",
                outbox == 0,
                format!("{outbox} missing documents or invalid latest events"),
            ),
            check_manifest_chain(s)?,
            check_snapshot_files(s)?,
            check_import_audits(s)?,
        ])
    })?);
    Ok(HealthReport { generated_at: now_rfc3339(), checks })
}

/// Rebuilds metadata from canonical facts; never unhides or deletes visits.
pub fn repair_health_issues(vault: &Vault) -> Result<HealthRepairReport> {
    let derived_damage = !super::doctor_derived::derived_health(vault)?.ok;
    let (missing, needs_repair) = vault.read(Db::Archive, |s| {
        let missing = missing_import_audits(s)?;
        let hidden = s.query_one(
            &format!("SELECT COUNT(*) FROM visits WHERE {BROKEN_VISIBILITY}"),
            (),
            |r| Ok(r.get::<i64>(0)?),
        )?;
        let damaged = !missing.is_empty()
            || hidden > 0
            || !incorrect_totals(s)?.is_empty()
            || incorrect_batch_count(s)? > 0
            || outbox_damage(s)? > 0;
        Ok((missing, damaged))
    })?;
    if !needs_repair && !derived_damage {
        return Ok(HealthRepairReport {
            notes: vec!["Doctor repair found no actionable damage.".into()],
            ..Default::default()
        });
    }
    let run = super::write_runs::start(vault, "doctor", "manual", &[])?;
    let result = (|| {
        let mut report = if needs_repair {
            repair_native_metadata(vault, run, &missing)?
        } else {
            HealthRepairReport { run_id: Some(run), ..Default::default() }
        };
        if derived_damage {
            report.cleared_derived_rows = super::doctor_derived::repair_derived(vault)?;
        }
        append_repair_notes(&mut report);
        Ok(report)
    })();
    let result = result.and_then(|report| {
        super::write_runs::complete(
            vault,
            run,
            serde_json::to_value(&report)?,
            json!([]),
            now_rfc3339(),
        )?;
        Ok(report)
    });
    if let Err(error) = &result {
        super::write_runs::end_failed(vault, run, error, json!({}), None);
    }
    result
}

fn repair_native_metadata(vault: &Vault, run: i64, missing: &[i64]) -> Result<HealthRepairReport> {
    let repaired = rewrite_import_audit_artifacts(vault, missing)?;
    let (refreshed_search_pages, queued_search_updates) = repair_search_metadata(vault)?;
    let repaired_batch_totals = repair_batch_totals(vault)?;
    let notes = record_audit_commit(vault, &repaired)?;
    let (repaired_visibility_rows,repaired_total_counters)=vault.write(Db::Archive,Priority::Batch,move |s| {
        let changed=s.execute(&format!("UPDATE visits SET reverted_by_run_id=?1 WHERE {BROKEN_VISIBILITY}"),[run])?;
        let totals=incorrect_totals(s)?;
        let count=totals.len();
        for (name,value) in totals {
            s.execute("INSERT INTO archive_totals(name,value) VALUES(?1,?2) ON CONFLICT(name) DO UPDATE SET value=excluded.value",vault_store::params![name,value])?;
        }
        Ok((changed as usize,count))
    })?;
    Ok(HealthRepairReport {
        run_id: Some(run),
        repaired_import_audits: repaired.len(),
        repaired_visibility_rows,
        repaired_total_counters,
        repaired_batch_totals,
        refreshed_search_pages,
        queued_search_updates,
        notes,
        ..Default::default()
    })
}

fn append_repair_notes(report: &mut HealthRepairReport) {
    for (amount, label) in [
        (report.repaired_import_audits, "Rebuilt import audit artifacts"),
        (report.repaired_visibility_rows, "Re-linked reverted visit rows"),
        (report.repaired_total_counters, "Recomputed archive total counters"),
        (report.repaired_batch_totals, "Recomputed import visibility counts"),
        (report.refreshed_search_pages, "Refreshed page summaries"),
        (report.queued_search_updates, "Queued search updates"),
        (report.cleared_derived_rows, "Cleared stale derived rows"),
    ] {
        if amount > 0 {
            report.notes.push(format!("{label}: {amount}."));
        }
    }
}

fn record_audit_commit(vault: &Vault, repaired: &[(i64, String)]) -> Result<Vec<String>> {
    if !vault.config().git_enabled || repaired.is_empty() {
        return Ok(Vec::new());
    }
    let (commit, warning) = git_audit::commit_all_optional(
        &vault.paths().audit_repo_path,
        "doctor: restore import audit artifacts\n\nKeep import provenance reviewable after an artifact is removed.",
    );
    let mut notes = warning.map(|w| vec![w.message]).unwrap_or_default();
    if let Some(commit) = commit {
        notes.push(format!("Recorded repaired import artifacts in audit commit {commit}."));
        let ids = repaired.iter().map(|(id, _)| *id).collect::<Vec<_>>();
        vault.write(Db::Archive, Priority::Interactive, move |s| {
            for id in ids {
                s.execute(
                    "UPDATE import_batches SET git_commit=?2 WHERE id=?1",
                    vault_store::params![id, commit.as_str()],
                )?;
            }
            Ok(())
        })?;
    }
    Ok(notes)
}

fn repair_search_metadata(vault: &Vault) -> Result<(usize, usize)> {
    let mut cursor = 0;
    let (mut pages, mut updates) = (0, 0);
    loop {
        let ids = vault.read(Db::Archive, move |s| {
            Ok(s.query(
                &format!(
                    "SELECT id FROM pages WHERE id>?1 AND ({DAMAGED_PAGES}) ORDER BY id LIMIT 1000"
                ),
                [cursor],
                |r| Ok(r.get::<i64>(0)?),
            )?)
        })?;
        let Some(last) = ids.last().copied() else { break };
        pages += ids.len();
        updates += ids.len();
        vault.write(Db::Archive, Priority::Batch, move |s| {
            for id in ids {
                super::search_documents::refresh(s, id)?;
            }
            Ok(())
        })?;
        cursor = last;
    }
    loop {
        let ids=vault.read(Db::Archive,|s|Ok(s.query(&format!("SELECT page_id FROM search_outbox WHERE {BROKEN_OUTBOX} ORDER BY seq LIMIT 1000"),(),|r|Ok(r.get::<i64>(0)?))?))?;
        if ids.is_empty() {
            break;
        }
        updates += ids.len();
        vault.write(Db::Archive,Priority::Batch,move |s| {
            for id in ids {
                s.execute("INSERT INTO search_outbox(page_id,change) SELECT ?1,CASE WHEN EXISTS(SELECT 1 FROM search_documents WHERE page_id=?1) THEN 'upsert' ELSE 'delete' END",[id])?;
            }
            Ok(())
        })?;
    }
    Ok((pages, updates))
}

fn repair_batch_totals(vault: &Vault) -> Result<usize> {
    let mut total = 0;
    loop {
        let changed=vault.write(Db::Archive,Priority::Batch,|s| {
            let ids=s.query(&format!("SELECT id FROM import_batches WHERE {BAD_BATCH_TOTAL} ORDER BY id LIMIT 1000"),(),|r|Ok(r.get::<i64>(0)?))?;
            for id in &ids {
                s.execute("UPDATE import_batches SET visible_items=(SELECT COUNT(*) FROM visits WHERE import_batch_id=?1 AND reverted_at IS NULL) WHERE id=?1",[*id])?;
            }
            Ok(ids.len())
        })?;
        if changed == 0 {
            return Ok(total);
        }
        total += changed;
    }
}

fn check_manifest_chain(s: &Session<'_>) -> Result<HealthCheck> {
    let mut previous = None;
    let mut problem = None;
    s.for_each(
        "SELECT id,parent_manifest_id,content_hash,file_path FROM manifests ORDER BY id",
        (),
        |r| {
            let id: i64 = r.get(0)?;
            let parent: Option<i64> = r.get(1)?;
            if previous.is_some() && parent != previous {
                problem.get_or_insert_with(|| {
                    format!("manifest {id} does not point to the previous manifest")
                });
            }
            if let Some(path) = r.get::<Option<String>>(3)? {
                match fs::read(&path) {
                    Ok(bytes) if sha256_hex(&bytes) == r.get::<String>(2)? => (),
                    Ok(_) => {
                        problem.get_or_insert_with(|| format!("manifest hash mismatch: {path}"));
                    }
                    Err(error) => {
                        problem.get_or_insert_with(|| {
                            format!("missing manifest artifact {path}: {error}")
                        });
                    }
                }
            }
            previous = Some(id);
            Ok(())
        },
    )?;
    Ok(check(
        "manifest-chain",
        "Manifest chain",
        problem.is_none(),
        problem.unwrap_or_else(|| "All recorded manifests agree.".into()),
    ))
}
fn check_snapshot_files(s: &Session<'_>) -> Result<HealthCheck> {
    let mut missing = 0;
    s.for_each("SELECT file_path FROM snapshots", (), |r| {
        if !Path::new(&r.get::<String>(0)?).exists() {
            missing += 1;
        }
        Ok(())
    })?;
    Ok(check(
        "snapshot-artifacts",
        "Snapshot artifacts",
        missing == 0,
        format!("{missing} missing snapshot artifacts"),
    ))
}
fn missing_import_audits(s: &Session<'_>) -> Result<Vec<i64>> {
    let mut missing = Vec::new();
    s.for_each("SELECT id,audit_path FROM import_batches WHERE status NOT IN ('running','reverting','restoring') ORDER BY id",(),|r| {
        if r.get::<Option<String>>(1)?.is_none_or(|p|p.is_empty() || !Path::new(&p).exists()) {missing.push(r.get(0)?);}
        Ok(())
    })?;
    Ok(missing)
}
fn check_import_audits(s: &Session<'_>) -> Result<HealthCheck> {
    let missing = missing_import_audits(s)?;
    Ok(check(
        "import-audit-artifacts",
        "Import audit artifacts",
        missing.is_empty(),
        format!("{} import batches do not have an audit artifact", missing.len()),
    ))
}
/// Rebuilds missing import audit artifacts from persisted import batch facts.
fn rewrite_import_audit_artifacts(
    vault: &crate::Vault,
    batch_ids: &[i64],
) -> Result<Vec<(i64, String)>> {
    if batch_ids.is_empty() {
        return Ok(Vec::new());
    }

    let mut repair_config = vault.config().clone();
    repair_config.git_enabled = false;
    let vault = vault.with_config(repair_config);
    let mut rewritten = Vec::new();
    for batch_id in batch_ids {
        let (audit_path, _) =
            crate::takeout::ensure_import_batch_audit_artifact(&vault, *batch_id, None)?;
        if let Some(audit_path) = audit_path {
            rewritten.push((*batch_id, audit_path));
        }
    }
    Ok(rewritten)
}
