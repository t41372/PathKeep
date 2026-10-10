//! Responsibilities: page archive facts and persist one derived batch.
//! Not responsible for: choosing rebuild modes, publication or queue leases.
//! Failure modes: tied timestamps skip visits, a read pins the whole archive,
//! source search terms disappear, or a batch writes into another generation.

use super::build_state::BuildState;
use super::{VisibleVisitBatchCursor, VisitRecord};
use crate::Vault;
use anyhow::Result;
use vault_store::{Db, Session, params_owned as params};

const SOURCE_COLUMNS: &str =
    "SELECT v.id,?1,v.source_profile_id,COALESCE(CAST(v.source_visit_id AS INTEGER),0),
    u.id,u.url,u.title,v.visit_time_ms,v.from_visit,v.transition_type,v.external_referrer_url
    FROM visits v INDEXED BY idx_visits_visible_profile_time_id JOIN urls u ON u.id=v.url_id
    WHERE v.source_profile_id=?2 AND v.reverted_at IS NULL AND v.id<=?3";

pub(super) fn source_page(
    vault: &Vault,
    state: &BuildState,
    after: Option<VisibleVisitBatchCursor>,
    limit: usize,
) -> Result<Vec<VisitRecord>> {
    let profile = state.profile_id.clone();
    let source_id = state.source_profile_id;
    let upper = state.source.max_visit_id;
    vault.read(Db::Archive,move |s| {
        let map=super::intelligence_visit_records::visit_from_row;
        let mut visits=if let Some(after)=after {
            let sql=format!("{SOURCE_COLUMNS} AND v.visit_time_ms=?4 AND v.id>?5 ORDER BY v.visit_time_ms,v.id LIMIT ?6");
            let mut rows=s.query(&sql,params![profile,source_id,upper,after.visit_time_ms,after.visit_id,limit as i64],map)?;
            if rows.len()<limit {
                let sql=format!("{SOURCE_COLUMNS} AND v.visit_time_ms>?4 ORDER BY v.visit_time_ms,v.id LIMIT ?5");
                rows.extend(s.query(&sql,params![profile,source_id,upper,after.visit_time_ms,(limit-rows.len()) as i64],map)?);
            }
            rows
        } else {
            s.query(&format!("{SOURCE_COLUMNS} ORDER BY v.visit_time_ms,v.id LIMIT ?4"),params![profile,source_id,upper,limit as i64],map)?
        };
        // Terms are hydrated in the same short archive snapshot. Classification happens later.
        let ids=visits.iter().map(|v|v.source_url_id).collect::<std::collections::BTreeSet<_>>();
        let mut terms=std::collections::HashMap::new();
        for chunk in ids.into_iter().collect::<Vec<_>>().chunks(400) {
            let slots=(2..chunk.len()+2).map(|index|format!("(?{index})")).collect::<Vec<_>>().join(",");
            let mut values=vec![vault_store::Value::Integer(source_id)];
            values.extend(chunk.iter().copied().map(vault_store::Value::Integer));
            s.for_each(&format!("WITH requested(url_id) AS (VALUES {slots}) SELECT t.url_id,t.normalized_term FROM requested CROSS JOIN search_terms t INDEXED BY idx_search_terms_profile_url_term ON t.url_id=requested.url_id WHERE t.source_profile_id=?1 AND t.reverted_at IS NULL ORDER BY t.id"),values,|r| {
                terms.entry(r.get::<i64>(0)?).or_insert(r.get::<String>(1)?);Ok(())
            })?;
        }
        for visit in &mut visits { visit.search_query=terms.get(&visit.source_url_id).cloned(); }
        Ok(visits)
    })
}

pub(super) fn persist_facts(
    s: &Session<'_>,
    state: &BuildState,
    visits: &[VisitRecord],
) -> Result<()> {
    let mut insert=s.prepare("INSERT OR REPLACE INTO visit_derived_facts_rows
        (generation,visit_id,profile_id,registrable_domain,canonical_url,domain_category,page_category,
        search_engine,search_query,is_new_domain,is_search_event,evidence_tier,taxonomy_source,taxonomy_pack,taxonomy_version,computed_at)
        VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16)")?;
    let mut seen=s.prepare("INSERT INTO intelligence_seen_domains(generation,profile_id,domain) VALUES(?1,?2,?3) ON CONFLICT DO NOTHING")?;
    for v in visits {
        insert.execute(params![
            state.facts_generation,
            v.visit_id,
            v.profile_id,
            v.registrable_domain,
            v.canonical_url,
            v.domain_category,
            v.page_category,
            v.search_engine,
            v.search_query,
            i64::from(v.is_new_domain),
            i64::from(v.is_search_event),
            v.evidence_tier,
            v.taxonomy_source,
            v.taxonomy_pack,
            v.taxonomy_version,
            state.computed_at
        ])?;
        if v.is_new_domain {
            seen.execute(params![state.facts_generation, v.profile_id, v.registrable_domain])?;
        }
    }
    Ok(())
}

pub(super) fn derived_page(
    vault: &Vault,
    state: &BuildState,
    after: Option<VisibleVisitBatchCursor>,
    limit: usize,
) -> Result<Vec<VisitRecord>> {
    let mut visits = source_page(vault, state, after, limit)?;
    if visits.is_empty() {
        return Ok(visits);
    }
    let ids = visits.iter().map(|v| v.visit_id).collect::<Vec<_>>();
    let facts_generation = state.facts_generation;
    let navigation_generation = state.navigation_generation;
    let derived=vault.read(Db::Intelligence,move |s| {
        let mut map=std::collections::HashMap::new();
        for chunk in ids.chunks(400) {
            let slots=(3..chunk.len()+3).map(|index|format!("(?{index})")).collect::<Vec<_>>().join(",");
            let mut values=vec![vault_store::Value::Integer(facts_generation),vault_store::Value::Integer(navigation_generation)];
            values.extend(chunk.iter().copied().map(vault_store::Value::Integer));
            s.for_each(&format!("WITH requested(visit_id) AS (VALUES {slots}) SELECT f.visit_id,f.canonical_url,f.registrable_domain,f.domain_category,f.page_category,
                f.search_engine,f.search_query,f.is_new_domain,f.is_search_event,f.evidence_tier,f.taxonomy_source,f.taxonomy_pack,f.taxonomy_version,
                a.session_id,a.trail_id FROM requested CROSS JOIN visit_derived_facts_rows f INDEXED BY idx_vdf_visit_generation ON f.visit_id=requested.visit_id LEFT JOIN visit_assignments a INDEXED BY idx_assignments_visit_generation ON a.generation=?2 AND a.visit_id=f.visit_id
                WHERE f.generation=?1"),values,|r| {
                let mut v=VisitRecord {visit_id:r.get(0)?,profile_id:String::new(),source_profile_id:0,source_visit_id:0,source_url_id:0,
                    url:String::new(),title:None,visit_time_ms:0,from_visit:None,transition_type:None,external_referrer_url:None,
                    canonical_url:r.get(1)?,registrable_domain:r.get(2)?,domain_category:r.get(3)?,page_category:r.get(4)?,
                    search_engine:r.get(5)?,search_query:r.get(6)?,is_new_domain:r.get::<i64>(7)?!=0,is_search_event:r.get::<i64>(8)?!=0,
                    evidence_tier:r.get(9)?,taxonomy_source:r.get(10)?,taxonomy_pack:r.get(11)?,taxonomy_version:r.get(12)?,
                    session_id:r.get(13)?,trail_id:r.get(14)?,display_name:None};
                v.display_name=super::display_name_for_domain(&v.registrable_domain);
                map.insert(v.visit_id,v);Ok(())
            })?;
        }
        Ok(map)
    })?;
    for v in &mut visits {
        let d = derived
            .get(&v.visit_id)
            .ok_or_else(|| anyhow::anyhow!("missing derived visit {}", v.visit_id))?;
        v.canonical_url = d.canonical_url.clone();
        v.registrable_domain = d.registrable_domain.clone();
        v.domain_category = d.domain_category.clone();
        v.page_category = d.page_category.clone();
        v.search_engine = d.search_engine.clone();
        v.search_query = d.search_query.clone();
        v.is_new_domain = d.is_new_domain;
        v.is_search_event = d.is_search_event;
        v.evidence_tier = d.evidence_tier.clone();
        v.taxonomy_source = d.taxonomy_source.clone();
        v.taxonomy_pack = d.taxonomy_pack.clone();
        v.taxonomy_version = d.taxonomy_version.clone();
        v.session_id = d.session_id.clone();
        v.trail_id = d.trail_id.clone();
        v.display_name = d.display_name.clone();
    }
    Ok(visits)
}

use super::{
    DailyRollupBundle, PathFlowRecord, QueryFamilyRecord, RefindPageRecord,
    ReopenedInvestigationRecord,
};

pub(super) fn persist_daily(
    s: &Session<'_>,
    generation: i64,
    rollups: &DailyRollupBundle,
) -> Result<()> {
    let mut insert_domain_daily_rollups=s.prepare("INSERT OR REPLACE INTO domain_daily_rollups_rows
 (generation,date_key, profile_id, registrable_domain, domain_category, visit_count, search_count, new_domain_visits, unique_urls, last_visit_ms)
 VALUES (?1,?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)")?;
    let mut insert_category_daily_rollups = s.prepare(
        "INSERT OR REPLACE INTO category_daily_rollups_rows
 (generation,date_key, profile_id, domain_category, visit_count, unique_domains)
 VALUES (?1,?2, ?3, ?4, ?5, ?6)",
    )?;
    let mut insert_engine_daily_rollups = s.prepare(
        "INSERT OR REPLACE INTO engine_daily_rollups_rows
 (generation,date_key, profile_id, search_engine, search_count)
 VALUES (?1,?2, ?3, ?4, ?5)",
    )?;
    let mut insert_daily_summary_rollups=s.prepare("INSERT OR REPLACE INTO daily_summary_rollups_rows
 (generation,date_key, profile_id, total_visits, total_searches, new_domains, unique_domains, hhi_score, discovery_rate, hours)
 VALUES (?1,?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)")?;

    for row in &rollups.domain_rows {
        insert_domain_daily_rollups.execute(params![
            generation,
            row.date_key.as_str(),
            row.profile_id.as_str(),
            row.registrable_domain.as_str(),
            row.domain_category.as_str(),
            row.visit_count,
            row.search_count,
            row.new_domain_visits,
            row.unique_urls,
            row.last_visit_ms
        ])?;
    }
    for (date_key, profile_id, domain_category, visit_count, unique_domains) in
        &rollups.category_rows
    {
        insert_category_daily_rollups.execute(params![
            generation,
            date_key,
            profile_id,
            domain_category,
            visit_count,
            unique_domains
        ])?;
    }
    for (date_key, profile_id, search_engine, search_count) in &rollups.engine_rows {
        insert_engine_daily_rollups.execute(params![
            generation,
            date_key,
            profile_id,
            search_engine,
            search_count
        ])?;
    }
    for row in &rollups.summary_rows {
        let hours = row.hours.iter().map(i64::to_string).collect::<Vec<_>>().join(",");
        insert_daily_summary_rollups.execute(params![
            generation,
            row.date_key.as_str(),
            row.profile_id.as_str(),
            row.total_visits,
            row.total_searches,
            row.new_domains,
            row.unique_domains,
            row.hhi_score,
            row.discovery_rate,
            hours
        ])?;
    }

    Ok(())
}

/// How a search is matched to the families that hold it: case and surrounding spaces ignored.
pub(crate) fn search_family_key(query: &str) -> String {
    query.trim().to_lowercase()
}

/// The keys a family is found under: its anchor and every wording, each once.
pub(super) fn family_query_keys(family: &QueryFamilyRecord) -> std::collections::BTreeSet<String> {
    std::iter::once(&family.anchor_query)
        .chain(&family.queries)
        .map(|query| search_family_key(query))
        .collect()
}

pub(super) fn persist_families(
    s: &Session<'_>,
    generation: i64,
    query_families: &[QueryFamilyRecord],
    computed_at: &str,
) -> Result<()> {
    let mut insert_query_families=s.prepare("INSERT OR REPLACE INTO query_families_rows
 (generation,family_id, profile_id, anchor_query, member_count, search_engine, first_seen_ms, last_seen_ms, queries_json, computed_at)
 VALUES (?1,?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)")?;

    let mut insert_query = s.prepare(
        "INSERT OR IGNORE INTO query_family_queries_rows
 (generation,family_id,profile_id,normalized_query) VALUES (?1,?2,?3,?4)",
    )?;
    for family in query_families {
        for key in family_query_keys(family) {
            insert_query.execute(params![generation, family.family_id, family.profile_id, key])?;
        }
        insert_query_families.execute(params![
            generation,
            family.family_id,
            family.profile_id,
            family.anchor_query,
            family.member_count,
            family.search_engine,
            family.first_seen_ms,
            family.last_seen_ms,
            serde_json::to_string(&family.queries)?,
            computed_at,
        ])?;
    }

    Ok(())
}

pub(super) fn persist_aggregates(
    s: &Session<'_>,
    generation: i64,
    refind_pages: &[RefindPageRecord],
    reopened: &[ReopenedInvestigationRecord],
    path_flows: &[PathFlowRecord],
    computed_at: &str,
) -> Result<()> {
    let mut insert_refind_pages=s.prepare("INSERT OR REPLACE INTO refind_pages_rows
 (generation,profile_id, canonical_url, url, title, registrable_domain, cross_day_count, trail_count, search_arrival_count,
              typed_revisit_count, refind_score, evidence_json, first_seen_ms, last_seen_ms, computed_at)
 VALUES (?1,?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)")?;
    let mut insert_reopened_investigations=s.prepare("INSERT OR REPLACE INTO reopened_investigations_rows
 (generation,investigation_id, profile_id, anchor_type, anchor_id, anchor_label, occurrence_count, distinct_days,
              first_seen_ms, last_seen_ms, evidence_json, computed_at)
 VALUES (?1,?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)")?;
    let mut insert_path_flows = s.prepare(
        "INSERT OR REPLACE INTO path_flows_rows
 (generation,profile_id, flow_pattern, step_count, occurrence_count, last_seen_ms)
 VALUES (?1,?2, ?3, ?4, ?5, ?6)",
    )?;

    for page in refind_pages {
        let evidence: serde_json::Value = serde_json::from_str(&page.evidence_json)?;
        let evidence=serde_json::json!({"visitIds":evidence["visitIds"],"totalVisitCount":evidence["totalVisitCount"]}).to_string();
        insert_refind_pages.execute(params![
            generation,
            page.profile_id,
            page.canonical_url,
            page.url,
            page.title,
            page.registrable_domain,
            page.cross_day_count,
            page.trail_count,
            page.search_arrival_count,
            page.typed_revisit_count,
            page.refind_score,
            evidence,
            page.first_seen_ms,
            page.last_seen_ms,
            computed_at,
        ])?;
    }
    for record in reopened {
        insert_reopened_investigations.execute(params![
            generation,
            record.investigation_id,
            record.profile_id,
            record.anchor_type,
            record.anchor_id,
            record.anchor_label,
            record.occurrence_count,
            record.distinct_days,
            record.first_seen_ms,
            record.last_seen_ms,
            if record.anchor_type == "reference_page" { "" } else { record.evidence_json.as_str() },
            computed_at,
        ])?;
    }
    for flow in path_flows {
        insert_path_flows.execute(params![
            generation,
            flow.profile_id,
            flow.flow_pattern,
            flow.step_count,
            flow.occurrence_count,
            flow.last_seen_ms,
        ])?;
    }

    Ok(())
}
