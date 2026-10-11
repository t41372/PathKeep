//! Responsibilities: capture plans for the SQL and bindings used by real read paths, and by
//! the deletes and updates that find their rows the same way.
//! Not responsible for: defining acceptable application plans or timing benchmarks.
//! Failure modes: capture misses prepared statements, changes query behavior, or runs when disabled.
use crate::{Result, Session};
use std::sync::{
    Mutex,
    atomic::{AtomicBool, Ordering},
};

/// The SQL and plan observed on the same native owner that serves the query.
#[derive(Debug, Clone)]
pub struct QueryPlan {
    pub sql: String,
    pub steps: Vec<String>,
}

#[derive(Default)]
pub(crate) struct PlanCapture {
    enabled: AtomicBool,
    plans: Mutex<Vec<QueryPlan>>,
    /// Each statement run whose program opens an ephemeral table, once per run.
    ephemeral: Mutex<Vec<String>>,
}
impl PlanCapture {
    pub fn set_enabled(&self, enabled: bool) {
        self.enabled.store(enabled, Ordering::Relaxed);
    }
    pub fn take(&self) -> Vec<QueryPlan> {
        std::mem::take(&mut *self.plans.lock().unwrap())
    }
    pub fn take_ephemeral(&self) -> Vec<String> {
        std::mem::take(&mut *self.ephemeral.lock().unwrap())
    }
    pub fn record(
        &self,
        session: &Session<'_>,
        sql: &str,
        params: &turso::params::Params,
    ) -> Result<()> {
        if !self.enabled.load(Ordering::Relaxed) {
            return Ok(());
        }
        // Turso gives every ephemeral table (`RETURNING`, a materialized subquery, a compound
        // select) a new temporary file each time the statement runs, with `temp_store=FILE`.
        let opens_ephemeral = session.runtime.block_on(async {
            let mut rows = session.conn.query(&format!("EXPLAIN {sql}"), params.clone()).await?;
            let mut found = false;
            while let Some(row) = rows.next().await? {
                found |=
                    matches!(row.get_value(1)?, turso::Value::Text(op) if op == "OpenEphemeral");
            }
            Ok::<_, turso::Error>(found)
        })?;
        if opens_ephemeral {
            self.ephemeral.lock().unwrap().push(sql.to_owned());
        }
        // Deletes and updates find their rows like a read does; one planned without its key
        // reads the whole table per row it touches.
        let command = sql.split_whitespace().next().unwrap_or("");
        if !["SELECT", "WITH", "DELETE", "UPDATE"]
            .iter()
            .any(|kind| command.eq_ignore_ascii_case(kind))
        {
            return Ok(());
        }
        let steps = session.runtime.block_on(async {
            let mut rows =
                session.conn.query(&format!("EXPLAIN QUERY PLAN {sql}"), params.clone()).await?;
            let mut steps = Vec::new();
            while let Some(row) = rows.next().await? {
                steps.push(row.get::<String>(3)?);
            }
            Ok::<_, turso::Error>(steps)
        })?;
        self.plans.lock().unwrap().push(QueryPlan { sql: sql.to_owned(), steps });
        Ok(())
    }
}
