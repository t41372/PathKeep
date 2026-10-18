//! Responsibilities: persist distinct keyword searches and publish their int8 projection.
//! Not responsible for: archive schema, lexical families, or page/visit maps.
//! Failure modes: resumed chunks repeat text; hidden generations leak; model fingerprints mix;
//! prompts change the search text; cancellation leaves a completed chunk unrecorded.
//! Tests: semantic-index.spec.ts observes exact inputs across passes and archive visibility.
use super::{
    AiProviderRuntime, AiRunControl, EmbeddingDescriptor, EmbeddingFingerprint, EmbeddingProvider,
    EmbeddingRole, FlatVectorIndex, VectorStore,
};
use crate::{Vault, config::ProjectPaths};
use anyhow::{Context, Result, ensure};
use std::sync::Arc;
use vault_store::{Db, params_owned as params};

// Only file names use this namespace. Headers keep the actual serving model fingerprint.
fn identity(provider: &AiProviderRuntime) -> String {
    format!("searches:{}", provider.config.id)
}

fn fingerprint(
    provider: &AiProviderRuntime,
    descriptor: EmbeddingDescriptor,
    dim: usize,
) -> Result<EmbeddingFingerprint> {
    EmbeddingFingerprint::from_descriptor(&EmbeddingDescriptor {
        provider_id: provider.config.id.clone(),
        model_id: provider.config.default_model.clone(),
        effective_dim: Some(dim),
        ..descriptor
    })
    .context("search embedding descriptor has no dimension")
}

/// A query has the same storage identity across engines, pages and index passes.
pub(super) fn key(query: &str) -> u64 {
    super::dedup::content_key_from_hash(&crate::utils::sha256_hex(query.as_bytes()))
}

/// The short-word rule is shared by the report and drill-in, including supplementary CJK.
pub(super) fn is_short(query: &str) -> bool {
    let mut characters = query.chars().filter(|character| !character.is_whitespace());
    let Some(first) = characters.next() else {
        return true;
    };
    let second = characters.next();
    if characters.next().is_some() {
        return false;
    }
    if second.is_none()
        && (crate::visit_taxonomy::is_cjk_like(first) || matches!(first as u32, 0x20000..=0x323af))
    {
        return true;
    }
    static LATIN: std::sync::LazyLock<regex::Regex> = std::sync::LazyLock::new(|| {
        regex::Regex::new(r"^[\p{Latin}\p{Nd}\s]+$").expect("literal short-search pattern")
    });
    LATIN.is_match(query)
}

/// Ask and drill-in must use exactly the same serving-index eligibility as period topics.
pub(super) fn available(
    paths: &ProjectPaths,
    provider: &AiProviderRuntime,
) -> Result<FlatVectorIndex> {
    super::validate_provider(provider, crate::AiProviderPurpose::Embedding)?;
    let index = FlatVectorIndex::open(paths, &provider.config.id, &provider.config.default_model)?;
    ensure!(!index.is_empty(), "the semantic index is empty; build it in Settings → AI");
    let engine = super::embedding_engine::select_embedding_provider(paths, provider)?;
    ensure!(
        super::search::semantic_config_drift_reason(paths, provider, &engine, &index, index.dim())?
            .is_none(),
        "the semantic index has drifted from this model; rebuild it in Settings → AI"
    );
    Ok(index)
}

/// Missing or stale search projections count as not indexed, never as another model's vectors.
pub(super) fn open(
    paths: &ProjectPaths,
    provider: &AiProviderRuntime,
    dim: usize,
) -> Result<FlatVectorIndex> {
    let index = FlatVectorIndex::open(paths, &identity(provider), &provider.config.default_model)?;
    if !index.is_empty() {
        let engine = super::embedding_engine::select_embedding_provider(paths, provider)?;
        ensure!(
            index.dim() == dim
                && index.fingerprint_hash()
                    == fingerprint(provider, engine.descriptor(), dim)?.hash(),
            "the search vectors have drifted from the serving model; update the index in Settings → AI"
        );
    }
    Ok(index)
}

/// A completed chunk is its own resume cursor; only missing keys reach the embedding engine.
pub(super) async fn backfill(
    vault: &Vault,
    provider: &AiProviderRuntime,
    control: Option<&Arc<dyn AiRunControl>>,
) -> Result<usize> {
    let paths = vault.paths();
    let engine = super::embedding_engine::select_embedding_provider(paths, provider)?;
    let store =
        VectorStore::for_provider(paths, &identity(provider), &provider.config.default_model);
    if let Some(header) = store.read_header()? {
        let fp = fingerprint(provider, engine.descriptor(), header.dim)?;
        if header.fingerprint_hash != fp.hash() {
            delete(paths, provider)?;
        }
    }
    if store.exists() {
        store.trim_torn_tail()?;
    }
    let mut existing = if store.exists() { store.existing_ids()? } else { Default::default() };
    let page_fingerprint =
        VectorStore::for_provider(paths, &provider.config.id, &provider.config.default_model)
            .read_header()?
            .map(|header| header.fingerprint_hash);
    let mut after = String::new();
    let mut embedded = 0;
    loop {
        super::control::checkpoint_ai_run(control, "reading keyword searches")?;
        let cursor = after.clone();
        // Insights can still serve an undone import until its background rebuild finishes.
        let texts = vault.read(Db::Intelligence, move |session| {
            Ok(session.query(
                "SELECT e.normalized_query
                 FROM search_events_rows e INDEXED BY idx_search_events_query_time
                 CROSS JOIN archive.visits v ON v.id=e.visit_id
                 WHERE e.query_kind='keyword' AND e.normalized_query>?1 AND v.reverted_at IS NULL
                   AND e.generation IN (SELECT navigation FROM intelligence_generations)
                 GROUP BY e.normalized_query ORDER BY e.normalized_query LIMIT 64",
                params![cursor],
                |row| Ok(row.get::<String>(0)?),
            )?)
        })?;
        if texts.is_empty() {
            break;
        }
        after = texts.last().unwrap().clone();
        let texts =
            texts.into_iter().filter(|text| !existing.contains(&key(text))).collect::<Vec<_>>();
        if texts.is_empty() {
            continue;
        }
        let vectors = super::control::await_with_ai_cancellation(
            control,
            "embedding keyword searches",
            engine.embed(&texts, EmbeddingRole::Document),
        )
        .await?;
        let keys = texts.iter().map(|text| key(text)).collect::<Vec<_>>();
        let (dim, records) = super::indexing::validate_embedding_batch_for_keys(&keys, &vectors)?;
        let fp = fingerprint(provider, engine.descriptor(), dim)?;
        // A service changing dimensions between page and search batches cannot publish one index.
        ensure!(
            page_fingerprint.as_ref().is_none_or(|hash| *hash == fp.hash()),
            "the embedding model changed between page and search backfills; rebuild the index"
        );
        if !store.exists() {
            VectorStore::for_provider(paths, &identity(provider), &provider.config.default_model)
                .create_stamped_at(paths, &fp, "normalized-query-v1".into())?;
        }
        ensure!(
            !store.is_stale_against(&fp)?,
            "search embedding dimensions changed during the index pass"
        );
        store.append_vectors(&records)?;
        existing.extend(keys);
        embedded += records.len();
    }
    super::control::checkpoint_ai_run(control, "publishing keyword search vectors")?;
    if store.exists() {
        super::vector_planes::build_planes_from_store(
            paths,
            &identity(provider),
            &provider.config.default_model,
        )?;
    }
    Ok(embedded)
}

/// Search sidecars follow the lifetime of their owning page index, including clear and retirement.
pub(super) fn delete(paths: &ProjectPaths, provider: &AiProviderRuntime) -> Result<()> {
    VectorStore::for_provider(paths, &identity(provider), &provider.config.default_model)
        .delete()?;
    super::vector_planes::delete_planes(
        paths,
        &identity(provider),
        &provider.config.default_model,
    )?;
    Ok(())
}
