//! Responsibilities: verify prepared operations stay inside a Store transaction.
//! Not responsible for: business schemas or queue scheduling.
//! Failure modes: cursors escape threads, reused statements retain bindings,
//! optional reads hide errors, reads mutate, or prepared SQL bypasses policy; an insert that
//! inserted nothing reports the previous row's id.

mod support;
use vault_store::{Db, OptionalRow, Priority, params_owned};

#[test]
fn prepared_statements_stream_and_rebind_in_both_key_modes() {
    for key in [None, Some([61; 32])] {
        let root = tempfile::tempdir().unwrap();
        let mut config = support::config(root.path());
        config.keys = [key; 5];
        let store = vault_store::Store::open(config).unwrap();
        store
            .write(Db::Archive, Priority::Batch, |s| {
                let mut insert = s.prepare("INSERT INTO visits(id,url) VALUES(?1,?2)")?;
                let url = String::from("https://fixture.test/");
                for id in 1..=101 {
                    let borrowed = &url;
                    insert.execute(params_owned![id, borrowed])?;
                }
                assert_eq!(url, "https://fixture.test/");
                Ok(())
            })
            .unwrap();
        store
            .read(Db::Archive, |s| {
                let mut query =
                    s.prepare("SELECT id FROM visits WHERE id>?1 ORDER BY id LIMIT 3")?;
                let first = query
                    .query_map([0], |r| Ok(r.get::<i64>(0)?))?
                    .collect::<vault_store::Result<Vec<_>>>()?;
                let last = query
                    .query_map([99], |r| Ok(r.get::<i64>(0)?))?
                    .collect::<vault_store::Result<Vec<_>>>()?;
                assert_eq!(first, [1, 2, 3]);
                assert_eq!(last, [100, 101]);
                assert!(
                    s.query_one("SELECT id FROM visits WHERE id=0", (), |r| Ok(r.get::<i64>(0)?))
                        .optional()?
                        .is_none()
                );
                assert!(
                    s.query_one("SELECT absent FROM visits", (), |r| Ok(r.get::<i64>(0)?))
                        .optional()
                        .is_err()
                );
                assert!(s.prepare("CREATE TEMP TABLE leak(id)").is_err());
                assert!(s.prepare("ATTACH ':memory:' AS leak").is_err());
                assert!(s.prepare("UPDATE visits SET url='changed'").is_err());
                Ok(())
            })
            .unwrap();
    }
}

#[test]
fn insert_returns_the_new_rowid_and_nothing_for_an_ignored_row() {
    for key in [None, Some([62; 32])] {
        let root = tempfile::tempdir().unwrap();
        let mut config = support::config(root.path());
        config.keys = [key; 5];
        let store = vault_store::Store::open(config).unwrap();
        store
            .write(Db::Archive, Priority::Batch, |s| {
                let insert = "INSERT OR IGNORE INTO visits(id,url) VALUES(?1,?2)";
                assert_eq!(s.insert(insert, params_owned![7_i64, "a"])?, Some(7));
                // Ignored: no id, not the one before it.
                assert_eq!(s.insert(insert, params_owned![7_i64, "again"])?, None);
                let next = s.insert("INSERT INTO visits(url) VALUES(?1)", ["b"])?;
                assert_eq!(next, Some(8));
                Ok(())
            })
            .unwrap();
    }
}
