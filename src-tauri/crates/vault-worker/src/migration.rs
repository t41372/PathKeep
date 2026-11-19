//! Responsibilities: run native portable-bundle operations through the owning Vault.
//! Not responsible for: ZIP verification, cryptography or desktop session publication.
//! Failure modes: export requires a legacy password, fresh-root import refuses, a file v0.3
//! exported reaches the native importer, source/agent data omitted, wrong bundle key reaches
//! live files. Tests below.
use anyhow::Result;
use std::path::PathBuf;
use vault_core::{
    ApplyImportOptions, ExportedBundle, ImportPreview, ImportResult, ProjectPaths, Vault,
    maintenance::InstalledArchive,
};

pub fn export_app_data(
    vault: &Vault,
    target: PathBuf,
    export_password: &str,
) -> Result<ExportedBundle> {
    crate::maintenance_jobs::run(
        &vault.paths().app_root,
        "exporting",
        crate::ForegroundWork::MoveExport,
        |cancel, progress| {
            vault_core::migration::native::export(vault, &target, export_password, cancel, progress)
        },
    )
}

/// Files v0.3 exported hold the old databases; the converter imports those.
fn made_by_v0_3(bundle: &std::path::Path) -> Result<bool> {
    Ok(vault_core::migration::read_and_validate_manifest(bundle)?.format_version
        == vault_convert_sqlcipher::bundle::FORMAT)
}

/// The portable manifest stays password-free. Visibility belongs to this destination.
#[derive(serde::Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct DeviceImportPreview {
    #[serde(flatten)]
    pub archive: ImportPreview,
    pub agent_conversations: AgentImportFacts,
}

#[derive(serde::Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct AgentImportFacts {
    /// Format 3 carries the conversation role even when it is empty.
    pub carried: bool,
    pub state_if_present: Option<vault_core::KindState>,
}

impl std::ops::Deref for DeviceImportPreview {
    type Target = ImportPreview;
    fn deref(&self) -> &Self::Target {
        &self.archive
    }
}

pub fn preview_import(paths: &ProjectPaths, bundle: PathBuf) -> Result<DeviceImportPreview> {
    vault_core::migration::native::imported_agent_state_if_present(paths)?;
    let archive = if made_by_v0_3(&bundle)? {
        vault_convert_sqlcipher::bundle::preview(paths, &bundle)?
    } else {
        vault_core::migration::native::preview(paths, &bundle)?
    };
    let carried = archive.manifest.format_version == 3;
    let state_if_present =
        carried.then_some(vault_core::migration::native::imported_agent_state_if_present(paths)?);
    Ok(DeviceImportPreview {
        archive,
        agent_conversations: AgentImportFacts { carried, state_if_present },
    })
}

pub fn apply_import(
    paths: &ProjectPaths,
    current: Option<&Vault>,
    owner: Option<std::sync::Arc<vault_store::OwnerLock>>,
    bundle: PathBuf,
    options: ApplyImportOptions,
) -> Result<(ImportResult, InstalledArchive)> {
    // A wipe cut short finishes first, as before initialization: the user confirmed it, and an
    // archive installed under its record would be deleted by it at the next start.
    crate::data_wipe::finish_interrupted_data_wipe()?;
    crate::context::load_unlocked_config(paths)?;
    let v0_3 = made_by_v0_3(&bundle)?;
    crate::maintenance_jobs::run(
        &paths.app_root,
        "importing",
        crate::ForegroundWork::MoveImport,
        |cancel, progress| {
            if v0_3 {
                vault_convert_sqlcipher::bundle::import(
                    paths, current, owner, &bundle, &options, cancel, progress,
                )
            } else {
                vault_core::migration::native::import(
                    paths, current, owner, &bundle, &options, cancel,
                )
            }
        },
    )
}
