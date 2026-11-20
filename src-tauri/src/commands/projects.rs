//! Native project commands share the worker contract with the development bridge.
#![cfg(not(test))]
use super::{
    blocking::run_blocking_command,
    data_kinds::{Request, desktop_events},
};
use crate::{command_error::CommandError, session::SessionState, worker_bridge};
use serde_json::Value;
use tauri::{AppHandle, State};
use vault_worker::data_kinds::{self as kinds, projects};

#[tauri::command]
pub(crate) async fn list_agent_projects(
    state: State<'_, SessionState>,
    request: Request<projects::ProjectQuery>,
) -> Result<Value, CommandError> {
    let session = state.inner().clone();
    run_blocking_command("list_agent_projects", move || {
        worker_bridge::list_agent_projects_impl(&session, request.0)
    })
    .await
}

#[tauri::command]
pub(crate) async fn get_agent_project(
    state: State<'_, SessionState>,
    request: Request<kinds::chats::Conversation>,
) -> Result<Value, CommandError> {
    let session = state.inner().clone();
    run_blocking_command("get_agent_project", move || {
        worker_bridge::get_agent_project_impl(&session, request.0)
    })
    .await
}

#[tauri::command]
pub(crate) async fn list_agent_project_suggestions(
    state: State<'_, SessionState>,
    request: Request<projects::Suggestions>,
) -> Result<Value, CommandError> {
    let session = state.inner().clone();
    run_blocking_command("list_agent_project_suggestions", move || {
        worker_bridge::list_agent_project_suggestions_impl(&session, request.0)
    })
    .await
}

#[tauri::command]
pub(crate) async fn edit_agent_project(
    state: State<'_, SessionState>,
    request: Request<projects::ProjectEdit>,
    app: AppHandle,
) -> Result<Value, CommandError> {
    let session = state.inner().clone();
    run_blocking_command("edit_agent_project", move || {
        worker_bridge::edit_agent_project_impl(&session, request.0, desktop_events(app))
    })
    .await
}
