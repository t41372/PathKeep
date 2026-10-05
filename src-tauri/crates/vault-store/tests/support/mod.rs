//! Responsibilities: isolated real-engine fixtures shared by integration tests.
//! Not responsible for: mocking storage or retaining disposable databases.
//! Failure modes: fixtures share paths or survive tests; TempDir isolates and removes them.
#![allow(dead_code)] // Each integration binary uses a different subset of these fixtures.
use std::path::Path;
use vault_store::{Config, Db, Migration, Priority, Result, Store};

pub fn config(root: &Path) -> Config {
    let mut config = Config::new(root);
    config.migrations.register(Db::Archive, vec![Migration::sql(1, "CREATE TABLE visits(id INTEGER PRIMARY KEY, url TEXT, title TEXT, visited_at INTEGER, starred INTEGER DEFAULT 0)")]);
    config
}
pub fn open() -> (tempfile::TempDir, Store) {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(config(dir.path())).unwrap();
    (dir, store)
}
pub fn count(store: &Store) -> i64 {
    store
        .read(Db::Archive, |conn| {
            conn.query_one("SELECT count(*) FROM visits", (), |row| Ok(row.get::<i64>(0)?))
        })
        .unwrap()
}
pub fn insert(store: &Store, id: i64) -> Result<()> {
    store.write(Db::Archive, Priority::Batch, move |tx| {
        tx.execute("INSERT INTO visits(id,url,title,visited_at) VALUES (?1,'https://example.test','page',?1)", [id])?;
        Ok(())
    })
}
