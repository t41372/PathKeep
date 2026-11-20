//! Interactive policy snapshots and metadata preview for the source picker.
//! The owner worker handles admission, storage, enumeration and cancellation.
#![cfg(not(test))]
use super::blocking::run_blocking_command;
use crate::{command_error::CommandError, session::SessionState, worker_bridge};
use serde_json::Value;
use tauri::{AppHandle, Emitter, State, ipc::Channel};
use vault_worker::data_kinds::{self as kinds, preview};

pub(super) fn desktop_events(app: AppHandle) -> kinds::backups::Events {
    std::sync::Arc::new(move |name, event| {
        let _ = app.emit(name, event);
    })
}

/// Preserve the domain error envelope even when the request wrapper is missing.
pub(crate) struct Request<T>(pub(super) T);
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
    request: Request<kinds::ListKinds>,
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
    state: State<'_, SessionState>,
    app: AppHandle,
    request: Request<kinds::SourcesPatch>,
) -> Result<kinds::Settings, CommandError> {
    let session = state.inner().clone();
    run_blocking_command("update_agent_sources", move || {
        worker_bridge::update_agent_sources_impl(&session, request.0, desktop_events(app))
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
    let frames = run_blocking_command("preview_agent_sources", move || {
        worker_bridge::preview_agent_sources_impl(&session, request.0)
    })
    .await?;
    Ok(forward_frames(frames, on_event))
}
pub(super) fn forward_frames(
    mut frames: preview::Frames,
    on_event: Channel<Value>,
) -> preview::Started {
    let receipt = frames.started.clone();
    tauri::async_runtime::spawn(async move {
        while let Some(frame) = frames.next_frame().await {
            if on_event.send(frame).is_err() {
                break;
            }
        }
    });
    receipt
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

#[tauri::command]
pub(crate) async fn enable_agent_history(
    state: State<'_, SessionState>,
    app: AppHandle,
    request: Request<kinds::backups::Enable>,
) -> Result<Value, CommandError> {
    let session = state.inner().clone();
    run_blocking_command("enable_agent_history", move || {
        worker_bridge::enable_agent_history_impl(&session, request.0, desktop_events(app))
    })
    .await
}

#[tauri::command]
pub(crate) async fn start_backup(
    state: State<'_, SessionState>,
    app: AppHandle,
    request: Request<kinds::backups::Start>,
) -> Result<Value, CommandError> {
    let session = state.inner().clone();
    run_blocking_command("start_backup", move || {
        worker_bridge::start_backup_impl(&session, request.0, desktop_events(app))
    })
    .await
}

#[tauri::command]
pub(crate) async fn list_archive_runs(
    state: State<'_, SessionState>,
    request: Request<kinds::backups::ListRuns>,
) -> Result<Value, CommandError> {
    let session = state.inner().clone();
    run_blocking_command("list_archive_runs", move || {
        worker_bridge::list_archive_runs_impl(&session, request.0)
    })
    .await
}

#[tauri::command]
pub(crate) async fn get_archive_run(
    state: State<'_, SessionState>,
    request: Request<kinds::backups::GetRun>,
) -> Result<Value, CommandError> {
    let session = state.inner().clone();
    run_blocking_command("get_archive_run", move || {
        worker_bridge::get_archive_run_impl(&session, request.0)
    })
    .await
}

#[tauri::command]
pub(crate) async fn list_background_tasks(
    state: State<'_, SessionState>,
    request: Request<kinds::backups::ListTasks>,
) -> Result<Value, CommandError> {
    let session = state.inner().clone();
    run_blocking_command("list_background_tasks", move || {
        worker_bridge::list_background_tasks_impl(&session, request.0)
    })
    .await
}

#[tauri::command]
pub(crate) async fn get_task_summary(
    state: State<'_, SessionState>,
    request: Request<kinds::Empty>,
) -> Result<Value, CommandError> {
    let session = state.inner().clone();
    run_blocking_command("get_task_summary", move || {
        worker_bridge::get_task_summary_impl(&session, request.0)
    })
    .await
}

#[tauri::command]
pub(crate) async fn control_background_task(
    state: State<'_, SessionState>,
    app: AppHandle,
    request: Request<kinds::backups::Control>,
) -> Result<Value, CommandError> {
    let session = state.inner().clone();
    run_blocking_command("control_background_task", move || {
        worker_bridge::control_background_task_impl(&session, request.0, desktop_events(app))
    })
    .await
}

#[tauri::command]
pub(crate) async fn list_agent_sources(
    state: State<'_, SessionState>,
    request: Request<kinds::backups::ListSources>,
) -> Result<Value, CommandError> {
    let session = state.inner().clone();
    run_blocking_command("list_agent_sources", move || {
        worker_bridge::list_agent_sources_impl(&session, request.0)
    })
    .await
}

#[tauri::command]
pub(crate) async fn get_agent_overview(
    state: State<'_, SessionState>,
    request: Request<kinds::backups::Overview>,
) -> Result<Value, CommandError> {
    let session = state.inner().clone();
    run_blocking_command("get_agent_overview", move || {
        worker_bridge::get_agent_overview_impl(&session, request.0)
    })
    .await
}

#[tauri::command]
pub(crate) async fn retry_agent_store(
    state: State<'_, SessionState>,
    app: AppHandle,
    request: Request<kinds::Empty>,
) -> Result<kinds::KindList, CommandError> {
    let session = state.inner().clone();
    run_blocking_command("retry_agent_store", move || {
        worker_bridge::retry_agent_store_impl(&session, request.0, |event| {
            let _ = app.emit(kinds::POLICY_EVENT, event);
        })
    })
    .await
}
#[tauri::command]
pub(crate) async fn dismiss_agent_hint(
    app: AppHandle,
    request: Request<kinds::DismissHint>,
) -> Result<kinds::HintAcknowledgement, CommandError> {
    run_blocking_command("dismiss_agent_hint", move || {
        worker_bridge::dismiss_agent_hint_impl(request.0, desktop_events(app))
    })
    .await
}

#[tauri::command]
pub(crate) async fn preview_agent_history_repair(
    state: State<'_, SessionState>,
    request: Request<kinds::repair::Preview>,
) -> Result<vault_core::maintenance::ConversationRepairPreview, CommandError> {
    let session = state.inner().clone();
    run_blocking_command("preview_agent_history_repair", move || {
        worker_bridge::preview_agent_history_repair_impl(&session, request.0)
    })
    .await
}

#[tauri::command]
pub(crate) async fn repair_agent_history(
    state: State<'_, SessionState>,
    app: AppHandle,
    request: Request<kinds::repair::Execute>,
) -> Result<kinds::repair::Repaired, CommandError> {
    let session = state.inner().clone();
    run_blocking_command("repair_agent_history", move || {
        worker_bridge::repair_agent_history_impl(&session, request.0, |event| {
            let _ = app.emit(kinds::POLICY_EVENT, event);
        })
    })
    .await
}

#[tauri::command]
pub(crate) async fn preview_archive_safety_copy(
    state: State<'_, SessionState>,
    request: Request<kinds::Empty>,
) -> Result<kinds::repair::SafetyCopyPreview, CommandError> {
    let session = state.inner().clone();
    run_blocking_command("preview_archive_safety_copy", move || {
        worker_bridge::preview_archive_safety_copy_impl(&session, request.0)
    })
    .await
}
#[tauri::command]
pub(crate) async fn create_archive_safety_copy(
    state: State<'_, SessionState>,
    request: Request<kinds::repair::SaveCopy>,
) -> Result<vault_core::RecoverySnapshot, CommandError> {
    let session = state.inner().clone();
    run_blocking_command("create_archive_safety_copy", move || {
        worker_bridge::create_archive_safety_copy_impl(&session, request.0)
    })
    .await
}

#[tauri::command]
pub(crate) async fn save_onboarding_choices(
    request: Request<vault_core::OnboardingChoices>,
) -> Result<vault_core::OnboardingChoices, CommandError> {
    run_blocking_command("save_onboarding_choices", move || {
        worker_bridge::save_onboarding_choices_impl(request.0)
    })
    .await
}
