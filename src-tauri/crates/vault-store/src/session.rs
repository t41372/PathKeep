//! Responsibilities: keep statements/cursors on one thread and scope each transaction.
//! Not responsible for: scheduling, application SQL or accumulating large result sets.
//! Failure modes: reads write; errors/panics leak transactions; cursor escape shares
//! ownership; failed rollback is hidden; doctor cannot inspect encrypted files or metadata. Tests: concurrency, api compile-fail docs.
use crate::{Error, Result};
use std::{
    cell::{Cell, RefCell},
    marker::PhantomData,
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};
use tokio::runtime::Runtime;

/// Borrowed synchronous access exists only during an owner-thread closure.
/// Cursors never escape; map rows to application values before returning.
///
/// ```compile_fail
/// fn escape(store: &vault_store::Store) {
///     store.read(vault_store::Db::Archive, |conn| Ok(conn));
/// }
/// ```
/// ```compile_fail
/// fn send(conn: &vault_store::Session<'_>) {
///     std::thread::scope(|s| { s.spawn(move || conn.execute("SELECT 1", ())); });
/// }
/// ```
pub struct Session<'a> {
    pub(crate) statements: Cell<usize>,
    pub(crate) runtime: &'a Runtime,
    pub(crate) conn: &'a turso::Connection,
    pub(crate) prepared: Option<&'a crate::statements::StatementCache>,
    pub(crate) read_only: bool,
    pub(crate) _thread: PhantomData<Rc<()>>,
    pub(crate) scratch: Option<&'a crate::checkpoint::Gate>,
    pub(crate) scratch_read: RefCell<Option<crate::checkpoint::ReadGuard<'a>>>,
}
impl<'a> Session<'a> {
    pub(crate) fn with_scratch(mut self, gate: Option<&'a crate::checkpoint::Gate>) -> Self {
        self.scratch = gate;
        self
    }
    fn check_sql(&self, sql: &str) -> Result<()> {
        if crate::sql::validate(sql, self.read_only)?
            && let Some(gate) = self.scratch
        {
            let mut held = self.scratch_read.borrow_mut();
            if held.is_none() {
                *held = Some(gate.enter());
            }
        }
        self.statements.set(self.statements.get() + 1);
        Ok(())
    }
    /// Counts application statements in this job, for bounded-write diagnostics.
    pub fn statement_count(&self) -> usize {
        self.statements.get()
    }

    /// Doctor needs migration metadata without opening another database owner.
    pub fn schema_version(&self) -> Result<i64> {
        crate::migration::version(self)
    }

    /// Runs Turso's structural and row/index consistency checks on this snapshot.
    /// This fixed read-only pragma does not expose arbitrary connection settings.
    pub fn integrity_check(&self) -> Result<Vec<String>> {
        self.runtime.block_on(async {
            let mut rows = self.conn.query("PRAGMA integrity_check", ()).await?;
            let mut messages = Vec::new();
            while let Some(row) = rows.next().await? {
                messages.push(row.get::<String>(0)?);
            }
            Ok(messages)
        })
    }

    /// Runs `work` with this connection's compiled statement for `sql`, compiling
    /// it only the first time. The statement goes back to the cache afterwards.
    fn with_statement<T>(
        &self,
        sql: &str,
        work: impl FnOnce(&mut turso::Statement) -> Result<T>,
    ) -> Result<T> {
        self.check_sql(sql)?;
        let mut statement = match self.prepared {
            Some(cache) => cache.take(self.runtime, self.conn, sql)?,
            None => self.runtime.block_on(self.conn.prepare(sql))?,
        };
        let result = work(&mut statement);
        if let Some(cache) = self.prepared {
            if changes_schema(sql) {
                cache.clear();
            } else {
                cache.put(sql, statement);
            }
        }
        result
    }

    /// Apply one statement within the transaction already owned by the facade.
    pub fn execute(&self, sql: &str, params: impl turso::IntoParams) -> Result<u64> {
        self.with_statement(sql, |statement| {
            Ok(self.runtime.block_on(statement.execute(params))?)
        })
    }
    /// Collect a bounded application page. For unbounded queries use `for_each`.
    pub fn query<T>(
        &self,
        sql: &str,
        params: impl turso::IntoParams,
        mut map: impl FnMut(&turso::Row) -> Result<T>,
    ) -> Result<Vec<T>> {
        let mut values = Vec::new();
        self.for_each(sql, params, |row| {
            values.push(map(row)?);
            Ok(())
        })?;
        Ok(values)
    }
    /// Decode a single result without exposing a live cursor to the caller.
    pub fn query_one<T>(
        &self,
        sql: &str,
        params: impl turso::IntoParams,
        map: impl FnOnce(&turso::Row) -> Result<T>,
    ) -> Result<T> {
        self.with_statement(sql, |statement| {
            self.runtime.block_on(async {
                let mut rows = statement.query(params).await?;
                let row = rows.next().await?.ok_or(turso::Error::QueryReturnedNoRows)?;
                let result = map(&row)?;
                // Drain RETURNING so its statement completes before the transaction.
                while rows.next().await?.is_some() {}
                Ok(result)
            })
        })
    }
    /// Stream rows without an archive-sized allocation. Long walks should use Store::walk.
    pub fn for_each(
        &self,
        sql: &str,
        params: impl turso::IntoParams,
        mut visit: impl FnMut(&turso::Row) -> Result<()>,
    ) -> Result<()> {
        self.with_statement(sql, |statement| {
            self.runtime.block_on(async {
                let mut rows = statement.query(params).await?;
                while let Some(row) = rows.next().await? {
                    visit(&row)?
                }
                Ok(())
            })
        })
    }
}

fn changes_schema(sql: &str) -> bool {
    let first = sql.trim_start().split(|c: char| !c.is_ascii_alphabetic()).next().unwrap_or("");
    ["CREATE", "ALTER", "DROP"].iter().any(|ddl| first.eq_ignore_ascii_case(ddl))
}

pub(crate) fn catch_job<T>(work: impl FnOnce() -> Result<T>) -> Result<T> {
    catch_unwind(AssertUnwindSafe(work)).unwrap_or_else(|payload| {
        let message = payload
            .downcast_ref::<String>()
            .cloned()
            .or_else(|| payload.downcast_ref::<&str>().map(|s| (*s).to_owned()))
            .unwrap_or_else(|| "non-string panic payload".into());
        Err(Error::JobPanicked { message })
    })
}

pub(crate) fn transaction<T>(
    session: &Session<'_>,
    work: impl FnOnce(&Session<'_>) -> Result<T>,
) -> Result<T> {
    // Each database already has one writer. IMMEDIATE would also reserve the
    // attached scratch writer, unnecessarily serializing unrelated databases.
    session.runtime.block_on(session.conn.execute("BEGIN", ()))?;
    let result = catch_job(|| work(session));
    match result {
        Ok(value) => match session.runtime.block_on(session.conn.execute("COMMIT", ())) {
            Ok(_) => Ok(value),
            Err(error) => rollback(session, Error::from(error)),
        },
        Err(error) => rollback(session, error),
    }
}
fn rollback<T>(s: &Session<'_>, original: Error) -> Result<T> {
    // Some engine errors auto-rollback; only rollback a transaction that remains open.
    if s.conn.is_autocommit()? {
        return Err(original);
    }
    match s.runtime.block_on(s.conn.execute("ROLLBACK", ())) {
        Ok(_) => Err(original),
        Err(error) => {
            Err(Error::Rollback { original: Box::new(original), rollback: Box::new(error.into()) })
        }
    }
}
