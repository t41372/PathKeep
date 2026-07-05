//! Worker-layer flows for stars (favorites / 加星).
//!
//! These thin wrappers load the unlocked config + project paths from the
//! desktop session, hand the request to `vault_core::stars`, and let the
//! `worker_bridge` layer convert errors to strings.
//!
//! Stars are user-authored content that lives in the canonical archive
//! (migration 014), so every call respects the same encryption-key flow as
//! the rest of the archive surfaces.

use crate::context::load_unlocked_config;
use anyhow::Result;
use std::collections::HashMap;
use vault_core::{SetStarRequest, StarCounts, StarEntityKind, StarListItem, StarSort};

/// Adds (or refreshes) a star for the canonical entity.
pub fn set_star(session_database_key: Option<&str>, request: SetStarRequest) -> Result<()> {
    let paths = vault_core::project_paths()?;
    let config = load_unlocked_config(&paths)?;
    vault_core::set_star(&paths, &config, session_database_key, request)
}

/// Removes a star for the canonical entity (no-op when not starred).
pub fn unset_star(session_database_key: Option<&str>, request: SetStarRequest) -> Result<()> {
    let paths = vault_core::project_paths()?;
    let config = load_unlocked_config(&paths)?;
    vault_core::unset_star(&paths, &config, session_database_key, request)
}

/// Returns the starred status for the supplied (visible) keys only.
pub fn is_starred_batch(
    session_database_key: Option<&str>,
    kind: StarEntityKind,
    keys: &[String],
) -> Result<HashMap<String, bool>> {
    let paths = vault_core::project_paths()?;
    let config = load_unlocked_config(&paths)?;
    vault_core::is_starred_batch(&paths, &config, session_database_key, kind, keys)
}

/// Lists the Starred hub, enriched and ordered by the chosen sort.
pub fn list_stars(
    session_database_key: Option<&str>,
    kind: Option<StarEntityKind>,
    sort: StarSort,
    limit: Option<usize>,
) -> Result<Vec<StarListItem>> {
    let paths = vault_core::project_paths()?;
    let config = load_unlocked_config(&paths)?;
    vault_core::list_stars(&paths, &config, session_database_key, kind, sort, limit)
}

/// Rolls up per-kind star counts for the Starred hub header / nav badge.
pub fn star_counts(session_database_key: Option<&str>) -> Result<StarCounts> {
    let paths = vault_core::project_paths()?;
    let config = load_unlocked_config(&paths)?;
    vault_core::star_counts(&paths, &config, session_database_key)
}
