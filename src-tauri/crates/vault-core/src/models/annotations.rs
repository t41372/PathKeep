//! Per-URL notes and tags model.
//!
//! The Browse detail panel writes free-text notes and a set of tags against a
//! URL. These models are the contract between the desktop shell and the
//! `annotations` module — kept here so the type definitions are co-located
//! with the other backend read/write models.

use serde::{Deserialize, Serialize};

/// One per-URL annotation row. `notes` is the empty string when the user has
/// never written a note for the URL — the row only exists if there is either
/// a non-empty note OR at least one tag, so a fresh URL with no annotations
/// surfaces as `None` from `get_annotation`.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct UrlAnnotation {
    pub url: String,
    #[serde(default)]
    pub notes: String,
    #[serde(default)]
    pub tags: Vec<String>,
    /// RFC 3339 timestamp of the last notes write. Empty when no note has
    /// been written and only tags exist.
    #[serde(default)]
    pub updated_at: String,
    /// RFC 3339 timestamp of when the notes row first appeared. Same as
    /// `updated_at` until a subsequent write.
    #[serde(default)]
    pub created_at: String,
    /// Profile id that wrote the most-recent notes update, if any. Audit
    /// only — not part of the URL key.
    #[serde(default)]
    pub source_profile: Option<String>,
}

/// A request to set or replace the notes body for a URL. An empty notes
/// string clears the row entirely (unless tags still exist).
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SetNotesRequest {
    pub url: String,
    pub notes: String,
    #[serde(default)]
    pub source_profile: Option<String>,
}

/// A request to replace the full tag set for a URL. Tags are deduplicated
/// and trimmed before persistence; an empty list removes all tags.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ReplaceTagsRequest {
    pub url: String,
    pub tags: Vec<String>,
    #[serde(default)]
    pub source_profile: Option<String>,
}
