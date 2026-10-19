//! Responsibilities: canonical search, owner indexing and session contracts.
//! Not responsible for: UI presentation, archive writes or cryptography.
//! Failure modes: partial coverage hidden, stale visibility, lost cursors and orphaned jobs.
//! Tests: search::tests against plaintext and encrypted Vault owners.
mod documents;
mod eligibility;
mod generations;
mod indexer;
mod query;
pub(crate) mod regex;
mod regex_prefilter;
mod session;
#[cfg(test)]
mod tests;
mod types;
pub use indexer::{Indexer, IndexerPhase, IndexerState};

/// How far the index is behind the archive right now.
pub fn indexer_state(vault: &crate::Vault) -> anyhow::Result<IndexerState> {
    let canonical_seq = vault.store().read(vault_store::Db::Archive, documents::watermark)?;
    Ok(vault.search().state_against(canonical_seq))
}
pub(crate) use session::Scores;
pub use session::SearchSession;
pub use types::*;
mod stream;
pub use stream::{SearchEvent, SearchSessions, SearchStarted};
mod history;
pub use history::{history, page};
pub(crate) use history::{history_for_root, page_prepared, walk_history_for_export};
#[cfg(test)]
mod benchmark;
#[cfg(test)]
mod fixture;
#[cfg(test)]
mod ported_grouped;
#[cfg(test)]
mod ported_regex;
mod visits;
pub(crate) use visits::newest_visible_by_page;
pub(crate) use visits::starred_pages;
pub use visits::visits;
#[cfg(test)]
mod coverage_tests;
#[cfg(test)]
mod fuzzy_tests;
#[cfg(test)]
mod projection_tests;
#[cfg(test)]
mod recovery_tests;
