//! Schedule worker flows.
//!
//! These helpers bridge the schedule UI/CLI surface to `vault-platform`'s
//! native scheduler adapters while keeping the product contract centered on
//! preview/manual/execute/verify.

use crate::context::load_unlocked_config;
use anyhow::{Context, Result};
use std::path::PathBuf;
use vault_core::{SchedulePlan, ScheduleStatus, load_config};
use vault_platform::{
    ScheduleParameters, apply_schedule, preview_schedule, remove_schedule,
    schedule_status as detect_schedule_status,
};

/// Builds a platform-specific schedule plan without installing anything.
pub fn preview_schedule_plan(
    platform: Option<&str>,
    executable_path: Option<PathBuf>,
) -> Result<SchedulePlan> {
    let paths = vault_core::project_paths()?;
    let config = load_config(&paths)?;
    let executable = executable_path
        .or_else(|| std::env::current_exe().ok())
        .context("resolving executable path for schedule preview")?;
    preview_schedule(
        platform,
        executable.as_path(),
        &paths,
        &ScheduleParameters {
            due_after_hours: config.due_after_hours,
            check_interval_hours: config.schedule_check_interval_hours,
        },
    )
}

/// Applies a native schedule plan.
pub fn apply_schedule_plan(plan: &SchedulePlan) -> Result<vault_core::ApplyResult> {
    let paths = vault_core::project_paths()?;
    apply_schedule(plan, &paths)
}

/// Removes a previously applied native schedule plan.
pub fn remove_schedule_plan(plan: &SchedulePlan) -> Result<vault_core::ApplyResult> {
    let paths = vault_core::project_paths()?;
    remove_schedule(plan, &paths)
}

/// Loads the current schedule status and annotates it with the last successful backup.
pub fn schedule_status(
    session_database_key: Option<&str>,
    platform: Option<&str>,
    executable_path: Option<PathBuf>,
) -> Result<ScheduleStatus> {
    let paths = vault_core::project_paths()?;
    let config = load_unlocked_config(&paths)?;
    let executable = executable_path
        .or_else(|| std::env::current_exe().ok())
        .context("resolving executable path for schedule status")?;
    let mut status = detect_schedule_status(
        platform,
        executable.as_path(),
        &paths,
        &ScheduleParameters {
            due_after_hours: config.due_after_hours,
            check_interval_hours: config.schedule_check_interval_hours,
        },
    )?;
    status.last_successful_backup_at =
        vault_core::archive_status(&paths, &config, session_database_key)?.last_successful_backup_at;
    Ok(status)
}
