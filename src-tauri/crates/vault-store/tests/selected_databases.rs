//! Responsibilities: prove a partial store owns only the selected real databases.
//! Not responsible for: application schemas or pretending file IO succeeded.
//! Failure modes: opening or snapshotting touches disabled files; missing dependency
//! stores fail after mutation; caller order breaks attachments; maintenance panics on
//! an absent worker. These tests are written before the selectable-store implementation.

#[path = "support/observed_io.rs"]
mod observed_io;

use std::{fs, path::Path, sync::Arc};
use vault_store::{CancelToken, Config, Db, Error, Migration, Priority, Store};

fn selected_config(root: &Path) -> Config {
    let mut config = Config::new(root);
    config.databases = vec![Db::SourceEvidence, Db::Archive, Db::Scratch];
    config.keys = [Some([61; 32]); vault_store::Db::COUNT];
    config.migrations.register(
        Db::Archive,
        vec![Migration::sql(1, "CREATE TABLE sessions(id INTEGER PRIMARY KEY, title TEXT)")],
    );
    config.migrations.register(
        Db::SourceEvidence,
        vec![Migration::sql(1, "CREATE TABLE captures(id INTEGER PRIMARY KEY, body BLOB)")],
    );
    config
}

#[test]
fn captures_and_snapshots_preserve_selected_data_without_touching_disabled_files() {
    let root = tempfile::tempdir().unwrap();
    let target = tempfile::tempdir().unwrap();
    let mut config = selected_config(root.path());
    let disabled = [config.paths.intelligence.clone(), config.paths.agent.clone()];
    for path in &disabled {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, b"unrelated files must not be opened as databases").unwrap();
    }
    let io = Arc::new(observed_io::ObservedIo::new());
    config.io = Some(io.clone());
    let store = Store::open(config).unwrap();
    store
        .write(Db::Archive, Priority::Batch, |tx| {
            tx.execute("INSERT INTO sessions VALUES(1,'retained conversation')", ())?;
            Ok(())
        })
        .unwrap();
    store
        .write(Db::SourceEvidence, Priority::Batch, |tx| {
            tx.execute("INSERT INTO captures VALUES(1,X'0001FF')", ())?;
            Ok(())
        })
        .unwrap();
    store.pause().unwrap().snapshot(target.path()).unwrap();
    store.close().unwrap();

    let restored = Store::open(selected_config(target.path())).unwrap();
    assert_eq!(
        restored
            .read(Db::Archive, |s| s.query_one(
                "SELECT title FROM sessions WHERE id=1",
                (),
                |r| Ok(r.get::<String>(0)?)
            ))
            .unwrap(),
        "retained conversation"
    );
    assert_eq!(
        restored
            .read(Db::SourceEvidence, |s| s.query_one(
                "SELECT body FROM captures WHERE id=1",
                (),
                |r| Ok(r.get::<Vec<u8>>(0)?)
            ))
            .unwrap(),
        [0, 1, 255]
    );
    restored.close().unwrap();
    for path in &disabled {
        assert_eq!(fs::read(path).unwrap(), b"unrelated files must not be opened as databases");
        assert!(!io.opened.lock().unwrap().iter().any(|opened| Path::new(opened) == path));
        assert!(!target.path().join(path.strip_prefix(root.path()).unwrap()).exists());
    }
}

#[test]
fn selection_order_does_not_break_archive_or_scratch_attachments() {
    let root = tempfile::tempdir().unwrap();
    let mut config = selected_config(root.path());
    config.databases.insert(0, Db::Intelligence);
    let store = Store::open(config).unwrap();
    store
        .write(Db::Archive, Priority::Batch, |tx| {
            tx.execute("INSERT INTO sessions VALUES(9,'shared archive')", ())?;
            Ok(())
        })
        .unwrap();
    store
        .write(Db::Scratch, Priority::Batch, |tx| {
            tx.execute("CREATE TABLE marker(id INTEGER)", ())?;
            tx.execute("INSERT INTO marker VALUES(27)", ())?;
            Ok(())
        })
        .unwrap();
    assert_eq!(
        store
            .read(Db::Intelligence, |s| s
                .query_one("SELECT id FROM archive.sessions", (), |r| Ok(r.get::<i64>(0)?)))
            .unwrap(),
        9
    );
    assert_eq!(
        store
            .read(Db::SourceEvidence, |s| s
                .query_one("SELECT id FROM scratch.marker", (), |r| Ok(r.get::<i64>(0)?)))
            .unwrap(),
        27
    );
}

#[test]
fn invalid_selections_are_rejected_before_creating_the_store_root() {
    for databases in [
        vec![],
        vec![Db::Scratch, Db::Archive, Db::Archive],
        vec![Db::Archive],
        vec![Db::Scratch, Db::Intelligence],
    ] {
        let parent = tempfile::tempdir().unwrap();
        let root = parent.path().join("not-created");
        let mut config = selected_config(&root);
        config.databases = databases.clone();
        assert!(
            matches!(Store::open(config), Err(Error::InvalidConfig(_))),
            "invalid selection was accepted: {databases:?}"
        );
        assert!(!root.exists(), "invalid selection mutated the filesystem: {databases:?}");
    }
}

#[test]
fn unused_path_settings_do_not_alias_or_open_another_selected_database() {
    let root = tempfile::tempdir().unwrap();
    let mut config = selected_config(root.path());
    config.paths.agent = config.paths.archive.clone();
    config.paths.intelligence = root.path().join("must-not-be-created/intelligence.db");
    let store = Store::open(config).unwrap();
    store
        .write(Db::Archive, Priority::Batch, |tx| {
            tx.execute("INSERT INTO sessions VALUES(1,'one writer')", ())?;
            Ok(())
        })
        .unwrap();
    assert!(!root.path().join("must-not-be-created").exists());
}

#[test]
fn disabled_database_operations_fail_without_creating_a_copy_or_panicking() {
    let root = tempfile::tempdir().unwrap();
    let destination = root.path().join("must-not-be-created/copy.db");
    let store = Store::open(selected_config(root.path())).unwrap();
    assert!(matches!(store.read(Db::Agent, |_| Ok(())), Err(Error::Closed)));
    assert!(matches!(
        store.write(Db::Agent, Priority::Interactive, |_| Ok(())),
        Err(Error::Closed)
    ));
    let pause = store.pause().unwrap();
    assert!(matches!(
        pause.copy_database(Db::Agent, destination.clone(), None, CancelToken::new(), |_| {}),
        Err(Error::Closed)
    ));
    pause.close().unwrap();
    assert!(!destination.parent().unwrap().exists());
    assert!(!store.is_open());
}
