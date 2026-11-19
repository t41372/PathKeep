//! Responsibilities: rewrap passwords, copy encryption modes and coordinate native maintenance.
//! Not responsible for: cipher implementations, SQLCipher decoding, AI internals or UI state.
//! Failure modes: wrong-password mutation, changed data key on rewrap, missing database,
//! cancelled copy replacing originals, mixed generations or old-key search after a swap.
//! Tests: maintenance::tests and install_tests on real plaintext/encrypted roots.
mod audit;
mod retention;
pub use retention::{preview_retention, prune};
pub(crate) mod creation;
pub mod install;
pub use audit::{record_mode_change, record_restore};
pub(crate) mod snapshot;
pub use snapshot::{
    list_snapshots, recover_native_maintenance, restore_snapshot, restore_snapshot_to, snapshot,
};
#[cfg(test)]
mod install_tests;
#[cfg(test)]
mod move_copies_tests;
#[cfg(test)]
mod restore_tests;
#[cfg(test)]
mod retention_tests;
#[cfg(all(test, unix))]
mod rewrap_crash_tests;
#[cfg(test)]
mod safety_copy_tests;
#[cfg(test)]
pub(crate) mod tests;

use crate::{AppConfig, ArchiveMode, ProjectPaths, Vault};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};
use vault_crypto::{DataKey, Envelope, KdfParams, write_envelope_file};
use vault_store::{CancelToken, Db};

/// Polling and channel transports share these phase/counter values.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Progress {
    pub phase: String,
    pub database: Option<String>,
    pub table: Option<String>,
    pub processed: u64,
    pub total: u64,
}

/// Native owners install these backend-only values in their session after a swap.
/// Neither the data key nor the Vault is serialized to a client.
#[derive(Debug)]
pub struct InstalledArchive {
    pub vault: Vault,
    pub data_key: Option<DataKey>,
    pub previous_files: PathBuf,
    /// Why the archive this replaced, kept as a safety copy, could not be sealed, when it could not
    /// ([`install::Replaced::unsealed`]): the restore or import that made it says so.
    pub previous_unsealed: Option<String>,
}

/// Changes only the envelope. Existing database, index and keychain keys stay valid.
pub fn rewrap_password(vault: &Vault, current: &str, new: &str) -> Result<()> {
    let _operation = crate::archive::ArchiveOpGate::acquire(vault.paths());
    vault.recheck_root()?;
    let _guard = crate::vault::ENVELOPE_ACCESS.lock().unwrap_or_else(|e| e.into_inner());
    // Keep the established typed wrong-password error at the authentication boundary.
    let _key = Vault::unlock_data_key(vault.paths(), current)?;
    let path = vault_layout::path(&vault.paths().app_root, vault_layout::password_envelope());
    let envelope = Envelope::parse(&vault_crypto::read_envelope_file(&path)?)?;
    let wrapped = envelope.rewrap(current.as_bytes(), new.as_bytes(), KdfParams::RECOMMENDED)?;
    write_envelope_file(&path, &wrapped)?;
    Ok(())
}

/// Store preflight authenticates all files; this repairs only a stale mode setting.
pub fn reconcile(vault: &Vault) -> Result<crate::ReconcileReport> {
    use std::io::Read;
    // A mode change replaces the files whose header this reads.
    let _operation = crate::archive::ArchiveOpGate::acquire(vault.paths());
    vault.recheck_root()?;
    let mut header = [0u8; 16];
    fs::File::open(
        vault.paths().artifact_path(vault_layout::db_artifact(vault_layout::primary_database())),
    )?
    .read_exact(&mut header)?;
    let mode = if &header == b"SQLite format 3\0" {
        ArchiveMode::Plaintext
    } else {
        ArchiveMode::Encrypted
    };
    // The control as it is now, not the vault's copy from when it was opened: a setting saved
    // since would otherwise be written back over.
    let gate = crate::ControlGate::acquire(vault.paths());
    let mut config = crate::load_config(vault.paths())?;
    let previous = config.archive_mode.clone();
    let repaired = previous != mode;
    if repaired {
        config.archive_mode = mode.clone();
        crate::save_config(&gate, &config)?;
    }
    Ok(crate::ReconcileReport { repaired, from_mode: repaired.then_some(previous), to_mode: mode })
}

/// Every database is copied and verified before the live owner is closed. A
/// cancellation during copying or index construction drops only staged output.
pub fn switch_mode(
    vault: &Vault,
    current: Option<&str>,
    mode: ArchiveMode,
    new_password: Option<&str>,
    cancel: CancelToken,
    progress: impl FnMut(Progress) + Send + 'static,
) -> Result<InstalledArchive> {
    let _operation = crate::archive::ArchiveOpGate::acquire(vault.paths());
    vault.recheck_root()?;
    cancel.check()?;
    if vault.config().archive_mode == ArchiveMode::Encrypted {
        Vault::unlock_data_key(
            vault.paths(),
            current.context("archive-password-required: enter the archive password")?,
        )?;
    }
    anyhow::ensure!(
        mode != vault.config().archive_mode,
        "use password rewrap for the existing mode"
    );
    let stage = staging_root(vault.paths())?;
    let mode_after = mode.clone();
    let mut config = vault.config().clone();
    config.archive_mode = mode;
    let key = create_envelope(stage.path(), &config, new_password)?;
    let owner = vault.store().owner()?;
    let pause = vault.pause_store()?;
    let progress = Arc::new(Mutex::new(progress));
    let copy_progress = progress.clone();
    copy_databases(
        &pause,
        stage.path(),
        key.as_ref(),
        &cancel,
        move |p| (copy_progress.lock().unwrap())(p),
        &vault_layout::mode_change_databases(vault.layout()).collect::<Vec<_>>(),
    )?;
    crate::config::save_copy_config(vault.paths(), stage.path(), &config)?;
    let staged = Vault::open_staged(
        crate::project_paths_with_root(stage.path()),
        config.clone(),
        key.as_ref(),
    )?;
    cancel.check()?;
    let index_progress = progress.clone();
    staged.search().rebuild_with_control(cancel.clone(), move |p| {
        (index_progress.lock().unwrap())(Progress {
            phase: "indexing".into(),
            database: None,
            table: None,
            processed: p.documents,
            total: 0,
        })
    })?;
    cancel.check()?;
    (progress.lock().unwrap())(Progress {
        phase: "snapshotting".into(),
        database: None,
        table: None,
        processed: 0,
        total: 0,
    });
    let snapshot_relative = PathBuf::from(
        vault_layout::spec(vault_layout::ArtifactId::NativeSnapshots).path.relative(),
    )
    .join(format!("rekey-{}", stage.path().file_name().unwrap().to_string_lossy()));
    snapshot(&staged, &stage.path().join(&snapshot_relative))?;
    staged.close()?;
    cancel.check()?;
    pause.close()?;
    // The replacement's control is the live one as it is now, with the new mode: a setting saved
    // while the copy ran is kept. Built and installed under one hold of the control's gate.
    let control = crate::ControlGate::acquire(vault.paths());
    let mut config = crate::load_config(vault.paths())?;
    config.archive_mode = mode_after;
    crate::config::save_copy_config(vault.paths(), stage.path(), &config)?;
    let mut files = database_files(vault.layout(), true);
    files.push(snapshot_relative.clone());
    install::install(&control, stage.path(), files, install::Keep::Nothing)?;
    drop(control);
    let previous = vault.paths().app_root.join(snapshot_relative);
    let opened = Vault::open_owned(vault.paths().clone(), config, key.as_ref(), Some(owner))?;
    // The safety snapshot taken before the change is sealed when it is taken.
    Ok(InstalledArchive {
        vault: opened,
        data_key: key,
        previous_files: previous,
        previous_unsealed: None,
    })
}

/// A private folder for one operation on the archive's file system, removed when dropped.
pub fn staging_root(paths: &ProjectPaths) -> Result<tempfile::TempDir> {
    fs::create_dir_all(&paths.staging_dir)?;
    Ok(tempfile::Builder::new()
        .prefix(vault_layout::family_prefix(vault_layout::ArtifactId::MaintenanceStages))
        .tempdir_in(&paths.staging_dir)?)
}

pub(crate) fn create_envelope(
    root: &Path,
    config: &AppConfig,
    password: Option<&str>,
) -> Result<Option<DataKey>> {
    if config.archive_mode == ArchiveMode::Plaintext {
        return Ok(None);
    }
    let (envelope, key) = Envelope::create(
        password.context("new archive password is required")?.as_bytes(),
        KdfParams::RECOMMENDED,
    )?;
    fs::create_dir_all(vault_layout::path(root, vault_layout::ArtifactId::ArchiveRoot))?;
    write_envelope_file(&vault_layout::path(root, vault_layout::ArtifactId::Envelope), &envelope)?;
    Ok(Some(key))
}

/// The same complete copy serves mode changes and portable bundles. ATTACH keeps
/// rows inside the engine; it never materializes a database-sized Rust collection.
pub(crate) fn copy_databases(
    pause: &vault_store::Maintenance<'_>,
    root: &Path,
    key: Option<&DataKey>,
    cancel: &CancelToken,
    progress: impl FnMut(Progress) + Send + 'static,
    databases: &[Db],
) -> Result<()> {
    let targets = vault_store::Paths::under(root);
    let progress = Arc::new(Mutex::new(progress));
    for &db in databases {
        let sink = progress.clone();
        let subkey = key.map(|key| key.derive(vault_layout::database_spec(db).key));
        pause.copy_database(
            db,
            targets.get(db).into(),
            subkey.as_ref().map(|k| *k.expose_bytes()),
            cancel.clone(),
            move |p| {
                (sink.lock().unwrap())(Progress {
                    phase: match p.phase {
                        vault_store::CopyPhase::Verifying => "verifying",
                        vault_store::CopyPhase::Copying => "copying",
                        vault_store::CopyPhase::Indexes => "indexing",
                    }
                    .into(),
                    database: Some(format!("{db:?}")),
                    table: Some(p.table),
                    processed: p.copied,
                    total: p.total,
                });
            },
        )?;
    }
    Ok(())
}

/// The complete installation set of an archive of `layout`, which also removes WALs and a
/// superseded envelope.
pub fn database_files(layout: vault_layout::ArchiveLayout, search: bool) -> Vec<PathBuf> {
    vault_layout::installation_files(layout, search)
}
