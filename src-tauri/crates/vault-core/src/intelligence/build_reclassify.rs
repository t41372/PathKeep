//! Responsibilities: when only the rules that put pages and sites into categories changed (the
//! matcher, a rule pack, a category the user set for a site), sort the visits already built into
//! their new categories without building Insights anew: classify each of the profile's URLs once
//! (a category depends on the URL, its title and its search, never on one visit), rewrite the
//! facts of the visits whose categories moved, and note their days and search trails, whose
//! daily rollups and compare sets are made again later (`build_stages::daily`,
//! `build_append`).
//! Not responsible for: rule changes that move a visit's site or search (those start over);
//! sessions, trails, paths, refind pages and query families, which no category touches.
//! Failure modes: a URL whose categories moved is missed, or a moved visit's day or trail is not
//! made again; a resumed build redoes a page of URLs twice (harmless: the rewrite is idempotent
//! and the days and trails are sets saved with the cursor).
//!
//! `tests/reclassify.rs` compares facts, daily rollups and compare sets after a category change
//! with a build from scratch under the new rules.

use super::build_state::{self, BuildState};
use super::site_dictionary::{
    classify_visit, load_enabled_search_engine_rules, load_site_dictionary_overrides,
};
use super::{CoreIntelligenceProgress, VisibleVisitBatchCursor, build_rows, local_date_key};
use crate::Vault;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeSet, HashMap};
use vault_store::{Db, OptionalRow, Priority, params_owned as params};

/// URLs classified per read.
const URL_PAGE: usize = 1024;

/// A build that sorts the visits already built into new categories, and how far it got.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(super) struct Reclassify {
    /// Visits up to this id were built with the old categories; later ones are new and
    /// classified with the new rules when derived.
    pub up_to_visit_id: i64,
    /// The last URL id checked, and whether every URL has been.
    pub url_after: i64,
    pub urls_done: bool,
    /// The local days and search trails of the visits whose categories moved.
    pub days: BTreeSet<String>,
    pub trails: BTreeSet<String>,
    /// The last of `days` whose rollups were made again.
    pub day_after: Option<String>,
}

/// One URL's categories, as stored on each of its visits' facts.
#[derive(Debug, Clone, PartialEq)]
struct Categories {
    domain_category: String,
    page_category: String,
    taxonomy_source: String,
    taxonomy_pack: Option<String>,
    taxonomy_version: Option<String>,
}

/// Classifies the profile's URLs again, a page at a time, and rewrites the facts of the visits
/// whose categories moved. Each page's rewrite and the cursor commit together.
pub(super) fn reclassify<F>(vault: &Vault, state: &mut BuildState, progress: &mut F) -> Result<()>
where
    F: FnMut(CoreIntelligenceProgress) -> Result<()>,
{
    let Some(mut work) = state.reclassify.clone() else { return Ok(()) };
    if work.urls_done {
        return Ok(());
    }
    let (overrides, rules) = vault.read(Db::Intelligence, |s| {
        Ok((load_site_dictionary_overrides(s)?, load_enabled_search_engine_rules(s)?))
    })?;
    let mut checked = 0;
    loop {
        let (source, after) = (state.source_profile_id, work.url_after);
        let urls = vault.read(Db::Archive, move |s| {
            let urls = s.query(
                "SELECT id,url,title FROM urls WHERE source_profile_id=?1 AND id>?2
                 ORDER BY id LIMIT ?3",
                params![source, after, URL_PAGE as i64],
                |r| Ok((r.get::<i64>(0)?, r.get::<String>(1)?, r.get::<Option<String>>(2)?)),
            )?;
            // A URL's search, as `build_rows::source_page` reads it: its first live term.
            let mut terms = HashMap::new();
            for chunk in urls.chunks(400) {
                let slots = (2..chunk.len() + 2)
                    .map(|index| format!("(?{index})"))
                    .collect::<Vec<_>>()
                    .join(",");
                let mut values = vec![vault_store::Value::Integer(source)];
                values.extend(chunk.iter().map(|(id, ..)| vault_store::Value::Integer(*id)));
                s.for_each(
                    &format!(
                        "WITH requested(url_id) AS (VALUES {slots})
                         SELECT t.url_id,t.normalized_term FROM requested
                         CROSS JOIN search_terms t INDEXED BY idx_search_terms_profile_url_term
                           ON t.url_id=requested.url_id
                         WHERE t.source_profile_id=?1 AND t.reverted_at IS NULL ORDER BY t.id"
                    ),
                    values,
                    |r| {
                        terms.entry(r.get::<i64>(0)?).or_insert(r.get::<String>(1)?);
                        Ok(())
                    },
                )?;
            }
            Ok((urls, terms))
        })?;
        let (urls, terms) = urls;
        let Some(&(last, ..)) = urls.last() else { break };
        let classified = urls
            .iter()
            .map(|(id, url, title)| {
                let term = terms.get(id).map(String::as_str);
                let entry = classify_visit(
                    url,
                    title.as_deref(),
                    term,
                    term.is_some(),
                    None,
                    None,
                    &overrides,
                    &rules,
                );
                let categories = Categories {
                    domain_category: entry.domain_category,
                    page_category: entry.page_category,
                    taxonomy_source: entry.taxonomy_source,
                    taxonomy_pack: entry.taxonomy_pack,
                    taxonomy_version: entry.taxonomy_version,
                };
                (*id, categories)
            })
            .collect::<Vec<_>>();
        work.url_after = last;
        checked += urls.len();
        let mut saved = state.clone();
        let mut next = work.clone();
        *state = vault.write(Db::Intelligence, Priority::Batch, move |s| {
            for (url_id, categories) in &classified {
                // Every visit of a URL carries the same categories, so one stands for all.
                let stored = s
                    .query_one(
                        "SELECT f.domain_category,f.page_category,f.taxonomy_source,
                                f.taxonomy_pack,f.taxonomy_version
                         FROM archive.visits v INDEXED BY idx_visits_visible_url_time
                         CROSS JOIN visit_derived_facts_rows f ON f.generation=?1 AND f.visit_id=v.id
                         WHERE v.url_id=?2 AND v.reverted_at IS NULL AND v.id<=?3 LIMIT 1",
                        params![saved.facts_generation, *url_id, next.up_to_visit_id],
                        |r| {
                            Ok(Categories {
                                domain_category: r.get(0)?,
                                page_category: r.get(1)?,
                                taxonomy_source: r.get(2)?,
                                taxonomy_pack: r.get(3)?,
                                taxonomy_version: r.get(4)?,
                            })
                        },
                    )
                    .optional()?;
                let Some(stored) = stored.filter(|stored| stored != categories) else {
                    continue;
                };
                // A new pack version alone rewrites the facts; only a category that moved changes
                // what the rollups and compare sets hold.
                let moved_category = stored.domain_category != categories.domain_category
                    || stored.page_category != categories.page_category;
                let moved = s.query(
                    "SELECT v.id,v.visit_time_ms,a.trail_id
                     FROM archive.visits v INDEXED BY idx_visits_visible_url_time
                     LEFT JOIN visit_assignments a INDEXED BY idx_assignments_visit_generation
                       ON a.visit_id=v.id AND a.generation=?3
                     WHERE v.url_id=?1 AND v.reverted_at IS NULL AND v.id<=?2",
                    params![*url_id, next.up_to_visit_id, saved.navigation_generation],
                    |r| Ok((r.get::<i64>(0)?, r.get::<i64>(1)?, r.get::<Option<String>>(2)?)),
                )?;
                for (visit_id, time, trail) in moved {
                    s.execute(
                        "UPDATE visit_derived_facts_rows SET domain_category=?3,page_category=?4,
                           taxonomy_source=?5,taxonomy_pack=?6,taxonomy_version=?7
                         WHERE generation=?1 AND visit_id=?2",
                        params![
                            saved.facts_generation,
                            visit_id,
                            categories.domain_category.as_str(),
                            categories.page_category.as_str(),
                            categories.taxonomy_source.as_str(),
                            categories.taxonomy_pack.as_deref(),
                            categories.taxonomy_version.as_deref()
                        ],
                    )?;
                    if moved_category {
                        next.days.insert(local_date_key(time));
                        next.trails.extend(trail);
                    }
                }
            }
            saved.reclassify = Some(next);
            build_state::save(s, &saved)?;
            Ok(saved)
        })?;
        work = state.reclassify.clone().unwrap_or_default();
        progress(build_state::progress(state, "reclassify", checked, checked))?;
    }
    work.urls_done = true;
    state.reclassify = Some(work);
    let saved = state.clone();
    vault.write(Db::Intelligence, Priority::Batch, move |s| build_state::save(s, &saved))
}

/// Makes the daily rollups of each day a moved visit fell on again, one day at a time, before
/// the daily stage extends the rollups with new visits. A day's category rows are removed first,
/// since a category can leave a day; its other rows are replaced by key.
pub(super) fn days<F>(
    vault: &Vault,
    state: &mut BuildState,
    limit: usize,
    progress: &mut F,
) -> Result<()>
where
    F: FnMut(CoreIntelligenceProgress) -> Result<()>,
{
    let Some(work) = state.reclassify.clone() else { return Ok(()) };
    let pending = work
        .days
        .iter()
        .filter(|day| work.day_after.as_ref().is_none_or(|done| *day > done))
        .cloned()
        .collect::<Vec<_>>();
    let total = pending.len();
    for (index, day) in pending.into_iter().enumerate() {
        let range = crate::models::DateRange { start: day.clone(), end: day.clone() };
        let (start, end) = super::date_range_bounds(&range)?;
        let mut cursor =
            Some(VisibleVisitBatchCursor { visit_time_ms: start - 1, visit_id: i64::MAX });
        let mut accumulator =
            super::intelligence_daily_rollup_state::DailyRollupAccumulator::default();
        'day: loop {
            let batch = build_rows::derived_page(vault, state, cursor, limit)?;
            let Some(last) = batch.last() else { break };
            cursor = Some(VisibleVisitBatchCursor {
                visit_time_ms: last.visit_time_ms,
                visit_id: last.visit_id,
            });
            for visit in &batch {
                if visit.visit_time_ms >= end {
                    break 'day;
                }
                accumulator.add_visit(visit);
            }
        }
        let bundle = accumulator.finish();
        let mut saved = state.clone();
        if let Some(work) = saved.reclassify.as_mut() {
            work.day_after = Some(day.clone());
        }
        *state = vault.write(Db::Intelligence, Priority::Batch, move |s| {
            s.execute(
                "DELETE FROM category_daily_rollups_rows
                 WHERE generation=?1 AND date_key=?2 AND profile_id=?3",
                params![saved.daily_generation, day.as_str(), saved.profile_id.as_str()],
            )?;
            build_rows::persist_daily(s, saved.daily_generation, &bundle)?;
            s.execute(
                "INSERT INTO intelligence_day_counts VALUES(?1,?2,?3,?4,?5,?6,?7)
                 ON CONFLICT(generation,profile_id,date_key) DO UPDATE SET
                   domains=excluded.domains,categories=excluded.categories,
                   engines=excluded.engines,summaries=excluded.summaries",
                params![
                    saved.daily_generation,
                    saved.profile_id.as_str(),
                    day.as_str(),
                    bundle.domain_rows.len() as i64,
                    bundle.category_rows.len() as i64,
                    bundle.engine_rows.len() as i64,
                    bundle.summary_rows.len() as i64
                ],
            )?;
            build_state::save(s, &saved)?;
            Ok(saved)
        })?;
        progress(build_state::progress(state, "daily-rollups", index + 1, total))?;
    }
    Ok(())
}
