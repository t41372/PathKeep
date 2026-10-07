//! Responsibilities: restart the actual owner after process exit during conversion or installation.
//! Not responsible for: synthetic journals, SQLCipher decoding or frontend rendering.
//! Failure modes: retained SQLCipher shadows a recoverable native archive, credentials
//! are read before envelope recovery, replay duplicates rows, or originals are lost.
use super::*;
use std::{fs, process::Command};
use vault_store::Db;

#[test]
fn restart_recovers_before_conversion_and_keychain_decisions() {
    let _guard = crate::tests::lock_env();
    let names = [crate::tests::PROJECT_ROOT_OVERRIDE_ENV, crate::tests::TEST_KEYRING_OVERRIDE_ENV];
    let old = names.map(std::env::var_os);
    for operation in ["copy", "conversion-install", "mode-install"] {
        let root = tempfile::tempdir().unwrap();
        unsafe {
            std::env::set_var(names[0], root.path());
            std::env::set_var(names[1], root.path().join("keyring"));
        }
        let paths = vault_core::project_paths().unwrap();
        let config = AppConfig {
            initialized: true,
            git_enabled: false,
            archive_mode: ArchiveMode::Encrypted,
            remember_database_key_in_keyring: true,
            ..Default::default()
        };
        vault_core::save_config(&paths, &config).unwrap();
        vault_core::test_support::legacy_fixtures::seed_reader_pages(
            &paths,
            &config,
            Some("password"),
            &[("https://example.com/%7Erestart", "Restart proof")],
        );
        let original = fs::read(&paths.archive_database_path).unwrap();
        if operation == "mode-install" {
            let installed = vault_convert_sqlcipher::convert(
                &paths,
                &config,
                Some("password"),
                Default::default(),
                |_| {},
            )
            .unwrap();
            crate::security::store_data_key_in_keyring(installed.data_key.as_ref().unwrap())
                .unwrap();
            installed.vault.close().unwrap();
        }
        let status = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "session::recovery_tests::conversion_restart_child",
                "--ignored",
                "--nocapture",
            ])
            .env("PATHKEEP_RESTART_OPERATION", operation)
            .status()
            .unwrap();
        assert_eq!(status.code(), Some(86));
        if operation != "copy" {
            assert!(vault_core::maintenance::install::pending(&paths));
        }
        let session = VaultSession::default();
        session.begin_startup();
        if operation == "mode-install" {
            session.finish_startup().unwrap();
            assert!(session.typed_password().is_none());
        } else {
            let error = session.finish_startup().unwrap_err();
            assert!(error.starts_with("archive-conversion-required:"), "{error}");
            assert!(
                crate::assess_archive_upgrade(Some("password"))
                    .unwrap()
                    .conversion
                    .unwrap()
                    .required
            );
            crate::convert_archive_with_progress(&session, &config, Some("password"), |_| {})
                .unwrap();
        }
        assert!(!vault_core::maintenance::install::pending(&paths));
        let vault = session.vault().unwrap();
        assert_eq!(
            vault
                .store()
                .read(Db::Archive, |s| s
                    .query_one("SELECT count(*) FROM visits", (), |r| Ok(r.get::<i64>(0)?)))
                .unwrap(),
            1
        );
        assert_eq!(vault.config().archive_mode, ArchiveMode::Encrypted);
        assert_eq!(fs::read(&paths.archive_database_path).unwrap(), original);
        session.close_vault().unwrap();
    }
    for (name, value) in names.into_iter().zip(old) {
        crate::tests::restore_env_var(name, value.as_deref());
    }
}

#[test]
#[ignore = "process-exit child used by restart recovery acceptance"]
fn conversion_restart_child() {
    let paths = vault_core::project_paths().unwrap();
    let config = vault_core::load_config(&paths).unwrap();
    let operation = std::env::var("PATHKEEP_RESTART_OPERATION").unwrap();
    if operation == "mode-install" {
        let (vault, _) = Vault::open_password(paths, config, "password", None).unwrap();
        vault_core::fault_inject::exit_process_at_next_checkpoint("native-install-preserved");
        vault_core::maintenance::switch_mode(
            &vault,
            Some("password"),
            ArchiveMode::Plaintext,
            None,
            Default::default(),
            |_| {},
        )
        .unwrap();
    } else {
        if operation == "conversion-install" {
            vault_core::fault_inject::exit_process_at_next_checkpoint("native-install-installed");
        }
        vault_convert_sqlcipher::convert(
            &paths,
            &config,
            Some("password"),
            Default::default(),
            |p| {
                if operation == "copy" && p.table.as_deref() == Some("urls") {
                    std::process::exit(86);
                }
            },
        )
        .unwrap();
    }
    panic!("the requested crash point did not run");
}
