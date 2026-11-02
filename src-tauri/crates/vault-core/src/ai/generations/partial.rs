//! Responsibilities: retain stopped initial fills and recover their local projections after open.
//! Not responsible for: provider calls, queue success or declaring full archive coverage.
use super::*;
use crate::ai::{AiProviderRuntime, VectorStore, VisitContentMap, search_vectors};
use std::fs;
use vault_store::Priority;

#[derive(Serialize, Deserialize)]
struct Intent {
    run_id: i64,
    relative: String,
    previous_pointer: Option<String>,
}
fn marker(dir: &Path) -> PathBuf {
    dir.join("partial.json")
}
pub(super) fn pending(dir: &Path) -> bool {
    marker(dir).exists()
}

pub(super) fn has_complete_serving(vault: &Vault) -> Result<bool> {
    let configured =
        vault.config().ai.embedding_providers.iter().map(|p| p.id.clone()).collect::<HashSet<_>>();
    let pairs = vault.read(Db::Intelligence, |s| {
        Ok(s.query("SELECT provider_id,model FROM ai_index_ledger WHERE state='ready'", (), |r| {
            Ok((r.get::<String>(0)?, r.get::<String>(1)?))
        })?)
    })?;
    for (provider, model) in pairs {
        if configured.contains(&provider) && resolve(vault, &provider, &model)?.is_some() {
            return Ok(true);
        }
    }
    Ok(false)
}
pub(super) fn write_intent(
    build: &BuildGeneration,
    run_id: i64,
    previous_pointer: Option<String>,
) -> Result<()> {
    crate::durable_io::atomic_durable_write(
        &marker(&build.source.paths().vectors_dir),
        &serde_json::to_vec(&Intent {
            run_id,
            relative: build.relative().into(),
            previous_pointer,
        })?,
    )
}
impl BuildGeneration {
    pub(in crate::ai) fn finish_intent(&self) -> Result<()> {
        let path = marker(&self.source.paths().vectors_dir);
        if path.exists() {
            crate::durable_io::remove_file_durably(&path)?;
        }
        Ok(())
    }
    /// A stop still leaves useful derived work. Publish only when no complete serving index wins,
    /// preserving the stopped job's state and full-pass coverage fields.
    pub(in crate::ai) fn publish_partial(&self, provider: &AiProviderRuntime) -> Result<bool> {
        let path = marker(&self.source.paths().vectors_dir);
        if !path.exists() {
            return Ok(false);
        }
        let intent: Intent = serde_json::from_slice(&fs::read(&path)?)?;
        if !authorized(&self.lease.vault, &self.identity, &intent)? {
            self.finish_intent()?;
            return Ok(false);
        }
        trim_sources(self.source.paths(), provider)?;
        if VectorStore::for_provider(
            self.source.paths(),
            &provider.config.id,
            &provider.config.default_model,
        )
        .count()?
            == 0
        {
            self.finish_intent()?;
            return Ok(false);
        }
        if validate_serving(&self.lease).is_err() {
            #[cfg(test)]
            self.lease.vault.vector_generations.test_hook("partial_before_prepare");
            self.prepare(provider)?;
        } else {
            self.sync_prepared()?;
        }
        self.lease.vault.vector_generations.projection_cancel.check()?;
        // A rebuild or reset over a partial index that stopped after a chunk must not take the
        // place of the partial that covers more; that one keeps serving and the new lineage waits
        // for its job to go on (GC keeps a paused or failed job's sources).
        if let Some(previous) = intent.previous_pointer.as_deref().filter(|p| *p != intent.relative)
            && serving_count(&self.lease.vault, previous) > self.lease.metadata()?.vector_count
        {
            self.finish_intent()?;
            return Ok(false);
        }
        let (identity, config) = (self.identity.clone(), self.lease.vault.config().clone());
        let published=self.lease.vault.write(Db::Intelligence,Priority::Batch,move |s| {
            if !authorized_from(s,&config,&identity,&intent)? {return Ok(false)}
            s.execute("UPDATE ai_index_ledger SET sidecar_table=?3,state='incomplete',source_watermark=0,build_finished_at=NULL,last_indexed_at=?4,failure_reason=NULL,last_failure_at=NULL WHERE provider_id=?1 AND model=?2",params![identity.provider_id,identity.model,intent.relative,crate::utils::now_rfc3339()])?;
            Ok(true)
        })?;
        self.finish_intent()?;
        Ok(published)
    }
}
/// How many vectors the generation at `relative` serves; nothing when its metadata is unreadable.
fn serving_count(vault: &Vault, relative: &str) -> u64 {
    fs::read(vault.paths().vectors_dir.join(relative).join("serving.json"))
        .ok()
        .and_then(|bytes| serde_json::from_slice::<ServingMetadata>(&bytes).ok())
        .map_or(0, |metadata| metadata.vector_count)
}
fn authorized(vault: &Vault, id: &LineageIdentity, intent: &Intent) -> Result<bool> {
    let (config, id, intent) = (
        vault.config().clone(),
        id.clone(),
        Intent {
            run_id: intent.run_id,
            relative: intent.relative.clone(),
            previous_pointer: intent.previous_pointer.clone(),
        },
    );
    vault.read(Db::Intelligence, move |s| authorized_from(s, &config, &id, &intent))
}
fn authorized_from(
    s: &Session<'_>,
    config: &crate::AppConfig,
    id: &LineageIdentity,
    intent: &Intent,
) -> Result<bool> {
    let configured =
        config.ai.embedding_providers.iter().map(|p| p.id.as_str()).collect::<HashSet<_>>();
    let ready = s.query(
        "SELECT provider_id FROM ai_index_ledger WHERE state='ready' AND sidecar_table<>''",
        (),
        |r| Ok(r.get::<String>(0)?),
    )?;
    if ready.iter().any(|provider| configured.contains(provider.as_str())) {
        return Ok(false);
    }
    let row=s.query_one("SELECT state,last_run_id,sidecar_table FROM ai_index_ledger WHERE provider_id=?1 AND model=?2",params![id.provider_id,id.model],|r|Ok((r.get::<String>(0)?,r.get::<Option<i64>>(1)?,r.get::<String>(2)?))).optional()?;
    Ok(row.is_some_and(|(state, run, pointer)| {
        state != "ready"
            && state != "cleared"
            && state != "clearing"
            && run == Some(intent.run_id)
            && (pointer == intent.previous_pointer.as_deref().unwrap_or("")
                || pointer == intent.relative
                || pointer.is_empty())
    }))
}
fn trim_sources(paths: &ProjectPaths, provider: &AiProviderRuntime) -> Result<()> {
    for id in [provider.config.id.clone(), search_vectors::identity(provider)] {
        let store = VectorStore::for_provider(paths, &id, &provider.config.default_model);
        store.trim_torn_tail()?;
        if store.exists() {
            crate::durable_io::fsync_file_durably(store.path())?;
        }
        let map = VisitContentMap::for_provider(paths, &id, &provider.config.default_model);
        map.trim_torn_tail()?;
        if map.path().exists() {
            crate::durable_io::fsync_file_durably(map.path())?;
        }
    }
    Ok(())
}
fn intents(vault: &Vault) -> Result<Vec<PathBuf>> {
    let root = &vault.paths().vectors_dir;
    let mut found = Vec::new();
    if !root.exists() {
        return Ok(found);
    }
    for table in fs::read_dir(root)? {
        let table = table?;
        if !table.file_type()?.is_dir()
            || matches!(table.file_name().to_str(), Some("rebuild" | "trash"))
        {
            continue;
        }
        for lineage in fs::read_dir(table.path())? {
            let lineage = lineage?;
            if lineage.file_type()?.is_dir() && pending(&lineage.path()) {
                found.push(lineage.path());
            }
        }
    }
    Ok(found)
}
#[cfg(test)]
pub(in crate::ai) fn recover_partial_indexes(vault: &Vault) -> Result<()> {
    let Some(_collection) = vault.vector_generations.admit() else { return Ok(()) };
    recover_pending(vault)
}
/// Called under collector admission; writer pins exclude active appends, immutable readers remain.
pub(super) fn recover_pending(vault: &Vault) -> Result<()> {
    let mut failures = Vec::new();
    for dir in intents(vault)? {
        if vault.vector_generations.is_closed() {
            break;
        }
        let recovered = (|| -> Result<()> {
            let id = identity(&dir)?;
            let intent: Intent = serde_json::from_slice(&fs::read(marker(&dir))?)?;
            ensure!(
                valid_pointer(&intent.relative, &id.provider_id, &id.model)
                    && vault.paths().vectors_dir.join(&intent.relative).parent()
                        == Some(dir.as_path()),
                "invalid partial recovery identity"
            );
            let lease = {
                let mut registry =
                    vault.vector_generations.registry.lock().unwrap_or_else(|e| e.into_inner());
                if registry.writers.contains(&dir)
                    || registry.collecting.iter().any(|p| p.starts_with(&dir) || dir.starts_with(p))
                {
                    return Ok(());
                }
                let lineage = dir
                    .file_name()
                    .unwrap()
                    .to_str()
                    .ok_or_else(|| anyhow::anyhow!("invalid partial lineage"))?;
                let mut lease = pin(vault, intent.relative.clone(), lineage.into(), &mut registry);
                registry.writers.insert(dir.clone());
                lease.writer = true;
                lease
            };
            fs::create_dir_all(&lease.paths.vectors_dir)?;
            let build = BuildGeneration {
                source: vault.with_vectors_dir(dir.clone()),
                output: lease.paths.clone(),
                lease,
                identity: id.clone(),
                fresh: false,
            };
            let provider = AiProviderRuntime {
                config: crate::AiProviderConfig {
                    id: id.provider_id,
                    default_model: id.model,
                    ..Default::default()
                },
                api_key: None,
            };
            build.publish_partial(&provider)?;
            Ok(())
        })();
        if let Err(error) = recovered {
            failures.push(format!("{}: {error:#}", dir.display()));
        }
    }
    ensure!(failures.is_empty(), "partial index recovery: {}", failures.join("; "));
    Ok(())
}
/// Startup schedules only local derived recovery. Collector admission/closed cancellation makes
/// close wait for bounded work and prevents any writes after database ownership is released.
pub(crate) fn schedule_recovery(vault: &Vault) {
    let pending = match intents(vault) {
        Ok(pending) => !pending.is_empty(),
        Err(error) => {
            eprintln!("pathkeep: discovering partial indexes: {error:#}");
            false
        }
    };
    if !pending {
        return;
    }
    let vault = vault.clone();
    if let Err(error) =
        std::thread::Builder::new().name("semantic-index-recovery".into()).spawn(move || {
            if let Err(error) = super::gc::collect(&vault, true) {
                eprintln!("pathkeep: partial index recovery will retry: {error:#}");
            }
        })
    {
        eprintln!("pathkeep: scheduling partial index recovery: {error:#}");
    }
}
