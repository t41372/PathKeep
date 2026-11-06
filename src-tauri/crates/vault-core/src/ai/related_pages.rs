//! Responsibilities: find related pages from the serving int8 plane without model calls.
//! Not responsible for: building indexes, changing history filters, or naming subjects.
//! Failure modes: the source or a duplicate returns; hidden/deleted visits leak; a stale position
//! points at another page; folding leaves fewer than six; single/identical/zero vectors fail;
//! an unindexed URL borrows another vector; `_` shortens `download_manga.py` to `download`.
//! Tests: projection and numeric/folding edges below; semantic-index.spec.ts covers the real panel,
//! visibility, site balance, selection and optional availability. The opt-in release probe measures
//! the production read path at one million indexed visits.

use super::{AiProviderRuntime, FlatVectorIndex, ReverseVisitMap, visit_position_map::PointLookup};
use crate::{HistoryEntry, Vault};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    time::Instant,
};
use vault_store::{CancelToken, Db, params_owned as params};

const LIMIT: usize = 6;
/// Positions read before settling for what was found: three batches, 256, 512 and 1,024.
const MAX_CANDIDATES: usize = 1_792;
const SCORE_EPSILON: f32 = 8. * f32::EPSILON;

/// Preserves the archived identity so a related row can become History's selection.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RelatedPage {
    pub history_id: i64,
    pub url: String,
    pub title: Option<String>,
    pub domain: String,
    pub profile_id: String,
    pub last_visit_time: i64,
    pub score: f32,
}

/// Keeps absence, an index publication and a genuinely empty result distinct in the panel.
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "camelCase")]
pub enum RelatedPages {
    Unavailable {
        reason: String,
    },
    #[serde(rename_all = "camelCase")]
    NotIndexed {
        static_model: bool,
    },
    #[serde(rename_all = "camelCase")]
    Updating {
        static_model: bool,
    },
    #[serde(rename_all = "camelCase")]
    Ready {
        static_model: bool,
        items: Vec<RelatedPage>,
    },
}

impl RelatedPages {
    /// Provider resolution failures hide the optional section without failing URL details.
    pub fn unavailable(reason: impl Into<String>) -> Self {
        Self::Unavailable { reason: reason.into() }
    }
}

#[derive(Default, Serialize)]
pub(super) struct Timings {
    point_lookup_ms: f64,
    cutoff_ms: f64,
    scan_ms: f64,
    top_k_ms: f64,
    reverse_lookups_ms: f64,
    hydrate_ms: f64,
    total_ms: f64,
}

fn elapsed(start: Instant) -> f64 {
    start.elapsed().as_secs_f64() * 1000.
}

/// Reuses the serving model's vectors; this read never embeds text or starts a search session.
pub fn related_pages(
    vault: &Vault,
    provider: Option<&AiProviderRuntime>,
    url: &str,
    visit: Option<i64>,
) -> Result<RelatedPages> {
    measured(vault, provider, url, visit, &mut Timings::default())
}

pub(super) fn measured(
    vault: &Vault,
    provider: Option<&AiProviderRuntime>,
    url: &str,
    visit: Option<i64>,
    timings: &mut Timings,
) -> Result<RelatedPages> {
    let total = Instant::now();
    let config = vault.config();
    if !config.ai.enabled || !config.ai.semantic_index_enabled {
        return Ok(RelatedPages::unavailable("semantic search is off"));
    }
    let Some(provider) = provider else {
        return Ok(RelatedPages::unavailable("no embedding provider is configured"));
    };
    let Some(pinned) = super::generations::serving_vault(
        vault,
        &provider.config.id,
        &provider.config.default_model,
    )?
    else {
        return Ok(RelatedPages::unavailable("the semantic index is empty"));
    };
    let vault = &pinned;
    // Match topics' availability checks, including the engine descriptor, without inference.
    let available = || -> Result<FlatVectorIndex> {
        super::validate_provider(provider, crate::AiProviderPurpose::Embedding)?;
        let index = FlatVectorIndex::open(
            vault.paths(),
            &provider.config.id,
            &provider.config.default_model,
        )?;
        anyhow::ensure!(!index.is_empty(), "the semantic index is empty");
        let engine = super::embedding_engine::select_embedding_provider(vault.paths(), provider)?;
        anyhow::ensure!(
            super::search::semantic_config_drift_reason(
                vault.paths(),
                provider,
                &engine,
                &index,
                index.dim()
            )?
            .is_none(),
            "the semantic index has drifted from this model"
        );
        Ok(index)
    };
    let index = match available() {
        Ok(index) => index,
        Err(error) => return Ok(RelatedPages::unavailable(error.to_string())),
    };
    let static_model = super::runtime_uses_static(provider);
    let start = Instant::now();
    let Some(mut positions) = PointLookup::open(
        vault.paths(),
        &provider.config.id,
        &provider.config.default_model,
        &index,
    )?
    else {
        return Ok(RelatedPages::Updating { static_model });
    };
    let source = source(vault, url, visit, &mut positions)?;
    timings.point_lookup_ms = elapsed(start);
    let Some((source, position)) = source else {
        return Ok(RelatedPages::NotIndexed { static_model });
    };
    let vector = index.vector_at(position);
    if !vector.iter().any(|v| *v != 0.) {
        return Ok(RelatedPages::NotIndexed { static_model });
    }
    let source_key = fold_key(&source);
    let start = Instant::now();
    let threshold = published_cutoff(&index);
    timings.cutoff_ms = elapsed(start);
    let start = Instant::now();
    let scores = index.score_all(&vector, &CancelToken::new())?;
    timings.scan_ms = elapsed(start);
    let start = Instant::now();
    let mut candidates = scores
        .into_iter()
        .enumerate()
        .filter(|(_, score)| score.is_finite() && *score + SCORE_EPSILON >= threshold)
        .collect::<Vec<_>>();
    timings.top_k_ms += elapsed(start);
    let reverse = ReverseVisitMap::for_provider(
        vault.paths(),
        &provider.config.id,
        &provider.config.default_model,
    );
    let mut rows = Vec::new();
    let mut batch = 256;
    let mut read = 0;
    let items = loop {
        let start = Instant::now();
        let count = batch.min(candidates.len());
        let compare = |a: &(usize, f32), b: &(usize, f32)| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0));
        if count < candidates.len() {
            candidates.select_nth_unstable_by(count, compare);
        }
        let mut top = candidates.drain(..count).collect::<Vec<_>>();
        top.sort_unstable_by(compare);
        // Read the entire boundary tie so recency, not plane order, decides the last row.
        if let Some((_, boundary)) = top.last().copied() {
            candidates.retain(|candidate| {
                if candidate.1 == boundary {
                    top.push(*candidate);
                    false
                } else {
                    true
                }
            });
        }
        read += top.len();
        let scores =
            top.into_iter().map(|(pos, score)| (index.key(pos), score)).collect::<HashMap<_, _>>();
        timings.top_k_ms += elapsed(start);
        let start = Instant::now();
        let by_visit = reverse
            .first_history_id_for_content_keys(&scores.keys().copied().collect::<HashSet<_>>())?
            .into_iter()
            .map(|(key, id)| (id, scores[&key]))
            .collect::<HashMap<_, _>>();
        timings.reverse_lookups_ms += elapsed(start);
        let start = Instant::now();
        let visits = crate::search::newest_visible_by_page(
            vault,
            &by_visit.keys().copied().collect::<Vec<_>>(),
        )?;
        rows.extend(visits.into_iter().map(|(id, v)| RelatedPage {
            history_id: v.id,
            domain: site(&v.url),
            url: v.url,
            title: v.title,
            profile_id: v.profile_id,
            last_visit_time: v.visit_time,
            score: by_visit[&id],
        }));
        timings.hydrate_ms += elapsed(start);
        let start = Instant::now();
        let selected = select(rows.clone(), url, &source_key);
        // Beyond six rows, read further only to replace the source site's fourth row, and only so
        // far: most pages' neighbours are all on their own site, which no amount of reading changes.
        let balanced = selected.iter().filter(|row| row.domain == source_key.0).count() <= 3;
        let done = candidates.is_empty()
            || read >= MAX_CANDIDATES
            || (selected.len() == LIMIT && balanced);
        timings.top_k_ms += elapsed(start);
        if done {
            break selected;
        }
        batch *= 2;
    };
    if !positions.is_current(
        vault.paths(),
        &provider.config.id,
        &provider.config.default_model,
        &index,
    )? {
        return Ok(RelatedPages::Updating { static_model });
    }
    timings.total_ms = elapsed(total);
    Ok(RelatedPages::Ready { static_model, items })
}

fn site(url: &str) -> String {
    crate::visit_taxonomy::registrable_domain_for_url(url)
        .unwrap_or_else(|| crate::utils::url_domain(url))
}

fn fold_key(visit: &HistoryEntry) -> (String, String) {
    (site(&visit.url), headline(visit.title.as_deref().unwrap_or("")))
}

fn source(
    vault: &Vault,
    url: &str,
    visit: Option<i64>,
    positions: &mut PointLookup,
) -> Result<Option<(HistoryEntry, usize)>> {
    if let Some(id) = visit {
        let selected = crate::search::visits(vault, &[id])?.into_iter().find(|row| row.url == url);
        if let Some(selected) = selected
            && let Some(position) = positions.get(id)?
        {
            return Ok(Some((selected, position)));
        }
    }
    let address = url.to_owned();
    let urls = vault.store().read(Db::Archive, move |s| {
        s.query("SELECT id FROM urls WHERE url=?1", params![address], |r| Ok(r.get::<i64>(0)?))
    })?;
    let mut best = None;
    for url_id in urls {
        let mut before = (i64::MAX, i64::MAX);
        loop {
            // A seek per profile, not a sort or walk of all the URL's visits. Skip visits newer
            // than the published position file before looking for the newest indexed one.
            let max_id = positions.max_visit();
            let visits=vault.store().read(Db::Archive,move|s|s.query("SELECT id,visit_time_ms FROM visits INDEXED BY idx_visits_visible_url_time WHERE url_id=?1 AND reverted_at IS NULL AND (visit_time_ms,id)<(?2,?3) AND id<=?4 ORDER BY visit_time_ms DESC,id DESC LIMIT 32",params![url_id,before.0,before.1,max_id],|r|Ok((r.get::<i64>(0)?,r.get::<i64>(1)?))))?;
            if visits.is_empty() {
                break;
            }
            let mut found = false;
            for (id, time) in visits {
                before = (time, id);
                if let Some(position) = positions.get(id)? {
                    if best.is_none_or(|(at, vid, _)| (time, id) > (at, vid)) {
                        best = Some((time, id, position));
                    }
                    found = true;
                    break;
                }
            }
            if found {
                break;
            }
        }
    }
    let Some((_, id, position)) = best else { return Ok(None) };
    Ok(crate::search::visits(vault, &[id])?.into_iter().next().map(|v| (v, position)))
}

fn headline(title: &str) -> String {
    let title = title.to_lowercase();
    let cut = [" - ", " | ", " – ", " — ", "_"]
        .iter()
        .filter_map(|separator| title.find(separator))
        .min();
    let title = match cut {
        Some(at) if title[..at].chars().count() >= 6 => &title[..at],
        _ => &title,
    };
    // Only numeric badges are folded; ordinary parenthesised words remain meaningful.
    static COUNTS: std::sync::LazyLock<regex::Regex> = std::sync::LazyLock::new(|| {
        regex::Regex::new(r"\([0-9]+\)|\[[0-9]+\]|（[0-9]+）").expect("literal count pattern")
    });
    COUNTS.replace_all(title, "").chars().filter(|c| !c.is_whitespace()).collect()
}

fn select(rows: Vec<RelatedPage>, url: &str, source: &(String, String)) -> Vec<RelatedPage> {
    let mut folded = HashMap::<(String, String), RelatedPage>::new();
    for row in rows {
        let key = (row.domain.clone(), headline(row.title.as_deref().unwrap_or("")));
        if row.url == url || &key == source {
            continue;
        }
        folded
            .entry(key)
            .and_modify(|old| {
                let score = old.score.max(row.score);
                if (row.last_visit_time, row.history_id) > (old.last_visit_time, old.history_id) {
                    *old = row.clone();
                }
                old.score = score;
            })
            .or_insert(row);
    }
    let mut rows = folded.into_values().collect::<Vec<_>>();
    rows.sort_unstable_by(|a, b| {
        b.score
            .total_cmp(&a.score)
            .then(b.last_visit_time.cmp(&a.last_visit_time))
            .then(b.history_id.cmp(&a.history_id))
    });
    let mut chosen = Vec::new();
    let mut own = 0;
    for (i, row) in rows.iter().enumerate() {
        if row.domain == source.0 {
            if own == 3 {
                continue;
            }
            own += 1;
        }
        chosen.push(i);
        if chosen.len() == LIMIT {
            break;
        }
    }
    if chosen.len() < LIMIT {
        for i in 0..rows.len() {
            if !chosen.contains(&i) {
                chosen.push(i);
            }
            if chosen.len() == LIMIT {
                break;
            }
        }
    }
    chosen.sort_unstable();
    chosen.into_iter().map(|i| rows[i].clone()).collect()
}

/// The cut-off depends only on the plane, so it is sampled once per published projection (its id
/// is new on every build); sampling touches 40,000 scattered records, more than the scan costs.
fn published_cutoff(index: &FlatVectorIndex) -> f32 {
    static CUTOFF: std::sync::Mutex<Option<(String, f32)>> = std::sync::Mutex::new(None);
    let id = index.projection_id();
    if let Some((published, value)) =
        CUTOFF.lock().unwrap_or_else(std::sync::PoisonError::into_inner).as_ref()
        && !id.is_empty()
        && published == id
    {
        return *value;
    }
    // Sampling reads scattered mmap pages. Readers of another generation must not wait behind
    // disk faults while holding the cache mutex; redundant samples are harmless.
    let value = cutoff(index);
    *CUTOFF.lock().unwrap_or_else(std::sync::PoisonError::into_inner) =
        Some((id.to_owned(), value));
    value
}

fn cutoff(index: &FlatVectorIndex) -> f32 {
    if index.len() < 2 {
        return 1.;
    }
    let mut seed = 0x706174686b656570_u64;
    let mut next = || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        seed
    };
    let mut scores = (0..20_000)
        .map(|_| {
            let a = (next() % index.len() as u64) as usize;
            let b = (a + 1 + (next() % (index.len() - 1) as u64) as usize) % index.len();
            index.score_pair(a, b)
        })
        .collect::<Vec<_>>();
    *scores.select_nth_unstable_by((20_000_usize * 98).div_ceil(100) - 1, f32::total_cmp).1
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::{
        EmbeddingDtype, EmbeddingFingerprint, EmbeddingPooling, ReverseVisitMap, VectorStore,
        VisitContentMap, build_planes_from_store,
    };

    #[test]
    fn folding_and_selection_keep_distinct_pages_even_at_numeric_edges() {
        assert_eq!(headline("A useful thread - Board | Forum"), "ausefulthread");
        assert_eq!(headline("短文 - Board"), "短文-board");
        assert_eq!(headline("Inbox (28) [3] （2）"), "inbox");
        assert_eq!(headline("Story (part two)"), "story(parttwo)");
        assert_eq!(headline("download_manga.py"), "download");
        let row = |id, site: &str, title: &str, score, time| RelatedPage {
            history_id: id,
            url: format!("https://{site}/{id}"),
            title: Some(title.into()),
            domain: site.into(),
            profile_id: "chrome:Default".into(),
            last_visit_time: time,
            score,
        };
        let source = ("own.example".into(), headline("Source page"));
        let mut pages = vec![
            row(1, "own.example", "Source page - Board", 1., 1),
            row(2, "own.example", "Another article", 0.99, 2),
            row(3, "own.example", "Another article | Board", 0.98, 30),
        ];
        pages.extend((4..=9).map(|id| row(id, "own.example", &format!("Own page {id}"), 0.97, id)));
        pages.extend([
            row(10, "other.example", "Across sites", 0.96, 10),
            row(11, "third.example", "Third site", 0.95, 11),
        ]);
        let result = select(pages.clone(), "https://own.example/1", &source);
        assert_eq!(result.len(), 6);
        assert!(!result.iter().any(|r| r.history_id == 1 || r.history_id == 2));
        assert_eq!(result.iter().find(|r| r.history_id == 3).unwrap().score, 0.99);
        assert!(result.iter().any(|r| r.history_id == 10));
        assert!(result.iter().any(|r| r.history_id == 11));
        pages.extend(
            (12..=16).map(|id| row(id, "other.example", &format!("Other page {id}"), 0.94, id)),
        );
        let result = select(pages, "https://own.example/1", &source);
        assert_eq!(result.iter().filter(|r| r.domain == "own.example").count(), 3);
        assert_eq!(result[1].history_id, 9); // equal scores prefer the newest visible visit
        assert!(select(Vec::new(), "", &source).is_empty());

        let dir = tempfile::tempdir().unwrap();
        let paths = crate::config::project_paths_with_root(dir.path());
        let fp = EmbeddingFingerprint::new(
            "related-test",
            "model",
            4,
            EmbeddingDtype::Float32,
            true,
            EmbeddingPooling::Mean,
            None,
        );
        let store = VectorStore::create_stamped(&paths, &fp).unwrap();
        store.append_vectors(&[(1, vec![0.1, 0.2, 0.3, 0.4])]).unwrap();
        build_planes_from_store(&paths, "related-test", "model").unwrap();
        let one = FlatVectorIndex::open(&paths, "related-test", "model").unwrap();
        assert_eq!(cutoff(&one), 1.);
        store
            .append_vectors(&(2..=20).map(|id| (id, vec![0.1, 0.2, 0.3, 0.4])).collect::<Vec<_>>())
            .unwrap();
        build_planes_from_store(&paths, "related-test", "model").unwrap();
        let identical = FlatVectorIndex::open(&paths, "related-test", "model").unwrap();
        let threshold = cutoff(&identical);
        let scores = identical.score_all(&identical.vector_at(0), &CancelToken::new()).unwrap();
        assert!(scores.iter().all(|score| *score + SCORE_EPSILON >= threshold));
    }

    #[test]
    fn point_lookup_refuses_missing_torn_or_replaced_projections() {
        let dir = tempfile::tempdir().unwrap();
        let paths = crate::config::project_paths_with_root(dir.path());
        let fp = EmbeddingFingerprint::new(
            "related-test",
            "model",
            2,
            EmbeddingDtype::Float32,
            true,
            EmbeddingPooling::Mean,
            None,
        );
        let store = VectorStore::create_stamped(&paths, &fp).unwrap();
        store.append_vectors(&[(1, vec![1., 0.]), (2, vec![0., 1.])]).unwrap();
        build_planes_from_store(&paths, "related-test", "model").unwrap();
        let index = FlatVectorIndex::open(&paths, "related-test", "model").unwrap();
        assert!(
            super::super::visit_position_map::PointLookup::open(
                &paths,
                "related-test",
                "model",
                &index
            )
            .unwrap()
            .is_none()
        );
        let map = VisitContentMap::for_provider(&paths, "related-test", "model");
        map.ensure_created(&paths).unwrap();
        map.append(&[(7, 1), (12, 2)]).unwrap();
        let reverse = ReverseVisitMap::for_provider(&paths, "related-test", "model");
        reverse.build_from_visit_map(&paths, &map, &fp.hash()).unwrap();
        let mut lookup = super::super::visit_position_map::PointLookup::open(
            &paths,
            "related-test",
            "model",
            &index,
        )
        .unwrap()
        .unwrap();
        assert_eq!(lookup.get(7).unwrap().map(|p| index.key(p)), Some(1));
        assert_eq!(lookup.get(8).unwrap(), None);
        assert_eq!(lookup.get(999).unwrap(), None);
        assert_eq!(lookup.get(-1).unwrap(), None);
        let position_path = super::super::visit_position_map::path(&paths, "related-test", "model");
        let old = std::fs::read(&position_path).unwrap();
        map.append(&[(7, 2)]).unwrap();
        reverse.build_from_visit_map(&paths, &map, &fp.hash()).unwrap();
        std::fs::write(&position_path, &old).unwrap();
        assert!(
            super::super::visit_position_map::PointLookup::open(
                &paths,
                "related-test",
                "model",
                &index
            )
            .unwrap()
            .is_none()
        );
        reverse.build_from_visit_map(&paths, &map, &fp.hash()).unwrap();
        std::fs::OpenOptions::new().write(true).open(&position_path).unwrap().set_len(12).unwrap();
        assert!(
            super::super::visit_position_map::PointLookup::open(
                &paths,
                "related-test",
                "model",
                &index
            )
            .unwrap()
            .is_none()
        );
        reverse.build_from_visit_map(&paths, &map, &fp.hash()).unwrap();
        build_planes_from_store(&paths, "related-test", "model").unwrap();
        let replacement = FlatVectorIndex::open(&paths, "related-test", "model").unwrap();
        assert!(
            super::super::visit_position_map::PointLookup::open(
                &paths,
                "related-test",
                "model",
                &replacement
            )
            .unwrap()
            .is_none()
        );
    }
}
