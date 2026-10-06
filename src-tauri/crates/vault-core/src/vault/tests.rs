//! Responsibilities: verify the real storage owner's lifecycle in both key modes.
//! Not responsible for: native keychain access or application state wiring.
//! Failure modes: missing schema, wrong paths, two owners, unfinished runs reported
//! as running after reopen, committed progress lost during recovery, key ignored.

use super::*;
use vault_store::{Db, Error, Priority};

#[test]
fn temporary_vaults_have_the_complete_schema_in_both_key_modes() {
    for key in [None, Some([37; 32])] {
        let vault = Vault::open_temp_with_key(key).unwrap();
        assert!(vault.paths().archive_database_path.ends_with("history-vault.db"));
        let count = vault
            .store()
            .read(Db::Archive, |s| {
                s.query_one("SELECT COUNT(*) FROM search_outbox", (), |r| Ok(r.get::<i64>(0)?))
            })
            .unwrap();
        assert_eq!(count, 0);
        assert!(!vault.paths().app_root.join("derived/history-search.sqlite").exists());
    }
}

#[test]
fn reopen_interrupts_unfinished_runs_without_losing_progress() {
    for key in [None, Some([93; 32])] {
        let root = tempfile::tempdir().unwrap();
        let paths = crate::project_paths_with_root(root.path());
        let config = AppConfig { initialized: true, ..AppConfig::default() };
        let vault = Vault::open(paths.clone(), config.clone(), key).unwrap();
        assert!(matches!(
            Vault::open(paths.clone(), config.clone(), key),
            Err(Error::OwnedElsewhere { .. })
        ));
        vault.store().write(Db::Archive, Priority::Batch, |s| {
            s.execute("INSERT INTO runs(run_type,trigger,started_at,status,stats_json) VALUES('backup','manual','2026-10-04','running','{\"processedRecords\":10000}')", ())?;
            s.execute("INSERT INTO profile_watermarks(profile_id,last_visit_id,updated_at) VALUES('chrome:test',10000,'2026-10-04')", ())?;
            Ok(())
        }).unwrap();
        drop(vault);
        let vault = Vault::open(paths, config, key).unwrap();
        let (status, progress, watermark) = vault.store().read(Db::Archive, |s| {
            s.query_one("SELECT status,stats_json,(SELECT last_visit_id FROM profile_watermarks WHERE profile_id='chrome:test') FROM runs WHERE run_type='backup'", (), |r| Ok((r.get::<String>(0)?, r.get::<String>(1)?, r.get::<i64>(2)?)))
        }).unwrap();
        assert_eq!(status, "interrupted");
        assert_eq!(progress, "{\"processedRecords\":10000}");
        assert_eq!(watermark, 10000);
    }
}

#[test]
fn encrypted_vault_refuses_another_key() {
    let root = tempfile::tempdir().unwrap();
    let paths = crate::project_paths_with_root(root.path());
    drop(Vault::open(paths.clone(), AppConfig::default(), Some([17; 32])).unwrap());
    assert!(Vault::open(paths.clone(), AppConfig::default(), None).is_err());
    assert!(Vault::open(paths, AppConfig::default(), Some([18; 32])).is_err());
}

#[test]
fn vault_uses_the_reserved_owner_without_acquiring_another_lock() {
    let root = tempfile::tempdir().unwrap();
    let paths = crate::project_paths_with_root(root.path());
    let owner = Arc::new(vault_store::OwnerLock::acquire(root.path()).unwrap());
    let vault = Vault::open_owned(paths, AppConfig::default(), None, Some(owner.clone())).unwrap();
    assert!(vault_store::OwnerLock::acquire(root.path()).is_err());
    drop(vault);
    assert!(vault_store::OwnerLock::acquire(root.path()).is_err());
    drop(owner);
    assert!(vault_store::OwnerLock::acquire(root.path()).is_ok());
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
fn upgrade_password_is_checked_before_creating_any_turso_files() {
    let root = tempfile::tempdir().unwrap();
    let paths = crate::project_paths_with_root(root.path());
    crate::config::ensure_paths(&paths).unwrap();
    let config = AppConfig {
        initialized: true,
        archive_mode: crate::ArchiveMode::Encrypted,
        ..Default::default()
    };
    drop(crate::archive::open_archive_connection(&paths, &config, Some("right password")).unwrap());
    let before = std::fs::read(&paths.archive_database_path).unwrap();
    assert!(Vault::open_password(paths.clone(), config.clone(), "wrong password", None).is_err());
    assert!(!paths.app_root.join("archive/keys.envelope").exists());
    assert!(!paths.app_root.join("archive/history-vault.db").exists());
    assert_eq!(before, std::fs::read(&paths.archive_database_path).unwrap());
    let (vault, _) = Vault::open_password(paths.clone(), config, "right password", None).unwrap();
    assert!(paths.app_root.join("archive/keys.envelope").exists());
    drop(vault);
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
fn new_encrypted_archive_persists_one_envelope_after_open() {
    let root = tempfile::tempdir().unwrap();
    let paths = crate::project_paths_with_root(root.path());
    let (vault, first) =
        Vault::open_password(paths.clone(), AppConfig::default(), "password", None).unwrap();
    let path = paths.app_root.join("archive/keys.envelope");
    let envelope = std::fs::read(&path).unwrap();
    assert_eq!(
        std::fs::read_dir(path.parent().unwrap())
            .unwrap()
            .filter(|entry| entry
                .as_ref()
                .unwrap()
                .file_name()
                .to_string_lossy()
                .contains("envelope"))
            .count(),
        1
    );

    assert!(Vault::unlock_data_key(&paths, "wrong password").is_err());
    assert_eq!(envelope, std::fs::read(&path).unwrap());
    drop(vault);
    let (_, second) = Vault::open_password(paths, AppConfig::default(), "password", None).unwrap();
    assert_eq!(first.expose_bytes(), second.expose_bytes());
    assert_eq!(envelope, std::fs::read(path).unwrap());
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
fn failed_store_open_does_not_publish_an_envelope() {
    let root = tempfile::tempdir().unwrap();
    let paths = crate::project_paths_with_root(root.path());
    let _other_owner = vault_store::OwnerLock::acquire(root.path()).unwrap();
    assert!(Vault::open_password(paths.clone(), AppConfig::default(), "password", None).is_err());
    assert!(!paths.app_root.join("archive/keys.envelope").exists());
    assert!(!paths.app_root.join("archive/history-vault.db").exists());
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

#[test]
fn passwordless_native_unlock_refuses_only_legacy_database_opens() {
    let vault = Vault::open_temp_with_key(Some([65; 32])).unwrap();
    let paths = crate::project_paths_with_root(&vault.paths().app_root);
    let config = vault.config();
    assert!(crate::legacy_archive_status(&paths, config, None).unwrap().unlocked);
    let errors = [
        crate::archive::open_archive_connection(&paths, config, None).unwrap_err(),
        crate::archive::open_intelligence_connection(&paths, config, None).unwrap_err(),
        crate::archive::open_legacy_source_evidence_connection(&paths, config, None).unwrap_err(),
    ];
    for error in errors {
        assert!(error.to_string().starts_with("storage-migration-incomplete:"), "{error:#}");
    }
}
