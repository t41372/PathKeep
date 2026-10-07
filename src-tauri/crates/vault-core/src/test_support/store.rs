//! Responsibilities: inspect real Store state in behavioral tests with owned SQL inputs.
//! Not responsible for: emulating SQLite connections, transactions or engine behavior.
//! Failure modes: a test reads another file or observes uncommitted state. Every
//! query goes through Store's production read queue and every edit through its writer.

use std::sync::Arc;
use vault_store::{Db, IntoParams, Priority, Result, Row, Store};

pub(crate) struct Probe {
    store: Arc<Store>,
    db: Db,
}
impl Probe {
    pub(crate) fn new(vault: &crate::Vault) -> Self {
        Self::for_db(vault, Db::Archive)
    }
    pub(crate) fn for_db(vault: &crate::Vault, db: Db) -> Self {
        Self { store: vault.store().clone(), db }
    }
    pub(crate) fn query_row<T: Send + 'static>(
        &self,
        sql: &str,
        params: impl IntoParams,
        map: impl FnOnce(&Row) -> Result<T> + Send + 'static,
    ) -> Result<T> {
        let sql = sql.to_owned();
        let params = params.into_params()?;
        self.store.read(self.db, move |s| s.query_one(&sql, params, map))
    }
    pub(crate) fn query<T: Send + 'static>(
        &self,
        sql: &str,
        params: impl IntoParams,
        map: impl FnMut(&Row) -> Result<T> + Send + 'static,
    ) -> Result<Vec<T>> {
        let sql = sql.to_owned();
        let params = params.into_params()?;
        self.store.read(self.db, move |s| s.query(&sql, params, map))
    }
    pub(crate) fn execute(&self, sql: &str, params: impl IntoParams) -> Result<u64> {
        let sql = sql.to_owned();
        let params = params.into_params()?;
        self.store.write(self.db, Priority::Interactive, move |s| s.execute(&sql, params))
    }
}
