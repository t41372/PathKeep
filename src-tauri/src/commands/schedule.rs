//! Tauri commands for backup schedule preview/apply/remove/status.

#[cfg(not(test))]
use crate::{session::SessionState, worker_bridge};
#[cfg(not(test))]
use tauri::State;

#[cfg(not(test))]
#[tauri::command]
/// Previews the native scheduler plan for one platform.
pub(crate) fn preview_schedule(
    platform: Option<String>,
) -> Result<vault_core::SchedulePlan, String> {
    worker_bridge::preview_schedule_impl(platform)
}

#[cfg(not(test))]
#[tauri::command]
/// Applies one previously previewed native schedule plan.
pub(crate) fn apply_schedule(
    plan: vault_core::SchedulePlan,
) -> Result<vault_core::ApplyResult, String> {
    worker_bridge::apply_schedule_impl(plan)
}

#[cfg(not(test))]
#[tauri::command]
/// Removes one previously previewed native schedule plan.
pub(crate) fn remove_schedule(
    plan: vault_core::SchedulePlan,
) -> Result<vault_core::ApplyResult, String> {
    worker_bridge::remove_schedule_impl(plan)
}

#[cfg(not(test))]
#[tauri::command]
/// Loads install/due-state information for the selected scheduler platform.
pub(crate) fn schedule_status(
    platform: Option<String>,
    state: State<'_, SessionState>,
) -> Result<vault_core::ScheduleStatus, String> {
    worker_bridge::schedule_status_impl(platform, state.get_key().as_deref())
}
