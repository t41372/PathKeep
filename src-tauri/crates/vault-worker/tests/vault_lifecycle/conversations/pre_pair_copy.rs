//! Build the two historical copy shapes exercised by repair. Their schema cutoffs stay
//! fixed when new migrations are added to the product.
use std::{fs, path::Path};
use vault_store::{Config, Db, Store};

#[derive(Clone, Copy)]
pub(super) enum LegacyCopy {
    BeforePairSeal,
    PendingSeeds,
}

impl LegacyCopy {
    pub(super) fn replace_roles(self, snapshot: &Path) {
        let previous = tempfile::tempdir().unwrap();
        let mut config = Config::new(previous.path());
        config.databases = vec![Db::AgentHistoryEvidence, Db::AgentHistoryReadModel, Db::Scratch];
        let versions = match self {
            Self::BeforePairSeal => [60, 112],
            Self::PendingSeeds => [66, 188],
        };
        let roles = [Db::AgentHistoryEvidence, Db::AgentHistoryReadModel];
        for (role, version) in roles.into_iter().zip(versions) {
            config.migrations.register(
                role,
                agent_history::schema_steps(role).into_iter().take(version).collect(),
            );
        }
        Store::open(config).unwrap().close().unwrap();
        for (db, version) in roles.into_iter().zip(versions) {
            let relative = vault_layout::database_path(db);
            let old = previous.path().join(relative);
            let current = snapshot.join(relative);
            populate(&old, &current);
            let conn = rusqlite::Connection::open(&old).unwrap();
            if matches!(self, Self::PendingSeeds) {
                // The observed copy had its seed rows, a lagging ledger and no seal table.
                conn.execute_batch("DROP TABLE ah_pair_seal").unwrap();
                conn.execute(
                    "DELETE FROM __pk_migrations WHERE version > ?1",
                    [version as i64 - 1],
                )
                .unwrap();
            }
            let has_seal = conn
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM sqlite_schema WHERE name='ah_pair_seal')",
                    [],
                    |r| r.get::<_, bool>(0),
                )
                .unwrap();
            assert!(!has_seal);
            drop(conn);
            fs::copy(old, current).unwrap();
        }
    }
}

fn identifier(s: &str) -> String {
    format!("\"{}\"", s.replace('"', "\"\""))
}

fn populate(old: &Path, current: &Path) {
    let mut connection = rusqlite::Connection::open(old).unwrap();
    connection.execute("ATTACH ?1 AS populated", [current.to_str().unwrap()]).unwrap();
    connection.execute_batch("PRAGMA foreign_keys=OFF").unwrap();
    let tx = connection.transaction().unwrap();
    let tables = tx
        .prepare("SELECT name FROM main.sqlite_schema WHERE type='table'")
        .unwrap()
        .query_map([], |r| r.get::<_, String>(0))
        .unwrap()
        .collect::<rusqlite::Result<Vec<_>>>()
        .unwrap();
    for name in tables {
        if name.starts_with("sqlite_") || name.starts_with("__") {
            continue;
        }
        let table = identifier(&name);
        let columns = tx
            .prepare(&format!("SELECT * FROM main.{table} LIMIT 0"))
            .unwrap()
            .column_names()
            .into_iter()
            .map(identifier)
            .collect::<Vec<_>>()
            .join(",");
        tx.execute(&format!("DELETE FROM main.{table}"), []).unwrap();
        tx.execute(
            &format!("INSERT INTO main.{table}({columns}) SELECT {columns} FROM populated.{table}"),
            [],
        )
        .unwrap();
    }
    tx.commit().unwrap();
    connection.execute_batch("DETACH populated; PRAGMA wal_checkpoint(TRUNCATE)").unwrap();
}
