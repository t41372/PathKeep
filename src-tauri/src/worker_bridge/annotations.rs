//! Worker-bridge helpers for per-URL annotations (notes + tags).

use super::worker_result;

/// Reads the annotation bundle for a URL — see `vault_core::get_annotation`.
#[cfg_attr(test, allow(dead_code))]
pub(crate) fn get_annotation_impl(
    session_database_key: Option<&str>,
    url: &str,
) -> Result<Option<vault_core::UrlAnnotation>, String> {
    worker_result(vault_worker::get_annotation(session_database_key, url))
}

/// Sets or clears the notes body for a URL.
#[cfg_attr(test, allow(dead_code))]
pub(crate) fn set_notes_impl(
    session_database_key: Option<&str>,
    request: vault_core::SetNotesRequest,
) -> Result<vault_core::UrlAnnotation, String> {
    worker_result(vault_worker::set_notes(session_database_key, request))
}

/// Replaces the full tag set for a URL.
#[cfg_attr(test, allow(dead_code))]
pub(crate) fn replace_tags_impl(
    session_database_key: Option<&str>,
    request: vault_core::ReplaceTagsRequest,
) -> Result<vault_core::UrlAnnotation, String> {
    worker_result(vault_worker::replace_tags(session_database_key, request))
}

/// Lists URLs that carry at least one annotation, newest-first.
#[cfg_attr(test, allow(dead_code))]
pub(crate) fn list_annotations_impl(
    session_database_key: Option<&str>,
    limit: Option<usize>,
) -> Result<Vec<vault_core::UrlAnnotation>, String> {
    worker_result(vault_worker::list_annotations(session_database_key, limit))
}

/// Searches notes by case-insensitive substring.
#[cfg_attr(test, allow(dead_code))]
pub(crate) fn search_annotations_impl(
    session_database_key: Option<&str>,
    query: &str,
    limit: Option<usize>,
) -> Result<Vec<vault_core::UrlAnnotation>, String> {
    worker_result(vault_worker::search_annotations(session_database_key, query, limit))
}
