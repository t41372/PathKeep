//! Responsibilities: page ranked navigation flows and stream date-scoped visit sequences.
//! Not responsible for: rebuilding persisted flows or source-evidence hydration.
//! Failure modes: a result cap hides flows, repeated domains inflate windows, a session
//! boundary joins unrelated paths, or all-time reads replay the entire archive.
use super::{date_range_bounds, rfc3339_from_millis};
use crate::{
    Vault,
    models::{IntelligencePage, PathFlow, PathFlowRequest, PathFlowStep},
};
use anyhow::Result;
use std::collections::HashMap;
use vault_store::{Db, Session, params_owned as params};

pub fn get_path_flows(
    vault: &Vault,
    request: &PathFlowRequest,
) -> Result<IntelligencePage<PathFlow>> {
    let request = request.clone();
    vault.read(Db::Intelligence, move |s| get_path_flows_with_connection(s, &request))
}
pub(crate) fn get_path_flows_with_connection(
    s: &Session<'_>,
    request: &PathFlowRequest,
) -> Result<IntelligencePage<PathFlow>> {
    let (start, end) = date_range_bounds(&request.date_range)?;
    let steps = request.step_count.clamp(2, 4) as usize;
    let limit = request.limit.unwrap_or(20).clamp(1, 1000) as usize;
    let after = super::paging::cursor::<(i64, i64, String)>(request.cursor.as_deref())?;
    if request.date_range.start.as_str()
        <= super::intelligence_overview_snapshot::ALL_TIME_SCOPE_START
        && request.date_range.end >= chrono::Local::now().format("%Y-%m-%d").to_string()
    {
        let rows = if let Some(profile) = request.profile_id.as_deref() {
            // One profile has one row per flow, so its list is the rows table in index order and
            // later pages seek to the cursor (bounded form, only present after the first page).
            let after_clause = if after.is_some() {
                "AND r.occurrence_count<=?3 AND (r.occurrence_count<?3 OR (r.last_seen_ms<=?4 AND (r.last_seen_ms<?4 OR r.flow_pattern>?5)))"
            } else {
                ""
            };
            s.query(&format!("SELECT r.flow_pattern,r.occurrence_count,r.last_seen_ms FROM path_flows_rows r INDEXED BY idx_path_flows_profile_rank
                WHERE r.profile_id=?1 AND r.step_count=?2 AND r.generation IN (SELECT aggregates FROM intelligence_generations) {after_clause}
                ORDER BY r.occurrence_count DESC,r.last_seen_ms DESC,r.flow_pattern LIMIT ?6"),
                params![profile,steps as i64,after.as_ref().map(|v|v.0),after.as_ref().map(|v|v.1),after.as_ref().map(|v|v.2.as_str()),(limit+1) as i64],|r|Ok((r.get::<String>(0)?,r.get::<i64>(1)?,r.get::<i64>(2)?)))?
        } else {
            // All profiles merge each flow's rows, so every page groups the materialized flows
            // of the requested length again; see the keyset review in turso-review-ledger.md.
            s.query("SELECT flow_pattern,SUM(occurrence_count),MAX(last_seen_ms) FROM path_flows
            WHERE (?1 IS NULL OR profile_id=?1) AND step_count=?2 GROUP BY flow_pattern
            HAVING ?4 IS NULL OR SUM(occurrence_count)<?4 OR (SUM(occurrence_count)=?4 AND (MAX(last_seen_ms)<?5 OR (MAX(last_seen_ms)=?5 AND flow_pattern>?6)))
            ORDER BY SUM(occurrence_count) DESC,MAX(last_seen_ms) DESC,flow_pattern LIMIT ?3",
            params![request.profile_id,steps as i64,(limit+1) as i64,after.as_ref().map(|v|v.0),after.as_ref().map(|v|v.1),after.as_ref().map(|v|v.2.as_str())],|r|Ok((r.get::<String>(0)?,r.get::<i64>(1)?,r.get::<i64>(2)?)))?
        };
        let rows = rows
            .into_iter()
            .map(|(pattern, count, time)| {
                let cursor = (count, time, pattern.clone());
                (flow(request.profile_id.as_deref(), steps, pattern, count, time), cursor)
            })
            .collect();
        return super::paging::finish(rows, limit);
    }
    let mut statement = s.prepare(
        // The range's visits by time, each joined by visit id to its facts and session: through
        // the facts view Turso read every fact of the generation (72 s for 30 days at 14.4M).
        "SELECT a.session_id,f.registrable_domain,v.visit_time_ms
        FROM archive.visits v INDEXED BY idx_visits_visible_time_id
        CROSS JOIN visit_derived_facts_rows f INDEXED BY idx_vdf_visit_generation
          ON f.visit_id=v.id AND f.generation IN (SELECT facts FROM intelligence_generations)
        CROSS JOIN visit_assignments a INDEXED BY idx_assignments_visit_generation
          ON a.visit_id=v.id AND a.generation IN (SELECT navigation FROM intelligence_generations)
        WHERE v.reverted_at IS NULL AND v.visit_time_ms>=?2 AND v.visit_time_ms<?3
          AND (?1 IS NULL OR f.profile_id=?1)
        ORDER BY a.session_id,v.visit_time_ms,v.id",
    )?;
    let rows = statement.query_map(params![request.profile_id, start, end], |r| {
        Ok((r.get::<String>(0)?, r.get::<String>(1)?, r.get::<i64>(2)?))
    })?;
    let mut flows = HashMap::new();
    let mut session = String::new();
    let mut sequence = Vec::new();
    for row in rows {
        let (next, domain, time) = row?;
        if session != next {
            finalize_path_sequence(&sequence, steps, &mut flows);
            sequence.clear();
            session = next;
        }
        if sequence.last().is_some_and(|(last, _)| last == &domain) {
            sequence.last_mut().unwrap().1 = time;
        } else {
            finalize_path_sequence(&sequence, steps, &mut flows);
            sequence.push((domain, time));
            if sequence.len() > steps {
                sequence.remove(0);
            }
        }
    }
    finalize_path_sequence(&sequence, steps, &mut flows);
    let mut ranked = flows
        .into_iter()
        .filter_map(|((pattern, _), aggregate)| {
            let key = (aggregate.occurrence_count, aggregate.last_seen_ms, pattern.clone());
            let follows = after.as_ref().is_none_or(|previous| {
                key.0 < previous.0
                    || (key.0 == previous.0
                        && (key.1 < previous.1 || (key.1 == previous.1 && key.2 > previous.2)))
            });
            follows
                .then(|| (flow(request.profile_id.as_deref(), steps, pattern, key.0, key.1), key))
        })
        .collect::<Vec<_>>();
    ranked.sort_by(|a, b| {
        b.1.0.cmp(&a.1.0).then_with(|| b.1.1.cmp(&a.1.1)).then_with(|| a.1.2.cmp(&b.1.2))
    });
    ranked.truncate(limit + 1);
    super::paging::finish(ranked, limit)
}
fn flow(profile: Option<&str>, steps: usize, pattern: String, count: i64, time: i64) -> PathFlow {
    PathFlow {
        flow_id: format!("flow:{}:{steps}:{pattern}", profile.unwrap_or("all-profiles")),
        steps: build_path_flow_steps(&pattern),
        flow_pattern: pattern,
        step_count: steps as i64,
        occurrence_count: count,
        last_seen_at: rfc3339_from_millis(time),
    }
}
#[derive(Default)]
pub(super) struct FlowAggregate {
    occurrence_count: i64,
    last_seen_ms: i64,
}
/// Whether a window of consecutive sites is a path: every site in it is a different one. A
/// window that comes back to a site already in it ("search engine → site → search engine") is
/// a round trip, which said nothing and filled the list on a real archive.
pub(super) fn is_path(window: &[(String, i64)]) -> bool {
    window
        .iter()
        .enumerate()
        .all(|(index, (site, _))| window[..index].iter().all(|(earlier, _)| earlier != site))
}

pub(super) fn finalize_path_sequence(
    sequence: &[(String, i64)],
    steps: usize,
    flows: &mut HashMap<(String, usize), FlowAggregate>,
) {
    if steps < 2 || sequence.len() < steps {
        return;
    }
    for window in sequence.windows(steps).filter(|window| is_path(window)) {
        let pattern =
            window.iter().map(|(domain, _)| domain.as_str()).collect::<Vec<_>>().join(" → ");
        let time = window.iter().map(|(_, time)| *time).max().unwrap();
        let aggregate = flows.entry((pattern, steps)).or_default();
        aggregate.occurrence_count += 1;
        aggregate.last_seen_ms = aggregate.last_seen_ms.max(time);
    }
}
pub(super) fn build_path_flow_steps(pattern: &str) -> Vec<PathFlowStep> {
    pattern
        .split(" → ")
        .enumerate()
        .map(|(index, label)| PathFlowStep {
            index: index as i64,
            label: label.into(),
            registrable_domain: is_registrable_domain(label).then(|| label.to_owned()),
        })
        .collect()
}
fn is_registrable_domain(value: &str) -> bool {
    let mut count = 0;
    for part in value.split('.') {
        count += 1;
        if part.is_empty() || !part.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
            return false;
        }
    }
    count >= 2
}
