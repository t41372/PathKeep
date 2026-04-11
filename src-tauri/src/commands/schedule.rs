#[cfg(not(test))]
use crate::{session::SessionState, worker_bridge};
#[cfg(not(test))]
use tauri::State;

#[cfg(not(test))]
#[tauri::command]
pub(crate) fn preview_schedule(
    platform: Option<String>,
) -> Result<vault_core::SchedulePlan, String> {
    worker_bridge::preview_schedule_impl(platform)
}

#[cfg(not(test))]
#[tauri::command]
pub(crate) fn apply_schedule(
    plan: vault_core::SchedulePlan,
) -> Result<vault_core::ApplyResult, String> {
    worker_bridge::apply_schedule_impl(plan)
}

#[cfg(not(test))]
#[tauri::command]
pub(crate) fn remove_schedule(
    plan: vault_core::SchedulePlan,
) -> Result<vault_core::ApplyResult, String> {
    worker_bridge::remove_schedule_impl(plan)
}

#[cfg(not(test))]
#[tauri::command]
pub(crate) fn schedule_status(
    platform: Option<String>,
    state: State<'_, SessionState>,
) -> Result<vault_core::ScheduleStatus, String> {
    worker_bridge::schedule_status_impl(platform, state.get_key().as_deref())
}
