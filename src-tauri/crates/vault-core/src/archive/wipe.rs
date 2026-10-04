//! "Delete all data": the fixed list of what PathKeep removes, a preview of it, and the removal.
//!
//! ## Responsibilities
//! - Name every file and folder under the app root that holds the user's history or something
//!   derived from it.
//! - Preview them with their sizes and the archive's visit count.
//! - Remove them under the archive write lock, with an on-disk marker so a wipe cut short by a
//!   crash or power loss finishes on the next launch instead of leaving half an archive behind.
//!
//! ## Not responsible for
//! - The confirmation word, keychain entries, stopping background workers and the in-memory
//!   session key. `vault-worker` owns those and passes the platform steps in as closures.
//! - The user's browser profiles. Nothing here reads a browser path: every target is a fixed path
//!   under [`ProjectPaths::app_root`].
//!
//! ## What is removed
//! - Everything in `archive/` except the write-lock sentinel: the archive and source-evidence
//!   databases with their WAL/SHM files, import backstops (`*.bak-*`) and rekey/restore/import
//!   markers.
//! - `derived/` (search, intelligence, AI vectors, agent conversations), `sidecars/`,
//!   `raw-snapshots/`, `staging/`, `quarantine/`, `audit/` (run ledger and manifests), `exports/`,
//!   `models/` (downloaded embedding models) and `integrations/`.
//! - Import backstops in the app root, the App Lock state and passcode files, the Stronghold vault
//!   and its salt, and `config.json`.
//!
//! ## What stays, and why
//! - `logs/` and `diagnostics/`. The log plugin keeps `rust.log` open, and if a wipe fails the logs
//!   are the only record of why. They hold no visit rows.
//! - `schedule/` and an installed OS backup task. Removing a LaunchAgent or scheduled task is its
//!   own preview-and-apply flow, and test and dev runs share the OS scheduler with the user's real
//!   install. With no config, a scheduled run stops at "archive has not been initialized".
//! - `archive/.pk-archive-write.lock`. Other processes lock that file's inode; deleting it would
//!   let a second process lock a new file while the first still holds the old one.
//!
//! ## Ways this can go wrong, and what guards each
//! 1. A browser profile gets touched. Targets come only from `ProjectPaths`; the test runs a real
//!    backup from a fixture profile and checks those files are byte-identical afterwards.
//! 2. A backup, import or rekey writes while files are being deleted. The wipe holds the
//!    in-process gate and the cross-process write lock for its whole run, so a scheduled backup
//!    that starts meanwhile defers and an in-flight one finishes first.
//! 3. Background workers keep writing and recreate an empty archive. `config.json` goes first, so
//!    every worker loop stops at its next check, and `quiesce` waits for running workers before
//!    anything else is deleted.
//! 4. A crash midway leaves part of the archive, and onboarding reopens it. The marker is written
//!    before anything is deleted and removed last; launch and archive initialization finish an
//!    interrupted wipe first.
//! 5. A symlinked folder (say `models/` on another disk) is followed and its target emptied.
//!    Symlinks are unlinked, never followed.
//! 6. A second wipe fails because everything is already gone. Missing targets are skipped.
//! 7. Stale in-process caches make the next archive in the same process skip its bootstrap. Each
//!    cache already re-checks that its file exists (or has its schema table) before trusting
//!    itself; the test builds, backs up and searches a new archive in the same process.
//!
//! ## Performance notes
//! - The preview takes the visit count from the last backup's cached totals; it only counts rows
//!   when no successful run has cached them. Folder sizes are a metadata walk.

use super::{
    count_visible_archive_totals, open_archive_connection, open_intelligence_connection,
    read_models::load_cached_archive_totals,
    write_lock::{ARCHIVE_WRITE_LOCK_FILE, ArchiveOpGate, ArchiveWriteLock},
};
use crate::{
    config::ProjectPaths,
    durable_io::{atomic_durable_write, remove_file_durably},
    models::{AppConfig, WipeItem, WipePreview},
};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::{
    fs, io,
    path::{Path, PathBuf},
};

/// Marker file in the app root that exists exactly while a wipe is unfinished.
const WIPE_MARKER_FILE: &str = ".pk-wipe-in-progress.json";

/// Keychain entries a wipe must clear, saved in the marker because `config.json` (where the
/// provider list lives) is the first thing the wipe deletes.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WipeSecrets {
    /// Every configured AI provider id, LLM and embedding.
    pub provider_ids: Vec<String>,
}

impl WipeSecrets {
    /// Collects the AI provider ids whose API keys may be in the keychain.
    pub fn from_config(config: &AppConfig) -> Self {
        let provider_ids = config
            .ai
            .llm_providers
            .iter()
            .chain(&config.ai.embedding_providers)
            .map(|provider| provider.id.clone())
            .collect();
        Self { provider_ids }
    }
}

/// Lists what "Delete all data" would remove, with sizes and the visible visit count.
///
/// `clears_keychain` is left `false`; the caller fills it from the platform keychain.
pub fn preview_data_wipe(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
) -> Result<WipePreview> {
    // Counted before sizing: opening the archive can create the search database it attaches.
    let visit_count = if config.initialized && paths.archive_database_path.exists() {
        visible_visit_count(paths, config, key)?
    } else {
        0
    };
    let items: Vec<WipeItem> = wipe_targets(paths)?
        .into_iter()
        .map(|path| WipeItem { bytes: path_size(&path), path: path.display().to_string() })
        .collect();
    Ok(WipePreview {
        total_bytes: items.iter().map(|item| item.bytes).sum(),
        items,
        visit_count,
        clears_keychain: false,
    })
}

/// Deletes everything [`preview_data_wipe`] lists, leaving PathKeep not initialized.
///
/// `quiesce` runs after `config.json` is gone and before anything else is deleted; it should stop
/// background work. `clear_secrets` runs after the files are gone. Both run under the write lock.
pub fn wipe_all_data(
    paths: &ProjectPaths,
    secrets: &WipeSecrets,
    quiesce: impl FnOnce() -> Result<()>,
    clear_secrets: impl FnOnce(&WipeSecrets) -> Result<()>,
) -> Result<()> {
    let _gate = ArchiveOpGate::acquire(paths);
    let _write_lock =
        ArchiveWriteLock::acquire(paths).context("waiting for other archive work to finish")?;
    fs::create_dir_all(&paths.app_root)
        .with_context(|| format!("creating {}", paths.app_root.display()))?;
    let marker = serde_json::to_vec(secrets).context("serializing the wipe marker")?;
    atomic_durable_write(&wipe_marker_path(paths), &marker).context("writing the wipe marker")?;
    run_wipe(paths, secrets, quiesce, clear_secrets)
}

/// Asks running AI and intelligence queue jobs to stop at their next checkpoint.
///
/// Called before a wipe with the archive still in place. A long rebuild that kept going could
/// otherwise open fresh connections after the wipe and write the old archive's derived rows into
/// whatever archive the user creates next.
pub fn request_running_jobs_stop(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
) -> Result<()> {
    if !paths.intelligence_database_path.exists() {
        return Ok(());
    }
    open_intelligence_connection(paths, config, key)?
        .execute_batch(
            "UPDATE ai_jobs SET stop_requested = 1 WHERE state = 'running';
             UPDATE intelligence_jobs SET stop_requested = 1 WHERE state = 'running';",
        )
        .context("asking running jobs to stop")
}

/// True while a wipe has started but not finished.
pub fn data_wipe_interrupted(paths: &ProjectPaths) -> bool {
    wipe_marker_path(paths).exists()
}

/// Finishes a wipe that a crash or failure cut short. Returns whether there was one.
///
/// The user already confirmed it, so this runs without asking again. An unreadable marker still
/// finishes the file deletion; only the provider keys it would have named stay in the keychain.
pub fn finish_interrupted_data_wipe(
    paths: &ProjectPaths,
    quiesce: impl FnOnce() -> Result<()>,
    clear_secrets: impl FnOnce(&WipeSecrets) -> Result<()>,
) -> Result<bool> {
    let marker_path = wipe_marker_path(paths);
    if !marker_path.exists() {
        return Ok(false);
    }
    let _gate = ArchiveOpGate::acquire(paths);
    let _write_lock =
        ArchiveWriteLock::acquire(paths).context("waiting for other archive work to finish")?;
    let secrets = fs::read(&marker_path)
        .ok()
        .and_then(|bytes| serde_json::from_slice::<WipeSecrets>(&bytes).ok())
        .unwrap_or_default();
    run_wipe(paths, &secrets, quiesce, clear_secrets)?;
    Ok(true)
}

/// The deletion itself. Callers hold the gate and the write lock and have written the marker.
fn run_wipe(
    paths: &ProjectPaths,
    secrets: &WipeSecrets,
    quiesce: impl FnOnce() -> Result<()>,
    clear_secrets: impl FnOnce(&WipeSecrets) -> Result<()>,
) -> Result<()> {
    remove_file_durably(&paths.config_path).context("removing the configuration")?;
    quiesce()?;

    // Keep going past a failure so one locked file does not leave everything else behind; the
    // marker stays, and the next launch retries what is left.
    let failures: Vec<String> = wipe_targets(paths)?
        .iter()
        .filter_map(|target| {
            remove_path(target).err().map(|error| format!("{}: {error}", target.display()))
        })
        .collect();
    if !failures.is_empty() {
        anyhow::bail!("could not remove {}", failures.join("; "));
    }

    clear_secrets(secrets)?;
    remove_file_durably(&wipe_marker_path(paths)).context("removing the wipe marker")
}

/// Every existing path the wipe removes. See the module docs for what is left alone.
fn wipe_targets(paths: &ProjectPaths) -> Result<Vec<PathBuf>> {
    let mut targets = Vec::new();

    let archive_dir =
        paths.archive_database_path.parent().context("archive database path has no parent")?;
    targets.extend(
        entries_of(archive_dir)?
            .into_iter()
            .filter(|path| path.file_name().is_none_or(|name| name != ARCHIVE_WRITE_LOCK_FILE)),
    );

    targets.extend([
        paths.derived_dir.clone(),
        paths.sidecars_dir.clone(),
        paths.raw_snapshots_dir.clone(),
        paths.staging_dir.clone(),
        paths.quarantine_dir.clone(),
        paths.audit_repo_path.clone(),
        paths.exports_dir.clone(),
        paths.models_dir.clone(),
        paths.app_root.join("integrations"),
        paths.app_root.join("app-lock-state.json"),
        paths.app_root.join("app-lock-passcode.json"),
        paths.stronghold_path.clone(),
        paths.stronghold_salt_path.clone(),
        paths.config_path.clone(),
    ]);

    // Whole-app imports keep the replaced folders beside the originals as `<name>.bak-<time>`.
    targets.extend(entries_of(&paths.app_root)?.into_iter().filter(|path| {
        path.file_name().is_some_and(|name| name.to_string_lossy().contains(".bak-"))
    }));

    targets.retain(|path| fs::symlink_metadata(path).is_ok());
    Ok(targets)
}

fn entries_of(dir: &Path) -> Result<Vec<PathBuf>> {
    match fs::read_dir(dir) {
        Ok(entries) => entries
            .map(|entry| entry.map(|entry| entry.path()))
            .collect::<io::Result<Vec<_>>>()
            .with_context(|| format!("listing {}", dir.display())),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(error) => Err(error).with_context(|| format!("listing {}", dir.display())),
    }
}

/// Removes a file, folder or symlink. A symlink is unlinked; its target is never touched.
fn remove_path(path: &Path) -> io::Result<()> {
    let result = match fs::symlink_metadata(path) {
        Ok(meta) if meta.is_dir() => fs::remove_dir_all(path),
        // Windows keeps directory symlinks as directories, which `remove_file` refuses.
        Ok(meta) if meta.file_type().is_symlink() => {
            fs::remove_file(path).or_else(|_| fs::remove_dir(path))
        }
        Ok(_) => fs::remove_file(path),
        Err(error) => Err(error),
    };
    match result {
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        other => other,
    }
}

/// Bytes on disk under `path`, without following symlinks.
fn path_size(path: &Path) -> u64 {
    let Ok(meta) = fs::symlink_metadata(path) else {
        return 0;
    };
    if !meta.is_dir() {
        return if meta.file_type().is_symlink() { 0 } else { meta.len() };
    }
    fs::read_dir(path)
        .map(|entries| entries.flatten().map(|entry| path_size(&entry.path())).sum())
        .unwrap_or(0)
}

fn visible_visit_count(paths: &ProjectPaths, config: &AppConfig, key: Option<&str>) -> Result<i64> {
    let connection = open_archive_connection(paths, config, key)?;
    let totals = match load_cached_archive_totals(&connection)? {
        Some(cached) => cached,
        None => count_visible_archive_totals(&connection)?,
    };
    Ok(i64::try_from(totals.total_visits).unwrap_or(i64::MAX))
}

fn wipe_marker_path(paths: &ProjectPaths) -> PathBuf {
    paths.app_root.join(WIPE_MARKER_FILE)
}

#[cfg(test)]
mod tests;
