//! "Delete all data": a preview of what PathKeep removes, and the removal.
//!
//! ## Responsibilities
//! - Preview everything under the app root that holds the user's history or something derived
//!   from it, with sizes and the archive's visit count.
//! - Remove it under the archive write lock, with an on-disk marker so a wipe cut short by a
//!   crash or power loss finishes on the next launch instead of leaving half an archive behind.
//!
//! ## Not responsible for
//! - Deciding what is removed. Each artifact's row in `vault_layout` says what a wipe does with
//!   it (`WipePolicy`); this module reads those answers and keeps no list of its own.
//! - The confirmation word, keychain entries, the OS backup task, stopping background workers and
//!   the in-memory session key. `vault-worker` owns those and passes the platform steps in as
//!   closures.
//! - The user's browser profiles. Nothing here reads a browser path: every target is a path under
//!   [`ProjectPaths::app_root`].
//!
//! ## What is removed
//! - Everything in `archive/` except the write-lock and owner-lock sentinels, whether or not the
//!   table knows the file: the archive and source-evidence databases with their WAL/SHM files,
//!   import backstops (`*.bak-*`) and rekey/restore/import markers.
//! - The folders whose row says so, whole, including anything inside the table does not list:
//!   `derived/` (search, intelligence, AI vectors, agent conversations), `sidecars/`,
//!   `raw-snapshots/`, `staging/`, `quarantine/`, `audit/` (run ledger and manifests), `exports/`,
//!   `models/` (downloaded embedding models), `integrations/` and `schedule/` (scheduled-run logs
//!   and attempt history).
//! - Import backstops in the app root (any name containing `.bak-`), the App Lock state and
//!   passcode files, the Stronghold files versions before 0.5 left (`vault.hold`,
//!   `stronghold-salt.txt`), and, last of all, `config.json`.
//! - Outside the app root, through the caller's closures: the archive key and AI provider keys in
//!   the keychain, and an installed automatic backup (LaunchAgent or Task Scheduler task).
//!
//! ## What stays, and why
//! - `logs/` and `diagnostics/`. The log plugin keeps `rust.log` open, and if a wipe fails the logs
//!   are the only record of why. They hold no visit rows.
//! - `audit/scheduler/remove-<time>.json`, when an automatic backup was removed. The scheduler
//!   writes this record of the removal after `audit/` is gone; it names the task, nothing else.
//! - `archive/.pk-archive-write.lock` and `archive/.pk-owner.lock`. Deleting either inode
//!   would let a second process lock a new file while the first still holds the old one.
//! - A file directly in the app root that no row selects and whose name has no `.bak-` in it.
//!   PathKeep writes none itself, but a bundle the user exported straight into this folder is
//!   one, and it survives. That is a known gap, not a decision.
//!
//! ## Order
//! Gate and write lock, the intent, `config.json` replaced by an empty one, `quiesce`, the files,
//! the keychain, the automatic backup, a scan for anything left, the intent, `config.json`.
//!
//! `config.json` is never missing while data is still there. A release that cannot read this
//! archive stops at its config; without one it would take the folder for a first run. So the
//! control is emptied first (every worker stops at its next look at it) and removed last, after
//! the intent. A crash between those two leaves an empty, not-initialized control in an empty
//! folder.
//!
//! The intent is written under a name PathKeep 0.5 and earlier do not know: they act on their own
//! marker before reading the config. That marker is still read and finished, as the record a wipe
//! of theirs may have left.
//!
//! The task goes after the files so that a removal that fails cannot keep any history on disk. A
//! failed removal does not fail the wipe: the data is gone, the app is not initialized, and the
//! [`WipeReport`] says what is still installed so the UI can tell the user. The intent is removed
//! anyway; keeping it would rerun the whole wipe at every launch and before every onboarding, and
//! a task the OS will not let PathKeep remove would block setup for good.
//!
//! ## Ways this can go wrong, and what guards each
//! 1. A browser profile gets touched. Targets come only from `ProjectPaths`; the test runs a real
//!    backup from a fixture profile and checks those files are byte-identical afterwards.
//! 2. A backup, import or rekey writes while files are being deleted. The wipe holds the
//!    in-process gate and the cross-process write lock for its whole run, so a scheduled backup
//!    that starts meanwhile defers and an in-flight one finishes first.
//! 3. Background workers keep writing and recreate an empty archive. `config.json` is replaced by
//!    a not-initialized one first, so every worker loop stops at its next check, and `quiesce`
//!    waits for running workers before anything else is deleted. A scan after the deletion, which
//!    does not use the list of targets, refuses to finish while anything that means an archive is
//!    left.
//! 4. A crash midway leaves part of the archive, and onboarding reopens it. The intent is written
//!    before anything is deleted and removed after everything but the control; launch and archive
//!    initialization finish an interrupted wipe first.
//! 5. A symlinked folder (say `models/` on another disk) is followed and its target emptied.
//!    Symlinks are unlinked, never followed.
//! 6. A second wipe fails because everything is already gone. Missing targets are skipped.
//! 7. Stale in-process caches make the next archive in the same process skip its bootstrap. Each
//!    cache already re-checks that its file exists (or has its schema table) before trusting
//!    itself; the test builds, backs up and searches a new archive in the same process.
//! 8. The automatic backup survives and fails every run with "archive has not been initialized".
//!    Its plan is taken before `config.json` goes and saved in the marker, so an interrupted wipe
//!    removes it on the next launch too; `vault-worker` tests install a sandboxed task and check it
//!    is gone.
//!
//! 9. Deleting the owner lock lets a second app acquire a different inode. The deletion
//!    list retains it; desktop `owner_lock_wipe` proves the file and OS lock survive.
//! 10. An intent this build did not write, or one it cannot read, deletes something the user
//!     never confirmed. Its identity and version are checked, and an unreadable one stops the
//!     wipe.
//! 11. An intent finishes a wipe of another archive than the one the user confirmed: the folder
//!     was restored from a backup or imported into after a wipe could not finish, or the intent
//!     was carried in from another folder. The intent records the control the user confirmed
//!     deleting and the emptied control the wipe writes; it is finished only while the folder
//!     holds one of the two, and is discarded otherwise. Restore, import and conversion finish a
//!     pending wipe before they install anything (`vault-worker`), and refuse while one is
//!     pending. A marker of PathKeep 0.5 is not acted on beside a stamped control: that release
//!     cannot have started a wipe of an archive it cannot read.
//! 12. A temporary file of an interrupted atomic write keeps the old settings past the wipe. The
//!     `.pk-durable-*` files directly in the folder are removed with the rest.
//!
//! ## Performance notes
//! - The preview takes the visit count from the last backup's cached totals; it only counts rows
//!   when no successful run has cached them. Folder sizes are a metadata walk.

#[cfg(test)]
use super::write_lock::ARCHIVE_WRITE_LOCK_FILE;
use super::write_lock::{ArchiveOpGate, ArchiveWriteLock};
use crate::{
    config::ProjectPaths,
    durable_io::{atomic_durable_write, remove_file_durably},
    models::{AppConfig, SchedulePlan, WipeItem, WipePreview, WipeReport},
};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::{
    fs, io,
    path::{Path, PathBuf},
};

/// The intent this build writes. It exists exactly while a wipe is unfinished.
const WIPE_INTENT_FILE: &str =
    vault_layout::spec(vault_layout::ArtifactId::WipeIntentV2).path.relative();
/// The marker PathKeep 0.5 and earlier wrote instead. Read and finished, never written.
const LEGACY_MARKER_FILE: &str =
    vault_layout::spec(vault_layout::ArtifactId::WipeIntent).path.relative();
const IDENTITY: &str = "pathkeep-wipe";
const VERSION: u32 = 1;

/// The intent as it is stored: what the user confirmed, with what checks it is ours.
#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub(crate) struct WipeIntent {
    identity: String,
    version: u32,
    /// SHA-256 of `config.json` as it was when the user confirmed; `None` if there was none.
    control: Option<String>,
    /// The emptied `config.json` the wipe writes first, exactly. `None` if there was no control.
    emptied: Option<String>,
    #[serde(flatten)]
    outside: WipeOutsideState,
}

/// What a wipe clears outside the app root, saved in the intent because `config.json` (where the
/// provider list and the schedule settings live) is emptied before anything is deleted.
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct WipeOutsideState {
    /// Every configured AI provider id, LLM and embedding, whose API key may be in the keychain.
    pub provider_ids: Vec<String>,
    /// The installed automatic backup to remove, planned while the config was still there.
    #[serde(default)]
    pub schedule: Option<SchedulePlan>,
}

impl WipeOutsideState {
    /// Collects the AI provider ids from `config`; `schedule` is the installed task, if any.
    pub fn from_config(config: &AppConfig, schedule: Option<SchedulePlan>) -> Self {
        let provider_ids = config
            .ai
            .llm_providers
            .iter()
            .chain(&config.ai.embedding_providers)
            .map(|provider| provider.id.clone())
            .collect();
        Self { provider_ids, schedule }
    }
}

/// Lists what "Delete all data" would remove, with sizes and the visible visit count.
///
/// `clears_keychain`, `removes_schedule` and `schedule_items` are left empty; the caller fills them
/// from the platform keychain and scheduler.
pub fn preview_data_wipe(paths: &ProjectPaths) -> Result<WipePreview> {
    // Read from the open archive's kept totals. Without an open archive (locked, or
    // not created yet) there is nothing PathKeep can count, and the old-format files
    // a conversion leaves behind are never opened here.
    let visit_count = if let Some(store) = crate::vault::live::store(&paths.app_root) {
        store.read(vault_store::Db::Archive, |s| {
            s.query_one("SELECT value FROM archive_totals WHERE name='visits'", (), |r| {
                Ok(r.get(0)?)
            })
        })?
    } else {
        0
    };
    // Everything the wipe removes, including what goes after the rest (`config.json`).
    let mut targets = wipe_targets(paths)?;
    targets.extend(
        vault_layout::artifacts()
            .filter(|s| s.lifecycle.wipe == vault_layout::WipePolicy::RemoveAfterCompletion)
            .map(|s| paths.artifact_path(s.id))
            .filter(|path| fs::symlink_metadata(path).is_ok()),
    );
    let items: Vec<WipeItem> = targets
        .into_iter()
        .map(|path| WipeItem { bytes: path_size(&path), path: path.display().to_string() })
        .collect();
    Ok(WipePreview {
        total_bytes: items.iter().map(|item| item.bytes).sum(),
        items,
        visit_count,
        ..WipePreview::default()
    })
}

/// Deletes everything [`preview_data_wipe`] lists, leaving PathKeep not initialized.
///
/// `quiesce` runs after `config.json` is gone and before anything else is deleted; it should stop
/// background work. `clear_secrets` runs after the files are gone, then `remove_schedule` when
/// `outside` names an installed task. All run under the write lock; see the module docs for the
/// order and for why a failed schedule removal is reported instead of failing the wipe.
pub fn wipe_all_data(
    paths: &ProjectPaths,
    outside: &WipeOutsideState,
    quiesce: impl FnOnce() -> Result<()>,
    clear_secrets: impl FnOnce(&WipeOutsideState) -> Result<()>,
    remove_schedule: impl FnOnce(&SchedulePlan) -> Result<()>,
) -> Result<WipeReport> {
    crate::admission::guard(&paths.app_root)?;
    let _gate = ArchiveOpGate::acquire(paths);
    let _write_lock =
        ArchiveWriteLock::acquire(paths).context("waiting for other archive work to finish")?;
    fs::create_dir_all(&paths.app_root)
        .with_context(|| format!("creating {}", paths.app_root.display()))?;
    // Under the locks: the control the user confirmed deleting is the one on disk now.
    let control = crate::admission::read_control(&paths.app_root)?;
    let emptied = control.as_ref().map(|_| emptied_control(paths)).transpose()?;
    let intent = WipeIntent {
        identity: IDENTITY.into(),
        version: VERSION,
        control: control.as_deref().map(crate::utils::sha256_hex),
        emptied,
        outside: outside.clone(),
    };
    let bytes = serde_json::to_vec(&intent).context("serializing the wipe intent")?;
    atomic_durable_write(&intent_path(paths), &bytes).context("writing the wipe intent")?;
    crate::fault_inject::checkpoint("wipe-intent-written")?;
    run_wipe(paths, &intent.into_recorded(), quiesce, clear_secrets, remove_schedule)
}

/// Asks running AI and intelligence queue jobs to stop at their next checkpoint.
///
/// Called before a wipe with the archive still in place. A long rebuild that kept going could
/// otherwise open fresh connections after the wipe and write the old archive's derived rows into
/// whatever archive the user creates next. Jobs only run in a process with the archive open.
pub fn request_running_jobs_stop(paths: &ProjectPaths) -> Result<()> {
    let Some(store) = crate::vault::live::store(&paths.app_root) else { return Ok(()) };
    store
        .write(vault_store::Db::Intelligence, vault_store::Priority::Interactive, |s| {
            s.execute("UPDATE ai_jobs SET stop_requested=1 WHERE state='running'", ())?;
            s.execute("UPDATE intelligence_jobs SET stop_requested=1 WHERE state='running'", ())?;
            Ok(())
        })
        .context("asking running jobs to stop")
}

/// True while a wipe has started but not finished.
pub fn data_wipe_interrupted(paths: &ProjectPaths) -> bool {
    intent_path(paths).exists() || legacy_marker_path(paths).exists()
}

/// Finishes a wipe that a crash or failure cut short. `None` when there was none.
///
/// The user already confirmed it, so this runs without asking again, but only on a record that
/// says so: an intent whose identity or version is wrong, or that does not decode, deletes
/// nothing. Intents are written whole or not at all, so such a file was not left by a wipe.
pub fn finish_interrupted_data_wipe(
    paths: &ProjectPaths,
    quiesce: impl FnOnce() -> Result<()>,
    clear_secrets: impl FnOnce(&WipeOutsideState) -> Result<()>,
    remove_schedule: impl FnOnce(&SchedulePlan) -> Result<()>,
) -> Result<Option<WipeReport>> {
    if !data_wipe_interrupted(paths) {
        return Ok(None);
    }
    // The marker is acted on only in a root this build may use. With the control already gone
    // (the wipe removes it first) the marker itself has to decode: see `admission::census`.
    crate::admission::inspect(&paths.app_root)?;
    // Checked before the locks: a record this build will not act on leaves the folder exactly as
    // it is, lock files included.
    recorded_outside_state(paths)?;
    let _gate = ArchiveOpGate::acquire(paths);
    let _write_lock =
        ArchiveWriteLock::acquire(paths).context("waiting for other archive work to finish")?;
    // Read again under the locks. Another process may have finished this wipe while this one
    // waited, and the folder may hold a new archive since: a record that is gone is not acted on.
    if !data_wipe_interrupted(paths) {
        return Ok(None);
    }
    let recorded = recorded_outside_state(paths)?;
    if !recorded.describes(paths)? {
        // Not the archive the user confirmed deleting. Nothing of it is touched; the record,
        // which can no longer be acted on, is removed so that no later start acts on it either.
        eprintln!(
            "pathkeep: discarding a delete-all-data record that does not match this archive: {}",
            recorded.path.display()
        );
        remove_file_durably(&recorded.path).context("removing a stale wipe record")?;
        return Ok(None);
    }
    run_wipe(paths, &recorded, quiesce, clear_secrets, remove_schedule).map(Some)
}

/// An interrupted wipe's record, whichever release wrote it.
struct Recorded {
    path: PathBuf,
    outside: WipeOutsideState,
    /// What this build's intent recorded; for a 0.5 marker, nothing.
    binding: Option<(Option<String>, Option<String>)>,
}

impl WipeIntent {
    fn into_recorded(self) -> Recorded {
        Recorded {
            path: PathBuf::new(),
            outside: self.outside,
            binding: Some((self.control, self.emptied)),
        }
    }
}

impl Recorded {
    /// Whether the folder still holds the archive the user confirmed deleting: the control they
    /// confirmed, or the emptied one this wipe wrote. A marker of PathKeep 0.5 binds to nothing,
    /// so it is honoured as that release would, except beside a stamped control: 0.5 cannot have
    /// started a wipe of an archive it cannot read.
    fn describes(&self, paths: &ProjectPaths) -> Result<bool> {
        let current = crate::admission::read_control(&paths.app_root)?;
        Ok(match &self.binding {
            Some((confirmed, emptied)) => match current {
                None => confirmed.is_none(),
                Some(bytes) => {
                    confirmed.as_deref() == Some(crate::utils::sha256_hex(&bytes).as_str())
                        || emptied.as_deref().map(str::as_bytes) == Some(bytes.as_slice())
                }
            },
            None => match current {
                None => true,
                Some(bytes) => crate::admission::parse_control(&paths.app_root, &bytes)
                    .map(|control| control.compatibility.is_none())
                    .unwrap_or(false),
            },
        })
    }
}

/// What the interrupted wipe's record says, and which record it is.
fn recorded_outside_state(paths: &ProjectPaths) -> Result<Recorded> {
    let unreadable = |path: &Path| {
        anyhow::anyhow!(
            "{}: the delete-all-data record {} is not one this build wrote; nothing was deleted",
            crate::admission::Reason::ControlInvalid.code(),
            path.display()
        )
    };
    let intent = intent_path(paths);
    if intent.exists() {
        let record = fs::read(&intent)
            .ok()
            .and_then(|bytes| serde_json::from_slice::<WipeIntent>(&bytes).ok())
            .filter(|record| record.identity == IDENTITY && record.version == VERSION)
            .ok_or_else(|| unreadable(&intent))?;
        return Ok(Recorded { path: intent, ..record.into_recorded() });
    }
    let legacy = legacy_marker_path(paths);
    let outside = fs::read(&legacy)
        .ok()
        .and_then(|bytes| serde_json::from_slice::<WipeOutsideState>(&bytes).ok())
        .ok_or_else(|| unreadable(&legacy))?;
    Ok(Recorded { path: legacy, outside, binding: None })
}

/// Whether the marker PathKeep 0.5 left decodes. A folder without a control is finished only on
/// a marker that does.
pub(crate) fn legacy_marker_is_readable(paths: &ProjectPaths) -> bool {
    fs::read(legacy_marker_path(paths))
        .ok()
        .is_some_and(|bytes| serde_json::from_slice::<WipeOutsideState>(&bytes).is_ok())
}

/// The empty, not-initialized control that replaces the archive's: default settings, the
/// archive's compatibility stamp kept.
fn emptied_control(paths: &ProjectPaths) -> Result<String> {
    let compatibility =
        crate::admission::guard(&paths.app_root)?.and_then(|control| control.compatibility);
    let mut config = AppConfig::default();
    crate::models::normalize_app_config(&mut config);
    crate::config::encode_config(&config, compatibility.as_ref())
}

/// Replaces the control with the empty one. The intent's own copy when it has one, so a wipe
/// finished by another build writes the same bytes the intent recorded. Nothing if there is no
/// control (a marker of PathKeep 0.5 is finished after that release had already removed it).
fn empty_control(paths: &ProjectPaths, recorded: &Recorded) -> Result<()> {
    if !paths.config_path.exists() {
        return Ok(());
    }
    let content = match &recorded.binding {
        Some((_, Some(emptied))) => emptied.clone(),
        _ => emptied_control(paths)?,
    };
    atomic_durable_write(&paths.config_path, content.as_bytes())
        .context("replacing the configuration with an empty one")
}

/// The deletion itself. Callers hold the gate and the write lock and have written the intent.
fn run_wipe(
    paths: &ProjectPaths,
    recorded: &Recorded,
    quiesce: impl FnOnce() -> Result<()>,
    clear_secrets: impl FnOnce(&WipeOutsideState) -> Result<()>,
    remove_schedule: impl FnOnce(&SchedulePlan) -> Result<()>,
) -> Result<WipeReport> {
    let outside = &recorded.outside;
    empty_control(paths, recorded)?;
    crate::fault_inject::checkpoint("wipe-control-emptied")?;
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
    crate::fault_inject::checkpoint("wipe-files-removed")?;

    clear_secrets(outside)?;

    let report = match &outside.schedule {
        None => WipeReport::default(),
        Some(plan) => match remove_schedule(plan) {
            Ok(()) => WipeReport { schedule_removed: true, schedule_error: None },
            Err(error) => {
                WipeReport { schedule_removed: false, schedule_error: Some(format!("{error:#}")) }
            }
        },
    };
    // Not the list of targets above: the same rule that decides whether a folder without a
    // control holds an archive. Whatever it finds, a worker that did not stop or a file the
    // table does not name, is still the user's data.
    let records = [intent_path(paths), legacy_marker_path(paths)];
    let left: Vec<String> = crate::admission::archive_files(&paths.app_root)?
        .into_iter()
        .filter(|relative| !records.iter().any(|record| paths.app_root.join(relative) == *record))
        .collect();
    if let Some(first) = left.first() {
        anyhow::bail!(
            "could not remove {first} and {} more; the wipe will run again",
            left.len() - 1
        );
    }
    for record in &records {
        remove_file_durably(record).context("removing the wipe intent")?;
    }
    crate::fault_inject::checkpoint("wipe-intent-removed")?;
    remove_file_durably(&paths.config_path).context("removing the configuration")?;
    Ok(report)
}

/// Every existing path the wipe removes. See the module docs for what is left alone.
fn wipe_targets(paths: &ProjectPaths) -> Result<Vec<PathBuf>> {
    let mut targets = Vec::new();

    use vault_layout::{WipePolicy, artifacts, spec};
    // Preserve the existing error for a malformed caller-supplied canonical path.
    paths.archive_db.parent().context("archive database path has no parent")?;
    for container in
        artifacts().filter(|s| s.lifecycle.wipe == WipePolicy::ArchiveChildrenExceptLocks)
    {
        let kept: Vec<_> = artifacts()
            .filter(|s| s.lifecycle.wipe == WipePolicy::PreserveRuntime)
            .filter(|s| {
                Path::new(s.path.relative()).parent() == Some(Path::new(container.path.relative()))
            })
            .map(|s| vault_layout::file_name(s.id))
            .collect();
        targets.extend(entries_of(&paths.artifact_path(container.id))?.into_iter().filter(
            |path| path.file_name().is_none_or(|name| !kept.iter().any(|keep| name == *keep)),
        ));
    }
    targets.extend(vault_layout::wipe_targets().into_iter().map(|id| paths.artifact_path(id)));
    for family in artifacts().filter(|s| s.lifecycle.wipe == WipePolicy::RootFamily) {
        let pattern = vault_layout::family_pattern(family.id);
        targets.extend(
            entries_of(&paths.app_root.join(spec(family.id).path.relative()))?.into_iter().filter(
                |path| {
                    path.file_name().is_some_and(|name| pattern.matches(&name.to_string_lossy()))
                },
            ),
        );
    }

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

fn intent_path(paths: &ProjectPaths) -> PathBuf {
    paths.app_root.join(WIPE_INTENT_FILE)
}

fn legacy_marker_path(paths: &ProjectPaths) -> PathBuf {
    paths.app_root.join(LEGACY_MARKER_FILE)
}

#[cfg(test)]
mod tests;
