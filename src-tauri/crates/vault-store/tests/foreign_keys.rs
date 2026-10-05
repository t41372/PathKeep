//! Responsibilities: prove every database rejects orphan writes on normal connections.
//! Not responsible for: application schemas or cross-database references.
//! Failure modes: the engine's default disables FKs; one database misses setup;
//! failed writes persist; cascading deletes leave child rows behind.
use vault_store::{Config, Db, Error, Migration, Priority, Store};

#[test]
fn foreign_key_violations_fail_on_every_database() {
    for key in [None, Some([0x5a; 32])] {
        let root = tempfile::tempdir().unwrap();
        let mut config = Config::new(root.path());
        config.key = key;
        for db in Db::ALL {
            config.migrations.register(db, vec![Migration::rust(1, |tx| {
                tx.execute("CREATE TABLE parent(id INTEGER PRIMARY KEY)", ())?;
                tx.execute("CREATE TABLE child(id INTEGER PRIMARY KEY, parent_id INTEGER NOT NULL REFERENCES parent(id) ON DELETE CASCADE)", ())?;
                Ok(())
            })]);
        }
        let store = Store::open(config).unwrap();
        for db in Db::ALL {
            let result = store.write(db, Priority::Interactive, |tx| {
                tx.execute("INSERT INTO child VALUES(1,99)", ())?;
                Ok(())
            });
            assert!(
                matches!(result, Err(Error::Engine(turso::Error::Constraint(_)))),
                "{db:?}: {result:?}"
            );
            store
                .write(db, Priority::Interactive, |tx| {
                    tx.execute("INSERT INTO parent VALUES(1)", ())?;
                    tx.execute("INSERT INTO child VALUES(1,1)", ())?;
                    tx.execute("DELETE FROM parent WHERE id=1", ())?;
                    Ok(())
                })
                .unwrap();
            let children = store
                .read(db, |tx| {
                    tx.query_one("SELECT count(*) FROM child", (), |r| Ok(r.get::<i64>(0)?))
                })
                .unwrap();
            assert_eq!(children, 0, "{db:?}");
        }
    }
}
