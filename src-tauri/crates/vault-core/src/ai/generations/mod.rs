//! Responsibilities: resolve and pin one immutable serving generation for an entire operation.
//! Not responsible for: ranking, embedding or queue policy.
//! Registry protects only pins, reservations and renames. No database or slow file work under it.
//! Database closures receive plain identities and never own leases.
//! Writers pin before preparing files; the pointer transaction needs no registry lock.
mod build;
mod gc;
mod legacy;
mod partial;
#[cfg(test)]
pub(in crate::ai) use partial::recover_partial_indexes;
pub(crate) use partial::schedule_recovery;

pub(super) use build::BuildGeneration;
pub use gc::collect as collect_index_generations;
pub(super) use gc::collect;
#[cfg(test)]
pub(super) use legacy::interrupt_after_move;
pub(crate) use legacy::migrate_flat_indexes;

use super::vector_store::VectorStoreHeader;
use crate::{ProjectPaths, Vault, ai_sidecar::provider_table_name};
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};
use vault_store::{Db, OptionalRow, Session, params_owned as params};

// These reserved diagnostic reasons carry recovery ownership across failed build/clear attempts.
pub(in crate::ai) const MISSING_GENERATION: &str = "semantic-generation-unavailable";
pub(in crate::ai) const LEGACY_NEEDS_PASS: &str = "semantic-generation-needs-pass";
pub(in crate::ai) const LEGACY_WAS_READY: &str = "semantic-generation-legacy-ready";
fn is_recovery_note(note: &str) -> bool {
    matches!(note, MISSING_GENERATION | LEGACY_NEEDS_PASS | LEGACY_WAS_READY)
}
pub(in crate::ai) fn recovery_note(
    s: &Session<'_>,
    provider: &str,
    model: &str,
) -> Result<Option<String>> {
    Ok(s.query_one(
        "SELECT failure_reason FROM ai_index_ledger WHERE provider_id=?1 AND model=?2",
        params![provider, model],
        |r| Ok(r.get::<Option<String>>(0)?),
    )
    .optional()?
    .flatten()
    .filter(|note| is_recovery_note(note)))
}

#[cfg(test)]
type TestHook = Box<dyn FnOnce() + Send>;

#[derive(Default)]
pub(crate) struct GenerationState {
    #[cfg(test)]
    hooks: Mutex<HashMap<&'static str, TestHook>>,
    registry: Mutex<Registry>,
    /// Signalled whenever GC releases a reservation; a build waits on it instead of failing.
    released: std::sync::Condvar,
    collector: std::sync::atomic::AtomicBool,
    idle: (Mutex<()>, std::sync::Condvar),
    closed: std::sync::atomic::AtomicBool,
    projection_cancel: vault_store::CancelToken,
}
#[derive(Default)]
struct Registry {
    pins: HashMap<PathBuf, usize>,
    retired: HashSet<PathBuf>,
    reaping: bool,
    dirty: bool,
    collecting: HashSet<PathBuf>,
    writers: HashSet<PathBuf>,
}

/// Immutable source identities captured after projection, never read from mutable build files.
#[derive(Default, Serialize, Deserialize)]
pub(super) struct ServingMetadata {
    pub version: u32,
    pub provider_id: String,
    pub model: String,
    pub lineage: String,
    pub generation: String,
    pub pages: Option<VectorStoreHeader>,
    pub searches: Option<VectorStoreHeader>,
    pub vector_count: u64,
}

/// The pair identity survives file cleanup until the last lineage row has been deleted.
#[derive(Clone, Serialize, Deserialize)]
struct LineageIdentity {
    provider_id: String,
    model: String,
    job_id: Option<i64>,
    source_watermark: i64,
    upper_history_id: i64,
    #[serde(default)]
    sources_valid: bool,
}

/// Retains every serving file until the operation has closed all its file/mmap handles.
/// Declare this before those handles, so Rust drops it after them.
pub(crate) struct GenerationLease {
    vault: Vault,
    paths: ProjectPaths,
    relative: String,
    lineage: String,
    writer: bool,
}
impl GenerationLease {
    pub(super) fn paths(&self) -> &ProjectPaths {
        &self.paths
    }
    pub(super) fn lineage(&self) -> &str {
        &self.lineage
    }
    pub(super) fn relative(&self) -> &str {
        &self.relative
    }
    pub(super) fn metadata(&self) -> Result<ServingMetadata> {
        let value: ServingMetadata =
            serde_json::from_slice(&std::fs::read(self.paths.vectors_dir.join("serving.json"))?)?;
        ensure!(
            value.version == 1
                && value.lineage == self.lineage
                && self.relative
                    == format!(
                        "{}/{}/{}",
                        provider_table_name(&value.provider_id, &value.model),
                        value.lineage,
                        value.generation
                    ),
            "invalid serving metadata"
        );
        Ok(value)
    }
}
impl Drop for GenerationLease {
    fn drop(&mut self) {
        let state = &self.vault.vector_generations;
        if state.closed.load(std::sync::atomic::Ordering::Acquire) {
            return;
        }
        let mut registry = state.registry.lock().unwrap_or_else(|e| e.into_inner());
        let path = &self.paths.vectors_dir;
        if let Some(count) = registry.pins.get_mut(path) {
            *count -= 1;
            if *count == 0 {
                registry.pins.remove(path);
            }
        }
        if self.writer {
            registry.writers.remove(path.parent().unwrap());
            // A build waiting for this lineage's writer wakes up here (`BuildGeneration::open`).
            state.released.notify_all();
        }
        if !registry.retired.contains(path) {
            return;
        }
        drop(registry);
        // Requests never do row scans or file deletion in Drop.
        collect_in_background(&self.vault);
    }
}

/// Runs garbage collection on one background thread, coalescing every request made while it
/// runs into one more pass. For work that ends without an index pass of its own (a dropped
/// lease on a retired generation, a cancelled paused or queued job), so nothing waits for it.
pub fn collect_in_background(vault: &Vault) {
    let state = &vault.vector_generations;
    if state.closed.load(std::sync::atomic::Ordering::Acquire) {
        return;
    }
    let mut registry = state.registry.lock().unwrap_or_else(|e| e.into_inner());
    registry.dirty = true;
    if registry.reaping {
        return;
    }
    registry.reaping = true;
    drop(registry);
    let vault = vault.clone();
    {
        let spawned =
            std::thread::Builder::new().name("semantic-index-gc".into()).spawn(move || {
                loop {
                    {
                        let mut registry = vault
                            .vector_generations
                            .registry
                            .lock()
                            .unwrap_or_else(|e| e.into_inner());
                        registry.dirty = false;
                    }
                    if let Err(error) = collect(&vault, true) {
                        eprintln!("pathkeep: semantic index GC: {error:#}");
                    }
                    let mut registry =
                        vault.vector_generations.registry.lock().unwrap_or_else(|e| e.into_inner());
                    if !registry.dirty
                        || vault
                            .vector_generations
                            .closed
                            .load(std::sync::atomic::Ordering::Acquire)
                    {
                        registry.reaping = false;
                        break;
                    }
                }
            });
        if let Err(error) = spawned {
            state.registry.lock().unwrap_or_else(|e| e.into_inner()).reaping = false;
            eprintln!("pathkeep: starting semantic index GC: {error}");
        }
    }
}

fn pin(
    vault: &Vault,
    relative: String,
    lineage: String,
    registry: &mut Registry,
) -> GenerationLease {
    let mut paths = vault.paths().clone();
    paths.vectors_dir = paths.vectors_dir.join(&relative);
    *registry.pins.entry(paths.vectors_dir.clone()).or_default() += 1;
    GenerationLease { vault: vault.clone(), paths, relative, lineage, writer: false }
}

/// Read once before opening any plane; the registry closes the pointer-read/GC race.
pub(super) fn resolve(
    vault: &Vault,
    provider: &str,
    model: &str,
) -> Result<Option<Arc<GenerationLease>>> {
    if let Some(lease) = &vault.generation_lease {
        let Ok(metadata) = lease.metadata() else { return Ok(None) };
        ensure!(
            metadata.provider_id == provider && metadata.model == model,
            "generation belongs to another index"
        );
        return Ok(Some(lease.clone()));
    }
    // A pin closes the read/GC race. Re-read after pinning, with no mutex held: publication
    // may have changed the pointer while we waited, and a collector may have reserved its name.
    for _ in 0..4 {
        let Some(relative) = read_pointer(vault, provider, model)? else { return Ok(None) };
        let lineage = relative.split('/').nth(1).expect("validated pointer").to_owned();
        let path = vault.paths().vectors_dir.join(&relative);
        let lease = {
            let mut registry =
                vault.vector_generations.registry.lock().unwrap_or_else(|e| e.into_inner());
            // GC never reserves a generation a pointer named when it looked, so either this
            // pointer has just moved or it was published after that look. Wait, then read again:
            // a reservation is not a missing index, and a build that took it for one would start
            // a new lineage. Never called inside a database closure, so GC can always finish.
            let reserved = |registry: &mut Registry| {
                registry.collecting.iter().any(|reserved| path.starts_with(reserved))
            };
            if reserved(&mut registry) {
                #[cfg(test)]
                vault.vector_generations.test_hook("resolve_waits_for_gc");
                drop(
                    vault
                        .vector_generations
                        .released
                        .wait_while(registry, reserved)
                        .unwrap_or_else(|e| e.into_inner()),
                );
                continue;
            }
            Arc::new(pin(vault, relative.clone(), lineage, &mut registry))
        };
        if read_pointer(vault, provider, model)? != Some(relative) {
            continue;
        }
        if let Err(error) = validate_serving(&lease) {
            eprintln!("pathkeep: semantic index unavailable: {error:#}");
            return Ok(None);
        }
        return Ok(Some(lease));
    }
    Ok(None)
}

fn read_pointer(vault: &Vault, provider: &str, model: &str) -> Result<Option<String>> {
    let (provider, model) = (provider.to_owned(), model.to_owned());
    vault.read(Db::Intelligence, move |s| pointer(s, &provider, &model))
}

/// Whether a failed serving check proves the generation unusable (files gone, torn, or not the
/// ones the metadata describes), as opposed to a read that failed this time and may pass the
/// next (a sharing violation while something scans a new file, too many open files, a disk
/// error). Pointer repair clears a pointer only on proof: clearing it on a passing fault makes
/// the next build start a new lineage and embed every visit again.
fn serving_defect(error: &anyhow::Error) -> bool {
    !error.chain().any(|cause| {
        cause.downcast_ref::<std::io::Error>().is_some_and(|io| {
            !matches!(
                io.kind(),
                std::io::ErrorKind::NotFound
                    | std::io::ErrorKind::UnexpectedEof
                    | std::io::ErrorKind::InvalidData
            )
        })
    })
}

// Only cheap immutable file checks; never scans sources or creates a projection.
fn validate_serving(lease: &GenerationLease) -> Result<()> {
    let metadata = lease.metadata()?;
    for (provider, header) in [
        (metadata.provider_id.clone(), metadata.pages.as_ref()),
        (super::search_vectors::identity_of(&metadata.provider_id), metadata.searches.as_ref()),
    ] {
        if let Some(header) = header {
            let plane = super::vector_planes::Int8Plane::for_provider(
                lease.paths(),
                &provider,
                &metadata.model,
            );
            let actual = plane.map()?.ok_or_else(|| anyhow::anyhow!("missing serving plane"))?;
            ensure!(
                actual.header.fingerprint_hash == header.fingerprint_hash
                    && actual.header.dim == header.dim,
                "serving plane identity mismatch"
            );
            if provider == metadata.provider_id {
                ensure!(
                    actual.len() as u64 == metadata.vector_count,
                    "serving vector count mismatch"
                );
            }
        }
    }
    if metadata.vector_count > 0 {
        let header = metadata
            .pages
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("missing serving source identity"))?;
        ensure!(
            super::ReverseVisitMap::for_provider(
                lease.paths(),
                &metadata.provider_id,
                &metadata.model
            )
            .projection_header(&header.fingerprint_hash)?
            .is_some(),
            "missing or invalid serving visit projections"
        );
    }
    Ok(())
}

pub(super) fn pointer(s: &Session<'_>, provider: &str, model: &str) -> Result<Option<String>> {
    let value = s
        .query_one(
            "SELECT sidecar_table FROM ai_index_ledger WHERE provider_id=?1 AND model=?2",
            params![provider, model],
            |r| Ok(r.get::<String>(0)?),
        )
        .optional()?;
    Ok(value.filter(|value| valid_pointer(value, provider, model)))
}
fn valid_pointer(value: &str, provider: &str, model: &str) -> bool {
    let parts = value.split('/').collect::<Vec<_>>();
    parts.len() == 3
        && parts[0] == provider_table_name(provider, model)
        && valid_name(parts[1])
        && valid_name(parts[2])
        && parts[2].starts_with('g')
}
fn valid_name(value: &str) -> bool {
    !value.is_empty() && value.bytes().all(|b| b.is_ascii_alphanumeric())
}
fn identity(dir: &Path) -> Result<LineageIdentity> {
    Ok(serde_json::from_slice(&std::fs::read(dir.join("identity.json"))?)?)
}
fn write_identity(dir: &Path, identity: &LineageIdentity) -> Result<()> {
    crate::durable_io::atomic_durable_write(
        &dir.join("identity.json"),
        &serde_json::to_vec(identity)?,
    )
}
fn sync_files(dir: &Path) -> Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        if entry.file_type()?.is_file() {
            crate::durable_io::fsync_file_durably(&entry.path())?;
        }
    }
    crate::durable_io::fsync_dir(dir)
}

/// Nested read helpers share this view's original lease, including snapshot signatures and maps.
pub(super) fn serving_vault(vault: &Vault, provider: &str, model: &str) -> Result<Option<Vault>> {
    let Some(lease) = resolve(vault, provider, model)? else { return Ok(None) };
    let mut view = vault.with_vectors_dir(lease.paths().vectors_dir.clone());
    view.generation_lease = Some(lease);
    Ok(Some(view))
}

pub(super) fn serving_metadata(paths: &ProjectPaths) -> Result<ServingMetadata> {
    Ok(serde_json::from_slice(&std::fs::read(paths.vectors_dir.join("serving.json"))?)?)
}

/// Admission token, not a held mutex: database and file work run without any lock guard.
struct Collection<'a>(&'a GenerationState);
impl Drop for Collection<'_> {
    fn drop(&mut self) {
        let _idle = self.0.idle.0.lock().unwrap_or_else(|e| e.into_inner());
        self.0.collector.store(false, std::sync::atomic::Ordering::Release);
        self.0.idle.1.notify_all();
    }
}
impl GenerationState {
    #[cfg(test)]
    pub(crate) fn set_test_hook(&self, name: &'static str, hook: impl FnOnce() + Send + 'static) {
        self.hooks.lock().unwrap().insert(name, Box::new(hook));
    }
    #[cfg(test)]
    pub(crate) fn test_hook(&self, name: &'static str) {
        let hook = self.hooks.lock().unwrap_or_else(|e| e.into_inner()).remove(name);
        if let Some(hook) = hook {
            hook();
        }
    }

    pub(crate) fn is_closed(&self) -> bool {
        self.closed.load(std::sync::atomic::Ordering::Acquire)
    }
    fn admit(&self) -> Option<Collection<'_>> {
        let mut idle = self.idle.0.lock().unwrap_or_else(|e| e.into_inner());
        while self.collector.load(std::sync::atomic::Ordering::Acquire)
            && !self.closed.load(std::sync::atomic::Ordering::Acquire)
        {
            idle = self.idle.1.wait(idle).unwrap_or_else(|e| e.into_inner());
        }
        if self.closed.load(std::sync::atomic::Ordering::Acquire) {
            return None;
        }
        self.collector.store(true, std::sync::atomic::Ordering::Release);
        Some(Collection(self))
    }
    /// Close admission before waiting; the active collector checks closed between row batches.
    pub(crate) fn stop_reaper(&self) {
        let mut idle = self.idle.0.lock().unwrap_or_else(|e| e.into_inner());
        self.closed.store(true, std::sync::atomic::Ordering::Release);
        self.projection_cancel.cancel();
        while self.collector.load(std::sync::atomic::Ordering::Acquire) {
            idle = self.idle.1.wait(idle).unwrap_or_else(|e| e.into_inner());
        }
    }
}

#[cfg(test)]
mod tests;
