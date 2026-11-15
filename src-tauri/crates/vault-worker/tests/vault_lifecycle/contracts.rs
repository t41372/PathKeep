//! Responsibilities: pin current-tree paths and on-disk migration ledgers across owner reopen.
//! Not responsible for: asserting release-generated v0.5.0 compatibility; that fixture is deferred.

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
        // Not opened below: the released layout has no agent-history stores.
        agent_history_evidence: decoded.join("archive/agent-history.db"),
        agent_history_read_model: decoded.join("derived/agent-history-read-model.db"),
    };
    config.databases =
        vec![Db::Archive, Db::SourceEvidence, Db::Intelligence, Db::Agent, Db::Scratch];
    config.keys = [
        Some(*key.derive(KeyPurpose::ArchiveDatabase).expose_bytes()),
        Some(*key.derive(KeyPurpose::SourceEvidenceDatabase).expose_bytes()),
        Some(*key.derive(KeyPurpose::IntelligenceDatabase).expose_bytes()),
        Some(*key.derive(KeyPurpose::AgentDatabase).expose_bytes()),
        Some(*key.derive(KeyPurpose::ScratchDatabase).expose_bytes()),
        None,
        None,
    ];
    // Existing copies must already have these versions. These literal reader ceilings run
    // no schema implementation and cannot manufacture a missing fixture marker/table.
    let mut migrations = MigrationRegistry::default();
    migrations.register(Db::Archive, vec![Migration::rust(23, |_| Ok(()))]);
    migrations.register(Db::SourceEvidence, vec![Migration::rust(1, |_| Ok(()))]);
    migrations.register(Db::Intelligence, vec![Migration::rust(2, |_| Ok(()))]);
    migrations.register(Db::Agent, vec![Migration::rust(1, |_| Ok(()))]);
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
        let expected_ledger = serde_json::json!({
            "archive/history-vault.db":[1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18,19,20,21,22,23],
            "archive/source-evidence.db":[1], "derived/intelligence.db":[1,2],
            "derived/agent.db":[1], "derived/scratch.db":[]
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
