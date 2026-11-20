//! Publish an independently openable copy of the conversations selected for import undo.
//! The caller holds the archive operation and producer guards and marks Trash after publication.
use crate::{Vault, maintenance};
use anyhow::{Context, Result};
use std::{fs, path::PathBuf};
use vault_layout::{ArtifactId, Db};

pub(super) fn publish(vault: &Vault, id: &str) -> Result<PathBuf> {
    crate::storage_volume::require_snapshot_volume(vault.paths())?;
    let stage = maintenance::staging_root(vault.paths())?;
    let work = crate::project_paths_with_root(&stage.path().join("work"));
    let config = vault.config().clone();
    crate::config::write_staged_control(
        &work,
        &config,
        Some(&crate::admission::written_compatibility()),
    )?;
    let key = vault.data_key();
    let empty = Vault::open_staged(work.clone(), config.clone(), key)?;
    empty.close()?;
    let role = Db::AgentHistoryEvidence;
    let derived = key.map(|k| *k.derive(vault_layout::database_spec(role).key).expose_bytes());
    let selected = id.to_owned();
    vault.store().pause()?.with_staged_input(
        role,
        work.app_root.join(vault_layout::database_path(role)),
        derived,
        move |s| agent_history::archive::move_import::copy_eligible(s, &selected),
    )?;
    let verified = Vault::open_staged(work.clone(), config, key)?;
    verified.verify_conversion_source(&Default::default())?;
    verified.agent_history()?.rebuild_from_retained_evidence(&Default::default())?;
    verified.close()?;
    let package = stage.path().join("copy");
    fs::create_dir(&package)?;
    for db in [role, Db::AgentHistoryReadModel] {
        let path = vault_layout::database_path(db);
        maintenance::install::rename(&work.app_root.join(path), &package.join(path))?;
    }
    let envelope = key.map(|_| vault.paths().artifact_path(ArtifactId::Envelope));
    agent_history::Archive::write_detached_control(&package, envelope.as_deref())?;
    let name = stage.path().file_name().context("undo stage needs a file name")?.to_string_lossy();
    let target = vault
        .paths()
        .app_root
        .join(vault_layout::NATIVE_COPIES_V06)
        .join(format!("undo-import-{name}"));
    maintenance::install::rename(&package, &target)?;
    Ok(target)
}
