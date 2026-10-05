//! Responsibilities: reserve an empty migration list for disposable scratch.
//! Not responsible for: operation-specific staging tables or startup cleanup.
//! Failure modes: scratch is versioned as durable state or survives reopening.
//! Tests: catalog_and_crud, reopen_preserves_data, newer_versions_are_refused.
//! Drops: none; vault-store recreates this database after successful preflight.

pub(super) fn migrations() -> Vec<vault_store::Migration> {
    Vec::new()
}
