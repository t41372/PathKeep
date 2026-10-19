//! Responsibilities: resolve the unlocked session's serving model for the reads that reuse the
//! semantic index's vectors (History's related pages, similar searches, the Insights search-groups
//! card) and start the card's snapshot warm after an Insights build.
//! Not responsible for: scoring, grouping, index publication, or changing configuration.
//! Failure modes: reads bypass App Lock or use the chosen model while another one is serving; a
//! warm holds up the Insights lane or outlives "Delete all data".
//! Tests: semantic-index.spec.ts exercises the real commands with configured and removed services.
use crate::{
    context::{load_unlocked_config, selected_optional_embedding_runtime},
    intelligence::SEARCH_GROUP_WORKERS,
    job_runtime::{WorkerStopControl, maybe_spawn_worker_pool},
};
use anyhow::Result;
use std::sync::Arc;
use vault_core::AiRunControl;
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

/// Reads the optional Insights card under the unlocked session's serving index.
pub fn search_groups_card(
    vault: &Vault,
    request: &vault_core::ScopedDateRangeRequest,
) -> Result<vault_core::SearchGroupsCard> {
    let config = load_unlocked_config(vault.paths())?;
    if !config.ai.enabled || !config.ai.semantic_index_enabled {
        return Ok(vault_core::SearchGroupsCard::unavailable());
    }
    let provider = selected_optional_embedding_runtime(vault, &config)?;
    let card =
        vault_core::search_groups_card(&vault.with_config(config), provider.as_ref(), request)?;
    // A new day or an upgrade leaves windows to compute; the card polls while they update.
    if card.needs_warm {
        warm_search_groups(vault);
    }
    Ok(card)
}

/// Insights can be built after the index; warm the same snapshots in that order too. A first
/// warm groups every month, which can take a minute on a large archive, so it runs on its own
/// worker (counted, and stopped, like the queues) instead of holding up the Insights lane.
pub(crate) fn warm_search_groups(vault: &Vault) {
    let vault = vault.clone();
    maybe_spawn_worker_pool("pathkeep-search-groups", &SEARCH_GROUP_WORKERS, 1, move || {
        let warm = || -> Result<()> {
            let config = load_unlocked_config(vault.paths())?;
            if !config.ai.enabled || !config.ai.semantic_index_enabled {
                return Ok(());
            }
            if let Some(provider) = selected_optional_embedding_runtime(&vault, &config)? {
                let control: Arc<dyn AiRunControl> = Arc::new(WorkerStopControl);
                vault_core::warm_search_group_snapshots(
                    &vault.with_config(config),
                    &provider,
                    Some(&control),
                )?;
            }
            Ok(())
        };
        if let Err(error) = warm() {
            log::warn!("warming search groups after Insights: {error:#}");
        }
    });
}
