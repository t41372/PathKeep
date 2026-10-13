//! Failure modes: reads write schemas, a query escapes its owner, stale scopes leak,
//! or bounded results lose continuation. Native render-path and paging tests cover these.
//! Entity explainability surfaces for Core Intelligence.
//!
//! ## Responsibilities
//! - Translate persisted deterministic entities into explanation payloads that
//!   show why an entity exists.
//! - Reuse persisted evidence rows instead of recomputing entity state during an
//!   explanation request; a habit is worked out from its site's daily rollups, as the
//!   Habits card does.
//! - Keep explanation-specific parsing and factor-building out of the rebuild
//!   pipeline.
//!
//! ## Not responsible for
//! - Route-level detail pages and top-level overview cards.
//! - Rebuilding or mutating intelligence tables.
//! - Host-artifact generation.
//!
//! ## Dependencies
//! - `intelligence_refind` for persisted refind score evidence.
//! - Parent-module helpers for query normalization and trail/session ids.
//! - The explicit entity-type dispatcher to enforce the accepted explainability
//!   surface.
//!
//! ## Performance notes
//! - Explanations only read the evidence needed for one entity id and do not
//!   trigger rebuilds or full-table scans beyond the scoped helper queries.

use super::{
    intelligence_explain_helpers::{
        explainability_factor, load_domain_visit_ids, load_membership_visit_ids,
        load_query_family_visit_ids, load_recent_path_flow_visit_ids, parse_compare_set_entity_id,
        parse_path_flow_entity_id, parse_two_part_entity_id,
    },
    normalize_query,
};
use crate::models::{EntityExplanationRequest, ExplainabilityFactor, Explanation, HabitPattern};
use anyhow::{Context, Result};
use std::collections::{BTreeSet, HashSet};
use vault_store::{OptionalRow, Session, params_owned as params};

/// Builds the public explanation payload for one Core Intelligence entity id.
///
/// The entity type is validated against the accepted deterministic dispatcher so
/// the frontend cannot ask for explanations that the backend does not ship.
pub fn explain_entity(
    vault: &crate::Vault,
    request: &EntityExplanationRequest,
) -> Result<Explanation> {
    let request = request.clone();
    vault.read(vault_store::Db::Intelligence, move |connection| {
        let request = &request;

        let entity_type = request.entity_type.as_str();
        let cursor = request.cursor.as_deref();
        let limit = request.page_size.unwrap_or(50).clamp(1, 1000) as usize;
        match entity_type {
            "session" => explain_session(connection, &request.entity_id, cursor, limit),
            "search_trail" => explain_search_trail(connection, &request.entity_id, cursor, limit),
            "query_family" => explain_query_family(connection, &request.entity_id, cursor, limit),
            "refind_page" => {
                let explanation = super::intelligence_refind::build_refind_explanation(
                    connection,
                    &request.entity_id,
                    None,
                    request.cursor.as_deref(),
                    request.page_size.unwrap_or(50).clamp(1, 1000) as usize,
                )?;
                Ok(Explanation {
                    entity_type: "refind_page".to_string(),
                    entity_id: explanation.canonical_url.clone(),
                    trigger_rule: format!("Refind score >= {:.1}", explanation.refind_score),
                    trigger_rule_code: Some(Explanation::TRIGGER_RULE_REFIND_SCORE.to_string()),
                    // The score travels STRUCTURALLY so the panel interpolates it into localized copy
                    // instead of regexing it back out of the prose above.
                    trigger_rule_score: Some(explanation.refind_score),
                    factors: explanation
                        .factors
                        .into_iter()
                        .map(|factor| ExplainabilityFactor {
                            label: factor.signal,
                            raw_value: factor.raw_value,
                            weight: factor.weight,
                            contribution: factor.contribution,
                        })
                        .collect(),
                    participating_visit_ids: explanation.visit_ids,
                    participating_visit_ids_next_cursor: explanation.visit_ids_next_cursor,
                    ..Default::default()
                })
            }
            "reopened_investigation" => explain_reopened_investigation(
                connection,
                &request.entity_id,
                request.cursor.as_deref(),
                request.page_size.unwrap_or(50).clamp(1, 1000) as usize,
            ),
            "habit_pattern" => explain_habit_pattern(connection, &request.entity_id, cursor, limit),
            "path_flow" => explain_path_flow(connection, &request.entity_id),
            "compare_set" => explain_compare_set(connection, &request.entity_id, cursor, limit),
            _ => anyhow::bail!(
                "Core Intelligence explainability does not support entity type '{entity_type}'."
            ),
        }
    })
}

/// Explains why a set of archive visits was grouped into one session and
/// whether that session crossed the deep-dive thresholds.
fn explain_session(
    connection: &Session<'_>,
    session_id: &str,
    cursor: Option<&str>,
    limit: usize,
) -> Result<Explanation> {
    let (visit_count, search_count, domain_count, is_deep_dive, first_visit_ms, last_visit_ms) =
        connection
            .query_one(
                "SELECT visit_count, search_count, domain_count, is_deep_dive, first_visit_ms, last_visit_ms
                 FROM sessions
                 WHERE session_id = ?1",
                [session_id],
                |row| {
                    Ok((
                        row.get::< i64>(0)?,
                        row.get::< i64>(1)?,
                        row.get::< i64>(2)?,
                        row.get::< i64>(3)? != 0,
                        row.get::< i64>(4)?,
                        row.get::< i64>(5)?,
                    ))
                },
            )
            .optional()?
            .with_context(|| format!("session {session_id} was not found"))?;
    let visit_ids = load_membership_visit_ids(connection, session_id, false, cursor, limit)?;
    let navigation_chain_depth = connection
        .query_one(
            "SELECT COUNT(*)
             FROM visit_derived_facts
             JOIN archive.visits AS visits ON visits.id = visit_derived_facts.visit_id
             WHERE visit_derived_facts.session_id = ?1
               AND visits.from_visit > 0",
            [session_id],
            |row| Ok(row.get::<i64>(0)?),
        )
        .unwrap_or(0);
    let duration_minutes = ((last_visit_ms - first_visit_ms).max(0) as f32) / 60_000.0;
    // Both the prose and the localized copy quote the ONE grouping threshold the structural builder
    // actually applies (`SESSION_GAP_MS`), so the stated number can never drift from the behavior.
    let gap_minutes = super::SESSION_GAP_MS / 60_000;
    Ok(Explanation {
        entity_type: "session".to_string(),
        entity_id: session_id.to_string(),
        trigger_rule: if is_deep_dive {
            "Deep dive session matched the navigation-depth, domain-count, and visit-count thresholds."
                .to_string()
        } else {
            format!(
                "Visits were grouped into one session because adjacent gaps stayed within {gap_minutes} minutes."
            )
        },
        trigger_rule_code: Some(
            if is_deep_dive {
                Explanation::TRIGGER_RULE_SESSION_DEEP_DIVE
            } else {
                Explanation::TRIGGER_RULE_SESSION_GAP
            }
            .to_string(),
        ),
        // Only the gap rule quotes a threshold; the deep-dive rule names no number.
        trigger_rule_gap_minutes: (!is_deep_dive).then_some(gap_minutes),
        factors: vec![
            explainability_factor("visit_count", visit_count as f32, 1.0, visit_count as f32),
            explainability_factor(
                "search_count",
                search_count as f32,
                0.5,
                search_count as f32 * 0.5,
            ),
            explainability_factor(
                "unique_domain_count",
                domain_count as f32,
                0.6,
                domain_count as f32 * 0.6,
            ),
            explainability_factor(
                "navigation_chain_depth",
                navigation_chain_depth as f32,
                0.7,
                navigation_chain_depth as f32 * 0.7,
            ),
            explainability_factor(
                "duration_minutes",
                duration_minutes,
                0.15,
                duration_minutes * 0.15,
            ),
        ],
        participating_visit_ids: visit_ids.items,
        participating_visit_ids_next_cursor: visit_ids.next_cursor,
        ..Default::default()
    })
}

/// Explains why a search trail exists by surfacing its anchor query, trail
/// depth, and the exact visits captured in the trail membership table.
fn explain_search_trail(
    connection: &Session<'_>,
    trail_id: &str,
    cursor: Option<&str>,
    limit: usize,
) -> Result<Explanation> {
    let (initial_query, reformulation_count, visit_count, max_depth, landing_domain) = connection
        .query_one(
            "SELECT initial_query, reformulation_count, visit_count, max_depth, landing_domain
             FROM search_trails
             WHERE trail_id = ?1",
            [trail_id],
            |row| {
                Ok((
                    row.get::<String>(0)?,
                    row.get::<i64>(1)?,
                    row.get::<i64>(2)?,
                    row.get::<i64>(3)?,
                    row.get::<Option<String>>(4)?,
                ))
            },
        )
        .optional()?
        .with_context(|| format!("search trail {trail_id} was not found"))?;
    let visit_ids = load_membership_visit_ids(connection, trail_id, true, cursor, limit)?;
    let landing_score = if landing_domain.is_some() { 1.0 } else { 0.0 };
    Ok(Explanation {
        entity_type: "search_trail".to_string(),
        entity_id: trail_id.to_string(),
        trigger_rule: format!(
            "Search trail anchored by '{}' and extended through navigation ancestry within the session window.",
            initial_query
        ),
        trigger_rule_code: Some(Explanation::TRIGGER_RULE_SEARCH_TRAIL.to_string()),
        // The anchor query travels STRUCTURALLY: quoting it inside prose forced the panel to re-parse
        // its own quote characters, which broke on any query containing them.
        trigger_rule_query: Some(initial_query.clone()),
        factors: vec![
            explainability_factor("visit_count", visit_count as f32, 1.0, visit_count as f32),
            explainability_factor(
                "reformulation_count",
                reformulation_count as f32,
                0.8,
                reformulation_count as f32 * 0.8,
            ),
            explainability_factor("max_depth", max_depth as f32, 0.6, max_depth as f32 * 0.6),
            explainability_factor("landing_detected", landing_score, 0.5, landing_score * 0.5),
        ],
        participating_visit_ids: visit_ids.items,
        participating_visit_ids_next_cursor: visit_ids.next_cursor,
        ..Default::default()
    })
}

/// Explains why multiple related queries collapsed into one family and returns
/// the concrete search visits that back that family.
fn explain_query_family(
    connection: &Session<'_>,
    family_id: &str,
    cursor: Option<&str>,
    limit: usize,
) -> Result<Explanation> {
    let (profile_id, anchor_query, member_count, search_engine, queries_json) = connection
        .query_one(
            "SELECT profile_id, anchor_query, member_count, search_engine, queries_json
             FROM query_families
             WHERE family_id = ?1",
            [family_id],
            |row| {
                Ok((
                    row.get::<String>(0)?,
                    row.get::<String>(1)?,
                    row.get::<i64>(2)?,
                    row.get::<String>(3)?,
                    row.get::<String>(4)?,
                ))
            },
        )
        .optional()?
        .with_context(|| format!("query family {family_id} was not found"))?;
    let queries = serde_json::from_str::<Vec<String>>(&queries_json).unwrap_or_default();
    let normalized_queries =
        queries.iter().map(|query| normalize_query(query)).collect::<HashSet<_>>();
    let visit_ids = load_query_family_visit_ids(
        connection,
        &profile_id,
        &search_engine,
        &queries,
        cursor,
        limit,
    )?;
    Ok(Explanation {
        entity_type: "query_family".to_string(),
        entity_id: family_id.to_string(),
        trigger_rule: format!(
            "Queries were merged into one family because their Jaccard or containment similarity matched '{}'.",
            anchor_query
        ),
        trigger_rule_code: Some(Explanation::TRIGGER_RULE_QUERY_FAMILY.to_string()),
        // Structural anchor query: quoting it inside prose forced the panel to re-parse its own quote
        // characters, which broke on any anchor containing them.
        trigger_rule_query: Some(anchor_query.clone()),
        factors: vec![
            explainability_factor("member_count", member_count as f32, 1.0, member_count as f32),
            explainability_factor(
                "distinct_query_count",
                normalized_queries.len() as f32,
                0.7,
                normalized_queries.len() as f32 * 0.7,
            ),
        ],
        participating_visit_ids: visit_ids.items,
        participating_visit_ids_next_cursor: visit_ids.next_cursor,
        ..Default::default()
    })
}

/// Explains why a deterministic reopened-investigation row fired, including the
/// repeated anchor evidence that pushed it over the threshold.
fn explain_reopened_investigation(
    connection: &Session<'_>,
    investigation_id: &str,
    cursor: Option<&str>,
    limit: usize,
) -> Result<Explanation> {
    let (anchor_type, anchor_id, occurrence_count, distinct_days, profile_id) = connection
        .query_one(
            "SELECT anchor_type, anchor_id, occurrence_count, distinct_days, profile_id
             FROM reopened_investigations
             WHERE investigation_id = ?1",
            [investigation_id],
            |row| {
                Ok((
                    row.get::<String>(0)?,
                    row.get::<String>(1)?,
                    row.get::<i64>(2)?,
                    row.get::<i64>(3)?,
                    row.get::<String>(4)?,
                ))
            },
        )
        .optional()?
        .with_context(|| format!("reopened investigation {investigation_id} was not found"))?;
    let mut participating_visit_ids_next_cursor = None;
    let participating_visit_ids = match anchor_type.as_str() {
        "query_family" => {
            let (profile_id, search_engine, queries_json) = connection.query_one(
                "SELECT profile_id, search_engine, queries_json
                 FROM query_families
                 WHERE family_id = ?1",
                [anchor_id.as_str()],
                |row| Ok((row.get::<String>(0)?, row.get::<String>(1)?, row.get::<String>(2)?)),
            )?;
            let queries = serde_json::from_str::<Vec<String>>(&queries_json).unwrap_or_default();
            let page = load_query_family_visit_ids(
                connection,
                &profile_id,
                &search_engine,
                &queries,
                cursor,
                limit,
            )?;
            participating_visit_ids_next_cursor = page.next_cursor;
            page.items
        }
        "reference_page" => {
            let page =
                super::refind_evidence::visits(connection, &profile_id, &anchor_id, cursor, limit)?;
            participating_visit_ids_next_cursor = page.next_cursor;
            page.items
        }
        _ => Vec::new(),
    };
    Ok(Explanation {
        entity_type: "reopened_investigation".to_string(),
        entity_id: investigation_id.to_string(),
        trigger_rule: "This investigation reopened because the same anchor reappeared across distinct days or repeated deterministic evidence."
            .to_string(),
        trigger_rule_code: Some(Explanation::TRIGGER_RULE_REOPENED_INVESTIGATION.to_string()),
        factors: vec![
            explainability_factor(
                "occurrence_count",
                occurrence_count as f32,
                1.0,
                occurrence_count as f32,
            ),
            explainability_factor(
                "distinct_days",
                distinct_days as f32,
                0.8,
                distinct_days as f32 * 0.8,
            ),
        ],
        participating_visit_ids,
        participating_visit_ids_next_cursor,
        ..Default::default()
    })
}

/// Explains the cadence evidence behind one habit-pattern row so the UI can
/// describe why a domain looks recurring instead of incidental.
fn explain_habit_pattern(
    connection: &Session<'_>,
    entity_id: &str,
    cursor: Option<&str>,
    limit: usize,
) -> Result<Explanation> {
    let (profile_id, registrable_domain) = parse_two_part_entity_id(entity_id, "habit_pattern")?;
    let HabitPattern { habit_type, mean_interval_days, cv, visit_count, is_interrupted, .. } =
        super::phase_three::site_habit(connection, profile_id, registrable_domain)?
            .with_context(|| format!("habit pattern {entity_id} was not found"))?;
    let visit_ids =
        load_domain_visit_ids(connection, profile_id, registrable_domain, cursor, limit)?;
    Ok(Explanation {
        entity_type: "habit_pattern".to_string(),
        entity_id: entity_id.to_string(),
        trigger_rule: if is_interrupted {
            format!(
                "{} cadence was detected and later crossed its interruption threshold.",
                habit_type
            )
        } else {
            format!("{} cadence was detected from repeated cross-day visits.", habit_type)
        },
        trigger_rule_code: Some(
            if is_interrupted {
                Explanation::TRIGGER_RULE_HABIT_PATTERN_INTERRUPTED
            } else {
                Explanation::TRIGGER_RULE_HABIT_PATTERN
            }
            .to_string(),
        ),
        // The habit TYPE code travels structurally; the front end owns its localized name, so it no
        // longer has to capture the name from the sentence with a regex.
        trigger_rule_habit_type: Some(habit_type.clone()),
        factors: vec![
            explainability_factor("visit_count", visit_count as f32, 1.0, visit_count as f32),
            explainability_factor(
                "mean_interval_days",
                mean_interval_days,
                0.7,
                mean_interval_days * 0.7,
            ),
            explainability_factor("coefficient_of_variation", cv, -0.6, cv * -0.6),
            explainability_factor(
                "interrupted",
                if is_interrupted { 1.0 } else { 0.0 },
                0.4,
                if is_interrupted { 0.4 } else { 0.0 },
            ),
        ],
        participating_visit_ids: visit_ids.items,
        participating_visit_ids_next_cursor: visit_ids.next_cursor,
        ..Default::default()
    })
}

/// Explains the latest sequence of visits that matched a persisted path-flow
/// pattern without rebuilding path-flow state on demand.
fn explain_path_flow(connection: &Session<'_>, entity_id: &str) -> Result<Explanation> {
    let (profile_id, step_count, flow_pattern) = parse_path_flow_entity_id(entity_id)?;
    let (occurrence_count, last_seen_ms) = connection
        .query_one(
            "SELECT occurrence_count, last_seen_ms
             FROM path_flows
             WHERE profile_id = ?1
               AND step_count = ?2
               AND flow_pattern = ?3",
            params![profile_id, step_count, flow_pattern],
            |row| Ok((row.get::<i64>(0)?, row.get::<i64>(1)?)),
        )
        .optional()?
        .with_context(|| format!("path flow {entity_id} was not found"))?;
    let participating_visit_ids = load_recent_path_flow_visit_ids(
        connection,
        profile_id,
        step_count,
        flow_pattern,
        last_seen_ms,
    )?;
    Ok(Explanation {
        entity_type: "path_flow".to_string(),
        entity_id: entity_id.to_string(),
        trigger_rule: "This flow pattern recurs across session-local domain n-grams.".to_string(),
        trigger_rule_code: Some(Explanation::TRIGGER_RULE_PATH_FLOW.to_string()),
        factors: vec![
            explainability_factor("step_count", step_count as f32, 0.6, step_count as f32 * 0.6),
            explainability_factor(
                "occurrence_count",
                occurrence_count as f32,
                1.0,
                occurrence_count as f32,
            ),
        ],
        participating_visit_ids,
        ..Default::default()
    })
}

/// Explains why a compare-set candidate exists by surfacing alternating pages
/// inside one trail and the visit ids that participated in the comparison.
fn explain_compare_set(
    connection: &Session<'_>,
    entity_id: &str,
    cursor: Option<&str>,
    limit: usize,
) -> Result<Explanation> {
    let (trail_id, page_category) = parse_compare_set_entity_id(entity_id)?;
    let mut statement = connection.prepare(
        "SELECT search_trail_members.visit_id,
                visit_derived_facts.canonical_url,
                visit_derived_facts.registrable_domain,
                search_trail_members.ordinal
         FROM search_trail_members
         JOIN visit_derived_facts ON visit_derived_facts.visit_id = search_trail_members.visit_id
         WHERE search_trail_members.trail_id = ?1
           AND visit_derived_facts.page_category = ?2
         ORDER BY search_trail_members.ordinal ASC, search_trail_members.visit_id ASC",
    )?;
    let members = statement.query_map(params![trail_id, page_category], |row| {
        Ok((row.get::<i64>(0)?, row.get::<String>(1)?, row.get::<String>(2)?, row.get::<i64>(3)?))
    })?;
    let after = super::paging::cursor::<(i64, i64)>(cursor)?;
    let mut canonical_urls = BTreeSet::new();
    let mut domains = BTreeSet::new();
    let mut alternations = 0_i64;
    let mut previous_url = None::<String>;
    let mut ids = Vec::new();
    for member in members {
        let (visit_id, canonical_url, domain, ordinal) = member?;
        canonical_urls.insert(canonical_url.clone());
        domains.insert(domain);
        if previous_url.as_ref().is_some_and(|previous| previous != &canonical_url) {
            alternations += 1;
        }
        previous_url = Some(canonical_url);
        let key = (ordinal, visit_id);
        if after.is_none_or(|after| key > after) && ids.len() <= limit {
            ids.push((visit_id, key));
        }
    }
    if canonical_urls.is_empty() {
        anyhow::bail!("compare set {entity_id} was not found");
    }
    let ids = super::paging::finish(ids, limit)?;

    Ok(Explanation {
        entity_type: "compare_set".to_string(),
        entity_id: entity_id.to_string(),
        trigger_rule:
            "This compare set alternated between multiple comparable pages within one search trail."
                .to_string(),
        trigger_rule_code: Some(Explanation::TRIGGER_RULE_COMPARE_SET.to_string()),
        factors: vec![
            explainability_factor(
                "page_count",
                canonical_urls.len() as f32,
                0.8,
                canonical_urls.len() as f32 * 0.8,
            ),
            explainability_factor(
                "domain_count",
                domains.len() as f32,
                0.7,
                domains.len() as f32 * 0.7,
            ),
            explainability_factor(
                "alternation_count",
                alternations as f32,
                1.0,
                alternations as f32,
            ),
        ],
        participating_visit_ids: ids.items,
        participating_visit_ids_next_cursor: ids.next_cursor,
        ..Default::default()
    })
}
