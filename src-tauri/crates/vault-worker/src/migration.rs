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

#[cfg(test)]
mod tests {
    use super::*;
    fn exported() -> (Vault, tempfile::TempDir, PathBuf) {
        let vault = Vault::open_temp().unwrap();
        let root = tempfile::tempdir().unwrap();
        let target = root.path().join("archive.pathkeep-bundle");
        export_app_data(&vault, target.clone(), "export password").unwrap();
        (vault, root, target)
    }
    #[test]
    fn export_app_data_writes_a_bundle_at_the_requested_target() {
        let (_vault, _root, target) = exported();
        assert!(target.is_file());
    }
    #[test]
    fn preview_import_reads_a_bundle_written_by_export_app_data() {
        let (_vault, root, target) = exported();
        let preview =
            preview_import(&vault_core::project_paths_with_root(root.path()), target).unwrap();
        assert_eq!(preview.manifest.archive_mode, "encrypted");
        assert!(preview.manifest.files.iter().any(|f| f.path == "archive/agent.db"));
    }
    #[test]
    fn apply_import_restores_an_exported_bundle_onto_a_fresh_project_root() {
        let (_vault, _root, target) = exported();
        let destination = tempfile::tempdir().unwrap();
        let paths = vault_core::project_paths_with_root(destination.path());
        let (report, archive) = apply_import(
            &paths,
            None,
            None,
            target,
            ApplyImportOptions {
                confirm_overwrite: true,
                source_archive_key: Some("export password".into()),
            },
        )
        .unwrap();
        assert_eq!(report.final_schema_version, 1);
        assert!(archive.vault.paths().archive_database_path.is_file());
        assert!(Vault::unlock_data_key(&paths, "export password").is_ok());
    }
}
