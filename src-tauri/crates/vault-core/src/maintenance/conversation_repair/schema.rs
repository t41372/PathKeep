//! Checks required tables, columns and indexes before using a repair input.
//! A structurally valid b-tree can still lack the ledger that failed at startup.
use anyhow::Result;
use std::collections::BTreeMap;
use vault_store::{Db, Store};

pub(super) struct Schema(BTreeMap<(String, String), Vec<String>>);

impl Schema {
    pub fn read(store: &Store, db: Db) -> Result<Self> {
        Ok(Self(store.read(db, |s| {
            let objects = s.query(
                "SELECT type,name FROM sqlite_schema WHERE name GLOB 'ah_*' ORDER BY type,name",
                (),
                |r| Ok((r.get::<String>(0)?, r.get::<String>(1)?)),
            )?;
            let mut result = BTreeMap::new();
            for (kind, name) in objects {
                let columns = if kind == "table" { s.table_columns(&name)? } else { Vec::new() };
                result.insert((kind, name), columns);
            }
            Ok(result)
        })?))
    }

    pub fn check(&self, store: &Store, db: Db) -> Result<()> {
        let actual = Self::read(store, db)?;
        for (key, columns) in &self.0 {
            if actual.0.get(key) != Some(columns) {
                return Err(agent_history::Error::Integrity(format!(
                    "{db:?} has a missing or incompatible {}",
                    key.1
                ))
                .into());
            }
        }
        Ok(())
    }
}
