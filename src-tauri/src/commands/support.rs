#[cfg(not(test))]
use crate::file_manager;

#[cfg(not(test))]
#[tauri::command]
pub(crate) fn record_frontend_error(
    request: vault_core::FrontendErrorReportRequest,
) -> Result<vault_core::CrashReportSummary, String> {
    let paths = vault_core::project_paths().map_err(|error| error.to_string())?;
    vault_core::record_frontend_error(&paths, &request).map_err(|error| error.to_string())
}

#[cfg(not(test))]
#[tauri::command]
pub(crate) fn open_path_in_file_manager(path: String) -> Result<String, String> {
    file_manager::open_path_in_file_manager_impl(path)
}

#[cfg(not(test))]
#[tauri::command]
pub(crate) fn open_external_url(url: String) -> Result<String, String> {
    file_manager::open_external_url_impl(url)
}
