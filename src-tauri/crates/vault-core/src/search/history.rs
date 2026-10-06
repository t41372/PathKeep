//! Responsibilities: adapt the existing paged history and export contracts to search.
//! Not responsible for: timeline reads, opening databases or a separate search engine.
//! Failure modes: a facade truncates global matches, loses continuation or hides build state.
//! Tests: ported archive, annotation, worker and export search suites.
use super::{SearchEvent, SearchRequest, SearchStep};
use crate::{HistoryQuery, HistoryQueryResponse, Vault};
use anyhow::{Context, Result};
use std::sync::mpsc;

pub fn history(vault: &Vault, query: HistoryQuery) -> Result<HistoryQueryResponse> {
    history_for_root(vault.paths(), query)
}
/// The unported archive dispatcher only resolves an existing owner; it never opens storage.
pub(crate) fn history_for_root(
    paths: &crate::ProjectPaths,
    query: HistoryQuery,
) -> Result<HistoryQueryResponse> {
    let (store, indexer, sessions) = crate::vault::live::search(&paths.app_root)?;
    let request = SearchRequest {
        query: query.q.unwrap_or_default(),
        sort: query.sort,
        page_size: query.limit.unwrap_or(100).max(1) as usize,
        profile_id: query.profile_id,
        browser_kind: query.browser_kind,
        domain: query.domain,
        start_time_ms: query.start_time_ms,
        end_time_ms: query.end_time_ms,
        regex: query.regex_mode.unwrap_or(false),
        ..Default::default()
    };
    let page_size = request.page_size;
    let mut step = page_from(
        store.clone(),
        indexer.clone(),
        sessions.clone(),
        request.clone(),
        query.cursor.as_deref(),
        None,
    )?;
    let target_page = query.page.unwrap_or(1).max(1) as usize;
    let mut current_page = 1;
    if query.cursor.is_none() {
        while current_page < target_page && !step.items.is_empty() {
            let Some(cursor) = step.cursor.clone() else { break };
            step = page_from(
                store.clone(),
                indexer.clone(),
                sessions.clone(),
                request.clone(),
                Some(&cursor),
                None,
            )?;
            current_page += 1;
        }
    }
    let total = step.count.value as usize;
    let coverage = step.coverage();
    let next_cursor = step.cursor;
    let counted = query.include_total.unwrap_or(true);
    let returned = step.items.len();
    Ok(HistoryQueryResponse {
        search_state: Some(coverage),
        total: if counted { total } else { returned },
        total_visits: Some(step.visits.value as usize),
        total_exact: counted && step.count.exact,
        items: step.items.into_iter().map(|h| h.visit).collect(),
        page: query.page.unwrap_or(1) as usize,
        page_size,
        page_count: total.div_ceil(page_size).max(1),
        has_previous: query.cursor.is_some() || current_page > 1,
        has_next: next_cursor.is_some(),
        next_cursor,
        ..Default::default()
    })
}

/// Complete a page through the same session registry used by streaming transports.
/// The returned opaque cursor contains its session identity; callers must release it
/// with `cancel_search` when they stop paging.
pub fn page(vault: &Vault, request: SearchRequest, cursor: Option<&str>) -> Result<SearchStep> {
    page_from(
        vault.store().clone(),
        vault.search().clone(),
        vault.search_sessions().clone(),
        request,
        cursor,
        None,
    )
}
pub(crate) fn page_prepared(
    vault: &Vault,
    request: SearchRequest,
    session: super::SearchSession,
) -> Result<SearchStep> {
    page_from(
        vault.store().clone(),
        vault.search().clone(),
        vault.search_sessions().clone(),
        request,
        None,
        Some(session),
    )
}
fn page_from(
    store: std::sync::Arc<vault_store::Store>,
    indexer: std::sync::Arc<super::Indexer>,
    sessions: std::sync::Arc<super::SearchSessions>,
    request: SearchRequest,
    cursor: Option<&str>,
    prepared: Option<super::SearchSession>,
) -> Result<SearchStep> {
    let (tx, rx) = mpsc::channel();
    let sink = Box::new(move |event| tx.send(event).is_ok());
    let session_id = if let Some(cursor) = &cursor {
        let (id, token) = cursor.split_once(':').context("invalid search cursor")?;
        sessions.next_with_sink(id, token.into(), sink, &request)?;
        id.to_owned()
    } else {
        match prepared {
            Some(session) => {
                sessions.start_prepared(store, indexer, request, session, sink)?.session_id
            }
            None => sessions.start_owned(store, indexer, request, sink)?.session_id,
        }
    };
    let step: SearchStep = loop {
        match rx.recv()? {
            SearchEvent::Step(step) if !step.provisional && step.count.exact => break *step,
            SearchEvent::Error(error) => {
                let _ = sessions.cancel(&session_id);
                anyhow::bail!(error)
            }
            SearchEvent::Cancelled => anyhow::bail!("search cancelled"),
            _ => {}
        }
    };
    let mut step = step;
    step.cursor = step.cursor.map(|cursor| format!("{session_id}:{cursor}"));
    if step.cursor.is_none() {
        let _ = sessions.cancel(&session_id);
    }
    Ok(step)
}

/// Search export retains one reader and one output page, including arbitrarily deep cursors.
pub(crate) fn walk_history_for_export(
    paths: &crate::ProjectPaths,
    query: HistoryQuery,
    cancel: vault_store::CancelToken,
    mut visit: impl FnMut(&crate::HistoryEntry) -> Result<()>,
) -> Result<()> {
    let (store, indexer, _) = crate::vault::live::search(&paths.app_root)?;
    let request = SearchRequest {
        query: query.q.unwrap_or_default(),
        sort: query.sort,
        page_size: 1000,
        profile_id: query.profile_id,
        browser_kind: query.browser_kind,
        domain: query.domain,
        start_time_ms: query.start_time_ms,
        end_time_ms: query.end_time_ms,
        regex: query.regex_mode.unwrap_or(false),
        ..Default::default()
    };
    // An export is a complete artifact. Wait for the canonical prefix present at
    // its start, while interactive search continues to serve partial build results.
    let target = store.read(vault_store::Db::Archive, super::documents::watermark)?;
    loop {
        cancel.check()?;
        let state = indexer.state();
        match state.phase {
            super::IndexerPhase::Failed
            | super::IndexerPhase::Cancelled
            | super::IndexerPhase::Closed => anyhow::bail!(
                "search export cannot obtain a complete index: {:?}: {:?}",
                state.phase,
                state.error
            ),
            super::IndexerPhase::Building => {}
            _ if state.applied_seq >= target => break,
            _ => {}
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    let mut session = super::SearchSession::start_owned(&store, &indexer, request, cancel.clone())?;
    loop {
        cancel.check()?;
        let page = session.finish_page()?;
        for hit in page.items {
            cancel.check()?;
            visit(&hit.visit)?;
        }
        let Some(cursor) = page.cursor else { break };
        session.next_page(&cursor)?;
    }
    Ok(())
}
