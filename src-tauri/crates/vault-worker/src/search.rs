//! Responsibilities: expose the owner's streaming search surface to every transport.
//! Not responsible for: opening databases, rewriting queries or limiting search coverage.
//! Failure modes: divergent worker/desktop behavior and sessions surviving Vault close.
//! Tests: search_tests and MCP search contracts.
use anyhow::Result;
use vault_core::{
    Vault,
    search::{IndexerState, SearchEvent, SearchRequest, SearchStarted},
};
pub fn start_search(
    vault: &Vault,
    request: SearchRequest,
    sink: impl FnMut(SearchEvent) -> bool + Send + 'static,
) -> Result<SearchStarted> {
    vault.search_sessions().start(vault, request, sink)
}
pub fn next_search_page(vault: &Vault, session_id: &str, cursor: String) -> Result<()> {
    vault.search_sessions().next_page(session_id, cursor)
}
pub fn cancel_search(vault: &Vault, session_id: &str) -> Result<()> {
    vault.search_sessions().cancel(session_id)
}
pub fn search_indexer_state(vault: &Vault) -> Result<IndexerState> {
    vault_core::search::indexer_state(vault)
}
