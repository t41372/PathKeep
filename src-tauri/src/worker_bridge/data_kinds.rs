//! Preserves typed policy and preview refusals across both desktop transports.
//! Serialization and event delivery belong to the calling transport.
use crate::command_error::CommandError;
use vault_worker::data_kinds::Refusal;

/// Preserve existing error classification for legacy settings commands.
pub(super) fn settings_result<T>(result: anyhow::Result<T>) -> Result<T, CommandError> {
    match result {
        Err(error) if error.is::<Refusal>() => surface_result(Err(error)),
        other => super::worker_result(other),
    }
}

pub(crate) fn surface_result<T>(result: anyhow::Result<T>) -> Result<T, CommandError> {
    result.map_err(|error| {
        let mut envelope = CommandError::classified(format!("{error:#}"));
        if let Some(reason) = error.downcast_ref::<Refusal>() {
            envelope.code = Some(reason.code().into());
            if matches!(reason, Refusal::Locked) {
                envelope.action_hint = Some("unlock".into());
                envelope.retry_hint = Some("retry-after-action".into());
            }
        } else if envelope.code.is_none() {
            envelope.code = Some("kind-unavailable".into())
        }
        envelope
    })
}

use crate::session::SessionState;
use serde_json::Value;
use vault_worker::data_kinds::{self as kinds, preview};

pub(crate) fn list_data_kinds_impl(
    session: &SessionState,
    _: kinds::Empty,
) -> Result<kinds::KindList, CommandError> {
    surface_result(kinds::list(session))
}
pub(crate) fn get_agent_settings_impl(_: kinds::Empty) -> Result<kinds::Settings, CommandError> {
    surface_result(kinds::settings())
}
pub(crate) fn set_data_kind_state_impl(
    session: &SessionState,
    request: kinds::SetState,
    emit: impl FnOnce(kinds::PolicyChange),
) -> Result<kinds::Transition, CommandError> {
    surface_result(kinds::set_state(session, request, emit))
}
pub(crate) fn update_agent_sources_impl(
    request: kinds::SourcesPatch,
    emit: impl FnOnce(kinds::PolicyChange),
) -> Result<kinds::Settings, CommandError> {
    surface_result(kinds::update_sources(request, emit))
}
pub(crate) fn detect_data_sources_impl(request: preview::Detect) -> Result<Value, CommandError> {
    surface_result(preview::detect(request))
}
pub(crate) fn preview_agent_sources_impl(
    session: &SessionState,
    request: preview::PreviewRequest,
) -> Result<preview::Frames, CommandError> {
    surface_result(preview::start(session, request))
}
pub(crate) fn cancel_agent_read_impl(
    request: preview::CancelRequest,
) -> Result<Value, CommandError> {
    surface_result(preview::cancel(request))
}
