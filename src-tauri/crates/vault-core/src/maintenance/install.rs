//! Responsibilities: journal a verified multi-file installation and recover interrupted renames.
//! Not responsible for: SQL, keys, verification or choosing application files.
//! Failure modes: mixed generations, lost originals, stale WAL, crash during rollback,
//! configuration committed ahead of data, deleting the owner lock, a crash that leaves no
//! `config.json` (an older release then takes the folder for a first run), a release that does
//! not know this journal replaying it. Tests: install_tests.
//!
//! The control is not moved like the other entries. It is copied into the backup and replaced in
//! one rename, on install and on rollback, so `config.json` exists at every point of the
//! transaction. The journal is written under a name PathKeep 0.5 and earlier do not know, with an
//! identity and a version that are checked before anything is replayed; the name those releases
//! wrote is still read, as a record they may have left.
use crate::{
    ProjectPaths,
    durable_io::{atomic_durable_write, fsync_dir, remove_file_durably},
};
use anyhow::Result;
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
    stage: PathBuf,
    backup: PathBuf,
    entries: Vec<Entry>,
    keep_previous: bool,
    committed: bool,
}

pub(super) fn prepare(
    paths: &ProjectPaths,
    stage: &Path,
    files: Vec<PathBuf>,
    keep_previous: bool,
) -> Result<Journal> {
    sync_tree(stage)?;
    let backup = tempfile::Builder::new()
        .prefix(vault_layout::family_prefix(vault_layout::ArtifactId::PreviousArchives))
        .tempdir_in(&paths.app_root)?
        .keep();
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
        stage: stage.into(),
        backup,
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
    let entry = &journal.entries[step / 2];
    let target = paths.app_root.join(&entry.relative);
    if step % 2 == 0 {
        if entry.previous && is_control(entry) {
            // Copied, not moved: the live control stays until its replacement lands on it.
            let bytes = fs::read(&target)?;
            let backup = journal.backup.join(&entry.relative);
            fs::create_dir_all(backup.parent().unwrap())?;
            atomic_durable_write(&backup, &bytes)?;
        } else if entry.previous {
            rename(&target, &journal.backup.join(&entry.relative))?;
        }
        crate::fault_inject::checkpoint("native-install-preserved")?;
    } else if entry.staged {
        rename(&journal.stage.join(&entry.relative), &target)?;
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
    Ok(journal.backup)
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
    if journal.committed { finish_at(journal, record) } else { rollback_at(paths, journal, record) }
}

fn is_control(entry: &Entry) -> bool {
    entry.relative == Path::new(CONTROL)
}

fn rollback(paths: &ProjectPaths, journal: &Journal) -> Result<()> {
    rollback_at(paths, journal, &paths.app_root.join(JOURNAL))
}

fn rollback_at(paths: &ProjectPaths, journal: &Journal, record: &Path) -> Result<()> {
    for entry in journal.entries.iter().rev() {
        let backup = journal.backup.join(&entry.relative);
        let target = paths.app_root.join(&entry.relative);
        if backup.exists() && is_control(entry) {
            // One rename over the live control. A journal of PathKeep 0.5 moved the control into
            // the backup, so the live one may be missing; the rename covers both.
            rename(&backup, &target)?;
        } else if backup.exists() {
            remove(&target)?;
            rename(&backup, &target)?;
        } else if !entry.previous {
            remove(&target)?;
        }
    }
    remove_file_durably(record)?;
    remove(&journal.stage)?;
    remove(&journal.backup)
}

fn finish(paths: &ProjectPaths, journal: &Journal) -> Result<()> {
    finish_at(journal, &paths.app_root.join(JOURNAL))
}

fn finish_at(journal: &Journal, record: &Path) -> Result<()> {
    if !journal.keep_previous {
        remove(&journal.backup)?;
    }
    remove_file_durably(record)?;
    remove(&journal.stage)
}

fn rename(from: &Path, to: &Path) -> Result<()> {
    fs::create_dir_all(to.parent().unwrap())?;
    fs::rename(from, to)?;
    fsync_dir(from.parent().unwrap())?;
    fsync_dir(to.parent().unwrap())
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
