//! Responsibilities: journal a verified multi-file installation, recover interrupted renames, and
//! keep what an install replaced: removed, left in its backup folder, or made a safety copy.
//! Not responsible for: SQL, keys, verification or choosing application files.
//! Failure modes: mixed generations, lost originals, stale WAL, crash during rollback,
//! configuration committed ahead of data, deleting the owner lock, a crash that leaves no
//! `config.json` (an older release then takes the folder for a first run), a release that does
//! not know this journal replaying it, a journal carried into another archive's folder removing or
//! replacing that archive's files, a rollback cut short that cannot be finished (a lockout), a
//! failed publish rolled back while the record on disk says published. Tests: install_tests.
//!
//! The control is not moved like the other entries. It is copied into the backup and replaced in
//! one rename on install, and from that copy by an atomic write on rollback, so `config.json`
//! exists at every point of the transaction.
//!
//! Where a file is says what the install did with it. The backup folder is made, under a new random
//! name, when the install begins, and only the install moves anything into it: an entry the backup
//! holds is an original the install moved out of its place, and the original of an entry the backup
//! does not hold was never moved, or has been put back. That alone decides every step of a
//! rollback. Before a rollback takes a file out of its place (the staged file the install put
//! there, so that the original can come back, or a file that was not there before), it checks that
//! the file holds what the journal recorded of the staged one, every byte (`content_identity`);
//! anything else there stops the rollback before anything moves, and the journal stays. The
//! control binds the journal to its folder: its bytes before and after the install are recorded,
//! and a rollback starts only where the control is one of the two. Every install moves
//! `config.json`, so a journal carried into another archive's folder, even with the backup folder
//! it names (two folders merged, or the journal's files restored over another archive), meets that
//! archive's control and changes nothing. A faithful copy of the whole folder, made by any tool,
//! with or without the files' times, recovers like the original.
//!
//! Every step of a rollback is one rename or one atomic write. A file taken out of its place is
//! renamed into the stage, which goes with the journal at the end, so a rollback cut short never
//! leaves a half-deleted folder that a later run could not recognise.
//!
//! A publish that reports an error may still have put the published record on disk (the flush
//! after its rename failed). The install then stops where it is, and the next recovery goes the way
//! the record on disk says, as after a crash; rolling back in memory could meet a record that says
//! published, and be finished the other way.
//!
//! The journals of PathKeep 0.5 record no identities. One of them is rolled back only where the
//! backup folder it names, randomly named when the install began, is in this folder, and never
//! beside a control with a stamp: that release cannot open such an archive, and this build replays
//! a 0.5 journal before it stamps one, so beside a stamp it was carried in. It is set aside.
//!
//! The journal is written under a name PathKeep 0.5 and earlier do not know, with an identity and
//! a version that are checked before anything is replayed; the name those releases wrote is still
//! read, as a record they may have left. Its stage and backup are recorded relative to the
//! folder.
//!
//! What a restore or an import replaces becomes a safety copy (the user's decision, 2026-11-15):
//! listed with the others, counted in storage usage, freed one by one in "Free up space", never
//! deleted on its own. Before that, PathKeep left it in the backup folder, which no screen shows
//! and nothing but "delete all data" removes, so every restore or import silently doubled the
//! archive's size. The journal names the copy, so a crash after the new archive is published still
//! ends with it.
use crate::{
    ProjectPaths, content_identity,
    durable_io::{atomic_durable_write, fsync_dir, remove_file_durably},
};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
};

const JOURNAL: &str =
    vault_layout::spec(vault_layout::ArtifactId::InstallJournalV2).path.relative();
/// The name PathKeep 0.5 and earlier gave the same journal.
const LEGACY_JOURNAL: &str =
    vault_layout::spec(vault_layout::ArtifactId::InstallJournal).path.relative();
const CONTROL: &str = vault_layout::spec(vault_layout::ArtifactId::Config).path.relative();
const IDENTITY: &str = "pathkeep-install";
const VERSION: u32 = 1;
/// Where a journal of PathKeep 0.5 or earlier is set aside when it cannot be this folder's: an
/// old-format import journal next to a native archive, or any of them beside a stamp.
const STALE_LEGACY_JOURNAL: &str =
    vault_layout::spec(vault_layout::ArtifactId::StaleImportJournal).path.relative();
/// The folder in the stage that a rollback renames files into when it takes them out of their
/// place. It goes with the stage.
const DISCARDED: &str = ".discarded";

/// Startup checks for recovery before it decides whether a native archive exists. A current
/// journal that cannot be read still counts: recovery reports it rather than ignoring it.
pub fn pending(paths: &ProjectPaths) -> bool {
    fs::symlink_metadata(paths.app_root.join(JOURNAL)).is_ok()
        || fs::read(paths.app_root.join(LEGACY_JOURNAL))
            .ok()
            .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
            .is_some_and(|journal| journal.get("stage").is_some())
}

/// Startup holds the owner reservation and waits for any in-process maintenance
/// before interpreting a journal. Normal live-Vault opens use the caller's gate.
pub fn recover_before_open(paths: &ProjectPaths) -> Result<bool> {
    let _operation = crate::archive::ArchiveOpGate::acquire(paths);
    recover(paths)
}

/// What becomes of the files an install replaces once the new ones are published.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Keep {
    /// Removed: the caller made its own copy before installing (a mode change seals one).
    Nothing,
    /// Left in the backup folder: the old-format conversion, whose backup holds only the control
    /// it replaced.
    Backup,
    /// Made a safety copy, offered for a restore like the others.
    SafetyCopy(Before),
}

/// The operations whose replaced archive is kept as a safety copy, by the name the restore list
/// shows it under (`RecoverySnapshot::source_op`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Before {
    Restore,
    Import,
}

impl Before {
    const ALL: [Before; 2] = [Before::Restore, Before::Import];

    pub fn op(self) -> &'static str {
        match self {
            Before::Restore => "restore",
            Before::Import => "import",
        }
    }

    fn prefix(self) -> String {
        format!("before-{}-", self.op())
    }

    /// The operation a copy's folder name says it was made before.
    pub(crate) fn of_copy(name: &str) -> Option<Before> {
        Self::ALL.into_iter().find(|before| name.starts_with(&before.prefix()))
    }

    /// Where the files an install replaced become a copy: beside the other copies, under the random
    /// part of the backup folder's name, so a recovery from the journal finds the same place.
    fn copy_of(self, backup: &Path) -> Option<PathBuf> {
        let name = backup.to_str()?;
        let random = name.strip_prefix(vault_layout::family_prefix(
            vault_layout::ArtifactId::PreviousArchives,
        ))?;
        let copies = vault_layout::spec(vault_layout::ArtifactId::NativeSnapshots).path.relative();
        Some(Path::new(copies).join(format!("{}{random}", self.prefix())))
    }
}

#[derive(Serialize, Deserialize, schemars::JsonSchema)]
pub(crate) struct Entry {
    relative: PathBuf,
    previous: bool,
    staged: bool,
    /// What the staged file holds (`content_identity`), recorded before it moves into place: a
    /// rollback takes out of the place only a file that holds this. Absent when nothing is staged,
    /// and in the journals of PathKeep 0.5.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    installed: Option<String>,
    /// What the control held before the install, for the control only. With `installed`, what
    /// binds the journal to its folder.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    original: Option<String>,
}

/// A durable prepare record is the rollback decision; `committed` publishes the
/// entire generation. Recovery uses the same entries as in-band installation.
#[derive(Serialize, Deserialize, schemars::JsonSchema)]
pub(crate) struct Journal {
    /// `pathkeep-install`. Absent in the journals PathKeep 0.5 and earlier wrote.
    #[serde(default)]
    identity: String,
    /// The version of this record's meaning. Absent (0) in the journals of 0.5 and earlier.
    #[serde(default)]
    version: u32,
    /// The stage, relative to the archive folder. PathKeep 0.5 and earlier wrote absolute paths.
    stage: PathBuf,
    /// The folder the replaced files are moved into, relative to the archive folder. Absolute in
    /// the journals of 0.5 and earlier.
    backup: PathBuf,
    entries: Vec<Entry>,
    keep_previous: bool,
    /// The safety copy the replaced files become once the new ones are published, relative to the
    /// archive folder (`Keep::SafetyCopy`). Absent when they are left in the backup or removed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    kept_as: Option<PathBuf>,
    committed: bool,
}

impl Journal {
    /// Written by this build's line: every entry says what its staged file holds.
    fn current(&self) -> bool {
        self.identity == IDENTITY
    }
}

/// A journal's stage, backup and copy in this folder, once it is clear they are this folder's.
struct Located {
    stage: PathBuf,
    backup: PathBuf,
    kept_as: Option<PathBuf>,
}

/// Places a journal's stage and backup in this folder. Every path in it must be one this folder's
/// installs produce: a backup named like `maintenance.bak-*` directly in the folder, a stage
/// under `staging/`, entries that stay inside the folder and outside the stage, and a copy named
/// after the backup among the other copies. A 0.5 journal's absolute paths are taken relative to
/// where its backup was, so a folder moved since is still recovered.
fn locate(paths: &ProjectPaths, journal: &Journal) -> Result<Located> {
    use std::path::Component;
    let foreign = |what: &str| {
        anyhow::anyhow!(
            "{}: the install record's {what} is not one of this folder's; nothing was changed",
            crate::admission::Reason::ControlInvalid.code()
        )
    };
    let plain = |path: &Path| {
        !path.as_os_str().is_empty()
            && path.components().all(|part| matches!(part, Component::Normal(_)))
    };
    let current = journal.current();
    if current == journal.backup.is_absolute() || current == journal.stage.is_absolute() {
        return Err(foreign("form"));
    }
    let (backup, stage) = if current {
        (journal.backup.clone(), journal.stage.clone())
    } else {
        let old_root = journal.backup.parent().ok_or_else(|| foreign("backup"))?;
        let name = journal.backup.file_name().ok_or_else(|| foreign("backup"))?;
        let stage = journal.stage.strip_prefix(old_root).map_err(|_| foreign("stage"))?;
        (PathBuf::from(name), stage.to_path_buf())
    };
    let backup_family = vault_layout::family_pattern(vault_layout::ArtifactId::PreviousArchives);
    let staging = vault_layout::spec(vault_layout::ArtifactId::Staging).path.relative();
    if !plain(&backup)
        || backup.components().count() != 1
        || !backup_family.matches(&backup.to_string_lossy())
    {
        return Err(foreign("backup"));
    }
    if !plain(&stage) || stage.components().count() < 2 || !stage.starts_with(staging) {
        return Err(foreign("stage"));
    }
    // The stage is where a rollback renames what it takes out of place; no entry lives there.
    if !journal
        .entries
        .iter()
        .all(|entry| plain(&entry.relative) && !entry.relative.starts_with(&stage))
    {
        return Err(foreign("list of files"));
    }
    let kept_as = match &journal.kept_as {
        None => None,
        Some(copy)
            if Before::ALL.iter().any(|before| before.copy_of(&backup).as_ref() == Some(copy)) =>
        {
            Some(paths.app_root.join(copy))
        }
        Some(_) => return Err(foreign("safety copy")),
    };
    Ok(Located { stage: paths.app_root.join(stage), backup: paths.app_root.join(backup), kept_as })
}

pub(super) fn prepare(
    paths: &ProjectPaths,
    stage: &Path,
    files: Vec<PathBuf>,
    keep: Keep,
) -> Result<Journal> {
    sync_tree(stage)?;
    // Compared as absolute paths: the folder may have been given relative to the working
    // directory, and a stage made by `tempfile` is always absolute.
    let stage_relative = std::path::absolute(stage)?
        .strip_prefix(std::path::absolute(&paths.app_root)?)
        .context("an install stage outside the archive folder")?
        .to_path_buf();
    let entries = files
        .into_iter()
        .map(|relative| entry(paths, stage, relative))
        .collect::<Result<Vec<_>>>()?;
    // Removing the control is not something an install does: a replacement brings its own.
    if entries.iter().any(|entry| is_control(entry) && entry.previous && !entry.staged) {
        anyhow::bail!("an installation without a config.json would remove the archive's control");
    }
    let backup = tempfile::Builder::new()
        .prefix(vault_layout::family_prefix(vault_layout::ArtifactId::PreviousArchives))
        .tempdir_in(&paths.app_root)?
        .keep();
    // The backup folder's own entry is durable before the journal names it, and before the only
    // copy of anything is moved into it.
    fsync_dir(&paths.app_root)?;
    let backup_relative = PathBuf::from(backup.file_name().context("a backup folder name")?);
    let kept_as = match keep {
        Keep::SafetyCopy(before) => {
            Some(before.copy_of(&backup_relative).context("a backup folder name")?)
        }
        Keep::Nothing | Keep::Backup => None,
    };
    let journal = Journal {
        identity: IDENTITY.into(),
        version: VERSION,
        stage: stage_relative,
        backup: backup_relative,
        entries,
        keep_previous: keep != Keep::Nothing,
        kept_as,
        committed: false,
    };
    save(paths, &journal)?;
    Ok(journal)
}

/// One file of an install, with what its staged file holds, read once the stage is on disk.
fn entry(paths: &ProjectPaths, stage: &Path, relative: PathBuf) -> Result<Entry> {
    let live = paths.app_root.join(&relative);
    let installed = content_identity::of(&stage.join(&relative))
        .with_context(|| format!("reading the staged {}", relative.display()))?;
    let original = if relative == Path::new(CONTROL) {
        content_identity::of(&live).context("reading the control")?
    } else {
        None
    };
    Ok(Entry {
        previous: exists(&live)?,
        staged: installed.is_some(),
        relative,
        installed,
        original,
    })
}

/// Whether anything is at `path`, a symlink included. Only "not found" is absence: a file that
/// cannot be looked at is not taken for one that is not there.
pub(crate) fn exists(path: &Path) -> Result<bool> {
    match fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => {
            Err(anyhow::Error::new(error).context(format!("looking at {}", path.display())))
        }
    }
}

pub(super) fn sync_tree(root: &Path) -> Result<()> {
    let mut directories = Vec::new();
    for entry in walkdir::WalkDir::new(root) {
        let entry = entry?;
        if entry.file_type().is_file() {
            crate::durable_io::fsync_file_durably(entry.path())?;
        } else if entry.file_type().is_dir() {
            directories.push(entry.into_path());
        }
    }
    for directory in directories.into_iter().rev() {
        fsync_dir(&directory)?;
    }
    fsync_dir(root.parent().unwrap())
}

fn save(paths: &ProjectPaths, journal: &Journal) -> Result<()> {
    atomic_durable_write(&paths.app_root.join(JOURNAL), &serde_json::to_vec(journal)?)
}

pub(super) fn publish(paths: &ProjectPaths, journal: &mut Journal) -> Result<()> {
    journal.committed = true;
    save(paths, journal)?;
    // Where a flush that fails after the rename reports: the published record is on disk.
    crate::fault_inject::checkpoint("native-install-publish-saved")
}

pub(super) fn install_step(paths: &ProjectPaths, journal: &mut Journal, step: usize) -> Result<()> {
    let located = locate(paths, journal)?;
    let entry = &journal.entries[step / 2];
    let target = paths.app_root.join(&entry.relative);
    if step % 2 == 0 {
        if entry.previous && is_control(entry) {
            // Copied, not moved: the live control stays until its replacement lands on it.
            let bytes = fs::read(&target)?;
            let backup = located.backup.join(&entry.relative);
            create_dirs_durably(backup.parent().unwrap())?;
            atomic_durable_write(&backup, &bytes)?;
        } else if entry.previous {
            rename(&target, &located.backup.join(&entry.relative))?;
        }
        crate::fault_inject::checkpoint("native-install-preserved")?;
    } else if entry.staged {
        rename(&located.stage.join(&entry.relative), &target)?;
        crate::fault_inject::checkpoint("native-install-installed")?;
    }
    Ok(())
}

/// Caller has verified staged files and closed all live handles before entering, and holds the
/// control's gate: the install replaces the control, and the caller wrote the stage's from the
/// live one under the same hold.
///
/// Returns where the replaced files are kept: the safety copy, or the backup folder. Neither exists
/// when nothing was replaced, or when `keep` is `Keep::Nothing`.
///
/// After an error the live files are as the record on disk says: rolled back when a step failed;
/// as they are when the publish failed, for the next recovery to finish or roll back. A caller
/// does not touch them either way.
pub fn install(
    gate: &crate::ControlGate,
    stage: &Path,
    files: Vec<PathBuf>,
    keep: Keep,
) -> Result<PathBuf> {
    let paths = gate.paths();
    let mut journal = prepare(paths, stage, files, keep)?;
    let moved = (|| {
        for step in 0..journal.entries.len() * 2 {
            install_step(paths, &mut journal, step)?;
        }
        Ok::<_, anyhow::Error>(())
    })();
    if let Err(error) = moved {
        // The record on disk says not published: a recovery would roll back too.
        rollback(paths, &journal)?;
        return Err(error);
    }
    publish(paths, &mut journal)?;
    crate::fault_inject::checkpoint("native-install-published")?;
    finish(paths, &journal)?;
    Ok(paths.app_root.join(journal.kept_as.as_ref().unwrap_or(&journal.backup)))
}

/// Run under the owner's lock before opening any database. A pending generation
/// rolls back; a published generation finishes cleanup. Both paths are idempotent.
///
/// A journal PathKeep 0.5 or earlier left is replayed first, as those releases would have: the
/// folder it was written in has no stamp, so this build is allowed to finish their work.
pub fn recover(paths: &ProjectPaths) -> Result<bool> {
    // A journal is not replayed in an archive that needs a newer PathKeep. A damaged control
    // does not stop it: rolling the install back is what puts the previous one back.
    crate::admission::refuse_newer(&paths.app_root)?;
    // A rollback writes the control back, and the bring-up's record is about the control too.
    let _control = crate::ControlGate::acquire(paths);
    let mut recovered = recover_legacy(paths)?;
    // The same file holds the record of stores being created when an archive is brought up to
    // this build's layout (`creation`), under its own identity.
    if let Some(record) = super::creation::read(paths)? {
        super::creation::recover(paths, record)?;
        return Ok(true);
    }
    let path = paths.app_root.join(JOURNAL);
    let bytes = match fs::read(&path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(recovered),
        Err(error) => return Err(error.into()),
    };
    // Nothing is replayed from a record this build did not write in this shape.
    let journal = serde_json::from_slice::<Journal>(&bytes)
        .ok()
        .filter(|journal| journal.current() && journal.version == VERSION)
        .ok_or_else(|| {
            anyhow::anyhow!(
                "{}: the install record {} is not one this build can replay",
                crate::admission::Reason::ControlInvalid.code(),
                path.display()
            )
        })?;
    replay(paths, &journal, &path)?;
    recovered = true;
    Ok(recovered)
}

/// The journal under the name PathKeep 0.5 and earlier used.
fn recover_legacy(paths: &ProjectPaths) -> Result<bool> {
    let path = paths.app_root.join(LEGACY_JOURNAL);
    let bytes = match fs::read(&path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error.into()),
    };
    if serde_json::from_slice::<serde_json::Value>(&bytes)?.get("stage").is_none() {
        // An import of the old SQLCipher format left this. Before conversion the converter's
        // launch recovery rolls it back. Once the archive has been converted it predates the
        // native files; rolling it back would put the old config.json over them, so keep it
        // aside instead of acting on it.
        if paths.archive_db.exists() {
            rename(&path, &paths.app_root.join(STALE_LEGACY_JOURNAL))?;
        }
        return Ok(false);
    }
    if stamped(paths) {
        // PathKeep 0.5 does not open an archive with a stamp, and this build replays a 0.5
        // journal before it stamps one: this one was carried in. Nothing it names is touched.
        rename(&path, &paths.app_root.join(STALE_LEGACY_JOURNAL))?;
        return Ok(false);
    }
    let journal: Journal = serde_json::from_slice(&bytes)?;
    replay(paths, &journal, &path)?;
    Ok(true)
}

/// Whether the folder's control carries a stamp. One that cannot be read or parsed says nothing
/// either way, and a 0.5 journal beside it is replayed as before.
fn stamped(paths: &ProjectPaths) -> bool {
    crate::admission::read_control(&paths.app_root)
        .ok()
        .flatten()
        .and_then(|bytes| crate::admission::parse_control(&paths.app_root, &bytes).ok())
        .is_some_and(|control| control.compatibility.is_some())
}

fn replay(paths: &ProjectPaths, journal: &Journal, record: &Path) -> Result<()> {
    if journal.committed {
        finish_at(paths, journal, record)
    } else {
        rollback_at(paths, journal, record)
    }
}

fn is_control(entry: &Entry) -> bool {
    entry.relative == Path::new(CONTROL)
}

fn rollback(paths: &ProjectPaths, journal: &Journal) -> Result<()> {
    rollback_at(paths, journal, &paths.app_root.join(JOURNAL))
}

/// What a rollback does with one entry. Every entry's is decided before anything moves, so a
/// journal that does not describe this folder changes nothing in it.
enum Undo {
    /// The original is in place, or nothing was there and nothing is.
    Leave,
    /// Nothing was there before: the installed file is taken out of its place.
    Discard,
    /// The original comes back from the backup, after what is in its place, the installed file,
    /// is taken out.
    PutBack,
    /// The control's copy in the backup is written over the control.
    WriteControl,
}

fn rollback_at(paths: &ProjectPaths, journal: &Journal, record: &Path) -> Result<()> {
    let located = locate(paths, journal)?;
    let plan = undo_plan(paths, journal, &located)?;
    for (entry, undo) in journal.entries.iter().rev().zip(plan) {
        let backup = located.backup.join(&entry.relative);
        let target = paths.app_root.join(&entry.relative);
        match undo {
            Undo::Leave => {}
            Undo::Discard => discard(&located, entry, &target)?,
            Undo::PutBack => {
                discard(&located, entry, &target)?;
                rename(&backup, &target)?;
            }
            // Written over the live control from the copy, in the control's own folder. A journal
            // of PathKeep 0.5 moved the control into the backup, so the live one may be missing;
            // the write covers both.
            Undo::WriteControl => atomic_durable_write(&target, &fs::read(&backup)?)?,
        }
        crate::fault_inject::checkpoint("native-install-undone")?;
    }
    remove_file_durably(record)?;
    remove(&located.stage)?;
    remove(&located.backup)
}

/// Takes what is at `target` out of its place in one rename, into the stage. A rollback cut short
/// before the rename finds it still there, one cut short after finds the place empty: never half
/// of a folder.
fn discard(located: &Located, entry: &Entry, target: &Path) -> Result<()> {
    if !exists(target)? {
        return Ok(());
    }
    let away = located.stage.join(DISCARDED).join(&entry.relative);
    remove(&away)?;
    rename(target, &away)?;
    crate::fault_inject::checkpoint("native-install-discarded")
}

/// Every entry's undo, the last installed first; or why none can be done, with nothing moved.
fn undo_plan(paths: &ProjectPaths, journal: &Journal, located: &Located) -> Result<Vec<Undo>> {
    // Without identities (a journal of PathKeep 0.5) the backup folder binds the journal to its
    // folder: it exists from the moment the journal does until the journal is gone, and its name is
    // random. Without it here, this is another folder's journal: it would delete files of this one
    // that it takes for its own new ones.
    if !journal.current() && !located.backup.is_dir() {
        anyhow::bail!(
            "{}: the install record names a backup folder that is not in this folder; nothing was \
             changed",
            crate::admission::Reason::ControlInvalid.code()
        );
    }
    journal
        .entries
        .iter()
        .rev()
        .map(|entry| {
            let target = paths.app_root.join(&entry.relative);
            let backup = located.backup.join(&entry.relative);
            if !journal.current() {
                undo_legacy(entry, &target, &backup)
            } else if is_control(entry) {
                undo_control(entry, &target, &backup)
            } else {
                undo_current(entry, &target, &backup)
            }
        })
        .collect()
}

/// A file of a current journal, decided by where it is. In the backup: the install moved the
/// original out, and it comes back over the staged file or an empty place. Not in the backup: the
/// original was never moved, or is back; a file that was not there before goes only if it is the
/// staged one.
fn undo_current(entry: &Entry, target: &Path, backup: &Path) -> Result<Undo> {
    if exists(backup)? {
        return if entry.previous && (!exists(target)? || holds_installed(entry, target)?) {
            Ok(Undo::PutBack)
        } else {
            Err(not_moved(entry))
        };
    }
    match (entry.previous, exists(target)?) {
        (true, true) | (false, false) => Ok(Undo::Leave),
        (true, false) => Err(lost(entry)),
        (false, true) if holds_installed(entry, target)? => Ok(Undo::Discard),
        (false, true) => Err(not_moved(entry)),
    }
}

/// The control of a current journal: never absent while a current journal exists (copied into the
/// backup, replaced by one rename, written back atomically), so it must be the one from before the
/// install or the one the install brought. Anything else, or nothing, means this is not the
/// journal's folder.
fn undo_control(entry: &Entry, target: &Path, backup: &Path) -> Result<Undo> {
    let here = content_identity::of(target)?;
    if !entry.previous {
        // The folder had no control (a restore repairing it): the install brought the only one.
        return match here {
            None => Ok(Undo::Leave),
            Some(_) if here == entry.installed => Ok(Undo::Discard),
            Some(_) => Err(not_moved(entry)),
        };
    }
    if here.is_none() || (here != entry.original && here != entry.installed) {
        return Err(not_moved(entry));
    }
    match content_identity::of(backup)? {
        None if here == entry.original => Ok(Undo::Leave),
        Some(kept) if Some(&kept) == entry.original.as_ref() => Ok(Undo::WriteControl),
        _ => Err(not_moved(entry)),
    }
}

/// Whether the file at `target` holds what the journal recorded of its staged file. A current
/// journal records it for every staged file; without it nothing tells this install's file from
/// another one.
fn holds_installed(entry: &Entry, target: &Path) -> Result<bool> {
    Ok(entry.installed.is_some() && content_identity::of(target)? == entry.installed)
}

/// A journal of PathKeep 0.5 puts back whatever its backup holds and takes out what was not there
/// before, as that release did.
fn undo_legacy(entry: &Entry, target: &Path, backup: &Path) -> Result<Undo> {
    Ok(if exists(backup)? && is_control(entry) {
        Undo::WriteControl
    } else if exists(backup)? {
        Undo::PutBack
    } else if !entry.previous {
        Undo::Discard
    } else if !exists(target)? {
        return Err(lost(entry));
    } else {
        Undo::Leave
    })
}

fn not_moved(entry: &Entry) -> anyhow::Error {
    anyhow::anyhow!(
        "{}: {} is not a file this install moved; the install record is kept and nothing was \
         changed",
        crate::admission::Reason::ControlInvalid.code(),
        entry.relative.display()
    )
}

/// It was there before the install, and it is neither in place nor in the backup. Saying the
/// rollback succeeded would lose track of it for good.
fn lost(entry: &Entry) -> anyhow::Error {
    anyhow::anyhow!(
        "{} is neither in place nor in the backup; the install record is kept",
        entry.relative.display()
    )
}

fn finish(paths: &ProjectPaths, journal: &Journal) -> Result<()> {
    finish_at(paths, journal, &paths.app_root.join(JOURNAL))
}

fn finish_at(paths: &ProjectPaths, journal: &Journal, record: &Path) -> Result<()> {
    // Finishing only removes the stage, the backup and the record, and moves the backup to its
    // copy, each at a name this folder's installs produce.
    let located = locate(paths, journal)?;
    match &located.kept_as {
        Some(copy) => keep_as_copy(&located.backup, copy)?,
        None if !journal.keep_previous => remove(&located.backup)?,
        None => {}
    }
    remove_file_durably(record)?;
    remove(&located.stage)
}

/// Makes the backup of a published install a safety copy. Run again after a crash, it finishes
/// what is left: the move is one rename, and the seal is written whole or not at all.
fn keep_as_copy(backup: &Path, copy: &Path) -> Result<()> {
    if backup.is_dir() {
        let replaced = walkdir::WalkDir::new(backup)
            .into_iter()
            .any(|entry| entry.map_or(true, |entry| !entry.file_type().is_dir()));
        if !replaced {
            // Nothing was there to replace (an import into an empty folder): no copy of nothing.
            return remove(backup);
        }
        if copy.exists() {
            // The copy is named after the backup folder, whose name was new when the install began,
            // so this does not happen. If it does, the replaced files stay where they are rather
            // than go over another copy.
            eprintln!(
                "PathKeep install: {} is already there; the replaced files stay in {}",
                copy.display(),
                backup.display()
            );
            return Ok(());
        }
        rename(backup, copy)?;
    }
    if copy.is_dir() && !copy.join(super::snapshot::SEAL).exists() {
        // A replaced archive that cannot be sealed, usually because its control is what was
        // damaged, cannot be restored either. It stays, counted and freed with the other copies,
        // and the restore list does not offer it.
        if let Err(error) = super::snapshot::seal_replaced(copy) {
            eprintln!(
                "PathKeep install: the replaced archive kept in {} could not be sealed: {error:#}",
                copy.display()
            );
        }
    }
    Ok(())
}

/// Moves a file or folder and makes the move durable. The destination's directory is flushed
/// first: if power fails between the two flushes the entry may show in both places, never in
/// neither (`durable_io::fsync_dir`). New parent folders are made durable before anything is
/// moved into them.
pub(crate) fn rename(from: &Path, to: &Path) -> Result<()> {
    create_dirs_durably(to.parent().unwrap())?;
    fs::rename(from, to)?;
    fsync_dir(to.parent().unwrap())?;
    fsync_dir(from.parent().unwrap())
}

/// Creates `dir` and any missing parents, each one's entry flushed in its own parent.
fn create_dirs_durably(dir: &Path) -> Result<()> {
    if dir.is_dir() {
        return Ok(());
    }
    let parent = dir.parent().context("a folder without a parent")?;
    create_dirs_durably(parent)?;
    match fs::create_dir(dir) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(error) => return Err(error.into()),
    }
    fsync_dir(parent)
}

pub(super) fn remove(path: &Path) -> Result<()> {
    match fs::symlink_metadata(path) {
        Ok(meta) => {
            if meta.is_dir() {
                fs::remove_dir_all(path)?;
            } else {
                fs::remove_file(path)?;
            }
            fsync_dir(path.parent().unwrap())?;
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(e.into()),
    }
    Ok(())
}
