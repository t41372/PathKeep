//! Responsibilities: refuse a newer schema version, and seed full-column rows in every database
//! for the maintenance round trips.
//! Not responsible for: mocking Turso, reusing migration SQL as expectations or query ports.
//! Failure modes: an older app writing to a newer schema, a maintenance step losing a table.
mod agent;
mod archive;
mod intelligence;
mod lineage;
mod source_evidence;

use std::{num::NonZeroUsize, path::Path};
use vault_store::{Config, Db, Error, Migration, Priority, Session, Store};

struct Fixture {
    table: &'static str,
    columns: &'static str,
    values: &'static str,
}
struct Catalog {
    db: Db,
    rows: &'static [Fixture],
}
const CATALOGS: &[Catalog] = &[
    Catalog { db: Db::Archive, rows: archive::ROWS },
    Catalog { db: Db::SourceEvidence, rows: source_evidence::ROWS },
    Catalog { db: Db::Intelligence, rows: intelligence::ROWS },
    Catalog { db: Db::Agent, rows: agent::ROWS },
];
const KEYS: [Option<[u8; 32]>; 2] = [None, Some([0x5a; 32])];

fn config(root: &Path, key: Option<[u8; 32]>) -> Config {
    let mut config = Config::new(root);
    config.keys = [key; 5];
    config.readers_per_database = NonZeroUsize::new(1).unwrap();
    config.migrations = super::migration_registry();
    config
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

/// Reuse independent full-column fixtures when proving maintenance preserves tables.
pub(crate) fn seed_maintenance(vault: &crate::Vault) {
    for catalog in CATALOGS {
        vault.store().write(catalog.db, Priority::Batch, move |s| seed(s, catalog.rows)).unwrap();
    }
}

pub(crate) fn maintenance_rows(
    vault: &crate::Vault,
    databases: &[Db],
) -> Vec<(String, Vec<Vec<vault_store::Value>>)> {
    let mut result = Vec::new();
    for catalog in CATALOGS.iter().filter(|c| databases.contains(&c.db)) {
        for fixture in catalog.rows {
            if fixture.table == "search_outbox" {
                continue;
            }
            let sql = format!("SELECT {} FROM {} ORDER BY rowid", fixture.columns, fixture.table);
            let rows = vault
                .store()
                .read(catalog.db, move |s| {
                    s.query(&sql, (), |r| {
                        (0..r.column_count()).map(|i| r.get_value(i).map_err(Into::into)).collect()
                    })
                })
                .unwrap();
            result.push((format!("{:?}.{}", catalog.db, fixture.table), rows));
        }
    }
    result
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
                matches!(Store::open(config(root.path(), key)), Err(Error::SchemaTooNew { db: found_db, found: 5, supported }) if found_db == db && supported == if db == Db::Scratch {0} else if db == Db::Intelligence {2} else {1})
            );
        }
    }
}
