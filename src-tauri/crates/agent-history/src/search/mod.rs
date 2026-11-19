//! Responsibilities: expose bounded full-text queries and explicit index synchronization.
//! Not responsible for: native parsing, archive publication, UI search syntax or models.
//! Search never repairs derived state. A canonical/index watermark mismatch is an
//! explicit error so callers cannot mistake an incomplete result for no matches.

mod index;
mod initial;
pub(crate) use initial::InitialSearch;
mod query;
mod schema;
mod sync;
mod tokenizer;

use crate::Harness;
use serde::{Deserialize, Serialize};

/// Literal, case-insensitive terms are combined with AND; harness/project filters
/// are exact. A result limit bounds returned material, never the searched corpus.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SearchQuery {
    pub text: String,
    pub harness: Option<Harness>,
    pub project_path: Option<String>,
    pub limit: usize,
}

/// A matching native event, with a bounded plain-text excerpt from canonical data.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SearchHit {
    pub session_id: String,
    pub event_id: i64,
    pub ordinal: u64,
    pub text: String,
    pub score: f32,
    pub put_away: bool,
}

/// `total` counts matching events across the entire index. `complete` describes
/// normalization coverage; an index behind canonical publication returns an error.
/// This API returns top-k results; `next_cursor` is reserved and always absent.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SearchPage {
    pub items: Vec<SearchHit>,
    pub next_cursor: Option<String>,
    pub total: u64,
    pub complete: bool,
}

/// Reports durable synchronization, including whether derived state was rebuilt.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SearchSyncReport {
    pub indexed_events: u64,
    pub watermark: i64,
    pub rebuilt: bool,
}
