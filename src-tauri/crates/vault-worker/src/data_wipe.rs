//! Worker side of "Delete all data".
//!
//! ## Responsibilities
//! - Refuse anything but the literal confirmation word, and refuse while App Lock is locked.
//! - Stop this process's background work before the databases go: queue jobs, queue workers,
//!   streaming chats and model downloads.
//! - Clear the archive key and AI provider keys from the system keychain.
//! - Finish a wipe that a crash cut short, before anything else touches the app root.
//!
//! ## Not responsible for
//! - Which files are removed, the preview, locking and the crash marker:
//!   `vault_core::archive::wipe` owns those, and its module docs list what is kept and why.
//! - The in-memory session key held by the desktop shell; the Tauri layer drops it.

use crate::{
    context::load_unlocked_config,
    intelligence::{cancel_model_download, running_background_workers},
};
use anyhow::Result;
use std::time::{Duration, Instant};
use vault_core::{AppConfig, ProjectPaths, WipePreview, WipeSecrets};
use vault_platform::{keyring_clear_database_key, keyring_clear_provider_api_key, keyring_status};

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
    Ok(preview)
}

/// Deletes all PathKeep data after the user typed [`WIPE_CONFIRMATION_WORD`].
///
/// Afterwards the app reports itself not initialized, so the shell routes to onboarding.
pub fn wipe_all_data(confirmation: &str, session_database_key: Option<&str>) -> Result<()> {
    if confirmation != WIPE_CONFIRMATION_WORD {
        anyhow::bail!(
            "Deleting all data needs the confirmation word {WIPE_CONFIRMATION_WORD}; nothing was deleted."
        );
    }
    let paths = vault_core::project_paths()?;
    let config = load_unlocked_config(&paths)?;
    let secrets = WipeSecrets::from_config(&config);
    vault_core::wipe_all_data(
        &paths,
        &secrets,
        || stop_background_work(&paths, Some((&config, session_database_key))),
        clear_keychain,
    )
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
    vault_core::finish_interrupted_data_wipe(
        &paths,
        || stop_background_work(&paths, None),
        clear_keychain,
    )
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

fn clear_keychain(secrets: &WipeSecrets) -> Result<()> {
    keyring_clear_database_key()?;
    for provider_id in &secrets.provider_ids {
        keyring_clear_provider_api_key(provider_id)?;
    }
    Ok(())
}
