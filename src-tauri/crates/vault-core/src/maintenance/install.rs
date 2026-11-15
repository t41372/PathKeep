//! Responsibilities: journal a verified multi-file installation and recover interrupted renames.
//! Not responsible for: SQL, keys, verification or choosing application files.
//! Failure modes: mixed generations, lost originals, stale WAL, crash during rollback,
//! configuration committed ahead of data, deleting the owner lock, a crash that leaves no
//! `config.json` (an older release then takes the folder for a first run), a release that does
//! not know this journal replaying it. Tests: install_tests.
//!
//! The control is not moved like the other entries. It is copied into the backup and replaced in
//! one rename on install, and from that copy by an atomic write on rollback, so `config.json`
//! exists at every point of the transaction.
//!
//! A journal acts only on the folder it was written in. The current one records its stage and
//! backup relative to the folder; a rollback runs only when the backup folder it names (randomly
//! named when the install began) is in this folder. A journal carried into another folder, by a
//! copy or a backup restore, finds no such folder and is refused before anything moves; one in a
//! folder that was moved whole is recovered where it now is. The journal is written under a name PathKeep 0.5 and earlier do not know, with an
//! identity and a version that are checked before anything is replayed; the name those releases
//! wrote is still read, as a record they may have left.
use crate::{
    ProjectPaths,
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
/// Where an old-format import journal found next to a native archive is set aside.
const STALE_LEGACY_JOURNAL: &str =
    vault_layout::spec(vault_layout::ArtifactId::StaleImportJournal).path.relative();

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

#[derive(Serialize, Deserialize, schemars::JsonSchema)]
pub(crate) struct Entry {
    relative: PathBuf,
    previous: bool,
    staged: bool,
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
    committed: bool,
}

/// A journal's stage and backup in this folder, once it is clear they are this folder's.
struct Located {
    stage: PathBuf,
    backup: PathBuf,
}

/// Places a journal's stage and backup in this folder. Every path in it must be one this folder's
/// installs produce: a backup named like `maintenance.bak-*` directly in the folder, a stage
/// under `staging/`, entries that stay inside the folder. A 0.5 journal's absolute paths are
/// taken relative to where its backup was, so a folder moved since is still recovered.
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
    let current = journal.identity == IDENTITY;
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
    if !journal.entries.iter().all(|entry| plain(&entry.relative)) {
        return Err(foreign("list of files"));
    }
    Ok(Located { stage: paths.app_root.join(stage), backup: paths.app_root.join(backup) })
}

pub(super) fn prepare(
    paths: &ProjectPaths,
    stage: &Path,
    files: Vec<PathBuf>,
    keep_previous: bool,
) -> Result<Journal> {
    sync_tree(stage)?;
    // Compared as absolute paths: the folder may have been given relative to the working
    // directory, and a stage made by `tempfile` is always absolute.
    let stage_relative = std::path::absolute(stage)?
        .strip_prefix(std::path::absolute(&paths.app_root)?)
        .context("an install stage outside the archive folder")?
        .to_path_buf();
    let backup = tempfile::Builder::new()
        .prefix(vault_layout::family_prefix(vault_layout::ArtifactId::PreviousArchives))
        .tempdir_in(&paths.app_root)?
        .keep();
    // The backup folder's own entry is durable before the journal names it, and before the only
    // copy of anything is moved into it.
    fsync_dir(&paths.app_root)?;
    let backup_relative = PathBuf::from(backup.file_name().context("a backup folder name")?);
    let entries: Vec<Entry> = files
        .into_iter()
        .map(|relative| Entry {
            previous: paths.app_root.join(&relative).exists(),
            staged: stage.join(&relative).exists(),
            relative,
        })
        .collect();
    // Removing the control is not something an install does: a replacement brings its own.
    if entries.iter().any(|entry| is_control(entry) && entry.previous && !entry.staged) {
        let _ = fs::remove_dir(&backup);
        anyhow::bail!("an installation without a config.json would remove the archive's control");
    }
    let journal = Journal {
        identity: IDENTITY.into(),
        version: VERSION,
        stage: stage_relative,
        backup: backup_relative,
        entries,
        keep_previous,
        committed: false,
    };
    save(paths, &journal)?;
    Ok(journal)
}

fn sync_tree(root: &Path) -> Result<()> {
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
    save(paths, journal)
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

/// Caller has verified staged files and closed all live handles before entering.
pub fn install(
    paths: &ProjectPaths,
    stage: &Path,
    files: Vec<PathBuf>,
    keep_previous: bool,
) -> Result<PathBuf> {
    let mut journal = prepare(paths, stage, files, keep_previous)?;
    let outcome = (|| {
        for step in 0..journal.entries.len() * 2 {
            install_step(paths, &mut journal, step)?;
        }
        publish(paths, &mut journal)
    })();
    if let Err(error) = outcome {
        journal.committed = false;
        rollback(paths, &journal)?;
        return Err(error);
    }
    crate::fault_inject::checkpoint("native-install-published")?;
    finish(paths, &journal)?;
    Ok(paths.app_root.join(&journal.backup))
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
    let mut recovered = recover_legacy(paths)?;
    let path = paths.app_root.join(JOURNAL);
    let bytes = match fs::read(&path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(recovered),
        Err(error) => return Err(error.into()),
    };
    // Nothing is replayed from a record this build did not write in this shape.
    let journal = serde_json::from_slice::<Journal>(&bytes)
        .ok()
        .filter(|journal| journal.identity == IDENTITY && journal.version == VERSION)
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
    let journal: Journal = serde_json::from_slice(&bytes)?;
    replay(paths, &journal, &path)?;
    Ok(true)
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

fn rollback_at(paths: &ProjectPaths, journal: &Journal, record: &Path) -> Result<()> {
    let located = locate(paths, journal)?;
    // The backup folder exists from the moment the journal does until the journal is gone, and
    // its name is random. Without it here, this is another folder's journal: it would delete files
    // of this one that it takes for its own new ones.
    if !located.backup.is_dir() {
        anyhow::bail!(
            "{}: the install record names a backup folder that is not in this folder; nothing was \
             changed",
            crate::admission::Reason::ControlInvalid.code()
        );
    }
    for entry in journal.entries.iter().rev() {
        let backup = located.backup.join(&entry.relative);
        let target = paths.app_root.join(&entry.relative);
        if backup.exists() && is_control(entry) {
            // Written over the live control from the copy, in the control's own folder. A journal
            // of PathKeep 0.5 moved the control into the backup, so the live one may be missing;
            // the write covers both.
            atomic_durable_write(&target, &fs::read(&backup)?)?;
        } else if backup.exists() {
            remove(&target)?;
            rename(&backup, &target)?;
        } else if !entry.previous {
            remove(&target)?;
        } else if fs::symlink_metadata(&target).is_err() {
            // It was there before the install, and it is neither in its place nor in the backup.
            // Saying the rollback succeeded would lose track of it for good.
            anyhow::bail!(
                "{} is neither in place nor in the backup; the install record is kept",
                entry.relative.display()
            );
        }
    }
    remove_file_durably(record)?;
    remove(&located.stage)?;
    remove(&located.backup)
}

fn finish(paths: &ProjectPaths, journal: &Journal) -> Result<()> {
    finish_at(paths, journal, &paths.app_root.join(JOURNAL))
}

fn finish_at(paths: &ProjectPaths, journal: &Journal, record: &Path) -> Result<()> {
    // Finishing only removes the stage, the backup and the record, each at a name this folder's
    // installs produce.
    let located = locate(paths, journal)?;
    if !journal.keep_previous {
        remove(&located.backup)?;
    }
    remove_file_durably(record)?;
    remove(&located.stage)
}

/// Moves a file or folder and makes the move durable. The destination's directory is flushed
/// first: if power fails between the two flushes the entry may show in both places, never in
/// neither (`durable_io::fsync_dir`). New parent folders are made durable before anything is
/// moved into them.
fn rename(from: &Path, to: &Path) -> Result<()> {
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

fn remove(path: &Path) -> Result<()> {
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
