//! Worker-layer wrappers for whole-app Export / Import.
//!
//! These functions hydrate the unlocked config + canonical project paths
//! from the desktop session and delegate the actual zip pack / unpack to
//! `vault-core::migration`. The session key flow matches every other
//! archive surface so encrypted archives can be exported without leaking
//! plaintext through a temporary on-disk path that lives outside the
//! project staging dir.

use crate::context::load_unlocked_config;
use anyhow::Result;
use std::path::PathBuf;
use vault_core::{ApplyImportOptions, ExportedBundle, ImportPreview, ImportResult};

/// Packs the entire local project into a `.pathkeep-bundle` zip at
/// `target_path`. The caller (Settings → Data Migration) supplies the
/// path; this layer only enforces that the active session knows the
/// archive unlock key when the archive is encrypted.
pub fn export_app_data(
    session_database_key: Option<&str>,
    target_path: PathBuf,
) -> Result<ExportedBundle> {
    vault_core::require_complete_storage_migration("full-archive export")?;
    let paths = vault_core::project_paths()?;
    let config = load_unlocked_config(&paths)?;
    vault_core::export_app_data(&paths, &config, session_database_key, &target_path)
}

/// Validates a bundle's manifest + sha256 anti-tamper sidecar and returns
/// the preview the Settings UI shows before the user confirms the
/// destructive overwrite. Read-only.
pub fn preview_import(bundle_path: PathBuf) -> Result<ImportPreview> {
    vault_core::require_complete_storage_migration("full-archive import")?;
    let paths = vault_core::project_paths()?;
    vault_core::preview_import(&paths, &bundle_path)
}

/// Applies a previously-previewed bundle onto the live project tree.
/// `options.confirm_overwrite` must be set when the target already has
/// an initialized archive.
pub fn apply_import(
    session_database_key: Option<&str>,
    bundle_path: PathBuf,
    options: ApplyImportOptions,
) -> Result<ImportResult> {
    vault_core::require_complete_storage_migration("full-archive import")?;
    let paths = vault_core::project_paths()?;
    let config = load_unlocked_config(&paths)?;
    vault_core::apply_import(&paths, &config, session_database_key, &bundle_path, &options)
}

#[cfg(test)]
mod tests {
    //! Responsibilities: verify mixed-storage refusals before filesystem mutation.
    //! Not responsible for: the legacy bundle format, covered by vault-core migration tests.
    //! Failure modes: export creates a misleading bundle; import overwrites one store only.
    use super::*;
    #[test]
    fn export_is_unavailable_until_storage_migration_completes() {
        let root = tempfile::tempdir().unwrap();
        let target = root.path().join("archive.pathkeep-bundle");
        let error = export_app_data(None, target.clone()).unwrap_err();
        assert!(error.downcast_ref::<vault_core::StorageMigrationIncomplete>().is_some());
        assert!(!target.exists());
    }
    #[test]
    fn preview_import_reports_the_typed_migration_refusal() {
        let error = preview_import(PathBuf::from("absent.pathkeep-bundle")).unwrap_err();
        assert!(error.downcast_ref::<vault_core::StorageMigrationIncomplete>().is_some());
    }
    #[test]
    fn import_cannot_replace_archive_files_during_migration() {
        let root = tempfile::tempdir().unwrap();
        let bundle = root.path().join("archive.pathkeep-bundle");
        std::fs::write(&bundle, "preserve these bytes").unwrap();
        let error = apply_import(
            None,
            bundle.clone(),
            ApplyImportOptions { confirm_overwrite: true, ..Default::default() },
        )
        .unwrap_err();
        assert!(error.downcast_ref::<vault_core::StorageMigrationIncomplete>().is_some());
        assert_eq!(std::fs::read_to_string(bundle).unwrap(), "preserve these bytes");
    }
}
