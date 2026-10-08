//! Responsibilities: bounded daily, navigation and aggregate rebuild passes.
//! Not responsible for: opening storage, schema installation or job claims.
//! Failure modes: batch boundaries split a session, restart duplicates members,
//! daily rows lose cross-batch uniqueness, or publication exposes partial generations.

use super::build_rows;
use super::build_state::{self, BuildState};
use super::intelligence_daily_rollup_state::DailyRollupAccumulator;
use super::intelligence_structural_stream::{StructuralTailPersist, StructuralTailStreamState};
use super::*;
use crate::Vault;
use anyhow::Result;
use std::collections::BTreeMap;
use vault_store::{Db, Priority, params_owned as params};

pub(super) fn daily<F>(
    vault: &Vault,
    state: &mut BuildState,
    limit: usize,
    progress: &mut F,
) -> Result<usize>
where
    F: FnMut(CoreIntelligenceProgress) -> Result<()>,
{
    if state.daily_done {
        return Ok(0);
    }
    let mut cursor = state.daily_cursor;
    if cursor.is_none()
        && !state.full
        && let Some(date) = state.dirty_dates.first()
    {
        let range = crate::models::DateRange { start: date.clone(), end: date.clone() };
        let (start, _) = date_range_bounds(&range)?;
        cursor = Some(VisibleVisitBatchCursor { visit_time_ms: start - 1, visit_id: i64::MAX });
    }
    let mut current_date = None::<String>;
    let mut accumulator = DailyRollupAccumulator::default();
    let mut day_end = cursor;
    let mut processed = 0;
    loop {
        let batch = build_rows::derived_page(vault, state, cursor, limit)?;
        if batch.is_empty() {
            break;
        }
        for visit in &batch {
            let date = local_date_key(visit.visit_time_ms);
            if current_date.as_deref().is_some_and(|old| old != date) {
                let bundle = std::mem::take(&mut accumulator).finish();
                persist_day(
                    vault,
                    state,
                    current_date.as_ref().unwrap(),
                    day_end,
                    &bundle,
                    progress,
                )?;
            }
            current_date = Some(date);
            accumulator.add_visit(visit);
            day_end = Some(VisibleVisitBatchCursor {
                visit_time_ms: visit.visit_time_ms,
                visit_id: visit.visit_id,
            });
        }
        cursor = day_end;
        processed += batch.len();
        progress(build_state::progress(
            state,
            "daily-rollups",
            processed,
            state.source.visible_visit_count as usize,
        ))?;
    }
    if let Some(date) = current_date {
        persist_day(vault, state, &date, day_end, &accumulator.finish(), progress)?;
    }
    let generation = state.daily_generation;
    let profile = state.profile_id.clone();
    let counts=vault.read(Db::Intelligence,move |s|Ok(s.query_one("SELECT COALESCE(SUM(domains),0),COALESCE(SUM(categories),0),COALESCE(SUM(engines),0),COALESCE(SUM(summaries),0) FROM intelligence_day_counts WHERE generation=?1 AND profile_id=?2",params![generation,profile],|r|Ok([r.get::<i64>(0)?,r.get(1)?,r.get(2)?,r.get(3)?]))?))?;
    for (table, count) in [
        "domain_daily_rollups",
        "category_daily_rollups",
        "engine_daily_rollups",
        "daily_summary_rollups",
    ]
    .into_iter()
    .zip(counts)
    {
        state.counts.insert(table.into(), count as usize);
    }
    state.daily_done = true;
    super::build_pipeline::checkpoint(vault, state, RebuildMode::DailyRollup)?;
    Ok(processed)
}

fn persist_day<F>(
    vault: &Vault,
    state: &mut BuildState,
    date: &str,
    cursor: Option<VisibleVisitBatchCursor>,
    bundle: &DailyRollupBundle,
    progress: &mut F,
) -> Result<()>
where
    F: FnMut(CoreIntelligenceProgress) -> Result<()>,
{
    macro_rules! part {
        ($field:ident) => {
            for chunk in bundle.$field.chunks(512) {
                let piece = DailyRollupBundle { $field: chunk.to_vec(), ..Default::default() };
                let generation = state.daily_generation;
                vault.write(Db::Intelligence, Priority::Batch, move |s| {
                    build_rows::persist_daily(s, generation, &piece)
                })?;
                progress(build_state::progress(
                    state,
                    "daily-rollups",
                    state.processed,
                    state.source.visible_visit_count as usize,
                ))?;
            }
        };
    }
    part!(domain_rows);
    part!(category_rows);
    part!(engine_rows);
    part!(summary_rows);
    state.daily_cursor = cursor;
    let saved = state.clone();
    let date = date.to_owned();
    let sizes = [
        bundle.domain_rows.len(),
        bundle.category_rows.len(),
        bundle.engine_rows.len(),
        bundle.summary_rows.len(),
    ];
    vault.write(Db::Intelligence,Priority::Batch,move |s| {
        s.execute("INSERT INTO intelligence_day_counts VALUES(?1,?2,?3,?4,?5,?6,?7) ON CONFLICT(generation,profile_id,date_key) DO UPDATE SET domains=excluded.domains,categories=excluded.categories,engines=excluded.engines,summaries=excluded.summaries",params![saved.daily_generation,saved.profile_id,date,sizes[0] as i64,sizes[1] as i64,sizes[2] as i64,sizes[3] as i64])?;
        build_state::save(s,&saved)
    })
}

pub(super) fn navigation<F>(
    vault: &Vault,
    state: &mut BuildState,
    limit: usize,
    progress: &mut F,
) -> Result<()>
where
    F: FnMut(CoreIntelligenceProgress) -> Result<()>,
{
    if state.navigation_done {
        return Ok(());
    }
    let mut stream: StructuralTailStreamState =
        state.stream_json.as_deref().map(serde_json::from_str).transpose()?.unwrap_or_default();
    if let Some(session) = stream.current_session.as_mut() {
        let mut after = String::new();
        loop {
            let generation = state.navigation_generation;
            let id = session.record.session_id.clone();
            let cursor = after.clone();
            let page=vault.read(Db::Intelligence,move |s|Ok(s.query("SELECT domain,visit_count FROM session_domains WHERE generation=?1 AND session_id=?2 AND domain>?3 ORDER BY domain LIMIT 1024",params![generation,id,cursor],|r|Ok((r.get::<String>(0)?,r.get::<i64>(1)?)))?))?;
            if page.is_empty() {
                break;
            }
            after = page.last().unwrap().0.clone();
            for (domain, count) in page {
                session.domain_counts.insert(domain, count as usize);
            }
            progress(build_state::progress(
                state,
                "sessions",
                stream.report.processed_visits,
                state.source.visible_visit_count as usize,
            ))?;
        }
    }
    loop {
        let batch = build_rows::derived_page(vault, state, state.structural_cursor, limit)?;
        if batch.is_empty() {
            break;
        }
        let last = batch.last().unwrap();
        state.structural_cursor = Some(VisibleVisitBatchCursor {
            visit_time_ms: last.visit_time_ms,
            visit_id: last.visit_id,
        });
        let mut saved = state.clone();
        stream = vault.write(Db::Intelligence, Priority::Batch, move |s| {
            let mut persist =
                StructuralTailPersist::new(s, saved.navigation_generation, &saved.profile_id)?;
            for visit in batch {
                stream.process_visit(
                    StructuralVisitRecord {
                        visit_id: visit.visit_id,
                        profile_id: visit.profile_id,
                        url: visit.url,
                        visit_time_ms: visit.visit_time_ms,
                        from_visit: visit.from_visit,
                        registrable_domain: visit.registrable_domain,
                        search_engine: visit.search_engine,
                        search_query: visit.search_query,
                        is_new_domain: visit.is_new_domain,
                        is_search_event: visit.is_search_event,
                    },
                    &saved.computed_at,
                    &mut persist,
                )?;
            }
            persist.flush_domain_counts()?;
            drop(persist);
            saved.stream_json = Some(serde_json::to_string(&stream)?);
            build_state::save(s, &saved)?;
            Ok(stream)
        })?;
        state.stream_json = Some(serde_json::to_string(&stream)?);
        progress(build_state::progress(
            state,
            "sessions",
            stream.report.processed_visits,
            state.source.visible_visit_count as usize,
        ))?;
    }
    // Flush a copy so the next append can continue the last open session/trail.
    let mut finished = stream.clone();
    let saved = state.clone();
    let report = vault.write(Db::Intelligence, Priority::Batch, move |s| {
        let mut persist =
            StructuralTailPersist::new(s, saved.navigation_generation, &saved.profile_id)?;
        finished.finish(&saved.computed_at, &mut persist)?;
        Ok(finished.report)
    })?;
    for (table, count) in [
        ("sessions", report.sessions),
        ("search_trails", report.trails),
        ("search_events", report.trails),
        ("search_trail_members", report.member_count),
        ("search_event_terms", report.term_count),
    ] {
        state.counts.insert(table.into(), count);
    }
    state.navigation_done = true;
    state.stream_json = Some(serde_json::to_string(&stream)?);
    super::build_pipeline::checkpoint(vault, state, RebuildMode::StructuralRebuild)
}

pub(super) fn aggregates<F>(
    vault: &Vault,
    state: &mut BuildState,
    limit: usize,
    progress: &mut F,
) -> Result<()>
where
    F: FnMut(CoreIntelligenceProgress) -> Result<()>,
{
    if state.aggregates_done {
        return Ok(());
    }
    super::build_refind::rebuild(vault, state, limit, progress)?;
    let mut accumulator =
        super::intelligence_structural_aggregates::StructuralAggregateAccumulator::default();
    let mut cursor = None;
    let mut processed = 0;
    loop {
        let batch = build_rows::derived_page(vault, state, cursor, limit)?;
        if batch.is_empty() {
            break;
        }
        for visit in &batch {
            accumulator.add_summary_visit(visit);
        }
        let last = batch.last().unwrap();
        cursor = Some(VisibleVisitBatchCursor {
            visit_time_ms: last.visit_time_ms,
            visit_id: last.visit_id,
        });
        processed += batch.len();
        progress(build_state::progress(state, "refind-pages", processed, state.processed))?;
    }
    let (_, flows, habits) = accumulator.finish();
    let families = load_families(vault, state, progress)?;
    let sources = load_sources(vault, state)?;
    let reopened =
        super::intelligence_structural_aggregates::build_reopened_investigations(&families, &[]);
    for chunk in families.chunks(512) {
        let rows = chunk.to_vec();
        let generation = state.aggregate_generation;
        let at = state.computed_at.clone();
        vault.write(Db::Intelligence, Priority::Batch, move |s| {
            build_rows::persist_families(s, generation, &rows, &at)
        })?;
        progress(build_state::progress(state, "search-trails", processed, state.processed))?;
    }
    macro_rules! write {
        ($rows:ident,$batch:ident,$pages:expr,$sources:expr,$habits:expr,$reopened:expr,$flows:expr) => {
            for chunk in $rows.chunks(512) {
                let $batch = chunk.to_vec();
                let generation = state.aggregate_generation;
                let at = state.computed_at.clone();
                vault.write(Db::Intelligence, Priority::Batch, move |s| {
                    build_rows::persist_aggregates(
                        s, generation, $pages, $sources, $habits, $reopened, $flows, &at,
                    )
                })?;
                progress(build_state::progress(state, "refind-pages", processed, state.processed))?;
            }
        };
    }
    write!(sources, rows, &[], &rows, &[], &[], &[]);
    write!(habits, rows, &[], &[], &rows, &[], &[]);
    write!(reopened, rows, &[], &[], &[], &rows, &[]);
    write!(flows, rows, &[], &[], &[], &[], &rows);
    for (table, count) in [
        ("query_families", families.len()),
        (
            "query_family_queries",
            families.iter().map(|family| build_rows::family_query_keys(family).len()).sum(),
        ),
        ("source_effectiveness", sources.len()),
        ("habit_patterns", habits.len()),
        (
            "reopened_investigations",
            state.counts.get("reopened_investigations").copied().unwrap_or(0) + reopened.len(),
        ),
        ("path_flows", flows.len()),
    ] {
        state.counts.insert(table.into(), count);
    }
    state.aggregates_done = true;
    super::build_pipeline::checkpoint(vault, state, RebuildMode::StructuralRebuild)
}

fn load_families<F>(
    vault: &Vault,
    state: &BuildState,
    progress: &mut F,
) -> Result<Vec<QueryFamilyRecord>>
where
    F: FnMut(CoreIntelligenceProgress) -> Result<()>,
{
    let mut accumulator = super::intelligence_structural_build::QueryFamilyAccumulator::default();
    let mut after = 0;
    loop {
        let generation = state.navigation_generation;
        let profile = state.profile_id.clone();
        let rows=vault.read(Db::Intelligence,move |s|Ok(s.query("SELECT visit_id,profile_id,search_engine,raw_query,normalized_query,query_kind,trail_id,visit_time_ms FROM search_events_rows WHERE generation=?1 AND profile_id=?2 AND visit_id>?3 AND query_kind='keyword' ORDER BY visit_id LIMIT 4096",params![generation,profile,after],|r|Ok(SearchEventRecord {visit_id:r.get(0)?,profile_id:r.get(1)?,search_engine:r.get(2)?,raw_query:r.get(3)?,normalized_query:r.get(4)?,query_kind:parse_search_query_kind(&r.get::<String>(5)?),trail_id:r.get(6)?,visit_time_ms:r.get(7)?}))?))?;
        if rows.is_empty() {
            break;
        }
        after = rows.last().unwrap().visit_id;
        for event in &rows {
            accumulator.add_event(event);
        }
        progress(build_state::progress(
            state,
            "search-trails",
            after as usize,
            state.source.max_visit_id as usize,
        ))?;
    }
    Ok(accumulator.finish())
}

fn load_sources(vault: &Vault, state: &BuildState) -> Result<Vec<SourceEffectivenessRecord>> {
    let mut counts = BTreeMap::<String, (i64, i64, i64, i64)>::new();
    let mut after = String::new();
    loop {
        let generation = state.navigation_generation;
        let profile = state.profile_id.clone();
        let cursor = after.clone();
        let rows=vault.read(Db::Intelligence,move |s|Ok(s.query("SELECT trail_id,landing_domain,first_visit_ms,last_visit_ms FROM search_trails_rows WHERE generation=?1 AND profile_id=?2 AND trail_id>?3 ORDER BY trail_id LIMIT 4096",params![generation,profile,cursor],|r|Ok((r.get::<String>(0)?,r.get::<Option<String>>(1)?,r.get::<i64>(2)?,r.get::<i64>(3)?)))?))?;
        if rows.is_empty() {
            break;
        }
        after = rows.last().unwrap().0.clone();
        for (_, domain, first, last) in rows {
            if let Some(domain) = domain {
                let e = counts.entry(domain).or_insert((0, 0, first, last));
                e.0 += 1;
                e.2 = e.2.min(first);
                e.3 = e.3.max(last);
            }
        }
    }
    let mut after = String::new();
    loop {
        let generation = state.aggregate_generation;
        let profile = state.profile_id.clone();
        let cursor = after.clone();
        let rows=vault.read(Db::Intelligence,move |s|Ok(s.query("SELECT canonical_url,registrable_domain,first_seen_ms,last_seen_ms FROM refind_pages_rows WHERE generation=?1 AND profile_id=?2 AND canonical_url>?3 ORDER BY canonical_url LIMIT 1024",params![generation,profile,cursor],|r|Ok((r.get::<String>(0)?,r.get::<String>(1)?,r.get::<i64>(2)?,r.get::<i64>(3)?)))?))?;
        if rows.is_empty() {
            break;
        }
        after = rows.last().unwrap().0.clone();
        for (_, domain, first, last) in rows {
            let e = counts.entry(domain).or_insert((0, 0, first, last));
            e.1 += 1;
            e.2 = e.2.min(first);
            e.3 = e.3.max(last);
        }
    }
    Ok(counts.into_iter().map(|(domain,(landings,references,first,last))|SourceEffectivenessRecord {
        profile_id:state.profile_id.clone(),registrable_domain:domain,source_role:if references>=landings && references>0 {"reference"} else {"landing"}.into(),
        trail_count:landings,stable_landing_count:landings,effectiveness_score:landings as f32*2.5+references as f32*1.5,
        evidence_json:serde_json::json!({"stableLandingCount":landings,"referenceCount":references,"trailCount":landings}).to_string(),first_seen_ms:first,last_seen_ms:last}).collect())
}
