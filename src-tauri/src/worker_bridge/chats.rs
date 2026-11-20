//! Share the conversation command contract between native and development transports.
use crate::{command_error::CommandError, session::SessionState};
use serde_json::Value;
use vault_worker::data_kinds::{backups::Events, chats};
pub(crate) fn list_agent_conversations_impl(
    session: &SessionState,
    request: chats::ListQuery,
) -> Result<Value, CommandError> {
    super::data_kinds::surface_result(chats::list(session, request))
}
pub(crate) fn get_agent_conversation_counts_impl(
    session: &SessionState,
    request: chats::ListQuery,
) -> Result<Value, CommandError> {
    super::data_kinds::surface_result(chats::counts(session, request))
}
pub(crate) fn get_agent_conversation_impl(
    session: &SessionState,
    request: chats::Conversation,
) -> Result<Value, CommandError> {
    super::data_kinds::surface_result(chats::header(session, request))
}
pub(crate) fn get_agent_conversation_info_impl(
    session: &SessionState,
    request: chats::InfoRequest,
) -> Result<Value, CommandError> {
    super::data_kinds::surface_result(chats::info(session, request))
}
pub(crate) fn read_agent_conversation_window_impl(
    session: &SessionState,
    request: chats::WindowQuery,
) -> Result<Value, CommandError> {
    super::data_kinds::surface_result(chats::window(session, request))
}
pub(crate) fn list_agent_conversation_branches_impl(
    session: &SessionState,
    request: chats::BranchesQuery,
) -> Result<Value, CommandError> {
    super::data_kinds::surface_result(chats::branches(session, request))
}
pub(crate) fn read_agent_record_impl(
    session: &SessionState,
    request: chats::RawQuery,
) -> Result<Value, CommandError> {
    super::data_kinds::surface_result(chats::raw(session, request))
}
pub(crate) fn read_agent_attachment_impl(
    session: &SessionState,
    request: chats::AttachmentQuery,
) -> Result<Value, CommandError> {
    super::data_kinds::surface_result(chats::attachment(session, request))
}
pub(crate) fn edit_agent_conversation_impl(
    session: &SessionState,
    request: chats::Edit,
    events: Events,
) -> Result<Value, CommandError> {
    super::data_kinds::surface_result(chats::edit(session, request, events))
}

pub(crate) fn search_agent_conversations_impl(
    session: &SessionState,
    request: chats::ScreenQuery,
) -> Result<vault_worker::data_kinds::preview::Frames, CommandError> {
    super::data_kinds::surface_result(chats::search(session, request))
}

pub(crate) fn find_in_agent_conversation_impl(
    session: &SessionState,
    request: chats::FindRequest,
) -> Result<vault_worker::data_kinds::preview::Frames, CommandError> {
    super::data_kinds::surface_result(chats::find(session, request))
}

pub(crate) fn export_agent_conversation_impl<R: tauri::Runtime>(
    app: Option<&tauri::AppHandle<R>>,
    session: &SessionState,
    q: vault_worker::data_kinds::exports::Request,
    events: Events,
) -> Result<Value, CommandError> {
    let job = super::data_kinds::surface_result(vault_worker::data_kinds::exports::Job::begin(
        session,
        &q.conversation.id,
        q.export_id,
        events,
    ))?;
    let q = q.conversation;
    let ext = q.format.extension();
    let path = super::save_path(app, &format!("conversation.{ext}"), (ext, &[ext]))?;
    super::data_kinds::surface_result(job.check())?;
    let Some(path) = path else { return Ok(Value::Null) };
    super::data_kinds::surface_result(job.write(session, q, &path))
}

pub(crate) fn get_agent_viewer_preferences_impl(
    q: vault_worker::data_kinds::Empty,
) -> Result<Value, CommandError> {
    super::data_kinds::surface_result(vault_worker::data_kinds::viewer::get(q))
}
pub(crate) fn set_agent_viewer_preferences_impl(
    session: &SessionState,
    q: vault_worker::data_kinds::viewer::Patch,
    events: Events,
) -> Result<Value, CommandError> {
    super::data_kinds::surface_result(vault_worker::data_kinds::viewer::set(session, q, events))
}

pub(crate) fn query_agent_palette_impl(
    session: &SessionState,
    request: chats::Palette,
) -> Result<Value, CommandError> {
    super::data_kinds::surface_result(chats::palette(session, request))
}

/// Both transports ask the platform to select the same validated source file.
pub(crate) fn reveal_file_impl(path: String) -> Result<String, CommandError> {
    Ok(vault_platform::reveal_file(path)?)
}

pub(crate) fn get_agent_semantic_index_impl(
    s: &SessionState,
    q: vault_worker::data_kinds::Empty,
) -> Result<Value, CommandError> {
    super::data_kinds::surface_result(vault_worker::data_kinds::semantic::status(s, q))
}
pub(crate) fn control_agent_semantic_index_impl(
    s: &SessionState,
    q: vault_worker::data_kinds::semantic::Control,
    events: Events,
) -> Result<Value, CommandError> {
    super::data_kinds::surface_result(vault_worker::data_kinds::semantic::control(s, q, events))
}

pub(crate) fn cancel_agent_export_impl(
    q: vault_worker::data_kinds::exports::Cancel,
) -> Result<Value, CommandError> {
    super::data_kinds::surface_result(vault_worker::data_kinds::exports::cancel(q))
}
