//! Page a complete ranking in its deterministic score/event/chunk order. The cursor points
//! at the first eligible rank withheld for the next page. The small ranking cache is optional:
//! after eviction the same query vector and generation reconstruct exactly the same ordering.
use super::{authorize_provider, chosen, preview};
use crate::data_kinds::Refusal;
use crate::{
    VaultSession,
    context::{selected_embedding_provider_runtime, tokio_runtime},
};
use agent_history::{Error as ReaderError, search::ScreenQuery};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::{
    collections::VecDeque,
    path::Path,
    sync::{Arc, LazyLock, Mutex, Weak},
};
use vault_core::{
    AiCapability, AiRefusal, Vault,
    ai::{
        conversation_vectors::{self as vectors, Ranking},
        conversations::consent,
    },
};

const PAGE_SIZE: usize = 100;
#[derive(Serialize, Deserialize, PartialEq)]
struct View {
    generation: String,
    scope: String,
    projection: i64,
    marks: i64,
}
#[derive(Serialize, Deserialize)]
struct Cursor {
    view: View,
    vector: String,
    next: usize,
}
struct Cached {
    owner: Weak<vault_store::Store>,
    root: std::path::PathBuf,
    scope: String,
    ranking: Arc<Ranking>,
}
static RECENT: LazyLock<Mutex<VecDeque<Cached>>> = LazyLock::new(Default::default);
pub(super) fn forget(root: &Path) {
    RECENT.lock().unwrap().retain(|c| c.root != root);
}
fn cached(v: &Vault, view: &View, cursor: Option<&Cursor>) -> Option<Arc<Ranking>> {
    let mut recent = RECENT.lock().unwrap();
    let pos = recent.iter().position(|c| {
        c.owner.ptr_eq(&Arc::downgrade(v.store()))
            && c.scope == view.scope
            && c.ranking.generation.generation == view.generation
            && cursor.is_none_or(|cur| cur.vector == c.ranking.query_digest)
    })?;
    let entry = recent.remove(pos)?;
    let ranking = entry.ranking.clone();
    recent.push_front(entry);
    Some(ranking)
}
pub(in crate::data_kinds) fn search(
    session: &VaultSession,
    vault: Vault,
    q: ScreenQuery,
) -> Result<preview::Frames> {
    if q.query.trim().is_empty()
        || q.query.len() > 4096
        || q.roles.is_empty()
        || q.filters.limit.is_some()
        || q.filters.cursor.is_some()
    {
        return Err(Refusal::InvalidRequest.into());
    }
    let cursor = q
        .cursor
        .as_deref()
        .map(serde_json::from_str::<Cursor>)
        .transpose()
        .map_err(|_| Refusal::InvalidCursor)?;
    let cfg = consent(&vault, AiCapability::SemanticIndex)?;
    let model = chosen(&cfg).ok_or(AiRefusal::ProviderMissing)?;
    let archive = vault.agent_history()?;
    let current = vectors::current(&archive)?.filter(|g| g.matches_config(model));
    let generation = match (current, cursor.as_ref()) {
        (Some(g), _) => g.generation,
        (None, Some(_)) => return Err(ReaderError::StaleCursor.into()),
        (None, None) => return Err(Refusal::SemanticNotReady.into()),
    };
    let scope =
        vault_core::utils::sha256_hex(&serde_json::to_vec(&(&q.query, &q.roles, &q.filters))?);
    let view = View {
        generation,
        scope,
        marks: archive.search_marks_generation()?,
        projection: archive.projection_generation()?,
    };
    if cursor.as_ref().is_some_and(|c| c.view != view) {
        return Err(ReaderError::StaleCursor.into());
    }
    let runtime =
        selected_embedding_provider_runtime(&cfg, None).context(AiRefusal::ProviderMissing)?;
    preview::semantic_stream(session, cfg, archive, move |a, cancel, send| {
        let check = || {
            cancel.check()?;
            authorize_provider(&vault, &runtime)
        };
        check()?;
        send("started", json!({"mode":"semantic","pageSize":PAGE_SIZE}))?;
        let ranking = match cached(&vault, &view, cursor.as_ref()) {
            Some(r) => r,
            None => {
                let engine = super::engine(&vault, &runtime)?;
                let r = Arc::new(
                    tokio_runtime()?
                        .block_on(vectors::rank(a, &runtime, &engine, &q.query, cancel, check))?,
                );
                if r.generation.generation != view.generation
                    || cursor.as_ref().is_some_and(|c| c.vector != r.query_digest)
                {
                    return Err(ReaderError::StaleCursor.into());
                }
                let mut recent = RECENT.lock().unwrap();
                recent.push_front(Cached {
                    owner: Arc::downgrade(vault.store()),
                    root: vault.paths().app_root.clone(),
                    scope: view.scope.clone(),
                    ranking: r.clone(),
                });
                recent.truncate(2);
                r
            }
        };
        let start = cursor.as_ref().map_or(0, |c| c.next);
        let remaining = ranking.hits.get(start..).ok_or(Refusal::InvalidCursor)?;
        let mut items = Vec::with_capacity(PAGE_SIZE);
        let mut next = None;
        'scan: for (block, batch) in remaining.chunks(PAGE_SIZE + 1).enumerate() {
            check()?;
            let addresses = batch.iter().map(|h| (h.event, h.chunk)).collect::<Vec<_>>();
            for (i, mut item) in a.semantic_page(&addresses, &q)? {
                let hit = &batch[i];
                if !ranking.matches(hit, item["text"].as_str().context("missing passage text")?) {
                    continue;
                }
                if items.len() == PAGE_SIZE {
                    next = Some(start + block * (PAGE_SIZE + 1) + i);
                    break 'scan;
                }
                item["score"] = hit.score.into();
                item["chunk"] = hit.chunk.into();
                items.push(item);
            }
        }
        check()?;
        if a.search_marks_generation()? != view.marks
            || a.projection_generation()? != view.projection
            || vectors::current(a)?.is_none_or(|g| g.generation != view.generation)
        {
            return Err(ReaderError::StaleCursor.into());
        }
        let stale = ranking.generation.projection != view.projection
            || ranking.generation.marks != view.marks;
        let hits = items.len();
        if hits > 0 {
            send("passages", json!({"items":items}))?;
        }
        let generation = view.generation.clone();
        let next_cursor = next
            .map(|next| {
                serde_json::to_string(&Cursor { view, vector: ranking.query_digest.clone(), next })
            })
            .transpose()?;
        send(
            "finished",
            json!({"hits":hits,"generation":generation,"stale":stale,"nextCursor":next_cursor}),
        )
    })
}
