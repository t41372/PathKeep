//! Responsibilities: rank every eligible lexical and semantic page, then retain dense fused scores.
//! Not responsible for: embedding queues, canonical writes or materialized result caching.
//! Failure modes: a filter applied after selection hides an eligible page; a visit outside the
//! scope lends its page a score; the query is embedded before an empty index is noticed;
//! per-visit or per-page SQL makes a large archive slow. Tests: AI complete-pool suites in
//! `ai/tests.rs`, `semantic_stage_bench` for cost.
use super::search::{
    history_entry_to_search_entry, resolve_date_filter_ms, semantic_config_drift_reason,
};
use super::{AiProviderRuntime, EmbeddingRole, FlatVectorIndex, embed_query};
use crate::{
    AiSearchNote, AiSearchRequest, AiSearchResponse, AppConfig, ProjectPaths, Vault,
    search::{Scores, SearchRequest, SearchSession, SearchStep},
};
use anyhow::Result;

struct Ranks {
    lexical: Vec<u32>,
    semantic_scores: Vec<f32>,
}
impl Ranks {
    fn new(length: usize) -> Self {
        Self { lexical: vec![0; length], semantic_scores: vec![f32::NAN; length] }
    }
}
struct SemanticScores {
    scores: Vec<f32>,
    positions: Vec<u32>,
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
fn ranked_query(
    config: &AppConfig,
    provider: Option<&AiProviderRuntime>,
    request: &AiSearchRequest,
) -> Result<SearchRequest> {
    let mut query = query(request);
    query.ranking_key = Some(serde_json::to_string(&(
        provider.map(|p| (&p.config.id, &p.config.default_model)),
        config.ai.hybrid_rrf_k,
        config.ai.lexical_weight,
        config.ai.semantic_weight,
        config.ai.starred_boost,
    ))?);
    Ok(query)
}

fn lexical_ranks(session: &mut SearchSession) -> Result<Ranks> {
    // Fuzzy fallback can restart the session, so resolve the semantic scope afterward.
    let lexical = session.collect_matches()?;
    let mut ranks = Ranks::new(session.with_newest(|times| times.len())?);
    for (rank, hit) in lexical.into_iter().enumerate() {
        ranks.lexical[hit.page_id as usize] = u32::try_from(rank + 1)?;
    }
    Ok(ranks)
}

/// Opens the serving index before the query is embedded, so an empty index never waits on a service.
fn semantic_index(
    vault: &Vault,
    paths: &ProjectPaths,
    provider: &AiProviderRuntime,
    notes: &mut Vec<AiSearchNote>,
) -> Result<Option<FlatVectorIndex>> {
    super::validate_provider(provider, crate::AiProviderPurpose::Embedding)?;
    if let Some(reason) =
        super::search_store::staleness(vault, &provider.config.id, &provider.config.default_model)?
    {
        notes.push(AiSearchNote::Stale { reason });
    }
    let index = FlatVectorIndex::open(paths, &provider.config.id, &provider.config.default_model)?;
    if index.is_empty() {
        notes.push(AiSearchNote::EmptySemanticIndex);
        return Ok(None);
    }
    Ok(Some(index))
}

fn semantic_scores(
    paths: &ProjectPaths,
    provider: &AiProviderRuntime,
    index: &FlatVectorIndex,
    vector: &[f32],
    cancel: &vault_store::CancelToken,
    allowlist: Option<&[u64]>,
    notes: &mut Vec<AiSearchNote>,
) -> Result<Option<SemanticScores>> {
    let engine = super::embedding_engine::select_embedding_provider(paths, provider)?;
    if let Some(reason) =
        semantic_config_drift_reason(paths, provider, &engine, index, vector.len())?
    {
        notes.push(reason);
        return Ok(None);
    }
    let mut scores = index.score_all(vector, cancel)?;
    if let Some(allowlist) = allowlist {
        let allowed = allowlist.iter().copied().collect::<std::collections::HashSet<_>>();
        for (position, score) in scores.iter_mut().enumerate() {
            if !allowed.contains(&index.key(position)) {
                *score = f32::NAN;
            }
        }
    }
    let positions = super::visit_position_map::load(
        paths,
        &provider.config.id,
        &provider.config.default_model,
        index,
        cancel,
    )?;
    Ok(Some(SemanticScores { scores, positions }))
}

/// Embeds the query and scores it when the serving index can answer it.
async fn query_semantic(
    vault: &Vault,
    paths: &ProjectPaths,
    provider: &AiProviderRuntime,
    query: &str,
    cancel: &vault_store::CancelToken,
    allowlist: Option<&[u64]>,
    notes: &mut Vec<AiSearchNote>,
) -> Result<Option<SemanticScores>> {
    let Some(index) = semantic_index(vault, paths, provider, notes)? else { return Ok(None) };
    let vector = embed_query(paths, provider, query.trim(), EmbeddingRole::Query).await?;
    semantic_scores(paths, provider, &index, &vector, cancel, allowlist, notes)
}

/// One pass over the search's own scope gives each page the best score of its in-scope visits.
fn scope_pass(
    session: &mut SearchSession,
    ranks: Ranks,
    semantic: SemanticScores,
) -> Result<Ranks> {
    let (ranks, _) =
        session.resolve_scope((ranks, semantic), |(ranks, semantic), visit, page| {
            let page = page as usize;
            if ranks.lexical.len() <= page {
                ranks.lexical.resize(page + 1, 0);
                ranks.semantic_scores.resize(page + 1, f32::NAN);
            }
            if let Some(position) = semantic.positions.get(visit as usize)
                && *position != super::visit_position_map::ABSENT
            {
                let score = semantic.scores[*position as usize];
                if !score.is_nan() {
                    let old = &mut ranks.semantic_scores[page];
                    *old = if old.is_nan() { score } else { old.max(score) };
                }
            }
        })?;
    Ok(ranks)
}

fn rank_and_fuse(
    vault: &Vault,
    config: &AppConfig,
    session: &SearchSession,
    ranks: Ranks,
) -> Result<Scores> {
    let mut semantic = ranks
        .semantic_scores
        .iter()
        .enumerate()
        .filter_map(|(id, score)| (!score.is_nan()).then_some(id as u32))
        .collect::<Vec<_>>();
    session.with_newest(|newest| {
        semantic.sort_unstable_by(|a, b| {
            let (a, b) = (*a as usize, *b as usize);
            ranks.semantic_scores[b]
                .total_cmp(&ranks.semantic_scores[a])
                .then(newest[b].cmp(&newest[a]))
                .then(b.cmp(&a))
        });
    })?;
    let mut scores =
        Scores { values: vec![0.; ranks.lexical.len()], reasons: vec![0; ranks.lexical.len()] };
    let k = config.ai.hybrid_rrf_k.max(1) as f32;
    for (id, rank) in ranks.lexical.into_iter().enumerate() {
        if rank != 0 {
            scores.values[id] = config.ai.lexical_weight / (k + (rank - 1) as f32);
            scores.reasons[id] = 1;
        }
    }
    for (rank, id) in semantic.into_iter().enumerate() {
        scores.values[id as usize] += config.ai.semantic_weight / (k + rank as f32);
        scores.reasons[id as usize] |= 2;
    }
    let max = scores.values.iter().copied().fold(0f32, f32::max);
    let scale = if max > 0. { 1. / max } else { 1. };
    for score in &mut scores.values {
        *score *= scale;
    }
    if config.ai.starred_boost > 0. {
        for id in crate::search::starred_pages(vault)? {
            if let Some(reason) = scores.reasons.get_mut(id as usize)
                && *reason != 0
            {
                *reason |= 4;
                scores.values[id as usize] += config.ai.starred_boost;
            }
        }
    }
    Ok(scores)
}

fn install_first_page(
    vault: &Vault,
    query: SearchRequest,
    mut session: SearchSession,
    scores: Scores,
    notes: Vec<AiSearchNote>,
) -> Result<SearchStep> {
    session.install_scores(scores)?;
    session.set_search_notes(notes)?;
    crate::search::page_prepared(vault, query, session)
}

pub(super) async fn page(
    vault: &Vault,
    paths: &ProjectPaths,
    config: &AppConfig,
    provider: Option<&AiProviderRuntime>,
    request: &AiSearchRequest,
) -> Result<AiSearchResponse> {
    let query = ranked_query(config, provider, request)?;
    let step = if let Some(cursor) = request.cursor.as_deref() {
        crate::search::page(vault, query, Some(cursor))?
    } else {
        let mut session = SearchSession::start(vault, query.clone())?;
        let mut ranks = lexical_ranks(&mut session)?;
        let cancel = session.cancel_token()?;
        let mut notes = Vec::new();
        let semantic = match provider {
            Some(provider) => {
                query_semantic(vault, paths, provider, &request.query, &cancel, None, &mut notes)
                    .await?
            }
            None => {
                notes.push(AiSearchNote::LexicalFallbackNoProvider);
                None
            }
        };
        // Without semantic scores the lexical ranks are already resolved; no scope pass is needed.
        let read_plane = semantic.is_some();
        if let Some(semantic) = semantic {
            ranks = scope_pass(&mut session, ranks, semantic)?;
            if ranks.semantic_scores.iter().all(|score| score.is_nan()) {
                notes.push(AiSearchNote::SemanticMatchesFilteredOut);
            }
        }
        if read_plane && let Some(provider) = provider {
            let serving = crate::EmbeddingIndexRef {
                provider_id: provider.config.id.clone(),
                model: provider.config.default_model.clone(),
            };
            if crate::semantic_index::chosen(config).as_ref() != Some(&serving) {
                notes.push(AiSearchNote::PreviousModel { model: serving.model });
            }
        }
        cancel.check()?;
        let scores = rank_and_fuse(vault, config, &session, ranks)?;
        install_first_page(vault, query, session, scores, notes)?
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
#[cfg(test)]
pub(super) async fn semantic_page(
    vault: &Vault,
    paths: &ProjectPaths,
    provider: &AiProviderRuntime,
    request: &AiSearchRequest,
    allowlist: Option<&[u64]>,
) -> Result<super::search::SemanticMatchReport> {
    let mut session = SearchSession::start(vault, query(request))?;
    let mut ranks = Ranks::new(session.with_newest(|times| times.len())?);
    let cancel = session.cancel_token()?;
    let mut notes = Vec::new();
    if let Some(semantic) =
        query_semantic(vault, paths, provider, &request.query, &cancel, allowlist, &mut notes)
            .await?
    {
        ranks = scope_pass(&mut session, ranks, semantic)?;
        if ranks.semantic_scores.iter().all(|score| score.is_nan()) {
            notes.push(AiSearchNote::SemanticMatchesFilteredOut);
        }
    }
    let reasons =
        ranks.semantic_scores.iter().map(|score| if score.is_nan() { 0 } else { 2 }).collect();
    session.install_scores(Scores { values: ranks.semantic_scores, reasons })?;
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

#[cfg(test)]
#[path = "semantic_stage_bench.rs"]
pub(super) mod semantic_stage_bench;
