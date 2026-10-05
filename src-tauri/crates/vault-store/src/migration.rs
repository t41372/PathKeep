//! Responsibilities: apply the owner-supplied ordered schema steps during open.
//! Not responsible for: defining business schemas or migrating on reads.
//! Failure modes: newer schema opens; failed steps advance version; partial steps
//! persist; duplicate/out-of-order versions are accepted; panics retain ownership.
//! Tests: migrations.
use crate::{Db, Error, Result, Session};
use std::{collections::HashMap, sync::Arc};
type Step = Arc<dyn Fn(&Session<'_>) -> Result<()> + Send + Sync>;

/// Owner crates supply immutable migration steps without accessing raw connections.
#[derive(Clone)]
pub struct Migration {
    pub(crate) version: i64,
    apply: Step,
}
impl Migration {
    pub fn sql(version: i64, sql: impl Into<String>) -> Self {
        let sql = sql.into();
        Self::rust(version, move |tx| {
            tx.execute(&sql, ())?;
            Ok(())
        })
    }
    pub fn rust(
        version: i64,
        apply: impl Fn(&Session<'_>) -> Result<()> + Send + Sync + 'static,
    ) -> Self {
        Self { version, apply: Arc::new(apply) }
    }
}
/// One ordered registry per database, evaluated only during Store::open.
#[derive(Default)]
pub struct MigrationRegistry {
    steps: HashMap<Db, Vec<Migration>>,
}
impl MigrationRegistry {
    pub fn register(&mut self, db: Db, steps: Vec<Migration>) {
        self.steps.insert(db, steps);
    }
    pub(crate) fn get(&self, db: Db) -> Vec<Migration> {
        self.steps.get(&db).cloned().unwrap_or_default()
    }
    pub(crate) fn validate(&self) -> Result<()> {
        for steps in self.steps.values() {
            let mut previous = 0;
            for step in steps {
                if step.version <= previous {
                    return Err(Error::InvalidConfig(
                        "migration versions must be positive and strictly increasing".into(),
                    ));
                }
                previous = step.version;
            }
        }
        Ok(())
    }
}
pub(crate) fn version(s: &Session<'_>) -> Result<i64> {
    s.runtime.block_on(async {
        let mut tables = s
            .conn
            .query("SELECT 1 FROM sqlite_schema WHERE type='table' AND name='__pk_migrations'", ())
            .await?;
        let exists = tables.next().await?.is_some();
        drop(tables);
        if !exists {
            return Ok(0);
        }
        let mut rows =
            s.conn.query("SELECT coalesce(max(version),0) FROM __pk_migrations", ()).await?;
        Ok(rows.next().await?.unwrap().get::<i64>(0)?)
    })
}
pub(crate) fn check_version(s: &Session<'_>, db: Db, steps: &[Migration]) -> Result<i64> {
    let found = version(s)?;
    let supported = steps.last().map_or(0, |s| s.version);
    if found > supported {
        return Err(Error::SchemaTooNew { db, found, supported });
    }
    Ok(found)
}
pub(crate) fn apply(s: &Session<'_>, db: Db, steps: &[Migration]) -> Result<()> {
    let found = check_version(s, db, steps)?;
    for step in steps.iter().filter(|step| step.version > found) {
        crate::session::transaction(s, |tx| {
            tx.runtime.block_on(tx.conn.execute(
                "CREATE TABLE IF NOT EXISTS __pk_migrations(version INTEGER PRIMARY KEY)",
                (),
            ))?;
            (step.apply)(tx)?;
            tx.runtime.block_on(
                tx.conn.execute("INSERT INTO __pk_migrations(version) VALUES (?1)", [step.version]),
            )?;
            Ok(())
        })?;
    }
    Ok(())
}
