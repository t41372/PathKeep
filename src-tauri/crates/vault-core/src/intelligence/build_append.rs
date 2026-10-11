//! Responsibilities: bring a profile's aggregates up to date in place after an append, by
//! building again only what the new visits touch: the pages they open, the paths on the days
//! they fall on, the search trails they join or start and the query families their searches
//! join; and after new categories (`build_reclassify`), the compare sets of the trails whose
//! pages moved. Everything is worked out first, reading only, and written in one transaction, so
//! a reader sees all of the old aggregates or all of the new ones.
//! Not responsible for: the full build (`build_stages::aggregates`), which an append too large
//! for one write uses instead (`IN_PLACE_LIMIT`); facts, rollups and sessions, which their own
//! stages extend in place.
//! Failure modes: something the new visits or categories touch is left as it was; the write is
//! cut in two (it is one transaction with the build's done mark); the search that opened the
//! trail still open at the last build changes kind once its landing is known, so the families
//! from that search on are not what a build from scratch makes (they are made again from every
//! search and only the ones that differ are written).
//!
//! The decision and its alternatives: `docs/architecture/research/2026-10-incremental-aggregates.md`.
//! `tests/append_aggregates.rs` compares every aggregate after appends with a build from
//! scratch; `tests/reclassify.rs` does the same after new categories.

use super::build_state::{self, BuildState};
use super::compare_sets::{self, CompareSetAccumulator, CompareSetRecord};
use super::intelligence_structural_aggregates::{
    RefindAddition, add_to_refind_page, build_reopened_investigations,
};
use super::intelligence_structural_build::QueryFamilyAccumulator;
use super::intelligence_structural_stream::StructuralTailStreamState;
use super::{
    CoreIntelligenceProgress, PathFlowRecord, QueryFamilyRecord, RebuildMode, RefindPageRecord,
    SearchEventRecord, VisibleVisitBatchCursor, build_rows, parse_search_query_kind,
};
use crate::Vault;
use crate::visit_taxonomy::Opener;
use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeSet, HashMap, HashSet};
use vault_store::{Db, OptionalRow, Priority, Session, params_owned as params};

/// The most new visits an append updates the aggregates for in place. A larger one (an import)
/// builds them again into a new generation, as a full build does: its changes would make too
/// large a write.
pub(super) const IN_PLACE_LIMIT: i64 = 100_000;

/// What an append's update of the aggregates needs from before it, taken when the build is
/// prepared: the navigation stage rewrites some of it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct AppendBase {
    /// The visits this append adds have larger ids.
    pub after_visit_id: i64,
    /// Compare sets are found again from the visit after this one: just before the search that
    /// opened the trail still open at the last build, or the last visit already built.
    pub walk_after: VisibleVisitBatchCursor,
    /// The first local day whose path segments the append can write (it rewrites the open
    /// segment and adds later ones), and the paths of those days before it.
    pub paths_from_day: String,
    pub paths_before: Vec<PathFlowRecord>,
    /// The search that opened the trail still open at the last build, and whether it was a
    /// keyword search then: the trail's landing can change that.
    pub open_search: Option<(i64, bool)>,
}

/// Takes what `aggregates` needs from `old`, the build the append continues, before the
/// navigation stage continues its open session and trail. `first_new_ms` is the earliest new
/// visit's time.
pub(super) fn base(vault: &Vault, old: &BuildState, first_new_ms: i64) -> Result<AppendBase> {
    let stream: StructuralTailStreamState =
        old.stream_json.as_deref().map(serde_json::from_str).transpose()?.unwrap_or_default();
    let first_new_day = old.zone().date_key(first_new_ms);
    let paths_from_day = stream
        .current_session
        .as_ref()
        .map(|session| &session.path)
        .filter(|path| path.has_own_sites())
        .map_or(first_new_day.clone(), |path| path.date_key.clone().min(first_new_day));
    let open = stream.current_trail.as_ref().and_then(|trail| {
        trail.record.members.first().map(|search| (trail.record.first_visit_ms, search.visit_id))
    });
    let walk_after = match open {
        Some((time, id)) => VisibleVisitBatchCursor { visit_time_ms: time, visit_id: id - 1 },
        None => VisibleVisitBatchCursor {
            visit_time_ms: first_new_ms,
            visit_id: old.source.max_visit_id,
        },
    };
    let (generation, profile) = (old.navigation_generation, old.profile_id.clone());
    let after_visit_id = old.source.max_visit_id;
    vault.read(Db::Intelligence, move |s| {
        let paths_before = super::path_flows::day_flows(s, generation, &profile, &paths_from_day)?;
        let open_search = match open {
            Some((_, id)) => Some((id, is_keyword(s, generation, id)?)),
            None => None,
        };
        Ok(AppendBase { after_visit_id, walk_after, paths_from_day, paths_before, open_search })
    })
}

/// The rows an append changes, worked out before anything is written.
struct Changes {
    /// Each page a new visit opened, with its row now (`None`: it no longer qualifies).
    pages: Vec<(String, Option<RefindPageRecord>)>,
    /// What each flow gains.
    paths: Vec<PathFlowRecord>,
    /// The trails walked again and their compare sets now.
    trails: BTreeSet<String>,
    compare_sets: Vec<CompareSetRecord>,
    /// Families that are new or differ, and stored ones that no longer exist.
    families: Vec<QueryFamilyRecord>,
    gone_families: Vec<String>,
}

/// Updates the aggregates of the published generation after an append; see the module doc.
pub(super) fn aggregates<F>(
    vault: &Vault,
    state: &mut BuildState,
    limit: usize,
    progress: &mut F,
) -> Result<()>
where
    F: FnMut(CoreIntelligenceProgress) -> Result<()>,
{
    let base = state.append.clone();
    let reclassified = state.reclassify.as_ref().map(|work| work.trails.clone());
    if base.is_none() && reclassified.is_none() {
        bail!("an in-place update with neither new visits nor new categories");
    }
    let mut pages = Vec::new();
    let mut paths = Vec::new();
    let mut trails = BTreeSet::new();
    let mut compare_sets = Vec::new();
    let mut families = Vec::new();
    let mut gone_families = Vec::new();
    if let Some(base) = &base {
        pages = page_rows(vault, state, base, limit, progress)?;
        let Some(gains) = path_changes(vault, state, base)? else {
            return start_over(vault, state, limit, progress);
        };
        paths = gains;
        (trails, compare_sets) = compare_sets_again(vault, state, base, limit, progress)?;
        progress(build_state::progress(state, "search-trails", 0, 1))?;
        let Some(changed) = family_changes(vault, state, base, progress)? else {
            return start_over(vault, state, limit, progress);
        };
        (families, gone_families) = changed;
        progress(build_state::progress(state, "search-trails", 1, 1))?;
    }
    // Trails whose pages moved to other categories, unless the append's walk saw them already.
    if let Some(reclassified) = reclassified {
        let rest = reclassified.difference(&trails).cloned().collect::<Vec<_>>();
        let (walked, sets) = trails_again(vault, state, &rest, limit)?;
        trails.extend(walked);
        compare_sets.extend(sets);
    }
    let changes = Changes { pages, paths, trails, compare_sets, families, gone_families };
    let mut saved = state.clone();
    saved.aggregates_done = true;
    *state = vault.write(Db::Intelligence, Priority::Batch, move |s| {
        write(s, &mut saved, changes)?;
        build_state::save(s, &saved)?;
        Ok(saved)
    })?;
    super::build_pipeline::checkpoint(vault, state, RebuildMode::StructuralRebuild)
}

/// Builds the aggregates again into a new generation, as a full build does, when an in-place
/// update cannot be trusted: the stored rows are not what the update assumes.
fn start_over<F>(
    vault: &Vault,
    state: &mut BuildState,
    limit: usize,
    progress: &mut F,
) -> Result<()>
where
    F: FnMut(CoreIntelligenceProgress) -> Result<()>,
{
    state.append = None;
    state.aggregate_generation = state.id;
    state.refind_done = false;
    state.refind_after = None;
    for (table, group) in build_state::TABLES {
        if *group == "aggregates" {
            state.counts.remove(*table);
        }
    }
    let saved = state.clone();
    vault.write(Db::Intelligence, Priority::Batch, move |s| build_state::save(s, &saved))?;
    super::build_stages::aggregates(vault, state, limit, progress)
}

fn is_keyword(s: &Session<'_>, generation: i64, visit_id: i64) -> Result<bool> {
    let kind = s
        .query_one(
            "SELECT query_kind FROM search_events_rows WHERE generation=?1 AND visit_id=?2",
            params![generation, visit_id],
            |r| Ok(r.get::<String>(0)?),
        )
        .optional()?;
    Ok(kind.is_some_and(|kind| parse_search_query_kind(&kind).is_keyword()))
}

/// Each page a new visit opened, with its row now: the stored row with the new visits added
/// (`add_to_refind_page`), or, for a page that had no row (it did not qualify before), the row
/// built from all its visits.
fn page_rows<F>(
    vault: &Vault,
    state: &BuildState,
    base: &AppendBase,
    limit: usize,
    progress: &mut F,
) -> Result<Vec<(String, Option<RefindPageRecord>)>>
where
    F: FnMut(CoreIntelligenceProgress) -> Result<()>,
{
    let (facts, navigation) = (state.facts_generation, state.navigation_generation);
    let (profile, after) = (state.profile_id.clone(), base.after_visit_id);
    let new_visits = vault.read(Db::Intelligence, move |s| {
        Ok(s.query(
            "SELECT f.canonical_url,f.visit_id,v.visit_time_ms,v.from_visit,a.trail_id
             FROM visit_derived_facts_rows f
             CROSS JOIN archive.visits v ON v.id=f.visit_id
             LEFT JOIN visit_assignments a INDEXED BY idx_assignments_visit_generation
               ON a.visit_id=f.visit_id AND a.generation=?4
             WHERE f.generation=?1 AND f.visit_id>?2 AND f.profile_id=?3 AND f.is_search_event=0",
            params![facts, after, profile, navigation],
            |r| {
                Ok((
                    r.get::<String>(0)?,
                    // (time, id), as `RefindAddition::visits` holds them.
                    (r.get::<i64>(2)?, r.get::<i64>(1)?),
                    Opener::of(r.get::<Option<i64>>(3)?) == Opener::Nothing,
                    r.get::<Option<String>>(4)?,
                ))
            },
        )?)
    })?;
    let mut by_page = std::collections::BTreeMap::<String, Vec<_>>::new();
    for (url, visit, typed, trail) in new_visits {
        by_page.entry(url).or_default().push((visit, typed, trail));
    }
    let total = by_page.len();
    let mut pages = Vec::with_capacity(total);
    let mut done = 0;
    let mut by_page = by_page.into_iter().peekable();
    while by_page.peek().is_some() {
        let chunk = by_page.by_ref().take(256).collect::<Vec<_>>();
        done += chunk.len();
        let (generation, profile) = (state.aggregate_generation, state.profile_id.clone());
        let zone = state.zone();
        let merged = vault.read(Db::Intelligence, move |s| {
            let mut merged = Vec::with_capacity(chunk.len());
            for (url, visits) in chunk {
                let Some((stored, ids, count)) = stored_refind_row(s, generation, &profile, &url)?
                else {
                    merged.push((url, None));
                    continue;
                };
                let mut addition = RefindAddition::default();
                let mut trails = BTreeSet::new();
                for ((time, id), typed, trail) in visits {
                    addition.days.insert(zone.date_key(time));
                    addition.visits.push((time, id));
                    addition.typed_revisits += i64::from(typed);
                    if let Some(trail) = trail {
                        addition.search_arrivals += 1;
                        trails.insert(trail);
                    }
                }
                for trail in trails {
                    let seen = s
                        .query_one(
                            "SELECT 1 FROM visit_assignments a
                               INDEXED BY idx_assignments_generation_trail
                             CROSS JOIN visit_derived_facts_rows f
                               ON f.generation=?3 AND f.visit_id=a.visit_id
                             WHERE a.generation=?1 AND a.trail_id IS NOT NULL AND a.trail_id=?2
                               AND a.visit_id<=?4 AND f.canonical_url=?5 LIMIT 1",
                            params![navigation, trail.as_str(), facts, after, url.as_str()],
                            |r| Ok(r.get::<i64>(0)?),
                        )
                        .optional()?
                        .is_some();
                    addition.new_trails += i64::from(!seen);
                }
                merged.push((url, Some(add_to_refind_page(zone, stored, &ids, count, addition))));
            }
            Ok(merged)
        })?;
        for (url, row) in merged {
            // A page without a row is read in full; it did not qualify, so it has few visits.
            let row = match row {
                Some(row) => Some(row),
                None => super::build_refind::page_record(vault, state, &url, limit)?,
            };
            pages.push((url, row));
        }
        progress(build_state::progress(state, "refind-pages", done, total))?;
    }
    Ok(pages)
}

/// A page's stored refind row, with its evidence sample of visit ids and its visit count.
fn stored_refind_row(
    s: &Session<'_>,
    generation: i64,
    profile: &str,
    url: &str,
) -> Result<Option<(RefindPageRecord, Vec<i64>, i64)>> {
    let row = s
        .query_one(
            "SELECT url,title,registrable_domain,cross_day_count,trail_count,search_arrival_count,
                    typed_revisit_count,refind_score,evidence_json,first_seen_ms,last_seen_ms
             FROM refind_pages_rows INDEXED BY idx_refind_pages_canonical
             WHERE canonical_url=?3 AND generation=?1 AND profile_id=?2",
            params![generation, profile, url],
            |r| {
                Ok(RefindPageRecord {
                    profile_id: profile.to_owned(),
                    canonical_url: url.to_owned(),
                    url: r.get(0)?,
                    title: r.get(1)?,
                    registrable_domain: r.get(2)?,
                    cross_day_count: r.get(3)?,
                    trail_count: r.get(4)?,
                    search_arrival_count: r.get(5)?,
                    typed_revisit_count: r.get(6)?,
                    refind_score: r.get::<f64>(7)? as f32,
                    evidence_json: r.get(8)?,
                    first_seen_ms: r.get(9)?,
                    last_seen_ms: r.get(10)?,
                    // Its days are stored already; an append adds only new ones.
                    days: Vec::new(),
                })
            },
        )
        .optional()?;
    let Some(row) = row else { return Ok(None) };
    let evidence: serde_json::Value = serde_json::from_str(&row.evidence_json)?;
    let ids = evidence["visitIds"]
        .as_array()
        .map(|ids| ids.iter().filter_map(serde_json::Value::as_i64).collect())
        .unwrap_or_default();
    let count = evidence["totalVisitCount"].as_i64().unwrap_or_default();
    Ok(Some((row, ids, count)))
}

/// What each flow gains: the paths of the days the append wrote, after it, less those of the
/// same days before it. `None` if a count went down, which an append cannot do (it only extends
/// the open segment and adds new ones): the stored flows are not what it assumes.
fn path_changes(
    vault: &Vault,
    state: &BuildState,
    base: &AppendBase,
) -> Result<Option<Vec<PathFlowRecord>>> {
    let (generation, profile, from_day) =
        (state.navigation_generation, state.profile_id.clone(), base.paths_from_day.clone());
    let after = vault.read(Db::Intelligence, move |s| {
        super::path_flows::day_flows(s, generation, &profile, &from_day)
    })?;
    let before = base
        .paths_before
        .iter()
        .map(|flow| ((flow.flow_pattern.as_str(), flow.step_count), flow))
        .collect::<HashMap<_, _>>();
    let kept = after
        .iter()
        .map(|flow| (flow.flow_pattern.as_str(), flow.step_count))
        .collect::<HashSet<_>>();
    if !before.keys().all(|key| kept.contains(key)) {
        return Ok(None);
    }
    let mut gains = Vec::new();
    for flow in &after {
        let (count, last) = before
            .get(&(flow.flow_pattern.as_str(), flow.step_count))
            .map_or((0, 0), |old| (old.occurrence_count, old.last_seen_ms));
        if flow.occurrence_count < count {
            return Ok(None);
        }
        if flow.occurrence_count > count || flow.last_seen_ms > last {
            gains.push(PathFlowRecord {
                occurrence_count: flow.occurrence_count - count,
                ..flow.clone()
            });
        }
    }
    Ok(Some(gains))
}

/// Walks the visits from the start of the trail still open at the last build (or the first new
/// visit) the way the full build walks them all: every trail seen is one the new visits joined
/// or started, and gets its compare sets again.
fn compare_sets_again<F>(
    vault: &Vault,
    state: &BuildState,
    base: &AppendBase,
    limit: usize,
    progress: &mut F,
) -> Result<(BTreeSet<String>, Vec<CompareSetRecord>)>
where
    F: FnMut(CoreIntelligenceProgress) -> Result<()>,
{
    let mut accumulator = CompareSetAccumulator::default();
    let mut trails = BTreeSet::new();
    let mut cursor = Some(base.walk_after);
    let mut walked = 0;
    loop {
        let batch = build_rows::derived_page(vault, state, cursor, limit)?;
        let Some(last) = batch.last() else { break };
        cursor = Some(VisibleVisitBatchCursor {
            visit_time_ms: last.visit_time_ms,
            visit_id: last.visit_id,
        });
        for visit in &batch {
            if let Some(trail) = &visit.trail_id {
                trails.insert(trail.clone());
            }
            accumulator.add(visit);
        }
        accumulator.resolve(vault, state.navigation_generation)?;
        walked += batch.len();
        progress(build_state::progress(state, "compare-sets", walked, state.dirty_visit_count))?;
    }
    Ok((trails, accumulator.finish(vault, state.navigation_generation)?))
}

/// The compare sets of `trails` again, each walked over its own span the way the full walk sees
/// it: from its search to its last visit, with the visits outside any trail in between, stopping
/// at another trail's visit.
fn trails_again(
    vault: &Vault,
    state: &BuildState,
    trails: &[String],
    limit: usize,
) -> Result<(BTreeSet<String>, Vec<CompareSetRecord>)> {
    let generation = state.navigation_generation;
    let ids = trails.to_vec();
    let mut spans = vault.read(Db::Intelligence, move |s| {
        let mut spans = Vec::new();
        for trail in ids {
            let span = s
                .query_one(
                    "SELECT t.first_visit_ms,t.last_visit_ms,m.visit_id FROM search_trails_rows t
                     JOIN search_trail_members_rows m
                       ON m.generation=t.generation AND m.trail_id=t.trail_id AND m.ordinal=0
                     WHERE t.generation=?1 AND t.trail_id=?2",
                    params![generation, trail.as_str()],
                    |r| Ok((r.get::<i64>(0)?, r.get::<i64>(1)?, r.get::<i64>(2)?)),
                )
                .optional()?;
            spans.extend(span.map(|span| (span, trail)));
        }
        Ok(spans)
    })?;
    spans.sort();
    let mut accumulator = CompareSetAccumulator::default();
    for ((first, last, search), trail) in &spans {
        let mut cursor =
            Some(VisibleVisitBatchCursor { visit_time_ms: *first, visit_id: search - 1 });
        'trail: loop {
            let batch = build_rows::derived_page(vault, state, cursor, limit)?;
            let Some(tail) = batch.last() else { break };
            cursor = Some(VisibleVisitBatchCursor {
                visit_time_ms: tail.visit_time_ms,
                visit_id: tail.visit_id,
            });
            for visit in &batch {
                let other = visit.trail_id.as_ref().is_some_and(|id| id != trail);
                if visit.visit_time_ms > *last || other {
                    break 'trail;
                }
                accumulator.add(visit);
            }
        }
        accumulator.resolve(vault, generation)?;
    }
    let sets = accumulator.finish(vault, generation)?;
    Ok((spans.into_iter().map(|(_, trail)| trail).collect(), sets))
}

/// The families the new keyword searches change, added in visit order to the stored families
/// as a full build would, and the ids of stored families that no longer exist. When the search
/// that opened the last build's open trail changed kind, the families are made again from every
/// search and compared with the stored ones. `None` when the stored families are not numbered
/// 1 to N.
fn family_changes<F>(
    vault: &Vault,
    state: &BuildState,
    base: &AppendBase,
    progress: &mut F,
) -> Result<Option<(Vec<QueryFamilyRecord>, Vec<String>)>>
where
    F: FnMut(CoreIntelligenceProgress) -> Result<()>,
{
    let Some(stored) = stored_families(vault, state)? else { return Ok(None) };
    let navigation = state.navigation_generation;
    let flipped = match base.open_search {
        Some((visit_id, was_keyword)) => {
            vault.read(Db::Intelligence, move |s| is_keyword(s, navigation, visit_id))?
                != was_keyword
        }
        None => false,
    };
    if flipped {
        let fresh = super::build_stages::load_families(vault, state, progress)?;
        let gone = stored.iter().skip(fresh.len()).map(|family| family.family_id.clone()).collect();
        let changed = fresh
            .into_iter()
            .enumerate()
            .filter(|(index, family)| stored.get(*index) != Some(family))
            .map(|(_, family)| family)
            .collect();
        return Ok(Some((changed, gone)));
    }
    let before = stored.iter().map(|family| family.member_count).collect::<Vec<_>>();
    // The last search the stored families took: the first new one may continue its trail.
    let (profile, after) = (state.profile_id.clone(), base.after_visit_id);
    let last = vault.read(Db::Intelligence, move |s| {
        Ok(s.query_one(
            "SELECT visit_id,profile_id,search_engine,raw_query,normalized_query,query_kind,
                    trail_id,visit_time_ms
             FROM search_events_rows INDEXED BY idx_search_events_profile_visit
             WHERE profile_id=?3 AND visit_id<=?2 AND generation=?1 AND query_kind='keyword'
             ORDER BY visit_id DESC LIMIT 1",
            params![navigation, after, profile],
            search_event,
        )
        .optional()?)
    })?;
    let mut accumulator = QueryFamilyAccumulator::resume(state.zone(), stored, last);
    let mut after = base.after_visit_id;
    loop {
        let profile = state.profile_id.clone();
        let events = vault.read(Db::Intelligence, move |s| {
            Ok(s.query(
                "SELECT visit_id,profile_id,search_engine,raw_query,normalized_query,query_kind,
                        trail_id,visit_time_ms
                 FROM search_events_rows
                 WHERE generation=?1 AND visit_id>?2 AND profile_id=?3 AND query_kind='keyword'
                 ORDER BY visit_id LIMIT 4096",
                params![navigation, after, profile],
                search_event,
            )?)
        })?;
        let Some(last) = events.last() else { break };
        after = last.visit_id;
        for event in &events {
            accumulator.add_event(event);
        }
    }
    let changed = accumulator
        .families()
        .iter()
        .enumerate()
        .filter(|(index, family)| before.get(*index) != Some(&family.member_count))
        .map(|(_, family)| family.clone())
        .collect();
    Ok(Some((changed, Vec::new())))
}

/// The profile's stored families in the order they were made, each with the days it was
/// searched on (kept on its reopened row when more than one). `None` when their numbers are not
/// 1 to N.
/// A row of `visit_id,profile_id,search_engine,raw_query,normalized_query,query_kind,trail_id,
/// visit_time_ms` from `search_events_rows`.
fn search_event(r: &vault_store::Row) -> vault_store::Result<SearchEventRecord> {
    Ok(SearchEventRecord {
        visit_id: r.get(0)?,
        profile_id: r.get(1)?,
        search_engine: r.get(2)?,
        raw_query: r.get(3)?,
        normalized_query: r.get(4)?,
        query_kind: parse_search_query_kind(&r.get::<String>(5)?),
        trail_id: r.get(6)?,
        visit_time_ms: r.get(7)?,
    })
}

fn stored_families(vault: &Vault, state: &BuildState) -> Result<Option<Vec<QueryFamilyRecord>>> {
    let generation = state.aggregate_generation;
    let mut families = Vec::new();
    let mut after = String::new();
    loop {
        let profile = state.profile_id.clone();
        let cursor = after.clone();
        let page = vault.read(Db::Intelligence, move |s| {
            Ok(s.query(
                "SELECT family_id,profile_id,anchor_query,member_count,search_engine,first_seen_ms,
                        last_seen_ms,queries_json
                 FROM query_families_rows WHERE generation=?1 AND family_id>?2 AND profile_id=?3
                 ORDER BY family_id LIMIT 4096",
                params![generation, cursor, profile],
                |r| {
                    let family = QueryFamilyRecord {
                        family_id: r.get(0)?,
                        profile_id: r.get(1)?,
                        anchor_query: r.get(2)?,
                        member_count: r.get(3)?,
                        search_engine: r.get(4)?,
                        first_seen_ms: r.get(5)?,
                        last_seen_ms: r.get(6)?,
                        queries: Vec::new(),
                        active_days: 1,
                    };
                    Ok((family, r.get::<String>(7)?))
                },
            )?)
        })?;
        let Some((last, _)) = page.last() else { break };
        after = last.family_id.clone();
        for (mut family, queries) in page {
            family.queries = serde_json::from_str(&queries)?;
            families.push(family);
        }
    }
    let number = |family: &QueryFamilyRecord| {
        family.family_id.rsplit(':').next().and_then(|number| number.parse::<usize>().ok())
    };
    families.sort_by_key(number);
    if families.iter().enumerate().any(|(index, family)| number(family) != Some(index + 1)) {
        return Ok(None);
    }
    let profile = state.profile_id.clone();
    let days = vault
        .read(Db::Intelligence, move |s| {
            Ok(s.query(
                "SELECT anchor_id,distinct_days FROM reopened_investigations_rows
                 WHERE generation=?1 AND investigation_id>=?2 AND investigation_id<?3
                   AND anchor_type='query_family'",
                params![
                    generation,
                    format!("reopened:{profile}:family:"),
                    format!("reopened:{profile}:family;")
                ],
                |r| Ok((r.get::<String>(0)?, r.get::<i64>(1)?)),
            )?)
        })?
        .into_iter()
        .collect::<HashMap<_, _>>();
    for family in &mut families {
        if let Some(days) = days.get(&family.family_id) {
            family.active_days = *days;
        }
    }
    Ok(Some(families))
}

/// Writes every change and adjusts the row counts, inside the caller's transaction.
fn write(s: &Session<'_>, state: &mut BuildState, changes: Changes) -> Result<()> {
    let generation = state.aggregate_generation;
    let profile = state.profile_id.clone();
    let at = state.computed_at.clone();
    let mut counts = HashMap::<&str, (usize, usize)>::new();
    let mut count = |table: &'static str, added: usize, removed: u64| {
        let entry = counts.entry(table).or_default();
        entry.0 += added;
        entry.1 += removed as usize;
    };

    let mut pages = Vec::new();
    for (url, row) in changes.pages {
        let removed = s.execute(
            "DELETE FROM refind_pages_rows WHERE generation=?1 AND profile_id=?2 AND canonical_url=?3",
            params![generation, profile.as_str(), url.as_str()],
        )?;
        count("refind_pages", 0, removed);
        let removed = s.execute(
            "DELETE FROM reopened_investigations_rows WHERE generation=?1 AND investigation_id=?2",
            params![generation, format!("reopened:{profile}:{url}")],
        )?;
        count("reopened_investigations", 0, removed);
        pages.extend(row);
    }
    let reopened_pages = build_reopened_investigations(&[], &pages);
    count("refind_pages", pages.len(), 0);
    count("reopened_investigations", reopened_pages.len(), 0);
    build_rows::persist_aggregates(s, generation, &pages, &reopened_pages, &[], &at)?;
    // Each record holds the days its new visits add (`RefindPageRecord::days`).
    let days = super::refind_days::by_day(&pages);
    count("refind_day_pages", super::refind_days::add(s, generation, &profile, days)?, 0);

    for flow in &changes.paths {
        // A new flow's row is inserted; a stored one gains the new count. Not one upsert with
        // `RETURNING`, which opens a temporary file per flow (`Session::insert`). The update names
        // the key's index: a lookup planned through the profile's rank index read every flow of
        // that length, 19 ms a flow, 94 s for an append's 4,812 at 14.4M visits.
        let key = || {
            params![
                generation,
                profile.as_str(),
                flow.flow_pattern.as_str(),
                flow.step_count,
                flow.occurrence_count,
                flow.last_seen_ms
            ]
        };
        let created = s
            .insert(
                "INSERT OR IGNORE INTO path_flows_rows
                   (generation,profile_id,flow_pattern,step_count,occurrence_count,last_seen_ms)
                 VALUES (?1,?2,?3,?4,?5,?6)",
                key(),
            )?
            .is_some();
        if !created {
            s.execute(
                "UPDATE path_flows_rows INDEXED BY sqlite_autoindex_path_flows_rows_1
                 SET occurrence_count=occurrence_count+?5, last_seen_ms=MAX(last_seen_ms,?6)
                 WHERE generation=?1 AND profile_id=?2 AND flow_pattern=?3 AND step_count=?4",
                key(),
            )?;
        }
        count("path_flows", usize::from(created), 0);
    }

    for trail in &changes.trails {
        let removed = s.execute(
            "DELETE FROM compare_sets_rows
             WHERE generation=?1 AND compare_set_id>=?2 AND compare_set_id<?3",
            params![generation, format!("compare:{trail}:"), format!("compare:{trail};")],
        )?;
        count("compare_sets", 0, removed);
    }
    count("compare_sets", changes.compare_sets.len(), 0);
    compare_sets::persist(s, generation, &changes.compare_sets, &at)?;

    let rewritten = changes.families.iter().map(|family| family.family_id.as_str());
    for family_id in rewritten.chain(changes.gone_families.iter().map(String::as_str)) {
        let removed = s.execute(
            "DELETE FROM query_families_rows WHERE generation=?1 AND family_id=?2",
            params![generation, family_id],
        )?;
        count("query_families", 0, removed);
        let removed = s.execute(
            "DELETE FROM query_family_queries_rows WHERE generation=?1 AND family_id=?2",
            params![generation, family_id],
        )?;
        count("query_family_queries", 0, removed);
        let removed = s.execute(
            "DELETE FROM reopened_investigations_rows WHERE generation=?1 AND investigation_id=?2",
            params![generation, format!("reopened:{profile}:{family_id}")],
        )?;
        count("reopened_investigations", 0, removed);
    }
    let reopened_families = build_reopened_investigations(&changes.families, &[]);
    count("query_families", changes.families.len(), 0);
    count(
        "query_family_queries",
        changes.families.iter().map(|family| build_rows::family_query_keys(family).len()).sum(),
        0,
    );
    count("reopened_investigations", reopened_families.len(), 0);
    build_rows::persist_families(s, generation, &changes.families, &at)?;
    build_rows::persist_aggregates(s, generation, &[], &reopened_families, &[], &at)?;

    for (table, (added, removed)) in counts {
        let value = state.counts.entry(table.into()).or_default();
        *value = (*value + added).saturating_sub(removed);
    }
    Ok(())
}
