//! Worker-layer flows for per-URL notes and tags.
//!
//! These thin wrappers load the unlocked config + project paths from the
//! desktop session, hand the request to `vault_core::annotations`, and let
//! the `worker_bridge` layer convert errors to strings.
//!
//! Annotations are user-authored content that lives in the canonical
//! archive (migration 011), so every call respects the same encryption-key
//! flow as the rest of the archive surfaces.

use crate::context::load_unlocked_config;
use anyhow::Result;
use vault_core::{ReplaceTagsRequest, SetNotesRequest, UrlAnnotation};

/// Reads the annotation bundle for a single URL. Returns `None` when no
/// notes have been written and no tags are attached.
pub fn get_annotation(
    session_database_key: Option<&str>,
    url: &str,
) -> Result<Option<UrlAnnotation>> {
    let paths = vault_core::project_paths()?;
    let config = load_unlocked_config(&paths)?;
    vault_core::get_annotation(&paths, &config, session_database_key, url)
}

/// Sets or clears the notes body for a URL.
pub fn set_notes(
    session_database_key: Option<&str>,
    request: SetNotesRequest,
) -> Result<UrlAnnotation> {
    let paths = vault_core::project_paths()?;
    let config = load_unlocked_config(&paths)?;
    vault_core::set_notes(&paths, &config, session_database_key, request)
}

/// Replaces the full tag set for a URL.
pub fn replace_tags(
    session_database_key: Option<&str>,
    request: ReplaceTagsRequest,
) -> Result<UrlAnnotation> {
    let paths = vault_core::project_paths()?;
    let config = load_unlocked_config(&paths)?;
    vault_core::replace_tags(&paths, &config, session_database_key, request)
}

/// Lists URLs that carry at least one annotation, newest-first.
pub fn list_annotations(
    session_database_key: Option<&str>,
    limit: Option<usize>,
) -> Result<Vec<UrlAnnotation>> {
    let paths = vault_core::project_paths()?;
    let config = load_unlocked_config(&paths)?;
    vault_core::list_annotations(&paths, &config, session_database_key, limit)
}

/// Searches notes by case-insensitive substring.
pub fn search_annotations(
    session_database_key: Option<&str>,
    query: &str,
    limit: Option<usize>,
) -> Result<Vec<UrlAnnotation>> {
    let paths = vault_core::project_paths()?;
    let config = load_unlocked_config(&paths)?;
    vault_core::search_annotations(&paths, &config, session_database_key, query, limit)
}
