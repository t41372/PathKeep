//! Responsibilities: bring an archive up to the layout this build writes, so that a crash at any
//! point is finished from a record, a store the layout requires never comes back empty without
//! one, and nothing already in the archive is ever deleted or written over.
//! Not responsible for: what the stores contain (their migrations), installs (`install`), or
//! deciding whether a build may use the archive (`admission`).
//!
//! The sequence (design amendment A1, revised 2026-11-15), run when the live archive is opened:
//! 1. The stores this layout adds to the released one that do not exist yet are made in a new
//!    folder under `staging/`, with the archive's keys, then closed with everything in their main
//!    files and flushed. Nothing in the archive has changed yet. A folder with none of the released
//!    stores is a new archive, and gets all of them.
//! 2. A record names those stores, what each holds once made, and that folder, and keeps the
//!    stamped `config.json` step 3 writes. It is written where the install journal is written,
//!    under the journal's current name, which PathKeep 0.5 and earlier do not know.
//! 3. `config.json` gets the stamp. From here an older release stops at the config.
//! 4. Each store is moved into its place, only where nothing is, and only if it still holds what
//!    it held when made.
//! 5. The folder is removed, then the record, and the store opens.
//!
//! How this could fail, and what guards it:
//! - A crash after the stamp leaves a required store missing, and the next open would create it
//!   empty as if nothing had been there. The record says it is being made: recovery moves in what
//!   the folder holds, and an open makes again what the folder lost. Without a record a missing
//!   required store of a stamped archive is damage and its group is refused
//!   (`check_required_stores`), because the store layer would otherwise create an empty file.
//! - A crash before the stamp, then PathKeep 0.5 opens the folder (it can: there is no stamp) and
//!   creates and fills some of the stores the record names. Before the stamp the record is only
//!   discarded with its folder; nothing in the archive is touched.
//! - A crash in the middle of making a store leaves a half-written file, in the folder under
//!   `staging/` only, which goes with the record. The archive receives a store only whole, by one
//!   rename.
//! - A record carried into another archive's folder (two folders merged, or the record restored
//!   over another archive). Its stores are there, and nothing is moved over a file, so nothing of
//!   that archive changes. Nothing a record names is ever deleted from its place.
//! - A store that cannot be looked at (a folder without permission) taken for a missing one, and
//!   made again over it. Only "not found" is missing; any other error stops before the record.
//! - A store's `-wal` or `-shm` left in place without its main file: a new main file beside it
//!   would be read with another file's log. Nothing is moved in there, and the open is refused.
//! - A released archive missing one of its own stores (Ask conversations, Insights) has it made
//!   empty and stamped as if the upgrade had added it, and the loss goes unnoticed. Only the
//!   stores this layout introduces are made; the open of such an archive is refused instead.
//! - On a file system without a journal (exFAT, FAT32), a power cut during the move of a store
//!   into its place can leave it under both names. Removing the folder would then free data the
//!   store in place still uses. A store the record moves in whose place holds the same bytes goes
//!   into the quarantine (`quarantine/<folder name>/`) rather than with the folder.
//! - A copy of the data folder cut short leaves a made store shorter than it was made. Moved in,
//!   it would be the store the archive opens, with no record left to make it again. The record
//!   keeps what each made store holds; one that does not hold it is not moved in, and the next open
//!   makes it again.
//! - A folder stores were moved out of is left without a record, and whatever removes it later
//!   (the next bring-up, "Free up space" clearing `staging/`) deletes a second name of a store in
//!   place (above). A folder made again for lost stores is recorded before anything leaves it, and
//!   every folder is removed while the record still names it.
//! - On a file system without a journal a power cut during the rename that writes the stamp can
//!   leave no `config.json`, and the folder would read as an archive that lost its control. The
//!   record keeps the stamped control. A folder without one, holding a record whose folder still
//!   holds every store it names as made (which binds the record to this folder), is that bring-up
//!   cut short (`admission::census`), and recovery writes the control from the record. A record
//!   carried in without its folder, or whose control is not one this build stamps, writes nothing.
//! - Paths in the record are plain names joined by `/` (`recorded_path`), read the same on every
//!   system.
//!
//! Tests: creation/tests.rs.

use super::install::{exists, remove, rename, sync_tree};
use crate::{
    ProjectPaths, content_identity,
    durable_io::{atomic_durable_write, fsync_dir, remove_file_durably},
    recorded_path::RecordedPath,
};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Component, Path, PathBuf};
use vault_layout::{ArchiveLayout, ArtifactId, CompanionUse, Db};

const IDENTITY: &str = "pathkeep-create";
const VERSION: u32 = 1;
/// The folders stores are made in, under `staging/`.
const FOLDER_PREFIX: &str = "creating-";

/// The stores being made, where they were made, and the control the bring-up writes.
#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub(crate) struct CreationRecord {
    identity: String,
    version: u32,
    /// Each store being made, with what it holds once made. A store in the folder that does not
    /// hold it (a copy of the folder cut short) is not moved in: it is made again.
    creating: Vec<Made>,
    /// The folder they were made in, relative to the archive folder: `staging/creating-*`. Recorded
    /// before anything is moved out of it.
    made_in: RecordedPath,
    /// `config.json` as the stamp writes it. Where a power cut during that write left no control
    /// (a file system without a journal), recovery writes this.
    control: String,
}

impl CreationRecord {
    /// Whether the folder the record names holds every store it names, as made. Before any store
    /// leaves it, that is what binds the record to this data folder: a record carried in without
    /// its folder says nothing about this one.
    fn made_here(&self, paths: &ProjectPaths) -> Result<bool> {
        let folder = paths.app_root.join(&self.made_in);
        for Made { relative, holds } in &self.creating {
            if content_identity::of(&folder.join(relative))?.as_ref() != Some(holds) {
                return Ok(false);
            }
        }
        Ok(true)
    }
}

/// Whether the folder holds a record of a bring-up this build wrote whose stores are still where
/// they were made. A folder without a control and with such a record is that bring-up, cut by a
/// power cut that took the stamped control (`admission::census`): the record puts it back.
pub(crate) fn puts_control_back(paths: &ProjectPaths) -> bool {
    matches!(read(paths), Ok(Some(record)) if record.made_here(paths).unwrap_or(false))
}

/// One store being made.
#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub(crate) struct Made {
    /// Its place, relative to the archive folder.
    relative: RecordedPath,
    /// What it holds once made and closed (`content_identity`).
    holds: String,
}

fn record_path(paths: &ProjectPaths) -> PathBuf {
    vault_layout::path(&paths.app_root, ArtifactId::InstallJournalV2)
}

/// The record, when the journal file holds one. `None` when the file is absent or holds an
/// install journal.
pub(crate) fn read(paths: &ProjectPaths) -> Result<Option<CreationRecord>> {
    let bytes = match std::fs::read(record_path(paths)) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    let identity = serde_json::from_slice::<serde_json::Value>(&bytes)
        .ok()
        .and_then(|value| value.get("identity").and_then(|v| v.as_str()).map(str::to_owned));
    if identity.as_deref() != Some(IDENTITY) {
        return Ok(None);
    }
    let record = serde_json::from_slice::<CreationRecord>(&bytes)
        .ok()
        .filter(|record| record.version == VERSION)
        .filter(|record| record.creating.iter().all(|made| store_of(&made.relative).is_some()))
        .filter(|record| is_made_in(&record.made_in))
        .filter(|record| {
            crate::admission::parse_control(&paths.app_root, record.control.as_bytes())
                .is_ok_and(|control| control.compatibility.is_some())
        })
        .with_context(|| {
            format!(
                "{}: the store creation record is not one this build can act on",
                crate::admission::Reason::ControlInvalid.code()
            )
        })?;
    Ok(Some(record))
}

/// The store at `relative`, when it is one a layout this build knows requires; a record names
/// nothing else.
fn store_of(relative: &Path) -> Option<Db> {
    vault_layout::databases_in(ArchiveLayout::NEWEST)
        .into_iter()
        .find(|db| Path::new(vault_layout::database_path(*db)) == relative)
}

/// A folder this module makes: `staging/creating-*`, nothing else.
fn is_made_in(relative: &Path) -> bool {
    let staging = vault_layout::spec(ArtifactId::Staging).path.relative();
    let parts: Vec<_> = relative.components().collect();
    matches!(
        parts.as_slice(),
        [Component::Normal(first), Component::Normal(name)]
            if Path::new(first) == Path::new(staging)
                && name.to_str().is_some_and(|name| name.starts_with(FOLDER_PREFIX))
    )
}

/// Brings the live archive up to this build's layout when it is a released-format archive that is
/// set up, and makes the stores a record left after a crash still lacks. The caller holds the
/// owner lock and has run install recovery; `key` is the archive's, for the stores it makes.
pub(crate) fn promote_if_needed(
    paths: &ProjectPaths,
    key: Option<&vault_crypto::DataKey>,
) -> Result<bool> {
    // The stamp is a write of the control, read and written under its gate.
    let _control = crate::ControlGate::acquire(paths);
    if let Some(record) = read(paths)? {
        // Recovery leaves a record only after the stamp, when the folder the stores were made in
        // lost some of them.
        complete(paths, record, key)?;
        return Ok(true);
    }
    // A folder without a control has nothing to bring up: setting up an archive writes its
    // control before the first open.
    let Some(bytes) = crate::admission::read_control(&paths.app_root)? else { return Ok(false) };
    let document = crate::config::decode_config(&paths.app_root, &bytes)?;
    if document.compatibility.is_some() || !document.config.initialized {
        return Ok(false);
    }
    let layout = crate::admission::WRITTEN_LAYOUT;
    if layout == ArchiveLayout::BrowsingV1 {
        return Ok(false);
    }
    let creating = adding(paths, layout)?;
    // The archive's settings with this build's stamp.
    let control = crate::config::encode_config(
        &document.config,
        Some(&crate::admission::written_compatibility()),
    )?;
    if !creating.is_empty() {
        // A crash while stores were being made left their folder without a record: nothing names
        // it, and nothing else makes folders of this name.
        remove_unrecorded_folders(paths)?;
        let (made_in, creating) = make(paths, layout, &creating, key)?;
        crate::fault_inject::checkpoint("promotion-made")?;
        let record = CreationRecord {
            identity: IDENTITY.into(),
            version: VERSION,
            creating,
            made_in,
            control,
        };
        save(paths, &record)?;
        crate::fault_inject::checkpoint("promotion-recorded")?;
        stamp(paths, &record.control)?;
        crate::fault_inject::checkpoint("promotion-stamped")?;
        complete(paths, record, key)?;
    } else {
        stamp(paths, &control)?;
    }
    Ok(true)
}

fn save(paths: &ProjectPaths, record: &CreationRecord) -> Result<()> {
    let path = record_path(paths);
    std::fs::create_dir_all(path.parent().context("a record without a folder")?)?;
    atomic_durable_write(&path, &serde_json::to_vec(record)?)
        .context("recording the stores being made")
}

fn remove_unrecorded_folders(paths: &ProjectPaths) -> Result<()> {
    let entries = match std::fs::read_dir(&paths.staging_dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error.into()),
    };
    for entry in entries {
        let entry = entry?;
        // Nothing is moved out of a folder before a record names it.
        if entry.file_name().to_str().is_some_and(|name| name.starts_with(FOLDER_PREFIX)) {
            remove(&entry.path())?;
        }
    }
    Ok(())
}

fn stamp(paths: &ProjectPaths, control: &str) -> Result<()> {
    atomic_durable_write(&paths.config_path, control.as_bytes())
        .context("writing the archive's format stamp")
}

/// The stores the bring-up makes: those `layout` adds to the released one that are not there, or
/// every store where none of the released ones is (a new archive). A released store missing beside
/// the others is a loss, not something the upgrade adds: it stops the bring-up. The scratch store
/// is not one: the store makes it at every open, and nothing in it outlives a session.
fn adding(paths: &ProjectPaths, layout: ArchiveLayout) -> Result<Vec<PathBuf>> {
    let scratch = vault_layout::scratch_database();
    let released = vault_layout::databases_in(ArchiveLayout::BrowsingV1);
    let (mut missing, mut lost, mut kept) = (Vec::new(), Vec::new(), 0);
    for db in vault_layout::databases_in(layout).into_iter().filter(|db| *db != scratch) {
        let relative = PathBuf::from(vault_layout::database_path(db));
        let there = exists(&paths.app_root.join(&relative))?;
        match (released.contains(&db), there) {
            (true, true) => kept += 1,
            (true, false) => lost.push(relative),
            (false, true) => {}
            (false, false) => missing.push(relative),
        }
    }
    if kept == 0 {
        missing.extend(lost);
        return Ok(missing);
    }
    if let Some(first) = lost.first() {
        anyhow::bail!(
            "archive-store-missing: {} is missing from an archive that has its other stores; \
             nothing was made or stamped",
            first.display()
        );
    }
    Ok(missing)
}

/// Makes the stores at `creating` in a new folder under `staging/`, as an open of the archive
/// would: with the archive's keys and every migration. A store whose own needs another (the
/// scratch store, or the history its views read) gets an empty one beside it there, which is not
/// moved in. Closed with everything in the main files, so a store is one file, and flushed.
fn make(
    paths: &ProjectPaths,
    layout: ArchiveLayout,
    creating: &[PathBuf],
    key: Option<&vault_crypto::DataKey>,
) -> Result<(RecordedPath, Vec<Made>)> {
    std::fs::create_dir_all(&paths.staging_dir)?;
    let folder =
        tempfile::Builder::new().prefix(FOLDER_PREFIX).tempdir_in(&paths.staging_dir)?.keep();
    let made = crate::project_paths_with_root(&folder);
    let mut wanted: Vec<Db> = creating.iter().filter_map(|path| store_of(path)).collect();
    let mut index = 0;
    while index < wanted.len() {
        for needed in vault_layout::database_spec(wanted[index]).dependencies {
            if !wanted.contains(needed) {
                wanted.push(*needed);
            }
        }
        index += 1;
    }
    let mut storage = crate::vault::storage_config(&made, layout, key, None)
        .map_err(|error| anyhow::anyhow!("preparing the stores being made: {error}"))?;
    storage.databases =
        vault_layout::startup_databases().into_iter().filter(|db| wanted.contains(db)).collect();
    let store = vault_store::Store::open(storage).context("making the stores this layout adds")?;
    store.pause()?.close()?;
    drop(store);
    sync_tree(&folder)?;
    fsync_dir(&paths.staging_dir)?;
    let made = creating
        .iter()
        .map(|relative| {
            let holds = content_identity::of(&folder.join(relative))?
                .with_context(|| format!("{} was not made", relative.display()))?;
            Ok(Made { relative: RecordedPath::new(relative)?, holds })
        })
        .collect::<Result<Vec<_>>>()?;
    // By name: the folder is absolute, and the archive's own path may be relative.
    let name = folder.file_name().context("a folder without a name")?;
    let made_in = Path::new(vault_layout::spec(ArtifactId::Staging).path.relative()).join(name);
    Ok((RecordedPath::new(&made_in)?, made))
}

/// Moves each store the record names from where it was made into its place, only where nothing is
/// there, and removes the folder; makes again, in a new folder recorded before anything leaves it,
/// any the folder lost; then removes the record. A store already in place stays and the one made
/// for it goes with the folder. A folder goes while the record still names it, so none is ever
/// left that no record names.
fn complete(
    paths: &ProjectPaths,
    mut record: CreationRecord,
    key: Option<&vault_crypto::DataKey>,
) -> Result<()> {
    let lost = move_in(paths, &record)?;
    remove_made_folder(paths, &record)?;
    if !lost.is_empty() {
        let layout = crate::admission::root_layout(&paths.app_root)?;
        (record.made_in, record.creating) = make(paths, layout, &lost, key)?;
        save(paths, &record)?;
        crate::fault_inject::checkpoint("promotion-remade")?;
        let still = move_in(paths, &record)?;
        anyhow::ensure!(still.is_empty(), "the stores made again were not where they were made");
        remove_made_folder(paths, &record)?;
    }
    crate::fault_inject::checkpoint("promotion-moved")?;
    remove_file_durably(&record_path(paths)).context("removing the store creation record")?;
    crate::fault_inject::checkpoint("promotion-record-removed")
}

/// Removes the folder a record's stores were made in, after setting aside, in the quarantine, each
/// of those stores whose place in the archive holds the same bytes: a move cut by a power cut on a
/// file system without a journal leaves one file under both names, and removing one name frees
/// data the other uses. Only the stores the record moves in can have a second name; the rest of
/// the folder (the stores made beside them, the lock) never left it.
fn remove_made_folder(paths: &ProjectPaths, record: &CreationRecord) -> Result<()> {
    let folder = paths.app_root.join(&record.made_in);
    let quarantine = paths.quarantine_dir.join(record.made_in.file_name().unwrap_or_default());
    let mut twins = Vec::new();
    for Made { relative, .. } in &record.creating {
        let (made, place) = (folder.join(relative), paths.app_root.join(relative));
        if exists(&made)?
            && exists(&place)?
            && crate::content_identity::of(&made)? == crate::content_identity::of(&place)?
        {
            twins.push(relative);
        }
    }
    for relative in twins {
        rename(&folder.join(relative), &super::install::unused(&quarantine, relative)?)?;
        crate::maintenance::notices::remember(
            &paths.app_root,
            "archive-check-disk",
            Some(&quarantine),
        );
        eprintln!(
            "PathKeep bring-up: {} may have two names after a power cut on a file system without \
             a journal; the other is kept in {} until the disk is checked",
            relative.display(),
            quarantine.display()
        );
    }
    remove(&folder)
}

/// The stores of `record` moved into their places; returns the ones its folder no longer holds
/// whole that are still missing from the archive.
fn move_in(paths: &ProjectPaths, record: &CreationRecord) -> Result<Vec<PathBuf>> {
    let folder = paths.app_root.join(&record.made_in);
    let mut lost = Vec::new();
    for Made { relative, holds } in &record.creating {
        let place = paths.app_root.join(relative);
        if exists(&place)? {
            continue;
        }
        for suffix in companions() {
            let beside = with_suffix(&place, suffix);
            anyhow::ensure!(
                !exists(&beside)?,
                "archive-store-missing: {} is there without {}; nothing was moved in",
                beside.display(),
                place.display()
            );
        }
        let made = folder.join(relative);
        if content_identity::of(&made)?.as_ref() == Some(holds) {
            rename(&made, &place)?;
            crate::fault_inject::checkpoint("promotion-store-moved")?;
        } else {
            lost.push(relative.to_path_buf());
        }
    }
    Ok(lost)
}

/// The files a store keeps beside its main file.
fn companions() -> impl Iterator<Item = &'static str> {
    vault_layout::CompanionFamily::Native
        .suffixes(CompanionUse::Install)
        .iter()
        .copied()
        .filter(|suffix| !suffix.is_empty())
}

fn with_suffix(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.as_os_str().to_owned();
    name.push(suffix);
    PathBuf::from(name)
}

/// Identify files the store must find at preflight. Only a creation record may authorize
/// an absent durable database in a stamped layout. Released layouts retain their old policy.
pub(crate) fn check_required_stores(
    paths: &ProjectPaths,
    layout: ArchiveLayout,
) -> Result<Vec<Db>> {
    if layout == ArchiveLayout::BrowsingV1 {
        return Ok(Vec::new());
    }
    let record = read(paths)?;
    Ok(vault_layout::databases_in(layout)
        .into_iter()
        .filter(|db| {
            let relative = Path::new(vault_layout::database_path(*db));
            *db != vault_layout::scratch_database()
                && !record.as_ref().is_some_and(|record| {
                    record.creating.iter().any(|made| *made.relative == *relative)
                })
        })
        .collect())
}

/// Install recovery hands a creation record here, before the store opens and without the key.
pub(crate) fn recover(paths: &ProjectPaths, record: CreationRecord) -> Result<()> {
    // A power cut during the rename that wrote the stamp can leave no control, on a file system
    // without a journal: the record holds the one it wrote, and the bring-up goes on from there.
    if crate::admission::read_control(&paths.app_root)?.is_none() {
        anyhow::ensure!(
            record.made_here(paths)?,
            "{}: there is no config.json, and the record of the stores being made is not this \
             folder's (the folder it names does not hold them); nothing was changed",
            crate::admission::Reason::ControlMissing.code()
        );
        stamp(paths, &record.control)?;
    }
    let stamped = crate::admission::root_layout(&paths.app_root)? != ArchiveLayout::BrowsingV1;
    if !stamped {
        // Nothing was moved in under it, and an older release may have created and filled any of
        // these stores since: discard the folder it names and the record, touch nothing in the
        // archive. The next open records what is missing then.
        remove(&paths.app_root.join(&record.made_in))?;
        return remove_file_durably(&record_path(paths)).context("discarding a creation record");
    }
    if move_in(paths, &record)?.is_empty() {
        remove_made_folder(paths, &record)?;
        crate::fault_inject::checkpoint("promotion-moved")?;
        remove_file_durably(&record_path(paths)).context("removing the store creation record")?;
        crate::fault_inject::checkpoint("promotion-record-removed")?;
    }
    // Otherwise the record stays: the next open has the key to make what the folder lost.
    Ok(())
}

#[cfg(test)]
pub(crate) mod tests;
