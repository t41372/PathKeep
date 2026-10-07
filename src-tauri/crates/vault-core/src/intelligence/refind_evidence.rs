//! Responsibilities: read complete refind evidence and related trails with cursors.
//! Not responsible for: computing scores or choosing the representative profile.
//! Failure modes: the persisted evidence sample hides visits, unrelated canonical pages leak,
//! a shared trail repeats, or a page boundary skips equal timestamps.
use super::{intelligence_sessions::trail_summary_from_row, paging};
use crate::models::{IntelligencePage, TrailSummary};
use anyhow::Result;
use vault_store::{Session, params_owned as params};

const VISITS: &str = "FROM intelligence_generations g
    CROSS JOIN visit_derived_facts_rows f INDEXED BY idx_vdf_generation_canonical_visit
      ON f.generation=g.facts AND f.profile_id=g.profile_id
    CROSS JOIN archive.visits v ON v.id=f.visit_id
    WHERE g.profile_id=?1 AND f.is_search_event=0 AND f.canonical_url=?2 AND v.reverted_at IS NULL";

pub(super) fn visits(
    s: &Session<'_>,
    profile: &str,
    url: &str,
    cursor: Option<&str>,
    limit: usize,
) -> Result<IntelligencePage<i64>> {
    let after = paging::cursor::<i64>(cursor)?;
    let rows=s.query(&format!("SELECT f.visit_id {VISITS} AND (?3 IS NULL OR f.visit_id>?3) ORDER BY f.visit_id LIMIT ?4"),params![profile,url,after,(limit+1) as i64],|r| {let id=r.get::<i64>(0)?;Ok((id,id))})?;
    paging::finish(rows, limit)
}
pub(super) fn days(
    s: &Session<'_>,
    profile: &str,
    url: &str,
    start: i64,
    end: i64,
) -> Result<Vec<String>> {
    Ok(s.query(&format!("SELECT DISTINCT strftime('%Y-%m-%d',datetime(v.visit_time_ms/1000,'unixepoch','localtime')) {VISITS} AND v.visit_time_ms>=?3 AND v.visit_time_ms<?4 ORDER BY 1 DESC"),params![profile,url,start,end],|r|Ok(r.get::<String>(0)?))?)
}
pub(super) fn trails(
    s: &Session<'_>,
    profile: &str,
    url: &str,
    start: i64,
    end: i64,
    cursor: Option<&str>,
    limit: usize,
) -> Result<IntelligencePage<TrailSummary>> {
    let after = paging::cursor::<(i64, String)>(cursor)?;
    let rows=s.query("WITH related AS (
        SELECT DISTINCT a.trail_id FROM intelligence_generations g
        CROSS JOIN visit_derived_facts_rows f INDEXED BY idx_vdf_generation_canonical_visit
          ON f.generation=g.facts AND f.profile_id=g.profile_id
        CROSS JOIN visit_assignments a INDEXED BY idx_assignments_visit_generation
          ON a.visit_id=f.visit_id AND a.generation=g.navigation
        CROSS JOIN archive.visits v ON v.id=f.visit_id
        WHERE g.profile_id=?1 AND f.is_search_event=0 AND f.canonical_url=?2
          AND a.trail_id IS NOT NULL AND v.reverted_at IS NULL AND v.visit_time_ms>=?3 AND v.visit_time_ms<?4
    ) SELECT t.trail_id,t.session_id,t.initial_query,t.search_engine,t.reformulation_count,t.visit_count,
        t.landing_url,t.landing_domain,t.first_visit_ms,t.last_visit_ms,t.max_depth,t.queries_json
    FROM related CROSS JOIN search_trails_rows t ON t.trail_id=related.trail_id
    WHERE t.generation=(SELECT navigation FROM intelligence_generations WHERE profile_id=?1)
      AND (?5 IS NULL OR t.last_visit_ms<?5 OR (t.last_visit_ms=?5 AND t.trail_id<?6))
    ORDER BY t.last_visit_ms DESC,t.trail_id DESC LIMIT ?7",
    params![profile,url,start,end,after.as_ref().map(|v|v.0),after.as_ref().map(|v|v.1.as_str()),(limit+1) as i64],|r|Ok((trail_summary_from_row(r)?,(r.get::<i64>(9)?,r.get::<String>(0)?))))?;
    paging::finish(rows, limit)
}
