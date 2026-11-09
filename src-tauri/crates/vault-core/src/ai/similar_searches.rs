//! Responsibilities: rank all currently served keyword searches against one search vector.
//! Not responsible for: window clustering, lexical wordings, or embedding inference.
//! Failure modes: undone searches return; short/self/known wordings occupy rows; a missing source
//! borrows a vector; ties vary; one-vector and identical corpora fail at the sampled cut-off.
//! Tests: semantic-index.spec.ts exercises reads and navigation; search_groups_bench measures 1M.
use super::{
    AiProviderRuntime, FlatVectorIndex, search_groups_cluster::percentile, search_vectors,
};
use crate::Vault;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, Mutex},
};
use vault_store::{CancelToken, Db, params_owned as params};

/// A wording links directly to its own drill-in, preserving its all-time search evidence.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SimilarSearch {
    pub query: String,
    pub searches: u64,
    pub last_searched_at: i64,
}

/// Optional availability is separate from a source whose vector has not been built yet.
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "camelCase")]
pub enum SimilarSearches {
    Unavailable {
        reason: String,
    },
    #[serde(rename_all = "camelCase")]
    NotIndexed {
        static_model: bool,
    },
    #[serde(rename_all = "camelCase")]
    Ready {
        static_model: bool,
        items: Vec<SimilarSearch>,
    },
}
impl SimilarSearches {
    /// Session/provider failures hide this optional section without failing the search page.
    pub fn unavailable(reason: impl Into<String>) -> Self {
        Self::Unavailable { reason: reason.into() }
    }
}
fn normalized(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase()
}

/// This read reuses the serving model, even while another model is selected for a new build.
pub fn similar_searches(
    vault: &Vault,
    provider: Option<&AiProviderRuntime>,
    query: &str,
    wordings: &[String],
) -> Result<SimilarSearches> {
    if !vault.config().ai.enabled || !vault.config().ai.semantic_index_enabled {
        return Ok(SimilarSearches::unavailable("semantic search is off"));
    }
    let Some(provider) = provider else {
        return Ok(SimilarSearches::unavailable("no embedding provider is configured"));
    };
    let Some(pinned) = super::generations::serving_vault(
        vault,
        &provider.config.id,
        &provider.config.default_model,
    )?
    else {
        return Ok(SimilarSearches::unavailable("the semantic index is empty"));
    };
    let vault = &pinned;
    let available = || -> Result<FlatVectorIndex> {
        let pages = search_vectors::available(vault.paths(), provider)?;
        search_vectors::open(vault.paths(), provider, pages.dim())
    };
    let index = match available() {
        Ok(index) => index,
        Err(e) => return Ok(SimilarSearches::unavailable(e.to_string())),
    };
    read(vault, &index, query, wordings, super::runtime_uses_static(provider))
}

/// Kept callable by the opt-in probe so its timing includes current-generation counts and rows.
pub(super) fn read(
    vault: &Vault,
    index: &FlatVectorIndex,
    query: &str,
    wordings: &[String],
    static_model: bool,
) -> Result<SimilarSearches> {
    let source = normalized(query);
    let mut excluded = wordings.iter().map(|s| normalized(s)).collect::<HashSet<_>>();
    excluded.insert(source.clone());
    let current = current_searches(vault, index)?;
    let source_key = search_vectors::key(&source);
    if !current.rows.contains_key(&source_key) {
        return Ok(SimilarSearches::NotIndexed { static_model });
    }
    let source_pos = (0..index.len()).find(|&p| index.key(p) == source_key);
    let Some(source_pos) = source_pos else {
        return Ok(SimilarSearches::NotIndexed { static_model });
    };
    if !index.has_vector(source_pos) {
        return Ok(SimilarSearches::NotIndexed { static_model });
    }
    let vector = index.vector_at(source_pos);
    let threshold = current.threshold;
    let scores = index.score_all(&vector, &CancelToken::new())?;
    let mut top = Vec::<(usize, f32, i64)>::new();
    for &position in current.positions.iter() {
        let score = scores[position];
        let row = &current.rows[&index.key(position)];
        if !score.is_finite()
            || score + 8. * f32::EPSILON < threshold
            || excluded.contains(&row.query)
        {
            continue;
        }
        top.push((position, score, row.last_searched_at));
        top.sort_by(|a, b| {
            b.1.total_cmp(&a.1).then(b.2.cmp(&a.2)).then(index.key(a.0).cmp(&index.key(b.0)))
        });
        top.truncate(8);
    }
    let mut items =
        top.into_iter().map(|(p, _, _)| current.rows[&index.key(p)].clone()).collect::<Vec<_>>();
    // Only the eight selected wordings need their original spelling: one index seek each for its
    // newest event. (A correlated subquery over `json_each` was not planned as seeks: 0.9 s.)
    let queries = items.iter().map(|row| row.query.clone()).collect::<Vec<_>>();
    let spellings = vault.read(Db::Intelligence, move |session| {
        let mut spellings = Vec::with_capacity(queries.len());
        for query in queries {
            let spelling = session
                .query(
                    "SELECT e.raw_query FROM search_events_rows e INDEXED BY idx_search_events_query_time
                     CROSS JOIN archive.visits v ON v.id=e.visit_id
                     WHERE e.query_kind='keyword' AND e.normalized_query=?1 AND v.reverted_at IS NULL
                       AND e.generation IN (SELECT navigation FROM intelligence_generations)
                     ORDER BY e.visit_time_ms DESC,e.visit_id DESC LIMIT 1",
                    params![query.clone()],
                    |row| Ok(row.get::<String>(0)?),
                )?
                .into_iter()
                .next();
            spellings.push((query, spelling));
        }
        Ok(spellings)
    })?;
    let spellings = spellings.into_iter().collect::<HashMap<_, _>>();
    items.retain_mut(|row| {
        let Some(Some(spelling)) = spellings.get(&row.query) else {
            return false;
        };
        row.query.clone_from(spelling);
        true
    });
    Ok(SimilarSearches::Ready { static_model, items })
}

/// The searches the current Insights generation still serves, with the drill-in's cut-off.
struct Current {
    stamp: Stamp,
    rows: HashMap<u64, SimilarSearch>,
    /// Plane positions of the searches that can be listed (current, not short, with a vector).
    positions: Vec<usize>,
    threshold: f32,
}

/// What `Current` depends on: the archive folder, the Insights generations, the last archive run
/// that can show or hide visits, and the search plane's projection.
#[derive(Clone, PartialEq, Eq)]
struct Stamp {
    root: std::path::PathBuf,
    generations: String,
    archive_run: i64,
    projection: String,
}

/// Reading every search event takes about two seconds at a million distinct searches, and the
/// answer only changes with an Insights build or an archive run, so it is kept until one of
/// those moves.
fn current_searches(vault: &Vault, index: &FlatVectorIndex) -> Result<Arc<Current>> {
    static CACHE: Mutex<Option<Arc<Current>>> = Mutex::new(None);
    let generations = vault.read(Db::Intelligence, |session| {
        Ok(session.query_one(
            "SELECT COALESCE(GROUP_CONCAT(profile_id||':'||navigation,','),'')
             FROM (SELECT profile_id,navigation FROM intelligence_generations ORDER BY profile_id)",
            (),
            |row| Ok(row.get::<String>(0)?),
        )?)
    })?;
    let archive_run = vault.read(Db::Archive, |session| {
        Ok(session.query_one(
            "SELECT COALESCE(MAX(id),0) FROM runs
             WHERE run_type IN ('backup','import','rollback','restore','snapshot_restore')",
            (),
            |row| Ok(row.get::<i64>(0)?),
        )?)
    })?;
    let stamp = Stamp {
        root: vault.paths().app_root.clone(),
        generations,
        archive_run,
        projection: index.projection_id().to_owned(),
    };
    if let Some(cached) = CACHE.lock().unwrap_or_else(std::sync::PoisonError::into_inner).as_ref()
        && cached.stamp == stamp
    {
        return Ok(Arc::clone(cached));
    }
    let rows = vault.read(Db::Intelligence, move |session| {
        let mut rows = HashMap::new();
        session.for_each(
            "SELECT e.normalized_query,COUNT(*),MAX(e.visit_time_ms)
             FROM search_events_rows e INDEXED BY idx_search_events_query_time
             CROSS JOIN archive.visits v ON v.id=e.visit_id
             WHERE e.query_kind='keyword' AND e.generation IN (SELECT navigation FROM intelligence_generations)
               AND v.reverted_at IS NULL
             GROUP BY e.normalized_query",
            (),
            |row| {
                let query = row.get::<String>(0)?;
                rows.insert(search_vectors::key(&query), SimilarSearch {
                    query,
                    searches: row.get::<i64>(1)?.max(0) as u64,
                    last_searched_at: row.get(2)?,
                });
                Ok(())
            },
        )?;
        Ok(rows)
    })?;
    // Sample only the current generation's eligible vectors: removed searches do not move the cut.
    let positions = (0..index.len())
        .filter(|&position| {
            rows.get(&index.key(position)).is_some_and(|row| !search_vectors::is_short(&row.query))
                && index.has_vector(position)
        })
        .collect::<Vec<_>>();
    let threshold =
        percentile(positions.len(), 99, |a, b| index.score_pair(positions[a], positions[b]));
    let current = Arc::new(Current { stamp, rows, positions, threshold });
    *CACHE.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = Some(Arc::clone(&current));
    Ok(current)
}
