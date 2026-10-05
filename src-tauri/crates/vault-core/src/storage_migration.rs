//! Responsibilities: identify operations unsafe while readers and writers use different stores.
//! Not responsible for: converting SQLite files, bridging data or selecting UI copy.
//! Failure modes: an unsafe operation mutates files before refusing; callers lose the typed
//! reason. Tests: storage_migration::tests and worker boundary tests.

/// Stable refusal shared by desktop commands, worker calls and MCP.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StorageMigrationIncomplete {
    pub operation: &'static str,
}
impl std::fmt::Display for StorageMigrationIncomplete {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "storage-migration-incomplete: {} is unavailable until the storage migration completes",
            self.operation
        )
    }
}
impl std::error::Error for StorageMigrationIncomplete {}

/// This build has Turso writers and SQLite readers. Never bridge them implicitly.
pub fn require_complete_storage_migration(operation: &'static str) -> anyhow::Result<()> {
    Err(StorageMigrationIncomplete { operation }.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn migration_refusal_keeps_a_typed_operation() {
        let error = require_complete_storage_migration("archive rekey").unwrap_err();
        assert_eq!(
            error.downcast_ref::<StorageMigrationIncomplete>().unwrap().operation,
            "archive rekey"
        );
        assert!(error.to_string().contains("unavailable until the storage migration completes"));
    }
}

#[cfg(test)]
mod ai_boundary_tests {
    #[test]
    fn public_ai_starred_filter_refuses_before_reading_either_store() {
        let vault = crate::Vault::open_temp().unwrap();
        let request = crate::AiSearchRequest { starred_only: Some(true), ..Default::default() };
        let error = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(crate::ai::semantic_search_history(
                &vault,
                vault.paths(),
                vault.config(),
                None,
                None,
                &request,
            ))
            .unwrap_err();
        assert!(error.downcast_ref::<super::StorageMigrationIncomplete>().is_some());
    }
}
