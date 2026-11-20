//! Explicit recovery of a damaged control or a missing secondary 0.5 store.
//! The archive's browsing files, envelope and conversation evidence remain the sources of truth.
use crate::{AppConfig, ArchiveMode, ProjectPaths, Vault, admission, config};
use anyhow::{Context, Result, ensure};
use std::sync::Arc;
use vault_layout::{ArchiveLayout, ArtifactId, Db};
use vault_store::{OwnerLock, Store};

/// Replace only missing/invalid settings, after verifying the existing browsing stores and key.
pub fn rebuild_settings(
    paths: &ProjectPaths,
    owner: Option<Arc<OwnerLock>>,
    password: Option<&str>,
) -> Result<()> {
    admission::refuse_newer(&paths.app_root)?;
    super::rollback::require_resolved(&paths.app_root)?;
    let refusal = admission::inspect(&paths.app_root)
        .err()
        .context("archive-control-recovery-unavailable: settings are readable")?;
    ensure!(
        matches!(
            refusal.reason,
            admission::Reason::ControlMissing | admission::Reason::ControlInvalid
        ),
        "archive-control-recovery-unavailable: this control cannot be rebuilt"
    );
    ensure!(
        !super::install::pending(paths) && !crate::data_wipe_interrupted(paths),
        "archive-operation-interrupted: settle the pending operation first"
    );
    let _operation = crate::archive::ArchiveOpGate::acquire(paths);
    let _owner = match owner {
        Some(o) => o,
        None => Arc::new(OwnerLock::acquire(&paths.app_root)?),
    };
    let _control = crate::ControlGate::acquire(paths);
    let encrypted = paths.artifact_path(ArtifactId::Envelope).try_exists()?;
    let key = if encrypted {
        Some(Vault::unlock_data_key(
            paths,
            password
                .context("archive-current-password-required: enter the current archive password")?,
        )?)
    } else {
        None
    };
    let storage =
        crate::vault::storage_config(paths, ArchiveLayout::BrowsingV1, key.as_ref(), None)?;
    for role in [Db::Archive, Db::Intelligence, Db::Agent, Db::SourceEvidence] {
        Store::inspect_existing(&storage, role, |s| {
            let version = s.schema_version()?;
            let supported = crate::store_schema::schema_version(role);
            if version > supported {
                return Err(vault_store::Error::SchemaTooNew {
                    db: role,
                    found: version,
                    supported,
                });
            }
            Ok(())
        })?;
    }
    let mut settings = AppConfig {
        initialized: true,
        archive_mode: if encrypted { ArchiveMode::Encrypted } else { ArchiveMode::Plaintext },
        ..Default::default()
    };
    crate::hydrate_app_lock_config(paths, &mut settings)?;
    settings.app_lock.enabled = settings.app_lock.passcode_configured;
    let has_conversations = paths.artifact_path(ArtifactId::AgentHistoryEvidence).try_exists()?
        || paths.artifact_path(ArtifactId::AgentHistoryReadModel).try_exists()?;
    let stamp = has_conversations.then(admission::written_compatibility);
    let bytes = config::encode_config(&settings, stamp.as_ref())?;
    crate::durable_io::atomic_durable_write(&paths.config_path, bytes.as_bytes())
}

/// A missing v0.5 secondary can be replaced independently before ordinary promotion.
pub fn replace_missing_secondary(
    paths: &ProjectPaths,
    owner: Option<Arc<OwnerLock>>,
    role: Db,
    password: Option<&str>,
) -> Result<()> {
    admission::require_control(&paths.app_root)?;
    super::rollback::require_resolved(&paths.app_root)?;
    ensure!(
        matches!(role, Db::Agent | Db::Intelligence)
            && admission::root_layout(&paths.app_root)? == ArchiveLayout::BrowsingV1,
        "archive-store-recovery-unavailable: expected a missing 0.5 secondary store"
    );
    let settings = crate::load_config(paths)?;
    crate::ensure_app_lock_unlocked(paths, &settings)?;
    let _op = crate::archive::ArchiveOpGate::acquire(paths);
    let _owner = match owner {
        Some(o) => o,
        None => Arc::new(OwnerLock::acquire(&paths.app_root)?),
    };
    ensure!(
        !super::install::pending(paths) && !crate::data_wipe_interrupted(paths),
        "archive-operation-interrupted: settle pending maintenance first"
    );
    ensure!(
        paths.native_archive_path().is_file()
            && !paths.app_root.join(vault_layout::database_path(role)).try_exists()?,
        "archive-store-recovery-unavailable: selected file is present or browsing is missing"
    );
    let key = if settings.archive_mode == ArchiveMode::Encrypted {
        Some(Vault::unlock_data_key(
            paths,
            password
                .context("archive-current-password-required: enter the current archive password")?,
        )?)
    } else {
        None
    };
    let stage = super::staging_root(paths)?;
    let staged = crate::project_paths_with_root(stage.path());
    let storage =
        crate::vault::storage_config(&staged, ArchiveLayout::BrowsingV1, key.as_ref(), None)?;
    Store::open(storage)?.close()?;
    let rel = vault_layout::database_path(role);
    super::install::rename(&stage.path().join(rel), &paths.app_root.join(rel))
}

/// Identify a missing released secondary before the owner lock, logs and background services.
/// A 0.5 configuration may exist before the first backup has created any native database.
pub fn missing_released_store(paths: &ProjectPaths) -> Result<Option<std::path::PathBuf>> {
    if !matches!(admission::inspect(&paths.app_root)?,admission::RootState::Admitted(ref c) if c.compatibility.is_none())
        || !paths.native_archive_path().is_file()
        || super::install::pending(paths)
        || crate::data_wipe_interrupted(paths)
    {
        return Ok(None);
    }
    for db in [Db::SourceEvidence, Db::Agent, Db::Intelligence] {
        let file = paths.app_root.join(vault_layout::database_path(db));
        if !file.try_exists()? {
            return Ok(Some(file));
        }
    }
    Ok(None)
}
