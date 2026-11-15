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

/// The file that seals a snapshot: what it holds and each file's digest.
pub(super) const SEAL: &str = "snapshot.json";

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
        // A copy of what a restore replaced has no folder for the stores the restore left alone.
        .filter(|directory| directory.is_dir())
        .chain([root.parent().unwrap().to_owned()])
    {
        crate::durable_io::fsync_dir(&directory)?;
    }
    crate::durable_io::atomic_durable_write(&root.join(SEAL), &serde_json::to_vec(&files)?)
}

/// Makes the archive an install replaced (`install::Keep::SafetyCopy`) a copy a restore can use.
/// Its search index goes, as a snapshot has none: a restore builds its own. The rest is sealed as
/// it is.
pub(super) fn seal_replaced(copy: &Path) -> Result<()> {
    let layout = crate::admission::root_layout(copy)?;
    let sealed = database_files(layout, false);
    for rebuilt in database_files(layout, true).into_iter().filter(|file| !sealed.contains(file)) {
        install::remove(&copy.join(rebuilt))?;
    }
    seal(copy)
}

pub(crate) fn verify(root: &Path) -> Result<()> {
    let proof: Vec<FileProof> = serde_json::from_slice(&fs::read(root.join(SEAL))?)?;
    // What this snapshot must hold depends on the archive it was taken of, not on this build. Not
    // the agent conversations: a restore keeps the archive's own and never takes a copy's, so a
    // copy without them, like the one of what a restore replaced, restores all the same.
    let layout = crate::admission::root_layout(root)?;
    for required in vault_layout::required_snapshot_files(
        layout,
        false,
        &[vault_layout::DataScope::AgentHistory],
    )
    .map(PathBuf::from)
    {
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
    restore_snapshot_to(vault.paths(), Some(vault), None, source, password, None, cancel)
}

/// Recovery may replace an unopenable archive; a live Vault is optional.
///
/// A restore brings back browsing data only. This archive's agent conversations are carried over
/// to the restored one, never rolled back: they are only ever added to, and the tools delete their
/// own transcripts, so a rollback could only lose them (the user's decision, 2026-11-15). They
/// stay where they are when the copy has the same key, and are copied under the copy's key when
/// it has another (a mode change since). `password` opens the copy. `live_key` is this archive's
/// key, wanted only when no archive is open and the conversations are encrypted; an open
/// archive's own key is used otherwise.
pub fn restore_snapshot_to(
    paths: &ProjectPaths,
    current: Option<&Vault>,
    reservation: Option<std::sync::Arc<vault_store::OwnerLock>>,
    source: &Path,
    password: Option<&str>,
    live_key: Option<&vault_crypto::DataKey>,
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
    // The copy's key, from its own envelope: the restored archive is under it.
    let source_paths = crate::project_paths_with_root(source);
    let key = if crate::load_config(&source_paths)?.archive_mode == ArchiveMode::Encrypted {
        Some(Vault::unlock_data_key(
            &source_paths,
            password.ok_or_else(|| {
                anyhow::anyhow!("archive-password-required: enter the archive password")
            })?,
        )?)
    } else {
        None
    };
    // Decided before anything is staged: one that needs this archive's key and has none stops
    // the restore here, with nothing changed.
    let live_key = current.and_then(Vault::data_key).or(live_key);
    let carry = agent_history_carry(paths, key.as_ref(), live_key)?;
    cancel.check()?;
    let stage = staging_root(paths)?;
    copy_snapshot(source, stage.path())?;
    let staged_paths = crate::project_paths_with_root(stage.path());
    let config = crate::load_config(&staged_paths)?;
    // The restored archive keeps the stamp of the one it replaces, or a newer compatible one the
    // snapshot carries; an older snapshot never lowers it. The stage opens below with every store
    // that stamp requires; its agent-history ones are empty, and replaced or left out below.
    let stamp = crate::config::replacement_compatibility(paths, stage.path())?;
    crate::config::write_staged_control(&staged_paths, &config, Some(&stamp))?;
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
        let pause = vault.store().pause()?;
        if carry == Carry::Convert {
            carry_over(&pause, stage.path(), key.as_ref(), &cancel)?;
        }
        pause.close()?;
        vault.close()?;
    } else if carry == Carry::Convert {
        // No archive is open, maybe because some other store is damaged: the conversations' own
        // stores are opened by themselves under this archive's key.
        let mut storage = crate::vault::storage_config(
            paths,
            vault_layout::ArchiveLayout::NEWEST,
            live_key,
            Some(owner.clone()),
        )?;
        storage.databases = agent_history_databases();
        storage.databases.push(vault_layout::scratch_database());
        let store = vault_store::Store::open(storage)?;
        let pause = store.pause()?;
        carry_over(&pause, stage.path(), key.as_ref(), &cancel)?;
        pause.close()?;
        store.close()?;
    }
    let files = match carry {
        // Left where they are, out of the installation and its journal.
        Carry::InPlace => vault_layout::installation_files_except(
            layout,
            true,
            &[vault_layout::DataScope::AgentHistory],
        ),
        // Converted ones from the stage, or the stage's empty ones; the search index of the
        // conversations is built again.
        Carry::Convert | Carry::Fresh => database_files(layout, true),
    };
    // The snapshot's settings are what a restore brings back; the swap is one hold of the
    // control's gate. The archive it replaces becomes a safety copy, so the restore can be undone.
    let control = crate::ControlGate::acquire(paths);
    let previous = install::install(
        &control,
        stage.path(),
        files,
        install::Keep::SafetyCopy(install::Before::Restore),
    )?;
    drop(control);
    let opened = Vault::open_owned(paths.clone(), config, key.as_ref(), Some(owner))?;
    Ok(InstalledArchive { vault: opened, data_key: key, previous_files: previous })
}

/// What a restore does with this archive's agent conversations.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Carry {
    /// The copy has the same key, or neither is encrypted: the files stay where they are.
    InPlace,
    /// The copy has another key: they are copied under it and installed with the copy.
    Convert,
    /// This archive has none: the restored one starts with empty stores.
    Fresh,
}

fn agent_history_databases() -> Vec<vault_store::Db> {
    vault_layout::databases_in(vault_layout::ArchiveLayout::NEWEST)
        .into_iter()
        .filter(|&db| {
            vault_layout::spec(vault_layout::db_artifact(db)).scope
                == vault_layout::DataScope::AgentHistory
        })
        .collect()
}

/// Compares keys, not key files: a password change rewraps the same key, so a copy taken before
/// one is under the same key and nothing is converted. Only a mode change makes a new key.
fn agent_history_carry(
    paths: &ProjectPaths,
    copy_key: Option<&vault_crypto::DataKey>,
    live_key: Option<&vault_crypto::DataKey>,
) -> Result<Carry> {
    let file = |db| vault_layout::path(&paths.app_root, vault_layout::db_artifact(db));
    if !agent_history_databases().into_iter().any(|db| file(db).exists()) {
        return Ok(Carry::Fresh);
    }
    let plaintext =
        crate::detect_disk_encryption_mode(&file(vault_store::Db::AgentHistoryEvidence))
            == crate::DiskEncryptionMode::Plaintext;
    // The sub-key that opens the conversations' evidence: equal exactly when the data keys are.
    let opens = |key: &vault_crypto::DataKey| {
        *key.derive(vault_crypto::KeyPurpose::AgentHistoryEvidenceDatabase).expose_bytes()
    };
    Ok(match (plaintext, copy_key, live_key) {
        (true, None, _) => Carry::InPlace,
        (true, Some(_), _) => Carry::Convert,
        (false, _, None) => anyhow::bail!(
            "agent-history-key-required: enter the archive's current password, so its agent \
             conversations can be kept; nothing was changed"
        ),
        (false, Some(copy), Some(live)) if opens(copy) == opens(live) => Carry::InPlace,
        (false, _, Some(_)) => Carry::Convert,
    })
}

/// Copies this archive's conversations into the stage under the copy's key, in place of the
/// empty stores the stage opened with.
fn carry_over(
    pause: &vault_store::Maintenance<'_>,
    stage: &Path,
    key: Option<&vault_crypto::DataKey>,
    cancel: &CancelToken,
) -> Result<()> {
    let databases = agent_history_databases();
    for &db in &databases {
        let main = vault_layout::path(stage, vault_layout::db_artifact(db));
        for suffix in
            vault_layout::CompanionFamily::Native.suffixes(vault_layout::CompanionUse::Install)
        {
            match fs::remove_file(PathBuf::from(format!("{}{suffix}", main.display()))) {
                Err(error) if error.kind() != std::io::ErrorKind::NotFound => {
                    return Err(error.into());
                }
                _ => {}
            }
        }
    }
    super::copy_databases(pause, stage, key, cancel, |_| {}, &databases)
}

/// Stages the copy's files, all but its agent conversations: a restore does not bring them back.
fn copy_snapshot(source: &Path, target: &Path) -> Result<()> {
    for relative in vault_layout::installation_files_except(
        crate::admission::root_layout(source)?,
        false,
        &[vault_layout::DataScope::AgentHistory],
    ) {
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

/// Directory snapshots keep all databases together; discovery stays keyless. One that cannot be
/// read is listed as not openable rather than hiding the others.
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
        if !path.join(SEAL).is_file() {
            continue;
        }
        let archive = path.join(vault_layout::database_path(vault_layout::primary_database()));
        let header = |archive: &Path| -> std::io::Result<(u64, [u8; 16])> {
            let mut header = [0u8; 16];
            fs::File::open(archive)?.read_exact(&mut header)?;
            Ok((fs::metadata(archive)?.len(), header))
        };
        let (size, encrypted) = match header(&archive) {
            Ok((size, header)) => (size, &header != b"SQLite format 3\0"),
            Err(_) => (0, false),
        };
        let created = fs::metadata(&path)?
            .modified()
            .ok()
            .map(|time| chrono::DateTime::<chrono::Utc>::from(time).to_rfc3339());
        let source_op = install::Before::of_copy(&entry.file_name().to_string_lossy())
            .map_or("maintenance", install::Before::op);
        let name = path.display().to_string();
        result.push(crate::RecoverySnapshot {
            id: name.clone(),
            path: name,
            created_at: created,
            size_bytes: size,
            verified_openable: size >= 4096,
            encrypted,
            source_op: source_op.into(),
            label: label.into(),
        });
    }
    Ok(result)
}
