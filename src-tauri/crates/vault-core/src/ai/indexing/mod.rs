//! Semantic index build and ledger persistence.
//!
//! ## Responsibilities
//! - build or clear the semantic sidecar for one embedding provider
//! - keep the AI index ledger and unified `runs` trace in sync with build outcomes
//! - collect changed canonical visits and write compatibility metadata rows
//! - expose storage/watermark helpers consumed by AI read models
//!
//! ## Not responsible for
//! - assistant prompt orchestration or semantic result ranking
//! - provider client setup and request-format-specific embedding execution
//! - Settings-facing read-model assembly
//!
//! ## Module layout (W-AI-4 indexing split)
//! This is the orchestration root; the load-bearing logic lives in focused submodules so the
//! crash/resume dedup state and the SQLite/candidate plumbing are each cohesive and directly tested:
//! - [`backfill`] — the resumable embed loop ([`run_embedding_backfill`]) and the
//!   [`backfill::DedupTracker`] that encapsulates the W-AI-4c crash-window invariants.
//! - [`candidates`] — candidate collection + the pure embed-target selection (MEDIUM-4 dedup).
//! - [`store_rows`] — SQLite `ai_embeddings` compatibility-row helpers + batch validation.
//! - [`content`] — the embedding-content rendering + the enrichment→dedup-hash seam.
//!
//! ## Dependencies
//! - `super::control` for cooperative cancellation checkpoints
//! - `super::provider` for embedding retries and provider validation
//! - `crate::ai_sidecar` for optional vector sidecar synchronization
//!
//! ## Performance notes
//! - candidate collection diffs content hashes before embedding, which avoids re-embedding
//!   unchanged visits on large archives
//! - SQLite lookups chunk `history_id` predicates with `SQLITE_BATCH_SIZE` to keep
//!   statement size and memory bounded on 14.4M+ histories

#[cfg(test)]
use super::*;

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
pub use self::backfill::{build_ai_index, build_ai_index_with_control};
pub(super) use self::store_rows::provider_embedding_count;

#[cfg(test)]
pub(super) use self::candidates::{
    collect_stale_history_ids, collect_visit_chunk, select_embed_targets,
};
#[cfg(test)]
pub(super) use self::content::build_embedding_content;
#[cfg(test)]
pub(super) use self::store_rows::{
    chunk_size, cleanup_stale_embeddings, clear_provider_embeddings, upsert_embedding,
    validate_embedding_batch_for_keys,
};

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
    /// DISTINCT from `indexed_items`, which counts changed VISITS (each gets a SQLite row + map
    /// entry). A re-visit of an already-embedded page bumps `indexed_items` but appends NO vector, so
    /// `embedded_vectors == 0` with `indexed_items > 0` is legitimate ONLY when the store already
    /// holds the page's vector. The build's honesty note reports this real count rather than implying
    /// every counted row produced a vector.
    pub embedded_vectors: u64,
}

/// Collects the canonical visits that need fresh embeddings for the selected provider.
///
/// Legacy recency-ordered (DESC) collector kept for the existing change-detection/cleanup unit
/// tests. The production backfill uses [`candidates::collect_visit_chunk`] (ascending, resumable from
/// a cursor); this DESC variant is no longer on the production path. It lives here in the
/// test-support surface so `ai/tests.rs` can reach it via `indexing::collect_visits_to_index`.
#[cfg(test)]
pub(super) fn collect_visits_to_index(
    paths: &ProjectPaths,
    connection: &Connection,
    provider: &AiProviderRuntime,
    limit: Option<u32>,
) -> Result<Vec<IndexedVisit>> {
    let limit_sql = limit.unwrap_or(0).max(1);
    let sql = if limit.is_some() {
        "SELECT visits.id,
                source_profiles.profile_key,
                urls.url,
                urls.title,
                (visits.visit_time_ms * 1000 + 11644473600000000) AS visit_time
         FROM archive.visits AS visits
         JOIN archive.urls AS urls ON urls.id = visits.url_id
         JOIN archive.source_profiles AS source_profiles ON source_profiles.id = visits.source_profile_id
         WHERE visits.reverted_at IS NULL
         ORDER BY visits.visit_time_ms DESC
         LIMIT ?1"
    } else {
        "SELECT visits.id,
                source_profiles.profile_key,
                urls.url,
                urls.title,
                (visits.visit_time_ms * 1000 + 11644473600000000) AS visit_time
         FROM archive.visits AS visits
         JOIN archive.urls AS urls ON urls.id = visits.url_id
         JOIN archive.source_profiles AS source_profiles ON source_profiles.id = visits.source_profile_id
         WHERE visits.reverted_at IS NULL
         ORDER BY visits.visit_time_ms DESC"
    };

    let mut statement = connection.prepare(sql)?;
    let mut rows =
        if limit.is_some() { statement.query(params![limit_sql])? } else { statement.query([])? };

    let mut raw_visits = Vec::new();
    while let Some(row) = rows.next()? {
        raw_visits.push(IndexedVisit {
            history_id: row.get(0)?,
            profile_id: row.get(1)?,
            url: row.get(2)?,
            title: row.get(3)?,
            domain: String::new(),
            visited_at: crate::utils::chrome_time_to_rfc3339(row.get::<_, i64>(4)?),
            content: String::new(),
            content_hash: String::new(),
            content_key: 0,
            needs_embedding: true,
        });
    }

    let history_ids = raw_visits.iter().map(|visit| visit.history_id).collect::<Vec<_>>();
    let existing_hashes =
        candidates::load_existing_embedding_hashes(connection, provider, &history_ids)?;
    let enrichments = load_best_enrichment_map_by_history_ids(paths, connection, &history_ids)?;
    let mut visits = Vec::with_capacity(raw_visits.len());
    for mut visit in raw_visits {
        let enrichment = enrichments.get(&visit.history_id);
        let content = build_embedding_content_from_parts(
            &visit.profile_id,
            &visit.url,
            visit.title.as_deref(),
            &visit.visited_at,
            enrichment.and_then(|value| value.readable_title.as_deref()),
            content::enrichment_summary_for(enrichment),
            enrichment.and_then(|value| value.readable_text.as_deref()),
        );
        let content_hash = super::dedup::build_dedup_content_hash(
            &visit.url,
            visit.title.as_deref(),
            content::enrichment_summary_for(enrichment),
        );
        if existing_hashes.get(&visit.history_id) == Some(&content_hash) {
            continue;
        }
        visit.domain = url_domain(&visit.url);
        visit.content = content;
        visit.content_key = super::dedup::content_key_from_hash(&content_hash);
        visit.content_hash = content_hash;
        visits.push(visit);
    }
    Ok(visits)
}
