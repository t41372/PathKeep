//! Responsibilities: verify doctor can inspect real files without bypassing SQL policy.
//! Not responsible for: business consistency or repairs.
//! Failure modes: encrypted checks fail, schema metadata is wrong, inspection writes.
use vault_store::{Config, Db, Migration, Store};

#[test]
fn inspection_works_on_readers_in_both_storage_modes() {
    for key in [None, Some([74; 32])] {
        let root = tempfile::tempdir().unwrap();
        let mut config = Config::new(root.path());
        config.keys = [key; 5];
        config.migrations.register(
            Db::Archive,
            vec![Migration::sql(1, "CREATE TABLE facts(id INTEGER PRIMARY KEY)")],
        );
        let store = Store::open(config).unwrap();
        store
            .read(Db::Archive, |s| {
                assert_eq!(s.schema_version()?, 1);
                assert_eq!(s.integrity_check()?, vec!["ok"]);
                assert!(s.execute("PRAGMA user_version=7", ()).is_err());
                Ok(())
            })
            .unwrap();
    }
}
