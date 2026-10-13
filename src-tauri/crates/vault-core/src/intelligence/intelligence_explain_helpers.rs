//! Failure modes: reads write schemas, a query escapes its owner, stale scopes leak,
//! or bounded results lose continuation. Native render-path and paging tests cover these.
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
use vault_store::{Session, params_owned as params};

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

/// Pages navigation membership without hydrating every visit in a large session or trail.
pub(super) fn load_membership_visit_ids(
    s: &Session<'_>,
    entity: &str,
    trail: bool,
    cursor: Option<&str>,
    limit: usize,
) -> Result<crate::models::IntelligencePage<i64>> {
    let (table, key, index) = if trail {
        ("search_trail_members_rows", "trail_id", "idx_search_trail_members_order")
    } else {
        ("visit_assignments", "session_id", "idx_assignments_generation_session")
    };
    let after = super::paging::cursor::<(i64, i64)>(cursor)?;
    let base = format!(
        "SELECT visit_id,ordinal FROM {table} INDEXED BY {index} WHERE generation IN (SELECT navigation FROM intelligence_generations) AND {key}=?1"
    );
    let map = |r: &vault_store::Row| {
        let id = r.get::<i64>(0)?;
        Ok((id, (r.get::<i64>(1)?, id)))
    };
    let rows = if let Some((ordinal, id)) = after {
        let mut rows = s.query(
            &format!("{base} AND ordinal=?2 AND visit_id>?3 ORDER BY ordinal,visit_id LIMIT ?4"),
            params![entity, ordinal, id, (limit + 1) as i64],
            map,
        )?;
        if rows.len() <= limit {
            rows.extend(s.query(
                &format!("{base} AND ordinal>?2 ORDER BY ordinal,visit_id LIMIT ?3"),
                params![entity, ordinal, (limit + 1 - rows.len()) as i64],
                map,
            )?);
        }
        rows
    } else {
        s.query(
            &format!("{base} ORDER BY ordinal,visit_id LIMIT ?2"),
            params![entity, (limit + 1) as i64],
            map,
        )?
    };
    super::paging::finish(rows, limit)
}

/// Pages exact search evidence through the family's complete normalized query set.
pub(super) fn load_query_family_visit_ids(
    connection: &Session<'_>,
    profile_id: &str,
    search_engine: &str,
    queries: &[String],
    cursor: Option<&str>,
    limit: usize,
) -> Result<crate::models::IntelligencePage<i64>> {
    let normalized = queries.iter().map(|q| normalize_query(q)).collect::<Vec<_>>();
    let after = super::paging::cursor::<i64>(cursor)?.unwrap_or(0);
    let rows = connection.query(
        "SELECT DISTINCT e.visit_id FROM json_each(?3) q
         CROSS JOIN search_events e ON e.normalized_query=q.value
         WHERE e.profile_id=?1 AND e.search_engine=?2 AND e.visit_id>?4
         ORDER BY e.visit_id LIMIT ?5",
        params![
            profile_id,
            search_engine,
            serde_json::to_string(&normalized)?,
            after,
            (limit + 1) as i64
        ],
        |r| {
            let id = r.get::<i64>(0)?;
            Ok((id, id))
        },
    )?;
    super::paging::finish(rows, limit)
}

/// Pages a habit's complete domain evidence using visit IDs as stable keys.
pub(super) fn load_domain_visit_ids(
    connection: &Session<'_>,
    profile_id: &str,
    registrable_domain: &str,
    cursor: Option<&str>,
    limit: usize,
) -> Result<crate::models::IntelligencePage<i64>> {
    let after = super::paging::cursor::<i64>(cursor)?.unwrap_or(0);
    let rows = connection.query(
        "SELECT visit_id FROM visit_derived_facts WHERE profile_id=?1
         AND registrable_domain=?2 AND visit_id>?3 ORDER BY visit_id LIMIT ?4",
        params![profile_id, registrable_domain, after, (limit + 1) as i64],
        |r| {
            let id = r.get::<i64>(0)?;
            Ok((id, id))
        },
    )?;
    super::paging::finish(rows, limit)
}

/// Replays the most recent session-local domain sequence that matched a stored
/// path-flow pattern without touching unrelated profiles.
pub(super) fn load_recent_path_flow_visit_ids(
    connection: &Session<'_>,
    profile_id: &str,
    step_count: i64,
    flow_pattern: &str,
    last_seen_ms: i64,
) -> Result<Vec<i64>> {
    let target = flow_pattern.rsplit(" → ").collect::<Vec<_>>();
    let mut statement = connection.prepare(
        "SELECT a.session_id,f.registrable_domain,v.id
         FROM archive.visits v INDEXED BY idx_visits_visible_profile_time_id
         CROSS JOIN intelligence_generations g ON g.profile_id=?1
         CROSS JOIN visit_derived_facts_rows f INDEXED BY idx_vdf_visit_generation ON f.visit_id=v.id AND f.generation=g.facts
         CROSS JOIN visit_assignments a INDEXED BY idx_assignments_visit_generation ON a.visit_id=v.id AND a.generation=g.navigation
         WHERE v.source_profile_id=(SELECT id FROM archive.source_profiles WHERE profile_key=?1)
           AND v.reverted_at IS NULL AND v.visit_time_ms<=?2 AND a.session_id IS NOT NULL
         ORDER BY v.visit_time_ms DESC,v.id DESC",
    )?;
    let rows = statement.query_map(params![profile_id, last_seen_ms], |r| {
        Ok((r.get::<String>(0)?, r.get::<String>(1)?, r.get::<i64>(2)?))
    })?;
    let mut session = String::new();
    let mut recent = std::collections::VecDeque::<(String, i64)>::new();
    for row in rows {
        let (next_session, domain, id) = row?;
        if next_session != session {
            recent.clear();
            session = next_session;
        }
        // Descending traversal encounters the last visit in each repeated-domain run first.
        if recent.back().is_some_and(|(previous, _)| previous == &domain) {
            continue;
        }
        if recent.len() == step_count as usize {
            recent.pop_front();
        }
        recent.push_back((domain, id));
        if recent.len() == target.len()
            && recent.iter().zip(&target).all(|((domain, _), target)| domain == target)
        {
            return Ok(recent.into_iter().rev().map(|(_, id)| id).collect());
        }
    }
    Ok(Vec::new())
}
