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
