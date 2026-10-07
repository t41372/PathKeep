//! Responsibilities: resolve the desktop Vault for streaming search commands.
//! Not responsible for: search execution, channel buffering or UI copy.
//! Failure modes: transport reopens storage or loses typed owner errors.
//! Tests: dev bridge search contract tests.
use super::worker_result;
use crate::{command_error::CommandError, session::SessionState};
use vault_core::search::{IndexerState, SearchEvent, SearchRequest, SearchStarted};
pub(crate) fn start_search_impl(
    state: &SessionState,
    request: SearchRequest,
    sink: impl FnMut(SearchEvent) -> bool + Send + 'static,
) -> Result<SearchStarted, CommandError> {
    worker_result(vault_worker::start_search(&state.vault()?, request, sink))
}
pub(crate) fn next_search_page_impl(
    state: &SessionState,
    session_id: &str,
    cursor: String,
) -> Result<(), CommandError> {
    worker_result(vault_worker::next_search_page(&state.vault()?, session_id, cursor))
}
pub(crate) fn cancel_search_impl(
    state: &SessionState,
    session_id: &str,
) -> Result<(), CommandError> {
    worker_result(vault_worker::cancel_search(&state.vault()?, session_id))
}
pub(crate) fn search_indexer_state_impl(
    state: &SessionState,
) -> Result<IndexerState, CommandError> {
    worker_result(vault_worker::search_indexer_state(&state.vault()?))
}

/// Debug builds of the dev bridge only: lets E2E hold the search indexer to show partial coverage.
#[cfg(all(debug_assertions, feature = "devtools-bridge"))]
pub(crate) fn debug_set_search_indexer_paused_impl(
    state: &SessionState,
    paused: bool,
) -> Result<(), CommandError> {
    state.vault()?.search().set_paused_for_tests(paused);
    Ok(())
}
