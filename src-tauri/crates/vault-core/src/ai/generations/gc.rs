//! Responsibilities: reserve unowned paths, revalidate ownership, and reclaim outside registry.
//! Not responsible for: publishing indexes or inventing ownership from a dangling pointer.
use super::*;
use std::fs;
use vault_store::Priority;

/// Collector admission is an ownership token; no mutex spans database or slow file work.
pub fn collect(vault: &Vault, heal_rows: bool) -> Result<()> {
    let Some(_collection) = vault.vector_generations.admit() else { return Ok(()) };
    let root = &vault.paths().vectors_dir;
    fs::create_dir_all(root)?;
    let trash = root.join("trash");
    fs::create_dir_all(&trash)?;
    let mut failures = Vec::new();
    if let Err(error) = clean_trash(vault, &trash) {
        failures.push(format!("{error:#}"));
    }
    // Clear invalid pointers before deletion decisions, preserving explicit recovery ownership.
    // An ordinary empty ledger must not retain a failed replacement beside a complete fallback.
    // Either failing is retried next pass and must not stop reclamation of every other lineage;
    // each lineage below still rechecks its own pointer, and a dangling one deletes nothing.
    if let Err(error) = super::legacy::cleanup_old_copies(vault, heal_rows) {
        failures.push(format!("{error:#}"));
    }
    if let Err(error) = repair_pointers(vault) {
        failures.push(format!("{error:#}"));
    }
    if heal_rows {
        if let Err(error) = super::partial::recover_pending(vault) {
            failures.push(format!("{error:#}"));
        }
    }
    failures.extend(discover_row_lineages(vault));
    for table in fs::read_dir(root)? {
        let result = (|| -> Result<()> {
            let table = table?;
            if !table.file_type()?.is_dir()
                || matches!(table.file_name().to_str(), Some("rebuild" | "trash"))
            {
                return Ok(());
            }
            for lineage in fs::read_dir(table.path())? {
                if vault.vector_generations.closed.load(std::sync::atomic::Ordering::Acquire) {
                    break;
                }
                let result = (|| -> Result<()> {
                    let lineage = lineage?;
                    if lineage.file_type()?.is_dir() {
                        collect_lineage(vault, &lineage.path(), &trash, heal_rows)?;
                    }
                    Ok(())
                })();
                if let Err(error) = result {
                    failures.push(format!("{error:#}"));
                }
            }
            let _ = fs::remove_dir(table.path());
            Ok(())
        })();
        if let Err(error) = result {
            failures.push(format!("{error:#}"));
        }
    }
    // Snapshot outside registry: even existence checks must not hold up lease drops.
    let retired =
        vault.vector_generations.registry.lock().unwrap_or_else(|e| e.into_inner()).retired.clone();
    let missing = retired.into_iter().filter(|path| !path.exists()).collect::<Vec<_>>();
    {
        let mut registry =
            vault.vector_generations.registry.lock().unwrap_or_else(|e| e.into_inner());
        for path in missing {
            registry.retired.remove(&path);
        }
    }
    if let Err(error) = clean_trash(vault, &trash) {
        failures.push(format!("{error:#}"));
    }
    for error in &failures {
        eprintln!("pathkeep: semantic index GC will retry: {error}");
    }
    ensure!(failures.is_empty(), "semantic index GC: {}", failures.join("; "));
    Ok(())
}

pub(super) fn repair_pointers(vault: &Vault) -> Result<()> {
    let rows = vault.read(Db::Intelligence, |s| {
        Ok(s.query(
            "SELECT provider_id,model,sidecar_table FROM ai_index_ledger WHERE sidecar_table<>''",
            (),
            |r| Ok((r.get::<String>(0)?, r.get::<String>(1)?, r.get::<String>(2)?)),
        )?)
    })?;
    for (provider, model, old) in rows {
        // A pointer that is not a generation path is a v0.4.0 flat index the startup move has
        // not finished (`legacy::migrate`); clearing it would lose the index the move carries.
        if !valid_pointer(&old, &provider, &model) {
            continue;
        }
        let lease = {
            let mut registry =
                vault.vector_generations.registry.lock().unwrap_or_else(|e| e.into_inner());
            let path = vault.paths().vectors_dir.join(&old);
            // Reservation and pointer movement are transient, not evidence of missing files.
            if registry.collecting.iter().any(|reserved| path.starts_with(reserved)) {
                continue;
            }
            pin(vault, old.clone(), old.split('/').nth(1).unwrap().to_owned(), &mut registry)
        };
        let usable = match validate_serving(&lease) {
            Ok(()) => true,
            Err(error) if serving_defect(&error) => false,
            Err(error) => {
                eprintln!(
                    "pathkeep: semantic index pointer kept; its files could not be read this time: {error:#}"
                );
                true
            }
        };
        if !usable {
            vault.write(Db::Intelligence, Priority::Batch, move |s| {
                // A concurrent publication always wins over this repair.
                s.execute("UPDATE ai_index_ledger SET sidecar_table='',state='incomplete',failure_reason=?4 WHERE provider_id=?1 AND model=?2 AND sidecar_table=?3",params![provider,model,old,MISSING_GENERATION])?;
                Ok(())
            })?;
        }
    }
    Ok(())
}

struct Ownership {
    serving: Option<PathBuf>,
    keep: bool,
}
fn ownership(vault: &Vault, dir: &Path, id: Option<&LineageIdentity>) -> Result<Ownership> {
    let path = dir.to_owned();
    let root = vault.paths().vectors_dir.clone();
    let id = id.cloned();
    vault.read(Db::Intelligence, move |s| {
        if let Some(id) = id {
            let row = s.query_one("SELECT sidecar_table,state,last_cleared_at FROM ai_index_ledger WHERE provider_id=?1 AND model=?2",params![id.provider_id,id.model],|r| Ok((r.get::<String>(0)?,r.get::<String>(1)?,r.get::<Option<String>>(2)?))).optional()?;
            let pointer = row.as_ref().filter(|(p,_,_)| valid_pointer(p,&id.provider_id,&id.model)).map(|(p,_,_)| p.clone());
            let cleared_at = row.as_ref().and_then(|(_,_,cleared)| cleared.clone());
            let serving = pointer.as_ref().map(|p| root.join(p));
            let recovering = recovery_note(s,&id.provider_id,&id.model)?.is_some();
            let active = if let Some(job) = id.job_id {
                let state = s.query_one("SELECT state,finished_at FROM ai_jobs WHERE id=?1",[job],|r| Ok((r.get::<String>(0)?,r.get::<Option<String>>(1)?))).optional()?;
                match state {
                    Some((state, _)) if matches!(state.as_str(), "queued" | "running" | "paused" | "stale") => true,
                    // A failed job can be replayed from its cursor, so its lineage waits for the
                    // user — until something replaced the index it was building. Nothing ever
                    // changes a failed job, so without this its files would stay for good.
                    Some((state, finished_at)) if state == "failed" => {
                        !replaced_since(job, finished_at.as_deref(), pointer.as_deref(), cleared_at.as_deref())
                    }
                    _ => false,
                }
            } else { false };
            let serves = serving.as_ref().is_some_and(|p| p.parent()==Some(path.as_path()));
            Ok(Ownership { serving, keep: recovering || active || serves })
        } else {
            // An interrupted first identity write may still have a queue owner or serving pin.
            let name=path.file_name().and_then(|s| s.to_str()).unwrap_or_default();
            let job=name.strip_prefix('j').and_then(|s| s.parse::<i64>().ok());
            // Without its identity file the job never got far enough to be worth replaying.
            let active=if let Some(job)=job {s.query_one("SELECT EXISTS(SELECT 1 FROM ai_jobs WHERE id=?1 AND state IN ('queued','running','paused','stale'))",[job],|r| Ok(r.get::<i64>(0)?))?!=0} else {false};
            let pointers=s.query("SELECT sidecar_table FROM ai_index_ledger WHERE sidecar_table<>''",(),|r| Ok(r.get::<String>(0)?))?;
            let serving=pointers.into_iter().map(|pointer| root.join(pointer)).find(|pointer| pointer.parent()==Some(path.as_path()));
            Ok(Ownership { keep: active || serving.is_some(), serving })
        }
    })
}

/// Whether the index a failed job was building has been replaced since it failed: a later job's
/// lineage serves its provider and model (lineages are named `j<job id>`), or that provider and
/// model were cleared after the job ended (the ledger's `last_cleared_at`, kept per provider and
/// model, so a reset of another model does not count, and a reset replayed from an older job
/// does). A later update of the lineage that was serving does not count: a replayed rebuild
/// still has its work to do.
fn replaced_since(
    job: i64,
    failed_at: Option<&str>,
    pointer: Option<&str>,
    cleared_at: Option<&str>,
) -> bool {
    let serving_job = pointer
        .and_then(|pointer| pointer.split('/').nth(1))
        .and_then(|lineage| lineage.strip_prefix('j'))
        .and_then(|id| id.parse::<i64>().ok());
    if serving_job.is_some_and(|serving| serving > job) {
        return true;
    }
    let time = |at: Option<&str>| at.and_then(|at| chrono::DateTime::parse_from_rfc3339(at).ok());
    match (time(cleared_at), time(failed_at)) {
        (Some(cleared), Some(failed)) => cleared > failed,
        _ => false,
    }
}

/// A reservation prevents new reader/writer pins while ownership is rechecked outside the lock.
struct Reservation<'a> {
    state: &'a GenerationState,
    paths: Vec<PathBuf>,
}
impl Drop for Reservation<'_> {
    fn drop(&mut self) {
        let mut registry = self.state.registry.lock().unwrap_or_else(|e| e.into_inner());
        for path in &self.paths {
            registry.collecting.remove(path);
        }
        self.state.released.notify_all();
    }
}
fn collect_lineage(vault: &Vault, dir: &Path, trash: &Path, heal_rows: bool) -> Result<()> {
    if super::partial::pending(dir) {
        return Ok(());
    }
    let id = match identity(dir) {
        Ok(id) => Some(id),
        Err(error)
            if error
                .downcast_ref::<std::io::Error>()
                .is_some_and(|e| e.kind() == std::io::ErrorKind::NotFound) =>
        {
            None
        }
        Err(error) => return Err(error),
    };
    if let Some(id) = &id {
        ensure!(
            dir.parent().and_then(|p| p.file_name()).and_then(|p| p.to_str())
                == Some(provider_table_name(&id.provider_id, &id.model).as_str()),
            "lineage identity belongs to another index"
        );
    }
    let entries = fs::read_dir(dir)?
        .map(|e| e.and_then(|e| Ok((e.path(), e.file_type()?.is_dir()))))
        .collect::<std::io::Result<Vec<_>>>()?;
    // Never reserve the generation a pointer names. A build that took the reservation for a
    // missing index would start a new lineage and embed every visit again. Builds and partial
    // recovery publish only while a writer pins the new generation (the legacy move runs at open,
    // before any collector), but a writer can drop its pin between this look and the reservation.
    // The ownership check after reserving still keeps that generation, and readers and builds
    // wait out the reservation instead of reporting no index.
    let named = ownership(vault, dir, id.as_ref())?.serving;
    #[cfg(test)]
    vault.vector_generations.test_hook("before_reservation");
    let mut reservation = Reservation { state: &vault.vector_generations, paths: Vec::new() };
    let whole = {
        let mut registry = reservation.state.registry.lock().unwrap_or_else(|e| e.into_inner());
        let whole = !registry.pins.keys().any(|p| p.starts_with(dir))
            && named.as_ref().is_none_or(|named| named.parent() != Some(dir));
        if whole {
            registry.collecting.insert(dir.to_owned());
            reservation.paths.push(dir.to_owned());
        } else {
            for (path, is_dir) in &entries {
                if *is_dir && !registry.pins.contains_key(path) && Some(path) != named.as_ref() {
                    registry.collecting.insert(path.clone());
                    reservation.paths.push(path.clone());
                }
            }
        }
        whole
    };
    // A publication before reservation is visible here; publication after it needs an existing
    // writer pin, which prevents a whole-lineage reservation and protects that generation.
    #[cfg(test)]
    vault.vector_generations.test_hook("after_reservation");
    let owned = ownership(vault, dir, id.as_ref())?;
    // A dangling pointer discovered after preflight cannot authorize *any* deletion for this pair.
    if let Some(serving) = &owned.serving
        && !serving.is_dir()
    {
        return Ok(());
    }
    let mut doomed = Vec::new();
    {
        let mut registry = reservation.state.registry.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(serving) = &owned.serving {
            registry.retired.remove(serving);
        }
        for (path, is_dir) in &entries {
            if *is_dir && Some(path) != owned.serving.as_ref() {
                registry.retired.insert(path.clone());
                if whole || reservation.paths.contains(path) {
                    doomed.push(path.clone());
                }
            }
        }
    }
    // Damaged-pointer/migration ownership survives failed attempts. An ordinary empty ledger
    // never grants this retention; abandoned replacements beside a complete fallback can go.
    let recovering = recovering_pair(vault, id.as_ref())?;
    if recovering {
        return Ok(());
    }
    for path in doomed {
        trash_path(vault, &path, trash)?;
    }
    if !whole || owned.keep {
        return Ok(());
    }
    for (path, is_dir) in &entries {
        if !*is_dir && path.file_name().is_some_and(|name| name != "identity.json") {
            trash_path(vault, path, trash)?;
        }
    }
    if !heal_rows && id.is_some() {
        return Ok(());
    }
    if let Some(id) = id {
        super::super::indexing::store_rows::remove_lineage_rows(
            vault,
            &id.provider_id,
            &id.model,
            dir.file_name().unwrap().to_str().ok_or_else(|| anyhow::anyhow!("invalid lineage"))?,
        )?;
    }
    trash_path(vault, dir, trash)
}
fn recovering_pair(vault: &Vault, id: Option<&LineageIdentity>) -> Result<bool> {
    let Some(id) = id.cloned() else { return Ok(false) };
    vault.read(Db::Intelligence, move |s| {
        Ok(recovery_note(s, &id.provider_id, &id.model)?.is_some())
    })
}
fn trash_path(vault: &Vault, path: &Path, trash: &Path) -> Result<()> {
    let mut nonce = [0; 16];
    getrandom::fill(&mut nonce)?;
    let name = format!(
        "{}-{}-{}",
        hex::encode(nonce),
        std::process::id(),
        TRASH_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    );
    let target = trash.join(name);
    {
        let mut registry =
            vault.vector_generations.registry.lock().unwrap_or_else(|e| e.into_inner());
        // Only an already-reserved path can reach here. Rename is the sole file operation under
        // registry; recursive deletion, file data and directory barriers always happen outside it.
        fs::rename(path, &target)?;
        registry.retired.remove(path);
    }
    crate::durable_io::fsync_dir(trash)?;
    crate::durable_io::fsync_dir(path.parent().unwrap())?;
    Ok(())
}
static TRASH_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
/// One entry that cannot be deleted yet (a file still open on Windows) must not keep every
/// later entry in the trash; the failure is reported after the others are gone.
fn clean_trash(vault: &Vault, dir: &Path) -> Result<()> {
    let mut failure = None;
    for entry in fs::read_dir(dir)? {
        if vault.vector_generations.is_closed() {
            return Ok(());
        }
        #[cfg(test)]
        vault.vector_generations.test_hook("before_trash_delete");
        let entry = entry?;
        let removed = if entry.file_type()?.is_dir() {
            fs::remove_dir_all(entry.path())
        } else {
            fs::remove_file(entry.path())
        };
        if let Err(error) = removed {
            failure.get_or_insert(error);
        }
    }
    crate::durable_io::fsync_dir(dir)?;
    match failure {
        Some(error) => Err(error.into()),
        None => Ok(()),
    }
}

// Totals are one row per lineage; missing source identities remain retryable without row scans.
fn discover_row_lineages(vault: &Vault) -> Vec<String> {
    let rows = vault.read(Db::Intelligence, |s| {
        Ok(s.query("SELECT provider_id,model,lineage FROM ai_embedding_totals", (), |r| {
            Ok((r.get::<String>(0)?, r.get::<String>(1)?, r.get::<String>(2)?))
        })?)
    });
    let rows = match rows {
        Ok(rows) => rows,
        Err(e) => return vec![format!("discovering lineages: {e:#}")],
    };
    let mut failures = Vec::new();
    for (provider, model, lineage) in rows {
        let result = (|| -> Result<()> {
            ensure!(valid_name(&lineage), "invalid embedding lineage");
            let dir = vault
                .paths()
                .vectors_dir
                .join(provider_table_name(&provider, &model))
                .join(&lineage);
            if dir.join("identity.json").exists() {
                return Ok(());
            }
            let reservation = {
                let mut registry =
                    vault.vector_generations.registry.lock().unwrap_or_else(|e| e.into_inner());
                if registry.pins.keys().any(|p| p.starts_with(&dir)) {
                    return Ok(());
                }
                registry.collecting.insert(dir.clone());
                Reservation { state: &vault.vector_generations, paths: vec![dir.clone()] }
            };
            fs::create_dir_all(&dir)?;
            write_identity(
                &dir,
                &LineageIdentity {
                    provider_id: provider,
                    model,
                    job_id: lineage.strip_prefix('j').and_then(|n| n.parse().ok()),
                    source_watermark: 0,
                    upper_history_id: 0,
                    sources_valid: false,
                },
            )?;
            drop(reservation);
            Ok(())
        })();
        if let Err(e) = result {
            failures.push(format!("discovering lineage: {e:#}"));
        }
    }
    failures
}

#[cfg(test)]
mod replaced_tests {
    //! Failure modes: a failed rebuild's files stay for good after its index was replaced or
    //! reset; a reset of another model, or a rebuild of the old lineage, throws away a failed
    //! rebuild Replay could carry on.
    use super::replaced_since;

    const FAILED: Option<&str> = Some("2026-11-01T10:00:00.5+00:00");

    #[test]
    fn a_failed_rebuild_is_replaced_by_a_newer_lineage_or_a_later_reset_of_its_model() {
        assert!(replaced_since(11, FAILED, Some("t/j12/g3"), None));
        assert!(!replaced_since(11, FAILED, Some("t/j9/g4"), None), "the old lineage updated");
        assert!(
            !replaced_since(11, FAILED, None, None),
            "another model's reset is not on this row"
        );
        // A reset replayed from job 10 keeps its id; its time is what counts.
        assert!(replaced_since(11, FAILED, None, Some("2026-11-01T10:00:01+00:00")));
        assert!(!replaced_since(11, FAILED, None, Some("2026-11-01T09:59:59.999+00:00")));
    }
}
