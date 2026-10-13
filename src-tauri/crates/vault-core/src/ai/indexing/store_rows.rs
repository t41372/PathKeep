//! Native embedding metadata and embed-batch validation.
//! Failure modes: metadata totals drift on replacement or rollback, cleanup monopolizes a writer,
//! stale rows remain indexed, or a returned vector batch loses its input alignment.
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
use vault_store::{Session, params_owned as params};

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

/// Clears or prunes metadata with one bounded writer job at a time.
fn remove_metadata(
    vault: &crate::Vault,
    provider: &AiProviderRuntime,
    all: bool,
    control: Option<&Arc<dyn AiRunControl>>,
) -> Result<usize> {
    let mut after = 0;
    let mut removed = 0;
    let upper = vault.read(vault_store::Db::Intelligence, |s| {
        Ok(s.query_one("SELECT COALESCE(MAX(id),0) FROM ai_embeddings", (), |r| {
            Ok(r.get::<i64>(0)?)
        })?)
    })?;
    loop {
        checkpoint_ai_run(control, "Index cleanup was cancelled between batches.")?;
        let provider = provider.clone();
        let (last,count)=vault.write(vault_store::Db::Intelligence,vault_store::Priority::Batch,move |s| {
            let rows=s.query("SELECT id,content_bytes,metadata_bytes,CASE WHEN ?5=1 THEN 1 ELSE NOT EXISTS(SELECT 1 FROM archive.visits WHERE id=ai_embeddings.history_id AND reverted_at IS NULL LIMIT 1 OFFSET 0) END FROM ai_embeddings INDEXED BY idx_ai_embeddings_provider_id WHERE provider_id=?1 AND model=?2 AND id>?3 AND id<=?4 ORDER BY id LIMIT 512",params![provider.config.id,provider.config.default_model,after,upper,i64::from(all)],|r|Ok((r.get::<i64>(0)?,r.get::<i64>(1)?,r.get::<i64>(2)?,r.get::<i64>(3)?!=0)))?;
            let last=rows.last().map(|r|r.0);let mut count=0;let mut content=0;let mut metadata=0;
            let mut delete=s.prepare("DELETE FROM ai_embeddings WHERE id=?1")?;
            for (id,bytes,meta,remove) in rows {if remove {delete.execute([id])?;count+=1;content+=bytes;metadata+=meta;}}
            drop(delete);
            if count>0 {s.execute("UPDATE ai_embedding_totals SET last_indexed_at=CASE WHEN item_count-?3=0 THEN NULL ELSE last_indexed_at END,item_count=item_count-?3,content_bytes=content_bytes-?4,metadata_bytes=metadata_bytes-?5 WHERE provider_id=?1 AND model=?2",params![provider.config.id,provider.config.default_model,count,content,metadata])?;}
            Ok((last,count as usize))
        })?;
        removed += count;
        let Some(last) = last else {
            break;
        };
        after = last;
    }
    Ok(removed)
}
pub(in crate::ai) fn clear_provider_embeddings(
    vault: &crate::Vault,
    provider: &AiProviderRuntime,
    control: Option<&Arc<dyn AiRunControl>>,
) -> Result<usize> {
    remove_metadata(vault, provider, true, control)
}
pub(in crate::ai) fn cleanup_stale_embeddings(
    vault: &crate::Vault,
    provider: &AiProviderRuntime,
    control: Option<&Arc<dyn AiRunControl>>,
) -> Result<usize> {
    remove_metadata(vault, provider, false, control)
}

/// Upserts one SQLite compatibility row after a semantic embedding was produced.
///
/// Returns whether a metadata row for this `(history_id, provider, model)` already existed BEFORE
/// the write, so the backfill can count re-embeds (changed content) separately from first-time
/// embeds. The row keeps the lightweight provenance the read models report; the vector itself
/// lives only on the `derived/vectors/` plane, never in Turso (02 §A).
pub(in crate::ai) fn upsert_embedding(
    connection: &Session<'_>,
    provider: &AiProviderRuntime,
    visit: &IndexedVisit,
    indexed_at: &str,
) -> Result<bool> {
    let (prior_rows,prior_content,prior_metadata): (i64,i64,i64) = connection.query_one(
        "SELECT COUNT(*),COALESCE(SUM(content_bytes),0),COALESCE(SUM(metadata_bytes),0) FROM ai_embeddings
         WHERE history_id = ?1 AND provider_id = ?2 AND model = ?3",
        params![visit.history_id, provider.config.id, provider.config.default_model],
        |row| Ok((row.get(0)?,row.get(1)?,row.get(2)?)),
    )?;
    let metadata_bytes = (visit.url.len()
        + visit.title.as_ref().map_or(0, String::len)
        + visit.domain.len()
        + visit.visited_at.len()
        + visit.content_hash.len()
        + provider.config.id.len()
        + provider.config.default_model.len()
        + 8) as i64;
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
            indexed_at,
            metadata_bytes
        ],
    )?;
    connection.execute("INSERT INTO ai_embedding_totals(provider_id,model,item_count,content_bytes,metadata_bytes,last_indexed_at) VALUES(?1,?2,?3,?4,?5,?6) ON CONFLICT(provider_id,model) DO UPDATE SET item_count=item_count+excluded.item_count,content_bytes=content_bytes+excluded.content_bytes,metadata_bytes=metadata_bytes+excluded.metadata_bytes,last_indexed_at=excluded.last_indexed_at",params![provider.config.id,provider.config.default_model,1-prior_rows,visit.content.len() as i64-prior_content,metadata_bytes-prior_metadata,indexed_at])?;
    Ok(existed)
}
