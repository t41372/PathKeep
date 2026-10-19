//! Responsibilities: retain one verified immutable engine per model directory and file revision.
//! Not responsible for: downloads, provider identity or embedding execution.
use super::{ModelFile, StaticEngine};
use anyhow::{Context, Result};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, OnceLock},
    time::SystemTime,
};

#[derive(PartialEq, Eq)]
struct Revision(Vec<(u64, SystemTime)>);
struct CachedEngine {
    revision: Revision,
    engine: Arc<StaticEngine>,
}
static ENGINES: OnceLock<Mutex<HashMap<PathBuf, CachedEngine>>> = OnceLock::new();

fn revision(directory: &Path, files: &[ModelFile]) -> Result<Revision> {
    let paths = std::iter::once(directory.to_path_buf())
        .chain(std::iter::once(directory.join(super::super::model_files::VERIFIED_MARKER_FILE)))
        .chain(files.iter().map(|file| directory.join(file.name)));
    let stamps = paths
        .map(|path| {
            let metadata =
                std::fs::metadata(&path).with_context(|| format!("stat {}", path.display()))?;
            Ok((metadata.len(), metadata.modified()?))
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(Revision(stamps))
}

/// Strong references retain the tokenizer and mapping between searches; clean mapped pages remain
/// reclaimable by the OS. Metadata avoids rehashing hits.
/// The loading guard also prevents simultaneous queries from loading duplicate engines.
pub(super) fn load(
    directory: &Path,
    files: &[ModelFile],
    load_engine: impl FnOnce() -> Result<StaticEngine>,
) -> Result<Arc<StaticEngine>> {
    let directory = directory.canonicalize().unwrap_or_else(|_| directory.to_path_buf());
    let mut engines = ENGINES
        .get_or_init(Mutex::default)
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let stamp = match revision(&directory, files) {
        Ok(revision) => revision,
        Err(error) => {
            engines.remove(&directory);
            return Err(error);
        }
    };
    if let Some(cached) = engines.get(&directory) {
        if cached.revision == stamp {
            return Ok(cached.engine.clone());
        }
    }
    engines.remove(&directory);
    anyhow::ensure!(
        super::super::model_files::model_is_present_and_verified(&directory, files),
        "static model files in {} failed verification",
        directory.display()
    );
    let engine = Arc::new(load_engine()?);
    anyhow::ensure!(
        revision(&directory, files)? == stamp,
        "static model files changed while loading"
    );
    engines.insert(directory, CachedEngine { revision: stamp, engine: engine.clone() });
    Ok(engine)
}

/// A removed model must not stay resident merely because its availability check failed early.
pub(super) fn forget(directory: &Path) {
    let directory = directory.canonicalize().unwrap_or_else(|_| directory.to_path_buf());
    if let Some(engines) = ENGINES.get() {
        engines.lock().unwrap_or_else(std::sync::PoisonError::into_inner).remove(&directory);
    }
}
