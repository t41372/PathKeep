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
                let slots = (3..chunk.len() + 3).map(|i| format!("?{i}")).collect::<Vec<_>>();
                let mut values = vec![
                    vault_store::Value::Integer(generation),
                    vault_store::Value::Text(profile.clone()),
                ];
                values.extend(chunk.iter().cloned().map(vault_store::Value::Text));
                s.for_each(
                    &format!(
                        "SELECT trail_id,initial_query,landing_url FROM search_trails_rows
                         WHERE generation=?1 AND profile_id=?2 AND trail_id IN ({})",
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
    let (start, end) = date_range_bounds(&request.date_range)?;
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::DateRange;

    fn visit(id: i64, trail: Option<&str>, url: &str, category: &str, search: bool) -> VisitRecord {
        let domain = url.split('/').nth(2).unwrap_or_default().to_string();
        VisitRecord {
            visit_id: id,
            profile_id: "p".into(),
            source_profile_id: 1,
            source_visit_id: id,
            source_url_id: id,
            url: url.into(),
            title: Some(format!("Page {id}")),
            visit_time_ms: id * 60_000,
            from_visit: None,
            transition_type: None,
            external_referrer_url: None,
            canonical_url: url.into(),
            registrable_domain: domain,
            domain_category: "shopping".into(),
            page_category: category.into(),
            search_engine: search.then(|| "google".into()),
            search_query: search.then(|| "a7c vs x-t5".into()),
            is_new_domain: false,
            is_search_event: search,
            evidence_tier: "tier-a".into(),
            taxonomy_source: "test".into(),
            taxonomy_pack: None,
            taxonomy_version: None,
            display_name: None,
            session_id: Some("s".into()),
            trail_id: trail.map(Into::into),
        }
    }

    fn read(
        vault: &Vault,
        profile: Option<&str>,
        cursor: Option<String>,
    ) -> IntelligencePage<CompareSet> {
        get_compare_sets(
            vault,
            &ScopedPageRequest {
                date_range: DateRange { start: "1970-01-01".into(), end: "1970-01-02".into() },
                profile_id: profile.map(Into::into),
                limit: Some(1),
                cursor,
            },
        )
        .unwrap()
    }

    /// Two trails switch back and forth between product pages on two sites and become sets; a
    /// trail with two pages, and the pages outside any trail, do not. The read pages through
    /// them newest first and keeps to the profile asked for.
    #[test]
    fn sets_found_while_streaming_are_read_back_newest_first() {
        let vault = Vault::open_temp().unwrap();
        vault
            .write(Db::Intelligence, vault_store::Priority::Batch, |s| {
                s.execute("INSERT INTO intelligence_generations(profile_id,navigation,aggregates) VALUES('p',1,1)", ())?;
                for (trail, query, landing) in [
                    ("t1", "a7c vs x-t5", "https://shop-a.example/a7c"),
                    ("t2", "lens hood", "https://shop-a.example/hood"),
                    ("t3", "fuji x100", "https://shop-b.example/x100"),
                ] {
                    s.execute(
                        "INSERT INTO search_trails_rows(generation,trail_id,profile_id,session_id,initial_query,search_engine,
                           reformulation_count,visit_count,landing_url,landing_domain,first_visit_ms,last_visit_ms,max_depth,
                           queries_json,computed_at) VALUES(1,?1,'p','s',?2,'google',0,1,?3,NULL,0,0,0,'[]','')",
                        params![trail, query, landing],
                    )?;
                }
                Ok(())
            })
            .unwrap();
        let (a, b) = ("https://shop-a.example/a7c", "https://shop-b.example/xt5");
        let mut sets = CompareSetAccumulator::default();
        for record in [
            visit(1, Some("t1"), "https://www.google.com/search", "search_results", true),
            visit(2, Some("t1"), a, "product_page", false),
            visit(3, Some("t1"), b, "product_page", false),
            visit(4, Some("t1"), a, "product_page", false),
            visit(5, None, "https://news.example/", "article_page", false),
            visit(6, Some("t2"), "https://shop-a.example/hood", "product_page", false),
            visit(7, Some("t2"), "https://shop-b.example/hood", "product_page", false),
            visit(8, Some("t3"), "https://shop-b.example/x100", "product_page", false),
            visit(9, Some("t3"), "https://shop-c.example/x100", "product_page", false),
            visit(10, Some("t3"), "https://shop-b.example/x100", "product_page", false),
        ] {
            sets.add(&record);
            if record.visit_id == 4 {
                sets.resolve(&vault, 1).unwrap();
            }
        }
        let found = sets.finish(&vault, 1).unwrap();
        assert_eq!(found.len(), 2, "t1 and t3; t2 has two pages and one switch");
        vault
            .write(Db::Intelligence, vault_store::Priority::Batch, move |s| {
                persist(s, 1, &found, "")
            })
            .unwrap();

        let first = read(&vault, None, None);
        assert_eq!(first.items[0].search_query, "fuji x100");
        let second = read(&vault, None, first.next_cursor.clone());
        let set = &second.items[0];
        assert_eq!(set.search_query, "a7c vs x-t5");
        assert_eq!(set.compare_set_id, "compare:t1:product_page");
        assert_eq!(set.pages.len(), 2);
        assert!(set.pages.iter().any(|page| page.url == a && page.is_landing));
        assert_eq!(set.last_visited_at, rfc3339_from_millis(4 * 60_000));
        assert!(second.next_cursor.is_none());
        assert!(read(&vault, Some("other"), None).items.is_empty());
    }
}
