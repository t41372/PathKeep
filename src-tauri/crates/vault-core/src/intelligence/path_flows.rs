//! Responsibilities: page ranked navigation flows: all time from the stored flows, a date range
//! from the sessions' stored path segments; count a profile's flows from those segments for the
//! build that stores them, all of them or those of the last days an append rewrites.
//! Not responsible for: building the segments, or source-evidence hydration.
//! Failure modes: a result cap hides flows, repeated domains inflate windows, a session
//! boundary joins unrelated paths, a path crossing two segments counts twice or not at all, or a
//! read replays visits.
use super::{PathFlowRecord, rfc3339_from_millis};
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
    // The sessions' path segments of the range's days (see `PathSegment`): each path is counted in
    // the segment holding its last site, the earlier sites come from the segment's prefix.
    let mut statement = s.prepare(
        "SELECT p.prefix_len,p.sites_json FROM session_paths p INDEXED BY idx_session_paths_range
        WHERE p.date_key>=?2 AND p.date_key<=?3
          AND p.generation IN (SELECT navigation FROM intelligence_generations)
          AND (?1 IS NULL OR p.profile_id=?1)",
    )?;
    let rows = statement.query_map(
        params![request.profile_id, request.date_range.start, request.date_range.end],
        |r| Ok((r.get::<i64>(0)?, r.get::<String>(1)?)),
    )?;
    let mut flows = HashMap::new();
    for row in rows {
        let (prefix_len, sites) = row?;
        let sites: Vec<(String, i64)> = serde_json::from_str(&sites)?;
        count_segment_paths(&sites, prefix_len as usize, steps, &mut flows);
    }
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
/// Every path of a profile, counted from its sessions' path segments the way a date range counts
/// them: the all-time list a build stores. Pages through the segments in key order.
pub(super) fn profile_flows(
    vault: &Vault,
    generation: i64,
    profile: &str,
) -> Result<Vec<PathFlowRecord>> {
    const COLUMNS: &str = "SELECT session_id,segment,prefix_len,sites_json FROM session_paths
        WHERE generation=?1 AND profile_id=?2";
    let mut flows = HashMap::new();
    let mut after = None::<(String, i64)>;
    loop {
        let profile_id = profile.to_owned();
        let key = after.clone();
        let rows = vault.read(Db::Intelligence, move |s| {
            let map = |r: &vault_store::Row| -> vault_store::Result<(String, i64, i64, String)> {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))
            };
            let mut rows = Vec::new();
            if let Some((session, segment)) = &key {
                rows = s.query(
                    &format!(
                        "{COLUMNS} AND session_id=?3 AND segment>?4 ORDER BY segment LIMIT 4096"
                    ),
                    params![generation, profile_id.as_str(), session.as_str(), *segment],
                    map,
                )?;
            }
            if rows.len() < 4096 {
                let more = s.query(
                    &format!("{COLUMNS} AND session_id>?3 ORDER BY session_id,segment LIMIT ?4"),
                    params![
                        generation,
                        profile_id.as_str(),
                        key.as_ref().map_or("", |(session, _)| session.as_str()),
                        (4096 - rows.len()) as i64
                    ],
                    map,
                )?;
                rows.extend(more);
            }
            Ok(rows)
        })?;
        let Some((session, segment, ..)) = rows.last() else { break };
        after = Some((session.clone(), *segment));
        for (_, _, prefix_len, sites) in rows {
            count_all_lengths(prefix_len, &sites, &mut flows)?;
        }
    }
    Ok(records(profile, flows))
}

/// The paths of a profile's segments of `from_day` and later, counted like `profile_flows`. An
/// append writes only segments of those days (it rewrites the last open one and adds later
/// ones), so the difference of this count before and after it is what it adds to each flow.
pub(super) fn day_flows(
    s: &Session<'_>,
    generation: i64,
    profile: &str,
    from_day: &str,
) -> Result<Vec<PathFlowRecord>> {
    let rows = s.query(
        "SELECT prefix_len,sites_json FROM session_paths INDEXED BY idx_session_paths_range
         WHERE date_key>=?1 AND generation=?2 AND profile_id=?3",
        params![from_day, generation, profile],
        |r| Ok((r.get::<i64>(0)?, r.get::<String>(1)?)),
    )?;
    let mut flows = HashMap::new();
    for (prefix_len, sites) in rows {
        count_all_lengths(prefix_len, &sites, &mut flows)?;
    }
    Ok(records(profile, flows))
}

fn count_all_lengths(
    prefix_len: i64,
    sites_json: &str,
    flows: &mut HashMap<(String, usize), FlowAggregate>,
) -> Result<()> {
    let sites: Vec<(String, i64)> = serde_json::from_str(sites_json)?;
    for steps in 2..=4 {
        count_segment_paths(&sites, prefix_len as usize, steps, flows);
    }
    Ok(())
}

fn records(profile: &str, flows: HashMap<(String, usize), FlowAggregate>) -> Vec<PathFlowRecord> {
    flows
        .into_iter()
        .map(|((flow_pattern, steps), aggregate)| PathFlowRecord {
            profile_id: profile.to_owned(),
            flow_pattern,
            step_count: steps as i64,
            occurrence_count: aggregate.occurrence_count,
            last_seen_ms: aggregate.last_seen_ms,
        })
        .collect()
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

/// Counts the paths of `steps` sites (2 to 4) that end on one of the segment's own sites (those
/// after `prefix_len`), so a path that crosses into the next segment is counted there, once. A
/// whole session is a segment with no prefix.
pub(super) fn count_segment_paths(
    sites: &[(String, i64)],
    prefix_len: usize,
    steps: usize,
    flows: &mut HashMap<(String, usize), FlowAggregate>,
) {
    for end in prefix_len.max(steps - 1)..sites.len() {
        let window = &sites[end + 1 - steps..=end];
        if !is_path(window) {
            continue;
        }
        let pattern =
            window.iter().map(|(domain, _)| domain.as_str()).collect::<Vec<_>>().join(" → ");
        let aggregate = flows.entry((pattern, steps)).or_default();
        aggregate.occurrence_count += 1;
        aggregate.last_seen_ms = aggregate.last_seen_ms.max(window[steps - 1].1);
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
