//! Preview, prepare and resolve the previous-version handoff on the blocking owner thread.
#![cfg(not(test))]
use super::blocking::run_blocking_command;
use crate::{command_error::CommandError, session::SessionState, worker_bridge};
use tauri::State;

#[tauri::command]
pub(crate) async fn preview_archive_rollback()
-> Result<vault_core::maintenance::rollback::Preview, CommandError> {
    run_blocking_command("preview_archive_rollback", worker_bridge::rollback_preview).await
}

#[tauri::command]
pub(crate) async fn prepare_archive_rollback(
    state: State<'_, SessionState>,
    current_password: Option<String>,
) -> Result<std::path::PathBuf, CommandError> {
    let session = state.inner().clone();
    run_blocking_command("prepare_archive_rollback", move || {
        worker_bridge::rollback_prepare(&session, current_password)
    })
    .await
}

#[tauri::command]
pub(crate) async fn resolve_archive_handoff(
    state: State<'_, SessionState>,
    rejoin: bool,
    current_password: Option<String>,
    package_password: Option<String>,
) -> Result<(), CommandError> {
    let session = state.inner().clone();
    run_blocking_command("resolve_archive_handoff", move || {
        worker_bridge::rollback_resolve(&session, rejoin, current_password, package_password)
    })
    .await
}
