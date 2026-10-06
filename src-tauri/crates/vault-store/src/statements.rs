//! Responsibilities: keep one connection's compiled statements for reuse across jobs.
//! Not responsible for: validating SQL (session.rs) or caching results.
//! Failure modes: every call recompiles (Turso compiles on each `Connection::execute`,
//! which made a 10,000-row evidence batch spend minutes in the compiler); dynamic SQL
//! grows the cache without bound; a half-read statement keeps its read snapshot and
//! blocks checkpoints; a schema change on the same connection leaves a stale program
//! (Turso does not re-prepare `SELECT *` after this connection's own ALTER), so DDL
//! clears the cache. Session calls never nest (a row callback cannot run SQL), so a
//! statement is never reused while its rows are being read. Tests: tests/statements.rs.
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
}
struct Entry {
    statement: turso::Statement,
    used: u64,
}

impl StatementCache {
    pub fn new(compiled: Arc<AtomicU64>) -> Self {
        Self { entries: RefCell::default(), clock: Cell::new(0), compiled }
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
