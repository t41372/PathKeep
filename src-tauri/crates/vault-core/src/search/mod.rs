//! Responsibilities: canonical search, owner indexing and session contracts.
//! Not responsible for: UI presentation, archive writes or cryptography.
//! Failure modes: partial coverage hidden, stale visibility, lost cursors and orphaned jobs.
//! Tests: search::tests against plaintext and encrypted Vault owners.
mod documents;
mod eligibility;
mod generations;
mod indexer;
mod query;
pub(crate) mod regex;
mod regex_prefilter;
mod session;
#[cfg(test)]
mod tests;
mod types;
pub use indexer::{Indexer, IndexerPhase, IndexerState};
pub(crate) use session::Scores;
pub use session::SearchSession;
pub use types::*;
mod stream;
pub use stream::{SearchEvent, SearchSessions, SearchStarted};
mod history;
pub use history::{history, page};
pub(crate) use history::{history_for_root, page_prepared, walk_history_for_export};
#[cfg(test)]
mod benchmark;
#[cfg(test)]
mod fixture;
#[cfg(test)]
mod ported_grouped;
#[cfg(test)]
mod ported_regex;
mod visits;
pub use visits::visits;
pub(crate) use visits::{evidence, starred_pages};
#[cfg(test)]
mod coverage_tests;
#[cfg(test)]
mod fuzzy_tests;
#[cfg(test)]
mod ported_queries;
#[cfg(test)]
mod projection_tests;
#[cfg(test)]
mod recovery_tests;
