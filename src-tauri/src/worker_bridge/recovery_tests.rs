//! Responsibilities: restart through the desktop assessment and initialization commands.
//! Not responsible for: synthetic recovery files, UI rendering or intelligence execution.
//! Failure modes: a committed native generation is mistaken for its retained SQLCipher
//! input, stale credentials decide mode, or bootstrap opens the old database again.
use super::*;
use crate::test_support::{PROJECT_ROOT_OVERRIDE_ENV, TEST_KEYRING_OVERRIDE_ENV, lock_env};

#[test]
fn assessment_and_bootstrap_finish_a_published_mode_switch_after_process_exit() {
    let _guard = lock_env();
    let root = tempfile::tempdir().unwrap();
    let names = [PROJECT_ROOT_OVERRIDE_ENV, TEST_KEYRING_OVERRIDE_ENV];
    let old = names.map(std::env::var_os);
    unsafe {
        std::env::set_var(names[0], root.path());
        std::env::set_var(names[1], root.path().join("keyring"));
    }
    let paths = vault_core::project_paths().unwrap();
    let config = AppConfig {
        initialized: true,
        git_enabled: false,
        archive_mode: ArchiveMode::Encrypted,
        ..Default::default()
    };
    vault_core::save_config(&paths, &config).unwrap();
    vault_convert_sqlcipher::test_support::seed_reader_pages(
        &paths,
        &config,
        Some("old password"),
        &[("https://example.com/recovery", "Recovery")],
    );
    let session = SessionState::default();
    initialize_archive_with_progress_impl(config, Some("old password".into()), &session, |_| {})
        .unwrap();
    session.close_vault().unwrap();
    let status = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "worker_bridge::recovery_tests::published_mode_child",
            "--ignored",
            "--nocapture",
        ])
        .status()
        .unwrap();
    assert_eq!(status.code(), Some(86));
    assert!(vault_core::maintenance::install::pending(&paths));
    let restarted = SessionState::default();
    let assessment = assess_archive_upgrade_impl(&restarted, None).unwrap();
    assert!(!assessment.pending);
    assert!(!assessment.conversion.unwrap().required);
    assert!(!vault_core::maintenance::install::pending(&paths));
    let config = vault_core::load_config(&paths).unwrap();
    assert_eq!(config.archive_mode, ArchiveMode::Plaintext);
    let snapshot = initialize_archive_with_progress_impl(config, None, &restarted, |_| {}).unwrap();
    assert!(snapshot.archive_status.unlocked);
    assert!(!snapshot.archive_status.encrypted);
    assert_eq!(
        restarted
            .vault()
            .unwrap()
            .store()
            .read(vault_store::Db::Archive, |s| s.query_one(
                "SELECT count(*) FROM visits",
                (),
                |r| Ok(r.get::<i64>(0)?)
            ))
            .unwrap(),
        1
    );
    restarted.close_vault().unwrap();
    for (name, value) in names.into_iter().zip(old) {
        unsafe {
            match value {
                Some(v) => std::env::set_var(name, v),
                None => std::env::remove_var(name),
            }
        }
    }
}

#[test]
#[ignore = "process-exit child used by desktop recovery acceptance"]
fn published_mode_child() {
    let paths = vault_core::project_paths().unwrap();
    let config = vault_core::load_config(&paths).unwrap();
    let (vault, _) = vault_core::Vault::open_password(paths, config, "old password", None).unwrap();
    vault_core::fault_inject::exit_process_at_next_checkpoint("native-install-published");
    vault_core::maintenance::switch_mode(
        &vault,
        Some("old password"),
        ArchiveMode::Plaintext,
        None,
        Default::default(),
        |_| {},
    )
    .unwrap();
    panic!("the publication checkpoint was not reached");
}
