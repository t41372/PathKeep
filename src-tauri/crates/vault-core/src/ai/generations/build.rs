//! Responsibilities: own resumable lineage sources and prepare a complete durable generation.
//! Not responsible for: committing the pointer or choosing candidate visits.
use super::*;
use crate::ai::{AiProviderRuntime, ReverseVisitMap, VectorStore, VisitContentMap, search_vectors};

/// Pins source lineage and unpublished output through the final success transaction.
pub(in crate::ai) struct BuildGeneration {
    pub(super) source: Vault,
    pub(super) output: ProjectPaths,
    pub(super) lease: GenerationLease,
    pub(super) identity: LineageIdentity,
    pub fresh: bool,
}
impl BuildGeneration {
    pub(in crate::ai) fn open(
        vault: &Vault,
        provider: &AiProviderRuntime,
        job_id: Option<i64>,
        run_id: i64,
        full_rebuild: bool,
        resume: bool,
    ) -> Result<Self> {
        super::gc::repair_pointers(vault)?;
        let table = provider_table_name(&provider.config.id, &provider.config.default_model);
        // GC holds a lineage only while it decides and renames. Wait for it, then choose again,
        // since the lineage may be gone afterwards; failing would end a queued job for good.
        let (initial_pointer, _serving_lease, serving, lineage, source_dir, lease) = loop {
            let initial_pointer =
                read_pointer(vault, &provider.config.id, &provider.config.default_model)?;
            let serving_lease =
                resolve(vault, &provider.config.id, &provider.config.default_model)?;
            let serving = serving_lease.as_ref().map(|lease| lease.relative().to_owned());
            // Moved v0.4.0 sources are reused only while the ledger still says they need a pass
            // or lost their generation. After a Clear (no note) they are leftovers GC deletes:
            // reusing them would bring the cleared vectors back.
            let (pair, model) = (provider.config.id.clone(), provider.config.default_model.clone());
            let recovering = serving.is_none()
                && vault.read(Db::Intelligence, move |s| {
                    Ok(recovery_note(s, &pair, &model)?.is_some())
                })?;
            let lineage = if !full_rebuild && let Some(serving) = &serving {
                serving.split('/').nth(1).unwrap().to_owned()
            } else if !full_rebuild
                && recovering
                && identity(&vault.paths().vectors_dir.join(&table).join("legacy"))
                    .is_ok_and(|id| id.sources_valid)
            {
                "legacy".to_owned()
            } else {
                job_id.map_or_else(|| format!("r{run_id}"), |id| format!("j{id}"))
            };
            let source_dir = vault.paths().vectors_dir.join(&table).join(&lineage);
            // GC reserving the lineage, or partial recovery writing to it (minutes of projection
            // at 14.4M visits): both end, so wait for them rather than ending the job.
            let touched = |registry: &mut Registry| {
                registry
                    .collecting
                    .iter()
                    .any(|path| source_dir.starts_with(path) || path.starts_with(&source_dir))
                    || registry.writers.contains(&source_dir)
            };
            let mut registry =
                vault.vector_generations.registry.lock().unwrap_or_else(|e| e.into_inner());
            if touched(&mut registry) {
                #[cfg(test)]
                vault.vector_generations.test_hook("build_waits_for_gc");
                let registry = vault
                    .vector_generations
                    .released
                    .wait_while(registry, touched)
                    .unwrap_or_else(|e| e.into_inner());
                // The serving lease takes the registry when dropped.
                drop(registry);
                continue;
            }
            let relative = format!("{table}/{lineage}/g{run_id}");
            let mut lease = pin(vault, relative, lineage.clone(), &mut registry);
            registry.writers.insert(source_dir.clone());
            lease.writer = true;
            drop(registry);
            break (initial_pointer, serving_lease, serving, lineage, source_dir, lease);
        };
        ensure!(
            read_pointer(vault, &provider.config.id, &provider.config.default_model)?
                == initial_pointer,
            "serving index changed while pinning the build; retry the job"
        );
        let existing = identity(&source_dir).ok().filter(|i| {
            i.sources_valid
                && i.provider_id == provider.config.id
                && i.model == provider.config.default_model
        });
        let pair = provider.clone();
        let row_lineage = lineage.clone();
        let rows = vault.read(Db::Intelligence, move |s| {
            super::super::native_ledger::lineage_embedding_count(
                s,
                &pair.config.id,
                &pair.config.default_model,
                &row_lineage,
            )
        })?;
        let mut source_paths = vault.paths().clone();
        source_paths.vectors_dir = source_dir.clone();
        let complete_source = rows == 0
            || (VectorStore::for_provider(
                &source_paths,
                &provider.config.id,
                &provider.config.default_model,
            )
            .exists()
                && VisitContentMap::for_provider(
                    &source_paths,
                    &provider.config.id,
                    &provider.config.default_model,
                )
                .path()
                .exists());
        let serving_here = serving.as_ref().is_some_and(|pointer| {
            vault.paths().vectors_dir.join(pointer).parent() == Some(source_dir.as_path())
        });
        // A full rebuild continues its own sources when the index they serve is its own partial
        // (a crash inside its first chunk, published by startup recovery): starting over would
        // delete the generation search reads.
        let reuse =
            existing.is_some() && complete_source && (!full_rebuild || resume || serving_here);
        // A damaged source cannot authorize deleting any immutable serving or leased files.
        ensure!(
            !serving_here || reuse,
            "the serving lineage sources are missing or invalid; run a full rebuild"
        );
        {
            let registry =
                vault.vector_generations.registry.lock().unwrap_or_else(|e| e.into_inner());
            ensure!(
                reuse
                    || !registry.pins.keys().any(|p| p != &lease.paths.vectors_dir
                        && p.parent() == Some(source_dir.as_path())),
                "the lineage still has a reader or writer"
            );
        }
        if !reuse {
            // The writer pin protects this name while bounded provenance cleanup yields the DB
            // writer between batches. No reader mutex is held during the potentially large delete.
            crate::ai::indexing::store_rows::remove_lineage_rows(
                vault,
                &provider.config.id,
                &provider.config.default_model,
                lease.lineage(),
            )?;
            if source_dir.exists() {
                std::fs::remove_dir_all(&source_dir)?;
            }
        }
        std::fs::create_dir_all(&source_dir)?;
        let identity = if reuse {
            existing.unwrap()
        } else {
            let source_watermark = crate::ai::native_ledger::current_source_watermark(vault)?;
            let upper_history_id = vault.read(Db::Archive, |s| {
                Ok(s.query_one(
                    "SELECT COALESCE(MAX(id),0) FROM visits WHERE reverted_at IS NULL",
                    (),
                    |r| Ok(r.get::<i64>(0)?),
                )?)
            })?;
            let identity = LineageIdentity {
                provider_id: provider.config.id.clone(),
                model: provider.config.default_model.clone(),
                job_id,
                source_watermark,
                upper_history_id,
                sources_valid: true,
            };
            write_identity(&source_dir, &identity)?;
            identity
        };
        std::fs::create_dir(&lease.paths.vectors_dir)?;
        let source = vault.with_vectors_dir(source_dir);
        let build = Self { output: lease.paths.clone(), source, lease, identity, fresh: !reuse };
        if !super::partial::has_complete_serving(vault)? {
            super::partial::write_intent(&build, run_id, initial_pointer)?;
        }
        Ok(build)
    }
    pub(in crate::ai) fn source(&self) -> &Vault {
        &self.source
    }
    #[cfg(test)]
    pub(in crate::ai) fn output(&self) -> &ProjectPaths {
        &self.output
    }
    pub(in crate::ai) fn lineage(&self) -> &str {
        self.lease.lineage()
    }
    pub(in crate::ai) fn relative(&self) -> &str {
        self.lease.relative()
    }
    pub(in crate::ai) fn watermark(&self, current: i64, full_rebuild: bool) -> i64 {
        if full_rebuild { self.identity.source_watermark } else { current }
    }
    pub(in crate::ai) fn upper_history_id(&self, full_rebuild: bool) -> Option<i64> {
        full_rebuild.then_some(self.identity.upper_history_id)
    }
    pub(in crate::ai) fn prepare(&self, provider: &AiProviderRuntime) -> Result<()> {
        let source = self.source.paths();
        let output = &self.output;
        let id = &provider.config.id;
        let model = &provider.config.default_model;
        let cancel = &self.lease.vault.vector_generations.projection_cancel;
        cancel.check()?;
        // The planes hold each content key once (`read_records_streaming`: the last record of a
        // key wins), while the source store may hold a key twice after a torn resume. Readers
        // check the plane's length against this count, so it is the planes' count.
        let planes = super::super::vector_planes::build_planes_into_cancellable(
            source, output, id, model, cancel,
        )?;
        let store = VectorStore::for_provider(source, id, model);
        let page_header = store.read_header()?;
        ReverseVisitMap::for_provider(output, id, model).build_from_visit_map_cancellable(
            output,
            &VisitContentMap::for_provider(source, id, model),
            page_header.as_ref().map_or("", |h| h.fingerprint_hash.as_str()),
            cancel,
        )?;
        let searches =
            VectorStore::for_provider(source, &search_vectors::identity(provider), model);
        if searches.exists() {
            super::super::vector_planes::build_planes_into_cancellable(
                source,
                output,
                &search_vectors::identity(provider),
                model,
                cancel,
            )?;
        }
        let metadata = ServingMetadata {
            version: 1,
            provider_id: id.clone(),
            model: model.clone(),
            lineage: self.lineage().into(),
            generation: format!("g{}", self.relative().split("/g").last().unwrap()),
            pages: page_header,
            searches: searches.read_header()?,
            vector_count: planes.vectors as u64,
        };
        cancel.check()?;
        crate::durable_io::atomic_durable_write(
            &output.vectors_dir.join("serving.json"),
            &serde_json::to_vec(&metadata)?,
        )?;
        self.sync_prepared()?;
        // What readers will check, checked now: a generation they would reject fails the pass
        // here instead of being published and then taken down by pointer repair.
        validate_serving(&self.lease)
    }
    pub(super) fn sync_prepared(&self) -> Result<()> {
        let source = self.source.paths();
        let output = &self.output;
        #[cfg(test)]
        self.lease.vault.vector_generations.test_hook("before_prepared_sync");
        self.lease.vault.vector_generations.projection_cancel.check()?;
        sync_files(&source.vectors_dir)?;
        sync_files(&output.vectors_dir)?;
        crate::durable_io::fsync_dir(&source.vectors_dir)?;
        crate::durable_io::fsync_dir(source.vectors_dir.parent().unwrap())?;
        crate::durable_io::fsync_dir(&self.lease.vault.paths().vectors_dir)?;
        crate::durable_io::fsync_dir(self.lease.vault.paths().vectors_dir.parent().unwrap())?;
        Ok(())
    }
}
