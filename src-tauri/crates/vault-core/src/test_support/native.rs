//! Responsibilities: execute small test fixtures and assertions through Vault's real pools.
//! Not responsible for: product queries, schema installation or emulating SQLite storage.
//! Failure modes: fixture values borrow a caller stack, a script drops its final statement,
//! or an assertion reads a different database from the operation under test.
use crate::Vault;
use vault_store::{Db, IntoParams, Priority, Result, Row};

pub(crate) fn execute(vault: &Vault, db: Db, sql: &str, params: impl IntoParams) -> Result<u64> {
    let sql = sql.to_owned();
    let params = params.into_params()?;
    vault.store().write(db, Priority::Batch, move |s| s.execute(&sql, params))
}
pub(crate) fn one<T: Send + 'static>(
    vault: &Vault,
    db: Db,
    sql: &str,
    params: impl IntoParams,
    map: impl FnOnce(&Row) -> Result<T> + Send + 'static,
) -> Result<T> {
    let sql = sql.to_owned();
    let params = params.into_params()?;
    vault.store().read(db, move |s| s.query_one(&sql, params, map))
}
pub(crate) fn rows<T: Send + 'static>(
    vault: &Vault,
    db: Db,
    sql: &str,
    params: impl IntoParams,
    map: impl FnMut(&Row) -> Result<T> + Send + 'static,
) -> Result<Vec<T>> {
    let sql = sql.to_owned();
    let params = params.into_params()?;
    vault.store().read(db, move |s| s.query(&sql, params, map))
}
/// SQLite's lexer only finds script boundaries; every statement executes on Turso.
pub(crate) fn script(vault: &Vault, db: Db, sql: &str) -> Result<()> {
    let mut statements = Vec::new();
    let mut pending = String::new();
    for piece in sql.split_inclusive(';') {
        pending.push_str(piece);
        let text = std::ffi::CString::new(pending.as_str()).expect("fixture SQL contains no NUL");
        // sqlite3_complete reads this owned, terminated string and retains no pointer.
        if unsafe { rusqlite::ffi::sqlite3_complete(text.as_ptr()) } != 0 {
            statements.push(std::mem::take(&mut pending));
        }
    }
    if !pending.trim().is_empty() {
        statements.push(pending);
    }
    vault.store().write(db, Priority::Batch, move |s| {
        for statement in statements {
            s.execute(&statement, ())?;
        }
        Ok(())
    })
}

/// Fixture convenience around one real database; business helpers still receive Sessions.
#[derive(Clone)]
pub struct TestDatabase {
    pub vault: Vault,
    db: Db,
}
impl TestDatabase {
    pub fn new(vault: &Vault, db: Db) -> Self {
        Self { vault: vault.clone(), db }
    }
    pub fn execute(&self, sql: &str, params: impl IntoParams) -> Result<u64> {
        execute(&self.vault, self.db, sql, params)
    }
    pub fn execute_batch(&self, sql: &str) -> Result<()> {
        script(&self.vault, self.db, sql)
    }
    pub fn query_one<T: Send + 'static>(
        &self,
        sql: &str,
        params: impl IntoParams,
        map: impl FnOnce(&Row) -> Result<T> + Send + 'static,
    ) -> Result<T> {
        one(&self.vault, self.db, sql, params, map)
    }
    pub fn query_row<T: Send + 'static>(
        &self,
        sql: &str,
        params: impl IntoParams,
        map: impl FnOnce(&Row) -> Result<T> + Send + 'static,
    ) -> Result<T> {
        self.query_one(sql, params, map)
    }
    pub fn prepare(&self, sql: &str) -> Result<TestStatement> {
        Ok(TestStatement { db: self.clone(), sql: sql.to_owned() })
    }
    pub fn read<T: Send + 'static>(
        &self,
        f: impl FnOnce(&vault_store::Session<'_>) -> anyhow::Result<T> + Send + 'static,
    ) -> anyhow::Result<T> {
        self.vault.read(self.db, f)
    }
    pub fn write<T: Send + 'static>(
        &self,
        f: impl FnOnce(&vault_store::Session<'_>) -> anyhow::Result<T> + Send + 'static,
    ) -> anyhow::Result<T> {
        self.vault.write(self.db, Priority::Batch, f)
    }
}
pub struct TestStatement {
    db: TestDatabase,
    sql: String,
}
impl TestStatement {
    pub fn query_map<T: Send + 'static>(
        &mut self,
        params: impl IntoParams,
        map: impl FnMut(&Row) -> Result<T> + Send + 'static,
    ) -> Result<std::vec::IntoIter<Result<T>>> {
        Ok(rows(&self.db.vault, self.db.db, &self.sql, params, map)?
            .into_iter()
            .map(Ok)
            .collect::<Vec<_>>()
            .into_iter())
    }
    pub fn query_row<T: Send + 'static>(
        &mut self,
        params: impl IntoParams,
        map: impl FnOnce(&Row) -> Result<T> + Send + 'static,
    ) -> Result<T> {
        self.db.query_one(&self.sql, params, map)
    }
    pub fn execute(&mut self, params: impl IntoParams) -> Result<u64> {
        self.db.execute(&self.sql, params)
    }
}
