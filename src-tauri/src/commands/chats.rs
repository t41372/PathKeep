//! Conversation screen commands. Blocking database and decoder work runs on the worker pool.
#![cfg(not(test))]
use super::{blocking::run_blocking_command, data_kinds::Request};
use crate::{command_error::CommandError, session::SessionState, worker_bridge};
use serde_json::Value;
use tauri::{AppHandle, State};
use vault_worker::data_kinds::chats;
#[tauri::command]
pub(crate) async fn list_agent_conversations(
    state: State<'_, SessionState>,
    request: Request<chats::ListQuery>,
) -> Result<Value, CommandError> {
    let session = state.inner().clone();
    run_blocking_command("list_agent_conversations", move || {
        worker_bridge::list_agent_conversations_impl(&session, request.0)
    })
    .await
}
#[tauri::command]
pub(crate) async fn get_agent_conversation_counts(
    state: State<'_, SessionState>,
    request: Request<chats::ListQuery>,
) -> Result<Value, CommandError> {
    let session = state.inner().clone();
    run_blocking_command("get_agent_conversation_counts", move || {
        worker_bridge::get_agent_conversation_counts_impl(&session, request.0)
    })
    .await
}
#[tauri::command]
pub(crate) async fn get_agent_conversation(
    state: State<'_, SessionState>,
    request: Request<chats::Conversation>,
) -> Result<Value, CommandError> {
    let session = state.inner().clone();
    run_blocking_command("get_agent_conversation", move || {
        worker_bridge::get_agent_conversation_impl(&session, request.0)
    })
    .await
}
#[tauri::command]
pub(crate) async fn get_agent_conversation_info(
    state: State<'_, SessionState>,
    request: Request<chats::InfoRequest>,
) -> Result<Value, CommandError> {
    let session = state.inner().clone();
    run_blocking_command("get_agent_conversation_info", move || {
        worker_bridge::get_agent_conversation_info_impl(&session, request.0)
    })
    .await
}
#[tauri::command]
pub(crate) async fn read_agent_conversation_window(
    state: State<'_, SessionState>,
    request: Request<chats::WindowQuery>,
) -> Result<Value, CommandError> {
    let session = state.inner().clone();
    run_blocking_command("read_agent_conversation_window", move || {
        worker_bridge::read_agent_conversation_window_impl(&session, request.0)
    })
    .await
}
#[tauri::command]
pub(crate) async fn list_agent_conversation_branches(
    state: State<'_, SessionState>,
    request: Request<chats::BranchesQuery>,
) -> Result<Value, CommandError> {
    let session = state.inner().clone();
    run_blocking_command("list_agent_conversation_branches", move || {
        worker_bridge::list_agent_conversation_branches_impl(&session, request.0)
    })
    .await
}
#[tauri::command]
pub(crate) async fn read_agent_record(
    state: State<'_, SessionState>,
    request: Request<chats::RawQuery>,
) -> Result<Value, CommandError> {
    let session = state.inner().clone();
    run_blocking_command("read_agent_record", move || {
        worker_bridge::read_agent_record_impl(&session, request.0)
    })
    .await
}
#[tauri::command]
pub(crate) async fn read_agent_attachment(
    state: State<'_, SessionState>,
    request: Request<chats::AttachmentQuery>,
) -> Result<Value, CommandError> {
    let session = state.inner().clone();
    run_blocking_command("read_agent_attachment", move || {
        worker_bridge::read_agent_attachment_impl(&session, request.0)
    })
    .await
}
#[tauri::command]
pub(crate) async fn edit_agent_conversation(
    state: State<'_, SessionState>,
    app: AppHandle,
    request: Request<chats::Edit>,
) -> Result<Value, CommandError> {
    let session = state.inner().clone();
    run_blocking_command("edit_agent_conversation", move || {
        worker_bridge::edit_agent_conversation_impl(
            &session,
            request.0,
            super::data_kinds::desktop_events(app),
        )
    })
    .await
}

#[tauri::command]
pub(crate) async fn search_agent_conversations(
    state: State<'_, SessionState>,
    request: Request<chats::ScreenQuery>,
    on_event: tauri::ipc::Channel<Value>,
) -> Result<vault_worker::data_kinds::preview::Started, CommandError> {
    let session = state.inner().clone();
    let frames = run_blocking_command("search_agent_conversations", move || {
        worker_bridge::search_agent_conversations_impl(&session, request.0)
    })
    .await?;
    Ok(super::data_kinds::forward_frames(frames, on_event))
}

#[tauri::command]
pub(crate) async fn find_in_agent_conversation(
    state: State<'_, SessionState>,
    request: Request<chats::FindRequest>,
    on_event: tauri::ipc::Channel<Value>,
) -> Result<vault_worker::data_kinds::preview::Started, CommandError> {
    let session = state.inner().clone();
    let frames = run_blocking_command("find_in_agent_conversation", move || {
        worker_bridge::find_in_agent_conversation_impl(&session, request.0)
    })
    .await?;
    Ok(super::data_kinds::forward_frames(frames, on_event))
}

#[tauri::command]
pub(crate) async fn export_agent_conversation(
    app: AppHandle,
    state: State<'_, SessionState>,
    request: Request<chats::ConversationExport>,
) -> Result<Value, CommandError> {
    let session = state.inner().clone();
    run_blocking_command("export_agent_conversation", move || {
        worker_bridge::export_agent_conversation_impl(Some(&app), &session, request.0)
    })
    .await
}

#[tauri::command]
pub(crate) async fn get_agent_viewer_preferences(
    request: Request<vault_worker::data_kinds::Empty>,
) -> Result<Value, CommandError> {
    run_blocking_command("get_agent_viewer_preferences", move || {
        worker_bridge::get_agent_viewer_preferences_impl(request.0)
    })
    .await
}
#[tauri::command]
pub(crate) async fn set_agent_viewer_preferences(
    state: State<'_, SessionState>,
    app: AppHandle,
    request: Request<vault_worker::data_kinds::viewer::Patch>,
) -> Result<Value, CommandError> {
    let session = state.inner().clone();
    run_blocking_command("set_agent_viewer_preferences", move || {
        worker_bridge::set_agent_viewer_preferences_impl(
            &session,
            request.0,
            super::data_kinds::desktop_events(app),
        )
    })
    .await
}

#[tauri::command]
pub(crate) async fn query_agent_palette(
    state: State<'_, SessionState>,
    request: Request<chats::Palette>,
) -> Result<Value, CommandError> {
    let session = state.inner().clone();
    run_blocking_command("query_agent_palette", move || {
        worker_bridge::query_agent_palette_impl(&session, request.0)
    })
    .await
}

#[tauri::command]
pub(crate) async fn get_agent_semantic_index(
    state: State<'_, SessionState>,
    request: Request<vault_worker::data_kinds::Empty>,
) -> Result<Value, CommandError> {
    let session = state.inner().clone();
    run_blocking_command("get_agent_semantic_index", move || {
        worker_bridge::get_agent_semantic_index_impl(&session, request.0)
    })
    .await
}
#[tauri::command]
pub(crate) async fn control_agent_semantic_index(
    app: AppHandle,
    state: State<'_, SessionState>,
    request: Request<vault_worker::data_kinds::semantic::Control>,
) -> Result<Value, CommandError> {
    let session = state.inner().clone();
    run_blocking_command("control_agent_semantic_index", move || {
        worker_bridge::control_agent_semantic_index_impl(
            &session,
            request.0,
            super::data_kinds::desktop_events(app),
        )
    })
    .await
}
