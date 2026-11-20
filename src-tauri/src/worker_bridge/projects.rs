//! Adapt saved-project queries and archive edits for both desktop transports.
use crate::{command_error::CommandError, session::SessionState};
use serde_json::Value;
use vault_worker::data_kinds::{self as kinds, projects};

pub(crate) fn list_agent_projects_impl(
    s: &SessionState,
    request: projects::ProjectQuery,
) -> Result<Value, CommandError> {
    super::data_kinds::surface_result(projects::list(s, request))
}

pub(crate) fn get_agent_project_impl(
    s: &SessionState,
    request: kinds::chats::Conversation,
) -> Result<Value, CommandError> {
    super::data_kinds::surface_result(projects::get(s, request))
}

pub(crate) fn list_agent_project_suggestions_impl(
    s: &SessionState,
    request: projects::Suggestions,
) -> Result<Value, CommandError> {
    super::data_kinds::surface_result(projects::suggestions(s, request))
}

pub(crate) fn edit_agent_project_impl(
    s: &SessionState,
    request: projects::ProjectEdit,
    events: kinds::backups::Events,
) -> Result<Value, CommandError> {
    super::data_kinds::surface_result(projects::edit(s, request, events))
}
