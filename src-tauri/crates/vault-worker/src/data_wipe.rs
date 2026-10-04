//! Worker side of "Delete all data".
//!
//! ## Responsibilities
//! - Refuse anything but the literal confirmation word, and refuse while App Lock is locked.
//! - Stop this process's background work before the databases go: queue jobs, queue workers,
//!   streaming chats and model downloads.
//! - Clear the archive key and AI provider keys from the system keychain.
//! - Find an installed automatic backup (preview) and remove it through the scheduler's own remove
//!   path (execute). The plan is taken while the config still exists; the removal runs last.
//! - Finish a wipe that a crash cut short, before anything else touches the app root.
//!
//! ## Not responsible for
//! - Which files are removed, the preview, locking and the crash marker:
//!   `vault_core::archive::wipe` owns those, and its module docs list what is kept and why.
//! - The in-memory session key held by the desktop shell; the Tauri layer drops it.

use crate::{
    context::load_unlocked_config,
    intelligence::{cancel_model_download, running_background_workers},
    schedule::native_schedule_interval_hours,
};
use anyhow::{Context, Result};
use std::time::{Duration, Instant};
use vault_core::{
    AppConfig, ProjectPaths, SchedulePlan, WipeOutsideState, WipePreview, WipeReport,
};
use vault_platform::{
    ScheduleParameters, keyring_clear_database_key, keyring_clear_provider_api_key, keyring_status,
};

/// The word the user types to confirm. The frontend shows it; this is the check that counts.
pub const WIPE_CONFIRMATION_WORD: &str = "DELETE";

/// How long a wipe waits for queue workers to finish their current step. Workers check for stop
/// requests between steps, so this only runs out if one step takes longer; the files are removed
/// anyway and the worker exits at its next check because the config is gone.
const WORKER_STOP_TIMEOUT: Duration = Duration::from_secs(30);

/// Lists what "Delete all data" would remove.
pub fn preview_data_wipe(session_database_key: Option<&str>) -> Result<WipePreview> {
    let paths = vault_core::project_paths()?;
    let config = load_unlocked_config(&paths)?;
    let mut preview = vault_core::preview_data_wipe(&paths, &config, session_database_key)?;
    preview.clears_keychain = keyring_status().stored_secret;
    if let Some(installed) = installed_schedule(&paths, &config) {
        preview.removes_schedule = true;
        preview.schedule_items = installed.items;
    }
    Ok(preview)
}

/// Deletes all PathKeep data after the user typed [`WIPE_CONFIRMATION_WORD`].
///
/// Afterwards the app reports itself not initialized, so the shell routes to onboarding. An
/// installed automatic backup is removed last; if that fails the files are still gone and the
/// report carries the error.
pub fn wipe_all_data(confirmation: &str, session_database_key: Option<&str>) -> Result<WipeReport> {
    if confirmation != WIPE_CONFIRMATION_WORD {
        anyhow::bail!(
            "Deleting all data needs the confirmation word {WIPE_CONFIRMATION_WORD}; nothing was deleted."
        );
    }
    let paths = vault_core::project_paths()?;
    let config = load_unlocked_config(&paths)?;
    let schedule = installed_schedule(&paths, &config).map(|installed| installed.plan);
    let outside = WipeOutsideState::from_config(&config, schedule);
    let report = vault_core::wipe_all_data(
        &paths,
        &outside,
        || stop_background_work(&paths, Some((&config, session_database_key))),
        clear_keychain,
        |plan| remove_schedule(&paths, plan),
    )?;
    log_schedule_outcome(&report);
    Ok(report)
}

/// Finishes a wipe that was cut short. Returns whether there was one to finish.
///
/// Runs at launch and before archive initialization, so onboarding can never reopen a
/// half-deleted archive. The user confirmed the wipe when it started.
pub fn finish_interrupted_data_wipe() -> Result<bool> {
    let paths = vault_core::project_paths()?;
    if !vault_core::data_wipe_interrupted(&paths) {
        return Ok(false);
    }
    log::warn!(target: "pathkeep::data_wipe", "finishing a data wipe that was interrupted");
    let report = vault_core::finish_interrupted_data_wipe(
        &paths,
        || stop_background_work(&paths, None),
        clear_keychain,
        |plan| remove_schedule(&paths, plan),
    )?;
    if let Some(report) = &report {
        log_schedule_outcome(report);
    }
    Ok(report.is_some())
}

/// The installed automatic backup, as the scheduler sees it.
struct InstalledSchedule {
    /// The plan the scheduler's remove path takes.
    plan: SchedulePlan,
    /// What the scheduler reports installed, for the preview.
    items: Vec<String>,
}

/// Finds the automatic backup a wipe should remove.
///
/// `None` when nothing is installed, on Linux (setup there is manual), for a pre-rename install
/// (Backup's repair removes those) and when the scheduler cannot be asked: the wipe goes ahead,
/// and the preview does not promise a removal it will not attempt.
fn installed_schedule(paths: &ProjectPaths, config: &AppConfig) -> Option<InstalledSchedule> {
    let lookup = || -> Result<Option<InstalledSchedule>> {
        let executable = std::env::current_exe().context("resolving the PathKeep executable")?;
        let params = ScheduleParameters {
            due_after_hours: config.due_after_hours,
            check_interval_hours: native_schedule_interval_hours(config),
        };
        let status = vault_platform::schedule_status(None, &executable, paths, &params)?;
        if !matches!(status.install_state.as_str(), "installed" | "mismatch" | "permission-warning")
        {
            return Ok(None);
        }
        let plan = vault_platform::preview_schedule(None, &executable, paths, &params)?;
        let items = if status.detected_files.is_empty() {
            vec![plan.label.clone()]
        } else {
            status.detected_files
        };
        Ok(Some(InstalledSchedule { plan, items }))
    };
    lookup().unwrap_or_else(|error| {
        log::warn!(target: "pathkeep::data_wipe", "could not check the automatic backup: {error:#}");
        None
    })
}

/// Removes the automatic backup through the scheduler's own remove path (which also writes its
/// audit record). A step the scheduler marks as an error counts as a failure.
fn remove_schedule(paths: &ProjectPaths, plan: &SchedulePlan) -> Result<()> {
    let result = vault_platform::remove_schedule(plan, paths)?;
    if result.step_results.iter().any(|step| step.status == "error") {
        anyhow::bail!("{}", result.message);
    }
    Ok(())
}

fn log_schedule_outcome(report: &WipeReport) {
    if let Some(error) = &report.schedule_error {
        log::warn!(
            target: "pathkeep::data_wipe",
            "the data is deleted but the automatic backup is still installed: {error}"
        );
    } else if report.schedule_removed {
        log::info!(target: "pathkeep::data_wipe", "removed the automatic backup");
    }
}

/// Stops background work that writes under the app root. `archive` is the config and key the
/// archive was opened with, when it is still there to ask running jobs to stop.
fn stop_background_work(
    paths: &ProjectPaths,
    archive: Option<(&AppConfig, Option<&str>)>,
) -> Result<()> {
    if let Some((config, key)) = archive
        && let Err(error) = vault_core::request_running_jobs_stop(paths, config, key)
    {
        // Workers still stop at their next config check; this only makes it sooner.
        log::warn!(target: "pathkeep::data_wipe", "could not ask running jobs to stop: {error:#}");
    }
    cancel_model_download();
    vault_core::request_cancel_all_ai_chat_runs();

    let deadline = Instant::now() + WORKER_STOP_TIMEOUT;
    while running_background_workers() > 0 {
        if Instant::now() >= deadline {
            log::warn!(
                target: "pathkeep::data_wipe",
                "{} background workers still running after {WORKER_STOP_TIMEOUT:?}; deleting anyway",
                running_background_workers()
            );
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    Ok(())
}

fn clear_keychain(outside: &WipeOutsideState) -> Result<()> {
    keyring_clear_database_key()?;
    for provider_id in &outside.provider_ids {
        keyring_clear_provider_api_key(provider_id)?;
    }
    Ok(())
}
