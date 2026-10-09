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
    let mut pending = PendingDays::default();
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
                let day = current_date.take().unwrap();
                pending.add(day, std::mem::take(&mut accumulator).finish(), day_end);
                if pending.rows >= DAYS_PER_COMMIT_ROWS {
                    pending.flush(vault, state)?;
                }
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
        pending.add(date, accumulator.finish(), day_end);
    }
    pending.flush(vault, state)?;
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

/// Rollup rows a commit carries before it is written. Each commit costs a WAL sync and a
/// checkpoint, so committing every day separately made the daily stage most of a rebuild
/// (about 10,000 commits for 22k visits); whole days still go in together, with the cursor.
const DAYS_PER_COMMIT_ROWS: usize = 4_096;

/// Finished days waiting for one commit, with the cursor just after the last of them.
#[derive(Default)]
struct PendingDays {
    bundle: DailyRollupBundle,
    days: Vec<(String, [usize; 4])>,
    cursor: Option<VisibleVisitBatchCursor>,
    rows: usize,
}

impl PendingDays {
    fn add(&mut self, date: String, day: DailyRollupBundle, end: Option<VisibleVisitBatchCursor>) {
        let sizes = [
            day.domain_rows.len(),
            day.category_rows.len(),
            day.engine_rows.len(),
            day.summary_rows.len(),
        ];
        self.rows += sizes.iter().sum::<usize>();
        self.bundle.domain_rows.extend(day.domain_rows);
        self.bundle.category_rows.extend(day.category_rows);
        self.bundle.engine_rows.extend(day.engine_rows);
        self.bundle.summary_rows.extend(day.summary_rows);
        self.days.push((date, sizes));
        self.cursor = end;
    }

    /// Writes the rows, each day's counts and the advanced cursor in one transaction, so a
    /// resumed build starts after the last day it fully wrote.
    fn flush(&mut self, vault: &Vault, state: &mut BuildState) -> Result<()> {
        if self.days.is_empty() {
            return Ok(());
        }
        let PendingDays { bundle, days, cursor, .. } = std::mem::take(self);
        state.daily_cursor = cursor;
        let saved = state.clone();
        vault.write(Db::Intelligence, Priority::Batch, move |s| {
            build_rows::persist_daily(s, saved.daily_generation, &bundle)?;
            let mut count = s.prepare("INSERT INTO intelligence_day_counts VALUES(?1,?2,?3,?4,?5,?6,?7) ON CONFLICT(generation,profile_id,date_key) DO UPDATE SET domains=excluded.domains,categories=excluded.categories,engines=excluded.engines,summaries=excluded.summaries")?;
            for (date, sizes) in &days {
                count.execute(params![saved.daily_generation,saved.profile_id.clone(),date.clone(),sizes[0] as i64,sizes[1] as i64,sizes[2] as i64,sizes[3] as i64])?;
            }
            build_state::save(s, &saved)
        })
    }
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
    let mut compare = super::compare_sets::CompareSetAccumulator::default();
    let mut cursor = None;
    let mut processed = 0;
    loop {
        let batch = build_rows::derived_page(vault, state, cursor, limit)?;
        if batch.is_empty() {
            break;
        }
        for visit in &batch {
            compare.add(visit);
        }
        compare.resolve(vault, state.navigation_generation)?;
        let last = batch.last().unwrap();
        cursor = Some(VisibleVisitBatchCursor {
            visit_time_ms: last.visit_time_ms,
            visit_id: last.visit_id,
        });
        processed += batch.len();
        progress(build_state::progress(state, "refind-pages", processed, state.processed))?;
    }
    let flows =
        super::path_flows::profile_flows(vault, state.navigation_generation, &state.profile_id)?;
    let compare_sets = compare.finish(vault, state.navigation_generation)?;
    for chunk in compare_sets.chunks(512) {
        let rows = chunk.to_vec();
        let generation = state.aggregate_generation;
        let at = state.computed_at.clone();
        vault.write(Db::Intelligence, Priority::Batch, move |s| {
            super::compare_sets::persist(s, generation, &rows, &at)
        })?;
    }
    let families = load_families(vault, state, progress)?;
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
        ($rows:ident,$batch:ident,$pages:expr,$reopened:expr,$flows:expr) => {
            for chunk in $rows.chunks(512) {
                let $batch = chunk.to_vec();
                let generation = state.aggregate_generation;
                let at = state.computed_at.clone();
                vault.write(Db::Intelligence, Priority::Batch, move |s| {
                    build_rows::persist_aggregates(s, generation, $pages, $reopened, $flows, &at)
                })?;
                progress(build_state::progress(state, "refind-pages", processed, state.processed))?;
            }
        };
    }
    write!(reopened, rows, &[], &rows, &[]);
    write!(flows, rows, &[], &[], &rows);
    for (table, count) in [
        ("query_families", families.len()),
        (
            "query_family_queries",
            families.iter().map(|family| build_rows::family_query_keys(family).len()).sum(),
        ),
        (
            "reopened_investigations",
            state.counts.get("reopened_investigations").copied().unwrap_or(0) + reopened.len(),
        ),
        ("path_flows", flows.len()),
        ("compare_sets", compare_sets.len()),
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
