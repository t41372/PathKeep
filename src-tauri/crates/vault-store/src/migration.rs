//! Responsibilities: apply the owner-supplied ordered schema steps during open, and tell the
//! owner when an open starts updating a database that already had a schema.
//! Not responsible for: defining business schemas or migrating on reads.
//! Failure modes: newer schema opens; failed steps advance version; partial steps
//! persist; duplicate/out-of-order versions are accepted; panics retain ownership; an update
//! goes untold (the owner lets someone quit without asking) or a database that needs nothing is
//! reported as updating. Tests: migrations.
use crate::{Db, Error, Result, Session};
use std::{collections::HashMap, sync::Arc};
type Step = Arc<dyn Fn(&Session<'_>) -> Result<()> + Send + Sync>;
type Backfill = Arc<dyn Fn(&Session<'_>) -> Result<Option<BackfillProgress>> + Send + Sync>;
type Pending = Arc<dyn Fn(&Session<'_>) -> Result<bool> + Send + Sync>;
type Observer = Arc<dyn Fn(BackfillProgress) + Send + Sync>;
type UpdateObserver = Arc<dyn Fn(Db) + Send + Sync>;

/// A committed startup batch. None from the callback means no unfinished backfill remains.
#[derive(Debug, Clone)]
pub struct BackfillProgress {
    pub processed: u64,
    pub total: u64,
    pub table: Option<String>,
    pub complete: bool,
}

/// Owner crates supply immutable migration steps without accessing raw connections.
#[derive(Clone)]
pub struct Migration {
    pub(crate) version: i64,
    apply: Step,
    backfill: Option<(Pending, Backfill)>,
    observer: Option<Observer>,
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
        Self { version, apply: Arc::new(apply), backfill: None, observer: None }
    }
    /// Large data moves commit bounded batches before readers or later migrations can start.
    /// The callback must retain its own durable completion marker and resume cursor.
    ///
    /// `pending` reads that marker without changing anything: whether batches remain. An open
    /// whose versions are all applied asks it before the first batch, so it can tell the owner
    /// that it is about to update the database (`MigrationRegistry::on_update`) and not tell it
    /// when the backfill finished long ago.
    pub fn with_backfill(
        mut self,
        pending: impl Fn(&Session<'_>) -> Result<bool> + Send + Sync + 'static,
        batch: impl Fn(&Session<'_>) -> Result<Option<BackfillProgress>> + Send + Sync + 'static,
    ) -> Self {
        self.backfill = Some((Arc::new(pending), Arc::new(batch)));
        self
    }
    /// Observe only committed progress; callbacks must not reenter the storage owner.
    pub fn on_backfill_progress(
        mut self,
        observer: impl Fn(BackfillProgress) + Send + Sync + 'static,
    ) -> Self {
        self.observer = Some(Arc::new(observer));
        self
    }
}
/// One ordered registry per database, evaluated only during Store::open.
#[derive(Default)]
pub struct MigrationRegistry {
    steps: HashMap<Db, Vec<Migration>>,
    on_update: Option<UpdateObserver>,
}
/// One database's steps as an open runs them, with the registry's update observer.
pub(crate) struct Plan {
    steps: Vec<Migration>,
    on_update: Option<UpdateObserver>,
}
impl MigrationRegistry {
    /// The version a database reaches once every registered step has run; 0 with none.
    pub fn latest_version(&self, db: Db) -> i64 {
        self.steps.get(&db).and_then(|steps| steps.last()).map_or(0, |step| step.version)
    }
    pub fn register(&mut self, db: Db, steps: Vec<Migration>) {
        self.steps.insert(db, steps);
    }
    /// Told before an open changes a database that already had a schema: before the first
    /// version it applies there, or before the first batch of a backfill an earlier open left
    /// unfinished. Once per database and open. Never told for a database the open creates, nor
    /// for one that is up to date. It runs on that database's writer thread, before the change,
    /// and the open waits for it.
    pub fn on_update(&mut self, observer: impl Fn(Db) + Send + Sync + 'static) {
        self.on_update = Some(Arc::new(observer));
    }
    pub(crate) fn get(&self, db: Db) -> Vec<Migration> {
        self.steps.get(&db).cloned().unwrap_or_default()
    }
    pub(crate) fn plan(&self, db: Db) -> Plan {
        Plan { steps: self.get(db), on_update: self.on_update.clone() }
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
pub(crate) fn apply(
    s: &Session<'_>,
    db: Db,
    plan: &Plan,
    mut after_batch: impl FnMut() -> Result<()>,
) -> Result<()> {
    let found = check_version(s, db, &plan.steps)?;
    // A database at version 0 is being created, not updated.
    let mut told = found == 0;
    let tell = |told: &mut bool| {
        if !std::mem::replace(told, true)
            && let Some(observer) = &plan.on_update
        {
            observer(db);
        }
    };
    for step in &plan.steps {
        if step.version > found {
            tell(&mut told);
            crate::session::transaction(s, |tx| {
                tx.runtime.block_on(tx.conn.execute(
                    "CREATE TABLE IF NOT EXISTS __pk_migrations(version INTEGER PRIMARY KEY)",
                    (),
                ))?;
                if crate::opening::conversation_role(db) && crate::pair::ticket(tx)?.is_some() {
                    tx.execute("DELETE FROM ah_pair_seal", ())?;
                }
                (step.apply)(tx)?;
                tx.runtime.block_on(
                    tx.conn.execute(
                        "INSERT INTO __pk_migrations(version) VALUES (?1)",
                        [step.version],
                    ),
                )?;
                Ok(())
            })?;
        }
        // A version marker protects its new schema from older binaries. Its data backfill may
        // still need work after a crash; always consult its durable completion marker on reopen.
        if let Some((pending, batch)) = &step.backfill {
            if !told {
                if !crate::session::transaction(s, |tx| pending(tx))? {
                    continue;
                }
                tell(&mut told);
            }
            loop {
                let progress = crate::session::transaction(s, |tx| {
                    let result = batch(tx)?;
                    if result.is_some()
                        && crate::opening::conversation_role(db)
                        && crate::pair::ticket(tx)?.is_some()
                    {
                        tx.execute("DELETE FROM ah_pair_seal", ())?;
                    }
                    Ok(result)
                })?;
                let Some(progress) = progress else { break };
                after_batch()?;
                let complete = progress.complete;
                if let Some(observer) = &step.observer {
                    observer(progress);
                }
                if complete {
                    break;
                }
            }
        }
    }
    Ok(())
}
