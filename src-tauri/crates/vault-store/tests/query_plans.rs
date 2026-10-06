//! Responsibilities: verify plans use real SQL and bindings on the owner connection.
//! Not responsible for: application query policy.
//! Failure modes: capture executes different SQL, misses prepared reads, or remains enabled.
use vault_store::{Db, Priority};
#[test]
fn captures_direct_and_prepared_reads_only_when_enabled() {
    let root = tempfile::tempdir().unwrap();
    let store = vault_store::Store::open(vault_store::Config::new(root.path())).unwrap();
    store
        .write(Db::Intelligence, Priority::Interactive, |s| {
            s.execute("CREATE TABLE items(id INTEGER PRIMARY KEY,value TEXT)", ())?;
            s.execute("CREATE INDEX item_value ON items(value)", ())?;
            Ok(())
        })
        .unwrap();
    store.capture_query_plans(Db::Intelligence, true).unwrap();
    store
        .read(Db::Intelligence, |s| {
            s.query("SELECT id FROM items WHERE value=?1", ["sample"], |r| Ok(r.get::<i64>(0)?))?;
            s.prepare("SELECT id FROM items WHERE value=?1")?
                .query_map(["other"], |r| Ok(r.get::<i64>(0)?))?
                .collect::<vault_store::Result<Vec<_>>>()?;
            Ok(())
        })
        .unwrap();
    let plans = store.take_query_plans(Db::Intelligence).unwrap();
    assert_eq!(plans.len(), 2);
    assert!(plans.iter().all(|p| p.steps.iter().any(|s| s.contains("item_value"))));
    store.capture_query_plans(Db::Intelligence, false).unwrap();
    store
        .read(Db::Intelligence, |s| s.query_one("SELECT 1", (), |r| Ok(r.get::<i64>(0)?)))
        .unwrap();
    assert!(store.take_query_plans(Db::Intelligence).unwrap().is_empty());
}
