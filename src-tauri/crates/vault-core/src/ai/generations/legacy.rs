//! Responsibilities: finish committed old publications and move flat files into legacy lineage.
//! Not responsible for: building projections, scanning visits or making archive open depend on AI.
use super::*;
use crate::ai::{ReverseVisitMap, VectorStore, VisitContentMap, search_vectors};
use std::fs;

/// Startup owns the archive exclusively. Every rename is same-volume and destination-first on
/// retry; directory barriers are repeated even when a prior process already moved the file.
pub(crate) fn migrate_flat_indexes(vault: &Vault) -> Result<()> {
    migrate(vault, || Ok(()))
}
#[cfg(test)]
pub(in crate::ai) fn interrupt_after_move(vault: &Vault) -> Result<()> {
    migrate(vault, || anyhow::bail!("simulated crash after a durable rename"))
}
fn migrate(vault: &Vault, mut after_move: impl FnMut() -> Result<()>) -> Result<()> {
    let root = &vault.paths().vectors_dir;
    if !root.exists() {
        return Ok(());
    }
    finish_old_publications(vault)?;
    let pairs = vault.read(Db::Intelligence, |s| Ok(s.query("SELECT provider_id,model,sidecar_table,state,COALESCE(failure_reason,'') FROM ai_index_ledger UNION SELECT provider_id,model,'','incomplete','' FROM ai_embedding_totals WHERE NOT EXISTS(SELECT 1 FROM ai_index_ledger l WHERE l.provider_id=ai_embedding_totals.provider_id AND l.model=ai_embedding_totals.model)", (), |r| Ok((r.get::<String>(0)?,r.get::<String>(1)?,r.get::<String>(2)?,r.get::<String>(3)?,r.get::<String>(4)?)))?))?;
    let mut failures = Vec::new();
    for (provider, model, old, old_state, old_reason) in pairs {
        if valid_pointer(&old, &provider, &model) {
            continue;
        }
        let result = (|| -> Result<()> {
            let table = provider_table_name(&provider, &model);
            let dir = root.join(&table).join("legacy");
            let generation = dir.join("g0");
            let stems = [
                table.clone(),
                provider_table_name(&search_vectors::identity_of(&provider), &model),
            ];
            let files = fs::read_dir(root)?
                .collect::<std::io::Result<Vec<_>>>()?
                .into_iter()
                .filter(|entry| {
                    entry.file_type().is_ok_and(|t| t.is_file())
                        && stems.iter().any(|stem| {
                            entry
                                .file_name()
                                .to_string_lossy()
                                .strip_prefix(stem.as_str())
                                .is_some_and(|suffix| suffix.starts_with('.'))
                        })
                })
                .collect::<Vec<_>>();
            // Only flat files, or the note written before the first move, mean a move is due. A
            // legacy folder alone can be a cleared index whose row cleanup was cut short, or one
            // GC recreated from rows; publishing it would bring back an index with no vectors.
            let resuming = matches!(old_reason.as_str(), LEGACY_NEEDS_PASS | LEGACY_WAS_READY);
            if files.is_empty() && !resuming {
                return Ok(());
            }
            let was_ready = old_state == "ready" || old_reason == LEGACY_WAS_READY;
            let note = if was_ready { LEGACY_WAS_READY } else { LEGACY_NEEDS_PASS };
            let (mark_provider, mark_model) = (provider.clone(), model.clone());
            // Persist recovery ownership before touching either location, including totals-only
            // pairs. A later normal-job GC must not reclaim a halfway moved legacy source.
            vault.write(Db::Intelligence,vault_store::Priority::Batch,move |s| {
                s.execute("INSERT INTO ai_index_ledger(provider_id,model,sidecar_table,index_version,state,failure_reason) VALUES(?1,?2,'','generations-v1','incomplete',?3) ON CONFLICT(provider_id,model) DO UPDATE SET sidecar_table='',state='incomplete',failure_reason=excluded.failure_reason",params![mark_provider,mark_model,note])?;Ok(())
            })?;
            fs::create_dir_all(&generation)?;
            // Persist the entire destination path before a durable rename can remove a flat
            // source name. Syncing only the leaf would not persist table -> legacy reachability.
            for directory in
                [generation.as_path(), dir.as_path(), dir.parent().unwrap(), root.as_path()]
            {
                crate::durable_io::fsync_dir(directory)?;
            }
            // Repair append tails in both locations before a header/count/map reader can run.
            let mut source = vault.paths().clone();
            source.vectors_dir = dir.clone();
            trim_sources(vault.paths(), &source, &provider, &model)?;
            write_identity(
                &dir,
                &LineageIdentity {
                    provider_id: provider.clone(),
                    model: model.clone(),
                    job_id: None,
                    source_watermark: 0,
                    upper_history_id: 0,
                    sources_valid: true,
                },
            )?;
            for file in files {
                let input = file.path();
                let extension = input.extension().unwrap_or_default().to_string_lossy();
                if extension == "tmp" {
                    crate::durable_io::remove_file_durably(&input)?;
                    continue;
                }
                let target = if matches!(extension.as_ref(), "pkvec" | "pkmap") {
                    &dir
                } else {
                    &generation
                };
                move_once(&input, &target.join(file.file_name()))?;
                after_move()?;
            }
            // Finish barriers for entries moved before a crash, even if the flat directory is empty.
            crate::durable_io::fsync_dir(&generation)?;
            crate::durable_io::fsync_dir(&dir)?;
            crate::durable_io::fsync_dir(dir.parent().unwrap())?;
            crate::durable_io::fsync_dir(root)?;
            let store = VectorStore::for_provider(&source, &provider, &model);
            let mut output = source.clone();
            output.vectors_dir = generation.clone();
            // v0.4.0 projected planes only at the end of a pass, so a stopped pass left more
            // source vectors than the planes it served. Serve what it served: the plane's count.
            let plane =
                super::super::vector_planes::Int8Plane::for_provider(&output, &provider, &model)
                    .map()
                    .ok()
                    .flatten();
            let count = match &plane {
                Some(plane) => plane.len() as u64,
                None => store.count()?,
            };
            // An empty index is publishable only when no rows say visits were embedded; otherwise
            // its vectors are lost (a pass that recreated the sources failed) and a pass is due.
            let (pair, row_model) = (provider.clone(), model.clone());
            let rows = vault.read(Db::Intelligence, move |s| {
                super::super::native_ledger::lineage_embedding_count(s, &pair, &row_model, "legacy")
            })?;
            let projection_ready = (count == 0 && rows == 0)
                || (plane.is_some()
                    && ReverseVisitMap::for_provider(&output, &provider, &model).exists());
            let searches =
                VectorStore::for_provider(&source, &search_vectors::identity_of(&provider), &model);
            let search_ready = !searches.exists()
                || super::super::vector_planes::Int8Plane::for_provider(
                    &output,
                    &search_vectors::identity_of(&provider),
                    &model,
                )
                .exists();
            let pointer = if projection_ready && search_ready {
                let metadata = ServingMetadata {
                    version: 1,
                    provider_id: provider.clone(),
                    model: model.clone(),
                    lineage: "legacy".into(),
                    generation: "g0".into(),
                    pages: store.read_header()?,
                    searches: searches.read_header()?,
                    vector_count: count,
                };
                crate::durable_io::atomic_durable_write(
                    &generation.join("serving.json"),
                    &serde_json::to_vec(&metadata)?,
                )?;
                let relative = format!("{table}/legacy/g0");
                // Publish only what a reader accepts; otherwise the next GC would revoke it and
                // the "was ready" note would be lost. Unpublished, the next pass builds it.
                let lease = {
                    let mut registry =
                        vault.vector_generations.registry.lock().unwrap_or_else(|e| e.into_inner());
                    pin(vault, relative.clone(), "legacy".into(), &mut registry)
                };
                match validate_serving(&lease) {
                    Ok(()) => relative,
                    Err(error) => {
                        eprintln!("pathkeep: old semantic index needs a pass: {error:#}");
                        String::new()
                    }
                }
            } else {
                String::new()
            };
            let state = if !pointer.is_empty() && was_ready { "ready" } else { "incomplete" };
            let recovery = pointer.is_empty().then_some(note);
            let (provider, model) = (provider.clone(), model.clone());
            vault.write(Db::Intelligence,vault_store::Priority::Batch,move |s| {
                s.execute("UPDATE ai_index_ledger SET sidecar_table=?3,state=?4,failure_reason=?5 WHERE provider_id=?1 AND model=?2",params![provider,model,pointer,state,recovery])?;
                Ok(())
            })?;
            Ok(())
        })();
        if let Err(error) = result {
            failures.push(format!("{provider}/{model}: {error:#}"));
        }
    }
    ensure!(failures.is_empty(), "{}", failures.join("; "));
    Ok(())
}
fn trim_sources(
    flat: &ProjectPaths,
    source: &ProjectPaths,
    provider: &str,
    model: &str,
) -> Result<()> {
    for id in [provider.to_owned(), search_vectors::identity_of(provider)] {
        // A resumed destination is authoritative. Stale duplicates may even be corrupt; neither
        // read nor repair them before deciding which file owns the name.
        let moved = VectorStore::for_provider(source, &id, model);
        let store =
            if moved.exists() { moved } else { VectorStore::for_provider(flat, &id, model) };
        store.trim_torn_tail()?;
        if store.exists() {
            crate::durable_io::fsync_file_durably(store.path())?;
        }
        let moved = VisitContentMap::for_provider(source, &id, model);
        let map = if moved.path().exists() {
            moved
        } else {
            VisitContentMap::for_provider(flat, &id, model)
        };
        map.trim_torn_tail()?;
        if map.path().exists() {
            crate::durable_io::fsync_file_durably(map.path())?;
        }
    }
    Ok(())
}

fn move_once(source: &Path, target: &Path) -> Result<()> {
    if !target.exists() {
        fs::rename(source, target)?;
    }
    // Destination wins if an older migration left both names. Never overwrite the resumed file.
    crate::durable_io::fsync_dir(target.parent().unwrap())?;
    if source.exists() {
        crate::durable_io::remove_file_durably(source)?;
    }
    crate::durable_io::fsync_dir(source.parent().unwrap())
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct OldManifest {
    job_id: Option<i64>,
    provider_id: String,
    model: String,
    rows_since: String,
    #[serde(default)]
    files: Vec<String>,
    source_watermark: i64,
    #[serde(default)]
    cleanup_after: i64,
}
fn old_manifest(dir: &Path) -> Result<OldManifest> {
    Ok(serde_json::from_slice(&fs::read(dir.join("manifest.json"))?)?)
}
fn finish_old_publications(vault: &Vault) -> Result<()> {
    cleanup_old_copies(vault, false)
}

// Old copies share legacy provenance. Keep their marker at open; normal queue collection removes
// only their old timestamp range in bounded id batches before any new pass trusts those hashes.
pub(super) fn cleanup_old_copies(vault: &Vault, heal_rows: bool) -> Result<()> {
    let root = &vault.paths().vectors_dir;
    let rebuild = root.join("rebuild");
    if !rebuild.exists() {
        return Ok(());
    }
    for entry in fs::read_dir(&rebuild)? {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            crate::durable_io::remove_file_durably(&entry.path())?;
            continue;
        }
        let dir = entry.path();
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.ends_with(".publish") {
            // The old binary's commit point is this suffix plus manifest, even when it died
            // before marking the queue succeeded. The format contains no run id to correlate.
            let manifest = old_manifest(&dir)?;
            let expected = manifest.job_id.map(|job| format!("job-{job}.publish"));
            ensure!(
                expected.as_deref().is_none_or(|expected| expected == name),
                "legacy publication identity mismatch"
            );
            let stems = [
                provider_table_name(&manifest.provider_id, &manifest.model),
                provider_table_name(
                    &search_vectors::identity_of(&manifest.provider_id),
                    &manifest.model,
                ),
            ];
            let valid = |name: &str| {
                Path::new(name).file_name().and_then(|n| n.to_str()) == Some(name)
                    && stems.iter().any(|stem| {
                        name.strip_prefix(stem.as_str())
                            .is_some_and(|suffix| suffix.starts_with('.'))
                    })
            };
            ensure!(
                manifest.files.iter().all(|name| valid(name)),
                "invalid legacy publication file list"
            );
            let mut remaining = fs::read_dir(&dir)?.collect::<std::io::Result<Vec<_>>>()?;
            remaining.sort_by_key(|entry| entry.path().extension().is_some_and(|s| s == "pki8"));
            for file in remaining {
                let name = file.file_name();
                let name = name.to_string_lossy();
                if name.starts_with("manifest.json") {
                    continue;
                }
                ensure!(
                    file.file_type()?.is_file()
                        && valid(&name)
                        && manifest.files.iter().any(|f| f == &name),
                    "unexpected legacy publication file"
                );
                let target = root.join(name.as_ref());
                #[cfg(windows)]
                if target.exists() {
                    crate::durable_io::remove_file_durably(&target)?;
                }
                fs::rename(file.path(), &target)?;
                crate::durable_io::fsync_dir(root)?;
                crate::durable_io::fsync_dir(&dir)?;
            }
            for file in fs::read_dir(root)? {
                let file = file?;
                let name = file.file_name();
                let name = name.to_string_lossy();
                if file.file_type()?.is_file()
                    && valid(&name)
                    && !manifest.files.iter().any(|f| f == &name)
                {
                    crate::durable_io::remove_file_durably(&file.path())?;
                }
            }
            // Repeat barriers on an already emptied marker before removing it.
            crate::durable_io::fsync_dir(root)?;
            crate::durable_io::fsync_dir(&dir)?;
            fs::remove_dir_all(&dir)?;
        } else {
            for file in fs::read_dir(&dir)? {
                let file = file?;
                if !file.file_name().to_string_lossy().starts_with("manifest.json") {
                    if file.file_type()?.is_dir() {
                        fs::remove_dir_all(file.path())?;
                    } else {
                        crate::durable_io::remove_file_durably(&file.path())?;
                    }
                }
            }
            match old_manifest(&dir) {
                Ok(mut manifest) if heal_rows => {
                    cleanup_old_rows(vault, &dir, &mut manifest)?;
                    fs::remove_dir_all(&dir)?;
                }
                Ok(_) => {}
                Err(_) => fs::remove_dir_all(&dir)?,
            }
        }
        crate::durable_io::fsync_dir(&rebuild)?;
    }
    let _ = fs::remove_dir(&rebuild);
    crate::durable_io::fsync_dir(root)
}
fn cleanup_old_rows(vault: &Vault, dir: &Path, manifest: &mut OldManifest) -> Result<()> {
    loop {
        ensure!(
            !vault.vector_generations.is_closed(),
            "legacy row cleanup stopped because archive is closing"
        );
        let (provider, model, since, after) = (
            manifest.provider_id.clone(),
            manifest.model.clone(),
            manifest.rows_since.clone(),
            manifest.cleanup_after,
        );
        let last=vault.write(Db::Intelligence,vault_store::Priority::Batch,move |s| {
            let rows=s.query("SELECT id,indexed_at,content_bytes,metadata_bytes FROM ai_embeddings INDEXED BY idx_ai_embeddings_lineage_id WHERE provider_id=?1 AND model=?2 AND lineage='legacy' AND id>?3 ORDER BY id LIMIT 512",params![provider,model,after],|r|Ok((r.get::<i64>(0)?,r.get::<String>(1)?,r.get::<i64>(2)?,r.get::<i64>(3)?)))?;
            let last=rows.last().map(|r|r.0);
            let (mut count,mut content,mut metadata)=(0,0,0);
            for (id,at,bytes,meta) in rows {if at>=since {s.execute("DELETE FROM ai_embeddings WHERE id=?1",[id])?;count+=1;content+=bytes;metadata+=meta;}}
            s.execute("UPDATE ai_embedding_totals SET item_count=MAX(0,item_count-?3),content_bytes=MAX(0,content_bytes-?4),metadata_bytes=MAX(0,metadata_bytes-?5),last_indexed_at=CASE WHEN item_count<=?3 THEN NULL ELSE last_indexed_at END WHERE provider_id=?1 AND model=?2 AND lineage='legacy'",params![provider,model,count,content,metadata])?;
            Ok(last)
        })?;
        let Some(last) = last else { return Ok(()) };
        manifest.cleanup_after = last;
        crate::durable_io::atomic_durable_write(
            &dir.join("manifest.json"),
            &serde_json::to_vec(manifest)?,
        )?;
    }
}
