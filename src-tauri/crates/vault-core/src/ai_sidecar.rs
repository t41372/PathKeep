//! Deferred semantic-index sidecar boundary.
//!
//! ## Responsibilities
//! - Preserve the semantic sidecar API shape while optional AI is disabled for v0.1.0.
//! - Keep provider/model table-name derivation stable for future index rebuild compatibility.
//! - Report local sidecar storage bytes without linking the heavy vector-store runtime.
//!
//! ## Not responsible for
//! - Opening a vector database or syncing embedding payloads in v0.1.0.
//! - Running semantic nearest-neighbor search.
//! - Deciding when optional AI returns to the default desktop build.
//!
//! ## Dependencies
//! - Uses `ProjectPaths` for the existing sidecar directory contract.
//! - Uses `sha256_hex` to preserve stable provider/model naming.
//!
//! ## Performance notes
//! - This module is intentionally filesystem-only. It must not pull LanceDB,
//!   Arrow, DataFusion, or other vector-store dependencies into the v0.1.0 build.

use crate::{config::ProjectPaths, utils::sha256_hex};
use anyhow::{Result, bail};
use std::{
    fs,
    path::{Path, PathBuf},
};

const OPTIONAL_AI_DEFERRED_MESSAGE: &str = "AI Assistant, semantic search, embeddings, and vector indexing are coming in a future PathKeep release. PathKeep v0.1.0 ships the local archive and Core Intelligence first.";

/// One embedding row that the future sidecar sync path will persist.
///
/// The type remains part of the internal AI indexing contract so the rest of
/// the workspace can compile unchanged while the heavy vector-store dependency
/// is out of the v0.1.0 build.
#[derive(Debug, Clone)]
pub struct SidecarEmbeddingRow {
    pub history_id: i64,
    pub profile_id: String,
    pub url: String,
    pub title: Option<String>,
    pub domain: String,
    pub visited_at: String,
    pub provider_id: String,
    pub model: String,
    pub content_hash: String,
    pub indexed_at: String,
    pub vector: Vec<f32>,
}

/// One semantic-search result row returned by the future sidecar query path.
///
/// Keeping this shape stable avoids a wider API churn when optional AI returns
/// after the v0.1.0 release.
#[derive(Debug, Clone)]
pub struct SidecarSearchRow {
    pub history_id: i64,
    pub profile_id: String,
    pub url: String,
    pub title: Option<String>,
    pub domain: String,
    pub visited_at: String,
    pub score: f32,
}

/// Synchronizes embeddings into the optional vector sidecar when that feature is available.
///
/// In v0.1.0 this returns success only for no-op clear/empty sync requests so
/// cleanup and read-model code can remain harmless, while real embedding writes
/// fail fast with the release deferral message.
pub async fn sync_provider_embeddings(
    _paths: &ProjectPaths,
    _provider_id: &str,
    _model: &str,
    rows: &[SidecarEmbeddingRow],
    _full_rebuild: bool,
    clear_only: bool,
    _removed_history_ids: &[i64],
) -> Result<usize> {
    if clear_only || rows.is_empty() {
        return Ok(0);
    }

    bail!(OPTIONAL_AI_DEFERRED_MESSAGE)
}

/// Clears optional vector sidecar rows for one provider/model pair.
///
/// There is no vector sidecar in the v0.1.0 build, so this is a harmless no-op
/// that keeps derived-state cleanup and semantic-index clear requests bounded.
pub async fn clear_provider_embeddings(
    _paths: &ProjectPaths,
    _provider_id: &str,
    _model: &str,
) -> Result<usize> {
    Ok(0)
}

/// Counts optional vector sidecar rows for one provider/model pair.
///
/// Returning zero is the honest v0.1.0 state: compact SQLite metadata may still
/// exist from future-facing code paths, but no vector payload is linked here.
pub async fn count_provider_embeddings(
    _paths: &ProjectPaths,
    _provider_id: &str,
    _model: &str,
) -> Result<usize> {
    Ok(0)
}

/// Searches the optional vector sidecar for semantic matches.
///
/// The v0.1.0 build has no vector search dependency, so callers receive `None`
/// and can fall back without pretending semantic recall is available.
pub async fn search_provider_embeddings(
    _paths: &ProjectPaths,
    _provider_id: &str,
    _model: &str,
    _query_vector: &[f32],
    _profile_id: Option<&str>,
    _domain: Option<&str>,
    _limit: usize,
) -> Result<Option<Vec<SidecarSearchRow>>> {
    Ok(None)
}

/// Returns the total bytes currently consumed by optional sidecar storage.
///
/// This remains filesystem-backed so Settings and storage review surfaces can
/// still report leftover or future sidecar files without linking a vector DB.
pub fn sidecar_storage_bytes(paths: &ProjectPaths) -> u64 {
    directory_size(&sidecar_root(paths))
}

/// Returns the root directory reserved for optional vector sidecar tables.
///
/// The path stays stable even while the v0.1.0 build does not create or open a
/// vector database there.
pub fn sidecar_root(paths: &ProjectPaths) -> PathBuf {
    paths.semantic_index_dir.clone()
}

/// Derives the stable sidecar table name for one provider/model pair.
///
/// Future semantic-index rebuilds use this name to avoid mixing provider/model
/// embeddings. Keeping the derivation stable avoids unnecessary migrations.
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::project_paths_with_root;
    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;
    use tempfile::tempdir;

    fn row() -> SidecarEmbeddingRow {
        SidecarEmbeddingRow {
            history_id: 42,
            profile_id: "chrome:Default".to_string(),
            url: "https://example.com".to_string(),
            title: Some("Example".to_string()),
            domain: "example.com".to_string(),
            visited_at: "2026-04-29T00:00:00Z".to_string(),
            provider_id: "provider".to_string(),
            model: "model".to_string(),
            content_hash: "hash".to_string(),
            indexed_at: "2026-04-29T00:00:01Z".to_string(),
            vector: vec![1.0, 0.0, 0.0],
        }
    }

    #[tokio::test]
    async fn deferred_sidecar_accepts_only_noop_syncs() {
        let dir = tempdir().expect("tempdir");
        let paths = project_paths_with_root(dir.path());

        assert_eq!(
            sync_provider_embeddings(&paths, "provider", "model", &[], false, false, &[])
                .await
                .expect("empty sync"),
            0
        );
        assert_eq!(
            sync_provider_embeddings(&paths, "provider", "model", &[row()], false, true, &[])
                .await
                .expect("clear-only sync"),
            0
        );

        let error =
            sync_provider_embeddings(&paths, "provider", "model", &[row()], false, false, &[])
                .await
                .expect_err("embedding sync is deferred");
        assert!(error.to_string().contains("coming in a future PathKeep release"));
    }

    #[tokio::test]
    async fn deferred_sidecar_reports_empty_counts_and_searches() {
        let dir = tempdir().expect("tempdir");
        let paths = project_paths_with_root(dir.path());

        assert_eq!(clear_provider_embeddings(&paths, "provider", "model").await.expect("clear"), 0);
        assert_eq!(count_provider_embeddings(&paths, "provider", "model").await.expect("count"), 0);
        assert!(
            search_provider_embeddings(
                &paths,
                "provider",
                "model",
                &[1.0, 0.0, 0.0],
                Some("chrome:Default"),
                Some("example.com"),
                8,
            )
            .await
            .expect("search")
            .is_none()
        );
    }

    #[test]
    fn storage_and_table_helpers_stay_stable_without_vector_dependencies() {
        let dir = tempdir().expect("tempdir");
        let paths = project_paths_with_root(dir.path());

        assert_eq!(sidecar_storage_bytes(&paths), 0);
        fs::create_dir_all(sidecar_root(&paths).join("nested")).expect("create sidecar dir");
        fs::write(sidecar_root(&paths).join("nested").join("payload"), b"vector")
            .expect("write payload");

        assert_eq!(sidecar_storage_bytes(&paths), 6);
        assert_eq!(directory_size(&sidecar_root(&paths).join("missing")), 0);
        #[cfg(unix)]
        {
            let unreadable = sidecar_root(&paths).join("unreadable");
            fs::create_dir(&unreadable).expect("create unreadable directory");
            let original_permissions = fs::metadata(&unreadable).expect("metadata").permissions();
            fs::set_permissions(&unreadable, fs::Permissions::from_mode(0o000))
                .expect("lock unreadable directory");
            assert_eq!(directory_size(&unreadable), 0);
            fs::set_permissions(&unreadable, original_permissions)
                .expect("restore unreadable directory");
        }
        assert!(
            provider_table_name("openai-compatible", "text-embedding-3-small")
                .starts_with("pathkeep_openai_compatible_")
        );
    }
}
