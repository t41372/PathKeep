//! Responsibilities: rank full hybrid results using the selected serving index and its engine.
//! Not responsible for: embedding queues, canonical writes or caching materialized result rows.
//! Failure modes: newest pools hide old matches, facets trim a finite recall pool,
//! duplicate visits inflate ranks, strings consume corpus-sized RAM, cursors rerank on writes.
//! Tests: AI RRF/complete-pool suites, code-mode and MCP search contracts.
use super::search::{
    history_entry_to_search_entry, resolve_date_filter_ms, reverse_sidecars_usable,
    semantic_config_drift_reason,
};
use super::{
    AiProviderRuntime, EmbeddingRole, FlatVectorIndex, ReverseVisitMap, VisitContentMap,
    embed_query,
};
use crate::{
    AiSearchNote, AiSearchRequest, AiSearchResponse, AppConfig, ProjectPaths, Vault,
    search::{Scores, SearchRequest, SearchSession},
};
use anyhow::Result;
use std::collections::HashMap;

#[derive(Default)]
struct Ranks {
    lexical: u64,
    semantic: u64,
    semantic_score: Option<f32>,
    newest: i64,
    starred: bool,
}

pub(super) fn query(request: &AiSearchRequest) -> SearchRequest {
    let (start_time_ms, end_time_ms) = resolve_date_filter_ms(request);
    SearchRequest {
        query: request.query.clone(),
        sort: request.sort.clone(),
        page_size: request.limit.unwrap_or(8).max(1) as usize,
        profile_id: request.profile_id.clone(),
        browser_kind: request.browser_kind.clone(),
        domain: request.domain.clone(),
        start_time_ms,
        end_time_ms,
        starred: request.starred_only.unwrap_or(false),
        ..Default::default()
    }
}
pub(super) async fn page(
    vault: &Vault,
    paths: &ProjectPaths,
    config: &AppConfig,
    provider: Option<&AiProviderRuntime>,
    request: &AiSearchRequest,
) -> Result<AiSearchResponse> {
    let mut query = query(request);
    query.ranking_key = Some(serde_json::to_string(&(
        provider.map(|p| (&p.config.id, &p.config.default_model)),
        config.ai.hybrid_rrf_k,
        config.ai.lexical_weight,
        config.ai.semantic_weight,
        config.ai.starred_boost,
    ))?);
    let step = if let Some(cursor) = request.cursor.as_deref() {
        crate::search::page(vault, query, Some(cursor))?
    } else {
        let mut session = SearchSession::start(vault, query.clone())?;
        let cancel = session.cancel_token()?;
        let lexical = session.collect_matches()?;
        let mut ranks = HashMap::<u64, Ranks>::with_capacity(lexical.len());
        for (rank, hit) in lexical.into_iter().enumerate() {
            ranks.insert(
                hit.page_id,
                Ranks { lexical: rank as u64 + 1, newest: hit.visit_ms, ..Default::default() },
            );
        }
        let mut notes = Vec::new();
        if let Some(provider) = provider {
            let (semantic_notes, read_plane) =
                add_semantic(vault, paths, provider, &query, request, &cancel, None, &mut ranks)
                    .await?;
            notes = semantic_notes;
            let serving = crate::EmbeddingIndexRef {
                provider_id: provider.config.id.clone(),
                model: provider.config.default_model.clone(),
            };
            if read_plane && crate::semantic_index::chosen(config).as_ref() != Some(&serving) {
                notes.push(AiSearchNote::PreviousModel { model: serving.model });
            }
        } else {
            notes.push(AiSearchNote::LexicalFallbackNoProvider);
        }
        cancel.check()?;
        let ids = ranks.keys().copied().collect::<Vec<_>>();
        for (id, newest) in ids.iter().copied().zip(session.canonical_times(&ids)?) {
            ranks.get_mut(&id).unwrap().newest = newest;
        }
        let mut semantic = ranks
            .iter()
            .filter_map(|(id, r)| r.semantic_score.map(|score| (*id, score, r.newest)))
            .collect::<Vec<_>>();
        semantic
            .sort_unstable_by(|a, b| b.1.total_cmp(&a.1).then(b.2.cmp(&a.2)).then(b.0.cmp(&a.0)));
        for (rank, (id, _, _)) in semantic.into_iter().enumerate() {
            ranks.get_mut(&id).unwrap().semantic = rank as u64 + 1;
        }
        if config.ai.starred_boost > 0. {
            for page in crate::search::starred_pages(vault, &ids)? {
                ranks.get_mut(&page).unwrap().starred = true;
            }
        }
        let k = config.ai.hybrid_rrf_k.max(1) as f32;
        let score = |r: &Ranks| {
            (if r.lexical == 0 {
                0.
            } else {
                config.ai.lexical_weight / (k + (r.lexical - 1) as f32)
            }) + (if r.semantic == 0 {
                0.
            } else {
                config.ai.semantic_weight / (k + (r.semantic - 1) as f32)
            })
        };
        let max = ranks.values().map(score).fold(0f32, f32::max);
        let scale = if max > 0. { 1. / max } else { 1. };
        let scores = ranks
            .into_iter()
            .map(|(id, r)| {
                let boosted = r.starred && config.ai.starred_boost > 0.;
                let value = score(&r) * scale + if boosted { config.ai.starred_boost } else { 0. };
                let reason = match (r.lexical > 0, r.semantic > 0, boosted) {
                    (true, true, true) => "lexical+semantic+starred",
                    (true, true, false) => super::search::MATCH_REASON_LEXICAL_SEMANTIC,
                    (false, true, true) => "semantic+starred",
                    (false, true, false) => super::search::MATCH_REASON_SEMANTIC,
                    (_, false, true) => "lexical+starred",
                    _ => "lexical",
                };
                (id, (value, reason))
            })
            .collect::<Scores>();
        session.install_scores(scores)?;
        session.set_search_notes(notes)?;
        crate::search::page_prepared(vault, query, session)?
    };
    let note_codes = step.search_notes.clone();
    Ok(AiSearchResponse {
        search_state: Some(step.coverage()),
        total: step.count.value as usize,
        provider_id: provider.map_or("lexical-fallback", |p| p.config.id.as_str()).into(),
        model: provider.map_or("none", |p| p.config.default_model.as_str()).into(),
        items: step
            .items
            .iter()
            .map(|hit| {
                history_entry_to_search_entry(
                    &hit.visit,
                    hit.score,
                    hit.reason.as_deref().unwrap_or("lexical"),
                )
            })
            .collect(),
        notes: note_codes.iter().map(AiSearchNote::model_facing_text).collect(),
        note_codes,
        has_more: step.cursor.is_some(),
        next_cursor: step.cursor,
        applied_limit: Some(request.limit.unwrap_or(8).max(1)),
    })
}
fn hydrate_scores(
    vault: &Vault,
    paths: &ProjectPaths,
    provider: &AiProviderRuntime,
    scope: &SearchRequest,
    scores: &HashMap<u64, f32>,
    cancel: &vault_store::CancelToken,
    ranks: &mut HashMap<u64, Ranks>,
) -> Result<()> {
    let mut pending = Vec::<(i64, f32)>::with_capacity(512);
    let mut consume = |id: i64, key: u64| -> Result<()> {
        cancel.check()?;
        if let Some(score) = scores.get(&key) {
            pending.push((id, *score));
        }
        if pending.len() == 512 {
            flush(vault, scope, &mut pending, ranks)?;
        }
        Ok(())
    };
    if reverse_sidecars_usable(paths, provider)? {
        ReverseVisitMap::for_provider(paths, &provider.config.id, &provider.config.default_model)
            .visit_forward(cancel, &mut consume)?;
    } else {
        // The authoritative map resolves remapped visits with last-writer-wins semantics.
        // Its fallback stores only fixed-width IDs, never a corpus of hydrated text.
        for (id, key) in VisitContentMap::for_provider(
            paths,
            &provider.config.id,
            &provider.config.default_model,
        )
        .read_all()?
        {
            consume(id, key)?;
        }
    }
    flush(vault, scope, &mut pending, ranks)
}
fn flush(
    vault: &Vault,
    scope: &SearchRequest,
    pending: &mut Vec<(i64, f32)>,
    ranks: &mut HashMap<u64, Ranks>,
) -> Result<()> {
    let ids = pending.iter().map(|(id, _)| *id).collect::<Vec<_>>();
    let scores = pending.drain(..).collect::<HashMap<_, _>>();
    for hit in crate::search::evidence(vault, &ids, scope)? {
        let candidate = ranks.entry(hit.page_id as u64).or_default();
        let score = scores[&hit.visit.id];
        candidate.semantic_score =
            Some(candidate.semantic_score.map_or(score, |old| old.max(score)));
        candidate.newest = candidate.newest.max(hit.visit.visit_time);
    }
    Ok(())
}

async fn add_semantic(
    vault: &Vault,
    paths: &ProjectPaths,
    provider: &AiProviderRuntime,
    query: &SearchRequest,
    request: &AiSearchRequest,
    cancel: &vault_store::CancelToken,
    allowlist: Option<&[u64]>,
    ranks: &mut HashMap<u64, Ranks>,
) -> Result<(Vec<AiSearchNote>, bool)> {
    let mut notes = Vec::new();
    super::validate_provider(provider, crate::AiProviderPurpose::Embedding)?;
    if let Some(reason) =
        super::search_store::staleness(vault, &provider.config.id, &provider.config.default_model)?
    {
        notes.push(AiSearchNote::Stale { reason });
    }
    let index = FlatVectorIndex::open(paths, &provider.config.id, &provider.config.default_model)?;
    if index.is_empty() {
        notes.push(AiSearchNote::EmptySemanticIndex);
        return Ok((notes, false));
    }
    let engine = super::embedding_engine::select_embedding_provider(paths, provider)?;
    let vector = embed_query(paths, provider, request.query.trim(), EmbeddingRole::Query).await?;
    if let Some(reason) =
        semantic_config_drift_reason(paths, provider, &engine, &index, vector.len())?
    {
        notes.push(reason);
        return Ok((notes, false));
    }
    let allowlist =
        allowlist.map(|keys| keys.iter().copied().collect::<std::collections::HashSet<_>>());
    let mut scores = HashMap::with_capacity(index.len());
    index.visit_scores(&vector, cancel, |key, score| {
        if allowlist.as_ref().is_none_or(|keys| keys.contains(&key)) {
            scores.insert(key, score);
        }
        Ok(())
    })?;
    hydrate_scores(vault, paths, provider, query, &scores, cancel, ranks)?;
    if !ranks.values().any(|r| r.semantic_score.is_some()) {
        notes.push(AiSearchNote::SemanticMatchesFilteredOut);
    }
    Ok((notes, true))
}
#[cfg(test)]
pub(super) async fn semantic_page(
    vault: &Vault,
    paths: &ProjectPaths,
    provider: &AiProviderRuntime,
    request: &AiSearchRequest,
    allowlist: Option<&[u64]>,
) -> Result<super::search::SemanticMatchReport> {
    let query = query(request);
    let mut session = SearchSession::start(vault, query.clone())?;
    let cancel = session.cancel_token()?;
    let mut ranks = HashMap::new();
    let (notes, _) =
        add_semantic(vault, paths, provider, &query, request, &cancel, allowlist, &mut ranks)
            .await?;
    let ids = ranks.keys().copied().collect::<Vec<_>>();
    session.canonical_times(&ids)?;
    session.install_scores(
        ranks.into_iter().map(|(id, r)| (id, (r.semantic_score.unwrap(), "semantic"))).collect(),
    )?;
    let page = session.finish_page()?;
    Ok(super::search::SemanticMatchReport {
        hits: page
            .items
            .iter()
            .map(|hit| history_entry_to_search_entry(&hit.visit, hit.score, "semantic"))
            .collect(),
        notes,
    })
}
