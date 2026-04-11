use vault_core::SchedulePlan;

use super::worker_result;

pub(crate) fn preview_schedule_impl(platform: Option<String>) -> Result<SchedulePlan, String> {
    worker_result(vault_worker::preview_schedule_plan(platform.as_deref(), None))
}

pub(crate) fn apply_schedule_impl(plan: SchedulePlan) -> Result<vault_core::ApplyResult, String> {
    worker_result(vault_worker::apply_schedule_plan(&plan))
}

#[cfg_attr(test, allow(dead_code))]
pub(crate) fn remove_schedule_impl(plan: SchedulePlan) -> Result<vault_core::ApplyResult, String> {
    worker_result(vault_worker::remove_schedule_plan(&plan))
}

pub(crate) fn schedule_status_impl(
    platform: Option<String>,
    session_database_key: Option<&str>,
) -> Result<vault_core::ScheduleStatus, String> {
    worker_result(vault_worker::schedule_status(session_database_key, platform.as_deref(), None))
}
