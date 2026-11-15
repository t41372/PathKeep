//! Responsibilities: checkpoint snapshots and verified full-database restoration.
//! Not responsible for: browser checkpoint replay, passwords or retention decisions.
//! Failure modes: inconsistent multi-file copy, missing envelope, corrupted snapshot,
//! restoring only archive, incomplete inventory silently creates empty databases,
//! replacing originals before verification. Tests: maintenance::tests.
use super::{InstalledArchive, database_files, install, staging_root};
use crate::{ArchiveMode, ProjectPaths, Vault};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
};
use vault_store::CancelToken;

#[derive(Serialize, Deserialize, schemars::JsonSchema)]
pub(crate) struct FileProof {
    path: PathBuf,
    sha256: String,
}

/// A safety snapshot lands at a committed batch boundary across all databases.
pub fn snapshot(vault: &Vault, target: &Path) -> Result<()> {
    let _operation = crate::archive::ArchiveOpGate::acquire(vault.paths());
    vault.recheck_root()?;
    let pause = vault.store().pause()?;
    pause.snapshot(target)?;
    crate::config::save_copy_config(vault.paths(), target, vault.config())?;
    let envelope = vault_layout::path(&vault.paths().app_root, vault_layout::ArtifactId::Envelope);
    if envelope.exists() {
        fs::copy(envelope, vault_layout::path(target, vault_layout::ArtifactId::Envelope))?;
    }
    seal(target)
}

pub(crate) fn seal(root: &Path) -> Result<()> {
    // The snapshot's own control, written just before, says which stores it holds.
    let layout = crate::admission::root_layout(root)?;
    let files = database_files(layout, false)
        .into_iter()
        .filter(|p| root.join(p).is_file())
        .map(|path| {
            crate::durable_io::fsync_file_durably(&root.join(&path))?;
            Ok(FileProof { sha256: hash_file(&root.join(&path))?, path })
        })
        .collect::<Result<Vec<_>>>()?;
    for directory in vault_layout::snapshot_directories(layout)
        .into_iter()
        .map(|relative| root.join(relative))
        .chain([root.parent().unwrap().to_owned()])
    {
        crate::durable_io::fsync_dir(&directory)?;
    }
    crate::durable_io::atomic_durable_write(
        &root.join("snapshot.json"),
        &serde_json::to_vec(&files)?,
    )
}

pub(crate) fn verify(root: &Path) -> Result<()> {
    let proof: Vec<FileProof> = serde_json::from_slice(&fs::read(root.join("snapshot.json"))?)?;
    // What this snapshot must hold depends on the archive it was taken of, not on this build.
    let layout = crate::admission::root_layout(root)?;
    for required in vault_layout::required_snapshot_files(layout, false).map(PathBuf::from) {
        anyhow::ensure!(
            proof.iter().any(|file| file.path == required),
            "snapshot is missing {}",
            required.display()
        );
    }
    let config = crate::load_config(&crate::project_paths_with_root(root))?;
    if config.archive_mode == ArchiveMode::Encrypted {
        anyhow::ensure!(
            proof.iter().any(|file| file.path
                == Path::new(
                    vault_layout::spec(vault_layout::ArtifactId::Envelope).path.relative()
                )),
            "snapshot is missing keys.envelope"
        );
    }
    for file in proof {
        anyhow::ensure!(
            hash_file(&root.join(&file.path))? == file.sha256,
            "snapshot-digest-mismatch: {}",
            file.path.display()
        );
    }
    Ok(())
}

pub(crate) fn hash_file(path: &Path) -> Result<String> {
    let mut file = fs::File::open(path)?;
    let mut hash = Sha256::new();
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let n = file.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        hash.update(&buffer[..n]);
    }
    Ok(format!("{:x}", hash.finalize()))
}

/// Verifies and builds the replacement before closing the original owner.
pub fn restore_snapshot(
    vault: &Vault,
    source: &Path,
    password: Option<&str>,
    cancel: CancelToken,
) -> Result<InstalledArchive> {
    restore_snapshot_to(vault.paths(), Some(vault), None, source, password, cancel)
}

/// Recovery may replace an unopenable archive; a live Vault is optional.
pub fn restore_snapshot_to(
    paths: &ProjectPaths,
    current: Option<&Vault>,
    reservation: Option<std::sync::Arc<vault_store::OwnerLock>>,
    source: &Path,
    password: Option<&str>,
    cancel: CancelToken,
) -> Result<InstalledArchive> {
    let _operation = crate::archive::ArchiveOpGate::acquire(paths);
    // A damaged or missing control is what a restore repairs. One that needs a newer PathKeep
    // is not damage: replacing it would put an older archive over a newer one.
    crate::admission::refuse_newer(&paths.app_root)?;
    // A delete-all-data the user confirmed has not finished. Installing an archive now would put
    // it where that wipe, once finished, deletes it.
    anyhow::ensure!(
        !crate::data_wipe_interrupted(paths),
        "archive-wipe-pending: a delete-all-data has not finished; it finishes before anything is \
         restored or imported"
    );
    verify(source)?;
    // The snapshot's own control, read where it lies, before anything is staged in the target.
    if let Some(control) = crate::admission::read_control(source)? {
        crate::config::decode_config(source, &control)?;
    }
    cancel.check()?;
    let stage = staging_root(paths)?;
    copy_snapshot(source, stage.path())?;
    let staged_paths = crate::project_paths_with_root(stage.path());
    let config = crate::load_config(&staged_paths)?;
    let key = if config.archive_mode == ArchiveMode::Encrypted {
        Some(Vault::unlock_data_key(
            &staged_paths,
            password.ok_or_else(|| {
                anyhow::anyhow!("archive-password-required: enter the archive password")
            })?,
        )?)
    } else {
        None
    };
    let staged = Vault::open_staged(staged_paths, config.clone(), key.as_ref())?;
    let layout = staged.layout();
    for db in vault_layout::snapshot_databases(layout) {
        let result = staged.store().read(db, |s| s.integrity_check())?;
        anyhow::ensure!(result == ["ok"], "snapshot integrity: {db:?}: {result:?}");
    }
    staged.search().rebuild_with_control(cancel.clone(), |_| {})?;
    staged.close()?;
    cancel.check()?;
    let owner = match (current, reservation) {
        (Some(vault), _) => vault.store().owner()?,
        (_, Some(owner)) => owner,
        _ => std::sync::Arc::new(vault_store::OwnerLock::acquire(&paths.app_root)?),
    };
    if let Some(vault) = current {
        vault.store().pause()?.close()?;
        vault.close()?;
    }
    let previous = install::install(paths, stage.path(), database_files(layout, true), true)?;
    let opened = Vault::open_owned(paths.clone(), config, key.as_ref(), Some(owner))?;
    Ok(InstalledArchive { vault: opened, data_key: key, previous_files: previous })
}

fn copy_snapshot(source: &Path, target: &Path) -> Result<()> {
    for relative in database_files(crate::admission::root_layout(source)?, false) {
        if source.join(&relative).is_file() {
            let file = target.join(relative);
            fs::create_dir_all(file.parent().unwrap())?;
            fs::copy(source.join(file.strip_prefix(target)?), &file)?;
            crate::durable_io::fsync_file_durably(&file)?;
        }
    }
    Ok(())
}

/// Startup runs this before opening a Store; no password is required to roll back.
pub fn recover_native_maintenance(paths: &ProjectPaths) -> Result<bool> {
    let recovered = install::recover(paths)?;
    // Copying has not changed live files yet. Its abandoned destinations can be
    // discarded; the separately named SQLCipher staging tree remains resumable.
    if paths.staging_dir.exists() {
        for entry in fs::read_dir(&paths.staging_dir)? {
            let entry = entry?;
            if entry.file_type()?.is_dir()
                && vault_layout::family_pattern(vault_layout::ArtifactId::MaintenanceStages)
                    .matches(&entry.file_name().to_string_lossy())
            {
                fs::remove_dir_all(entry.path())?;
            }
        }
    }
    Ok(recovered)
}

/// Directory snapshots keep all databases together; discovery stays keyless.
pub fn list_snapshots(paths: &ProjectPaths) -> Result<Vec<crate::RecoverySnapshot>> {
    let root = paths.artifact_path(vault_layout::ArtifactId::NativeSnapshots);
    if !root.exists() {
        return Ok(Vec::new());
    }
    let label = match crate::load_config(paths).unwrap_or_default().preferred_language {
        crate::LanguagePreference::ZhCn | crate::LanguagePreference::ZhTw => "安全快照",
        _ => "Safety snapshot",
    };
    let mut result = Vec::new();
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        let path = entry.path();
        if !path.join("snapshot.json").is_file() {
            continue;
        }
        let archive = path.join(vault_layout::database_path(vault_layout::primary_database()));
        let size = fs::metadata(&archive)?.len();
        let mut header = [0u8; 16];
        fs::File::open(&archive)?.read_exact(&mut header)?;
        let created = fs::metadata(&path)?
            .modified()
            .ok()
            .map(|time| chrono::DateTime::<chrono::Utc>::from(time).to_rfc3339());
        let name = path.display().to_string();
        result.push(crate::RecoverySnapshot {
            id: name.clone(),
            path: name,
            created_at: created,
            size_bytes: size,
            verified_openable: size >= 4096,
            encrypted: &header != b"SQLite format 3\0",
            source_op: "maintenance".into(),
            label: label.into(),
        });
    }
    Ok(result)
}
