//! Tauri commands for stars (favorites / 加星).
//!
//! These commands let the desktop shell toggle and read stars from the paper
//! Explorer surfaces (list rows, detail panel, search results, assistant
//! evidence) and the Starred hub. Stars are user-authored content keyed by the
//! canonical entity in the archive (migration 014 + `vault-core::stars`).
//!
//! ## Performance notes
//! Every command runs the SQLite work off the Tauri UI thread via
//! `run_blocking_command`, so even on the 14.4M-row target the WebView never
//! blocks while a star write or the Starred-hub list resolves.

#[cfg(not(test))]
use super::blocking::run_blocking_command;
#[cfg(not(test))]
use crate::command_error::CommandError;
#[cfg(not(test))]
use crate::{session::SessionState, worker_bridge};
#[cfg(not(test))]
use std::collections::HashMap;
#[cfg(not(test))]
use tauri::State;

#[cfg(not(test))]
#[tauri::command]
/// Adds (or refreshes) a star for a page or domain.
pub(crate) async fn set_star(
    state: State<'_, SessionState>,
    request: vault_core::SetStarRequest,
) -> Result<(), CommandError> {
    let state = state.inner().clone();
    run_blocking_command("set_star", move || worker_bridge::set_star_impl(&state, request)).await
}

#[cfg(not(test))]
#[tauri::command]
/// Removes a star for a page or domain (no-op when not starred).
pub(crate) async fn unset_star(
    state: State<'_, SessionState>,
    request: vault_core::SetStarRequest,
) -> Result<(), CommandError> {
    let state = state.inner().clone();
    run_blocking_command("unset_star", move || worker_bridge::unset_star_impl(&state, request))
        .await
}

#[cfg(not(test))]
#[tauri::command]
/// Returns the starred status for the supplied (visible) keys only.
pub(crate) async fn get_star_status(
    state: State<'_, SessionState>,
    request: vault_core::StarStatusRequest,
) -> Result<HashMap<String, bool>, CommandError> {
    let state = state.inner().clone();
    run_blocking_command("get_star_status", move || {
        worker_bridge::is_starred_batch_impl(&state, request.entity_kind, &request.entity_keys)
    })
    .await
}

#[cfg(not(test))]
#[tauri::command]
/// Lists the Starred hub, enriched and ordered by the chosen sort.
pub(crate) async fn list_stars(
    state: State<'_, SessionState>,
    kind: Option<vault_core::StarEntityKind>,
    sort: vault_core::StarSort,
    limit: Option<usize>,
    cursor: Option<String>,
) -> Result<vault_core::StarPage, CommandError> {
    let state = state.inner().clone();
    run_blocking_command("list_stars", move || {
        worker_bridge::list_stars_impl(&state, kind, sort, limit, cursor)
    })
    .await
}

#[cfg(not(test))]
#[tauri::command]
/// Rolls up per-kind star counts for the Starred hub header.
pub(crate) async fn get_star_counts(
    state: State<'_, SessionState>,
) -> Result<vault_core::StarCounts, CommandError> {
    let state = state.inner().clone();
    run_blocking_command("get_star_counts", move || worker_bridge::star_counts_impl(&state)).await
}
