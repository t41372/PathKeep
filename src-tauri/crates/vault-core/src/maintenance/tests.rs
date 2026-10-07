//! Responsibilities: acceptance of native maintenance on real project roots.
//! Not responsible for: browser parsing or UI flows.
//! Failure modes: rewrap changes data/key; wrong passwords mutate the envelope;
//! copying loses a database; cancel replaces the source; recovery mixes generations.
use super::*;
use crate::{AppConfig, ArchiveMode, Vault, project_paths_with_root};
use vault_store::{Db, Priority};

pub(crate) fn encrypted() -> (tempfile::TempDir, Vault, vault_crypto::DataKey) {
    let root = tempfile::tempdir().unwrap();
    let paths = project_paths_with_root(root.path());
    crate::config::ensure_paths(&paths).unwrap();
    let config = AppConfig {
        initialized: true,
        git_enabled: false,
        archive_mode: ArchiveMode::Encrypted,
        ..Default::default()
    };
    crate::save_config(&paths, &config).unwrap();
    let (vault, key) = Vault::open_password(paths, config, "old password", None).unwrap();
    (root, vault, key)
}

#[test]
fn password_change_only_rewraps_and_the_remembered_data_key_still_opens() {
    let (_root, vault, key) = encrypted();
    let pause = vault.store().pause().unwrap();
    pause.checkpoint().unwrap();
    let paths = vault_store::Paths::under(&vault.paths().app_root);
    let before: Vec<_> =
        Db::ALL.into_iter().map(|db| std::fs::read(paths.get(db)).unwrap()).collect();
    let envelope = std::fs::read(vault.paths().app_root.join("archive/keys.envelope")).unwrap();
    assert!(
        rewrap_password(&vault, "incorrect", "new password")
            .unwrap_err()
            .downcast_ref::<crate::ArchivePasswordWrong>()
            .is_some()
    );
    assert_eq!(
        std::fs::read(vault.paths().app_root.join("archive/keys.envelope")).unwrap(),
        envelope
    );
    rewrap_password(&vault, "old password", "new password").unwrap();
    let after: Vec<_> =
        Db::ALL.into_iter().map(|db| std::fs::read(paths.get(db)).unwrap()).collect();
    assert_eq!(before, after);
    assert!(Vault::unlock_data_key(vault.paths(), "old password").is_err());
    assert_eq!(
        Vault::unlock_data_key(vault.paths(), "new password").unwrap().expose_bytes(),
        key.expose_bytes()
    );
    drop(pause);
    vault.close().unwrap();
    let reopened =
        Vault::open(vault.paths().clone(), vault.config().clone(), Some(*key.expose_bytes()))
            .unwrap();
    reopened
        .store()
        .read(Db::Archive, |s| {
            s.query_one("SELECT count(*) FROM pages", (), |r| Ok(r.get::<i64>(0)?))
        })
        .unwrap();
}

#[test]
fn mode_switch_round_trips_all_databases_and_cancel_keeps_original() {
    let vault = Vault::open_temp().unwrap();
    crate::test_support::search::seed_pages(&vault, 2);
    crate::config::ensure_paths(vault.paths()).unwrap();
    crate::save_config(vault.paths(), vault.config()).unwrap();
    for db in [Db::Archive, Db::SourceEvidence, Db::Intelligence, Db::Agent] {
        vault
            .store()
            .write(db, Priority::Batch, |s| {
                s.execute(
                    "CREATE TABLE maintenance_sentinel(id INTEGER PRIMARY KEY,body TEXT)",
                    (),
                )?;
                s.execute(
                    "INSERT INTO maintenance_sentinel VALUES(1,'retain every database')",
                    (),
                )?;
                Ok(())
            })
            .unwrap();
    }
    let cancel = vault_store::CancelToken::new();
    let stop = cancel.clone();
    assert!(
        switch_mode(&vault, None, ArchiveMode::Encrypted, Some("new"), cancel, move |progress| {
            if progress.phase == "copying" && progress.processed > 0 {
                stop.cancel();
            }
        },)
        .is_err()
    );
    assert!(vault.store().is_open());
    let encrypted =
        switch_mode(&vault, None, ArchiveMode::Encrypted, Some("new"), Default::default(), |_| {})
            .unwrap();
    assert_search(&encrypted.vault);
    assert!(encrypted.previous_files.join("snapshot.json").is_file());
    assert!(
        Vault::unlock_data_key(&project_paths_with_root(&encrypted.previous_files), "new").is_ok()
    );
    let plain = switch_mode(
        &encrypted.vault,
        Some("new"),
        ArchiveMode::Plaintext,
        None,
        Default::default(),
        |_| {},
    )
    .unwrap();
    assert_search(&plain.vault);
    assert!(plain.previous_files.join("snapshot.json").is_file());
    for db in [Db::Archive, Db::SourceEvidence, Db::Intelligence, Db::Agent] {
        let body = plain
            .vault
            .store()
            .read(db, |s| {
                s.query_one("SELECT body FROM maintenance_sentinel WHERE id=1", (), |r| {
                    Ok(r.get::<String>(0)?)
                })
            })
            .unwrap();
        assert_eq!(body, "retain every database");
    }
    assert!(!plain.vault.paths().app_root.join("archive/keys.envelope").exists());
}

fn assert_search(vault: &Vault) {
    vault.search().cycle().unwrap();
    let mut query = crate::search::SearchSession::start(
        vault,
        crate::search::SearchRequest { query: "ancienttransport".into(), ..Default::default() },
    )
    .unwrap();
    assert_eq!(query.finish_page().unwrap().items[0].page_id, 1);
}

#[test]
fn snapshot_restores_evidence_and_agent_alongside_the_archive() {
    let (_root, vault, _key) = encrypted();
    vault.store().write(Db::Archive,Priority::Batch,|s|{
        s.execute("INSERT INTO star(entity_kind,entity_key,starred_at) VALUES('url','https://example.com/','before')",())?;Ok(())
    }).unwrap();
    let path = vault.paths().raw_snapshots_dir.join("maintenance/test");
    snapshot(&vault, &path).unwrap();
    vault
        .store()
        .write(Db::Archive, Priority::Interactive, |s| {
            s.execute("DELETE FROM star", ())?;
            Ok(())
        })
        .unwrap();
    let restored =
        restore_snapshot(&vault, &path, Some("old password"), Default::default()).unwrap();
    assert_eq!(
        restored
            .vault
            .store()
            .read(Db::Archive, |s| s
                .query_one("SELECT count(*) FROM star", (), |r| Ok(r.get::<i64>(0)?)))
            .unwrap(),
        1
    );
}

#[test]
fn closing_a_finished_staged_index_does_not_cancel_the_maintenance_operation() {
    let vault = Vault::open_temp().unwrap();
    let cancel = vault_store::CancelToken::new();
    vault.search().rebuild_with_control(cancel.clone(), |_| {}).unwrap();
    vault.close().unwrap();
    cancel.check().unwrap();
}

#[test]
fn wipe_removes_native_databases_index_and_envelope_without_replacing_owner_lock() {
    let (_root, vault, _) = encrypted();
    let owner = vault.store().owner().unwrap();
    let paths = vault.paths().clone();
    crate::wipe_all_data(
        &paths,
        &crate::archive::WipeOutsideState::default(),
        || Ok(vault.close()?),
        |_| Ok(()),
        |_| Ok(()),
    )
    .unwrap();
    for db in vault_store::Db::ALL {
        assert!(!vault_store::Paths::under(&paths.app_root).get(db).exists());
    }
    assert!(!paths.derived_dir.join("search-index").exists());
    assert!(!paths.app_root.join("archive/keys.envelope").exists());
    assert!(paths.app_root.join("archive/.pk-owner.lock").exists());
    assert!(vault_store::OwnerLock::acquire(&paths.app_root).is_err());
    drop(owner);
}

#[test]
fn startup_rolls_back_a_mode_switch_interrupted_after_the_first_install() {
    let (_root, vault, _key) = encrypted();
    let paths = vault.paths().clone();
    let config = vault.config().clone();
    let original = std::fs::read(paths.app_root.join("archive/keys.envelope")).unwrap();
    let fault = crate::fault_inject::FaultGuard::panic_at("native-install-installed");
    let crashed = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        switch_mode(
            &vault,
            Some("old password"),
            ArchiveMode::Plaintext,
            None,
            Default::default(),
            |_| {},
        )
        .unwrap();
    }));
    drop(fault);
    assert!(crashed.is_err());
    assert!(crate::maintenance::install::pending(&paths));
    crate::recover_archive_on_launch(&paths, &config, None).unwrap();
    assert!(!crate::maintenance::install::pending(&paths));
    assert_eq!(std::fs::read(paths.app_root.join("archive/keys.envelope")).unwrap(), original);
    let (opened, _) = Vault::open_password(paths, config, "old password", None).unwrap();
    opened.close().unwrap();
}

#[test]
fn startup_rolls_back_interrupted_bundle_import_and_snapshot_restore() {
    for bundle in [false, true] {
        let (_root, vault, _key) = encrypted();
        let paths = vault.paths().clone();
        let config = vault.config().clone();
        let snapshot_path = paths.raw_snapshots_dir.join("maintenance/restore-test");
        let bundle_path = paths.app_root.join("portable.pathkeep");
        snapshot(&vault, &snapshot_path).unwrap();
        if bundle {
            crate::migration::native::export(
                &vault,
                &bundle_path,
                "export password",
                Default::default(),
                |_| {},
            )
            .unwrap();
        }
        vault.store().write(Db::Archive,Priority::Interactive,|s|{
            s.execute("INSERT INTO star(entity_kind,entity_key,starred_at) VALUES('url','https://example.com/','after')",())?;Ok(())
        }).unwrap();
        let fault = crate::fault_inject::FaultGuard::panic_at("native-install-installed");
        let crash = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            if bundle {
                crate::migration::native::import(
                    &paths,
                    Some(&vault),
                    None,
                    &bundle_path,
                    &crate::ApplyImportOptions {
                        confirm_overwrite: true,
                        source_archive_key: Some("export password".into()),
                    },
                    Default::default(),
                )
                .unwrap();
            } else {
                restore_snapshot(&vault, &snapshot_path, Some("old password"), Default::default())
                    .unwrap();
            }
        }));
        drop(fault);
        assert!(crash.is_err());
        let (recovered, _) = Vault::open_password(paths, config, "old password", None).unwrap();
        assert_eq!(
            recovered
                .store()
                .read(Db::Archive, |s| s
                    .query_one("SELECT count(*) FROM star", (), |r| Ok(r.get::<i64>(0)?)))
                .unwrap(),
            1
        );
    }
}

#[test]
fn startup_discards_unpublished_copy_files_but_keeps_conversion_resume_state() {
    let (_root, vault, _) = encrypted();
    let paths = vault.paths().clone();
    vault.close().unwrap();
    for name in ["maintenance-interrupted", "sqlcipher-conversion"] {
        std::fs::create_dir_all(paths.staging_dir.join(name)).unwrap();
        std::fs::write(paths.staging_dir.join(name).join("part.db"), b"unfinished").unwrap();
    }
    recover_native_maintenance(&paths).unwrap();
    assert!(!paths.staging_dir.join("maintenance-interrupted").exists());
    assert!(paths.staging_dir.join("sqlcipher-conversion/part.db").exists());
    assert!(Vault::unlock_data_key(&paths, "old password").is_ok());
}

#[test]
fn retention_keeps_a_native_safety_snapshot_and_updates_its_native_ledger() {
    let (_root, vault, _) = encrypted();
    for name in ["a", "b"] {
        let path = vault.paths().raw_snapshots_dir.join("maintenance").join(name);
        snapshot(&vault, &path).unwrap();
        record_restore(&vault, &path.display().to_string()).unwrap();
    }
    let result = prune(
        &vault,
        &crate::RetentionPruneRequest { bucket_ids: vec!["snapshots".into()] },
        &Default::default(),
    )
    .unwrap();
    assert!(result.deleted_files > 0);
    let rows = vault
        .store()
        .read(Db::Archive, |s| {
            s.query("SELECT file_path FROM snapshots", (), |r| Ok(r.get::<String>(0)?))
        })
        .unwrap();
    assert_eq!(rows.len(), 1);
    assert!(std::path::Path::new(&rows[0]).join("snapshot.json").exists());
    assert_eq!(
        vault
            .store()
            .read(Db::Archive, |s| s.query_one(
                "SELECT value FROM archive_totals WHERE name='snapshots'",
                (),
                |r| Ok(r.get::<i64>(0)?)
            ))
            .unwrap(),
        1
    );
}

#[test]
fn incomplete_snapshot_manifest_cannot_create_empty_replacement_databases() {
    let vault = Vault::open_temp().unwrap();
    let root = tempfile::tempdir().unwrap();
    snapshot(&vault, root.path()).unwrap();
    let manifest = root.path().join("snapshot.json");
    let mut proof: Vec<serde_json::Value> =
        serde_json::from_slice(&fs::read(&manifest).unwrap()).unwrap();
    proof.retain(|entry| entry["path"] != "archive/source-evidence.db");
    fs::write(manifest, serde_json::to_vec(&proof).unwrap()).unwrap();
    fs::remove_file(root.path().join("archive/source-evidence.db")).unwrap();
    assert!(restore_snapshot(&vault, root.path(), None, Default::default()).is_err());
    assert!(vault.store().is_open());
}
