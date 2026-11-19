//! Responsibilities: retain one verified immutable engine per model directory and file revision.
//! Not responsible for: downloads, provider identity or embedding execution.
use super::{ModelFile, StaticEngine};
use crate::ai::model_generations::{ModelDirectory, pin};
use anyhow::{Context, Result};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, OnceLock},
    time::SystemTime,
};

/// Each model file's length and modification time. The verified marker is not part of it: a status
/// read may rewrite the marker while an engine loads, and that changes nothing the engine reads.
#[derive(PartialEq, Eq)]
struct Revision(Vec<(&'static str, &'static str, u64, SystemTime)>);
struct CachedEngine {
    revision: Revision,
    engine: Arc<StaticEngine>,
}
static ENGINES: OnceLock<Mutex<HashMap<PathBuf, CachedEngine>>> = OnceLock::new();

fn revision(directory: &Path, files: &[ModelFile]) -> Result<Revision> {
    let stamps = files
        .iter()
        .map(|file| {
            let path = directory.join(file.name);
            let metadata =
                std::fs::metadata(&path).with_context(|| format!("stat {}", path.display()))?;
            Ok((file.name, file.sha256, metadata.len(), metadata.modified()?))
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(Revision(stamps))
}

/// Strong references retain the tokenizer and mapping between searches; clean mapped pages remain
/// reclaimable by the OS. Metadata avoids rehashing hits, and a miss hashes only files whose
/// stamps no longer match the verified marker. The loading guard also prevents simultaneous
/// queries from loading duplicate engines.
pub(super) fn load(
    root: &Path,
    files: &[ModelFile],
    load_engine: impl FnOnce(ModelDirectory) -> Result<StaticEngine>,
) -> Result<Arc<StaticEngine>> {
    let mut engines = ENGINES
        .get_or_init(Mutex::default)
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let selection = pin(root)?;
    let root = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
    let directory = selection.path().to_owned();
    if !crate::ai::model_files_present(&directory, files) {
        engines.remove(&root);
        return Err(crate::ai::EmbeddingServiceError::wrap(anyhow::anyhow!(
            "static model is not present in {}; download it (with consent) before selecting the static engine",
            directory.display()
        )));
    }
    let stamp = revision(&directory, files)?;
    if let Some(cached) = engines.get(&root) {
        if cached.engine.directory.path() == directory && cached.revision == stamp {
            return Ok(cached.engine.clone());
        }
    }
    engines.remove(&root);
    anyhow::ensure!(
        super::super::model_files::model_files_verified(&directory, files),
        "static model files in {} failed verification",
        directory.display()
    );
    let engine = Arc::new(load_engine(selection)?);
    anyhow::ensure!(
        revision(&directory, files)? == stamp,
        "static model files changed while loading"
    );
    engines.insert(root, CachedEngine { revision: stamp, engine: engine.clone() });
    Ok(engine)
}

/// Release the cache reference on publication. Borrowed engines retain their own pin.
pub(in crate::ai) fn forget_replaced_model(repo: &Path, current: &Path) {
    let repo = repo.canonicalize().unwrap_or_else(|_| repo.to_owned());
    let current = current.canonicalize().unwrap_or_else(|_| current.to_owned());
    if let Some(engines) = ENGINES.get() {
        let mut engines = engines.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        if engines.get(&repo).is_some_and(|cached| cached.engine.directory.path() != current) {
            engines.remove(&repo);
        }
    }
}

/// Release idle mappings before the wipe moves the model tree into its trash.
pub(crate) fn forget_archive(models: &Path) {
    if let Some(engines) = ENGINES.get() {
        let models = models.canonicalize().unwrap_or_else(|_| models.to_owned());
        engines
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .retain(|repo, _| !repo.starts_with(&models));
    }
}
