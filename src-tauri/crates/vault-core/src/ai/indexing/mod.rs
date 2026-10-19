//! Failure modes: metadata, vector maps and ledger watermarks disagree after interruption.
//! Semantic index build and ledger persistence.
//!
//! ## Responsibilities
//! - build or clear the semantic index for one embedding provider
//! - keep the AI index ledger and unified `runs` trace in sync with build outcomes
//! - collect changed canonical visits and write native metadata rows
//! - expose storage/watermark helpers consumed by AI read models
//!
//! ## Not responsible for
//! - assistant prompt orchestration or semantic result ranking
//! - provider client setup and request-format-specific embedding execution
//! - Settings-facing read-model assembly
//!
//! ## Module layout (W-AI-4 indexing split)
//! This is the orchestration root; the load-bearing logic lives in focused submodules so the
//! crash/resume dedup state and the Session/candidate queries are each cohesive and directly tested:
//! - [`backfill`] — the resumable embed loop ([`run_embedding_backfill`]) and the
//!   [`backfill::DedupTracker`] that encapsulates the W-AI-4c crash-window invariants.
//! - [`candidates`] — candidate collection + the pure embed-target selection (content-hash dedup).
//! - [`store_rows`] — Turso `ai_embeddings` metadata helpers + batch validation.
//! - [`content`] — the enrichment→dedup-hash seam.
//!
//! ## Dependencies
//! - `super::control` for cooperative cancellation checkpoints
//! - `super::provider` for embedding retries and provider validation
//! - `crate::ai_sidecar` for the stable provider/model file name
//!
//! ## Performance notes
//! - candidate collection diffs content hashes before embedding, which avoids re-embedding
//!   unchanged visits on large archives
//! - Session lookups chunk `history_id` predicates with `SQLITE_BATCH_SIZE` to keep
//!   statement size and memory bounded on 14.4M+ histories

mod backfill;
mod candidates;
mod content;
mod store_rows;

// Re-export the surface `super` (ai.rs) consumes so `ai::indexing::{...}` keeps working verbatim
// after the split. The PUBLIC entrypoints stay `pub`; `provider_embedding_count` is the one helper a
// non-test sibling (read_model / search) reaches through `ai.rs`, so it stays unconditional. The
// remaining helpers are consumed only by the AI test surface (`ai/tests.rs` + `ai.rs`'s cfg(test)
// imports) — the production loop reaches them directly via the submodule paths — so their
// `indexing::` re-export is gated `#[cfg(test)]` to stay warning-clean in release builds.
pub use self::backfill::{
    build_ai_index, build_ai_index_with_control, forget_unconfigured_indexes,
};

#[cfg(test)]
pub(super) use self::candidates::{collect_visit_chunk, select_embed_targets};
pub(crate) use self::content::enrichment_summary_for;
#[cfg(test)]
pub(super) use self::store_rows::{clear_provider_embeddings, upsert_embedding};

/// One canonical visit that needs to be embedded into the semantic sidecar.
///
/// The index builder keeps this shape separate from `HistoryEntry` so it can carry the
/// normalized embedding payload and the dedup identity used for incremental change detection.
///
/// W-AI-4c dedup: `content_hash` is now the VISIT-INDEPENDENT dedup hash (canonical URL + title +
/// reserved enrichment summary, see [`super::dedup::build_dedup_content_hash`]), and `content_key`
/// is its u64 vector-store key — so many visits of one page share both and need ONE embedding.
/// `content` is the per-visit TEXT fed to the model (still via the enrichment funnel); two visits
/// sharing a content_key embed identical-enough text, so embedding the first and mapping the rest is
/// exact.
#[derive(Debug, Clone)]
pub(super) struct IndexedVisit {
    pub history_id: i64,
    pub profile_id: String,
    pub url: String,
    pub title: Option<String>,
    pub domain: String,
    pub visited_at: String,
    pub content: String,
    /// Visit-independent dedup hash (the page identity); equal across all visits of one page.
    pub content_hash: String,
    /// The u64 vector-store key derived from `content_hash` (the dedup key).
    pub content_key: u64,
    /// Whether the stored content hash differs from the freshly built one, so this visit's CONTENT
    /// must be (re-)embedded. `false` lets the backfill skip an unchanged row without an embed call.
    /// With dedup, the FIRST visit of an un-embedded page sets this; later visits of the SAME page in
    /// the same scan are skipped via the content_key-already-embedded check, not this flag.
    pub needs_embedding: bool,
}

/// Aggregate outcome of one embedding-backfill pass, fed into the final `AiIndexReport`.
#[derive(Debug, Default)]
pub(super) struct BackfillOutcome {
    /// Vectors freshly embedded + persisted this pass.
    pub indexed_items: usize,
    /// Rows re-embedded because their content hash changed (subset of `indexed_items`).
    pub updated_items: usize,
    /// Rows scanned but skipped because their content was unchanged.
    pub skipped_items: usize,
    /// The actual returned vector length once observed (D4); `None` if nothing was embedded.
    pub effective_dim: Option<usize>,
    /// Count of vectors actually appended to the `.pkvec` store this pass (F3 0-byte honesty).
    ///
    /// DISTINCT from `indexed_items`, which counts changed VISITS (each gets a Turso row + map
    /// entry). A re-visit of an already-embedded page bumps `indexed_items` but appends NO vector, so
    /// `embedded_vectors == 0` with `indexed_items > 0` is legitimate ONLY when the store already
    /// holds the page's vector. The build's honesty note reports this real count rather than implying
    /// every counted row produced a vector.
    pub embedded_vectors: u64,
}

pub(super) use self::store_rows::validate_embedding_batch_for_keys;
