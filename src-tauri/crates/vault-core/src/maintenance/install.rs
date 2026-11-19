//! Responsibilities: journal a verified multi-file installation, recover interrupted renames, and
//! keep what an install replaced: removed, left in its backup folder, or made a safety copy.
//! Not responsible for: SQL, keys, verification or choosing application files.
//! Failure modes: mixed generations, lost originals, stale WAL, crash during rollback,
//! configuration committed ahead of data, deleting the owner lock, a crash that leaves no
//! `config.json` (an older release then takes the folder for a first run), a release that does
//! not know this journal replaying it, a journal carried into another archive's folder removing or
//! replacing that archive's files, a rollback cut short that cannot be finished (a lockout), a
//! failed publish rolled back while the record on disk says published, files of the new archive in
//! place without its control, a published journal finished over a copy of the folder that holds
//! something else, a control that is a link, a journal another system reads otherwise. Tests:
//! install_tests.
//!
//! The control is not moved like the other entries. It is copied into the backup and replaced in
//! one rename on install, and from that copy by an atomic write on rollback, so `config.json`
//! exists at every point of the transaction. It is also the first entry of every journal: an
//! install puts it in place before any other file, and a rollback, going the other way, takes it
//! away last. PathKeep 0.5 opens a folder whose control is missing or has no stamp, and between two
//! steps it would otherwise find files of the new archive with nothing keeping it out. A control
//! that is a link is refused before the journal is written: the journal would compare the link and
//! copy the file it points to, and the install would put a file where the link was.
//!
//! Where a file is says what the install did with it, and what it holds confirms it. The backup
//! folder is made, under a new random name, when the install begins, and only the install moves
//! anything into it: an entry the backup holds is an original the install moved out of its place,
//! and the original of an entry the backup does not hold was never moved, or has been put back.
//! The journal records what every file held before the install and what every staged file holds,
//! every byte (`content_identity`), before anything moves. A rollback puts back only an original
//! that still holds what it held (a copy of the folder cut short leaves a shorter file under the
//! same name), leaves in place only a file that holds the original, and takes out of its place
//! only the staged file; anything else stops the rollback before anything moves, and the journal
//! stays. The control binds the journal to its folder: a rollback starts only where the control is
//! one of the two it recorded, or where a cut rename left none and the backup holds its copy. Every
//! install moves `config.json`, so a journal carried into another archive's folder, even with the
//! backup folder it names (two folders merged, or the journal's files restored over another
//! archive), meets that archive's control and changes nothing. A faithful copy of the whole
//! folder, made by any tool, with or without the files' times, recovers like the original.
//!
//! Every step of a rollback is one rename or one atomic write, and no rename replaces anything. A
//! file taken out of its place is renamed into the stage under a name nothing has, and the stage
//! goes with the journal at the end, so a rollback cut short never leaves a half-deleted folder
//! that a later run could not recognise.
//!
//! On a file system without a journal (exFAT, FAT32), a power cut during a rename can leave the
//! file under both names, each pointing at the same data. Deleting one name then frees data the
//! other still uses, and the next file written there overwrites the archive. Where a rename between
//! a file left in place and another name of it may have been cut (the entry is in place and in the
//! backup while its staged or discarded name is still there, or the file in place is the original),
//! that other name goes into the quarantine (`quarantine/<backup folder name>/`), where the storage
//! screen counts it and the user frees it, rather than with the stage or the backup. Two names of a
//! file that leaves its place go together with the stage. The rename that makes a published
//! install's backup a safety copy can leave it under both names too: the copy is made, and the
//! backup's name goes into the quarantine.
//!
//! A publish that reports an error may still have put the published record on disk (the flush
//! after its rename failed). The install then stops where it is, and the next recovery goes the way
//! the record on disk says, as after a crash; rolling back in memory could meet a record that says
//! published, and be finished the other way.
//!
//! A published journal says the install finished in the folder it was written in, not that this
//! folder holds the result: it may be a copy made by a tool that copied some files before the
//! publish and some after. Before the stage and the backup go, recovery checks every entry
//! (`complete_published`): a staged file is in place holding what was staged, or, with nothing in
//! its place, the stage still holds it, and it is moved in; a file the install removed is gone from
//! its place, or holds what it held and goes back to the backup (to the quarantine where the backup
//! has a name of it already). A staged file in place whose stage still holds it may be one file
//! under two names: the stage's name goes into the quarantine. Anything else stops with every
//! record kept. An install finishing in the same call does not read the files again: there was no
//! gap in which anything could change.
//!
//! The journals of PathKeep 0.5 record no identities. One of them is rolled back only where the
//! backup folder it names, randomly named when the install began, is in this folder, and never
//! beside a control with a stamp: that release cannot open such an archive, and this build replays
//! a 0.5 journal before it stamps one, so beside a stamp it was carried in. It is set aside.
//!
//! The journal is written under a name PathKeep 0.5 and earlier do not know, with an identity and
//! a version that are checked before anything is replayed; the name those releases wrote is still
//! read, as a record they may have left. Every path in it is relative to the folder: plain names
//! joined by `/` (`recorded_path`), read the same on every system. A 0.5 journal keeps the native
//! absolute paths it was written with (`LegacyJournal`).
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
    recorded_path::RecordedPath,
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
    fn copy_of(self, backup: &Path) -> Option<RecordedPath> {
        let name = backup.to_str()?;
        let random = name.strip_prefix(vault_layout::family_prefix(
            vault_layout::ArtifactId::PreviousArchives,
        ))?;
        let copies = vault_layout::spec(vault_layout::ArtifactId::NativeSnapshots).path.relative();
        RecordedPath::new(&Path::new(copies).join(format!("{}{random}", self.prefix()))).ok()
    }
}

#[derive(Serialize, Deserialize, schemars::JsonSchema)]
pub(crate) struct Entry {
    relative: RecordedPath,
    previous: bool,
    staged: bool,
    /// What the staged file holds (`content_identity`), recorded before it moves into place: a
    /// rollback takes out of the place only a file that holds this. Absent when nothing is staged,
    /// and in the journals of PathKeep 0.5.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    installed: Option<String>,
    /// What the file in place held before the install, for every file that was there: a rollback
    /// puts back only a file that holds this, and leaves in place only a file that holds this. For
    /// the control, with `installed`, what binds the journal to its folder.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    original: Option<String>,
}

/// A durable prepare record is the rollback decision; `committed` publishes the
/// entire generation. Recovery uses the same entries as in-band installation. Every path is
/// relative to the archive folder, in the form every system reads the same (`RecordedPath`).
#[derive(Serialize, Deserialize, schemars::JsonSchema)]
pub(crate) struct Journal {
    /// `pathkeep-install`. Empty for a journal PathKeep 0.5 wrote (`LegacyJournal`).
    identity: String,
    /// The version of this record's meaning. 0 for a journal of 0.5.
    version: u32,
    stage: RecordedPath,
    /// The folder the replaced files are moved into.
    backup: RecordedPath,
    entries: Vec<Entry>,
    keep_previous: bool,
    /// The safety copy the replaced files become once the new ones are published
    /// (`Keep::SafetyCopy`). Absent when they are left in the backup or removed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    kept_as: Option<RecordedPath>,
    committed: bool,
}

/// The journal as PathKeep 0.5 and earlier wrote it, under its own name: the stage and the backup
/// as absolute paths, the files in the system's own form, and no identities.
#[derive(Deserialize)]
struct LegacyJournal {
    stage: PathBuf,
    backup: PathBuf,
    entries: Vec<LegacyEntry>,
    keep_previous: bool,
    committed: bool,
}

#[derive(Deserialize)]
struct LegacyEntry {
    relative: PathBuf,
    previous: bool,
    staged: bool,
}

impl LegacyJournal {
    /// Its paths taken relative to where its backup was, so a folder moved since is still
    /// recovered. A journal whose paths do not have that form is not one 0.5 wrote.
    fn into_journal(self) -> Result<Journal> {
        if !self.backup.is_absolute() || !self.stage.is_absolute() {
            return Err(foreign("form"));
        }
        let old_root = self.backup.parent().ok_or_else(|| foreign("backup"))?;
        let name = self.backup.file_name().ok_or_else(|| foreign("backup"))?;
        let stage = self.stage.strip_prefix(old_root).map_err(|_| foreign("stage"))?;
        let recorded = |path: &Path, what: &str| RecordedPath::new(path).map_err(|_| foreign(what));
        let entries = self
            .entries
            .into_iter()
            .map(|entry| {
                Ok(Entry {
                    relative: recorded(&entry.relative, "list of files")?,
                    previous: entry.previous,
                    staged: entry.staged,
                    installed: None,
                    original: None,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(Journal {
            identity: String::new(),
            version: 0,
            stage: recorded(stage, "stage")?,
            backup: recorded(Path::new(name), "backup")?,
            entries,
            keep_previous: self.keep_previous,
            kept_as: None,
            committed: self.committed,
        })
    }
}

fn foreign(what: &str) -> anyhow::Error {
    anyhow::anyhow!(
        "{}: the install record's {what} is not one of this folder's; nothing was changed",
        crate::admission::Reason::ControlInvalid.code()
    )
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
/// under `staging/`, entries outside the stage, and a copy named after the backup among the other
/// copies. Each is already a relative path of plain names, so it stays inside the folder.
fn locate(paths: &ProjectPaths, journal: &Journal) -> Result<Located> {
    let (backup, stage) = (&journal.backup, &journal.stage);
    let backup_family = vault_layout::family_pattern(vault_layout::ArtifactId::PreviousArchives);
    let staging = vault_layout::spec(vault_layout::ArtifactId::Staging).path.relative();
    if backup.components().count() != 1 || !backup_family.matches(&backup.to_string_lossy()) {
        return Err(foreign("backup"));
    }
    if stage.components().count() < 2 || !stage.starts_with(staging) {
        return Err(foreign("stage"));
    }
    // The stage is where a rollback renames what it takes out of place; no entry lives there.
    if journal.entries.iter().any(|entry| entry.relative.starts_with(&**stage)) {
        return Err(foreign("list of files"));
    }
    let kept_as = match &journal.kept_as {
        None => None,
        Some(copy)
            if Before::ALL.iter().any(|before| before.copy_of(backup).as_ref() == Some(copy)) =>
        {
            Some(paths.app_root.join(copy))
        }
        Some(_) => return Err(foreign("safety copy")),
    };
    Ok(Located { stage: paths.app_root.join(stage), backup: paths.app_root.join(backup), kept_as })
}

#[cfg(test)]
pub(super) fn prepare(
    paths: &ProjectPaths,
    stage: &Path,
    files: Vec<PathBuf>,
    keep: Keep,
) -> Result<Journal> {
    prepare_bound(paths, stage, files, keep, None)
}

pub(crate) type Originals = std::collections::BTreeMap<PathBuf, Option<String>>;

/// Capture the frozen inputs the caller is preserving, before it releases them.
pub(crate) fn originals(root: &Path, members: &[PathBuf]) -> std::io::Result<Originals> {
    members.iter().map(|p| Ok((p.clone(), content_identity::of(&root.join(p))?))).collect()
}

fn prepare_bound(
    paths: &ProjectPaths,
    stage: &Path,
    files: Vec<PathBuf>,
    keep: Keep,
    expected: Option<&Originals>,
) -> Result<Journal> {
    // The journal compares the control by what is at its name and copies it as bytes; a link would
    // be compared as a link, and the install would put a file where it was.
    if fs::symlink_metadata(paths.app_root.join(CONTROL)).is_ok_and(|meta| meta.is_symlink()) {
        anyhow::bail!(
            "{}: config.json is a link to another file; nothing was installed",
            crate::admission::Reason::ControlInvalid.code()
        );
    }
    sync_tree(stage)?;
    // Compared as absolute paths: the folder may have been given relative to the working
    // directory, and a stage made by `tempfile` is always absolute.
    let stage_relative = RecordedPath::new(
        std::path::absolute(stage)?
            .strip_prefix(std::path::absolute(&paths.app_root)?)
            .context("an install stage outside the archive folder")?,
    )?;
    let mut entries = files
        .into_iter()
        .map(|relative| entry(paths, stage, relative))
        .collect::<Result<Vec<_>>>()?;
    if let Some(expected) = expected {
        for entry in &entries {
            anyhow::ensure!(
                expected.get(&entry.relative.to_path_buf()) == Some(&entry.original),
                "installation-source-changed: {} differs from the preserved original",
                entry.relative.display()
            );
        }
    }
    // The control goes first, so the rollback, which goes the other way, undoes it last: no file
    // of the new archive is ever in place without the control that keeps PathKeep 0.5 out.
    entries.sort_by_key(|entry| !is_control(entry));
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
    let backup_relative =
        RecordedPath::new(Path::new(backup.file_name().context("a backup folder name")?))?;
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

/// One file of an install, with what its staged file and the file it replaces hold, read once the
/// stage is on disk and before anything moves.
fn entry(paths: &ProjectPaths, stage: &Path, relative: PathBuf) -> Result<Entry> {
    let relative = RecordedPath::new(&relative)?;
    let live = paths.app_root.join(&relative);
    let installed = content_identity::of(&stage.join(&relative))
        .with_context(|| format!("reading the staged {}", relative.display()))?;
    let original = content_identity::of(&live)
        .with_context(|| format!("reading the {} it replaces", relative.display()))?;
    Ok(Entry {
        previous: original.is_some(),
        staged: installed.is_some(),
        relative,
        installed,
        original,
    })
}

/// Whether the folder holds a record of this build of an install that brings a control. Such a
/// folder without a control is an interrupted install, not an archive that lost its control
/// (`admission::census`): an import into a new folder and a restore repairing a folder without a
/// control have none until the staged one lands, and on a file system without a journal a power
/// cut during the rename over the control can leave none. Recovery decides from what is there.
pub(crate) fn brings_control(root: &Path) -> bool {
    fs::read(root.join(JOURNAL))
        .ok()
        .and_then(|bytes| serde_json::from_slice::<Journal>(&bytes).ok())
        .filter(|journal| journal.current() && journal.version == VERSION)
        .is_some_and(|journal| {
            journal.entries.iter().any(|entry| is_control(entry) && entry.staged)
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
        if entry.previous {
            let actual = content_identity::of(&located.backup.join(&entry.relative))?;
            if actual != entry.original {
                // This install moved a concurrent replacement. Bind rollback to the
                // bytes actually moved before letting it restore the earlier entries.
                let name = entry.relative.display().to_string();
                anyhow::ensure!(actual.is_some(), "install backup disappeared: {name}");
                journal.entries[step / 2].original = actual;
                save(paths, journal)?;
                anyhow::bail!("installation-source-changed: {name} changed during preservation");
            }
        }
    } else if entry.staged && is_control(entry) && entry.previous {
        // Over the live control, in one rename: `config.json` is there at every step.
        rename_over(&located.stage.join(&entry.relative), &target)?;
        crate::fault_inject::checkpoint("native-install-installed")?;
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
/// Returns what became of the replaced files ([`Replaced`]).
///
/// After an error the live files are as the record on disk says: rolled back when a step failed;
/// as they are when the publish failed, for the next recovery to finish or roll back. A caller
/// does not touch them either way.
pub fn install(
    gate: &crate::ControlGate,
    stage: &Path,
    files: Vec<PathBuf>,
    keep: Keep,
) -> Result<Replaced> {
    install_bound(gate, stage, files, keep, None)
}

pub(crate) fn install_bound(
    gate: &crate::ControlGate,
    stage: &Path,
    files: Vec<PathBuf>,
    keep: Keep,
    expected: Option<&Originals>,
) -> Result<Replaced> {
    let paths = gate.paths();
    crate::fault_inject::checkpoint("native-install-before-prepare")?;
    let mut journal = prepare_bound(paths, stage, files, keep, expected)?;
    crate::fault_inject::checkpoint("native-install-prepared")?;
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
    let unsealed = finish(paths, &journal)?;
    let kept = paths.app_root.join(journal.kept_as.as_ref().unwrap_or(&journal.backup));
    Ok(Replaced { kept, unsealed })
}

/// What an install did with the files it replaced.
#[derive(Debug)]
pub struct Replaced {
    /// Where they are kept: the safety copy, or the backup folder. Neither exists when nothing was
    /// replaced, or when `keep` is `Keep::Nothing`.
    pub kept: PathBuf,
    /// Why the safety copy could not be sealed, when it could not. It is kept and listed all the
    /// same, and a restore from it seals it first; the operation tells the user now, so a copy that
    /// cannot be read is not first found out about when it is needed.
    pub unsealed: Option<String>,
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
            rename_over(&path, &paths.app_root.join(STALE_LEGACY_JOURNAL))?;
        }
        return Ok(false);
    }
    if stamped(paths) {
        // PathKeep 0.5 does not open an archive with a stamp, and this build replays a 0.5
        // journal before it stamps one: this one was carried in. Nothing it names is touched.
        rename_over(&path, &paths.app_root.join(STALE_LEGACY_JOURNAL))?;
        return Ok(false);
    }
    let journal = serde_json::from_slice::<LegacyJournal>(&bytes)?.into_journal()?;
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
        if journal.current() {
            complete_published(paths, journal)?;
        }
        // A start that finishes an install has no one to tell: the copy is listed as not checked.
        finish_at(paths, journal, record).map(drop)
    } else {
        rollback_at(paths, journal, record)
    }
}

/// What finishing a published install after a crash does with one entry. The journal says the
/// install was published in the folder it was written in; this folder may be a copy of it made by
/// a tool that copied some files before the publish and some after, cut short. So before the
/// stage and the backup go, every entry must be as the install left it, or one step from it.
enum Done {
    /// As the install left it.
    Stays,
    /// The staged file is still in the stage, and nothing is in its place: it is moved in.
    MoveIn,
    /// The staged file is in place, and the stage holds it too: a cut rename may have left it under
    /// both names (see the module docs), so the stage's goes into the quarantine, not with the
    /// stage.
    StageNameAside,
    /// An original the install moved out is in its place again, holding what it held: the
    /// replaced archive's (a log a copy carried there), it goes to the backup, with the rest of
    /// that archive.
    BackToBackup,
    /// The same, with a name of it in the backup already: into the quarantine.
    Aside,
}

/// Brings a folder whose published journal finds it one step short of the install up to it, and
/// stops, with every record kept and nothing moved, where it finds anything else.
fn complete_published(paths: &ProjectPaths, journal: &Journal) -> Result<()> {
    let located = locate(paths, journal)?;
    let plan = journal
        .entries
        .iter()
        .map(|entry| {
            let here = content_identity::of(&paths.app_root.join(&entry.relative))?;
            let staged = located.stage.join(&entry.relative);
            if entry.staged {
                let in_stage = content_identity::of(&staged)?;
                return match here {
                    Some(_) if here == entry.installed && in_stage == entry.installed => {
                        Ok(Done::StageNameAside)
                    }
                    Some(_) if here == entry.installed => Ok(Done::Stays),
                    None if in_stage == entry.installed => Ok(Done::MoveIn),
                    _ => Err(not_published(entry)),
                };
            }
            match here {
                None => Ok(Done::Stays),
                Some(_) if entry.previous && here == entry.original => {
                    if exists(&located.backup.join(&entry.relative))? {
                        Ok(Done::Aside)
                    } else {
                        Ok(Done::BackToBackup)
                    }
                }
                Some(_) => Err(not_published(entry)),
            }
        })
        .collect::<Result<Vec<_>>>()?;
    let quarantine = quarantine(paths, &located);
    for (entry, done) in journal.entries.iter().zip(plan) {
        let target = paths.app_root.join(&entry.relative);
        let staged = located.stage.join(&entry.relative);
        match done {
            Done::Stays => continue,
            Done::MoveIn => rename(&staged, &target)?,
            Done::StageNameAside => rename(&staged, &unused(&quarantine, &entry.relative)?)?,
            Done::BackToBackup => rename(&target, &located.backup.join(&entry.relative))?,
            Done::Aside => rename(&target, &unused(&quarantine, &entry.relative)?)?,
        }
        crate::fault_inject::checkpoint("native-install-completed")?;
    }
    Ok(())
}

fn not_published(entry: &Entry) -> anyhow::Error {
    anyhow::anyhow!(
        "{}: {} is not what the install published, nor one step from it; the install record is \
         kept and nothing was changed",
        crate::admission::Reason::ControlInvalid.code(),
        entry.relative.display()
    )
}

fn is_control(entry: &Entry) -> bool {
    *entry.relative == *Path::new(CONTROL)
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
    /// The control in place may be a second name of the staged one (see the module docs): it is
    /// taken out of its place by a rename, and the copy in the backup is written where it was.
    ReplaceControl,
    /// The original is in place, and other names of the entry may be second names of it: they go
    /// into the quarantine, the backup last.
    KeepAside,
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
            Undo::ReplaceControl => {
                discard(&located, entry, &target)?;
                atomic_durable_write(&target, &fs::read(&backup)?)?;
            }
            Undo::KeepAside => keep_aside(paths, &located, entry, &backup)?,
        }
        crate::fault_inject::checkpoint("native-install-undone")?;
    }
    remove_file_durably(record)?;
    remove(&located.stage)?;
    remove(&located.backup)
}

/// Takes what is at `target` out of its place in one rename, into the stage, under a name nothing
/// has: a name already there may be a second name of this file (see the module docs), and is never
/// deleted while the file is in place. A rollback cut short before the rename finds it still there,
/// one cut short after finds the place empty: never half of a folder.
fn discard(located: &Located, entry: &Entry, target: &Path) -> Result<()> {
    if !exists(target)? {
        return Ok(());
    }
    rename(target, &unused(&located.stage.join(DISCARDED), &entry.relative)?)?;
    crate::fault_inject::checkpoint("native-install-discarded")
}

/// Moves into the quarantine every other name of an entry whose original is in place and that may
/// share its data: the staged and discarded names holding the same bytes, then the backup's. The
/// backup's goes last, so a rollback cut short still finds the entry in two places, or finds the
/// quarantine holding some of it.
fn keep_aside(paths: &ProjectPaths, located: &Located, entry: &Entry, backup: &Path) -> Result<()> {
    let quarantine = quarantine(paths, located);
    let mut names = Vec::new();
    for name in std::iter::once(located.stage.join(&entry.relative))
        .chain(names_in(&located.stage.join(DISCARDED), &entry.relative)?)
    {
        if exists(&name)? && content_identity::of(&name)? == entry.original {
            names.push(name);
        }
    }
    names.push(backup.to_path_buf());
    for name in names {
        if exists(&name)? {
            rename(&name, &unused(&quarantine, &entry.relative)?)?;
            crate::fault_inject::checkpoint("native-install-kept")?;
        }
    }
    eprintln!(
        "PathKeep install: {} may have two names after a power cut on a file system without a \
         journal; its other names are kept in {} until the disk is checked",
        entry.relative.display(),
        quarantine.display()
    );
    Ok(())
}

/// Where a rollback keeps names it must not delete, under the backup folder's random name: a
/// rollback cut short finds them again, and they are this install's.
fn quarantine(paths: &ProjectPaths, located: &Located) -> PathBuf {
    paths.quarantine_dir.join(located.backup.file_name().unwrap_or_default())
}

/// The names of `relative` under `folder`, one per numbered subfolder (see `unused`).
fn names_in(folder: &Path, relative: &Path) -> Result<Vec<PathBuf>> {
    let numbered = match fs::read_dir(folder) {
        Ok(numbered) => numbered,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error.into()),
    };
    let mut names = Vec::new();
    for subfolder in numbered {
        let name = subfolder?.path().join(relative);
        if exists(&name)? {
            names.push(name);
        }
    }
    Ok(names)
}

/// `folder/<n>/<relative>` for the first `n` that has no such name.
pub(crate) fn unused(folder: &Path, relative: &Path) -> Result<PathBuf> {
    for n in 0u32.. {
        let name = folder.join(n.to_string()).join(relative);
        if !exists(&name)? {
            return Ok(name);
        }
    }
    unreachable!("a folder with four billion names of one file")
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
    let mut undos: Vec<(bool, Result<Undo>)> = journal
        .entries
        .iter()
        .rev()
        .map(|entry| {
            let target = paths.app_root.join(&entry.relative);
            let backup = located.backup.join(&entry.relative);
            let undo = if !journal.current() {
                undo_legacy(entry, &target, &backup)
            } else if is_control(entry) {
                undo_control(entry, &target, &backup, &located.stage.join(&entry.relative))
            } else {
                undo_current(paths, entry, located, &target, &backup)
            };
            (is_control(entry), undo)
        })
        .collect();
    // The control binds the journal to its folder: where it says the journal is not this
    // folder's, that is what is reported, not a file of the other folder taken for a lost one.
    if let Some(index) = undos.iter().position(|(control, undo)| *control && undo.is_err())
        && let (_, Err(error)) = undos.swap_remove(index)
    {
        return Err(error);
    }
    undos.into_iter().map(|(_, undo)| undo).collect()
}

/// A file of a current journal. Where it is says how far the install got, and what it holds
/// confirms it. A file that was not there before goes only if it is the staged one. An original in
/// the backup must hold what it held: it comes back over the staged file or an empty place. Not in
/// the backup, it was never moved or is back, and must be in place. In place and in the backup at
/// once is a rename cut short (see the module docs): with the staged file in place, the original
/// comes back over it; with the original in place, its other names are kept aside. Anything else
/// stops the rollback before anything moves: a backup cut short by a copy, a backup entry a copy
/// left out, someone else's file.
fn undo_current(
    paths: &ProjectPaths,
    entry: &Entry,
    located: &Located,
    target: &Path,
    backup: &Path,
) -> Result<Undo> {
    let here = content_identity::of(target)?;
    if !entry.previous {
        return match here {
            _ if exists(backup)? => Err(not_moved(entry)),
            None => Ok(Undo::Leave),
            Some(_) if here == entry.installed => Ok(Undo::Discard),
            Some(_) => Err(not_moved(entry)),
        };
    }
    let kept = content_identity::of(backup)?;
    if kept.is_some() && kept != entry.original {
        return Err(not_moved(entry));
    }
    match (&here, &kept) {
        (None, Some(_)) => Ok(Undo::PutBack),
        (None, None) => Err(lost(entry)),
        (Some(_), None) if here == entry.original => Ok(Undo::Leave),
        (Some(_), None) if here == entry.installed => Err(lost(entry)),
        (Some(_), None) => Err(not_moved(entry)),
        (Some(_), Some(_)) if here == entry.original && maybe_two_names(paths, entry, located)? => {
            Ok(Undo::KeepAside)
        }
        (Some(_), Some(_)) if here == entry.installed => Ok(Undo::PutBack),
        (Some(_), Some(_)) => Err(not_moved(entry)),
    }
}

/// Whether the original in place may share its data with its name in the backup, or with a staged
/// or discarded name holding the same bytes: a rename between them may have been cut. Outside a
/// cut, an entry is in place and in the backup only with the staged file in place, renamed in whole
/// from the stage: its staged name is gone, it was not discarded, and nothing of it is in the
/// quarantine. When the staged file holds what the original held, that is the only case that looks
/// like this one without being it. (An entry with nothing staged has no `installed`, so it always
/// differs from the original here.)
fn maybe_two_names(paths: &ProjectPaths, entry: &Entry, located: &Located) -> Result<bool> {
    Ok(entry.installed != entry.original
        || exists(&located.stage.join(&entry.relative))?
        || !names_in(&located.stage.join(DISCARDED), &entry.relative)?.is_empty()
        || !names_in(&quarantine(paths, located), &entry.relative)?.is_empty())
}

/// The control of a current journal. It is copied into the backup, not moved, and replaced by one
/// rename, so it is the one from before the install or the one the install brought; the backup's
/// copy must hold what the control held. Only a rename over it cut on a file system without a
/// journal leaves none in place, with the copy in the backup. Anything else, or nothing, means this
/// is not the journal's folder.
fn undo_control(entry: &Entry, target: &Path, backup: &Path, staged: &Path) -> Result<Undo> {
    let here = content_identity::of(target)?;
    if !entry.previous {
        // The folder had no control (an import into a new folder, a restore repairing one): the
        // install brought the only one.
        return match here {
            None => Ok(Undo::Leave),
            Some(_) if here == entry.installed => Ok(Undo::Discard),
            Some(_) => Err(not_moved(entry)),
        };
    }
    let kept = content_identity::of(backup)?;
    if kept.is_some() && kept != entry.original {
        return Err(not_moved(entry));
    }
    match (&here, &kept) {
        (None, Some(_)) => Ok(Undo::WriteControl),
        (None, None) => Err(lost(entry)),
        // Its copy was not made yet: the install has not touched it.
        (Some(_), None) if here == entry.original => Ok(Undo::Leave),
        (Some(_), None) => Err(not_moved(entry)),
        // The staged control still in the stage while the installed one is in place: the rename
        // between them may have been cut, and writing over the control would free data its
        // staged name still uses.
        (Some(_), Some(_)) if here == entry.installed && exists(staged)? => {
            Ok(Undo::ReplaceControl)
        }
        (Some(_), Some(_)) if here == entry.original || here == entry.installed => {
            Ok(Undo::WriteControl)
        }
        (Some(_), Some(_)) => Err(not_moved(entry)),
    }
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

/// It was there before the install, and the original is neither in place nor in the backup.
/// Saying the rollback succeeded would lose track of it for good.
fn lost(entry: &Entry) -> anyhow::Error {
    anyhow::anyhow!(
        "{} is neither in place nor in the backup; the install record is kept",
        entry.relative.display()
    )
}

/// Returns why the safety copy could not be sealed, when it could not ([`Replaced::unsealed`]).
fn finish(paths: &ProjectPaths, journal: &Journal) -> Result<Option<String>> {
    finish_at(paths, journal, &paths.app_root.join(JOURNAL))
}

fn finish_at(paths: &ProjectPaths, journal: &Journal, record: &Path) -> Result<Option<String>> {
    // Finishing only removes the stage, the backup and the record, and moves the backup to its
    // copy, each at a name this folder's installs produce.
    let located = locate(paths, journal)?;
    let unsealed = match &located.kept_as {
        Some(copy) => keep_as_copy(paths, &located, copy)?,
        None if !journal.keep_previous => {
            remove(&located.backup)?;
            None
        }
        None => None,
    };
    remove_file_durably(record)?;
    remove(&located.stage)?;
    Ok(unsealed)
}

/// Makes the backup of a published install a safety copy. Run again after a crash, it finishes
/// what is left: the move is one rename, and the seal is written whole or not at all.
fn keep_as_copy(paths: &ProjectPaths, located: &Located, copy: &Path) -> Result<Option<String>> {
    let backup = &located.backup;
    if backup.is_dir() {
        let replaced = walkdir::WalkDir::new(backup)
            .into_iter()
            .any(|entry| entry.map_or(true, |entry| !entry.file_type().is_dir()));
        if !replaced {
            // Nothing was there to replace (an import into an empty folder): no copy of nothing.
            return remove(backup).map(|()| None);
        }
        if !exists(copy)? {
            rename(backup, copy)?;
        } else if content_identity::of(copy)? == content_identity::of(backup)? {
            // The rename below was cut on a file system without a journal and left the replaced
            // archive under both names, each the same data (the module docs). The copy is made, and
            // the backup's name goes into the quarantine like the other names a cut rename leaves.
            let name = Path::new(backup.file_name().unwrap_or_default());
            rename(backup, &unused(&quarantine(paths, located), name)?)?;
        } else {
            // Another copy under this name, which the backup folder's random name rules out but for
            // a coincidence: the replaced files stay where they are rather than go over it.
            eprintln!(
                "PathKeep install: {} is already there; the replaced files stay in {}",
                copy.display(),
                backup.display()
            );
            return Ok(None);
        }
    }
    if copy.is_dir() && !copy.join(super::snapshot::SEAL).exists() {
        // A replaced archive the install cannot seal is still a copy: listed under the operation it
        // was made before, counted, and freed only when the user picks it. After an error while
        // reading it (a file another program held, a read that failed once) the restore that needs
        // it seals it first; one whose control is damaged cannot be sealed or restored from
        // (`snapshot::list_snapshots`). The error stops neither this install, published already,
        // nor the start that finishes one: the archive in place is the new one, and a copy that
        // can never be sealed must not keep it from opening. The restore or import says why.
        if let Err(error) = super::snapshot::seal_replaced(copy) {
            eprintln!(
                "PathKeep install: the replaced archive kept in {} could not be sealed: {error:#}",
                copy.display()
            );
            return Ok(Some(format!("{error:#}")));
        }
    }
    Ok(None)
}

/// Moves a file or folder to where nothing is, and makes the move durable. What appeared at `to`
/// since it was looked at stays, and the move fails (`durable_io::rename_no_replace`). The
/// destination's directory is flushed first (`durable_io::fsync_dir`), and new parent folders are
/// made durable before anything is moved into them. On a file system with a journal the rename is
/// one step. On FAT32 or exFAT a power cut can leave the entry in both places, which recovery
/// allows for (the module docs), or, since the system may write the source's folder first, in
/// neither, which no order of calls prevents there.
pub(crate) fn rename(from: &Path, to: &Path) -> Result<()> {
    create_dirs_durably(to.parent().unwrap())?;
    crate::durable_io::rename_no_replace(from, to)
        .with_context(|| format!("moving {} to {}", from.display(), to.display()))?;
    fsync_dir(to.parent().unwrap())?;
    fsync_dir(from.parent().unwrap())
}

/// `rename` over what is at `to`: the control's own rename on install, and a record set aside over
/// an older one.
fn rename_over(from: &Path, to: &Path) -> Result<()> {
    create_dirs_durably(to.parent().unwrap())?;
    vault_fs::rename(from, to)?;
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
    let mut found = false;
    vault_fs::retry_while_held(|| match fs::symlink_metadata(path) {
        Ok(meta) => {
            found = true;
            if meta.is_dir() { fs::remove_dir_all(path) } else { fs::remove_file(path) }
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e),
    })?;
    if found {
        fsync_dir(path.parent().unwrap())?;
    }
    Ok(())
}
