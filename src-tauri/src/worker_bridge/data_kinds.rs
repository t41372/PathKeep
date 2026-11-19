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
    request: kinds::ListKinds,
) -> Result<kinds::KindList, CommandError> {
    surface_result(kinds::delivery::read(|| kinds::list_with(session, request)))
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
    session: &SessionState,
    request: kinds::SourcesPatch,
    emit: kinds::backups::Events,
) -> Result<kinds::Settings, CommandError> {
    let mut changed = None;
    let settings =
        surface_result(kinds::update_sources(session, request, |event| changed = Some(event)))?;
    if let Some(event) = changed {
        emit(kinds::POLICY_EVENT, serde_json::json!(event));
        if settings.policy.agent_history != vault_core::KindState::Off {
            emit(
                "pathkeep://agent-settings-changed",
                serde_json::json!({"settingsToken":settings.settings_token(),"domains":["sources"]}),
            );
        }
    }
    Ok(settings)
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

pub(crate) fn enable_agent_history_impl(
    session: &SessionState,
    request: kinds::backups::Enable,
    events: kinds::backups::Events,
) -> Result<Value, CommandError> {
    surface_result(kinds::backups::enable(session, request, events))
}

pub(crate) fn start_backup_impl(
    session: &SessionState,
    request: kinds::backups::Start,
    events: kinds::backups::Events,
) -> Result<Value, CommandError> {
    surface_result(kinds::backups::start(session, request, events))
}

pub(crate) fn list_archive_runs_impl(
    session: &SessionState,
    request: kinds::backups::ListRuns,
) -> Result<Value, CommandError> {
    surface_result(kinds::delivery::read(|| kinds::backups::list_runs(session, request)))
}

pub(crate) fn get_archive_run_impl(
    session: &SessionState,
    request: kinds::backups::GetRun,
) -> Result<Value, CommandError> {
    surface_result(kinds::delivery::read(|| kinds::backups::get_run(session, request)))
}

pub(crate) fn list_background_tasks_impl(
    session: &SessionState,
    request: kinds::backups::ListTasks,
) -> Result<Value, CommandError> {
    surface_result(kinds::delivery::read(|| kinds::backups::tasks(session, request)))
}

pub(crate) fn get_task_summary_impl(
    session: &SessionState,
    request: kinds::Empty,
) -> Result<Value, CommandError> {
    surface_result(kinds::delivery::read(|| kinds::backups::task_summary(session, request)))
}

pub(crate) fn control_background_task_impl(
    session: &SessionState,
    request: kinds::backups::Control,
    events: kinds::backups::Events,
) -> Result<Value, CommandError> {
    surface_result(kinds::backups::control(session, request, events))
}

pub(crate) fn list_agent_sources_impl(
    session: &SessionState,
    request: kinds::backups::ListSources,
) -> Result<Value, CommandError> {
    surface_result(kinds::delivery::read(|| kinds::backups::sources(session, request)))
}

pub(crate) fn get_agent_overview_impl(
    session: &SessionState,
    request: kinds::backups::Overview,
) -> Result<Value, CommandError> {
    surface_result(kinds::delivery::read(|| kinds::backups::overview(session, request)))
}

pub(crate) fn retry_agent_store_impl(
    session: &SessionState,
    _: kinds::Empty,
    emit: impl FnOnce(kinds::PolicyChange),
) -> Result<kinds::KindList, CommandError> {
    surface_result(kinds::delivery::read(|| kinds::retry_agent_store(session, emit)))
}
pub(crate) fn dismiss_agent_hint_impl(
    req: kinds::DismissHint,
    events: kinds::backups::Events,
) -> Result<kinds::HintAcknowledgement, CommandError> {
    surface_result(kinds::delivery::read(|| kinds::dismiss_hint(req, events)))
}
