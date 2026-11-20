//! Shared transport adapter for the Agent Insights report and viewer settings.
use crate::{command_error::CommandError, session::SessionState};
use serde_json::Value;
use vault_worker::data_kinds as kinds;

pub(crate) fn get_agent_insights_impl(
    s: &SessionState,
    request: kinds::insights::Query,
) -> Result<kinds::preview::Frames, CommandError> {
    super::data_kinds::surface_result(kinds::insights::start(s, request))
}

pub(crate) fn get_agent_insights_preferences_impl(
    s: &SessionState,
    request: kinds::Empty,
) -> Result<Value, CommandError> {
    super::data_kinds::surface_result(kinds::insights::preferences(s, request))
}

pub(crate) fn set_agent_insights_preferences_impl(
    s: &SessionState,
    request: kinds::insights::PreferencePatch,
    events: kinds::backups::Events,
) -> Result<Value, CommandError> {
    super::data_kinds::surface_result(kinds::insights::update(s, request, events))
}
