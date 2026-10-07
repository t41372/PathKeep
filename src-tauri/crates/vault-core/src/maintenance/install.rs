//! Responsibilities: journal a verified multi-file installation and recover interrupted renames.
//! Not responsible for: SQL, keys, verification or choosing application files.
//! Failure modes: mixed generations, lost originals, stale WAL, crash during rollback,
//! configuration committed ahead of data, deleting the owner lock. Tests: install_tests.
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

const JOURNAL: &str = "archive/.pk-import-journal.json";

/// Startup checks for recovery before it decides whether a native archive exists.
pub fn pending(paths: &ProjectPaths) -> bool {
    fs::read(paths.app_root.join(JOURNAL))
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

#[derive(Serialize, Deserialize)]
pub(super) struct Entry {
    relative: PathBuf,
    previous: bool,
    staged: bool,
}

/// A durable prepare record is the rollback decision; `committed` publishes the
/// entire generation. Recovery uses the same entries as in-band installation.
#[derive(Serialize, Deserialize)]
pub(super) struct Journal {
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
    let backup =
        tempfile::Builder::new().prefix("maintenance.bak-").tempdir_in(&paths.app_root)?.keep();
    let entries = files
        .into_iter()
        .map(|relative| Entry {
            previous: paths.app_root.join(&relative).exists(),
            staged: stage.join(&relative).exists(),
            relative,
        })
        .collect();
    let journal = Journal { stage: stage.into(), backup, entries, keep_previous, committed: false };
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
        if entry.previous {
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
pub fn recover(paths: &ProjectPaths) -> Result<bool> {
    let bytes = match fs::read(paths.app_root.join(JOURNAL)) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error.into()),
    };
    if serde_json::from_slice::<serde_json::Value>(&bytes)?.get("stage").is_none() {
        return crate::migration::recover_interrupted_import(paths);
    }
    let journal: Journal = serde_json::from_slice(&bytes)?;
    if journal.committed {
        finish(paths, &journal)?;
    } else {
        rollback(paths, &journal)?;
    }
    Ok(true)
}

fn rollback(paths: &ProjectPaths, journal: &Journal) -> Result<()> {
    for entry in journal.entries.iter().rev() {
        let backup = journal.backup.join(&entry.relative);
        let target = paths.app_root.join(&entry.relative);
        if backup.exists() {
            remove(&target)?;
            rename(&backup, &target)?;
        } else if !entry.previous {
            remove(&target)?;
        }
    }
    remove_file_durably(&paths.app_root.join(JOURNAL))?;
    remove(&journal.stage)?;
    remove(&journal.backup)
}

fn finish(paths: &ProjectPaths, journal: &Journal) -> Result<()> {
    if !journal.keep_previous {
        remove(&journal.backup)?;
    }
    remove_file_durably(&paths.app_root.join(JOURNAL))?;
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
