//! Tauri commands for backup schedule preview/apply/remove/status.
//!
//! Applying, removing, repairing, and reading schedule status all shell out to the native scheduler
//! (launchd / Task Scheduler / cron) or read disk state — blocking I/O that runs on the blocking
//! thread pool so the WebView thread keeps painting. Preview is computed off the same path for
//! consistency and to keep any future I/O off the UI thread.

#[cfg(not(test))]
use super::blocking::run_blocking_command;
#[cfg(not(test))]
use crate::{session::SessionState, worker_bridge};
#[cfg(not(test))]
use tauri::State;

#[cfg(not(test))]
#[tauri::command]
/// Previews the native scheduler plan for one platform, off the UI thread.
pub(crate) async fn preview_schedule(
    platform: Option<String>,
) -> Result<vault_core::SchedulePlan, String> {
    run_blocking_command("preview_schedule", move || worker_bridge::preview_schedule_impl(platform))
        .await
}

#[cfg(not(test))]
#[tauri::command]
/// Applies one previously previewed native schedule plan, off the UI thread.
pub(crate) async fn apply_schedule(
    plan: vault_core::SchedulePlan,
) -> Result<vault_core::ApplyResult, String> {
    run_blocking_command("apply_schedule", move || worker_bridge::apply_schedule_impl(plan)).await
}

#[cfg(not(test))]
#[tauri::command]
/// Removes one previously previewed native schedule plan, off the UI thread.
pub(crate) async fn remove_schedule(
    plan: vault_core::SchedulePlan,
) -> Result<vault_core::ApplyResult, String> {
    run_blocking_command("remove_schedule", move || worker_bridge::remove_schedule_impl(plan)).await
}

#[cfg(not(test))]
#[tauri::command]
/// Repairs known scheduler conflicts after explicit user confirmation, off the UI thread.
pub(crate) async fn repair_schedule(
    plan: vault_core::SchedulePlan,
) -> Result<vault_core::ApplyResult, String> {
    run_blocking_command("repair_schedule", move || worker_bridge::repair_schedule_impl(plan)).await
}

#[cfg(not(test))]
#[tauri::command]
/// Loads install/due-state information for the selected scheduler platform, off the UI thread.
pub(crate) async fn schedule_status(
    platform: Option<String>,
    state: State<'_, SessionState>,
) -> Result<vault_core::ScheduleStatus, String> {
    let key = state.get_key();
    run_blocking_command("schedule_status", move || {
        worker_bridge::schedule_status_impl(platform, key.as_deref())
    })
    .await
}
