//! Responsibilities: ask the person where to save a file the app writes for them (an exported
//! conversation or history), through the system's save dialog.
//! Not responsible for: writing the file or naming it; the caller suggests the name.
//! Failure modes: no window to attach the dialog to; an end-to-end test cannot answer it.

use crate::command_error::CommandError;

use super::worker_result;

/// Where the person wants the file, suggested name and Downloads first; `None` when they
/// cancel. `filter` is the file kind's name and its extensions. An end-to-end test cannot answer
/// the system's dialog, so a debug build with `PATHKEEP_TEST_SAVE_DIR` saves there instead
/// (`vault_core::test_support::test_save_dir`). Blocks on the dialog: call it off the main thread.
pub(crate) fn save_path<R: tauri::Runtime>(
    app: Option<&tauri::AppHandle<R>>,
    name: &str,
    filter: (&str, &[&str]),
) -> Result<Option<std::path::PathBuf>, CommandError> {
    use tauri::Manager;
    use tauri_plugin_dialog::DialogExt;
    if let Some(dir) = vault_core::test_support::test_save_dir() {
        worker_result(std::fs::create_dir_all(&dir))?;
        return Ok(Some(dir.join(name)));
    }
    let app =
        app.ok_or_else(|| CommandError::internal("There is no window to ask where to save."))?;
    let mut dialog = app.dialog().file().set_file_name(name).add_filter(filter.0, filter.1);
    if let Ok(downloads) = app.path().download_dir() {
        dialog = dialog.set_directory(downloads);
    }
    dialog.blocking_save_file().map(|path| worker_result(path.into_path())).transpose()
}
