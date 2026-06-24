//! Tauri commands for diagnostics and shell integration helpers.
//!
//! Writing a crash report and spawning the native file manager / launcher are blocking I/O
//! (filesystem + process spawn), so each command runs on the blocking thread pool to keep the
//! WebView thread free.

#[cfg(not(test))]
use super::blocking::run_blocking_command;
#[cfg(not(test))]
use crate::file_manager;

#[cfg(not(test))]
#[tauri::command]
/// Persists a frontend error report into the local crash-report area, off the UI thread.
pub(crate) async fn record_frontend_error(
    request: vault_core::FrontendErrorReportRequest,
) -> Result<vault_core::CrashReportSummary, String> {
    run_blocking_command("record_frontend_error", move || {
        let paths = vault_core::project_paths().map_err(|error| error.to_string())?;
        vault_core::record_frontend_error(&paths, &request).map_err(|error| error.to_string())
    })
    .await
}

#[cfg(not(test))]
#[tauri::command]
/// Opens one filesystem path through the native file manager, off the UI thread.
pub(crate) async fn open_path_in_file_manager(path: String) -> Result<String, String> {
    run_blocking_command("open_path_in_file_manager", move || {
        file_manager::open_path_in_file_manager_impl(path)
    })
    .await
}

#[cfg(not(test))]
#[tauri::command]
/// Opens one trusted launcher URL through the native launcher, off the UI thread.
pub(crate) async fn open_external_url(url: String) -> Result<String, String> {
    run_blocking_command("open_external_url", move || file_manager::open_external_url_impl(url))
        .await
}

#[cfg(not(test))]
#[tauri::command]
/// Writes an exported AI-assistant conversation (Markdown / JSON) to a user-chosen path, off the
/// UI thread, and returns the byte count written.
pub(crate) async fn export_conversation_file(
    target_path: String,
    contents: String,
) -> Result<u64, String> {
    run_blocking_command("export_conversation_file", move || {
        file_manager::export_conversation_file_impl(target_path, contents)
    })
    .await
}
