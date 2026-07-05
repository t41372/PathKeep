//! Tauri commands for per-URL annotations (notes + tags).
//!
//! These commands let the desktop shell write the notes textarea and tag
//! chips from the paper Browse detail panel into the canonical archive.
//! Reads use the session database key so encrypted archives stay walled
//! behind the App Lock session.

//! All five commands touch encrypted SQLite, so they run on the blocking thread pool: on the
//! 14.4M-record baseline an annotation read/write/search must never block the WebView thread.

#[cfg(not(test))]
use super::blocking::run_blocking_command;
#[cfg(not(test))]
use crate::{session::SessionState, worker_bridge};
#[cfg(not(test))]
use tauri::State;

#[cfg(not(test))]
#[tauri::command]
/// Returns the annotation bundle for a URL (None when nothing is written), off the UI thread.
pub(crate) async fn get_url_annotation(
    state: State<'_, SessionState>,
    url: String,
) -> Result<Option<vault_core::UrlAnnotation>, String> {
    let key = state.get_key();
    run_blocking_command("get_url_annotation", move || {
        worker_bridge::get_annotation_impl(key.as_deref(), &url)
    })
    .await
}

#[cfg(not(test))]
#[tauri::command]
/// Sets or clears the notes body for a URL. Empty body removes the row. Off the UI thread.
pub(crate) async fn set_url_notes(
    state: State<'_, SessionState>,
    request: vault_core::SetNotesRequest,
) -> Result<vault_core::UrlAnnotation, String> {
    let key = state.get_key();
    run_blocking_command("set_url_notes", move || {
        worker_bridge::set_notes_impl(key.as_deref(), request)
    })
    .await
}

#[cfg(not(test))]
#[tauri::command]
/// Replaces the full tag set for a URL. An empty list removes all tags. Off the UI thread.
pub(crate) async fn replace_url_tags(
    state: State<'_, SessionState>,
    request: vault_core::ReplaceTagsRequest,
) -> Result<vault_core::UrlAnnotation, String> {
    let key = state.get_key();
    run_blocking_command("replace_url_tags", move || {
        worker_bridge::replace_tags_impl(key.as_deref(), request)
    })
    .await
}

#[cfg(not(test))]
#[tauri::command]
/// Lists URLs that carry at least one annotation, newest first. Off the UI thread.
pub(crate) async fn list_url_annotations(
    state: State<'_, SessionState>,
    limit: Option<usize>,
) -> Result<Vec<vault_core::UrlAnnotation>, String> {
    let key = state.get_key();
    run_blocking_command("list_url_annotations", move || {
        worker_bridge::list_annotations_impl(key.as_deref(), limit)
    })
    .await
}

#[cfg(not(test))]
#[tauri::command]
/// Searches notes by case-insensitive substring across the archive, off the UI thread.
pub(crate) async fn search_url_annotations(
    state: State<'_, SessionState>,
    query: String,
    limit: Option<usize>,
) -> Result<Vec<vault_core::UrlAnnotation>, String> {
    let key = state.get_key();
    run_blocking_command("search_url_annotations", move || {
        worker_bridge::search_annotations_impl(key.as_deref(), &query, limit)
    })
    .await
}
