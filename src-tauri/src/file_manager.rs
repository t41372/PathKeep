//! Tiny shell-launcher wrappers for the desktop support commands.
//!
//! The command facade keeps file-manager/URL opening isolated here so tests can
//! exercise the wrapper without pulling in the whole command module.

use crate::command_error::CommandError;

/// Opens one local path through the platform launcher and returns the same path on success.
pub(crate) fn open_path_in_file_manager_impl(path: String) -> Result<String, CommandError> {
    Ok(vault_platform::open_path_in_file_manager(path)?)
}

/// Opens one trusted launcher URL through the platform launcher and returns it on success.
pub(crate) fn open_external_url_impl(url: String) -> Result<String, CommandError> {
    Ok(vault_platform::open_external_url(url)?)
}

/// Opens a page from the user's history in their browser or the app that handles it. A `file://`
/// URL opens only when the archive holds a visit to it: such a URL can launch an application, and a
/// link in an Ask answer is written by a model that may have read a hostile page.
pub(crate) fn open_visited_page_impl(
    url: String,
    state: &crate::session::SessionState,
) -> Result<String, CommandError> {
    if !url.trim().to_ascii_lowercase().starts_with("file://") {
        return open_external_url_impl(url);
    }
    if !vault_worker::is_visited_url(&state.vault()?, &url).map_err(|error| error.to_string())? {
        return Err(CommandError::from(format!(
            "not-in-history: {url} is not a page in your history, so PathKeep did not open it."
        )));
    }
    Ok(vault_platform::open_visited_file_url(url)?)
}

/// Reveals the local log directory through the platform file manager, returning the revealed path.
///
/// Resolves the log directory from the project paths so the user (and any bug report) can reach
/// `logs/rust.log` in one click — diagnostics must never be a hidden, hand-typed path.
pub(crate) fn reveal_logs_impl() -> Result<String, CommandError> {
    let paths = vault_core::project_paths().map_err(|error| error.to_string())?;
    open_path_in_file_manager_impl(paths.logs_dir.display().to_string())
}
