//! Responsibilities: reuse native prepared statements and stream rows on the owning thread.
//! Not responsible for: transactions, database selection or business SQL.
//! Failure modes: rebinding overlaps a live cursor, a statement escapes its Session,
//! policy is bypassed, or optional reads suppress failures. Tests: statements.rs.

use crate::{Error, Result, Row, Session};

/// Borrows its transaction so preparation cannot export a connection or a cursor.
///
/// ```compile_fail
/// fn escape(store: &vault_store::Store) {
///     store.read(vault_store::Db::Archive, |s| s.prepare("SELECT 1"));
/// }
/// ```
pub struct Statement<'s, 'db> {
    sql: String,
    session: &'s Session<'db>,
    /// Present until drop, which hands it back to the connection's cache.
    native: Option<turso::Statement>,
}

impl<'db> Session<'db> {
    /// Reuses compilation within a batch while retaining SQL policy and thread ownership.
    pub fn prepare(&self, sql: &str) -> Result<Statement<'_, 'db>> {
        self.check_sql(sql)?;
        let native = self.take_statement(sql)?;
        Ok(Statement { session: self, native: Some(native), sql: sql.to_owned() })
    }
}

impl Drop for Statement<'_, '_> {
    fn drop(&mut self) {
        if let Some(native) = self.native.take() {
            self.session.return_statement(&self.sql, native);
        }
    }
}

impl Statement<'_, '_> {
    fn native(&mut self) -> &mut turso::Statement {
        self.native.as_mut().expect("a statement keeps its native statement until drop")
    }
    /// Executes with fresh parameter bindings in the current transaction.
    pub fn execute(&mut self, params: impl turso::IntoParams) -> Result<u64> {
        let runtime = self.session.runtime;
        Ok(runtime.block_on(self.native().execute(params))?)
    }

    /// Keeps the cursor tied to both this statement and its owning transaction.
    pub fn query(&mut self, params: impl turso::IntoParams) -> Result<Rows<'_>> {
        let params = params.into_params()?;
        self.session.record_plan(&self.sql, &params)?;
        let runtime = self.session.runtime;
        let native = runtime.block_on(self.native().query(params))?;
        Ok(Rows { runtime: self.session.runtime, native, _owner: std::marker::PhantomData })
    }

    /// Requires one row without collecting the result set.
    pub fn query_one<T>(
        &mut self,
        params: impl turso::IntoParams,
        map: impl FnOnce(&Row) -> Result<T>,
    ) -> Result<T> {
        let mut rows = self.query(params)?;
        let row = rows.next()?.ok_or(Error::Engine(turso::Error::QueryReturnedNoRows))?;
        let value = map(&row)?;
        while rows.next()?.is_some() {}
        Ok(value)
    }

    /// Decodes rows lazily; collecting is an explicit choice made by the business caller.
    pub fn query_map<T, F>(
        &mut self,
        params: impl turso::IntoParams,
        map: F,
    ) -> Result<MappedRows<'_, F>>
    where
        F: FnMut(&Row) -> Result<T>,
    {
        Ok(MappedRows { rows: self.query(params)?, map })
    }
}

/// A streaming cursor cannot outlive or leave its transaction's thread.
pub struct Rows<'a> {
    runtime: &'a tokio::runtime::Runtime,
    native: turso::Rows,
    _owner: std::marker::PhantomData<&'a mut std::rc::Rc<()>>,
}
impl Rows<'_> {
    /// Advances without collecting the remaining result into memory.
    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self) -> Result<Option<Row>> {
        Ok(self.runtime.block_on(self.native.next())?)
    }
}

/// Iterator decoding happens on the same thread as the connection.
pub struct MappedRows<'a, F> {
    rows: Rows<'a>,
    map: F,
}
impl<T, F: FnMut(&Row) -> Result<T>> Iterator for MappedRows<'_, F> {
    type Item = Result<T>;
    fn next(&mut self) -> Option<Self::Item> {
        match self.rows.next() {
            Ok(Some(row)) => Some((self.map)(&row)),
            Ok(None) => None,
            Err(error) => Some(Err(error)),
        }
    }
}

/// Absence is ordinary for unique-key reads; other errors remain visible.
pub trait OptionalRow<T> {
    fn optional(self) -> Result<Option<T>>;
}
impl<T> OptionalRow<T> for Result<T> {
    fn optional(self) -> Result<Option<T>> {
        match self {
            Ok(value) => Ok(Some(value)),
            Err(Error::Engine(turso::Error::QueryReturnedNoRows)) => Ok(None),
            Err(error) => Err(error),
        }
    }
}

/// Bind owned values without moving fields out of borrowed domain records.
#[macro_export]
macro_rules! params_owned {
    ($($value:expr),* $(,)?) => {
        $crate::params![$(($value).to_owned()),*]
    };
}
