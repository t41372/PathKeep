//! Responsibilities: expose complete lexical/date pages to optional AI consumers.
//! Not responsible for: embedding calls, semantic ranking, tool output budgets or queues.
//! Failure modes: an expanded candidate pool replaces real paging; facets are applied
//! after truncation; counts report a sampled pool; blank queries have no continuation.
//! Tests: AI complete-pool and date-order cursor contracts, code-mode and MCP tests.
use super::search::{
    MATCH_REASON_LEXICAL, MATCH_REASON_LEXICAL_DATE_ORDERED, MATCH_REASON_RECENT_VISIT,
    MATCH_REASON_STARRED_SUFFIX, history_entry_to_search_entry, resolve_date_filter_ms,
};
use crate::{
    AiSearchNote, AiSearchRequest, AiSearchResponse, Vault,
    search::{SearchRequest, SearchStep},
};
use anyhow::Result;
pub(super) fn lexical_page(vault: &Vault, request: &AiSearchRequest) -> Result<AiSearchResponse> {
    let (start_time_ms, end_time_ms) = resolve_date_filter_ms(request);
    let blank = request.query.trim().is_empty();
    let sort = crate::AiSearchSort::parse(request.sort.as_deref());
    let chronological = sort.is_date_ordered();
    let query = SearchRequest {
        query: request.query.clone(),
        sort: Some(if blank { "newest" } else { sort.search_sort() }.into()),
        page_size: request.limit.unwrap_or(8).max(1) as usize,
        profile_id: request.profile_id.clone(),
        domain: request.domain.clone(),
        start_time_ms,
        end_time_ms,
        starred: request.starred_only.unwrap_or(false),
        ..Default::default()
    };
    let step = crate::search::page(vault, query, request.cursor.as_deref())?;
    response(
        step,
        request,
        if blank {
            "recent-visits"
        } else if chronological {
            "date-ordered"
        } else {
            "lexical-fallback"
        },
        if blank {
            MATCH_REASON_RECENT_VISIT
        } else if chronological {
            MATCH_REASON_LEXICAL_DATE_ORDERED
        } else {
            MATCH_REASON_LEXICAL
        },
    )
}
fn response(
    step: SearchStep,
    request: &AiSearchRequest,
    provider: &str,
    reason: &str,
) -> Result<AiSearchResponse> {
    let max = step.items.first().map_or(1., |hit| hit.score).max(f32::MIN_POSITIVE);
    let note_codes = if provider == "lexical-fallback" {
        vec![AiSearchNote::LexicalFallbackNoProvider]
    } else {
        vec![]
    };
    let reason = if request.starred_only.unwrap_or(false) {
        format!("{reason}{MATCH_REASON_STARRED_SUFFIX}")
    } else {
        reason.to_owned()
    };
    Ok(AiSearchResponse {
        search_state: Some(step.coverage()),
        total: step.count.value as usize,
        provider_id: provider.into(),
        model: "none".into(),
        items: step
            .items
            .iter()
            .map(|hit| history_entry_to_search_entry(&hit.visit, hit.score / max, &reason))
            .collect(),
        notes: note_codes.iter().map(AiSearchNote::model_facing_text).collect(),
        note_codes,
        has_more: step.cursor.is_some(),
        next_cursor: step.cursor,
        applied_limit: Some(request.limit.unwrap_or(8).max(1)),
    })
}
