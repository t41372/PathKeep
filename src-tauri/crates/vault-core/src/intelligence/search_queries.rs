//! Responsibilities: stream exact-query groups and retain one ranked cursor page.
//! Not responsible for: archive search, query-family construction or provider calls.
//! Failure modes: a family sort buffers every result, offsets repeat or skip queries,
//! range counts include hidden visits, or related context refers to another family.
use super::intelligence_search_queries::{load_query_family_summary_map, load_trail_context_map};
use super::{
    date_range_bounds, display_name_for_search_engine_with_map, load_search_engine_display_names,
    normalize_query, rfc3339_from_millis,
};
use crate::{
    Vault,
    models::{SearchQueryListRequest, SearchQueryListResult, SearchQueryRow},
};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::{cmp::Reverse, collections::BTreeMap};
use vault_store::{Db, params_owned as params};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(tag = "sort", content = "after", rename_all = "kebab-case")]
enum Cursor {
    Newest(Reverse<i64>, String, String),
    Exact(Reverse<i64>, Reverse<i64>, String, String),
    Family(Reverse<i64>, Reverse<i64>, Reverse<i64>, String, String),
    Alphabetical(String, Reverse<i64>, String),
}
impl Cursor {
    fn for_row(row: &SearchQueryRow, sort: Option<&str>) -> Self {
        match sort.unwrap_or("newest") {
            "exact-frequency" | "exact_frequency" => Self::Exact(
                Reverse(row.exact_repeat_count),
                Reverse(row.searched_at_ms),
                row.search_engine.clone(),
                row.normalized_query.clone(),
            ),
            "family-frequency" | "family_frequency" => Self::Family(
                Reverse(row.family_count),
                Reverse(row.exact_repeat_count),
                Reverse(row.searched_at_ms),
                row.search_engine.clone(),
                row.normalized_query.clone(),
            ),
            "alphabetical" => Self::Alphabetical(
                row.normalized_query.clone(),
                Reverse(row.searched_at_ms),
                row.search_engine.clone(),
            ),
            _ => Self::Newest(
                Reverse(row.searched_at_ms),
                row.search_engine.clone(),
                row.normalized_query.clone(),
            ),
        }
    }
}

/// Counts the complete filtered scope while retaining only a page and one lookahead row.
pub fn get_search_queries(
    vault: &Vault,
    request: &SearchQueryListRequest,
) -> Result<SearchQueryListResult> {
    let request = request.clone();
    vault.read(Db::Intelligence,move |s| {
        let (start,end)=date_range_bounds(s, &request.date_range)?;
        let limit=request.page_size.clamp(1,1000) as usize;
        let after=super::paging::cursor::<Cursor>(request.cursor.as_deref())?;
        let filter=request.query.as_deref().map(normalize_query).filter(|q|!q.is_empty());
        let names=load_search_engine_display_names(s)?;
        let families=load_query_family_summary_map(s,request.profile_id.as_deref(),request.engine.as_deref(),start,end)?;
        let mut statement=s.prepare("WITH ranked AS (
            SELECT e.visit_id,e.profile_id,p.browser_kind,e.search_engine,e.raw_query,e.normalized_query,e.trail_id,e.visit_time_ms,
                COUNT(*) OVER (PARTITION BY e.search_engine,e.normalized_query) AS exact_repeat_count,
                ROW_NUMBER() OVER (PARTITION BY e.search_engine,e.normalized_query ORDER BY e.visit_time_ms DESC,e.visit_id DESC) AS row_rank
            FROM search_events_rows e INDEXED BY idx_search_events_kind_time
            CROSS JOIN intelligence_generations g ON g.profile_id=e.profile_id AND g.navigation=e.generation
            CROSS JOIN archive.visits v ON v.id=e.visit_id
            CROSS JOIN archive.source_profiles p ON p.profile_key=e.profile_id
            CROSS JOIN visit_derived_facts_rows f INDEXED BY idx_vdf_visit_generation ON f.visit_id=e.visit_id AND f.generation=g.facts
            WHERE e.query_kind='keyword' AND e.visit_time_ms>=?6 AND e.visit_time_ms<?7 AND v.reverted_at IS NULL
              AND (?1 IS NULL OR e.profile_id=?1) AND (?2 IS NULL OR p.browser_kind=?2)
              AND (?3 IS NULL OR e.search_engine=?3) AND (?4 IS NULL OR f.registrable_domain=?4)
              AND (?5 IS NULL OR e.normalized_query LIKE '%'||?5||'%' OR LOWER(e.raw_query) LIKE '%'||?5||'%')
        ) SELECT visit_id,profile_id,browser_kind,search_engine,raw_query,normalized_query,trail_id,visit_time_ms,exact_repeat_count
        FROM ranked WHERE row_rank=1")?;
        let rows=statement.query_map(params![request.profile_id,request.browser_kind,request.engine,request.domain,filter,start,end],|r| {
            let engine=r.get::<String>(3)?;let normalized=r.get::<String>(5)?;let count=r.get::<i64>(8)?;let time=r.get::<i64>(7)?;
            let family=families.get(&(engine.clone(),normalized.clone()));
            Ok(SearchQueryRow {visit_id:r.get(0)?,profile_id:r.get(1)?,browser_kind:r.get(2)?,display_name:display_name_for_search_engine_with_map(&engine,&names),search_engine:engine,raw_query:r.get(4)?,normalized_query:normalized,trail_id:r.get(6)?,searched_at:rfc3339_from_millis(time),searched_at_ms:time,exact_repeat_count:count,family_count:family.map(|v|v.1).unwrap_or(count.max(1)),family_id:family.map(|v|v.0.clone()),trail_initial_query:None,trail_reformulation_count:None})
        })?;
        let mut total=0;let mut selected=BTreeMap::new();
        for row in rows {
            let row=row?;total+=1;let key=Cursor::for_row(&row,request.sort.as_deref());
            if after.as_ref().is_some_and(|after|&key<=after) {continue;}
            selected.insert(key,row);if selected.len()>limit+1 {selected.pop_last();}
        }
        let next_cursor=if selected.len()>limit {selected.pop_last();Some(serde_json::to_string(selected.last_key_value().unwrap().0)?)} else {None};
        let ids=selected.values().filter_map(|row|row.trail_id.as_ref().map(|trail|(row.profile_id.clone(),trail.clone()))).collect::<Vec<_>>();
        let trails=load_trail_context_map(s,&ids)?;
        let rows=selected.into_values().map(|mut row| {
            if let Some(context)=row.trail_id.as_ref().and_then(|id|trails.get(id)) {row.trail_initial_query=Some(context.0.clone());row.trail_reformulation_count=Some(context.1);}
            row
        }).collect();
        Ok(SearchQueryListResult {rows,total,next_cursor,page_size:limit as u32})
    })
}
