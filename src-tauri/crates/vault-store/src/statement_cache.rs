//! Responsibilities: keep one connection's compiled statements for reuse across jobs, and what
//! `sql::validate` said about each statement text, so a text is checked once, not per call.
//! Not responsible for: the validation rules (sql.rs) or caching results.
//! Failure modes: every call recompiles (Turso compiles on each `Connection::execute`,
//! which made a 10,000-row evidence batch spend minutes in the compiler); dynamic SQL
//! grows the cache without bound; a half-read statement keeps its read snapshot and
//! blocks checkpoints; a schema change on the same connection leaves a stale program
//! (Turso does not re-prepare `SELECT *` after this connection's own ALTER), so DDL
//! clears the cache. Session calls never nest (a row callback cannot run SQL), so a
//! statement is never reused while its rows are being read. Tests: tests/statement_cache.rs.
use crate::Result;
use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};
use tokio::runtime::Runtime;

/// Distinct statements kept per connection. Application SQL is a fixed set of
/// strings plus `IN` lists of a few lengths; the bound only matters for SQL that
/// embeds values, which then recompiles instead of growing memory.
const CAPACITY: usize = 256;

pub(crate) struct StatementCache {
    entries: RefCell<HashMap<String, Entry>>,
    clock: Cell<u64>,
    compiled: Arc<AtomicU64>,
    /// Each text's validation, for a writer's and a reader's session apart. Checking a text again
    /// on every call took a tenth of a Takeout import's write time. Refusals are not kept.
    validated: RefCell<HashMap<String, [Option<(bool, bool)>; 2]>>,
}
struct Entry {
    statement: turso::Statement,
    used: u64,
}

impl StatementCache {
    pub fn new(compiled: Arc<AtomicU64>) -> Self {
        Self {
            entries: RefCell::default(),
            clock: Cell::new(0),
            compiled,
            validated: RefCell::default(),
        }
    }

    /// `sql::validate(sql, read_only)`, checked once per text. Dynamic SQL beyond a few times the
    /// statement bound starts the record over instead of growing it.
    pub fn validated(&self, sql: &str, read_only: bool) -> Result<(bool, bool)> {
        let slot = usize::from(read_only);
        if let Some(known) = self.validated.borrow().get(sql).and_then(|known| known[slot]) {
            return Ok(known);
        }
        let checked = crate::sql::validate(sql, read_only)?;
        let mut validated = self.validated.borrow_mut();
        if validated.len() >= 4 * CAPACITY {
            validated.clear();
        }
        validated.entry(sql.to_owned()).or_default()[slot] = Some(checked);
        Ok(checked)
    }

    /// Takes the statement out while it runs; `put` returns it.
    pub fn take(
        &self,
        runtime: &Runtime,
        conn: &turso::Connection,
        sql: &str,
    ) -> Result<turso::Statement> {
        if let Some(entry) = self.entries.borrow_mut().remove(sql) {
            return Ok(entry.statement);
        }
        self.compiled.fetch_add(1, Ordering::Relaxed);
        Ok(runtime.block_on(conn.prepare(sql))?)
    }

    /// Resets before keeping, so a statement stopped mid-read releases its snapshot.
    /// Turso re-prepares a kept statement itself after a schema change.
    pub fn put(&self, sql: &str, statement: turso::Statement) {
        if statement.reset().is_err() {
            return;
        }
        let used = self.clock.get() + 1;
        self.clock.set(used);
        let mut entries = self.entries.borrow_mut();
        if entries.len() >= CAPACITY
            && let Some(oldest) =
                entries.iter().min_by_key(|(_, entry)| entry.used).map(|(sql, _)| sql.clone())
        {
            entries.remove(&oldest);
        }
        entries.insert(sql.to_owned(), Entry { statement, used });
    }

    /// Schema changes invalidate every compiled program on this connection.
    pub fn clear(&self) {
        self.entries.borrow_mut().clear();
    }
}
