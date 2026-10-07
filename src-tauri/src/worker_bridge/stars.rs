//! Worker-bridge helpers for stars (favorites / 加星).

use super::worker_result;
use crate::command_error::CommandError;
use std::collections::HashMap;

/// Adds (or refreshes) a star — see `vault_core::set_star`.
#[cfg_attr(test, allow(dead_code))]
pub(crate) fn set_star_impl(
    state: &crate::session::SessionState,
    request: vault_core::SetStarRequest,
) -> Result<(), CommandError> {
    worker_result(vault_worker::set_star(&state.vault()?, request))
}

/// Removes a star — see `vault_core::unset_star`.
#[cfg_attr(test, allow(dead_code))]
pub(crate) fn unset_star_impl(
    state: &crate::session::SessionState,
    request: vault_core::SetStarRequest,
) -> Result<(), CommandError> {
    worker_result(vault_worker::unset_star(&state.vault()?, request))
}

/// Returns the starred status for the supplied (visible) keys only.
#[cfg_attr(test, allow(dead_code))]
pub(crate) fn is_starred_batch_impl(
    state: &crate::session::SessionState,
    kind: vault_core::StarEntityKind,
    keys: &[String],
) -> Result<HashMap<String, bool>, CommandError> {
    worker_result(vault_worker::is_starred_batch(&state.vault()?, kind, keys))
}

/// Lists the Starred hub, enriched and ordered by the chosen sort.
#[cfg_attr(test, allow(dead_code))]
pub(crate) fn list_stars_impl(
    state: &crate::session::SessionState,
    kind: Option<vault_core::StarEntityKind>,
    sort: vault_core::StarSort,
    limit: Option<usize>,
    cursor: Option<String>,
) -> Result<vault_core::StarPage, CommandError> {
    worker_result(vault_worker::list_stars(&state.vault()?, kind, sort, limit, cursor))
}

/// Rolls up per-kind star counts.
#[cfg_attr(test, allow(dead_code))]
pub(crate) fn star_counts_impl(
    state: &crate::session::SessionState,
) -> Result<vault_core::StarCounts, CommandError> {
    worker_result(vault_worker::star_counts(&state.vault()?))
}
