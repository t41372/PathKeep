//! Worker-bridge helpers for per-URL annotations (notes + tags).

use super::worker_result;
use crate::command_error::CommandError;

/// Reads the annotation bundle for a URL — see `vault_core::get_annotation`.
#[cfg_attr(test, allow(dead_code))]
pub(crate) fn get_annotation_impl(
    state: &crate::session::SessionState,
    url: &str,
) -> Result<Option<vault_core::UrlAnnotation>, CommandError> {
    worker_result(vault_worker::get_annotation(&state.vault()?, url))
}

/// Sets or clears the notes body for a URL.
#[cfg_attr(test, allow(dead_code))]
pub(crate) fn set_notes_impl(
    state: &crate::session::SessionState,
    request: vault_core::SetNotesRequest,
) -> Result<vault_core::UrlAnnotation, CommandError> {
    worker_result(vault_worker::set_notes(&state.vault()?, request))
}

/// Replaces the full tag set for a URL.
#[cfg_attr(test, allow(dead_code))]
pub(crate) fn replace_tags_impl(
    state: &crate::session::SessionState,
    request: vault_core::ReplaceTagsRequest,
) -> Result<vault_core::UrlAnnotation, CommandError> {
    worker_result(vault_worker::replace_tags(&state.vault()?, request))
}

/// Lists URLs that carry at least one annotation, newest-first.
#[cfg_attr(test, allow(dead_code))]
pub(crate) fn list_annotations_impl(
    state: &crate::session::SessionState,
    limit: Option<usize>,
    cursor: Option<String>,
) -> Result<vault_core::AnnotationPage, CommandError> {
    worker_result(vault_worker::list_annotations(&state.vault()?, limit, cursor))
}

/// Searches notes by case-insensitive substring.
#[cfg_attr(test, allow(dead_code))]
pub(crate) fn search_annotations_impl(
    state: &crate::session::SessionState,
    query: &str,
    limit: Option<usize>,
    cursor: Option<String>,
) -> Result<vault_core::AnnotationPage, CommandError> {
    worker_result(vault_worker::search_annotations(&state.vault()?, query, limit, cursor))
}
