//! Responsibilities: supply the fresh Turso schemas to the storage owner.
//! Not responsible for: opening files, query ports, old SQLite conversion or keys.
//! Failure modes: missing database registration, migrations rerun on reopen,
//! a newer version opens, or scratch acquires persistent application tables.
//! Tests: newer_versions_are_refused.
//! Drops: legacy migration ledgers are replaced by vault-store's __pk_migrations;
//! their checksums, timestamps and backup paths describe the old SQLite steps.
//!
//! Until a release has written these databases, every schema change edits version 1 in
//! place; no install holds an earlier version to migrate from. The first change after
//! that release adds version 2.

mod agent;
mod archive;
mod intelligence;
mod scratch;
mod source_evidence;

#[cfg(test)]
pub(crate) mod tests;

use vault_store::{Db, Migration, MigrationRegistry};

/// Doctor and startup agree on the current native archive revision.
pub(crate) const ARCHIVE_SCHEMA_VERSION: i64 = 1;

/// Gives the owner one complete schema contract before it opens any database.
pub fn migration_registry() -> MigrationRegistry {
    let mut registry = MigrationRegistry::default();
    registry.register(Db::Archive, vec![archive::migration()]);
    registry.register(Db::SourceEvidence, vec![source_evidence::migration()]);
    registry.register(Db::Intelligence, vec![intelligence::migration()]);
    registry.register(Db::Agent, vec![agent::migration()]);
    registry.register(Db::Scratch, scratch::migrations());
    registry
}

fn v1(statements: &'static [&'static str]) -> Migration {
    Migration::rust(1, move |tx| {
        for statement in statements {
            tx.execute(statement, ())?;
        }
        Ok(())
    })
}
