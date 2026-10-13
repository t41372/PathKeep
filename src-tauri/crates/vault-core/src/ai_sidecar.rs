//! Leftover semantic-index sidecar directory and the stable provider/model file name.
//!
//! ## Responsibilities
//! - Derive the stable provider/model name every vector file (`.pkvec`, planes, maps) is named by.
//! - Report the bytes still sitting in the old semantic-index sidecar directory.
//! - Keep the old sidecar clear step a harmless no-op for derived-state cleanup.
//!
//! ## Not responsible for
//! - Storing or searching vectors (that is `ai::vector_store` and the derived planes).
//!
//! ## Dependencies
//! - Uses `ProjectPaths` for the existing sidecar directory contract.
//! - Uses `sha256_hex` to preserve stable provider/model naming.
//!
//! ## Performance notes
//! - This module is filesystem-only. It must not pull a vector database or its dependencies
//!   into the build.

use crate::{config::ProjectPaths, utils::sha256_hex};
use anyhow::Result;
use std::{
    fs,
    path::{Path, PathBuf},
};

/// Clears optional vector sidecar rows for one provider/model pair.
///
/// There is no vector sidecar in the default build, so this is a harmless no-op
/// that keeps derived-state cleanup and semantic-index clear requests bounded.
pub async fn clear_provider_embeddings(
    _paths: &ProjectPaths,
    _provider_id: &str,
    _model: &str,
) -> Result<usize> {
    Ok(0)
}

/// The bytes the semantic index takes on disk, for Settings → Storage: its vectors and visit
/// maps, and anything earlier versions left in the old sidecar folder.
pub fn sidecar_storage_bytes(paths: &ProjectPaths) -> u64 {
    // The index is in `derived/vectors`; the old sidecar folder only holds what earlier
    // versions left there.
    directory_size(&paths.vectors_dir) + directory_size(&sidecar_root(paths))
}

/// Returns the root directory reserved for optional vector sidecar tables.
///
/// The path stays stable even while the default build does not create or open a
/// vector database there.
pub fn sidecar_root(paths: &ProjectPaths) -> PathBuf {
    paths.semantic_index_dir.clone()
}

/// Derives the stable file name for one provider/model pair.
///
/// Every vector file of an index (`.pkvec`, planes, visit maps) is named by it, so embeddings of
/// different models never share a file. Changing it orphans every existing index.
pub fn provider_table_name(provider_id: &str, model: &str) -> String {
    let provider = provider_id
        .chars()
        .map(|value| if value.is_ascii_alphanumeric() { value } else { '_' })
        .collect::<String>();
    let digest = sha256_hex(format!("{provider_id}::{model}").as_bytes());
    format!("pathkeep_{provider}_{}", &digest[..12])
}

fn directory_size(path: &Path) -> u64 {
    let Ok(metadata) = fs::metadata(path) else {
        return 0;
    };

    if metadata.is_file() {
        return metadata.len();
    }

    let Ok(entries) = fs::read_dir(path) else {
        return 0;
    };

    entries.flatten().map(|entry| directory_size(&entry.path())).sum()
}
