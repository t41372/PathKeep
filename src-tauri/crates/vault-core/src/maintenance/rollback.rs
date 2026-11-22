//! Prepare the current browsing archive for the released v0.5.0 reader. Newer conversation
//! files and recovery copies are detached by rename. The sibling receipt owns rejoin consent.
//! Installer download and process restart belong to the desktop.
mod receipt;
use crate::{
    ArchiveMode, ControlGate, ProjectPaths, Vault, admission, config, durable_io,
    maintenance::install,
};
use anyhow::{Context, Result, ensure};
use receipt::{Choice, Receipt, State};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Component, Path, PathBuf},
    sync::Arc,
};
use vault_layout::{ArtifactId as A, Db};
use vault_store::{CancelToken, OwnerLock};

const ROLES: [Db; 2] = [Db::AgentHistoryEvidence, Db::AgentHistoryReadModel];

/// A background startup with an outstanding handoff must wait for a foreground choice.
pub fn pending(root: &Path) -> Result<Option<PathBuf>> {
    Ok(receipt::pending(root)?.map(|(dir, _)| dir))
}

/// A live open or replacement may proceed only once the user resolves the previous-version handoff.
pub fn require_resolved(root: &Path) -> Result<()> {
    ensure!(
        pending(root)?.is_none(),
        "archive-handoff-choice-required: this archive was prepared for a previous version"
    );
    Ok(())
}

/// Detached safety copies remain selectable after rejoin. Their original package path stays
/// visible even if the current browsing archive has been wiped and initialized again.
pub fn detached_snapshots(paths: &ProjectPaths) -> Result<Vec<crate::RecoverySnapshot>> {
    admission::refuse_newer(&paths.app_root)?;
    let mut copies = Vec::new();
    for (dir, receipt) in receipt::receipts(&paths.app_root)? {
        if matches!(receipt.state, State::Resolved | State::HandedOff) {
            copies.extend(super::list_snapshots(&crate::project_paths_with_root(
                &dir.join("copies"),
            ))?);
        }
    }
    Ok(copies)
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Intent {
    copies: Vec<PathBuf>,
    source_control_hash: String,
}

fn copies(paths: &ProjectPaths) -> Result<Vec<PathBuf>> {
    let mut found = Vec::new();
    for family in [
        vault_layout::NATIVE_COPIES_V06,
        vault_layout::PREVIOUS_GENERATIONS_V06,
        vault_layout::MOVE_FILES_V3,
    ] {
        if paths.app_root.join(family).try_exists()? {
            found.push(PathBuf::from(family));
        }
    }
    for copy in super::list_snapshots(paths)? {
        let path = PathBuf::from(copy.path);
        let relative = path.strip_prefix(&paths.app_root)?;
        if found.iter().any(|p| relative.starts_with(p)) {
            continue;
        }
        if copy.source_op == "heldGeneration"
            || copy.source_op == "import"
            || copy.source_op == "restore"
            || crate::admission::guard(&path)?.is_some_and(|c| c.compatibility.is_some())
        {
            found.push(path.strip_prefix(&paths.app_root)?.into());
        }
    }
    for (file, version) in crate::migration::root_move_files(&paths.app_root)? {
        if version == 3 {
            found.push(file.strip_prefix(&paths.app_root)?.into());
        }
    }
    if paths.exports_dir.exists() {
        for entry in walkdir::WalkDir::new(&paths.exports_dir).follow_root_links(false) {
            let entry = entry?;
            if entry.path().starts_with(paths.app_root.join(vault_layout::MOVE_FILES_V3)) {
                continue;
            }
            if entry.file_type().is_file()
                && crate::migration::read_and_validate_manifest(entry.path())
                    .is_ok_and(|m| m.format_version == 3)
            {
                found.push(entry.path().strip_prefix(&paths.app_root)?.into());
            }
        }
    }
    Ok(found)
}

fn require_source(paths: &ProjectPaths) -> Result<()> {
    let stamp = admission::guard(&paths.app_root)?.and_then(|c| c.compatibility);
    ensure!(
        stamp.is_some_and(|c| c.stamp().revision == 1),
        "archive-rollback-unsupported: expected a revision 1 archive"
    );
    ensure!(
        !install::pending(paths) && !crate::data_wipe_interrupted(paths),
        "archive-recovery-required: finish pending maintenance first"
    );
    crate::storage_volume::require_snapshot_volume(paths)?;
    let root = fs::canonicalize(&paths.app_root)?;
    crate::storage_volume::require_handoff_volume(&root, &receipt::namespace(&root)?)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Preview {
    target_version: &'static str,
    package_folder: PathBuf,
    encrypted: bool,
    conversation_bytes: u64,
    copy_folders_and_files: Vec<PathBuf>,
    rejoin_copy_bytes: u64,
    needs_choice: bool,
}

/// No database is opened merely to show a package's location or space cost.
pub fn preview(paths: &ProjectPaths) -> Result<Preview> {
    admission::refuse_newer(&paths.app_root)?;
    let held = pending(&paths.app_root)?;
    let (folder, root, count) = if let Some(folder) = &held {
        (
            folder.clone(),
            folder.join("agent-history"),
            serde_json::from_slice::<Intent>(&fs::read(folder.join("reverse-intent.json"))?)?
                .copies,
        )
    } else {
        require_source(paths)?;
        (
            receipt::namespace(&fs::canonicalize(&paths.app_root)?)?,
            paths.app_root.clone(),
            copies(paths)?,
        )
    };
    let mut bytes = 0;
    for db in ROLES {
        let path = root.join(vault_layout::database_path(db));
        if path.try_exists()? {
            bytes += fs::metadata(path)?.len();
        }
    }
    Ok(Preview {
        target_version: "0.5.0",
        encrypted: if held.is_some() {
            root.join("agent-history-key.envelope").is_file()
        } else {
            paths.artifact_path(A::Envelope).is_file()
        },
        package_folder: folder,
        conversation_bytes: bytes,
        copy_folders_and_files: count,
        rejoin_copy_bytes: bytes,
        needs_choice: held.is_some(),
    })
}

/// Publish old-format control last. A prepared receipt can replay remaining moves after restart.
pub fn detach(
    paths: &ProjectPaths,
    current: Option<&Vault>,
    reserved: Option<Arc<OwnerLock>>,
    password: Option<&str>,
) -> Result<PathBuf> {
    admission::refuse_newer(&paths.app_root)?;
    let _op = crate::archive::ArchiveOpGate::acquire(paths);
    let _owner = owner(paths, current, reserved)?;
    if install::pending(paths) {
        super::recover_native_maintenance(paths)?;
    }
    let _gate = ControlGate::acquire(paths);
    if let Some((dir, mut receipt)) = receipt::pending(&paths.app_root)? {
        if receipt.state != State::HandedOff {
            finish_detach(paths, &dir, &mut receipt)?;
        }
        return Ok(dir);
    }
    require_source(paths)?;
    let settings = crate::load_config(paths)?;
    crate::ensure_app_lock_unlocked(paths, &settings)?;
    let encrypted = settings.archive_mode == ArchiveMode::Encrypted;
    let unlocked = if encrypted && current.is_none() {
        Some(Vault::unlock_data_key(
            paths,
            password
                .context("archive-current-password-required: enter the current archive password")?,
        )?)
    } else {
        None
    };
    let _backup = current.map(Vault::begin_backup).transpose()?;
    let _pause = current.map(Vault::pause_conversations);
    if let Some(live) = current {
        live.require_complete()?;
        live.verify_conversion_source(&CancelToken::new())?;
        live.pause_store()?.close()?;
    } else {
        // The owner can start this route without starting the desktop queue or search.
        let mut storage = crate::vault::storage_config(
            paths,
            admission::WRITTEN_LAYOUT,
            unlocked.as_ref(),
            Some(_owner.clone()),
        )?;
        storage.required_existing =
            storage.databases.iter().copied().filter(|db| *db != Db::Scratch).collect();
        let store = vault_store::Store::open(storage)?;
        store.close()?;
    }
    let (dir, mut receipt) = Receipt::create(&paths.app_root)?;
    let standalone = dir.join("agent-history");
    fs::create_dir(&standalone)?;
    agent_history::Archive::write_detached_control(
        &standalone,
        encrypted.then(|| paths.artifact_path(A::Envelope)).as_deref(),
    )?;
    let original = fs::read(&paths.config_path)?;
    durable_io::atomic_durable_write(&dir.join("settings.json"), &original)?;
    let mut older = settings;
    older.capture_policy = Default::default();
    older.agent_viewer = Default::default();
    older.dismissed_agent_hints.clear();
    durable_io::atomic_durable_write(
        &dir.join("target-config.json"),
        config::encode_config(&older, None)?.as_bytes(),
    )?;
    let intent =
        Intent { copies: copies(paths)?, source_control_hash: crate::utils::sha256_hex(&original) };
    durable_io::atomic_durable_write(
        &dir.join("reverse-intent.json"),
        &serde_json::to_vec(&intent)?,
    )?;
    receipt.save(&dir)?;
    finish_detach(paths, &dir, &mut receipt)?;
    Ok(dir)
}

fn owner(
    paths: &ProjectPaths,
    current: Option<&Vault>,
    reserved: Option<Arc<OwnerLock>>,
) -> Result<Arc<OwnerLock>> {
    if let Some(v) = current {
        return Ok(v.store().owner()?);
    }
    match reserved {
        Some(o) => Ok(o),
        None => Ok(Arc::new(OwnerLock::acquire(&paths.app_root)?)),
    }
}

fn finish_detach(paths: &ProjectPaths, dir: &Path, receipt: &mut Receipt) -> Result<()> {
    let target = fs::read(dir.join("target-config.json"))?;
    let live = fs::read(&paths.config_path)?;
    let intent: Intent = serde_json::from_slice(&fs::read(dir.join("reverse-intent.json"))?)?;
    ensure!(
        config::decode_config(&paths.app_root, &target)?.compatibility.is_none(),
        "invalid rollback control"
    );
    if receipt.state == State::Ready && live == target {
        receipt.state = State::HandedOff;
        return receipt.save(dir);
    }
    ensure!(
        crate::utils::sha256_hex(&live) == intent.source_control_hash,
        "archive-handoff-invalid: live control differs from the prepared archive"
    );
    let move_member = |relative: &Path, group: &str| -> Result<()> {
        let from = paths.app_root.join(relative);
        let to = dir.join(group).join(relative);
        match (from.try_exists()?, to.try_exists()?) {
            (true, false) => install::rename(&from, &to),
            (false, true) => Ok(()),
            _ => anyhow::bail!("archive-handoff-invalid: cannot identify {}", relative.display()),
        }
    };
    for db in ROLES {
        move_member(Path::new(vault_layout::database_path(db)), "agent-history")?;
    }
    let search = Path::new(vault_layout::spec(A::AgentHistorySearch).path.relative());
    if paths.app_root.join(search).exists() {
        move_member(search, "agent-history")?;
    }
    for rel in intent.copies {
        let old_root_export = rel.components().count() == 1
            && [&paths.app_root, &dir.join("copies")].iter().any(|root| {
                crate::migration::read_and_validate_manifest(&root.join(&rel))
                    .is_ok_and(|m| m.format_version == 3)
            });
        ensure!(
            rel.components().all(|p| matches!(p, Component::Normal(_)))
                && (rel.starts_with(vault_layout::spec(A::NativeSnapshots).path.relative())
                    || rel.starts_with(vault_layout::PREVIOUS_GENERATIONS_V06)
                    || rel.starts_with("exports")
                    || old_root_export),
            "archive-handoff-invalid: invalid saved-copy path"
        );
        move_member(&rel, "copies")?;
    }
    receipt.state = State::Ready;
    receipt.save(dir)?;
    durable_io::atomic_durable_write(&paths.config_path, &target)?;
    receipt.state = State::HandedOff;
    receipt.save(dir)
}

/// Keep browsing as the old app last saved it. The detached password and the current archive
/// password can differ. Only the two conversation roles are copied and verified.
pub fn resolve(
    paths: &ProjectPaths,
    reserved: Option<Arc<OwnerLock>>,
    rejoin: bool,
    current_password: Option<&str>,
    package_password: Option<&str>,
) -> Result<()> {
    admission::refuse_newer(&paths.app_root)?;
    let _op = crate::archive::ArchiveOpGate::acquire(paths);
    let _owner = owner(paths, None, reserved)?;
    let (dir, mut receipt) =
        receipt::pending(&paths.app_root)?.context("no pending archive handoff")?;
    ensure!(
        receipt.state == State::HandedOff,
        "archive-handoff-incomplete: finish preparing the previous version first"
    );
    let control = ControlGate::acquire(paths);
    let mut settings = crate::load_config(paths)?;
    crate::ensure_app_lock_unlocked(paths, &settings)?;
    let empty = matches!(admission::inspect(&paths.app_root)?, admission::RootState::Uninitialized)
        || !settings.initialized && !paths.native_archive_path().try_exists()?;
    if empty && !rejoin {
        receipt.choice = Choice::Fresh;
        receipt.state = State::Resolved;
        return receipt.save(&dir);
    }
    if empty {
        settings.initialized = true;
        settings.archive_mode = if dir.join("agent-history/agent-history-key.envelope").is_file() {
            ArchiveMode::Encrypted
        } else {
            ArchiveMode::Plaintext
        };
    }
    let stage = super::staging_root(paths)?;
    let current_key = if empty {
        super::create_envelope(stage.path(), &settings, current_password.or(package_password))?
    } else if settings.archive_mode == ArchiveMode::Encrypted {
        Some(Vault::unlock_data_key(
            paths,
            current_password.context(
                "archive-current-password-required: enter the current browsing password",
            )?,
        )?)
    } else {
        None
    };
    for db in ROLES {
        ensure!(
            !paths.app_root.join(vault_layout::database_path(db)).try_exists()?,
            "archive-handoff-destination-occupied: both conversation archives were kept"
        );
    }

    if rejoin {
        let source = dir.join("agent-history");
        agent_history::Archive::copy_detached_pair(
            &source,
            package_password.unwrap_or_default().as_bytes(),
            stage.path(),
            current_key.as_ref(),
        )?;
        let previous = config::decode_config(&dir, &fs::read(dir.join("settings.json"))?)?.config;
        settings.capture_policy = previous.capture_policy;
        settings.dismissed_agent_hints = previous.dismissed_agent_hints;
        settings.agent_viewer = previous.agent_viewer;
    }
    let staged = crate::project_paths_with_root(stage.path());
    config::write_staged_control(&staged, &settings, Some(&admission::written_compatibility()))?;
    let trial = Vault::open_staged(staged, settings, current_key.as_ref())?;
    trial.verify_conversion_source(&CancelToken::new())?;
    trial.close()?;
    let mut files =
        ROLES.iter().map(|db| PathBuf::from(vault_layout::database_path(*db))).collect::<Vec<_>>();
    if empty {
        files = super::database_files(admission::WRITTEN_LAYOUT, true);
    } else {
        files.push("config.json".into());
    }
    receipt.choice = if rejoin { Choice::Rejoin } else { Choice::Fresh };
    receipt.save(&dir)?;
    install::install(&control, stage.path(), files, install::Keep::Nothing)?;
    receipt.state = State::Resolved;
    receipt.save(&dir)?;
    if rejoin {
        vault_fs::remove_dir_all(dir.join("agent-history"))?;
    }
    Ok(())
}
