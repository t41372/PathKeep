//! Worker-bridge helpers for stars (favorites / 加星).

use super::worker_result;
use std::collections::HashMap;

/// Adds (or refreshes) a star — see `vault_core::set_star`.
#[cfg_attr(test, allow(dead_code))]
pub(crate) fn set_star_impl(
    session_database_key: Option<&str>,
    request: vault_core::SetStarRequest,
) -> Result<(), String> {
    worker_result(vault_worker::set_star(session_database_key, request))
}

/// Removes a star — see `vault_core::unset_star`.
#[cfg_attr(test, allow(dead_code))]
pub(crate) fn unset_star_impl(
    session_database_key: Option<&str>,
    request: vault_core::SetStarRequest,
) -> Result<(), String> {
    worker_result(vault_worker::unset_star(session_database_key, request))
}

/// Returns the starred status for the supplied (visible) keys only.
#[cfg_attr(test, allow(dead_code))]
pub(crate) fn is_starred_batch_impl(
    session_database_key: Option<&str>,
    kind: vault_core::StarEntityKind,
    keys: &[String],
) -> Result<HashMap<String, bool>, String> {
    worker_result(vault_worker::is_starred_batch(session_database_key, kind, keys))
}

/// Lists the Starred hub, enriched and ordered by the chosen sort.
#[cfg_attr(test, allow(dead_code))]
pub(crate) fn list_stars_impl(
    session_database_key: Option<&str>,
    kind: Option<vault_core::StarEntityKind>,
    sort: vault_core::StarSort,
    limit: Option<usize>,
) -> Result<Vec<vault_core::StarListItem>, String> {
    worker_result(vault_worker::list_stars(session_database_key, kind, sort, limit))
}

/// Rolls up per-kind star counts.
#[cfg_attr(test, allow(dead_code))]
pub(crate) fn star_counts_impl(
    session_database_key: Option<&str>,
) -> Result<vault_core::StarCounts, String> {
    worker_result(vault_worker::star_counts(session_database_key))
}
