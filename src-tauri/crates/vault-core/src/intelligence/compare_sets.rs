//! Responsibilities: find compare sets while the aggregate build streams a profile's visits,
//! store them, and read them back by range, newest first.
//! Not responsible for: what counts as a compare set (`phase_four::build_compare_sets_for_trail`)
//! or the detail of one set (`phase_four::get_compare_set_detail`, bounded by its trail).
//! Failure modes: a read that rebuilds sets from every trail member in the range (2.5 s on a
//! 387k-visit archive, millions of rows for all time at 14.4M), which is what this replaced; a
//! trail cut across stream batches losing pages; a set missing its trail's query or landing.

use super::phase_four::{CompareTrailMember, build_compare_sets_for_trail};
use super::{VisitRecord, date_range_bounds, rfc3339_from_millis};
use crate::Vault;
use crate::models::{CompareSet, CompareSetPage, IntelligencePage, ScopedPageRequest};
use anyhow::Result;
use std::collections::HashMap;
use vault_store::{Db, Session, params_owned as params};

/// One stored compare set: the set, whose browser profile it is and when it happened.
#[derive(Debug, Clone)]
pub(super) struct CompareSetRecord {
    pub profile_id: String,
    pub set: CompareSet,
    pub first_visit_ms: i64,
    pub last_visit_ms: i64,
}

/// Collects each search trail's pages as visits stream past in time order (one trail at a time,
/// with pages outside it in between), and turns finished trails into compare sets once their
/// query and landing are loaded, one query per batch.
#[derive(Debug, Default)]
pub(super) struct CompareSetAccumulator {
    profile_id: String,
    current: Vec<CompareTrailMember>,
    current_trail: Option<String>,
    finished: Vec<Vec<CompareTrailMember>>,
    pub sets: Vec<CompareSetRecord>,
}

impl CompareSetAccumulator {
    pub fn add(&mut self, visit: &VisitRecord) {
        // A trail's pages can have other pages of the session between them; it ends when the
        // next trail starts (trails follow one another, a search at a time).
        let Some(trail) = visit.trail_id.as_ref() else { return };
        if self.current_trail.as_ref() != Some(trail) {
            self.close_trail();
            self.current_trail = Some(trail.clone());
        }
        self.profile_id.clone_from(&visit.profile_id);
        if visit.is_search_event {
            return;
        }
        if super::phase_four::is_compare_candidate_category(&visit.page_category) {
            self.current.push(CompareTrailMember {
                trail_id: visit.trail_id.clone().unwrap_or_default(),
                initial_query: String::new(),
                landing_url: None,
                canonical_url: visit.canonical_url.clone(),
                url: visit.url.clone(),
                title: visit.title.clone(),
                registrable_domain: visit.registrable_domain.clone(),
                page_category: visit.page_category.clone(),
                visit_time_ms: visit.visit_time_ms,
            });
        }
    }

    /// A set needs two pages and two switches between them, so three pages at least.
    fn close_trail(&mut self) {
        let members = std::mem::take(&mut self.current);
        if members.len() >= 3 {
            self.finished.push(members);
        }
    }

    /// Builds sets for the trails finished so far. `generation` is the navigation generation
    /// the trails were written under.
    pub fn resolve(&mut self, vault: &Vault, generation: i64) -> Result<()> {
        if self.finished.is_empty() {
            return Ok(());
        }
        let trails = std::mem::take(&mut self.finished);
        let ids: Vec<String> = trails.iter().map(|members| members[0].trail_id.clone()).collect();
        let profile = self.profile_id.clone();
        let known = vault.read(Db::Intelligence, move |s| {
            let mut known = HashMap::new();
            for chunk in ids.chunks(400) {
                let slots = (3..chunk.len() + 3).map(|i| format!("(?{i})")).collect::<Vec<_>>();
                let mut values = vec![
                    vault_store::Value::Integer(generation),
                    vault_store::Value::Text(profile.clone()),
                ];
                values.extend(chunk.iter().cloned().map(vault_store::Value::Text));
                s.for_each(
                    &format!(
                        // Joined to the ids, so each one seeks the key; `trail_id IN (...)`
                        // was planned as a read of the whole generation.
                        "WITH requested(trail_id) AS (VALUES {})
                         SELECT t.trail_id,t.initial_query,t.landing_url FROM requested
                         CROSS JOIN search_trails_rows t
                           ON t.generation=?1 AND t.trail_id=requested.trail_id
                         WHERE t.profile_id=?2",
                        slots.join(",")
                    ),
                    values,
                    |r| {
                        known.insert(
                            r.get::<String>(0)?,
                            (r.get::<String>(1)?, r.get::<Option<String>>(2)?),
                        );
                        Ok(())
                    },
                )?;
            }
            Ok(known)
        })?;
        for mut members in trails {
            let Some((query, landing)) = known.get(&members[0].trail_id) else { continue };
            for member in &mut members {
                member.initial_query.clone_from(query);
                member.landing_url.clone_from(landing);
            }
            for set in build_compare_sets_for_trail(&members) {
                let times = members
                    .iter()
                    .filter(|member| member.page_category == set.page_category)
                    .map(|member| member.visit_time_ms);
                let (first, last) =
                    times.fold((i64::MAX, i64::MIN), |(lo, hi), time| (lo.min(time), hi.max(time)));
                self.sets.push(CompareSetRecord {
                    profile_id: self.profile_id.clone(),
                    set: CompareSet { last_visited_at: rfc3339_from_millis(last), ..set },
                    first_visit_ms: first,
                    last_visit_ms: last,
                });
            }
        }
        Ok(())
    }

    /// Ends the last trail and builds what is left.
    pub fn finish(&mut self, vault: &Vault, generation: i64) -> Result<Vec<CompareSetRecord>> {
        self.close_trail();
        self.resolve(vault, generation)?;
        Ok(std::mem::take(&mut self.sets))
    }
}

pub(super) fn persist(
    s: &Session<'_>,
    generation: i64,
    rows: &[CompareSetRecord],
    computed_at: &str,
) -> Result<()> {
    let mut insert = s.prepare(
        "INSERT OR REPLACE INTO compare_sets_rows
         (generation,compare_set_id,profile_id,trail_id,search_query,page_category,
          first_visit_ms,last_visit_ms,pages_json,computed_at)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)",
    )?;
    for row in rows {
        insert.execute(params![
            generation,
            row.set.compare_set_id.as_str(),
            row.profile_id.as_str(),
            row.set.trail_id.as_str(),
            row.set.search_query.as_str(),
            row.set.page_category.as_str(),
            row.first_visit_ms,
            row.last_visit_ms,
            serde_json::to_string(&row.set.pages)?,
            computed_at
        ])?;
    }
    Ok(())
}

/// Compare sets that happened in the range, newest first, a page at a time.
pub fn get_compare_sets(
    vault: &Vault,
    request: &ScopedPageRequest,
) -> Result<IntelligencePage<CompareSet>> {
    let request = request.clone();
    vault.read(Db::Intelligence, move |s| get_compare_sets_with_connection(s, &request))
}

pub(crate) fn get_compare_sets_with_connection(
    s: &Session<'_>,
    request: &ScopedPageRequest,
) -> Result<IntelligencePage<CompareSet>> {
    let (start, end) = date_range_bounds(s, &request.date_range)?;
    let limit = request.limit.unwrap_or(6).clamp(1, 1000) as usize;
    let after = super::paging::cursor::<(i64, String)>(request.cursor.as_deref())?;
    // Bounded cursor clause, only after the first page: the form Turso seeks on.
    let after_clause = if after.is_some() {
        "AND r.last_visit_ms<=?4 AND (r.last_visit_ms<?4 OR r.compare_set_id<?5)"
    } else {
        ""
    };
    let rows = s.query(
        &format!(
            "SELECT r.compare_set_id,r.trail_id,r.search_query,r.page_category,r.pages_json,r.last_visit_ms
             FROM compare_sets_rows r INDEXED BY idx_compare_sets_recent
             WHERE r.generation IN (SELECT aggregates FROM intelligence_generations)
               AND (?1 IS NULL OR r.profile_id=?1) AND r.last_visit_ms>=?2 AND r.first_visit_ms<?3
               {after_clause}
             ORDER BY r.last_visit_ms DESC,r.compare_set_id DESC LIMIT ?6"
        ),
        params![
            request.profile_id.as_deref(),
            start,
            end,
            after.as_ref().map(|value| value.0),
            after.as_ref().map(|value| value.1.as_str()),
            (limit + 1) as i64
        ],
        |r| {
            let id = r.get::<String>(0)?;
            let last = r.get::<i64>(5)?;
            let pages: Vec<CompareSetPage> =
                serde_json::from_str(&r.get::<String>(4)?).unwrap_or_default();
            Ok((
                CompareSet {
                    compare_set_id: id.clone(),
                    trail_id: r.get(1)?,
                    search_query: r.get(2)?,
                    page_category: r.get(3)?,
                    pages,
                    last_visited_at: rfc3339_from_millis(last),
                },
                (last, id),
            ))
        },
    )?;
    super::paging::finish(rows, limit)
}
