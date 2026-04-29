//! Worker-bridge helpers for native backup schedule flows.

use vault_core::SchedulePlan;

use super::worker_result;

/// Previews the native schedule plan for one platform.
pub(crate) fn preview_schedule_impl(platform: Option<String>) -> Result<SchedulePlan, String> {
    worker_result(vault_worker::preview_schedule_plan(platform.as_deref(), None))
}

/// Applies one previously previewed native schedule plan.
pub(crate) fn apply_schedule_impl(plan: SchedulePlan) -> Result<vault_core::ApplyResult, String> {
    worker_result(vault_worker::apply_schedule_plan(&plan))
}

#[cfg_attr(test, allow(dead_code))]
/// Removes one previously previewed native schedule plan.
pub(crate) fn remove_schedule_impl(plan: SchedulePlan) -> Result<vault_core::ApplyResult, String> {
    worker_result(vault_worker::remove_schedule_plan(&plan))
}

/// Repairs known scheduler conflicts after explicit user confirmation.
#[cfg_attr(test, allow(dead_code))]
pub(crate) fn repair_schedule_impl(plan: SchedulePlan) -> Result<vault_core::ApplyResult, String> {
    worker_result(vault_worker::repair_schedule_plan(&plan))
}

/// Loads status for the selected native scheduler implementation.
pub(crate) fn schedule_status_impl(
    platform: Option<String>,
    session_database_key: Option<&str>,
) -> Result<vault_core::ScheduleStatus, String> {
    worker_result(vault_worker::schedule_status(session_database_key, platform.as_deref(), None))
}
