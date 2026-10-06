//! Responsibilities: exercise the fresh schemas through real vault-store transactions.
//! Not responsible for: mocking Turso, reusing migration SQL as expectations or query ports.
//! Failure modes: catalog omissions, lost field values, disabled FKs, split commits,
//! repeated startup migrations, accepted future versions and retained scratch tables.
mod agent;
mod archive;
mod intelligence;
mod search;
mod source_evidence;

use std::{num::NonZeroUsize, path::Path};
use vault_store::{Config, Db, Error, Migration, Priority, Session, Store};

struct Fixture {
    table: &'static str,
    columns: &'static str,
    values: &'static str,
    updated: &'static str,
}
struct ForeignKey {
    table: &'static str,
    column: &'static str,
    missing: &'static str,
}
struct Catalog {
    db: Db,
    tables: &'static [&'static str],
    indexes: &'static [&'static str],
    views: &'static [&'static str],
    rows: &'static [Fixture],
    foreign_keys: &'static [ForeignKey],
}
const CATALOGS: &[Catalog] = &[
    Catalog {
        db: Db::Archive,
        tables: archive::TABLES,
        indexes: archive::INDEXES,
        views: &[],
        rows: archive::ROWS,
        foreign_keys: archive::FOREIGN_KEYS,
    },
    Catalog {
        db: Db::SourceEvidence,
        tables: source_evidence::TABLES,
        indexes: source_evidence::INDEXES,
        views: &[],
        rows: source_evidence::ROWS,
        foreign_keys: source_evidence::FOREIGN_KEYS,
    },
    Catalog {
        db: Db::Intelligence,
        tables: intelligence::TABLES,
        indexes: intelligence::INDEXES,
        views: intelligence::VIEWS,
        rows: intelligence::ROWS,
        foreign_keys: intelligence::FOREIGN_KEYS,
    },
    Catalog {
        db: Db::Agent,
        tables: agent::TABLES,
        indexes: agent::INDEXES,
        views: &[],
        rows: agent::ROWS,
        foreign_keys: agent::FOREIGN_KEYS,
    },
    Catalog {
        db: Db::Scratch,
        tables: &[],
        indexes: &[],
        views: &[],
        rows: &[],
        foreign_keys: &[],
    },
];
const KEYS: [Option<[u8; 32]>; 2] = [None, Some([0x5a; 32])];

fn config(root: &Path, key: Option<[u8; 32]>) -> Config {
    let mut config = Config::new(root);
    config.keys = [key; 5];
    config.readers_per_database = NonZeroUsize::new(1).unwrap();
    config.migrations = super::migration_registry();
    config
}
fn each_store(mut test: impl FnMut(&Store)) {
    for key in KEYS {
        let root = tempfile::tempdir().unwrap();
        let store = Store::open(config(root.path(), key)).unwrap();
        test(&store);
    }
}
fn seed(tx: &Session<'_>, rows: &[Fixture]) -> vault_store::Result<()> {
    for row in rows {
        tx.execute(
            &format!("INSERT INTO {} ({}) VALUES ({})", row.table, row.columns, row.values),
            (),
        )?;
    }
    Ok(())
}
fn number(store: &Store, db: Db, sql: &'static str) -> i64 {
    store.read(db, move |tx| tx.query_one(sql, (), |r| Ok(r.get(0)?))).unwrap()
}
fn assert_catalog(store: &Store, catalog: &'static Catalog) {
    for (kind, expected) in
        [("table", catalog.tables), ("index", catalog.indexes), ("view", catalog.views)]
    {
        let actual = store
            .read(catalog.db, move |tx| {
                tx.query(
                    "SELECT name FROM sqlite_schema WHERE type=?1 ORDER BY name",
                    [kind],
                    |r| Ok(r.get::<String>(0)?),
                )
            })
            .unwrap();
        let mut expected = expected.to_vec();
        expected.sort_unstable();
        assert_eq!(actual, expected, "{:?} {kind}", catalog.db);
    }
    assert_eq!(
        number(
            store,
            catalog.db,
            "SELECT count(*) FROM sqlite_schema WHERE type='trigger' OR upper(sql) LIKE '%VIRTUAL TABLE%'"
        ),
        0
    );
}

#[test]
fn catalog_and_crud() {
    each_store(|store| {
        for catalog in CATALOGS {
            assert_catalog(store, catalog);
            store.write(catalog.db, Priority::Batch, move |tx| seed(tx, catalog.rows)).unwrap();
            // Every persisted field is supplied and read, including optional payloads.
            store
                .read(catalog.db, move |tx| {
                    for row in catalog.rows {
                        let actual = tx.query_one(
                            &format!("SELECT {} FROM {} WHERE rowid=1", row.columns, row.table),
                            (),
                            |r| {
                                (0..r.column_count())
                                    .map(|i| Ok(r.get_value(i)?))
                                    .collect::<vault_store::Result<Vec<_>>>()
                            },
                        )?;
                        let expected =
                            tx.query_one(&format!("SELECT {}", row.values), (), |r| {
                                (0..r.column_count())
                                    .map(|i| Ok(r.get_value(i)?))
                                    .collect::<vault_store::Result<Vec<_>>>()
                            })?;
                        assert_eq!(actual, expected, "{} round trip", row.table);
                    }
                    Ok(())
                })
                .unwrap();
            store
                .write(catalog.db, Priority::Interactive, move |tx| {
                    for row in catalog.rows {
                        tx.execute(
                            &format!("UPDATE {} SET {} WHERE rowid=1", row.table, row.updated),
                            (),
                        )?;
                        assert_eq!(
                            tx.query_one(
                                &format!("SELECT {} FROM {} WHERE rowid=1", row.updated, row.table),
                                (),
                                |r| Ok(r.get::<i64>(0)?)
                            )?,
                            1,
                            "{} update",
                            row.table
                        );
                    }
                    for row in catalog.rows.iter().rev() {
                        tx.execute(&format!("DELETE FROM {} WHERE rowid=1", row.table), ())?;
                        assert_eq!(
                            tx.query_one(
                                &format!("SELECT count(*) FROM {} WHERE rowid=1", row.table),
                                (),
                                |r| Ok(r.get::<i64>(0)?)
                            )?,
                            0,
                            "{} delete",
                            row.table
                        );
                    }
                    Ok(())
                })
                .unwrap();
        }
    });
}

#[test]
fn foreign_keys() {
    each_store(|store| {
        for catalog in CATALOGS {
            store.write(catalog.db, Priority::Batch, move |tx| seed(tx, catalog.rows)).unwrap();
            for fk in catalog.foreign_keys {
                let result = store.write(catalog.db, Priority::Interactive, move |tx| {
                    tx.execute(
                        &format!(
                            "UPDATE {} SET {}={} WHERE rowid=1",
                            fk.table, fk.column, fk.missing
                        ),
                        (),
                    )?;
                    Ok(())
                });
                let error =
                    result.expect_err(&format!("{}.{} accepted an orphan", fk.table, fk.column));
                assert!(error.to_string().to_ascii_lowercase().contains("foreign key"), "{error}");
            }
        }
    });
}

#[test]
fn agent_cascades() {
    each_store(|store| {
        store
            .write(Db::Agent, Priority::Interactive, |tx| {
                seed(tx, agent::ROWS)?;
                tx.execute("DELETE FROM conversations WHERE id='sample'", ())?;
                Ok(())
            })
            .unwrap();
        for row in agent::ROWS {
            let table = row.table;
            let count = store
                .read(Db::Agent, move |tx| {
                    tx.query_one(&format!("SELECT count(*) FROM {table}"), (), |r| {
                        Ok(r.get::<i64>(0)?)
                    })
                })
                .unwrap();
            assert_eq!(count, 0, "{table} cascade");
        }
        store
            .write(Db::Intelligence, Priority::Interactive, |tx| {
                seed(tx, intelligence::ROWS)?;
                tx.execute("DELETE FROM intelligence_jobs WHERE id=1", ())?;
                Ok(())
            })
            .unwrap();
        assert_eq!(
            number(store, Db::Intelligence, "SELECT count(*) FROM intelligence_job_triggers"),
            0
        );
    });
}

#[test]
fn reopen_preserves_data() {
    for key in KEYS {
        let root = tempfile::tempdir().unwrap();
        {
            let store = Store::open(config(root.path(), key)).unwrap();
            for catalog in CATALOGS {
                store
                    .write(catalog.db, Priority::Interactive, move |tx| seed(tx, catalog.rows))
                    .unwrap();
            }
            store
                .write(Db::Scratch, Priority::Batch, |tx| {
                    tx.execute("CREATE TABLE disposable(id INTEGER PRIMARY KEY)", ())?;
                    Ok(())
                })
                .unwrap();
        }
        // CREATE TABLE statements deliberately lack IF NOT EXISTS: a replay fails.
        let store = Store::open(config(root.path(), key)).unwrap();
        for catalog in CATALOGS {
            assert_catalog(&store, catalog);
            for row in catalog.rows {
                let table = row.table;
                let count = store
                    .read(catalog.db, move |tx| {
                        tx.query_one(
                            &format!("SELECT count(*) FROM {table} WHERE rowid=1"),
                            (),
                            |r| Ok(r.get::<i64>(0)?),
                        )
                    })
                    .unwrap();
                assert_eq!(count, 1, "{table} reopen");
            }
        }
    }
}

#[test]
fn newer_versions_are_refused() {
    for key in KEYS {
        for db in Db::ALL {
            let root = tempfile::tempdir().unwrap();
            drop(Store::open(config(root.path(), key)).unwrap());
            let mut future = config(root.path(), key);
            future.migrations.register(
                db,
                vec![Migration::sql(5, "CREATE TABLE future_marker(id INTEGER PRIMARY KEY)")],
            );
            drop(Store::open(future).unwrap());
            assert!(
                matches!(Store::open(config(root.path(), key)), Err(Error::SchemaTooNew { db: found_db, found: 5, supported }) if found_db == db && supported == if db == Db::Scratch {0} else {1})
            );
        }
    }
}

/// Every durable database is created completely in one v1 transaction.
#[test]
fn consolidated_v1_contains_every_read_and_search_index() {
    each_store(|store| {
        for catalog in CATALOGS {
            assert_catalog(store, catalog);
            let expected = if catalog.db == Db::Scratch { 0 } else { 1 };
            assert_eq!(store.read(catalog.db, |s| s.schema_version()).unwrap(), expected);
        }
    });
}
