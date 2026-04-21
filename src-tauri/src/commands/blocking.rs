//! Shared helpers for command handlers that must avoid blocking the Tauri UI thread.
//!
//! Long-running archive and intelligence work still belongs in Rust, but the
//! command façade must execute it off the main thread so the WebView can keep
//! repainting busy overlays and progress updates honestly.

#[cfg(not(test))]
use tauri::async_runtime;

#[cfg(not(test))]
pub(super) async fn run_blocking_command<T: Send + 'static>(
    command_name: &'static str,
    task: impl FnOnce() -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    async_runtime::spawn_blocking(task).await.map_err(|error| {
        format!("PathKeep desktop command \"{command_name}\" join failed: {error}")
    })?
}
