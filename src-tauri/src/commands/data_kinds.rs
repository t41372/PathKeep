//! Interactive policy snapshots and metadata preview for the source picker.
//! The owner worker handles admission, storage, enumeration and cancellation.
#![cfg(not(test))]
use super::blocking::run_blocking_command;
use crate::{command_error::CommandError, session::SessionState, worker_bridge};
use serde_json::Value;
use tauri::{AppHandle, Emitter, State, ipc::Channel};
use vault_worker::data_kinds::{self as kinds, preview};

/// Preserve the domain error envelope even when the request wrapper is missing.
pub(crate) struct Request<T>(T);
impl<'de, R: tauri::Runtime, T: serde::Deserialize<'de>> tauri::ipc::CommandArg<'de, R>
    for Request<T>
{
    fn from_command(
        item: tauri::ipc::CommandItem<'de, R>,
    ) -> Result<Self, tauri::ipc::InvokeError> {
        T::deserialize(item).map(Self).map_err(|e| CommandError::invalid_request(e).into())
    }
}

#[tauri::command]
pub(crate) async fn list_data_kinds(
    state: State<'_, SessionState>,
    request: Request<kinds::Empty>,
) -> Result<kinds::KindList, CommandError> {
    let session = state.inner().clone();
    run_blocking_command("list_data_kinds", move || {
        worker_bridge::list_data_kinds_impl(&session, request.0)
    })
    .await
}
#[tauri::command]
pub(crate) async fn set_data_kind_state(
    state: State<'_, SessionState>,
    app: AppHandle,
    request: Request<kinds::SetState>,
) -> Result<kinds::Transition, CommandError> {
    let session = state.inner().clone();
    run_blocking_command("set_data_kind_state", move || {
        worker_bridge::set_data_kind_state_impl(&session, request.0, |event| {
            let _ = app.emit(kinds::POLICY_EVENT, event);
        })
    })
    .await
}
#[tauri::command]
pub(crate) async fn get_agent_settings(
    request: Request<kinds::Empty>,
) -> Result<kinds::Settings, CommandError> {
    run_blocking_command("get_agent_settings", move || {
        worker_bridge::get_agent_settings_impl(request.0)
    })
    .await
}
#[tauri::command]
pub(crate) async fn update_agent_sources(
    app: AppHandle,
    request: Request<kinds::SourcesPatch>,
) -> Result<kinds::Settings, CommandError> {
    run_blocking_command("update_agent_sources", move || {
        worker_bridge::update_agent_sources_impl(request.0, |event| {
            let _ = app.emit(kinds::POLICY_EVENT, event);
        })
    })
    .await
}
#[tauri::command]
pub(crate) async fn detect_data_sources(
    request: Request<preview::Detect>,
) -> Result<Value, CommandError> {
    run_blocking_command("detect_data_sources", move || {
        worker_bridge::detect_data_sources_impl(request.0)
    })
    .await
}
#[tauri::command]
pub(crate) async fn preview_agent_sources(
    state: State<'_, SessionState>,
    request: Request<preview::PreviewRequest>,
    on_event: Channel<Value>,
) -> Result<preview::Started, CommandError> {
    let session = state.inner().clone();
    let mut frames = run_blocking_command("preview_agent_sources", move || {
        worker_bridge::preview_agent_sources_impl(&session, request.0)
    })
    .await?;
    let receipt = frames.started.clone();
    tauri::async_runtime::spawn(async move {
        while let Some(frame) = frames.next_frame().await {
            if on_event.send(frame).is_err() {
                break;
            }
        }
    });
    Ok(receipt)
}
#[tauri::command]
pub(crate) async fn cancel_agent_read(
    request: Request<preview::CancelRequest>,
) -> Result<Value, CommandError> {
    run_blocking_command("cancel_agent_read", move || {
        worker_bridge::cancel_agent_read_impl(request.0)
    })
    .await
}
