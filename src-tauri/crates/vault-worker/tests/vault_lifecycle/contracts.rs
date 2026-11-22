//! Responsibilities: pin current-tree paths and on-disk migration ledgers across owner reopen.
//! Includes the released 0.5.0 move file and its originless safety copy.

use super::support::*;
use vault_store::Db;

#[test]
fn encrypted_files_use_the_literal_historical_key_purpose_at_each_path() {
    use vault_crypto::KeyPurpose;
    use vault_store::{Config, Migration, MigrationRegistry, Paths, Store};

    let fixture = Fixture::new(true);
    fixture.seed();
    let actual = observe(&fixture.root, Some(&fixture.vault()), "key-mapping");
    assert_eq!(actual["unknown"], serde_json::json!([]));
    assert_eq!(actual, expected("backup-true"));
    let key = vault_core::Vault::unlock_data_key(
        &vault_core::project_paths_with_root(&fixture.root),
        PASSWORD,
    )
    .unwrap();
    fixture.session.close_vault().unwrap();

    // Independently decode the physical files the worker wrote. Opening them only through
    // Vault again would miss a refactor that swapped both its write and read key assignments.
    let decoded = fixture.sandbox.path().join("independent-key-reader");
    for path in [
        "archive/history-vault.db",
        "archive/source-evidence.db",
        "derived/intelligence.db",
        "derived/agent.db",
        "archive/agent-history.db",
        "derived/agent-history-read-model.db",
    ] {
        let target = decoded.join(path);
        std::fs::create_dir_all(target.parent().unwrap()).unwrap();
        std::fs::copy(fixture.root.join(path), target).unwrap();
    }
    let mut config = Config::new(&decoded);
    config.paths = Paths {
        archive: decoded.join("archive/history-vault.db"),
        source_evidence: decoded.join("archive/source-evidence.db"),
        intelligence: decoded.join("derived/intelligence.db"),
        agent: decoded.join("derived/agent.db"),
        scratch: decoded.join("derived/scratch.db"),
        agent_history_evidence: decoded.join("archive/agent-history.db"),
        agent_history_read_model: decoded.join("derived/agent-history-read-model.db"),
    };
    config.databases = vec![
        Db::Archive,
        Db::SourceEvidence,
        Db::Intelligence,
        Db::Agent,
        Db::Scratch,
        Db::AgentHistoryEvidence,
        Db::AgentHistoryReadModel,
    ];
    config.keys = [
        Some(*key.derive(KeyPurpose::ArchiveDatabase).expose_bytes()),
        Some(*key.derive(KeyPurpose::SourceEvidenceDatabase).expose_bytes()),
        Some(*key.derive(KeyPurpose::IntelligenceDatabase).expose_bytes()),
        Some(*key.derive(KeyPurpose::AgentDatabase).expose_bytes()),
        Some(*key.derive(KeyPurpose::ScratchDatabase).expose_bytes()),
        Some(*key.derive(KeyPurpose::AgentHistoryEvidenceDatabase).expose_bytes()),
        Some(*key.derive(KeyPurpose::AgentHistoryReadModelDatabase).expose_bytes()),
    ];
    // Only the reader ceilings come from production. The literal paths, key purposes and
    // marker reads independently verify which physical file was encrypted with each key.
    let mut migrations = MigrationRegistry::default();
    for &db in &config.databases {
        let ceiling = vault_core::store_schema::schema_version(db);
        if ceiling > 0 {
            migrations.register(
                db,
                vec![Migration::rust(ceiling, |_| {
                    panic!("a current copy should already contain every migration")
                })],
            );
        }
    }
    config.migrations = migrations;
    let reader = Store::open(config).unwrap();
    let read = |db| {
        reader
            .read(db, |s| {
                s.query_one("SELECT body FROM lifecycle_marker", (), |r| Ok(r.get::<String>(0)?))
            })
            .unwrap()
    };
    assert_eq!(read(Db::Archive), "pkarchiveamber");
    assert_eq!(read(Db::SourceEvidence), "pkevidencebirch");
    assert_eq!(read(Db::Intelligence), "pkintelligencecoral");
    assert_eq!(read(Db::Agent), "pkagentdenim");
    assert_eq!(read(Db::AgentHistoryEvidence), "pkagenthistoryivory");
    assert_eq!(read(Db::AgentHistoryReadModel), "pkreadmodeljade");
    reader.close().unwrap();
}

#[test]
fn the_five_historical_database_paths_have_identical_bytes() {
    let paths = vault_store::Paths::under(std::path::Path::new(""));
    assert_eq!(paths.get(Db::Archive).as_os_str().as_encoded_bytes(), b"archive/history-vault.db");
    assert_eq!(
        paths.get(Db::SourceEvidence).as_os_str().as_encoded_bytes(),
        b"archive/source-evidence.db"
    );
    assert_eq!(
        paths.get(Db::Intelligence).as_os_str().as_encoded_bytes(),
        b"derived/intelligence.db"
    );
    assert_eq!(paths.get(Db::Agent).as_os_str().as_encoded_bytes(), b"derived/agent.db");
    assert_eq!(paths.get(Db::Scratch).as_os_str().as_encoded_bytes(), b"derived/scratch.db");
}

#[test]
fn current_tree_migration_ledgers_and_data_survive_owner_reopen() {
    for encrypted in [false, true] {
        let fixture = Fixture::new(encrypted);
        fixture.seed();
        let before = ledgers(&fixture.vault(), &fixture.sandbox.path().join("ledger-before"));
        let steps = |db| (1..=vault_core::store_schema::schema_version(db)).collect::<Vec<_>>();
        let expected_ledger = serde_json::json!({
            "archive/history-vault.db":steps(Db::Archive),
            "archive/source-evidence.db":steps(Db::SourceEvidence),
            "derived/intelligence.db":steps(Db::Intelligence),
            "derived/agent.db":steps(Db::Agent), "derived/scratch.db":[],
            "archive/agent-history.db": steps(Db::AgentHistoryEvidence),
            "derived/agent-history-read-model.db": steps(Db::AgentHistoryReadModel),
        });
        assert_eq!(before, expected_ledger);
        let before_disk =
            observe(&fixture.root, Some(&fixture.vault()), &format!("reopen-before-{encrypted}"));
        assert_eq!(before_disk["unknown"], serde_json::json!([]));
        assert_eq!(before_disk, expected(&format!("backup-{encrypted}")));
        fixture.session.close_vault().unwrap();
        let reopened = fixture.reopen(encrypted);
        assert_eq!(
            ledgers(&reopened, &fixture.sandbox.path().join("ledger-after")),
            expected_ledger
        );
        assert_eq!(store_marker(&reopened, Db::Archive).as_deref(), Some("pkarchiveamber"));
        assert_eq!(store_marker(&reopened, Db::SourceEvidence).as_deref(), Some("pkevidencebirch"));
        assert_eq!(
            store_marker(&reopened, Db::Intelligence).as_deref(),
            Some("pkintelligencecoral")
        );
        assert_eq!(store_marker(&reopened, Db::Agent).as_deref(), Some("pkagentdenim"));
        assert_eq!(
            store_marker(&reopened, Db::AgentHistoryEvidence).as_deref(),
            Some("pkagenthistoryivory")
        );
        assert_eq!(
            store_marker(&reopened, Db::AgentHistoryReadModel).as_deref(),
            Some("pkreadmodeljade")
        );
        let after = observe(&fixture.root, Some(&reopened), &format!("reopen-after-{encrypted}"));
        assert_eq!(after["unknown"], serde_json::json!([]));
        assert_eq!(after, expected(&format!("backup-{encrypted}")));
        assert_eq!(after, before_disk);
    }
}

fn ledgers(vault: &vault_core::Vault, decoded: &std::path::Path) -> serde_json::Value {
    let mut ledgers = serde_json::Map::new();
    // The public Store SQL gate refuses storage metadata reads. Its verified native copy
    // decodes each specified physical database to a private plaintext inspection file;
    // rusqlite then reads the full ledger independently, not the migration registry/max.
    std::fs::create_dir_all(decoded).unwrap();
    let pause = vault.store().pause().unwrap();
    for (db, path) in [
        (Db::Archive, "archive/history-vault.db"),
        (Db::SourceEvidence, "archive/source-evidence.db"),
        (Db::Intelligence, "derived/intelligence.db"),
        (Db::Agent, "derived/agent.db"),
        (Db::AgentHistoryEvidence, "archive/agent-history.db"),
        (Db::AgentHistoryReadModel, "derived/agent-history-read-model.db"),
    ] {
        let destination = decoded.join(path);
        std::fs::create_dir_all(destination.parent().unwrap()).unwrap();
        pause.copy_database(db, destination.clone(), None, Default::default(), |_| {}).unwrap();
        let reader = rusqlite::Connection::open_with_flags(
            destination,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
        )
        .unwrap();
        let rows: Vec<i64> = reader
            .prepare("SELECT version FROM __pk_migrations ORDER BY version")
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        ledgers.insert(path.into(), serde_json::json!(rows));
    }
    drop(pause);
    let scratch_tables = vault
        .store()
        .read(Db::Scratch, |s| {
            s.query("SELECT name FROM sqlite_schema WHERE type='table'", (), |r| {
                Ok(r.get::<String>(0)?)
            })
        })
        .unwrap();
    assert!(!scratch_tables.iter().any(|name| name == "__pk_migrations"));
    ledgers.insert("derived/scratch.db".into(), serde_json::json!([]));
    serde_json::Value::Object(ledgers)
}

#[test]
fn released_data_folder_opens_at_the_released_schema_versions_without_an_update() {
    let f = Fixture::new(false);
    let root = f.sandbox.path().join("released-folder");
    let fixtures = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../tests/fixtures/v0.5-archive");
    let facts: serde_json::Value =
        serde_json::from_slice(&std::fs::read(fixtures.join("expected.json")).unwrap()).unwrap();
    let file = std::fs::File::open(fixtures.join("data.tar.gz")).unwrap();
    tar::Archive::new(flate2::read::GzDecoder::new(file)).unpack(&root).unwrap();
    unsafe {
        std::env::set_var("CHB_PROJECT_ROOT", &root);
    }
    let password = facts["password"].as_str().unwrap();
    let assessment = vault_worker::assess_archive_upgrade(Some(password)).unwrap();
    assert!(!assessment.pending && assessment.phases.is_empty());
    let paths = vault_core::project_paths_with_root(&root);
    let config = vault_core::load_config(&paths).unwrap();
    // The debug observer writes a PID marker whenever a live open announces a database update.
    // The hold file itself is absent, so a regression fails the assertion without blocking.
    let hold = f.sandbox.path().join("updating");
    let previous = std::env::var_os("PATHKEEP_TEST_HOLD_SCHEMA_UPDATE");
    unsafe {
        std::env::set_var("PATHKEEP_TEST_HOLD_SCHEMA_UPDATE", &hold);
    }
    let opened = vault_core::Vault::open_password(paths, config, password, None);
    unsafe {
        match previous {
            Some(v) => std::env::set_var("PATHKEEP_TEST_HOLD_SCHEMA_UPDATE", v),
            None => std::env::remove_var("PATHKEEP_TEST_HOLD_SCHEMA_UPDATE"),
        }
    }
    let (vault, _) = opened.unwrap();
    assert!(!hold.with_extension("held").exists(), "a released database announced a schema update");
    let stored = ledgers(&vault, &f.sandbox.path().join("released-roles"));
    // These are the published v0.5.0 ceilings, independent of the current registry.
    for (role, version) in [
        (Db::Archive, 23),
        (Db::Intelligence, 2),
        (Db::SourceEvidence, 1),
        (Db::Agent, 1),
        (Db::Scratch, 0),
    ] {
        assert_eq!(vault_core::store_schema::schema_version(role), version);
        assert_eq!(
            stored[vault_layout::database_path(role)],
            serde_json::json!((1..=version).collect::<Vec<_>>())
        );
    }
    vault.close().unwrap();
}

/// Use the release's actual encrypted move bundle and its sealed safety copy.
#[test]
fn released_schema_23_gets_an_origin_only_when_installed() {
    use serde_json::{Value, from_value, json};
    use vault_worker::data_kinds::backups;
    let f = Fixture::new(false);
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../tests/fixtures/v0.5-archive");
    let facts: Value =
        serde_json::from_slice(&std::fs::read(dir.join("expected.json")).unwrap()).unwrap();
    let bundle = dir.join("move-file.pathkeep");
    let current = f.vault();
    let preview = vault_worker::preview_import(current.paths(), bundle.clone()).unwrap();
    assert_eq!(preview.manifest.archive_schema_version, 23);
    let current_version = vault_core::store_schema::schema_version(Db::Archive);
    assert!(preview.schema_up_to_date, "the released move file must show no update note");
    assert!(preview.migrations_to_apply.is_empty());
    let lease = f.session.begin_maintenance().unwrap();
    let password = facts["exportPassword"].as_str().unwrap();
    let (_, installed) = vault_worker::apply_import(
        current.paths(),
        Some(&current),
        f.session.owner(),
        bundle,
        vault_core::ApplyImportOptions {
            confirm_overwrite: true,
            source_archive_key: Some(password.into()),
        },
    )
    .unwrap();
    f.session
        .install_archive(
            installed,
            Some(password.into()),
            vault_worker::KeychainAfterInstall::AsSaved,
        )
        .unwrap();
    drop(lease);
    let list = || {
        backups::list_runs(
            &f.session,
            from_value(json!({"space":"browsing","includeArchiveWide":true,"limit":10})).unwrap(),
        )
        .unwrap()
    };
    let detail = |reference: &Value| {
        backups::get_run(&f.session, from_value(json!({"runRef":reference})).unwrap())
    };
    let first = list()["items"][0]["runRef"].clone();
    assert!(first.as_str().unwrap().starts_with("browsing:"));
    assert_eq!(detail(&first).unwrap()["runRef"], first);
    assert_eq!(
        ledgers(&f.vault(), &f.sandbox.path().join("released-ledger"))["archive/history-vault.db"]
            .as_array()
            .unwrap()
            .last(),
        Some(&json!(current_version))
    );
    let copies = vault_worker::list_recovery_snapshots().unwrap();
    let copy = copies
        .iter()
        .find(|c| {
            std::path::Path::new(&c.path).ends_with(facts["safetyCopy"]["name"].as_str().unwrap())
        })
        .unwrap();
    vault_worker::run_full_archive_restore(
        &f.session,
        Some(facts["password"].as_str().unwrap()),
        &vault_core::SnapshotRestoreRequest { snapshot_path: copy.path.clone() },
    )
    .unwrap();
    let restored = list()["items"][0]["runRef"].clone();
    assert_eq!(detail(&restored).unwrap()["runRef"], restored);
    assert_eq!(detail(&first).unwrap_err().to_string(), "record-unavailable");
}

#[test]
fn restored_v06_ledger_cannot_reuse_a_qualified_run_link() {
    use serde_json::{from_value, json};
    use vault_worker::data_kinds::backups;
    let f = Fixture::new(false);
    let snapshot = f.root.join("raw-snapshots/maintenance/qualified-origin");
    vault_core::maintenance::snapshot(&f.vault(), &snapshot).unwrap();
    f.create_browser();
    f.backup();
    let list = || {
        backups::list_runs(
            &f.session,
            from_value(json!({"space":"browsing","includeArchiveWide":true,"limit":10})).unwrap(),
        )
        .unwrap()
    };
    let old = list()["items"][0]["runRef"].clone();
    assert!(old.as_str().unwrap().starts_with("browsing:"));
    vault_worker::run_full_archive_restore(
        &f.session,
        None,
        &vault_core::SnapshotRestoreRequest { snapshot_path: snapshot.to_string_lossy().into() },
    )
    .unwrap();
    let restored = list()["items"][0].clone();
    assert_eq!(restored["operation"], "archive_restore");
    assert_eq!(
        restored["runRef"].as_str().unwrap().split('/').next_back(),
        old.as_str().unwrap().split('/').next_back()
    );
    assert_eq!(
        backups::get_run(&f.session, from_value(json!({"runRef":old})).unwrap())
            .unwrap_err()
            .to_string(),
        "record-unavailable"
    );
}

#[test]
fn a_run_dismissal_follows_its_ledger_through_reopen_restore_and_move() {
    use serde_json::{from_value, json};
    use vault_worker::data_kinds::backups;
    let f = Fixture::new(false);
    f.vault().store().write(Db::Archive, vault_store::Priority::Interactive, |s| {
        for id in [971, 972] {
            s.execute(
                "INSERT INTO runs(id,run_type,trigger,started_at,finished_at,status)
                 VALUES(?1,'backup','manual','2026-11-09T10:00:00Z','2026-11-09T10:01:00Z','partial')",
                [id],
            )?;
        }
        Ok(())
    }).unwrap();
    let seed_queue_job = || {
        f.vault().store().write(Db::Intelligence, vault_store::Priority::Interactive, |s| {
            s.execute(
                "INSERT INTO ai_jobs(id,job_type,state,priority,attempt,payload_json,available_at,created_at,updated_at)
                 VALUES(973,'index-build','failed',100,1,'{}','2026-11-09','2026-11-09','2026-11-10')",
                (),
            )?;
            Ok(())
        }).unwrap();
    };
    seed_queue_job();
    let dismiss = |task: &str| {
        backups::control(
            &f.session,
            from_value(json!({"taskId":task,"action":"dismiss"})).unwrap(),
            std::sync::Arc::new(|_, _| {}),
        )
        .unwrap();
    };
    let assert_saved = |queue_needs_attention: bool| {
        let rows = backups::tasks(
            &f.session,
            from_value(json!({"kind":"browsing","group":"attention","limit":100})).unwrap(),
        )
        .unwrap();
        let rows = rows["items"].as_array().unwrap();
        assert_eq!(rows.iter().any(|r| r["taskId"] == "browsing:index/973"), queue_needs_attention);
        assert!(!rows.iter().any(|r| r["taskId"] == "browsing:run/971"), "{rows:?}");
        assert!(rows.iter().any(|r| r["taskId"] == "browsing:run/972"), "{rows:?}");
        let count = f
            .vault()
            .store()
            .read(Db::Archive, |s| {
                s.query_one(
                    "SELECT count(*) FROM runs WHERE id IN (971,972) AND status='partial'",
                    (),
                    |r| Ok(r.get::<i64>(0)?),
                )
            })
            .unwrap();
        assert_eq!(count, 2);
    };
    dismiss("browsing:run/971");
    dismiss("browsing:index/973");
    f.session.close_vault().unwrap();
    f.reopen(false);
    assert_saved(false);
    let snapshot = f.root.join("raw-snapshots/maintenance/manual-dismissal");
    vault_core::maintenance::snapshot(&f.vault(), &snapshot).unwrap();
    dismiss("browsing:run/972");
    let restore = vault_core::SnapshotRestoreRequest {
        snapshot_path: snapshot.to_string_lossy().into_owned(),
    };
    vault_worker::run_full_archive_restore(&f.session, None, &restore).unwrap();
    assert_saved(false);
    let bundle = f.sandbox.path().join("acknowledged.pathkeep");
    vault_worker::export_app_data(&f.vault(), bundle.clone(), EXPORT_PASSWORD).unwrap();
    dismiss("browsing:run/972");
    let live = f.vault();
    let lease = f.session.begin_maintenance().unwrap();
    let (_, installed) = vault_worker::apply_import(
        live.paths(),
        Some(&live),
        f.session.owner(),
        bundle,
        vault_core::ApplyImportOptions {
            confirm_overwrite: true,
            source_archive_key: Some(EXPORT_PASSWORD.into()),
        },
    )
    .unwrap();
    f.session
        .install_archive(
            installed,
            Some(EXPORT_PASSWORD.into()),
            vault_worker::KeychainAfterInstall::AsSaved,
        )
        .unwrap();
    drop(lease);
    seed_queue_job();
    assert_saved(true);
}
