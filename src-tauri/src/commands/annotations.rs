//! Tauri commands for per-URL annotations (notes + tags).
//!
//! These commands let the desktop shell write the notes textarea and tag
//! chips from the paper Browse detail panel into the canonical archive.
//! Reads use the session database key so encrypted archives stay walled
//! behind the App Lock session.

#[cfg(not(test))]
use crate::{session::SessionState, worker_bridge};
#[cfg(not(test))]
use tauri::State;

#[cfg(not(test))]
#[tauri::command]
/// Returns the annotation bundle for a URL (None when nothing is written).
pub(crate) fn get_url_annotation(
    state: State<'_, SessionState>,
    url: String,
) -> Result<Option<vault_core::UrlAnnotation>, String> {
    worker_bridge::get_annotation_impl(state.get_key().as_deref(), &url)
}

#[cfg(not(test))]
#[tauri::command]
/// Sets or clears the notes body for a URL. Empty body removes the row.
pub(crate) fn set_url_notes(
    state: State<'_, SessionState>,
    request: vault_core::SetNotesRequest,
) -> Result<vault_core::UrlAnnotation, String> {
    worker_bridge::set_notes_impl(state.get_key().as_deref(), request)
}

#[cfg(not(test))]
#[tauri::command]
/// Replaces the full tag set for a URL. An empty list removes all tags.
pub(crate) fn replace_url_tags(
    state: State<'_, SessionState>,
    request: vault_core::ReplaceTagsRequest,
) -> Result<vault_core::UrlAnnotation, String> {
    worker_bridge::replace_tags_impl(state.get_key().as_deref(), request)
}

#[cfg(not(test))]
#[tauri::command]
/// Lists URLs that carry at least one annotation, newest first.
pub(crate) fn list_url_annotations(
    state: State<'_, SessionState>,
    limit: Option<usize>,
) -> Result<Vec<vault_core::UrlAnnotation>, String> {
    worker_bridge::list_annotations_impl(state.get_key().as_deref(), limit)
}

#[cfg(not(test))]
#[tauri::command]
/// Searches notes by case-insensitive substring across the archive.
pub(crate) fn search_url_annotations(
    state: State<'_, SessionState>,
    query: String,
    limit: Option<usize>,
) -> Result<Vec<vault_core::UrlAnnotation>, String> {
    worker_bridge::search_annotations_impl(state.get_key().as_deref(), &query, limit)
}
