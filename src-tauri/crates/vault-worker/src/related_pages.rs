//! Responsibilities: resolve the unlocked session's serving model for History's related pages.
//! Not responsible for: scoring, index publication, or changing configuration.
//! Failure modes: reads bypass App Lock or use the chosen model while another one is serving.
//! Tests: semantic-index.spec.ts exercises the real command with configured and removed services.
use crate::context::{load_unlocked_config, selected_optional_embedding_runtime};
use anyhow::Result;
use vault_core::{RelatedPages, Vault};

/// Resolves the same serving pair as semantic search without making an embedding request.
pub fn related_pages(vault: &Vault, url: &str, visit_id: Option<i64>) -> Result<RelatedPages> {
    let config = load_unlocked_config(vault.paths())?;
    if !config.ai.enabled || !config.ai.semantic_index_enabled {
        return Ok(RelatedPages::unavailable("semantic search is off"));
    }
    let provider = match selected_optional_embedding_runtime(vault, &config) {
        Ok(provider) => provider,
        Err(error) => return Ok(RelatedPages::unavailable(error.to_string())),
    };
    vault_core::related_pages(&vault.with_config(config), provider.as_ref(), url, visit_id)
}

/// Similar searches share the unlocked serving-provider resolution used by related pages.
pub fn similar_searches(
    vault: &Vault,
    query: &str,
    wordings: &[String],
) -> Result<vault_core::SimilarSearches> {
    let config = load_unlocked_config(vault.paths())?;
    if !config.ai.enabled || !config.ai.semantic_index_enabled {
        return Ok(vault_core::SimilarSearches::unavailable("semantic search is off"));
    }
    let provider = match selected_optional_embedding_runtime(vault, &config) {
        Ok(provider) => provider,
        Err(e) => return Ok(vault_core::SimilarSearches::unavailable(e.to_string())),
    };
    vault_core::similar_searches(&vault.with_config(config), provider.as_ref(), query, wordings)
}
