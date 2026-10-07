//! Responsibilities: preserve AI indexing, queue and read-model behavior on real Vault databases.
//! Not responsible for: search candidate selection or provider network integration.
//! Failure modes: recovery loses jobs, cursors skip visits, metadata drifts from vectors,
//! changed enrichment leaves stale vectors, or a page size becomes a coverage cap.
use super::*;
use crate::ai::search::semantic_search_history;
use crate::test_support::native::TestDatabase;
use vault_store::{Db, Priority, params_owned as params};
/// Shared with the code-mode and agent-tool suites, which seed the same native archive.
pub(in crate::ai) mod fixtures;
use fixtures::{
    embedding_count, prepared_archive, put_embedding, seed_embedding, seed_extra_visit,
    seed_failed_index_ledger, seed_visit,
};
mod coverage;
mod indexing;
mod status;
mod working_set;
