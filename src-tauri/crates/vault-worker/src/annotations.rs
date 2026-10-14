//! Worker-layer flows for per-URL notes and tags.
//!
//! These thin wrappers load the unlocked config + project paths from the
//! desktop session, hand the request to `vault_core::annotations`, and let
//! the `worker_bridge` layer convert errors to strings.
//!
//! Annotations are user-authored content that lives in the canonical
//! archive (migration 011), so every call respects the same encryption-key
//! flow as the rest of the archive surfaces.

use anyhow::Result;
use vault_core::Vault;
use vault_core::{ReplaceTagsRequest, SetNotesRequest, UrlAnnotation};

/// Reads the annotation bundle for a single URL. Returns `None` when no
/// notes have been written and no tags are attached.
pub fn get_annotation(vault: &Vault, url: &str) -> Result<Option<UrlAnnotation>> {
    vault_core::get_annotation(vault, url)
}

/// Sets or clears the notes body for a URL.
pub fn set_notes(vault: &Vault, request: SetNotesRequest) -> Result<UrlAnnotation> {
    vault_core::set_notes(vault, request)
}

/// Replaces the full tag set for a URL.
pub fn replace_tags(vault: &Vault, request: ReplaceTagsRequest) -> Result<UrlAnnotation> {
    vault_core::replace_tags(vault, request)
}

/// Lists URLs that carry at least one annotation, newest-first.
pub fn list_annotations(
    vault: &Vault,
    limit: Option<usize>,
    cursor: Option<String>,
) -> Result<vault_core::AnnotationPage> {
    vault_core::list_annotations(vault, limit, cursor)
}

/// Searches notes by case-insensitive substring.
pub fn search_annotations(
    vault: &Vault,
    query: &str,
    limit: Option<usize>,
    cursor: Option<String>,
) -> Result<vault_core::AnnotationPage> {
    vault_core::search_annotations(vault, query, limit, cursor)
}
