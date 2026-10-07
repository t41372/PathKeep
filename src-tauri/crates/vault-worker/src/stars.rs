//! Worker-layer flows for stars (favorites / 加星).
//!
//! These thin wrappers load the unlocked config + project paths from the
//! desktop session, hand the request to `vault_core::stars`, and let the
//! `worker_bridge` layer convert errors to strings.
//!
//! Stars are user-authored content that lives in the canonical archive
//! (migration 014), so every call respects the same encryption-key flow as
//! the rest of the archive surfaces.

use anyhow::Result;
use std::collections::HashMap;
use vault_core::Vault;
use vault_core::{SetStarRequest, StarCounts, StarEntityKind, StarPage, StarSort};

/// Adds (or refreshes) a star for the canonical entity.
pub fn set_star(vault: &Vault, request: SetStarRequest) -> Result<()> {
    vault_core::set_star(vault, request)
}

/// Removes a star for the canonical entity (no-op when not starred).
pub fn unset_star(vault: &Vault, request: SetStarRequest) -> Result<()> {
    vault_core::unset_star(vault, request)
}

/// Returns the starred status for the supplied (visible) keys only.
pub fn is_starred_batch(
    vault: &Vault,
    kind: StarEntityKind,
    keys: &[String],
) -> Result<HashMap<String, bool>> {
    vault_core::is_starred_batch(vault, kind, keys)
}

/// Lists the Starred hub, enriched and ordered by the chosen sort.
pub fn list_stars(
    vault: &Vault,
    kind: Option<StarEntityKind>,
    sort: StarSort,
    limit: Option<usize>,
    cursor: Option<String>,
) -> Result<StarPage> {
    vault_core::list_stars(vault, kind, sort, limit, cursor)
}

/// Rolls up per-kind star counts for the Starred hub header / nav badge.
pub fn star_counts(vault: &Vault) -> Result<StarCounts> {
    vault_core::star_counts(vault)
}
