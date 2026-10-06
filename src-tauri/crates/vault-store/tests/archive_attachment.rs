//! Responsibilities: verify intelligence joins read the canonical owner safely.
//! Not responsible for: intelligence algorithms or archive writes.
//! Failure modes: stale archive reads, wrong attachment keys, cross-database writes,
//! or an intelligence query prevents an archive transaction from committing.
use vault_store::{Config, Db, Migration, Priority, Store};

#[test]
fn intelligence_reads_committed_archive_facts_in_both_key_modes() {
    for key in [None, Some([64; 32])] {
        let root = tempfile::tempdir().unwrap();
        let mut config = Config::new(root.path());
        config.keys = [key; 5];
        config.migrations.register(
            Db::Archive,
            vec![Migration::sql(1, "CREATE TABLE facts(id INTEGER PRIMARY KEY)")],
        );
        let store = Store::open(config).unwrap();
        for id in 1..=3 {
            store
                .write(Db::Archive, Priority::Batch, move |s| {
                    s.execute("INSERT INTO facts VALUES(?1)", [id])?;
                    Ok(())
                })
                .unwrap();
            assert_eq!(
                store
                    .read(Db::Intelligence, |s| s.query_one(
                        "SELECT COUNT(*) FROM archive.facts",
                        (),
                        |r| Ok(r.get::<i64>(0)?)
                    ))
                    .unwrap(),
                id
            );
        }
        assert!(
            store
                .write(Db::Intelligence, Priority::Batch, |s| {
                    s.execute("DELETE FROM archive.facts", ())?;
                    Ok(())
                })
                .is_err()
        );
    }
}
