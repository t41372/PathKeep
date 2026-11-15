//! Responsibilities: bring an archive up to the layout this build writes, so that a crash at any
//! point is finished from a record, a store the layout requires never comes back empty without
//! one, and nothing already in the archive is ever deleted or written over.
//! Not responsible for: what the stores contain (their migrations), installs (`install`), or
//! deciding whether a build may use the archive (`admission`).
//!
//! The sequence (design amendment A1, revised 2026-11-15), run when the live archive is opened:
//! 1. The stores the layout requires that do not exist yet are made in a new folder under
//!    `staging/`, with the archive's keys, then closed with everything in their main files and
//!    flushed. Nothing in the archive has changed yet.
//! 2. A record names those stores and that folder. It is written where the install journal is
//!    written, under the journal's current name, which PathKeep 0.5 and earlier do not know.
//! 3. `config.json` gets the stamp. From here an older release stops at the config.
//! 4. Each store is moved into its place, only where nothing is.
//! 5. The record and the folder are removed, and the store opens.
//!
//! How this could fail, and what guards it:
//! - A crash after the stamp leaves a required store missing, and the next open would create it
//!   empty as if nothing had been there. The record says it is being made: recovery moves in what
//!   the folder holds, and an open makes again what the folder lost. Without a record a missing
//!   required store of a stamped archive is damage and the open is refused
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
//!
//! Tests: creation/tests.rs.

use super::install::{exists, remove, rename, sync_tree};
use crate::{
    ProjectPaths,
    durable_io::{atomic_durable_write, fsync_dir, remove_file_durably},
};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Component, Path, PathBuf};
use vault_layout::{ArchiveLayout, ArtifactId, CompanionUse, Db};

const IDENTITY: &str = "pathkeep-create";
const VERSION: u32 = 1;
/// The folders stores are made in, under `staging/`.
const FOLDER_PREFIX: &str = "creating-";

/// The stores being made, by their paths relative to the archive folder, and where they were made.
#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub(crate) struct CreationRecord {
    identity: String,
    version: u32,
    creating: Vec<PathBuf>,
    /// The folder they were made in, relative to the archive folder: `staging/creating-*`.
    made_in: PathBuf,
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
        .filter(|record| record.creating.iter().all(|path| store_of(path).is_some()))
        .filter(|record| is_made_in(&record.made_in))
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
    let creating = missing(paths, layout)?;
    if !creating.is_empty() {
        // A crash while stores were being made left their folder without a record: nothing names
        // it, and nothing else makes folders of this name.
        remove_unrecorded_folders(paths)?;
        let made_in = make(paths, layout, &creating, key)?;
        crate::fault_inject::checkpoint("promotion-made")?;
        let record =
            CreationRecord { identity: IDENTITY.into(), version: VERSION, creating, made_in };
        let path = record_path(paths);
        std::fs::create_dir_all(path.parent().context("a record without a folder")?)?;
        atomic_durable_write(&path, &serde_json::to_vec(&record)?)
            .context("recording the stores being made")?;
        crate::fault_inject::checkpoint("promotion-recorded")?;
        stamp(paths, &document.config)?;
        crate::fault_inject::checkpoint("promotion-stamped")?;
        complete(paths, record, key)?;
    } else {
        stamp(paths, &document.config)?;
    }
    Ok(true)
}

fn remove_unrecorded_folders(paths: &ProjectPaths) -> Result<()> {
    let entries = match std::fs::read_dir(&paths.staging_dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error.into()),
    };
    for entry in entries {
        let entry = entry?;
        if entry.file_name().to_str().is_some_and(|name| name.starts_with(FOLDER_PREFIX)) {
            remove(&entry.path())?;
        }
    }
    Ok(())
}

fn stamp(paths: &ProjectPaths, config: &crate::AppConfig) -> Result<()> {
    let stamped =
        crate::config::encode_config(config, Some(&crate::admission::written_compatibility()))?;
    atomic_durable_write(&paths.config_path, stamped.as_bytes())
        .context("writing the archive's format stamp")
}

/// The stores `layout` requires that are not there. The scratch store is not one: the store makes
/// it at every open, and nothing in it outlives a session.
fn missing(paths: &ProjectPaths, layout: ArchiveLayout) -> Result<Vec<PathBuf>> {
    let scratch = vault_layout::scratch_database();
    let mut missing = Vec::new();
    for db in vault_layout::databases_in(layout).into_iter().filter(|db| *db != scratch) {
        let relative = PathBuf::from(vault_layout::database_path(db));
        if !exists(&paths.app_root.join(&relative))? {
            missing.push(relative);
        }
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
) -> Result<PathBuf> {
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
    // By name: the folder is absolute, and the archive's own path may be relative.
    let name = folder.file_name().context("a folder without a name")?;
    Ok(Path::new(vault_layout::spec(ArtifactId::Staging).path.relative()).join(name))
}

/// Moves each store the record names from where it was made into its place, only where nothing is
/// there; makes again, in a new folder, any the first folder lost; then removes the record and the
/// folders. A store already in place stays and the one made for it goes with the folder.
fn complete(
    paths: &ProjectPaths,
    record: CreationRecord,
    key: Option<&vault_crypto::DataKey>,
) -> Result<()> {
    let lost = move_in(paths, &record)?;
    if !lost.is_empty() {
        let layout = crate::admission::root_layout(&paths.app_root)?;
        let again = make(paths, layout, &lost, key)?;
        let still = move_in(
            paths,
            &CreationRecord {
                identity: IDENTITY.into(),
                version: VERSION,
                creating: lost,
                made_in: again.clone(),
            },
        )?;
        anyhow::ensure!(still.is_empty(), "the stores made again were not where they were made");
        remove(&paths.app_root.join(again))?;
    }
    crate::fault_inject::checkpoint("promotion-moved")?;
    remove_file_durably(&record_path(paths)).context("removing the store creation record")?;
    remove(&paths.app_root.join(&record.made_in))
}

/// The stores of `record` moved into their places; returns the ones its folder no longer holds
/// that are still missing from the archive.
fn move_in(paths: &ProjectPaths, record: &CreationRecord) -> Result<Vec<PathBuf>> {
    let folder = paths.app_root.join(&record.made_in);
    let mut lost = Vec::new();
    for relative in &record.creating {
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
        if exists(&made)? {
            rename(&made, &place)?;
        } else {
            lost.push(relative.clone());
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

/// Before the store opens the live archive: every store its layout requires is there, or the
/// record says it is being made. The released layout is left as 0.4 and 0.5 had it.
pub(crate) fn check_required_stores(paths: &ProjectPaths, layout: ArchiveLayout) -> Result<()> {
    if layout == ArchiveLayout::BrowsingV1 {
        return Ok(());
    }
    let record = read(paths)?;
    let scratch = vault_layout::scratch_database();
    for db in vault_layout::databases_in(layout).into_iter().filter(|db| *db != scratch) {
        let relative = Path::new(vault_layout::database_path(db));
        let listed =
            record.as_ref().is_some_and(|record| record.creating.iter().any(|p| p == relative));
        if !listed && !exists(&paths.app_root.join(relative))? {
            anyhow::bail!(
                "archive-store-missing: {} is missing and nothing says it is being created",
                relative.display()
            );
        }
    }
    Ok(())
}

/// Install recovery hands a creation record here, before the store opens and without the key.
pub(crate) fn recover(paths: &ProjectPaths, record: CreationRecord) -> Result<()> {
    let stamped = crate::admission::root_layout(&paths.app_root)? != ArchiveLayout::BrowsingV1;
    if !stamped {
        // Nothing was moved in under it, and an older release may have created and filled any of
        // these stores since: discard the record and the folder it names, touch nothing in the
        // archive. The next open records what is missing then.
        remove_file_durably(&record_path(paths)).context("discarding a creation record")?;
        return remove(&paths.app_root.join(&record.made_in));
    }
    if move_in(paths, &record)?.is_empty() {
        crate::fault_inject::checkpoint("promotion-moved")?;
        remove_file_durably(&record_path(paths)).context("removing the store creation record")?;
        remove(&paths.app_root.join(&record.made_in))?;
    }
    // Otherwise the record stays: the next open has the key to make what the folder lost.
    Ok(())
}

#[cfg(test)]
pub(super) mod tests;
