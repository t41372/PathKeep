//! Responsibilities: bring an archive up to the layout this build writes, stamp first, so that a
//! crash at any point is finished from a record, and a store the layout requires never comes
//! back empty without one.
//! Not responsible for: what the stores contain (their migrations), installs (`install`), or
//! deciding whether a build may use the archive (`admission`).
//!
//! The sequence (design amendment A1), run when the live archive is opened:
//! 1. A record names the stores the layout requires that do not exist yet. It is written where
//!    the install journal is written, under the journal's current name, which PathKeep 0.5 and
//!    earlier do not know.
//! 2. `config.json` gets the stamp. From here an older release stops at the config.
//! 3. The store opens and creates the stores the record names.
//! 4. The record is removed.
//!
//! How this could fail, and what guards it:
//! - A crash after the stamp leaves a required store missing, and the next open creates it empty
//!   as if nothing had been there. The record says it was being created; without a record a
//!   missing required store of a stamped archive is damage and the open is refused
//!   (`check_required_stores`), because the store layer would otherwise create an empty file.
//! - A crash before the stamp, then PathKeep 0.5 opens the folder (it can: there is no stamp) and
//!   creates and fills some of the stores the record names. Deleting what the record names would
//!   delete that. Before the stamp the record is only discarded; nothing it names is touched.
//! - A crash after the stamp in the middle of creating a store leaves a half-written file. After
//!   the stamp no older release can have written to it, and this build removes the record before
//!   it writes anything else, so the named files are deleted and created again.
//!
//! Tests: creation/tests.rs.

use crate::{
    ProjectPaths,
    durable_io::{atomic_durable_write, remove_file_durably},
};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
};
use vault_layout::{ArchiveLayout, ArtifactId, CompanionUse};

const IDENTITY: &str = "pathkeep-create";
const VERSION: u32 = 1;

/// The stores being created, by their paths relative to the archive folder.
#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub(crate) struct CreationRecord {
    identity: String,
    version: u32,
    creating: Vec<PathBuf>,
}

fn record_path(paths: &ProjectPaths) -> PathBuf {
    vault_layout::path(&paths.app_root, ArtifactId::InstallJournalV2)
}

/// The record, when the journal file holds one. `None` when the file is absent or holds an
/// install journal.
pub(crate) fn read(paths: &ProjectPaths) -> Result<Option<CreationRecord>> {
    let bytes = match fs::read(record_path(paths)) {
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
        .filter(|record| record.creating.iter().all(|path| is_store(path)))
        .with_context(|| {
            format!(
                "{}: the store creation record is not one this build can act on",
                crate::admission::Reason::ControlInvalid.code()
            )
        })?;
    Ok(Some(record))
}

/// Only paths of stores a layout this build knows requires; a record names nothing else.
fn is_store(relative: &Path) -> bool {
    vault_layout::databases_in(ArchiveLayout::NEWEST)
        .into_iter()
        .any(|db| Path::new(vault_layout::database_path(db)) == relative)
}

/// Stamps the live archive and records the stores it now lacks, when it is a released-format
/// archive that is set up. The caller holds the owner lock and has run install recovery.
pub(crate) fn promote_if_needed(paths: &ProjectPaths) -> Result<bool> {
    // The stamp is a write of the control, read and written under its gate.
    let _control = crate::ControlGate::acquire(paths);
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
    let creating: Vec<PathBuf> = vault_layout::databases_in(layout)
        .into_iter()
        .map(|db| PathBuf::from(vault_layout::database_path(db)))
        .filter(|relative| fs::symlink_metadata(paths.app_root.join(relative)).is_err())
        .collect();
    let record = CreationRecord { identity: IDENTITY.into(), version: VERSION, creating };
    let path = record_path(paths);
    fs::create_dir_all(path.parent().context("a record without a folder")?)?;
    atomic_durable_write(&path, &serde_json::to_vec(&record)?)
        .context("recording the stores being created")?;
    crate::fault_inject::checkpoint("promotion-recorded")?;
    let stamped = crate::config::encode_config(
        &document.config,
        Some(&crate::admission::written_compatibility()),
    )?;
    atomic_durable_write(&paths.config_path, stamped.as_bytes())
        .context("writing the archive's format stamp")?;
    crate::fault_inject::checkpoint("promotion-stamped")?;
    Ok(true)
}

/// Before the store opens the live archive: every store its layout requires is there, or the
/// record says it is being created. The released layout is left as 0.4 and 0.5 had it.
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
        if !listed && fs::symlink_metadata(paths.app_root.join(relative)).is_err() {
            anyhow::bail!(
                "archive-store-missing: {} is missing and nothing says it is being created",
                relative.display()
            );
        }
    }
    Ok(())
}

/// After the store opened the live archive: the stores the record named exist now.
pub(crate) fn finish(paths: &ProjectPaths) -> Result<()> {
    if read(paths)?.is_some() {
        crate::fault_inject::checkpoint("promotion-created")?;
        remove_file_durably(&record_path(paths)).context("removing the store creation record")?;
    }
    Ok(())
}

/// Install recovery hands a creation record here, before the store opens.
pub(crate) fn recover(paths: &ProjectPaths, record: CreationRecord) -> Result<()> {
    let stamped = crate::admission::root_layout(&paths.app_root)? != ArchiveLayout::BrowsingV1;
    if !stamped {
        // Nothing was created under it, and an older release may have created and filled any of
        // these stores since: discard the record, touch nothing it names. The next open records
        // what is missing then.
        return remove_file_durably(&record_path(paths)).context("discarding a creation record");
    }
    for relative in &record.creating {
        let main = paths.app_root.join(relative);
        let suffixes = vault_layout::CompanionFamily::Native.suffixes(CompanionUse::Install);
        for suffix in suffixes {
            let file = PathBuf::from(format!("{}{suffix}", main.display()));
            remove_file_durably(&file)
                .with_context(|| format!("removing a half-created {}", file.display()))?;
        }
    }
    // The record stays until the next open has created the stores.
    Ok(())
}

#[cfg(test)]
mod tests;
