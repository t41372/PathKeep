//! Responsibilities: page visits, trails and members in their persisted navigation order.
//! Not responsible for: building sessions or changing archive visibility.
//! Failure modes: a long session exhausts memory, insertion order replaces chronology,
//! a cursor loses members, or a reader scans unrelated visits.
use super::{intelligence_sessions::trail_summary_from_row, paging};
use crate::{
    Vault,
    models::{
        IntelligencePage, SessionDetail, SessionDetailRequest, SessionSummary, SessionVisit,
        TrailDetail, TrailDetailRequest, TrailMember, TrailSummary,
    },
};
use anyhow::{Context, Result};
use vault_store::{Db, OptionalRow, Session, params_owned as params};

pub fn get_session_detail(vault: &Vault, request: &SessionDetailRequest) -> Result<SessionDetail> {
    let request = request.clone();
    vault.read(Db::Intelligence,move |s| {
        let (generation,facts,profile,session)=s.query_one("SELECT g.navigation,g.facts,g.profile_id,r.session_id,r.first_visit_ms,r.last_visit_ms,r.visit_count,r.search_count,r.domain_count,r.is_deep_dive,r.auto_title
            FROM intelligence_generations g CROSS JOIN sessions_rows r ON r.generation=g.navigation WHERE r.session_id=?1",[request.session_id.clone()],|r|Ok((r.get::<i64>(0)?,r.get::<i64>(1)?,r.get::<String>(2)?,SessionSummary {session_id:r.get(3)?,first_visit_ms:r.get(4)?,last_visit_ms:r.get(5)?,visit_count:r.get(6)?,search_count:r.get(7)?,domain_count:r.get(8)?,is_deep_dive:r.get::<i64>(9)?!=0,auto_title:r.get(10)?}))).optional()?.with_context(||format!("session {} was not found",request.session_id))?;
        let limit=request.page_size.unwrap_or(100).clamp(1,1000) as usize;
        let after=paging::cursor::<i64>(request.visits_cursor.as_deref())?.unwrap_or(-1);
        let rows=s.query("SELECT v.id,u.url,u.title,f.registrable_domain,v.visit_time_ms,f.is_search_event,f.search_query,f.search_engine,a.trail_id,v.transition_type,a.ordinal
            FROM visit_assignments a INDEXED BY idx_assignments_generation_session
            CROSS JOIN archive.visits v ON v.id=a.visit_id
            CROSS JOIN archive.urls u ON u.id=v.url_id
            CROSS JOIN visit_derived_facts_rows f INDEXED BY idx_vdf_visit_generation ON f.visit_id=a.visit_id AND f.generation=?3
            WHERE a.generation=?1 AND a.session_id=?2 AND a.ordinal>?4 AND v.reverted_at IS NULL
            ORDER BY a.ordinal,a.visit_id LIMIT ?5",params![generation,request.session_id,facts,after,(limit+1) as i64],|r|Ok((SessionVisit {visit_id:r.get(0)?,url:r.get(1)?,title:r.get(2)?,registrable_domain:r.get(3)?,visit_time_ms:r.get(4)?,is_search_event:r.get::<i64>(5)?!=0,search_query:r.get(6)?,search_engine:r.get(7)?,trail_id:r.get(8)?,transition_type:r.get::<Option<i64>>(9)?.map(|v|v.to_string())},r.get::<i64>(10)?)))?;
        let visits=paging::finish(rows,limit)?;
        let trails=session_trails(s,generation,&profile,&session,request.trails_cursor.as_deref(),limit)?;
        Ok(SessionDetail {session,visits:visits.items,visits_next_cursor:visits.next_cursor,trails:trails.items,trails_next_cursor:trails.next_cursor})
    })
}

fn session_trails(
    s: &Session<'_>,
    generation: i64,
    profile: &str,
    session: &SessionSummary,
    cursor: Option<&str>,
    limit: usize,
) -> Result<IntelligencePage<TrailSummary>> {
    let after = paging::cursor::<(i64, String)>(cursor)?;
    let read = |tail: &str, count: usize| -> Result<Vec<(TrailSummary, (i64, String))>> {
        let sql = format!(
            "SELECT trail_id,session_id,initial_query,search_engine,reformulation_count,visit_count,landing_url,landing_domain,first_visit_ms,last_visit_ms,max_depth,queries_json FROM search_trails_rows INDEXED BY idx_search_trails_profile_time_trail WHERE generation=?1 AND profile_id=?2 AND session_id=?3 AND first_visit_ms>=?4 AND first_visit_ms<=?5 {tail} ORDER BY first_visit_ms,trail_id LIMIT ?8"
        );
        Ok(s.query(
            &sql,
            params![
                generation,
                profile,
                session.session_id,
                session.first_visit_ms,
                session.last_visit_ms,
                after.as_ref().map(|v| v.0),
                after.as_ref().map(|v| v.1.as_str()),
                count as i64
            ],
            |r| Ok((trail_summary_from_row(r)?, (r.get::<i64>(8)?, r.get::<String>(0)?))),
        )?)
    };
    let mut rows = if after.is_some() {
        read("AND first_visit_ms=?6 AND trail_id>?7", limit + 1)?
    } else {
        read("", limit + 1)?
    };
    if after.is_some() && rows.len() < limit + 1 {
        rows.extend(read("AND first_visit_ms>?6", limit + 1 - rows.len())?);
    }
    paging::finish(rows, limit)
}

pub fn get_trail_detail(vault: &Vault, request: &TrailDetailRequest) -> Result<TrailDetail> {
    let request = request.clone();
    vault.read(Db::Intelligence,move |s| {
        let (trail,generation,facts)=s.query_one("SELECT r.trail_id,r.session_id,r.initial_query,r.search_engine,r.reformulation_count,r.visit_count,r.landing_url,r.landing_domain,r.first_visit_ms,r.last_visit_ms,r.max_depth,r.queries_json,g.navigation,g.facts
            FROM intelligence_generations g CROSS JOIN search_trails_rows r ON r.generation=g.navigation WHERE r.trail_id=?1",[request.trail_id.clone()],|r|Ok((trail_summary_from_row(r)?,r.get::<i64>(12)?,r.get::<i64>(13)?))).optional()?.with_context(||format!("trail {} was not found",request.trail_id))?;
        let limit=request.page_size.unwrap_or(100).clamp(1,1000) as usize;
        let after=paging::cursor::<i64>(request.cursor.as_deref())?.unwrap_or(-1);
        let rows=s.query("SELECT m.trail_id,m.visit_id,m.ordinal,m.role,u.url,f.canonical_url,u.title,f.registrable_domain,v.visit_time_ms,f.search_query
            FROM search_trail_members_rows m INDEXED BY idx_search_trail_members_order
            CROSS JOIN archive.visits v ON v.id=m.visit_id
            CROSS JOIN archive.urls u ON u.id=v.url_id
            LEFT JOIN visit_derived_facts_rows f INDEXED BY idx_vdf_visit_generation ON f.visit_id=m.visit_id AND f.generation=?3
            WHERE m.generation=?1 AND m.trail_id=?2 AND m.ordinal>?4 AND v.reverted_at IS NULL
            ORDER BY m.ordinal,m.visit_id LIMIT ?5",params![generation,request.trail_id,facts,after,(limit+1) as i64],|r|Ok((TrailMember {trail_id:r.get(0)?,visit_id:r.get(1)?,ordinal:r.get(2)?,role:r.get(3)?,url:r.get(4)?,canonical_url:r.get(5)?,title:r.get(6)?,registrable_domain:r.get(7)?,visit_time_ms:r.get(8)?,search_query:r.get(9)?},r.get::<i64>(2)?)))?;
        let page=paging::finish(rows,limit)?;
        Ok(TrailDetail {trail,members:page.items,members_next_cursor:page.next_cursor})
    })
}
