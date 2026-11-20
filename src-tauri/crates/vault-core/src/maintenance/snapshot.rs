//! Responsibilities: checkpoint snapshots and verified full-database restoration.
//! Not responsible for: browser checkpoint replay, passwords or retention decisions.
//! Failure modes: inconsistent multi-file copy, missing envelope, corrupted snapshot,
//! restoring only archive, incomplete inventory silently creates empty databases,
//! replacing originals before verification. Tests: maintenance::tests.
use super::{InstalledArchive, database_files, install, staging_root};
use crate::{ArchiveMode, ProjectPaths, Vault};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
};
use vault_store::{ArchiveFileRole, CancelToken, Db};

/// The file that seals a snapshot: what it holds and each file's digest.
pub(super) const SEAL: &str = "snapshot.json";

/// A specific copy member rejected by verification, including the path the user selected.
#[derive(Debug, thiserror::Error)]
#[error("{code}: {path}")]
pub struct SnapshotMemberFailure {
    pub code: &'static str,
    pub path: PathBuf,
    pub role: vault_store::ArchiveFileRole,
}

impl SnapshotMemberFailure {
    pub(super) fn new(code: &'static str, root: &Path, member: &Path) -> Self {
        Self {
            code,
            path: root.join(member),
            role: vault_store::ArchiveFileRole::for_member(member),
        }
    }
}

#[derive(Serialize, Deserialize, schemars::JsonSchema)]
pub(crate) struct FileProof {
    pub(super) path: PathBuf,
    pub(super) sha256: String,
}

/// A safety snapshot lands at a committed batch boundary across all databases.
pub fn snapshot(vault: &Vault, target: &Path) -> Result<()> {
    let _operation = crate::archive::ArchiveOpGate::acquire(vault.paths());
    vault.recheck_root()?;
    let mut pause = vault.pause_store()?;
    if vault.conversation_availability().is_ok() {
        return snapshot_paused(vault, &mut pause, target);
    }
    // Finish all reads before creating anything at the requested destination.
    let ancestor = target.ancestors().skip(1).find(|p| p.is_dir()).unwrap_or(Path::new("."));
    let trial = tempfile::tempdir_in(ancestor)?;
    let copy = trial.path().join("snapshot");
    snapshot_paused(vault, &mut pause, &copy)?;
    install::sync_tree(&copy)?;
    if target.try_exists()? {
        anyhow::ensure!(
            fs::symlink_metadata(target)?.is_dir() && fs::read_dir(target)?.next().is_none(),
            "snapshot target must be empty"
        );
        for entry in fs::read_dir(&copy)? {
            let entry = entry?;
            install::rename(&entry.path(), &target.join(entry.file_name()))?;
        }
        Ok(())
    } else {
        install::rename(&copy, target)
    }
}

pub(super) fn snapshot_paused(
    vault: &Vault,
    pause: &mut vault_store::Maintenance<'_>,
    target: &Path,
) -> Result<()> {
    pause.snapshot(target)?;
    finish_snapshot(vault, target)
}

pub(super) fn finish_snapshot(vault: &Vault, target: &Path) -> Result<()> {
    finish_snapshot_at(vault.paths(), target)
}

/// A held-only copy has its own control, envelope, file proofs and listed directory.
/// It can supply verified evidence for repair even when browsing is busy or unavailable.
pub(crate) fn publish_held(
    paths: &ProjectPaths,
    held: vault_store::HeldGeneration<'_>,
) -> Result<()> {
    let stage = super::conversation_repair::cache::stage(paths)?;
    let copy = stage.path().join("snapshot");
    held.copy_to(&copy.join(vault_layout::HELD_CONVERSATIONS))?;
    fs::write(copy.join(vault_layout::UNREADABLE_CONVERSATIONS), b"1\n")?;
    let name = format!(
        "held-generation-{}-{}",
        held.generation,
        stage.path().file_name().unwrap().to_string_lossy()
    );
    fs::create_dir_all(copy.join("archive"))?;
    finish_snapshot_at(paths, &copy)?;
    install::sync_tree(&copy)?;
    install::rename(
        &copy,
        &paths.artifact_path(vault_layout::ArtifactId::NativeSnapshots).join(name),
    )
}

fn finish_snapshot_at(paths: &ProjectPaths, target: &Path) -> Result<()> {
    let mut config = crate::load_config(paths)?;
    config.capture_policy = Default::default();
    config.onboarding_choices = None;
    config.agent_viewer = Default::default();
    config.dismissed_agent_hints.clear();
    crate::config::save_copy_config(paths, target, &config)?;
    let envelope = vault_layout::path(&paths.app_root, vault_layout::ArtifactId::Envelope);
    if config.archive_mode == ArchiveMode::Encrypted || envelope.try_exists()? {
        vault_store::copy_archive_file(
            ArchiveFileRole::Browsing,
            &envelope,
            &vault_layout::path(target, vault_layout::ArtifactId::Envelope),
        )?;
    }
    seal(target)
}

/// An unlinked original has no directory entry to restore on rollback. Publish
/// its completed safety copy before releasing the last handle. A later refusal
/// or crash must never sweep its only remaining bytes out with the install stage.
pub(super) fn keep_held_before_close(displaced: &Path, listed: &Path) -> Result<bool> {
    if !displaced.join(vault_layout::HELD_CONVERSATIONS).try_exists()? {
        return Ok(false);
    }
    install::sync_tree(displaced)?;
    install::rename(displaced, listed)?;
    Ok(true)
}

pub(crate) fn seal(root: &Path) -> Result<()> {
    // The snapshot's own control, written just before, says which stores it holds.
    let layout = crate::admission::root_layout(root)?;
    let held = root.join(vault_layout::HELD_CONVERSATIONS);
    let mut pinned = Vec::new();
    if held.try_exists()? {
        anyhow::ensure!(
            fs::symlink_metadata(&held)?.is_dir(),
            "invalid held-conversations directory"
        );
        for entry in walkdir::WalkDir::new(&held).follow_links(false).follow_root_links(false) {
            let entry = entry?;
            if entry.file_type().is_file() {
                pinned.push(entry.path().strip_prefix(root)?.to_owned());
            }
        }
    }
    let files = database_files(layout, false)
        .into_iter()
        .chain(pinned)
        .chain([PathBuf::from(vault_layout::UNREADABLE_CONVERSATIONS)])
        .filter(|p| root.join(p).is_file())
        .map(|path| {
            sync_member(ArchiveFileRole::for_member(&path), &root.join(&path))?;
            Ok(FileProof {
                sha256: hash_file(
                    &root.join(&path),
                    vault_store::ArchiveFileRole::for_member(&path),
                )?,
                path,
            })
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
    crate::durable_io::atomic_durable_write(&root.join(SEAL), &serde_json::to_vec(&files)?).map_err(
        |e| {
            vault_store::Error::writing(
                ArchiveFileRole::Browsing,
                &root.join(SEAL),
                std::io::Error::other(e),
            )
            .into()
        },
    )
}

/// Makes the archive an install replaced (`install::Keep::SafetyCopy`) a copy a restore can use.
/// Its search index goes, as a snapshot has none: a restore builds its own. The rest is sealed as
/// it is.
///
/// The install seals it once the new archive is in place. When that fails (a file another program
/// held, a read that failed once), the restore that needs the copy seals it then
/// ([`unsealed_replaced_archive`]): its files are the archive as it was replaced, moved whole by
/// one rename, and nothing writes to them since.
pub(super) fn seal_replaced(copy: &Path) -> Result<()> {
    let before =
        copy.file_name().and_then(|name| install::Before::of_copy(&name.to_string_lossy()));
    crate::fault_inject::checkpoint(&format!(
        "native-seal-before-{}",
        before.map_or("maintenance", install::Before::op)
    ))?;
    let layout = crate::admission::root_layout(copy)?;
    let paths = crate::project_paths_with_root(copy);
    // Strip even a foreign installation's grant from the displaced control.
    let mut document = crate::config::decode_document(&fs::read(&paths.config_path)?)?;
    if document.config.capture_policy != Default::default()
        || document.config.onboarding_choices.is_some()
        || document.config.agent_viewer != Default::default()
        || !document.config.dismissed_agent_hints.is_empty()
    {
        document.config.onboarding_choices = None;
        document.config.agent_viewer = Default::default();
        document.config.dismissed_agent_hints.clear();
        document.config.capture_policy = Default::default();
        let bytes =
            crate::config::encode_config(&document.config, document.compatibility.as_ref())?;
        crate::durable_io::atomic_durable_write(&paths.config_path, bytes.as_bytes())?;
    }
    let sealed = database_files(layout, false);
    for rebuilt in database_files(layout, true).into_iter().filter(|file| !sealed.contains(file)) {
        install::remove(&copy.join(rebuilt))?;
    }
    seal(copy)
}

/// Whether `source` is an archive a restore or an import replaced that has no seal yet, in this
/// archive's own folder of copies. Only such a copy is sealed by a restore: a safety snapshot
/// without a seal was cut short while it was written and may lack files, and a folder elsewhere is
/// not PathKeep's to write in.
fn unsealed_replaced_archive(paths: &ProjectPaths, source: &Path) -> bool {
    let copies = paths.artifact_path(vault_layout::ArtifactId::NativeSnapshots);
    let here = |path: &Path| fs::canonicalize(path).ok();
    source.is_dir()
        && !source.join(SEAL).exists()
        && source
            .file_name()
            .is_some_and(|name| install::Before::of_copy(&name.to_string_lossy()).is_some())
        && here(copies.as_path()).is_some()
        && source.parent().and_then(here) == here(copies.as_path())
}

pub(crate) fn verify(root: &Path) -> Result<()> {
    let seal = root.join(SEAL);
    let proof: Vec<FileProof> = serde_json::from_slice(&fs::read(&seal).map_err(|e| {
        vault_store::Error::preserve(vault_store::ArchiveFileRole::Browsing, &seal, e)
    })?)?;
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
    let browsing_files = vault_layout::installation_files_except(
        layout,
        false,
        &[vault_layout::DataScope::AgentHistory],
    );
    for file in proof.into_iter().filter(|p| browsing_files.contains(&p.path)) {
        if hash_file(&root.join(&file.path), vault_store::ArchiveFileRole::for_member(&file.path))?
            != file.sha256
        {
            return Err(
                SnapshotMemberFailure::new("snapshot-digest-mismatch", root, &file.path).into()
            );
        }
    }
    Ok(())
}

pub(crate) fn hash_file(path: &Path, role: vault_store::ArchiveFileRole) -> Result<String> {
    let read_error = |e| vault_store::Error::preserve(role, path, e);
    let mut file = fs::File::open(path).map_err(read_error)?;
    let mut hash = Sha256::new();
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let n = file.read(&mut buffer).map_err(read_error)?;
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
    if let Some(vault) = current {
        vault.check_browsing_files()?;
    }
    // A delete-all-data the user confirmed has not finished. Installing an archive now would put
    // it where that wipe, once finished, deletes it.
    anyhow::ensure!(
        !crate::data_wipe_interrupted(paths),
        "archive-wipe-pending: a delete-all-data has not finished; it finishes before anything is \
         restored or imported"
    );
    if unsealed_replaced_archive(paths, source) {
        seal_replaced(source).context("this safety copy could not be checked")?;
    }
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
    let conversations_here =
        crate::admission::guard(&paths.app_root).ok().flatten().is_some_and(|control| {
            crate::admission::layout_of(Some(&control))
                == vault_layout::ArchiveLayout::AgentHistoryV2
        }) || agent_history_databases()
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
        copy_browsing_file(envelope, &staged_envelope)?;
    }
    let staged = Vault::open_staged(staged_paths.clone(), config.clone(), target.key.as_ref())?;
    staged.renew_browsing_run_origin()?;
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
    let mut files = if conversations_here {
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
    let original_control = crate::content_identity::of(&paths.config_path)?;
    crate::migration::native::keep_machine_settings(
        &mut config,
        &crate::migration::native::settings_here(paths)?,
    );
    crate::config::write_staged_control(&staged_paths, &config, Some(&stamp))?;
    let mut opaque_copy = None;
    let mut expected = None;
    if let Some(vault) = current {
        vault.check_available_files()?;
        let mut pause = vault.pause_store()?;
        if vault.conversation_availability().is_err() {
            let relative = Path::new(
                vault_layout::spec(vault_layout::ArtifactId::NativeSnapshots).path.relative(),
            )
            .join(format!(
                "before-restore-{}",
                stage.path().file_name().context("restore stage has no name")?.to_string_lossy()
            ));
            anyhow::ensure!(
                !install::exists(&paths.app_root.join(&relative))?,
                "safety copy destination already exists"
            );
            let displaced = stage.path().join(&relative);
            let mut frozen =
                pause.snapshot_with(&displaced, || install::originals(&paths.app_root, &files))?;
            finish_snapshot(vault, &displaced)?;
            frozen.insert(PathBuf::from("config.json"), original_control);
            for db in vault_layout::snapshot_databases(layout) {
                let path = PathBuf::from(vault_layout::database_path(db));
                if frozen.contains_key(&path) {
                    anyhow::ensure!(
                        frozen[&path] == crate::content_identity::member(&displaced, &path)?,
                        "installation-source-changed: {} changed while the safety copy was made",
                        path.display()
                    );
                }
            }
            let listed = paths.app_root.join(&relative);
            if !keep_held_before_close(&displaced, &listed)? {
                frozen.insert(relative.clone(), None);
                files.push(relative);
            }
            expected = Some(frozen);
            opaque_copy = Some(listed);
        }
        pause.close()?;
    }
    let keep = if opaque_copy.is_some() {
        install::Keep::Nothing
    } else {
        install::Keep::SafetyCopy(install::Before::Restore)
    };
    let replaced = install::install_bound(&control, stage.path(), files, keep, expected.as_ref())?;
    drop(control);
    let opened = Vault::open_owned(paths.clone(), config, target.key.as_ref(), Some(owner))?;
    Ok(InstalledArchive {
        vault: opened,
        data_key: target.key,
        previous_files: opaque_copy.unwrap_or(replaced.kept),
        previous_unsealed: replaced.unsealed,
    })
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
    let pause = staged.pause_store()?;
    super::copy_databases(&pause, &converted, to, cancel, |_| {}, &databases)?;
    pause.close()?;
    let recreated = agent_history_databases().into_iter().chain([vault_layout::scratch_database()]);
    for db in databases.iter().copied().chain(recreated) {
        let relative = vault_layout::database_path(db);
        remove_with_companions(&stage.join(relative))?;
        if databases.contains(&db) {
            let (from, to) = (converted.join(relative), stage.join(relative));
            vault_fs::rename(&from, &to)?;
        }
    }
    // Search indexes the stage may have made under the copy's key; they are built again.
    let kept = database_files(layout, false);
    for rebuilt in database_files(layout, true).into_iter().filter(|file| !kept.contains(file)) {
        install::remove(&stage.join(rebuilt))?;
    }
    vault_fs::remove_dir_all(&converted)?;
    Ok(())
}

fn remove_with_companions(main: &Path) -> Result<()> {
    for suffix in
        vault_layout::CompanionFamily::Native.suffixes(vault_layout::CompanionUse::Install)
    {
        let file = PathBuf::from(format!("{}{suffix}", main.display()));
        match vault_fs::remove_file(&file) {
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
            copy_browsing_file(&source.join(&relative), &target.join(relative))?;
        }
    }
    Ok(())
}

fn copy_browsing_file(source: &Path, target: &Path) -> Result<()> {
    vault_store::copy_archive_file(ArchiveFileRole::Browsing, source, target)?;
    sync_member(ArchiveFileRole::Browsing, target)
}

fn sync_member(role: ArchiveFileRole, path: &Path) -> Result<()> {
    crate::durable_io::fsync_file_durably(path)
        .map_err(|e| vault_store::Error::writing(role, path, std::io::Error::other(e)).into())
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
                vault_fs::remove_dir_all(entry.path())?;
            }
        }
    }
    Ok(recovered)
}

pub(super) fn is_held_copy(path: &Path) -> bool {
    path.file_name().is_some_and(|name| name.to_string_lossy().starts_with("held-generation-"))
}

/// The canonical held prefix or one of its numbered predecessors, exactly as repair reads them.
pub(super) fn held_evidence_prefix(member: &Path) -> Option<(Option<u64>, &Path)> {
    let main = Path::new(vault_layout::database_path(Db::AgentHistoryEvidence));
    if !member.ends_with(main) {
        return None;
    }
    let prefix = member.ancestors().nth(main.components().count())?;
    let held = Path::new(vault_layout::HELD_CONVERSATIONS);
    if prefix == held {
        return Some((None, prefix));
    }
    if prefix.parent() != Some(held) {
        return None;
    }
    let generation = prefix.file_name()?.to_str()?.strip_prefix("older-")?.parse().ok()?;
    Some((Some(generation), prefix))
}

fn copy_inventory(
    root: &Path,
    sealed: bool,
    marked: bool,
    fallback_bytes: u64,
) -> (u64, crate::ConversationRepairSource) {
    use crate::ConversationRepairSource::*;
    let mut evidence = false;
    let mut model = false;
    let size = super::retention::measure_files(root, &mut |file| {
        let member = file.strip_prefix(root).expect("inventory member beneath copy");
        evidence |= member == Path::new(vault_layout::database_path(Db::AgentHistoryEvidence))
            || marked && held_evidence_prefix(member).is_some();
        model |= member == Path::new(vault_layout::database_path(Db::AgentHistoryReadModel));
    });
    let source = if !sealed {
        Unsealed
    } else if size.is_err() {
        // An incomplete directory listing cannot prove a required member is missing.
        CheckRequired
    } else if !evidence {
        MissingEvidence
    } else if !marked && !model {
        MissingReadModel
    } else {
        CheckRequired
    };
    (size.map_or(fallback_bytes, |(bytes, _)| bytes), source)
}

/// Directory snapshots keep all databases together; discovery stays keyless. One that cannot be
/// read is listed as not openable rather than hiding the others. A folder without a seal is a
/// safety snapshot cut short, freed with the other snapshots, unless it is the archive a restore or
/// an import replaced: that is a copy from the moment it has its name, sealed or not, so it is never
/// freed unpicked. Without a seal it is offered for a restore, which seals it first, when its
/// control can be read; one that cannot (a restore replaced a damaged archive) cannot be sealed or
/// restored from.
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
        let before = install::Before::of_copy(&entry.file_name().to_string_lossy());
        let held_copy = is_held_copy(&path);
        let sealed = path.join(SEAL).is_file();
        let replaced_archive = (before.is_some() || held_copy) && path.is_dir();
        if !sealed && !replaced_archive {
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

        let encrypted = if held_copy {
            path.join(vault_layout::spec(vault_layout::ArtifactId::Envelope).path.relative())
                .is_file()
        } else {
            encrypted
        };
        let source_op = if held_copy {
            "heldGeneration"
        } else if entry.file_name().to_string_lossy().starts_with("before-agent-repair-") {
            "agentRepair"
        } else if entry.file_name().to_string_lossy().starts_with("manual-") {
            "manual"
        } else {
            before.map_or("maintenance", install::Before::op)
        };
        let restorable = sealed || crate::admission::root_layout(&path).is_ok();
        let name = path.display().to_string();
        let conversations_unreadable =
            path.join(vault_layout::UNREADABLE_CONVERSATIONS).try_exists()?;
        let (size_bytes, conversation_repair_source) =
            copy_inventory(&path, sealed, conversations_unreadable, size);
        result.push(crate::RecoverySnapshot {
            id: name.clone(),
            path: name,
            created_at: created,
            // The whole copy, as Free up space shows and deletes it.
            size_bytes,
            verified_openable: !held_copy && restorable && size >= 4096,
            unchecked: !sealed,
            conversations_unreadable,
            conversation_repair_source,
            encrypted,
            source_op: source_op.into(),
            label: label.into(),
        });
    }
    Ok(result)
}

/// Publish a user-requested copy only after its files and seal are durable.
pub fn manual_snapshot(
    vault: &Vault,
    expected_unreadable: bool,
) -> Result<crate::RecoverySnapshot> {
    let _op = crate::archive::ArchiveOpGate::acquire(vault.paths());
    vault.recheck_root()?;
    let stage = staging_root(vault.paths())?;
    let target =
        vault.paths().artifact_path(vault_layout::ArtifactId::NativeSnapshots).join(format!(
            "manual-{}",
            stage.path().file_name().context("copy stage has no name")?.to_string_lossy()
        ));
    let copy = stage.path().join("snapshot");
    let mut pause = vault.pause_store()?;
    anyhow::ensure!(
        expected_unreadable == vault.conversation_availability().is_err(),
        "conversation-repair-preview-stale"
    );
    snapshot_paused(vault, &mut pause, &copy)?;
    install::sync_tree(&copy)?;
    install::rename(&copy, &target)?;
    drop(pause);
    list_snapshots(vault.paths())?
        .into_iter()
        .find(|c| Path::new(&c.path) == target)
        .context("could not list the new safety copy")
}
