//! Responsibilities: Tauri channel commands for owner search sessions.
//! Not responsible for: query execution, result retention or UI presentation.
//! Failure modes: start blocks the UI; transports use different request/event contracts.
//! Tests: the Rust dev bridge and worker contract suites.
#![cfg(not(test))]
use super::blocking::run_blocking_command;
use crate::{command_error::CommandError, session::SessionState, worker_bridge};
use tauri::{State, ipc::Channel};
use vault_core::search::{IndexerState, SearchEvent, SearchRequest, SearchStarted};
#[tauri::command]
pub(crate) async fn start_search(
    state: State<'_, SessionState>,
    request: SearchRequest,
    on_event: Channel<SearchEvent>,
) -> Result<SearchStarted, CommandError> {
    let state = state.inner().clone();
    run_blocking_command("start_search", move || {
        worker_bridge::start_search_impl(&state, request, move |event| on_event.send(event).is_ok())
    })
    .await
}
#[tauri::command]
pub(crate) async fn next_search_page(
    state: State<'_, SessionState>,
    session_id: String,
    cursor: String,
) -> Result<(), CommandError> {
    let state = state.inner().clone();
    run_blocking_command("next_search_page", move || {
        worker_bridge::next_search_page_impl(&state, &session_id, cursor)
    })
    .await
}
#[tauri::command]
pub(crate) async fn cancel_search(
    state: State<'_, SessionState>,
    session_id: String,
) -> Result<(), CommandError> {
    let state = state.inner().clone();
    run_blocking_command("cancel_search", move || {
        worker_bridge::cancel_search_impl(&state, &session_id)
    })
    .await
}
#[tauri::command]
pub(crate) async fn search_indexer_state(
    state: State<'_, SessionState>,
) -> Result<IndexerState, CommandError> {
    let state = state.inner().clone();
    run_blocking_command("search_indexer_state", move || {
        worker_bridge::search_indexer_state_impl(&state)
    })
    .await
}
