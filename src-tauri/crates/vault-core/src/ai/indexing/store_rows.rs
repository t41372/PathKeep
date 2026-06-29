//! SQLite `ai_embeddings` compatibility-row helpers + embed-batch shape validation.
//!
//! ## Responsibilities
//! - upsert / count / clear / stale-prune the lightweight `ai_embeddings` provenance rows that the
//!   read models report (the vectors themselves live ONLY on the `derived/vectors/` plane, 02 §A)
//! - validate one returned embedding batch against its content keys before it reaches the store
//! - size each resumable backfill chunk against any remaining caller limit
//!
//! ## Not responsible for
//! - the vector store / visit map I/O (`super::super::vector_store` / `visit_content_map` own those)
//! - candidate collection or the embed loop (sibling submodules own those)

use super::super::*;
use super::IndexedVisit;

/// Validates one embedded batch against its CONTENT KEYS and pairs each vector with its key (W-AI-4c).
///
/// Pure (no I/O) so the defensive guards are unit-tested directly:
/// - rejects a count mismatch (a short/over-long batch would desync the key↔vector join),
/// - rejects a ragged/empty (zero-dim) batch (a vector of a different dim than the first), since the
///   vector store is fixed-stride and a ragged record would corrupt every later read, and
/// - rejects an ALL-ZERO batch (F3 0-byte honesty): a provider that "succeeds" but returns only zero
///   vectors has produced NO usable embedding — counting it would land dead bytes in `.pkvec` while
///   the run reports rows indexed. This is exactly the dishonest "indexed N with an empty/zero store"
///   case, so it is surfaced as a real error instead of being silently appended + counted.
///
/// Returns the effective dim (the actual returned length of the first vector, D4) and the
/// `(content_key, vector)` records ready to append. With dedup the keys are the per-page content
/// keys (one per unique page), not visit ids — the loop deduped before calling this, so each key
/// here is distinct.
pub(in crate::ai) fn validate_embedding_batch_for_keys(
    content_keys: &[u64],
    vectors: &[Vec<f32>],
) -> Result<(usize, Vec<(u64, Vec<f32>)>)> {
    if vectors.len() != content_keys.len() {
        anyhow::bail!(
            "embedding provider returned {} vector(s) for {} input(s)",
            vectors.len(),
            content_keys.len()
        );
    }
    let effective_dim = vectors.first().map(Vec::len).unwrap_or_default();
    if effective_dim == 0 {
        anyhow::bail!("embedding provider returned an empty vector for the batch");
    }
    let mut records = Vec::with_capacity(content_keys.len());
    for (content_key, vector) in content_keys.iter().zip(vectors.iter()) {
        if vector.len() != effective_dim {
            anyhow::bail!(
                "embedding provider returned a ragged batch (content_key {content_key} dim {} vs {effective_dim})",
                vector.len()
            );
        }
        records.push((*content_key, vector.clone()));
    }
    // F3 (0-byte honesty): a batch where EVERY vector is all-zero carries no signal — appending it
    // would write dead bytes the index can never match while the run dishonestly reports rows indexed.
    if records.iter().all(|(_, vector)| vector.iter().all(|component| *component == 0.0)) {
        anyhow::bail!(
            "embedding provider returned only zero vectors for the batch (no usable embedding); refusing to count an empty index build"
        );
    }
    Ok((effective_dim, records))
}

/// Returns the chunk size for one pass, clamped by any remaining caller `limit`.
///
/// `EMBEDDING_BACKFILL_CHUNK` is a positive constant, so `clamp(1, CHUNK)` never inverts its
/// bounds (it would only panic if `max < min`).
pub(in crate::ai) fn chunk_size(remaining: Option<usize>) -> usize {
    match remaining {
        Some(cap) => cap.clamp(1, EMBEDDING_BACKFILL_CHUNK),
        None => EMBEDDING_BACKFILL_CHUNK,
    }
}

/// Deletes SQLite compatibility rows whose canonical history ids are no longer visible.
pub(in crate::ai) fn cleanup_stale_embeddings(
    connection: &Connection,
    provider: &AiProviderRuntime,
) -> Result<usize> {
    #[rustfmt::skip]
    let removed = connection.execute(
        DELETE_STALE_EMBEDDINGS_SQL,
        params![provider.config.id, provider.config.default_model],
    )?;
    Ok(removed)
}

/// Counts the current number of SQLite compatibility rows for one provider/model pair.
pub(in crate::ai) fn provider_embedding_count(
    connection: &Connection,
    provider_id: &str,
    model: &str,
) -> Result<i64> {
    #[rustfmt::skip]
    let count = connection.query_row(
        "SELECT COUNT(*) FROM ai_embeddings WHERE provider_id = ?1 AND model = ?2",
        params![provider_id, model],
        |row: &Row<'_>| row.get::<_, i64>(0),
    )?;
    Ok(count)
}

/// Clears all SQLite compatibility rows for one provider/model pair.
pub(in crate::ai) fn clear_provider_embeddings(
    connection: &Connection,
    provider: &AiProviderRuntime,
) -> Result<()> {
    #[rustfmt::skip]
    connection.execute(
        CLEAR_PROVIDER_EMBEDDINGS_SQL,
        params![provider.config.id, provider.config.default_model],
    )?;
    Ok(())
}

/// Upserts one SQLite compatibility row after a semantic embedding was produced.
///
/// Returns whether a metadata row for this `(history_id, provider, model)` already existed BEFORE
/// the write, so the backfill can count re-embeds (changed content) separately from first-time
/// embeds. The row keeps the lightweight provenance the read models report; the vector itself
/// lives only on the `derived/vectors/` plane, never in SQLite (02 §A).
pub(in crate::ai) fn upsert_embedding(
    connection: &Connection,
    provider: &AiProviderRuntime,
    visit: &IndexedVisit,
    indexed_at: &str,
) -> Result<bool> {
    let prior_rows: i64 = connection.query_row(
        "SELECT COUNT(*) FROM ai_embeddings
         WHERE history_id = ?1 AND provider_id = ?2 AND model = ?3",
        params![visit.history_id, provider.config.id, provider.config.default_model],
        |row: &Row<'_>| row.get(0),
    )?;
    let existed = prior_rows > 0;
    // Clear any prior row for this (history_id, provider, model) so a content-hash change does not
    // leave a duplicate UNIQUE(history_id, provider, model, content_hash) row behind.
    connection.execute(
        "DELETE FROM ai_embeddings
         WHERE history_id = ?1 AND provider_id = ?2 AND model = ?3",
        params![visit.history_id, provider.config.id, provider.config.default_model],
    )?;
    #[rustfmt::skip]
    connection.execute(
        UPSERT_EMBEDDING_SQL,
        params![
            visit.history_id,
            visit.profile_id,
            visit.url,
            visit.title,
            visit.domain,
            visit.visited_at,
            visit.content_hash,
            visit.content.len() as i64,
            provider.config.id,
            provider.config.default_model,
            indexed_at
        ],
    )?;
    Ok(existed)
}
