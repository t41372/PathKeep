//! Responsibilities: commit import visibility changes in resumable URL chunks.
//! Not responsible for: parsing, deleting provenance or rebuilding an index.
//! Failure modes: crash loses cursor, shared pages lose visits, replay double-counts,
//! restore turns a partial import into a complete import, concurrent producers change
//! one batch twice, cancellation blocks the queue.
//! Per-profile counters use only changed rows returned by the same transaction; no visit-sized allocation.
//! The profiles whose counted visits changed are kept on the run as it goes, so a resumed undo
//! still names every one of them for the Insights rebuild that follows (`changed_profiles`).
//! A visit another source also recorded (`archive::same_visit`): undoing its counted copy lets
//! the best other copy count; restoring links it again.
//! Tests: batch_search_tests and turso_write_path.
use super::*;
use crate::{
    Vault,
    archive::{same_visit, search_documents, write_runs},
};
use vault_store::{CancelToken, Db, Priority, Session, params};
const URLS_PER_CHUNK: usize = 1_000;

pub(super) fn change_visibility(
    vault: &Vault,
    batch: i64,
    restore: bool,
    cancel: &CancelToken,
    mut progress: impl FnMut(usize),
) -> Result<ImportBatchDetail> {
    let action = if restore { "restore" } else { "revert" };
    let run = vault.write(Db::Archive, Priority::Batch, move |s| begin(s, batch, restore))?;
    let mut changed_profiles = Vec::new();
    if let Some(run) = run {
        let result = (|| {
            loop {
                cancel.check()?;
                let changed = vault
                    .write(Db::Archive, Priority::Batch, move |s| chunk(s, batch, restore, run))?;
                if changed == 0 {
                    break;
                }
                progress(changed);
            }
            vault.write(Db::Archive,Priority::Batch,move |s| {
                write_runs::complete_in(s, run, json!({"importBatchId":batch,"action":action,"profilesProcessed":1}), json!([]))?;
                let text=s.query_one("SELECT summary_json FROM import_batches WHERE id=?1",[batch],|r|Ok(r.get::<String>(0)?))?;
                let stats=s.query_one("SELECT stats_json FROM runs WHERE id=?1",[run],|r|Ok(r.get::<String>(0)?))?;
                let mut summary:Value=serde_json::from_str(&text)?;
                let stats:Value=serde_json::from_str(&stats)?;
                let count=stats["newVisits"].as_u64().unwrap_or(0);
                let profiles=stats["changedProfiles"].as_array().map(|ids|ids.iter().filter_map(Value::as_i64).collect::<Vec<_>>()).unwrap_or_default();
                let at=now_rfc3339();
                let message=if restore {format!("Restored at {at}. Returned {count} hidden history rows to the visible archive view via restore run #{run}.")} else {format!("Reverted at {at}. Soft-hid {count} live history rows from the archive view.")};
                let note=TakeoutNote::new(if restore {"batch-restored"} else {"batch-reverted"},message.clone()).with_count(count as usize).with_at(at).with_run_id(run);
                summary["notes"].as_array_mut().context("batch notes must be an array")?.push(json!(message));
                summary["noteDetails"].as_array_mut().context("batch note details must be an array")?.push(serde_json::to_value(note)?);
                s.execute("UPDATE import_batches SET summary_json=?2 WHERE id=?1",params![batch,summary.to_string()])?;
                s.execute("UPDATE import_batches SET status=CASE WHEN ?2=1 THEN previous_status ELSE 'reverted' END,
                    reverted_at=CASE WHEN ?2=1 THEN NULL ELSE ?3 END,visibility_run_id=NULL,audit_path=NULL WHERE id=?1",
                    params![batch,i64::from(restore),now_rfc3339()])?;
                Ok(same_visit::profile_keys(s,&profiles)?)
            })
        })();
        match result {
            Ok(profiles) => changed_profiles = profiles,
            Err(error) => {
                write_runs::end_failed(vault, run, &error, json!({}), None);
                return Err(error);
            }
        }
    }
    let audit = batches::ensure_import_batch_audit_artifact(
        vault,
        batch,
        Some(if restore { "restored" } else { "reverted" }),
    );
    match run {
        // The rows are hidden or shown again; the record of it follows.
        Some(run) => {
            write_runs::after_success(vault, run, "writing its review record", audit);
        }
        None => {
            audit?;
        }
    }
    let mut detail = preview_import_batch(vault, batch)?;
    detail.changed_profiles = changed_profiles;
    Ok(detail)
}

fn begin(s: &Session<'_>, batch: i64, restore: bool) -> Result<Option<i64>> {
    let (status, profile, run) = s.query_one(
        "SELECT status,profile_id,visibility_run_id FROM import_batches WHERE id=?1",
        [batch],
        |r| Ok((r.get::<String>(0)?, r.get::<String>(1)?, r.get::<Option<i64>>(2)?)),
    )?;
    anyhow::ensure!(
        status != "running",
        "import batch {batch} is still running; cancel it before changing visibility"
    );
    let operation = if restore { "restoring" } else { "reverting" };
    if status == operation {
        let run = run.context("visibility operation has no run")?;
        let running =
            s.query_one("SELECT status FROM runs WHERE id=?1", [run], |r| Ok(r.get::<String>(0)?))?;
        anyhow::ensure!(running != "running", "import batch {batch} is already {operation}");
        s.execute(
            "UPDATE runs SET status='running',finished_at=NULL,error_message=NULL WHERE id=?1",
            [run],
        )?;
        return Ok(Some(run));
    }
    anyhow::ensure!(
        status != "restoring" && status != "reverting",
        "finish the current import visibility operation before reversing it"
    );
    if (restore && status != "reverted") || (!restore && status == "reverted") {
        return Ok(None);
    }
    let run = write_runs::start_in(
        s,
        if restore { "restore" } else { "rollback" },
        "manual",
        &[profile],
    )?;
    s.execute(
        "UPDATE runs SET stats_json=?2 WHERE id=?1",
        params![
            run,
            json!({"importBatchId":batch,"action":if restore {"restore"} else {"revert"}})
                .to_string()
        ],
    )?;
    s.execute("UPDATE import_batches SET previous_status=CASE WHEN ?2=0 THEN status ELSE previous_status END,
        status=?3,visibility_cursor=0,visibility_run_id=?4 WHERE id=?1",params![batch,i64::from(restore),operation,run])?;
    Ok(Some(run))
}

fn chunk(s: &Session<'_>, batch: i64, restore: bool, run: i64) -> Result<usize> {
    let cursor =
        s.query_one("SELECT visibility_cursor FROM import_batches WHERE id=?1", [batch], |r| {
            Ok(r.get::<i64>(0)?)
        })?;
    let ids=s.query("SELECT DISTINCT url_id FROM visits WHERE import_batch_id=?1 AND url_id>?2 ORDER BY url_id LIMIT ?3",
        params![batch,cursor,URLS_PER_CHUNK as i64],|r|Ok(r.get::<i64>(0)?))?;
    let Some(last) = ids.last().copied() else { return Ok(0) };
    let at = now_rfc3339();
    // The visits that change, then the change: not one `UPDATE … RETURNING`, which opens a
    // temporary file per URL (`Session::insert`).
    const CHANGES: &str = "import_batch_id=?1 AND url_id=?2 AND same_as IS NULL
        AND ((?3=1 AND reverted_at IS NOT NULL) OR (?3=0 AND reverted_at IS NULL))";
    let select = format!("SELECT id, source_profile_id, visit_time_ms FROM visits WHERE {CHANGES}");
    let update = format!(
        "UPDATE visits SET reverted_at=CASE WHEN ?3=1 THEN NULL ELSE ?4 END,
         reverted_by_run_id=CASE WHEN ?3=1 THEN NULL ELSE ?5 END WHERE {CHANGES}"
    );
    let mut pages = std::collections::BTreeSet::new();
    let mut visits = 0u64;
    let mut by_profile = std::collections::BTreeMap::<i64, i64>::new();
    for id in &ids {
        // Undo and restore move only the batch's own counted visits: a copy hidden behind
        // another source's copy of the same visit (`same_as`) stays hidden either way.
        let changed = s.query(&select, params![batch, *id, i64::from(restore)], |r| {
            Ok((r.get::<i64>(0)?, r.get::<i64>(1)?, r.get::<i64>(2)?))
        })?;
        s.execute(&update, params![batch, *id, i64::from(restore), at.as_str(), run])?;
        let (page, url) = s.query_one("SELECT page_id, url FROM urls WHERE id=?1", [*id], |r| {
            Ok((r.get::<i64>(0)?, r.get::<String>(1)?))
        })?;
        pages.insert(page);
        for (visit, profile, at_ms) in changed {
            visits += 1;
            *by_profile.entry(profile).or_default() += if restore { 1 } else { -1 };
            if restore {
                // Back, it is linked again: another source's copy may stand in for it, or it
                // may take that copy's place.
                match same_visit::link(s, run, visit, profile, &url, at_ms)? {
                    same_visit::Linked::Alone => {}
                    same_visit::Linked::Hidden { .. } => {
                        *by_profile.entry(profile).or_default() -= 1;
                    }
                    same_visit::Linked::Replaced { stand_in } => {
                        *by_profile.entry(stand_in.profile).or_default() -= 1;
                    }
                }
            } else if let Some(promoted) = same_visit::promote(s, visit, batch)? {
                // The best other copy of the visit counts in its place.
                *by_profile.entry(promoted.profile).or_default() += 1;
            }
        }
    }
    for (&profile, &delta) in &by_profile {
        crate::archive::totals::add_visits(s, profile, delta)?;
    }
    for page in pages {
        search_documents::refresh(s, page)?;
    }
    write_runs::record_progress(
        s,
        run,
        &[("newVisits", visits as usize), ("processedUrls", ids.len())],
    )?;
    write_runs::record_profiles(s, run, "changedProfiles", by_profile.keys().copied())?;
    s.execute(
        "UPDATE import_batches SET visibility_cursor=?2,visible_items=visible_items+?3 WHERE id=?1",
        params![batch, last, if restore { visits as i64 } else { -(visits as i64) }],
    )?;
    Ok(ids.len())
}
