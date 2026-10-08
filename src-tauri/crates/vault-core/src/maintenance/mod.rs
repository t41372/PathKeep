//! Responsibilities: rewrap passwords, copy encryption modes and coordinate native maintenance.
//! Not responsible for: cipher implementations, SQLCipher decoding, AI internals or UI state.
//! Failure modes: wrong-password mutation, changed data key on rewrap, missing database,
//! cancelled copy replacing originals, mixed generations or old-key search after a swap.
//! Tests: maintenance::tests and install_tests on real plaintext/encrypted roots.
mod audit;
mod retention;
pub use retention::{preview_retention, prune};
pub mod install;
pub use audit::{record_mode_change, record_restore};
mod snapshot;
pub use snapshot::{
    list_snapshots, recover_native_maintenance, restore_snapshot, restore_snapshot_to, snapshot,
};
#[cfg(test)]
mod install_tests;
#[cfg(all(test, unix))]
mod rewrap_crash_tests;
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

pub const DURABLE_DATABASES: [Db; 4] =
    [Db::Archive, Db::SourceEvidence, Db::Intelligence, Db::Agent];

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
}

/// Changes only the envelope. Existing database, index and keychain keys stay valid.
pub fn rewrap_password(vault: &Vault, current: &str, new: &str) -> Result<()> {
    let _operation = crate::archive::ArchiveOpGate::acquire(vault.paths());
    let _guard = crate::vault::ENVELOPE_ACCESS.lock().unwrap_or_else(|e| e.into_inner());
    // Keep the established typed wrong-password error at the authentication boundary.
    let _key = Vault::unlock_data_key(vault.paths(), current)?;
    let path = vault.paths().app_root.join("archive/keys.envelope");
    let envelope = Envelope::parse(&vault_crypto::read_envelope_file(&path)?)?;
    let wrapped = envelope.rewrap(current.as_bytes(), new.as_bytes(), KdfParams::RECOMMENDED)?;
    write_envelope_file(&path, &wrapped)?;
    Ok(())
}

/// Store preflight authenticates all files; this repairs only a stale mode setting.
pub fn reconcile(vault: &Vault) -> Result<crate::ReconcileReport> {
    use std::io::Read;
    let mut header = [0u8; 16];
    fs::File::open(&vault.paths().archive_database_path)?.read_exact(&mut header)?;
    let mode = if &header == b"SQLite format 3\0" {
        ArchiveMode::Plaintext
    } else {
        ArchiveMode::Encrypted
    };
    let previous = vault.config().archive_mode.clone();
    let repaired = previous != mode;
    if repaired {
        let mut config = vault.config().clone();
        config.archive_mode = mode.clone();
        crate::save_config(vault.paths(), &config)?;
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
    let mut config = vault.config().clone();
    config.archive_mode = mode;
    let key = create_envelope(stage.path(), &config, new_password)?;
    let owner = vault.store().owner()?;
    let pause = vault.store().pause()?;
    let progress = Arc::new(Mutex::new(progress));
    let copy_progress = progress.clone();
    copy_databases(
        &pause,
        stage.path(),
        key.as_ref(),
        &cancel,
        move |p| (copy_progress.lock().unwrap())(p),
        DURABLE_DATABASES.as_slice(),
    )?;
    crate::save_config(&crate::project_paths_with_root(stage.path()), &config)?;
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
    let snapshot_relative = PathBuf::from("raw-snapshots/maintenance")
        .join(format!("rekey-{}", stage.path().file_name().unwrap().to_string_lossy()));
    snapshot(&staged, &stage.path().join(&snapshot_relative))?;
    staged.close()?;
    cancel.check()?;
    pause.close()?;
    vault.close()?;
    let mut files = database_files(true);
    files.push(snapshot_relative.clone());
    install::install(vault.paths(), stage.path(), files, false)?;
    let previous = vault.paths().app_root.join(snapshot_relative);
    let opened = Vault::open_owned(vault.paths().clone(), config, key.as_ref(), Some(owner))?;
    Ok(InstalledArchive { vault: opened, data_key: key, previous_files: previous })
}

/// A private folder for one operation on the archive's file system, removed when dropped.
pub fn staging_root(paths: &ProjectPaths) -> Result<tempfile::TempDir> {
    fs::create_dir_all(&paths.staging_dir)?;
    Ok(tempfile::Builder::new().prefix("maintenance-").tempdir_in(&paths.staging_dir)?)
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
    fs::create_dir_all(root.join("archive"))?;
    write_envelope_file(&root.join("archive/keys.envelope"), &envelope)?;
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
        let subkey = key.map(|key| {
            key.derive(match db {
                Db::Archive => vault_crypto::KeyPurpose::ArchiveDatabase,
                Db::SourceEvidence => vault_crypto::KeyPurpose::SourceEvidenceDatabase,
                Db::Intelligence => vault_crypto::KeyPurpose::IntelligenceDatabase,
                Db::Agent => vault_crypto::KeyPurpose::AgentDatabase,
                Db::Scratch => vault_crypto::KeyPurpose::ScratchDatabase,
            })
        });
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

/// The complete installation set also removes WALs and a superseded envelope.
pub fn database_files(search: bool) -> Vec<PathBuf> {
    let mut files = Vec::new();
    for base in [
        "archive/history-vault.db",
        "archive/source-evidence.db",
        "derived/intelligence.db",
        "derived/agent.db",
        "derived/scratch.db",
    ] {
        for suffix in ["", "-wal", "-shm", "-tshm"] {
            files.push(format!("{base}{suffix}").into());
        }
    }
    files.extend(
        ["archive/keys.envelope", "archive/keys.envelope.pending", "config.json"]
            .into_iter()
            .map(PathBuf::from),
    );
    if search {
        files.push("derived/search-index".into());
    }
    files
}
