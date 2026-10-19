//! Responsibilities: verify the real storage owner's lifecycle in both key modes.
//! Not responsible for: native keychain access or application state wiring.
//! Failure modes: missing schema, wrong paths, two owners, unfinished runs reported
//! as running after reopen, committed progress lost during recovery, key ignored.

use super::*;
use vault_store::Db;

#[test]
fn encrypted_vault_refuses_another_key() {
    let root = tempfile::tempdir().unwrap();
    let paths = crate::project_paths_with_root(root.path());
    drop(Vault::open(paths.clone(), AppConfig::default(), Some([17; 32])).unwrap());
    assert!(Vault::open(paths.clone(), AppConfig::default(), None).is_err());
    assert!(Vault::open(paths, AppConfig::default(), Some([18; 32])).is_err());
}

#[test]
fn database_files_require_their_purpose_specific_keys() {
    let vault = Vault::open_temp_with_key(Some([13; 32])).unwrap();
    vault.store().close().unwrap();
    let data = vault_crypto::DataKey::from_bytes(zeroize::Zeroizing::new([13; 32]));
    let purposes = [
        vault_crypto::KeyPurpose::ArchiveDatabase,
        vault_crypto::KeyPurpose::SourceEvidenceDatabase,
        vault_crypto::KeyPurpose::IntelligenceDatabase,
        vault_crypto::KeyPurpose::AgentDatabase,
        vault_crypto::KeyPurpose::ScratchDatabase,
    ];
    let keys = purposes.map(|purpose| Some(*data.derive(purpose).expose_bytes()));
    let mut wrong = vault_store::Config::new(&vault.paths().app_root);
    wrong.migrations = crate::store_schema::migration_registry();
    wrong.keys = keys;
    wrong.keys.swap(0, 1);
    assert!(Store::open(wrong).is_err(), "database purpose keys must not be interchangeable");
    let mut correct = vault_store::Config::new(&vault.paths().app_root);
    correct.migrations = crate::store_schema::migration_registry();
    correct.keys = keys;
    drop(Store::open(correct).unwrap());
}

#[test]
fn a_password_open_beside_an_unconverted_archive_creates_nothing() {
    // An archive of the old format has to be converted first; a new envelope here would start
    // an empty native archive next to it.
    let root = tempfile::tempdir().unwrap();
    let paths = crate::project_paths_with_root(root.path());
    crate::config::ensure_paths(&paths).unwrap();
    let config = AppConfig {
        initialized: true,
        archive_mode: crate::ArchiveMode::Encrypted,
        ..Default::default()
    };
    std::fs::write(&paths.legacy.archive, [7u8; 4096]).unwrap();
    let before = std::fs::read(&paths.legacy.archive).unwrap();
    for password in ["right password", "wrong password"] {
        let error =
            Vault::open_password(paths.clone(), config.clone(), password, None).unwrap_err();
        assert!(error.to_string().starts_with("archive-conversion-required:"), "{error:#}");
    }
    assert!(!paths.app_root.join("archive/keys.envelope").exists());
    assert!(!paths.app_root.join("archive/keys.envelope.pending").exists());
    assert!(!paths.app_root.join("archive/history-vault.db").exists());
    assert_eq!(before, std::fs::read(&paths.legacy.archive).unwrap());
}

#[test]
fn missing_envelope_never_rekeys_existing_databases() {
    let root = tempfile::tempdir().unwrap();
    let paths = crate::project_paths_with_root(root.path());
    let config = AppConfig::default();
    drop(Vault::open(paths.clone(), config.clone(), Some([83; 32])).unwrap());
    let db = paths.app_root.join("archive/history-vault.db");
    let before = std::fs::read(&db).unwrap();
    let error = Vault::open_password(paths.clone(), config, "password", None).unwrap_err();
    assert!(error.downcast_ref::<super::EnvelopeMissing>().is_some());
    assert!(!paths.app_root.join("archive/keys.envelope").exists());
    assert_eq!(before, std::fs::read(db).unwrap());
}

#[test]
fn checkpoint_restore_takes_the_single_backup_guard() {
    let vault = Vault::open_temp().unwrap();
    let _backup = vault.begin_backup().unwrap();
    let error = crate::run_snapshot_restore(
        &vault,
        &crate::SnapshotRestoreRequest { snapshot_path: "unused".into() },
    )
    .unwrap_err();
    assert!(error.to_string().contains("backup-already-running"));
}

#[test]
fn pending_envelope_recovers_after_a_failure_with_database_files() {
    let root = tempfile::tempdir().unwrap();
    let paths = crate::project_paths_with_root(root.path());
    let config = AppConfig::default();
    let result = super::password::open_using(
        paths.clone(),
        config.clone(),
        "right",
        None,
        |paths, _, key, owner| {
            let mut storage = storage_config(&paths, Some(key), owner)?;
            storage.migrations.register(
                Db::SourceEvidence,
                vec![vault_store::Migration::rust(1, |_| {
                    Err(vault_store::Error::InvalidConfig(
                        "failure after history-vault.db exists".into(),
                    ))
                })],
            );
            let error = Store::open(storage).err().expect("injected migration failure");
            assert!(paths.app_root.join("archive/history-vault.db").exists());
            Err(error.into())
        },
    );
    assert!(result.is_err());
    let pending = paths.app_root.join("archive/keys.envelope.pending");
    let db = paths.app_root.join("archive/history-vault.db");
    assert!(db.exists() && pending.exists());
    let before = (std::fs::read(&db).unwrap(), std::fs::read(&pending).unwrap());
    let error = Vault::open_password(paths.clone(), config.clone(), "wrong", None).unwrap_err();
    assert!(error.downcast_ref::<super::password::ArchivePasswordWrong>().is_some());
    assert_eq!(before, (std::fs::read(&db).unwrap(), std::fs::read(&pending).unwrap()));
    drop(Vault::open_password(paths.clone(), config, "right", None).unwrap());
    assert!(!pending.exists());
    assert!(paths.app_root.join("archive/keys.envelope").exists());
}
#[test]
fn pending_envelope_crash_child() {
    let Some(root) = std::env::var_os("PATHKEEP_PENDING_ENVELOPE_ROOT") else { return };
    let paths = crate::project_paths_with_root(std::path::Path::new(&root));
    super::password::open_using(
        paths,
        AppConfig::default(),
        "right",
        None,
        |paths, config, key, owner| {
            let _vault = Vault::open_owned(paths, config, Some(key), owner)?;
            std::process::exit(81)
        },
    )
    .unwrap();
}
#[test]
fn crash_before_envelope_promotion_is_recovered_on_unlock() {
    let root = tempfile::tempdir().unwrap();
    let status = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "vault::tests::pending_envelope_crash_child"])
        .env("PATHKEEP_PENDING_ENVELOPE_ROOT", root.path())
        .status()
        .unwrap();
    assert_eq!(status.code(), Some(81));
    let paths = crate::project_paths_with_root(root.path());
    assert!(paths.app_root.join("archive/keys.envelope.pending").exists());
    drop(Vault::open_password(paths.clone(), AppConfig::default(), "right", None).unwrap());
    assert!(paths.app_root.join("archive/keys.envelope").exists());
}
