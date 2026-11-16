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
/// A restore brings back browsing data, never the archive's encryption or its agent
/// conversations (the user's decisions, 2026-11-15):
///
/// - The archive keeps its encryption mode, key and password. The copy's data is written under
///   them, converted when the copy was taken under another mode or key (a mode change since).
///   Restoring a copy from before encryption was turned on must not turn it off and write
///   everything, conversations included, unencrypted; nor put back a password changed since.
/// - The agent conversations stay where they are: they are only ever added to, and the tools
///   delete their own transcripts, so a rollback could only lose them.
///
/// `password` opens the copy when it is encrypted. `live_key` is this archive's key, wanted when
/// no archive is open and its files are encrypted; an open archive's own key is used otherwise. A
/// folder with no archive in it takes the copy's encryption, having none of its own to keep.
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
    let source_paths = crate::project_paths_with_root(source);
    let copy_mode = crate::load_config(&source_paths)?.archive_mode;
    let copy_key = if copy_mode == ArchiveMode::Encrypted {
        Some(Vault::unlock_data_key(
            &source_paths,
            password.ok_or_else(|| {
                anyhow::anyhow!("archive-password-required: enter the password of this copy")
            })?,
        )?)
    } else {
        None
    };
    // What the restored archive is under, decided before anything is staged: one that needs this
    // archive's key and has none stops here, with nothing changed.
    let target = Encryption::of_archive(paths, current.and_then(Vault::data_key).or(live_key))?
        .unwrap_or_else(|| Encryption::of_copy(source, copy_key.as_ref().map(owned)));
    let conversations_here = agent_history_databases()
        .into_iter()
        .any(|db| vault_layout::path(&paths.app_root, vault_layout::db_artifact(db)).exists());
    cancel.check()?;
    let stage = staging_root(paths)?;
    copy_snapshot(source, stage.path())?;
    let staged_paths = crate::project_paths_with_root(stage.path());
    let mut config = crate::load_config(&staged_paths)?;
    // The restored archive keeps the stamp of the one it replaces, or a newer compatible one the
    // snapshot carries; an older snapshot never lowers it. The stage opens below with every store
    // that stamp requires; its agent-history ones are empty, and left out below unless this
    // folder has none.
    let stamp = crate::config::replacement_compatibility(paths, stage.path())?;
    if !target.opens(copy_key.as_ref()) {
        crate::config::write_staged_control(&staged_paths, &config, Some(&stamp))?;
        convert_stage(stage.path(), &config, copy_key.as_ref(), target.key.as_ref(), &cancel)?;
    }
    config.archive_mode = target.mode();
    crate::config::write_staged_control(&staged_paths, &config, Some(&stamp))?;
    if let Some(envelope) = &target.envelope {
        let staged_envelope = vault_layout::path(stage.path(), vault_layout::password_envelope());
        fs::create_dir_all(staged_envelope.parent().unwrap())?;
        fs::copy(envelope, &staged_envelope)?;
        crate::durable_io::fsync_file_durably(&staged_envelope)?;
    }
    let staged = Vault::open_staged(staged_paths.clone(), config.clone(), target.key.as_ref())?;
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
    let files = if conversations_here {
        // Left where they are, out of the installation and its journal.
        vault_layout::installation_files_except(
            layout,
            true,
            &[vault_layout::DataScope::AgentHistory],
        )
    } else {
        // This folder has none: the stage's empty stores, under the restored archive's key.
        database_files(layout, true)
    };
    // The copy's settings come back, but not what belongs to this computer (App Lock, whether
    // the keychain remembers the key), as with an import: a restore must not turn App Lock off or
    // put the key in a keychain the user stopped using. Read as they are now, before the archive
    // closes, and installed in the same hold of the control's gate. The archive it replaces
    // becomes a safety copy, so the restore can be undone.
    let control = crate::ControlGate::acquire(paths);
    crate::migration::native::keep_machine_settings(
        &mut config,
        &crate::migration::native::settings_here(paths)?,
    );
    crate::config::write_staged_control(&staged_paths, &config, Some(&stamp))?;
    if let Some(vault) = current {
        vault.store().pause()?.close()?;
        vault.close()?;
    }
    let previous = install::install(
        &control,
        stage.path(),
        files,
        install::Keep::SafetyCopy(install::Before::Restore),
    )?;
    drop(control);
    let opened = Vault::open_owned(paths.clone(), config, target.key.as_ref(), Some(owner))?;
    Ok(InstalledArchive { vault: opened, data_key: target.key, previous_files: previous })
}

/// The encryption a restored archive is under: the archive's own, or the copy's when the folder
/// has no archive to keep it from.
struct Encryption {
    key: Option<vault_crypto::DataKey>,
    /// The envelope installed with it: the archive's own, or the copy's.
    envelope: Option<PathBuf>,
}

impl Encryption {
    /// The encryption of the archive in `paths`: encrypted when it has a key file (one written
    /// on its first open counts) or its key is given, unencrypted otherwise. Not by its control,
    /// which can be what is damaged, nor by its stores' headers, which a damaged store garbles.
    /// `None` when the folder holds no archive at all.
    fn of_archive(
        paths: &ProjectPaths,
        key: Option<&vault_crypto::DataKey>,
    ) -> Result<Option<Encryption>> {
        let envelope =
            [vault_layout::password_envelope(), vault_layout::ArtifactId::PendingEnvelope]
                .into_iter()
                .map(|id| vault_layout::path(&paths.app_root, id))
                .find(|file| file.is_file());
        let stores = [vault_layout::primary_database(), vault_store::Db::AgentHistoryEvidence];
        let archive_here = envelope.is_some()
            || stores.into_iter().any(|db| {
                vault_layout::path(&paths.app_root, vault_layout::db_artifact(db)).is_file()
            });
        if !archive_here {
            return Ok(None);
        }
        Ok(Some(match (envelope, key) {
            (None, None) => Encryption { key: None, envelope: None },
            (Some(_), None) => anyhow::bail!(
                "archive-current-password-required: enter the archive's current password; a \
                 restore keeps its encryption and password; nothing was changed"
            ),
            (envelope, Some(key)) => Encryption { key: Some(owned(key)), envelope },
        }))
    }

    /// The copy's own encryption, envelope included, for a folder with no archive to keep.
    fn of_copy(copy: &Path, key: Option<vault_crypto::DataKey>) -> Encryption {
        let envelope =
            key.is_some().then(|| vault_layout::path(copy, vault_layout::password_envelope()));
        Encryption { key, envelope }
    }

    fn mode(&self) -> ArchiveMode {
        if self.key.is_some() { ArchiveMode::Encrypted } else { ArchiveMode::Plaintext }
    }

    /// Whether files under `key` (`None`: unencrypted) are already under this encryption.
    /// Compares keys, not key files: a password change rewraps the same key.
    fn opens(&self, key: Option<&vault_crypto::DataKey>) -> bool {
        let subkey = |key: &vault_crypto::DataKey| {
            *key.derive(vault_crypto::KeyPurpose::ArchiveDatabase).expose_bytes()
        };
        match (&self.key, key) {
            (None, None) => true,
            (Some(ours), Some(theirs)) => subkey(ours) == subkey(theirs),
            _ => false,
        }
    }
}

/// A copy of a key the caller holds, for the restored archive to keep.
fn owned(key: &vault_crypto::DataKey) -> vault_crypto::DataKey {
    vault_crypto::DataKey::from_bytes(zeroize::Zeroizing::new(*key.expose_bytes()))
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

/// Rewrites the staged copy's databases from its key (`from`) to the restored archive's (`to`),
/// as a mode change does: copied table by table and verified, then put in place of the originals.
/// The stores the stage creates for itself (the empty agent-history ones, the scratch store) are
/// removed, to be created again under `to`.
fn convert_stage(
    stage: &Path,
    config: &crate::AppConfig,
    from: Option<&vault_crypto::DataKey>,
    to: Option<&vault_crypto::DataKey>,
    cancel: &CancelToken,
) -> Result<()> {
    let converted = stage.join("converted");
    let staged = Vault::open_staged(crate::project_paths_with_root(stage), config.clone(), from)?;
    let layout = staged.layout();
    let databases: Vec<_> = vault_layout::mode_change_databases(layout)
        .filter(|db| !agent_history_databases().contains(db))
        .collect();
    let pause = staged.store().pause()?;
    super::copy_databases(&pause, &converted, to, cancel, |_| {}, &databases)?;
    pause.close()?;
    staged.close()?;
    let recreated = agent_history_databases().into_iter().chain([vault_layout::scratch_database()]);
    for db in databases.iter().copied().chain(recreated) {
        let relative = vault_layout::database_path(db);
        remove_with_companions(&stage.join(relative))?;
        if databases.contains(&db) {
            let (from, to) = (converted.join(relative), stage.join(relative));
            crate::durable_io::retry_while_held(|| fs::rename(&from, &to))?;
        }
    }
    // Search indexes the stage may have made under the copy's key; they are built again.
    let kept = database_files(layout, false);
    for rebuilt in database_files(layout, true).into_iter().filter(|file| !kept.contains(file)) {
        install::remove(&stage.join(rebuilt))?;
    }
    crate::durable_io::retry_while_held(|| fs::remove_dir_all(&converted))?;
    Ok(())
}

fn remove_with_companions(main: &Path) -> Result<()> {
    for suffix in
        vault_layout::CompanionFamily::Native.suffixes(vault_layout::CompanionUse::Install)
    {
        let file = PathBuf::from(format!("{}{suffix}", main.display()));
        match crate::durable_io::retry_while_held(|| fs::remove_file(&file)) {
            Err(error) if error.kind() != std::io::ErrorKind::NotFound => return Err(error.into()),
            _ => {}
        }
    }
    Ok(())
}

/// Stages the copy's files, all but its agent conversations, which a restore does not bring back,
/// and its envelope: the restored archive keeps its own password.
fn copy_snapshot(source: &Path, target: &Path) -> Result<()> {
    let envelope =
        PathBuf::from(vault_layout::spec(vault_layout::password_envelope()).path.relative());
    for relative in vault_layout::installation_files_except(
        crate::admission::root_layout(source)?,
        false,
        &[vault_layout::DataScope::AgentHistory],
    ) {
        if relative != envelope && source.join(&relative).is_file() {
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
                crate::durable_io::retry_while_held(|| fs::remove_dir_all(entry.path()))?;
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
            // The whole copy, as Free up space shows and deletes it.
            size_bytes: super::retention::measure(&path).map_or(size, |(bytes, _)| bytes),
            verified_openable: size >= 4096,
            encrypted,
            source_op: source_op.into(),
            label: label.into(),
        });
    }
    Ok(result)
}
