//! Content-addressed readable-text blob storage.
//!
//! Deterministic enrichment may extract large readable bodies. Keeping those
//! blobs out of SQLite keeps the intelligence plane smaller while preserving a
//! rebuildable local cache.

use crate::{
    config::{ProjectPaths, ensure_paths},
    utils::sha256_hex,
};
use anyhow::{Context, Result};
use std::{
    fs,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone)]
pub(crate) struct StoredBlob {
    pub relative_path: String,
    pub byte_len: u64,
    pub content_hash: String,
}

pub(crate) fn store_readable_text_blob(
    paths: &ProjectPaths,
    text: Option<&str>,
) -> Result<Option<StoredBlob>> {
    let Some(text) = text.filter(|value| !value.trim().is_empty()) else {
        return Ok(None);
    };
    ensure_paths(paths)?;
    let content_hash = sha256_hex(text.as_bytes());
    let relative_path = blob_relative_path(&content_hash);
    let absolute_path = paths.intelligence_blobs_dir.join(&relative_path);
    if !absolute_path.exists() {
        if let Some(parent) = absolute_path.parent() {
            fs::create_dir_all(parent).with_context(|| format!("creating {}", parent.display()))?;
        }
        let temp_path = absolute_path.with_extension("tmp");
        fs::write(&temp_path, text).with_context(|| format!("writing {}", temp_path.display()))?;
        fs::rename(&temp_path, &absolute_path)
            .with_context(|| format!("persisting {}", absolute_path.display()))?;
    }
    Ok(Some(StoredBlob {
        relative_path: relative_path.display().to_string(),
        byte_len: text.len() as u64,
        content_hash,
    }))
}

pub(crate) fn load_readable_text_blob(
    paths: &ProjectPaths,
    relative_path: Option<&str>,
) -> Result<Option<String>> {
    let Some(relative_path) = relative_path.filter(|value| !value.trim().is_empty()) else {
        return Ok(None);
    };
    let absolute_path = paths.intelligence_blobs_dir.join(relative_path);
    if !absolute_path.exists() {
        return Ok(None);
    }
    fs::read_to_string(&absolute_path)
        .with_context(|| format!("reading {}", absolute_path.display()))
        .map(Some)
}

pub(crate) fn clear_readable_text_blobs(paths: &ProjectPaths) -> Result<()> {
    if paths.intelligence_blobs_dir.exists() {
        fs::remove_dir_all(&paths.intelligence_blobs_dir)
            .with_context(|| format!("clearing {}", paths.intelligence_blobs_dir.display()))?;
    }
    fs::create_dir_all(&paths.intelligence_blobs_dir)
        .with_context(|| format!("creating {}", paths.intelligence_blobs_dir.display()))?;
    Ok(())
}

fn blob_relative_path(content_hash: &str) -> PathBuf {
    let prefix = &content_hash[..2];
    let shard = &content_hash[2..4];
    Path::new(prefix).join(shard).join(format!("{content_hash}.txt"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::project_paths_with_root;
    use tempfile::tempdir;

    #[test]
    fn readable_text_blobs_are_deduplicated_by_hash() {
        let dir = tempdir().expect("tempdir");
        let paths = project_paths_with_root(dir.path());

        let first = store_readable_text_blob(&paths, Some("hello world"))
            .expect("store first blob")
            .expect("blob");
        let second = store_readable_text_blob(&paths, Some("hello world"))
            .expect("store second blob")
            .expect("blob");

        assert_eq!(first.relative_path, second.relative_path);
        assert_eq!(
            load_readable_text_blob(&paths, Some(&first.relative_path)).expect("load blob"),
            Some("hello world".to_string())
        );
    }

    #[test]
    fn clearing_blobs_resets_the_sidecar_directory() {
        let dir = tempdir().expect("tempdir");
        let paths = project_paths_with_root(dir.path());

        let stored = store_readable_text_blob(&paths, Some("hello world"))
            .expect("store blob")
            .expect("blob");
        assert!(paths.intelligence_blobs_dir.join(&stored.relative_path).exists());

        clear_readable_text_blobs(&paths).expect("clear blobs");
        assert!(paths.intelligence_blobs_dir.exists());
        assert!(!paths.intelligence_blobs_dir.join(&stored.relative_path).exists());
    }
}
