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
    let paths = vault_core::project_paths()?;
    let config = load_unlocked_config(&paths)?;
    vault_core::export_app_data(&paths, &config, session_database_key, &target_path)
}

/// Validates a bundle's manifest + sha256 anti-tamper sidecar and returns
/// the preview the Settings UI shows before the user confirms the
/// destructive overwrite. Read-only.
pub fn preview_import(bundle_path: PathBuf) -> Result<ImportPreview> {
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
    let paths = vault_core::project_paths()?;
    let config = load_unlocked_config(&paths)?;
    vault_core::apply_import(&paths, &config, session_database_key, &bundle_path, &options)
}

#[cfg(test)]
mod tests {
    //! Worker-layer migration smoke tests. The actual zip pack/unpack
    //! logic lives in `vault_core::migration` and is covered there. The
    //! tests below pin the worker's contract end-to-end: that
    //! `export_app_data` / `preview_import` / `apply_import` hydrate the
    //! correct project paths + unlocked config and that the bundle
    //! produced by export can round-trip through preview and apply onto
    //! a second project root.
    //!
    //! All three tests mutate `CHB_PROJECT_ROOT` / `CHB_TEST_KEYRING_DIR`
    //! so they serialise through the shared `lock_env()` guard alongside
    //! every other env-dependent vault-worker test.
    use super::*;
    use crate::tests::{
        PROJECT_ROOT_OVERRIDE_ENV, TEST_KEYRING_OVERRIDE_ENV, lock_env, restore_env_var,
    };
    use tempfile::tempdir;
    use vault_core::{AppConfig, ArchiveMode};

    fn write_test_config(paths: &vault_core::ProjectPaths, config: &AppConfig) {
        if let Some(parent) = paths.archive_database_path.parent() {
            std::fs::create_dir_all(parent).expect("archive db parent");
        }
        if let Some(parent) = paths.config_path.parent() {
            std::fs::create_dir_all(parent).expect("config parent");
        }
        std::fs::write(&paths.config_path, serde_json::to_string(config).expect("config json"))
            .expect("write config");
    }

    fn initialized_plaintext_config() -> AppConfig {
        AppConfig {
            initialized: true,
            archive_mode: ArchiveMode::Plaintext,
            ..AppConfig::default()
        }
    }

    /// Seeds a project root with an initialized plaintext archive and
    /// the matching on-disk config, then returns the canonical paths so
    /// the worker-layer entrypoints can find them through
    /// `vault_core::project_paths()`. The env override is owned by the
    /// caller because each test wants to swap roots between `set_var`
    /// calls.
    fn seed_project_root(root: &std::path::Path) -> vault_core::ProjectPaths {
        unsafe {
            std::env::set_var(PROJECT_ROOT_OVERRIDE_ENV, root);
            std::env::set_var(TEST_KEYRING_OVERRIDE_ENV, root.join("keyring"));
        }
        let paths = vault_core::project_paths().expect("paths");
        let config = initialized_plaintext_config();
        write_test_config(&paths, &config);
        vault_core::ensure_archive_initialized(&paths, &config, None).expect("init archive");
        paths
    }

    #[test]
    fn export_app_data_writes_a_bundle_at_the_requested_target() {
        let _guard = lock_env();
        let dir = tempdir().expect("tempdir");
        let original_project_root = std::env::var_os(PROJECT_ROOT_OVERRIDE_ENV);
        let original_keyring = std::env::var_os(TEST_KEYRING_OVERRIDE_ENV);

        let _paths = seed_project_root(dir.path());
        let target = dir.path().join("export.pathkeep-bundle");
        let bundle = export_app_data(None, target.clone()).expect("export");
        assert_eq!(bundle.bundle_path, target);
        assert!(target.exists(), "bundle file must be on disk at the requested target");
        // Manifest carries the config + archive db we just seeded.
        assert!(
            bundle.manifest.files.iter().any(|f| f.path == "archive/history-vault.sqlite"),
            "archive db missing from manifest: {:?}",
            bundle.manifest.files,
        );
        assert!(
            bundle.manifest.files.iter().any(|f| f.path == "config/config.json"),
            "config missing from manifest",
        );

        restore_env_var(PROJECT_ROOT_OVERRIDE_ENV, original_project_root.as_deref());
        restore_env_var(TEST_KEYRING_OVERRIDE_ENV, original_keyring.as_deref());
    }

    #[test]
    fn preview_import_reads_a_bundle_written_by_export_app_data() {
        let _guard = lock_env();
        let src_dir = tempdir().expect("src tempdir");
        let dest_dir = tempdir().expect("dest tempdir");
        let original_project_root = std::env::var_os(PROJECT_ROOT_OVERRIDE_ENV);
        let original_keyring = std::env::var_os(TEST_KEYRING_OVERRIDE_ENV);

        // Export from a seeded source root.
        let _src_paths = seed_project_root(src_dir.path());
        let bundle_path = src_dir.path().join("bundle.pathkeep-bundle");
        export_app_data(None, bundle_path.clone()).expect("export");

        // Swap the env to point at a fresh, uninitialized destination
        // root so the preview's overwrite flag stays false. The worker
        // entrypoint pulls `paths` from the env, so swapping it is the
        // only way to drive `preview_import` against a different root.
        unsafe {
            std::env::set_var(PROJECT_ROOT_OVERRIDE_ENV, dest_dir.path());
            std::env::set_var(TEST_KEYRING_OVERRIDE_ENV, dest_dir.path().join("keyring"));
        }

        let preview = preview_import(bundle_path).expect("preview");
        assert!(!preview.will_overwrite_existing, "fresh destination root must not flag overwrite",);
        assert_eq!(preview.manifest.archive_mode, "plaintext");

        restore_env_var(PROJECT_ROOT_OVERRIDE_ENV, original_project_root.as_deref());
        restore_env_var(TEST_KEYRING_OVERRIDE_ENV, original_keyring.as_deref());
    }

    #[test]
    fn apply_import_restores_an_exported_bundle_onto_a_fresh_project_root() {
        let _guard = lock_env();
        let src_dir = tempdir().expect("src tempdir");
        let dest_dir = tempdir().expect("dest tempdir");
        let original_project_root = std::env::var_os(PROJECT_ROOT_OVERRIDE_ENV);
        let original_keyring = std::env::var_os(TEST_KEYRING_OVERRIDE_ENV);

        // Export from a seeded source root.
        let _src_paths = seed_project_root(src_dir.path());
        let bundle_path = src_dir.path().join("bundle.pathkeep-bundle");
        export_app_data(None, bundle_path.clone()).expect("export");

        // Apply onto a separate, uninitialized destination. We still
        // need a config.json there because `load_unlocked_config` is
        // called inside the worker wrapper before delegating to core.
        unsafe {
            std::env::set_var(PROJECT_ROOT_OVERRIDE_ENV, dest_dir.path());
            std::env::set_var(TEST_KEYRING_OVERRIDE_ENV, dest_dir.path().join("keyring"));
        }
        let dest_paths = vault_core::project_paths().expect("dest paths");
        write_test_config(&dest_paths, &initialized_plaintext_config());

        let result =
            apply_import(None, bundle_path, ApplyImportOptions { confirm_overwrite: false, ..Default::default() })
                .expect("apply");
        assert!(
            dest_paths.archive_database_path.exists(),
            "archive db missing on destination after apply_import",
        );
        // The result echoes the bundle's schema version; just assert it
        // is a valid (non-zero) ledger entry — the exact number drifts
        // with future migrations and is asserted in vault-core itself.
        assert!(
            result.final_schema_version > 0,
            "apply_import should resolve to a real schema version",
        );

        restore_env_var(PROJECT_ROOT_OVERRIDE_ENV, original_project_root.as_deref());
        restore_env_var(TEST_KEYRING_OVERRIDE_ENV, original_keyring.as_deref());
    }
}
