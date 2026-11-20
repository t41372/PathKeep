//! Responsibilities: supply the fresh Turso schemas to the storage owner.
//! Not responsible for: opening files, query ports, old SQLite conversion or keys.
//! Failure modes: missing database registration, migrations rerun on reopen,
//! a newer version opens, or scratch acquires persistent application tables.
//! Tests: newer_versions_are_refused.
//! Drops: legacy migration ledgers are replaced by vault-store's __pk_migrations;
//! their checksums, timestamps and backup paths describe the old SQLite steps.
//!
//! PathKeep v0.4.0 has written version 1 databases. Released steps are immutable;
//! each database has its own ordered sequence of transactional migrations.

mod agent;
mod archive;
pub(crate) mod backup_identity;
mod intelligence;
mod intelligence_lineage;
mod scratch;
mod source_evidence;

#[cfg(test)]
pub(crate) mod tests;

use vault_store::{Db, Migration, MigrationRegistry};

/// The version each database is at once it opens: Doctor compares with it. Read from the
/// registered steps, so adding one cannot leave the check a version behind.
pub fn schema_version(db: Db) -> i64 {
    migration_registry().latest_version(db)
}

/// Gives the owner one complete schema contract before it opens any database.
pub fn migration_registry() -> MigrationRegistry {
    let mut registry = MigrationRegistry::default();
    for database in vault_layout::databases() {
        registry.register(database.db, migrations(database.db));
    }
    registry
}

// Schema ownership stays here; a new storage identity must explicitly select its provider.
fn migrations(db: Db) -> Vec<Migration> {
    match db {
        Db::Archive => {
            let mut steps = vec![archive::migration()];
            steps.extend(backup_identity::archive());
            steps.push(task_dismissals(24, &["runs"]));
            steps
        }
        Db::SourceEvidence => vec![source_evidence::migration()],
        Db::Intelligence => vec![
            intelligence::migration(),
            intelligence_lineage::migration(),
            task_dismissals(3, &["intelligence_jobs", "ai_jobs"]),
        ],
        Db::Agent => vec![agent::migration()],
        Db::Scratch => scratch::migrations(),
        // The agent-history crate owns its stores' schemas; this archive opens them in its Store.
        Db::AgentHistoryEvidence | Db::AgentHistoryReadModel => agent_history::schema_steps(db),
    }
}

// Dismissal names the failed attempt. Retrying the same job produces a different finish
// timestamp, so a subsequent failure can appear in Needs a look again.
fn task_dismissals(version: i64, tables: &'static [&'static str]) -> Migration {
    Migration::rust(version, move |tx| {
        for table in tables {
            tx.execute(&format!("ALTER TABLE {table} ADD COLUMN task_dismissed_for TEXT"), ())?;
            let (state, kind) =
                if *table == "runs" { ("status", "run_type") } else { ("state", "job_type") };
            tx.execute(
                &format!("CREATE INDEX idx_{table}_task_state ON {table}({state},{kind})"),
                (),
            )?;
        }
        Ok(())
    })
}
fn v1(statements: &'static [&'static str]) -> Migration {
    Migration::rust(1, move |tx| {
        for statement in statements {
            tx.execute(statement, ())?;
        }
        Ok(())
    })
}
