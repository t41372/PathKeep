//! Responsibilities: verify open preflight, configuration and scratch against real files.
//! Not responsible for: implementing encryption.
//! Failure modes: wrong keys change files; invalid later DB allows earlier migration;
//! custom IO is dropped; a non-ASCII path fails to open. Covered below.
#[path = "support/observed_io.rs"]
mod observed_io;
mod support;
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    sync::Arc,
};
use vault_store::{Config, Db, Error, Migration, Priority, Store};
fn snapshot(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    let mut files = BTreeMap::new();
    for entry in fs::read_dir(root).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            files.extend(snapshot(&path));
        } else {
            files.insert(path.clone(), fs::read(path).unwrap());
        }
    }
    files
}
#[test]
fn wrong_and_missing_keys_leave_every_file_unchanged() {
    let dir = tempfile::tempdir().unwrap();
    let mut config = support::config(dir.path());
    config.keys = [Some([23; 32]); 5];
    let store = Store::open(config).unwrap();
    support::insert(&store, 1).unwrap();
    drop(store);
    let before = snapshot(dir.path());
    for key in [Some([24; 32]), None] {
        let mut config = support::config(dir.path());
        config.keys = [key; 5];
        assert!(matches!(Store::open(config), Err(Error::WrongKey { .. })));
        assert_eq!(before, snapshot(dir.path()));
    }
    let mut config = support::config(dir.path());
    config.keys = [Some([23; 32]); 5];
    let store = Store::open(config).unwrap();
    assert_eq!(support::count(&store), 1);
}
#[test]
fn preflight_checks_later_databases_before_any_migration_or_scratch_cleanup() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(support::config(dir.path())).unwrap();
    store
        .write(Db::Scratch, Priority::Interactive, |tx| {
            tx.execute("CREATE TABLE keep_me(id INTEGER)", ())?;
            Ok(())
        })
        .unwrap();
    drop(store);
    fs::write(dir.path().join("derived/agent.db"), vec![0x55; 8192]).unwrap();
    let before = snapshot(dir.path());
    let mut config = support::config(dir.path());
    config.migrations.register(
        Db::Archive,
        vec![
            Migration::sql(1, "CREATE TABLE visits(id INTEGER)"),
            Migration::sql(2, "CREATE TABLE should_not_exist(id INTEGER)"),
        ],
    );
    assert!(matches!(Store::open(config), Err(Error::Corrupt { .. })));
    assert_eq!(before, snapshot(dir.path()));
}
#[test]
fn accepts_caller_io_and_non_ascii_uri_paths() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("資料 ' ? #");
    let mut config = Config::new(&root);
    config.keys = [Some([9; 32]); 5];
    let io = Arc::new(observed_io::ObservedIo::new());
    config.io = Some(io.clone());
    let paths = config.paths.clone();
    let store = Store::open(config).unwrap();
    for db in Db::ALL {
        assert!(
            io.opened.lock().unwrap().iter().any(|p| std::path::Path::new(p) == paths.get(db)),
            "IO not used for {db:?}"
        );
    }
    store
        .write(Db::Scratch, Priority::Batch, |tx| {
            tx.execute("CREATE TABLE marker(id INTEGER)", ())?;
            Ok(())
        })
        .unwrap();
    store
        .read(Db::Archive, |c| {
            c.query_one("SELECT count(*) FROM scratch.marker", (), |r| Ok(r.get::<i64>(0)?))
        })
        .unwrap();
}
