//! Star (favorites / 加星) models.
//!
//! A star marks a canonical entity — a page (`canonical_url`) or a source
//! (`registrable_domain`) — as a user favorite. These serde types are the
//! contract between the desktop shell, the worker layer, and the `stars`
//! backend module (migration 014). They mirror the annotations models: keyed
//! by the canonical entity, profile-agnostic, and portable across the export
//! bundle.

use serde::{Deserialize, Serialize};

/// The kind of entity a star points at. The MVP supports pages (`url`) and
/// sources (`domain`); `query_family` stars are deferred so the enum can grow
/// without breaking the on-disk `entity_kind` text values.
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum StarEntityKind {
    /// A single page, keyed by its `canonical_url`.
    #[default]
    Url,
    /// A whole source, keyed by its `registrable_domain`.
    Domain,
}

impl StarEntityKind {
    /// Returns the stable text value persisted in the `star.entity_kind`
    /// column. This is the single source of truth for the SQLite encoding —
    /// keep it aligned with the `#[serde]` rename so the wire and storage
    /// forms never drift.
    pub fn as_str(self) -> &'static str {
        match self {
            StarEntityKind::Url => "url",
            StarEntityKind::Domain => "domain",
        }
    }
}

/// How `list_stars` orders the Starred hub.
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum StarSort {
    /// Newest stars first (`starred_at DESC`) — rides the kind index.
    #[default]
    RecentlyStarred,
    /// Most-revisited first, via a join to the per-URL visit-count read model.
    MostRevisited,
}

/// A request to add or remove a star. `entity_key` is the `canonical_url`
/// (for `Url`) or the `registrable_domain` (for `Domain`); the backend
/// canonicalizes raw URLs before keying so callers may pass either.
///
/// Stars key by canonical_url BY DESIGN (annotations key by RAW url): a star is
/// page identity, so tracking-param + host-casing variants collapse onto one
/// canonical key and the star survives re-import. `set_star`, `unset_star`, and
/// `is_starred_batch` all canonicalize the supplied key before reading/writing.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SetStarRequest {
    pub entity_kind: StarEntityKind,
    pub entity_key: String,
    /// Audit-only profile id captured at write time. Never part of the key.
    #[serde(default)]
    pub source_profile: Option<String>,
}

/// A request to read star status for the currently-visible rows only. The
/// frontend batches one call per render window so the lookup never fans out
/// across the whole archive.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub struct StarStatusRequest {
    pub entity_kind: StarEntityKind,
    pub entity_keys: Vec<String>,
}

/// One starred entity returned by `list_stars`, enriched with the display
/// fields the Starred hub renders without a second round-trip.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct StarListItem {
    pub entity_kind: StarEntityKind,
    pub entity_key: String,
    /// RFC 3339 timestamp the star was created.
    pub starred_at: String,
    /// Registrable domain for the entity — equals `entity_key` for domain
    /// stars, derived from the canonical URL for page stars. Empty when it
    /// cannot be derived.
    #[serde(default)]
    pub domain: String,
    /// Best-known page title (page stars only), resolved from the canonical
    /// page (i.e. across every raw-URL variant that collapses to the star key).
    /// Empty when the archive has no row for the page yet.
    #[serde(default)]
    pub title: String,
    /// Total visit count for the entity, SUMMED across every raw-URL variant of
    /// the canonical page (or every URL on the domain). Drives the
    /// most-revisited sort and the hub's "{n}×" affordance.
    #[serde(default)]
    pub visit_count: i64,
}

/// Rollup of how many things the user has starred, per kind.
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct StarCounts {
    pub urls: i64,
    pub domains: i64,
}
