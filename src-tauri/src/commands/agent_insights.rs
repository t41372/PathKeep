//! Off-thread Agent Insights commands. Report frames use the same private channel as search.
#![cfg(not(test))]
use super::{
    blocking::run_blocking_command,
    data_kinds::{Request, desktop_events, forward_frames},
};
use crate::{command_error::CommandError, session::SessionState, worker_bridge};
use serde_json::Value;
use tauri::{AppHandle, State, ipc::Channel};
use vault_worker::data_kinds as kinds;

#[tauri::command]
pub(crate) async fn get_agent_insights(
    state: State<'_, SessionState>,
    request: Request<kinds::insights::Query>,
    on_event: Channel<Value>,
) -> Result<kinds::preview::Started, CommandError> {
    let session = state.inner().clone();
    let frames = run_blocking_command("get_agent_insights", move || {
        worker_bridge::get_agent_insights_impl(&session, request.0)
    })
    .await?;
    Ok(forward_frames(frames, on_event))
}

#[tauri::command]
pub(crate) async fn get_agent_insights_preferences(
    state: State<'_, SessionState>,
    request: Request<kinds::Empty>,
) -> Result<Value, CommandError> {
    let session = state.inner().clone();
    run_blocking_command("get_agent_insights_preferences", move || {
        worker_bridge::get_agent_insights_preferences_impl(&session, request.0)
    })
    .await
}

#[tauri::command]
pub(crate) async fn set_agent_insights_preferences(
    state: State<'_, SessionState>,
    request: Request<kinds::insights::PreferencePatch>,
    app: AppHandle,
) -> Result<Value, CommandError> {
    let session = state.inner().clone();
    run_blocking_command("set_agent_insights_preferences", move || {
        worker_bridge::set_agent_insights_preferences_impl(&session, request.0, desktop_events(app))
    })
    .await
}
