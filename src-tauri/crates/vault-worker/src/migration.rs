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
    crate::maintenance_jobs::run(&vault.paths().app_root, "exporting", |cancel, progress| {
        vault_core::migration::native::export(vault, &target, export_password, cancel, progress)
    })
}

/// Files v0.3 exported (format 1) hold the old databases; the converter imports those.
fn made_by_v0_3(bundle: &std::path::Path) -> Result<bool> {
    Ok(vault_core::migration::read_and_validate_manifest(bundle)?.format_version
        == vault_convert_sqlcipher::bundle::FORMAT)
}

pub fn preview_import(paths: &ProjectPaths, bundle: PathBuf) -> Result<ImportPreview> {
    if made_by_v0_3(&bundle)? {
        return vault_convert_sqlcipher::bundle::preview(paths, &bundle);
    }
    vault_core::migration::native::preview(paths, &bundle)
}

pub fn apply_import(
    paths: &ProjectPaths,
    current: Option<&Vault>,
    owner: Option<std::sync::Arc<vault_store::OwnerLock>>,
    bundle: PathBuf,
    options: ApplyImportOptions,
) -> Result<(ImportResult, InstalledArchive)> {
    crate::context::load_unlocked_config(paths)?;
    let v0_3 = made_by_v0_3(&bundle)?;
    crate::maintenance_jobs::run(&paths.app_root, "importing", |cancel, progress| {
        if v0_3 {
            vault_convert_sqlcipher::bundle::import(
                paths, current, owner, &bundle, &options, cancel, progress,
            )
        } else {
            vault_core::migration::native::import(paths, current, owner, &bundle, &options, cancel)
        }
    })
}
