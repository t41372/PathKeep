//! "Delete all data": a preview of what PathKeep removes, and the removal.
//!
//! ## Responsibilities
//! - Preview everything under the app root that holds the user's history or something derived
//!   from it, with sizes and the archive's visit count.
//! - Remove it under the archive write lock. Everything is first moved into one folder in the data
//!   folder, the trash, and deleted only once all of it is there. A wipe cut short before that is
//!   undone; after it, finished. Either way on the next launch, from a record written first.
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
//!   `stronghold-salt.txt`), a trash an earlier wipe left behind (`wipe-trash-*`), and, last of
//!   all, `config.json`.
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
//! Gate, write lock and control gate; the record an earlier wipe left, settled; the trash and the
//! intent; `config.json` replaced by an empty one; `quiesce`; the targets recorded in the intent;
//! every target moved into the trash and a scan for anything left outside it; the commit (the
//! intent says everything is in the trash); the trash deleted; the keychain; the automatic backup;
//! the intent; `config.json`.
//!
//! Before the commit nothing is deleted. A failure puts everything back in the same call, and a
//! crash leaves the intent from which the next launch does. After the commit nothing is put back:
//! the next launch deletes what is left. The moves are renames within one folder, so the window
//! in which a crash leaves a wipe to be finished rather than undone is the deletion itself. A
//! commit whose write reports an error may still be on disk (the flush after its rename failed):
//! the wipe then stops where it is, and the next launch goes the way the intent on disk says.
//!
//! Undoing gives back the archive the user confirmed deleting, whole: every target the intent
//! recorded comes back from the trash, and what stands in its place now goes into the trash
//! (`displaced/`), as does anything at a target the wipe did not record. Nothing is deleted, and
//! no store is left beside another's log or index (`-wal`, `-shm`), which SQLite would read with
//! it. A recorded target in neither place stops the undo before anything moves: an empty trash is
//! not proof that everything is back.
//!
//! `config.json` is never missing while data is still there. A release that cannot read this
//! archive stops at its config; without one it would take the folder for a first run. So the
//! control is emptied first (every worker stops at its next look at it) and removed last, after
//! the intent. The emptied control carries a stamp, this build's where the archive had none, so
//! PathKeep 0.5 does not take a folder in the middle of a wipe for a first run. A crash between
//! the intent and the control leaves an empty, not-initialized control in an empty folder; setup
//! writes its control without that stamp (`config::save_config`). Nor is the intent ever without
//! a control beside it: such a folder reads as an archive whose control is missing
//! (`admission::census`), which no launch settles. Where there was no control, the empty one is
//! written before the intent, and a wipe undone removes it after.
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
//! 2. A backup, import or rekey writes while files are being moved. The wipe holds the in-process
//!    gate and the cross-process write lock for its whole run, so a scheduled backup that starts
//!    meanwhile defers and an in-flight one finishes first.
//! 3. Background workers keep writing and recreate an empty archive. `config.json` is replaced by
//!    a not-initialized one first, so every worker loop stops at its next check, and `quiesce`
//!    waits for running workers before anything is moved. A scan after the moves, which does not
//!    use the list of targets, refuses to commit while anything that means an archive is left
//!    outside the trash; after the commit, what appears at a target is deleted where it is.
//! 4. A crash or a failure midway leaves part of the archive, and onboarding reopens it, or the
//!    user is left with neither the archive nor a finished wipe. Nothing is deleted before
//!    everything is in the trash; until then a failure puts it all back, and the next launch does
//!    after a crash. Launch, archive initialization and every install settle an interrupted wipe
//!    first.
//! 5. A symlinked folder (say `models/` on another disk) is followed and its target emptied.
//!    Symlinks are moved and unlinked, never followed.
//! 6. A second wipe fails because everything is already gone. Missing targets are skipped.
//! 7. Stale in-process caches make the next archive in the same process skip its bootstrap. Each
//!    cache already re-checks that its file exists (or has its schema table) before trusting
//!    itself; the test builds, backs up and searches a new archive in the same process.
//! 8. The automatic backup survives and fails every run with "archive has not been initialized".
//!    Its plan is taken before `config.json` goes and saved in the intent, so an interrupted wipe
//!    removes it on the next launch too; `vault-worker` tests install a sandboxed task and check it
//!    is gone.
//! 9. Deleting the owner lock lets a second app acquire a different inode. The deletion
//!    list retains it; desktop `owner_lock_wipe` proves the file and OS lock survive.
//! 10. An intent this build did not write, or one it cannot read, moves or deletes something the
//!     user never confirmed. Its identity and version are checked, the trash it names must be a
//!     `wipe-trash-*` folder directly in the data folder, and anything else stops the wipe.
//! 11. A record acts on another archive than the one the user confirmed: the folder was restored
//!     from a backup or imported into after a wipe could not finish, or the intent was carried in
//!     from another folder. The intent holds the control the user confirmed deleting and the
//!     emptied control the wipe writes, and acts only while the folder holds one of the two;
//!     anywhere else the intent goes and nothing else is touched, except that its trash, if it is
//!     here, gets that control back: the record was its only copy. Undone, it never overwrites
//!     what is there now: that goes into the trash. Finished, it deletes its
//!     own trash and, only while the control is the emptied one, what lies at a target. A copy of
//!     the folder made during a wipe is the same folder to it: undone before the commit, which
//!     gives the copy its archive back, and finished after it, which deletes the copy's copy of
//!     what the user confirmed deleting. Restore, import and conversion refuse while a wipe is
//!     pending, and `vault-worker` settles it before they install anything. A marker of PathKeep
//!     0.5 is finished only where there is no control: that release removed the control right
//!     after writing its marker, before deleting anything, so beside a control its wipe had
//!     deleted nothing.
//! 12. A temporary file of an interrupted atomic write keeps the old settings past the wipe. The
//!     `.pk-durable-*` files directly in the folder are removed with the rest.
//! 13. A second "Delete all data" overwrites the intent of one that did not end, and with it the
//!     control to put back and the provider keys to clear. It settles that intent first, as a
//!     launch would.
//! 14. Undoing mixes the archive with files that came into the folder since, or loses a file the
//!     user's next archive holds. Whatever stands where an entry goes is moved into the trash's
//!     `displaced/` folder, never deleted. The trash is then kept, and is listed with the next
//!     "Delete all data".
//! 15. A power cut during a rename on a file system without a journal (exFAT, FAT32) leaves one
//!     file under two names; deleting one name frees data the other still uses. Undoing deletes
//!     nothing that stood in an entry's place, the entry's own second name included: it goes into
//!     the trash, which is then kept.
//!
//! ## Performance notes
//! - The preview takes the visit count from the last backup's cached totals; it only counts rows
//!   when no successful run has cached them. Folder sizes are a metadata walk.
//! - Moving is one rename per target, whatever its size: the targets are whole folders and the
//!   files directly in `archive/`.

#[cfg(test)]
use super::write_lock::ARCHIVE_WRITE_LOCK_FILE;
use super::write_lock::{ArchiveOpGate, ArchiveWriteLock};
use crate::{
    config::ProjectPaths,
    durable_io::{atomic_durable_write, remove_file_durably},
    maintenance::install::{exists, rename},
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
/// The error of a wipe that failed after its commit, so that the window says the archive is gone
/// rather than that nothing was deleted. The desktop shell reads the code (`command_error.rs`).
const UNFINISHED: &str = "archive-wipe-unfinished: everything was moved out of the archive, and \
                          the next start finishes deleting it";
/// The error of a wipe whose commit could not be written: it may be on disk or not, so nothing is
/// put back now, and the next start goes the way the intent on disk says.
const STOPPED: &str = "archive-wipe-stopped: the wipe stopped after moving everything out of the \
                       archive; the next start finishes it or puts everything back";
/// Where an undone wipe moves what stood in the place of an entry it puts back, inside its trash.
const DISPLACED: &str = "displaced";

/// The intent as it is stored: what the user confirmed, where the wipe moves it, and how far it
/// got.
#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub(crate) struct WipeIntent {
    identity: String,
    version: u32,
    /// `config.json` as it was when the user confirmed, exactly; `None` if there was none. A wipe
    /// that is undone writes it back.
    control: Option<String>,
    /// The emptied `config.json` the wipe writes before it moves anything, exactly.
    emptied: String,
    /// The folder, directly in the data folder, that everything is moved into before anything is
    /// deleted (`wipe-trash-*`).
    trash: PathBuf,
    /// Every target the wipe moves into the trash, relative to the data folder, recorded once
    /// workers have stopped and before the first move. Absent before that: nothing has moved.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    targets: Option<Vec<PathBuf>>,
    /// Everything is in the trash. From here the wipe is finished, never undone.
    committed: bool,
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

/// What became of a wipe that a crash or a failure cut short, so that the caller can tell the
/// user: a finished wipe leaves onboarding, an undone one the archive they asked to delete.
#[derive(Debug, PartialEq)]
pub enum SettledWipe {
    /// It had moved everything into its trash. What it moved is deleted, as the user asked.
    Finished(WipeReport),
    /// It had not. Everything is back where it was, and so is the control the user had.
    RolledBack,
    /// Its record did not match the folder's control. The record is gone; nothing else was
    /// touched.
    Discarded,
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
/// `outside` says what to clear outside the folder (provider keys, the installed schedule) from
/// the control as it is once the wipe holds every lock. `quiesce` runs after `config.json` is
/// emptied and before anything is moved; it should stop background work. `clear_secrets` runs
/// after the files are gone, then `remove_schedule` when the outside state names an installed
/// task. A record an earlier wipe left is settled first, with these same steps, as a launch would.
/// See the module docs for the order and for why a failed schedule removal is reported instead of
/// failing the wipe.
pub fn wipe_all_data(
    paths: &ProjectPaths,
    outside: impl FnOnce(&AppConfig) -> Result<WipeOutsideState>,
    quiesce: impl Fn() -> Result<()>,
    clear_secrets: impl Fn(&WipeOutsideState) -> Result<()>,
    remove_schedule: impl Fn(&SchedulePlan) -> Result<()>,
) -> Result<WipeReport> {
    // Not only the control: a folder that holds an archive and no control is refused here, before
    // a record in it is acted on. Such a record holds the control it was written beside.
    crate::admission::inspect(&paths.app_root)?;
    let _gate = ArchiveOpGate::acquire(paths);
    let _write_lock =
        ArchiveWriteLock::acquire(paths).context("waiting for other archive work to finish")?;
    // From reading what to delete to the last removal, nothing else writes the control: a save
    // waiting for it then finds the folder emptied, and a provider key or a schedule waiting for
    // it is not added behind the wipe's back.
    let _control_gate = crate::ControlGate::acquire(paths);
    fs::create_dir_all(&paths.app_root)
        .with_context(|| format!("creating {}", paths.app_root.display()))?;
    // Writing this wipe's intent would lose what that one says: the control to put back, its
    // trash, the provider keys to clear.
    settle(paths, &quiesce, &clear_secrets, &remove_schedule)?;
    // Under the locks: the control the user confirmed deleting is the one on disk now.
    let control = crate::admission::read_control(&paths.app_root)?
        .map(String::from_utf8)
        .transpose()
        .context("config.json is not text; nothing was deleted")?;
    let outside = outside(&crate::load_config(paths)?)?;
    let emptied = emptied_control(paths)?;
    if control.is_none() {
        atomic_durable_write(&paths.config_path, emptied.as_bytes())
            .context("writing an empty configuration")?;
    }
    let trash = tempfile::Builder::new()
        .prefix(vault_layout::family_prefix(vault_layout::ArtifactId::WipeTrash))
        .tempdir_in(&paths.app_root)
        .context("making the folder the wipe moves everything into")?
        .keep();
    // The trash's own entry is durable before the intent names it.
    crate::durable_io::fsync_dir(&paths.app_root)?;
    let intent = WipeIntent {
        identity: IDENTITY.into(),
        version: VERSION,
        control,
        emptied,
        trash: PathBuf::from(trash.file_name().context("a trash folder name")?),
        targets: None,
        committed: false,
        outside,
    };
    save(paths, &intent)?;
    crate::fault_inject::checkpoint("wipe-intent-written")?;
    run_wipe(paths, intent, &quiesce, &clear_secrets, &remove_schedule)
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

/// Settles a wipe that a crash or a failure cut short. `None` when there was none.
///
/// Before its commit it is undone, after it finished, and either only where the folder holds a
/// control the wipe read or wrote: see [`SettledWipe`] and the module docs. The user confirmed the
/// wipe, so this runs without asking again, but only on a record that says so: an intent whose
/// identity or version is wrong, or that does not decode, settles nothing. Intents are written
/// whole or not at all, so such a file was not left by a wipe.
pub fn finish_interrupted_data_wipe(
    paths: &ProjectPaths,
    quiesce: impl Fn() -> Result<()>,
    clear_secrets: impl Fn(&WipeOutsideState) -> Result<()>,
    remove_schedule: impl Fn(&SchedulePlan) -> Result<()>,
) -> Result<Option<SettledWipe>> {
    if !data_wipe_interrupted(paths) {
        return Ok(None);
    }
    // The record is acted on only in a root this build may use. Without a control (a 0.5 wipe
    // removes it first) the marker itself has to decode: see `admission::census`.
    crate::admission::inspect(&paths.app_root)?;
    // Checked before the locks: a record this build will not act on leaves the folder exactly as
    // it is, lock files included.
    pending(paths)?;
    let _gate = ArchiveOpGate::acquire(paths);
    let _write_lock =
        ArchiveWriteLock::acquire(paths).context("waiting for other archive work to finish")?;
    let _control_gate = crate::ControlGate::acquire(paths);
    settle(paths, &quiesce, &clear_secrets, &remove_schedule)
}

/// Settles the record an earlier wipe left, if one is still there. Callers hold the gate, the
/// write lock and the control gate: another process may have settled the wipe while this one
/// waited for them, and the folder may hold a new archive since.
fn settle(
    paths: &ProjectPaths,
    quiesce: &impl Fn() -> Result<()>,
    clear_secrets: &impl Fn(&WipeOutsideState) -> Result<()>,
    remove_schedule: &impl Fn(&SchedulePlan) -> Result<()>,
) -> Result<Option<SettledWipe>> {
    if !data_wipe_interrupted(paths) {
        return Ok(None);
    }
    Ok(Some(match pending(paths)? {
        Pending::Intent(intent) if !intent.left_here(paths)? => {
            // Not the folder this wipe was in. The trash the intent names stays as it is, if it is
            // here at all, and is listed with the next "Delete all data". The control it was moved
            // with goes into it: the record is that control's only copy.
            let trash = intent.trash(paths)?;
            if let Some(control) = &intent.control
                && trash.is_dir()
            {
                atomic_durable_write(&trash.join(CONTROL), control.as_bytes())
                    .context("keeping the confirmed control with its trash")?;
            }
            eprintln!(
                "pathkeep: discarding a delete-all-data record that does not match this archive; \
                 {} is left as it is",
                intent.trash.display()
            );
            remove_file_durably(&intent_path(paths)).context("removing a stale wipe record")?;
            SettledWipe::Discarded
        }
        Pending::Intent(intent) if intent.committed => {
            quiesce()?;
            SettledWipe::Finished(finish(paths, &intent, clear_secrets, remove_schedule)?)
        }
        Pending::Intent(intent) => {
            roll_back(paths, &intent)?;
            SettledWipe::RolledBack
        }
        // That release removed the control right after writing its marker, before deleting
        // anything: beside a control its wipe had deleted nothing, and the marker goes.
        Pending::Legacy(_) if crate::admission::read_control(&paths.app_root)?.is_some() => {
            eprintln!(
                "pathkeep: discarding a PathKeep 0.5 delete-all-data marker beside a control"
            );
            remove_file_durably(&legacy_marker_path(paths))
                .context("removing a stale wipe record")?;
            SettledWipe::Discarded
        }
        Pending::Legacy(outside) => {
            quiesce()?;
            SettledWipe::Finished(finish_legacy(paths, &outside, clear_secrets, remove_schedule)?)
        }
    }))
}

/// An interrupted wipe's record, whichever release wrote it.
enum Pending {
    Intent(WipeIntent),
    /// A marker of PathKeep 0.5: what to clear outside the folder, and nothing else.
    Legacy(WipeOutsideState),
}

impl WipeIntent {
    /// Whether the folder holds a control this wipe read or wrote: the one the user confirmed
    /// deleting, or the emptied one.
    fn left_here(&self, paths: &ProjectPaths) -> Result<bool> {
        Ok(crate::admission::read_control(&paths.app_root)?.is_some_and(|bytes| {
            self.control.as_deref().map(str::as_bytes) == Some(bytes.as_slice())
                || self.emptied.as_bytes() == bytes.as_slice()
        }))
    }

    /// Whether the folder's control is the emptied one this wipe writes.
    fn emptied_here(&self, paths: &ProjectPaths) -> Result<bool> {
        Ok(crate::admission::read_control(&paths.app_root)?
            .is_some_and(|bytes| self.emptied.as_bytes() == bytes.as_slice()))
    }

    /// The trash, which has to be a `wipe-trash-*` folder directly in the data folder: an intent
    /// naming anything else would move into it or delete it.
    fn trash(&self, paths: &ProjectPaths) -> Result<PathBuf> {
        let name = self.trash.to_str().unwrap_or_default();
        anyhow::ensure!(
            self.trash.components().count() == 1
                && vault_layout::family_pattern(vault_layout::ArtifactId::WipeTrash).matches(name),
            "{}: the delete-all-data record names {} as its trash; nothing was moved or deleted",
            crate::admission::Reason::ControlInvalid.code(),
            self.trash.display()
        );
        Ok(paths.app_root.join(&self.trash))
    }
}

/// What the interrupted wipe's record says, and which record it is.
fn pending(paths: &ProjectPaths) -> Result<Pending> {
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
        record.trash(paths)?;
        return Ok(Pending::Intent(record));
    }
    let legacy = legacy_marker_path(paths);
    let outside = fs::read(&legacy)
        .ok()
        .and_then(|bytes| serde_json::from_slice::<WipeOutsideState>(&bytes).ok())
        .ok_or_else(|| unreadable(&legacy))?;
    Ok(Pending::Legacy(outside))
}

/// Whether the marker PathKeep 0.5 left decodes. A folder without a control is finished only on
/// a marker that does.
pub(crate) fn legacy_marker_is_readable(paths: &ProjectPaths) -> bool {
    fs::read(legacy_marker_path(paths))
        .ok()
        .is_some_and(|bytes| serde_json::from_slice::<WipeOutsideState>(&bytes).is_ok())
}

/// The empty, not-initialized control that replaces the archive's: default settings and a stamp,
/// the archive's, or this build's where it had none. PathKeep 0.5 opens a folder whose control
/// has no stamp, and takes a not-initialized one for a first run.
fn emptied_control(paths: &ProjectPaths) -> Result<String> {
    let compatibility = crate::admission::guard(&paths.app_root)?
        .and_then(|control| control.compatibility)
        .unwrap_or_else(crate::admission::written_compatibility);
    let mut config = AppConfig::default();
    crate::models::normalize_app_config(&mut config);
    crate::config::encode_config(&config, Some(&compatibility))
}

fn save(paths: &ProjectPaths, intent: &WipeIntent) -> Result<()> {
    let bytes = serde_json::to_vec(intent).context("serializing the wipe intent")?;
    atomic_durable_write(&intent_path(paths), &bytes).context("writing the wipe intent")
}

/// The wipe itself. Callers hold the gate, the write lock and the control gate, and have written
/// the intent. Until everything is in the trash and the intent says so, a failure puts it all
/// back.
fn run_wipe(
    paths: &ProjectPaths,
    mut intent: WipeIntent,
    quiesce: &impl Fn() -> Result<()>,
    clear_secrets: &impl Fn(&WipeOutsideState) -> Result<()>,
    remove_schedule: &impl Fn(&SchedulePlan) -> Result<()>,
) -> Result<WipeReport> {
    let moved = (|| {
        atomic_durable_write(&paths.config_path, intent.emptied.as_bytes())
            .context("replacing the configuration with an empty one")?;
        crate::fault_inject::checkpoint("wipe-control-emptied")?;
        quiesce()?;
        let targets = wipe_targets(paths)?
            .into_iter()
            .filter(|target| *target != paths.app_root.join(&intent.trash))
            .map(|target| target.strip_prefix(&paths.app_root).map(Path::to_path_buf))
            .collect::<Result<Vec<_>, _>>()
            .context("a target outside the data folder")?;
        intent.targets = Some(targets);
        save(paths, &intent)?;
        crate::fault_inject::checkpoint("wipe-targets-recorded")?;
        move_into_trash(paths, &intent)?;
        crate::fault_inject::checkpoint("wipe-moved")
    })();
    if let Err(error) = moved {
        // Nothing was deleted. What cannot be put back now, the next start puts back.
        roll_back(paths, &intent).with_context(|| {
            format!(
                "the wipe stopped ({error:#}) and could not put everything back; the next start \
                 will"
            )
        })?;
        return Err(error);
    }
    intent.committed = true;
    if let Err(error) =
        save(paths, &intent).and_then(|()| crate::fault_inject::checkpoint("wipe-commit-saved"))
    {
        // The commit may be on disk. Putting everything back now could meet it at the next start,
        // which would then delete what was put back.
        return Err(error.context(STOPPED));
    }
    // From the commit the wipe goes forward, and what fails now the next start finishes.
    (|| {
        crate::fault_inject::checkpoint("wipe-committed")?;
        finish(paths, &intent, clear_secrets, remove_schedule)
    })()
    .context(UNFINISHED)
}

/// Moves every recorded target into the trash, each by one rename. Then checks, with a scan that
/// does not use the list of targets (the same rule that decides whether a folder without a control
/// holds an archive), that nothing meaning an archive is left outside the trash: a worker that did
/// not stop, or a file the table does not name, is still the user's data.
fn move_into_trash(paths: &ProjectPaths, intent: &WipeIntent) -> Result<()> {
    let trash = intent.trash(paths)?;
    for relative in intent.targets.iter().flatten() {
        let target = paths.app_root.join(relative);
        rename(&target, &trash.join(relative))
            .with_context(|| format!("moving {} out of the archive", target.display()))?;
        crate::fault_inject::checkpoint("wipe-target-moved")?;
    }
    let left = left_outside(paths, Some(&intent.trash))?;
    if let Some(first) = left.first() {
        anyhow::bail!("could not move {first} and {} more out of the archive", left.len() - 1);
    }
    Ok(())
}

/// What still means an archive in the data folder, apart from the wipe's records and the trash.
fn left_outside(paths: &ProjectPaths, trash: Option<&Path>) -> Result<Vec<String>> {
    let records = [WIPE_INTENT_FILE, LEGACY_MARKER_FILE];
    Ok(crate::admission::archive_files(&paths.app_root)?
        .into_iter()
        .filter(|relative| !records.contains(&relative.as_str()))
        .filter(|relative| trash.is_none_or(|trash| !Path::new(relative).starts_with(trash)))
        .collect())
}

/// Undoes a wipe that had not committed: the archive the user confirmed deleting comes back whole
/// (see the module docs), then the control the user had over the emptied one, and the records go.
/// The trash goes too when nothing but emptied folders and what a file browser drops is left in
/// it. Before the targets were recorded nothing had moved, and only the control comes back.
fn roll_back(paths: &ProjectPaths, intent: &WipeIntent) -> Result<()> {
    let trash = intent.trash(paths)?;
    if let Some(targets) = &intent.targets {
        // Decided for every target before anything moves.
        let plan = targets
            .iter()
            .map(|relative| Ok((relative, back(paths, &trash, relative)?)))
            .collect::<Result<Vec<_>>>()?;
        for (relative, back) in plan {
            let (from, to) = (trash.join(relative), paths.app_root.join(relative));
            match back {
                Back::Stays => {}
                Back::Comes => {
                    rename(&from, &to).with_context(|| format!("putting {} back", to.display()))?
                }
                Back::ComesOver => {
                    displace(&trash, &paths.app_root, relative)?;
                    rename(&from, &to).with_context(|| format!("putting {} back", to.display()))?;
                }
            }
            crate::fault_inject::checkpoint("wipe-entry-put-back")?;
        }
        // What came to a target the wipe did not record is not the archive's.
        for target in wipe_targets(paths)? {
            let relative = target.strip_prefix(&paths.app_root).context("a target outside")?;
            if target != trash && !targets.iter().any(|recorded| recorded == relative) {
                displace(&trash, &paths.app_root, relative)?;
            }
        }
    }
    crate::fault_inject::checkpoint("wipe-put-back")?;
    let emptied = intent.emptied_here(paths)?;
    if emptied && let Some(control) = &intent.control {
        atomic_durable_write(&paths.config_path, control.as_bytes())
            .context("putting the configuration back")?;
    }
    if holds_files(&trash)? {
        eprintln!(
            "pathkeep: an undone delete-all-data kept files that stood in the archive's places, or \
             copies of its own, in {}",
            trash.display()
        );
    } else {
        remove_path(&trash).with_context(|| format!("removing {}", trash.display()))?;
    }
    remove_records(paths)?;
    if emptied && intent.control.is_none() {
        remove_file_durably(&paths.config_path).context("removing the empty configuration")?;
    }
    Ok(())
}

/// What undoing does with one recorded target.
enum Back {
    /// Never moved, or back already.
    Stays,
    /// Back from the trash into an empty place.
    Comes,
    /// Back from the trash, after what stands in its place is moved into the trash.
    ComesOver,
}

fn back(paths: &ProjectPaths, trash: &Path, relative: &Path) -> Result<Back> {
    let (kept, here) = (trash.join(relative), paths.app_root.join(relative));
    Ok(match (exists(&kept)?, exists(&here)?) {
        (false, true) => Back::Stays,
        (false, false) => anyhow::bail!(
            "{} is neither in the archive nor in {}; nothing was put back, and the next start \
             tries again",
            relative.display(),
            trash.display()
        ),
        (true, false) => Back::Comes,
        (true, true) => Back::ComesOver,
    })
}

/// Moves what stands at `root/relative` into the trash's `displaced/` folder, under a name
/// nothing has there.
fn displace(trash: &Path, root: &Path, relative: &Path) -> Result<()> {
    let away = (0u32..)
        .map(|n| trash.join(DISPLACED).join(n.to_string()).join(relative))
        .find(|away| !matches!(exists(away), Ok(true)))
        .context("a free name in the trash")?;
    rename(&root.join(relative), &away)
        .with_context(|| format!("moving {} out of the archive's way", relative.display()))?;
    crate::fault_inject::checkpoint("wipe-displaced")
}

/// Whether anything but folders and what a file browser drops is in `folder`.
fn holds_files(folder: &Path) -> Result<bool> {
    for entry in walkdir::WalkDir::new(folder) {
        let entry = match entry {
            Ok(entry) => entry,
            Err(error) if error.io_error().is_some_and(|e| e.kind() == io::ErrorKind::NotFound) => {
                return Ok(false);
            }
            Err(error) => return Err(error.into()),
        };
        if !entry.file_type().is_dir()
            && !entry.file_name().to_str().is_some_and(vault_layout::is_os_metadata)
        {
            return Ok(true);
        }
    }
    Ok(false)
}

/// The end of a committed wipe: the trash goes, and then, while the control is the emptied one
/// this wipe wrote, whatever lies at a target, the keychain and the automatic backup; then the
/// records and that control. Run again after a crash, it finishes what is left. Where the control
/// is the one the user confirmed deleting (put back by hand since), only the trash and the
/// records go.
fn finish(
    paths: &ProjectPaths,
    intent: &WipeIntent,
    clear_secrets: &impl Fn(&WipeOutsideState) -> Result<()>,
    remove_schedule: &impl Fn(&SchedulePlan) -> Result<()>,
) -> Result<WipeReport> {
    let ours = intent.emptied_here(paths)?;
    let trash = intent.trash(paths)?;
    remove_path(&trash).with_context(|| format!("deleting {}", trash.display()))?;
    if ours {
        remove_in_place(paths)?;
    }
    crate::fault_inject::checkpoint("wipe-files-removed")?;
    let mut report = WipeReport::default();
    if ours {
        clear_secrets(&intent.outside)?;
        report = remove_installed_schedule(&intent.outside, remove_schedule);
    }
    remove_records(paths)?;
    crate::fault_inject::checkpoint("wipe-intent-removed")?;
    if ours {
        remove_file_durably(&paths.config_path).context("removing the configuration")?;
    }
    Ok(report)
}

/// Finishes a wipe PathKeep 0.5 began, as that release did: it had removed the control and was
/// deleting in place.
fn finish_legacy(
    paths: &ProjectPaths,
    outside: &WipeOutsideState,
    clear_secrets: &impl Fn(&WipeOutsideState) -> Result<()>,
    remove_schedule: &impl Fn(&SchedulePlan) -> Result<()>,
) -> Result<WipeReport> {
    remove_in_place(paths)?;
    crate::fault_inject::checkpoint("wipe-files-removed")?;
    clear_secrets(outside)?;
    let report = remove_installed_schedule(outside, remove_schedule);
    remove_records(paths)?;
    crate::fault_inject::checkpoint("wipe-intent-removed")?;
    Ok(report)
}

/// Deletes every target where it lies, going on past a failure so that one locked file does not
/// leave everything else behind, then checks with the scan that nothing meaning an archive is
/// left. On a failure the record stays, and the next launch retries what is left.
fn remove_in_place(paths: &ProjectPaths) -> Result<()> {
    let failures: Vec<String> = wipe_targets(paths)?
        .iter()
        .filter_map(|target| {
            remove_path(target).err().map(|error| format!("{}: {error}", target.display()))
        })
        .collect();
    if !failures.is_empty() {
        anyhow::bail!("could not remove {}", failures.join("; "));
    }
    let left = left_outside(paths, None)?;
    if let Some(first) = left.first() {
        anyhow::bail!(
            "could not remove {first} and {} more; the wipe will run again",
            left.len() - 1
        );
    }
    Ok(())
}

fn remove_installed_schedule(
    outside: &WipeOutsideState,
    remove_schedule: &impl Fn(&SchedulePlan) -> Result<()>,
) -> WipeReport {
    match &outside.schedule {
        None => WipeReport::default(),
        Some(plan) => match remove_schedule(plan) {
            Ok(()) => WipeReport { schedule_removed: true, schedule_error: None },
            Err(error) => {
                WipeReport { schedule_removed: false, schedule_error: Some(format!("{error:#}")) }
            }
        },
    }
}

/// Removes the intent, and a 0.5 marker beside it: neither is acted on again.
fn remove_records(paths: &ProjectPaths) -> Result<()> {
    for record in [intent_path(paths), legacy_marker_path(paths)] {
        remove_file_durably(&record).context("removing the wipe intent")?;
    }
    Ok(())
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

/// The control's name in the data folder, and in a trash that keeps one.
const CONTROL: &str = vault_layout::spec(vault_layout::ArtifactId::Config).path.relative();

fn intent_path(paths: &ProjectPaths) -> PathBuf {
    paths.app_root.join(WIPE_INTENT_FILE)
}

fn legacy_marker_path(paths: &ProjectPaths) -> PathBuf {
    paths.app_root.join(LEGACY_MARKER_FILE)
}

#[cfg(test)]
mod tests;
