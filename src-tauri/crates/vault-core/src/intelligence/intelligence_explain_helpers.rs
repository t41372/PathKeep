//! Shared helper routines for Core Intelligence explainability.
//!
//! ## Responsibilities
//! - Parse stable explanation entity ids into typed components.
//! - Load the minimal visit-id evidence needed by explanation builders.
//! - Centralize common explainability factor construction so explanation modules
//!   do not duplicate the same bookkeeping logic.
//!
//! ## Not responsible for
//! - Dispatching explanation requests.
//! - Formatting top-level route payloads.
//! - Recomputing any intelligence entity state.
//!
//! ## Dependencies
//! - Parent-module query normalization helpers.
//! - `visit_derived_facts`, `search_events`, `path_flows`, and related archive
//!   joins in the intelligence plane.
//!
//! ## Performance notes
//! - Helper loaders stay scoped to one entity id or one normalized-query set and
//!   never trigger a global rebuild.

use super::normalize_query;
use crate::models::ExplainabilityFactor;
use anyhow::{Context, Result};
use rusqlite::{Connection, params};

/// Creates a consistently shaped explainability factor so each explanation
/// surface can focus on its evidence math instead of payload bookkeeping.
pub(super) fn explainability_factor(
    label: &str,
    raw_value: f32,
    weight: f32,
    contribution: f32,
) -> ExplainabilityFactor {
    ExplainabilityFactor { label: label.to_string(), raw_value, weight, contribution }
}

/// Decodes compare-set ids into the trail anchor and page-category key used by
/// the persisted compare-set tables.
pub(super) fn parse_compare_set_entity_id(entity_id: &str) -> Result<(&str, &str)> {
    let payload = entity_id.split_once("compare:").map(|(_, value)| value).unwrap_or(entity_id);
    payload
        .rsplit_once(':')
        .with_context(|| format!("compare_set explanations expect ids shaped like 'compare:<trail_id>:<page_category>', got {entity_id}"))
}

/// Reads `visitIds` from persisted evidence JSON while tolerating older rows
/// that may have partial or malformed payloads.
pub(super) fn extract_visit_ids_from_evidence_json(evidence_json: &str) -> Vec<i64> {
    serde_json::from_str::<serde_json::Value>(evidence_json)
        .ok()
        .and_then(|value| value.get("visitIds").cloned())
        .and_then(|value| value.as_array().cloned())
        .unwrap_or_default()
        .into_iter()
        .filter_map(|value| value.as_i64())
        .collect()
}

/// Parses the shared `<profile_id>::<entity>` id format used by several
/// deterministic explanation entities.
pub(super) fn parse_two_part_entity_id<'a>(
    entity_id: &'a str,
    entity_type: &str,
) -> Result<(&'a str, &'a str)> {
    entity_id.split_once("::").with_context(|| {
        format!("{entity_type} explanations use the '<profile_id>::<entity>' entity_id format.")
    })
}

/// Parses the persisted path-flow id into the profile scope, step count, and
/// normalized flow pattern expected by the structural read model.
pub(super) fn parse_path_flow_entity_id(entity_id: &str) -> Result<(&str, i64, &str)> {
    let mut parts = entity_id.splitn(3, "::");
    let profile_id = parts.next().filter(|value| !value.is_empty()).context(
        "path_flow explanations use the '<profile_id>::<step_count>::<flow_pattern>' entity_id format.",
    )?;
    let step_count = parts
        .next()
        .context("missing path_flow step_count")?
        .parse::<i64>()
        .context("path_flow step_count must be an integer")?;
    let flow_pattern =
        parts.next().filter(|value| !value.is_empty()).context("missing path_flow flow_pattern")?;
    Ok((profile_id, step_count, flow_pattern))
}

/// Loads the exact search visit ids behind one query family by matching the
/// normalized-query set stored in that family record.
pub(super) fn load_query_family_visit_ids(
    connection: &Connection,
    profile_id: &str,
    search_engine: &str,
    queries: &[String],
) -> Result<Vec<i64>> {
    if queries.is_empty() {
        return Ok(Vec::new());
    }
    let normalized_queries = queries.iter().map(|query| normalize_query(query)).collect::<Vec<_>>();
    let placeholders =
        std::iter::repeat_n("?", normalized_queries.len()).collect::<Vec<_>>().join(", ");
    let sql = format!(
        "SELECT visit_id
         FROM search_events
         WHERE profile_id = ?1
           AND search_engine = ?2
           AND normalized_query IN ({placeholders})
         ORDER BY visit_id ASC"
    );
    let mut statement = connection.prepare(&sql)?;
    let params = std::iter::once(&profile_id as &dyn rusqlite::ToSql)
        .chain(std::iter::once(&search_engine as &dyn rusqlite::ToSql))
        .chain(normalized_queries.iter().map(|query| query as &dyn rusqlite::ToSql));
    statement
        .query_map(rusqlite::params_from_iter(params), |row| row.get::<_, i64>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(Into::into)
}

/// Loads all visit ids for one registrable domain so domain-level explanations
/// can point back to archive facts instead of just counts.
pub(super) fn load_domain_visit_ids(
    connection: &Connection,
    profile_id: &str,
    registrable_domain: &str,
) -> Result<Vec<i64>> {
    connection
        .prepare(
            "SELECT visit_id
             FROM visit_derived_facts
             WHERE profile_id = ?1
               AND registrable_domain = ?2
             ORDER BY visit_id ASC",
        )?
        .query_map(params![profile_id, registrable_domain], |row| row.get::<_, i64>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(Into::into)
}

/// Replays the most recent session-local domain sequence that matched a stored
/// path-flow pattern without touching unrelated profiles.
pub(super) fn load_recent_path_flow_visit_ids(
    connection: &Connection,
    profile_id: &str,
    step_count: i64,
    flow_pattern: &str,
) -> Result<Vec<i64>> {
    let target_domains =
        flow_pattern.split(" → ").map(|value| value.to_string()).collect::<Vec<_>>();
    let mut statement = connection.prepare(
        "SELECT visit_derived_facts.session_id,
                visit_derived_facts.registrable_domain,
                visit_derived_facts.visit_id,
                visits.visit_time_ms
         FROM visit_derived_facts
         JOIN archive.visits AS visits ON visits.id = visit_derived_facts.visit_id
         WHERE visit_derived_facts.profile_id = ?1
           AND visit_derived_facts.session_id IS NOT NULL
         ORDER BY visit_derived_facts.session_id ASC, visits.visit_time_ms ASC, visits.id ASC",
    )?;
    let rows = statement
        .query_map([profile_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, i64>(3)?,
            ))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let mut current_session = String::new();
    let mut current_sequence = Vec::<(String, i64, i64)>::new();
    let mut latest_match = None::<(i64, Vec<i64>)>;
    for (session_id, domain, visit_id, visit_time_ms) in rows {
        if current_session != session_id {
            update_latest_path_flow_match(
                &current_sequence,
                step_count as usize,
                &target_domains,
                &mut latest_match,
            );
            current_session = session_id;
            current_sequence.clear();
        }
        if current_sequence.last().is_none_or(|(last_domain, _, _)| last_domain != &domain) {
            current_sequence.push((domain, visit_id, visit_time_ms));
        } else if let Some((_, last_visit_id, last_seen_ms)) = current_sequence.last_mut() {
            *last_visit_id = visit_id;
            *last_seen_ms = visit_time_ms;
        }
    }
    update_latest_path_flow_match(
        &current_sequence,
        step_count as usize,
        &target_domains,
        &mut latest_match,
    );
    Ok(latest_match.map(|(_, visit_ids)| visit_ids).unwrap_or_default())
}

/// Keeps only the newest matching sequence so path-flow explanations point to
/// the most relevant evidence a user can still inspect.
fn update_latest_path_flow_match(
    sequence: &[(String, i64, i64)],
    step_count: usize,
    target_domains: &[String],
    latest_match: &mut Option<(i64, Vec<i64>)>,
) {
    if sequence.len() < step_count || target_domains.len() != step_count {
        return;
    }
    for window in sequence.windows(step_count) {
        let domains = window.iter().map(|(domain, _, _)| domain).collect::<Vec<_>>();
        if domains.iter().zip(target_domains).all(|(left, right)| *left == right) {
            let last_seen_ms =
                window.iter().map(|(_, _, visit_time_ms)| *visit_time_ms).max().unwrap_or(0);
            let visit_ids = window.iter().map(|(_, visit_id, _)| *visit_id).collect::<Vec<_>>();
            match latest_match {
                Some((current_last_seen_ms, _)) if *current_last_seen_ms >= last_seen_ms => {}
                _ => *latest_match = Some((last_seen_ms, visit_ids)),
            }
        }
    }
}
