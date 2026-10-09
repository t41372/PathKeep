//! Responsibilities: stream canonical-page aggregates without retaining the archive's page set.
//! Not responsible for: sessions, query families or publishing generations.
//! Failure modes: canonical groups straddle a batch, a restart duplicates pages, chronological
//! provenance changes with key order, or one popular page prevents cancellation between reads.
use super::intelligence_structural_aggregates::{
    StructuralAggregateAccumulator, build_reopened_investigations,
};
use super::{
    CoreIntelligenceProgress, RefindPageRecord, VisitRecord, build_rows,
    build_state::{self, BuildState},
};
use crate::Vault;
use anyhow::Result;
use vault_store::{Db, Priority, params_owned as params};

const COLUMNS:&str="SELECT f.visit_id,f.profile_id,f.canonical_url,f.registrable_domain,
    u.url,u.title,v.visit_time_ms,v.from_visit,a.trail_id
    FROM visit_derived_facts_rows f INDEXED BY idx_vdf_generation_canonical_visit
    CROSS JOIN archive.visits v ON v.id=f.visit_id
    CROSS JOIN archive.urls u ON u.id=v.url_id
    LEFT JOIN visit_assignments a INDEXED BY idx_assignments_visit_generation ON a.visit_id=f.visit_id AND a.generation=?3
    WHERE f.generation=?1 AND f.profile_id=?2 AND f.is_search_event=0";

fn page(
    vault: &Vault,
    state: &BuildState,
    after: Option<(String, i64)>,
    limit: usize,
) -> Result<Vec<VisitRecord>> {
    let facts = state.facts_generation;
    let navigation = state.navigation_generation;
    let profile = state.profile_id.clone();
    vault.read(Db::Intelligence,move |s| {
        let map=|r:&vault_store::Row|->vault_store::Result<VisitRecord> {Ok(VisitRecord {
            visit_id:r.get(0)?,profile_id:r.get(1)?,canonical_url:r.get(2)?,registrable_domain:r.get(3)?,url:r.get(4)?,title:r.get(5)?,visit_time_ms:r.get(6)?,from_visit:r.get(7)?,trail_id:r.get(8)?,
            source_profile_id:0,source_visit_id:0,source_url_id:0,transition_type:None,external_referrer_url:None,domain_category:String::new(),page_category:String::new(),search_engine:None,search_query:None,is_new_domain:false,is_search_event:false,evidence_tier:String::new(),taxonomy_source:String::new(),taxonomy_pack:None,taxonomy_version:None,display_name:None,session_id:None,
        })};
        let mut rows;
        if let Some((key,id))=after {
            rows=s.query(&format!("{COLUMNS} AND f.canonical_url=?4 AND f.visit_id>?5 ORDER BY f.canonical_url,f.visit_id LIMIT ?6"),params![facts,profile,navigation,key,id,limit as i64],map)?;
            if rows.len()<limit {
                rows.extend(s.query(&format!("{COLUMNS} AND f.canonical_url>?4 ORDER BY f.canonical_url,f.visit_id LIMIT ?5"),params![facts,profile,navigation,key,(limit-rows.len()) as i64],map)?);
            }
        } else {
            rows=s.query(&format!("{COLUMNS} ORDER BY f.canonical_url,f.visit_id LIMIT ?4"),params![facts,profile,navigation,limit as i64],map)?;
        }
        Ok(rows)
    })
}

pub(super) fn rebuild<F>(
    vault: &Vault,
    state: &mut BuildState,
    limit: usize,
    progress: &mut F,
) -> Result<()>
where
    F: FnMut(CoreIntelligenceProgress) -> Result<()>,
{
    if state.refind_done {
        return Ok(());
    }
    let mut cursor = state.refind_after.clone().map(|key| (key, i64::MAX));
    let mut current = None::<String>;
    let mut first = None::<(i64, i64)>;
    let mut accumulator = StructuralAggregateAccumulator::default();
    let mut batch = Vec::new();
    let mut scanned = 0;
    let mut completed_key = None;
    let mut completed_groups = 0;
    loop {
        let rows = page(vault, state, cursor.clone(), limit)?;
        if rows.is_empty() {
            break;
        }
        for visit in &rows {
            if current.as_deref().is_some_and(|key| key != visit.canonical_url) {
                batch.extend(std::mem::take(&mut accumulator).finish().0);
                completed_key = current.take();
                first = None;
                completed_groups += 1;
                if completed_groups >= 512 {
                    persist(vault, state, &mut batch, completed_key.take())?;
                    completed_groups = 0;
                }
            }
            current = Some(visit.canonical_url.clone());
            // Input is indexed by page and visit ID. The reference builder takes its raw
            // URL/title from the chronologically first visit, including timestamp ties.
            let order = (visit.visit_time_ms, visit.visit_id);
            let earliest = first.is_none_or(|old| order < old);
            first = Some(first.map_or(order, |old| old.min(order)));
            accumulator.add_refind_visit(visit, earliest);
        }
        let last = rows.last().unwrap();
        cursor = Some((last.canonical_url.clone(), last.visit_id));
        scanned += rows.len();
        progress(build_state::progress(state, "refind-pages", scanned, state.processed))?;
    }
    if current.is_some() {
        batch.extend(accumulator.finish().0);
        completed_key = current;
    }
    persist(vault, state, &mut batch, completed_key)?;
    state.refind_done = true;
    let saved = state.clone();
    vault.write(Db::Intelligence, Priority::Batch, move |s| build_state::save(s, &saved))
}
fn persist(
    vault: &Vault,
    state: &mut BuildState,
    rows: &mut Vec<RefindPageRecord>,
    after: Option<String>,
) -> Result<()> {
    let rows = std::mem::take(rows);
    let reopened = build_reopened_investigations(&[], &rows);
    *state.counts.entry("refind_pages".into()).or_default() += rows.len();
    *state.counts.entry("reopened_investigations".into()).or_default() += reopened.len();
    if after.is_some() {
        state.refind_after = after;
    }
    let saved = state.clone();
    vault.write(Db::Intelligence, Priority::Batch, move |s| {
        build_rows::persist_aggregates(
            s,
            saved.aggregate_generation,
            &rows,
            &[],
            &reopened,
            &[],
            &saved.computed_at,
        )?;
        build_state::save(s, &saved)
    })
}
