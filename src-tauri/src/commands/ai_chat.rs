//! Tauri commands for streaming external LLM chat (W-AI-1).
//!
//! ## Responsibilities
//! - start a streaming chat run (`ai_chat_send`) and bridge each chunk to the
//!   `pathkeep://ai-stream` Tauri event via `AppHandle::emit`
//! - cancel a live streaming run (`ai_chat_cancel`)
//!
//! ## Not responsible for
//! - resolving providers, driving the stream, or owning the run registry (worker + vault-core)
//! - chat UI or conversation persistence (W-AI-2 / W-AI-3)
//!
//! ## Performance notes
//! `ai_chat_send` returns the run id quickly; the actual token streaming happens on a worker
//! thread, so the UI thread is never blocked. Events are emitted as they arrive.

#[cfg(not(test))]
use super::blocking::run_blocking_command;
#[cfg(not(test))]
use crate::{session::SessionState, worker_bridge};
#[cfg(not(test))]
use tauri::{AppHandle, Emitter, State};

#[cfg(not(test))]
#[tauri::command]
/// Starts a streaming chat run and returns its run id; chunks arrive on `pathkeep://ai-stream`.
pub(crate) async fn ai_chat_send(
    app: AppHandle,
    request: vault_core::AiChatSendRequest,
    state: State<'_, SessionState>,
) -> Result<vault_core::AiChatSendAck, String> {
    let session_database_key = state.get_key();
    run_blocking_command("ai_chat_send", move || {
        worker_bridge::ai_chat_send_impl(request, session_database_key.as_deref(), move |event| {
            let _ = app.emit(vault_core::AI_CHAT_STREAM_EVENT, &event);
        })
    })
    .await
}

#[cfg(not(test))]
#[tauri::command]
/// Requests cooperative cancellation of one live streaming chat run.
pub(crate) async fn ai_chat_cancel(
    run_id: String,
    state: State<'_, SessionState>,
) -> Result<vault_core::AiChatCancelResult, String> {
    let session_database_key = state.get_key();
    run_blocking_command("ai_chat_cancel", move || {
        worker_bridge::ai_chat_cancel_impl(run_id, session_database_key.as_deref())
    })
    .await
}
