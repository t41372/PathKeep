//! Entity explainability surfaces for Core Intelligence.
//!
//! ## Responsibilities
//! - Translate persisted deterministic entities into explanation payloads that
//!   show why an entity exists.
//! - Reuse persisted evidence rows instead of recomputing entity state during an
//!   explanation request.
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
    ensure_core_intelligence_schema,
    intelligence_explain_helpers::{
        explainability_factor, extract_visit_ids_from_evidence_json, load_domain_visit_ids,
        load_query_family_visit_ids, load_recent_path_flow_visit_ids, parse_compare_set_entity_id,
        parse_path_flow_entity_id, parse_two_part_entity_id,
    },
    normalize_query,
};
use crate::{
    archive::open_intelligence_connection,
    config::ProjectPaths,
    models::{AppConfig, EntityExplanationRequest, ExplainabilityFactor, Explanation},
};
use anyhow::{Context, Result};
use rusqlite::{Connection, OptionalExtension, params};
use std::collections::{BTreeSet, HashSet};

/// Builds the public explanation payload for one Core Intelligence entity id.
///
/// The entity type is validated against the accepted deterministic dispatcher so
/// the frontend cannot ask for explanations that the backend does not ship.
pub fn explain_entity(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    request: &EntityExplanationRequest,
) -> Result<Explanation> {
    let connection = open_intelligence_connection(paths, config, key)?;
    ensure_core_intelligence_schema(&connection)?;
    let entity_type = request.entity_type.as_str();
    match entity_type {
        "session" => explain_session(&connection, &request.entity_id),
        "search_trail" => explain_search_trail(&connection, &request.entity_id),
        "query_family" => explain_query_family(&connection, &request.entity_id),
        "refind_page" => {
            let explanation = super::intelligence_refind::build_refind_explanation(
                &connection,
                &request.entity_id,
            )?;
            Ok(Explanation {
                entity_type: "refind_page".to_string(),
                entity_id: explanation.canonical_url.clone(),
                trigger_rule: format!("Refind score >= {:.1}", explanation.refind_score),
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
            })
        }
        "reopened_investigation" => explain_reopened_investigation(&connection, &request.entity_id),
        "habit_pattern" => explain_habit_pattern(&connection, &request.entity_id),
        "path_flow" => explain_path_flow(&connection, &request.entity_id),
        "compare_set" => explain_compare_set(&connection, &request.entity_id),
        _ => anyhow::bail!(
            "Core Intelligence explainability does not support entity type '{entity_type}'."
        ),
    }
}

/// Explains why a set of archive visits was grouped into one session and
/// whether that session crossed the deep-dive thresholds.
fn explain_session(connection: &Connection, session_id: &str) -> Result<Explanation> {
    let (visit_count, search_count, domain_count, is_deep_dive, first_visit_ms, last_visit_ms) =
        connection
            .query_row(
                "SELECT visit_count, search_count, domain_count, is_deep_dive, first_visit_ms, last_visit_ms
                 FROM sessions
                 WHERE session_id = ?1",
                [session_id],
                |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, i64>(2)?,
                        row.get::<_, i64>(3)? != 0,
                        row.get::<_, i64>(4)?,
                        row.get::<_, i64>(5)?,
                    ))
                },
            )
            .optional()?
            .with_context(|| format!("session {session_id} was not found"))?;
    let visit_ids = connection
        .prepare(
            "SELECT visit_id
             FROM visit_derived_facts
             WHERE session_id = ?1
             ORDER BY visit_id ASC",
        )?
        .query_map([session_id], |row| row.get::<_, i64>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let navigation_chain_depth = connection
        .query_row(
            "SELECT COUNT(*)
             FROM visit_derived_facts
             JOIN archive.visits AS visits ON visits.id = visit_derived_facts.visit_id
             WHERE visit_derived_facts.session_id = ?1
               AND visits.from_visit IS NOT NULL",
            [session_id],
            |row| row.get::<_, i64>(0),
        )
        .unwrap_or(0);
    let duration_minutes = ((last_visit_ms - first_visit_ms).max(0) as f32) / 60_000.0;
    Ok(Explanation {
        entity_type: "session".to_string(),
        entity_id: session_id.to_string(),
        trigger_rule: if is_deep_dive {
            "Deep dive session matched the navigation-depth, domain-count, and visit-count thresholds."
                .to_string()
        } else {
            "Visits were grouped into one session because adjacent gaps stayed within 30 minutes."
                .to_string()
        },
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
        participating_visit_ids: visit_ids,
    })
}

/// Explains why a search trail exists by surfacing its anchor query, trail
/// depth, and the exact visits captured in the trail membership table.
fn explain_search_trail(connection: &Connection, trail_id: &str) -> Result<Explanation> {
    let (initial_query, reformulation_count, visit_count, max_depth, landing_domain) = connection
        .query_row(
            "SELECT initial_query, reformulation_count, visit_count, max_depth, landing_domain
             FROM search_trails
             WHERE trail_id = ?1",
            [trail_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, Option<String>>(4)?,
                ))
            },
        )
        .optional()?
        .with_context(|| format!("search trail {trail_id} was not found"))?;
    let visit_ids = connection
        .prepare(
            "SELECT visit_id
             FROM search_trail_members
             WHERE trail_id = ?1
             ORDER BY ordinal ASC, visit_id ASC",
        )?
        .query_map([trail_id], |row| row.get::<_, i64>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let landing_score = if landing_domain.is_some() { 1.0 } else { 0.0 };
    Ok(Explanation {
        entity_type: "search_trail".to_string(),
        entity_id: trail_id.to_string(),
        trigger_rule: format!(
            "Search trail anchored by '{}' and extended through navigation ancestry within the session window.",
            initial_query
        ),
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
        participating_visit_ids: visit_ids,
    })
}

/// Explains why multiple related queries collapsed into one family and returns
/// the concrete search visits that back that family.
fn explain_query_family(connection: &Connection, family_id: &str) -> Result<Explanation> {
    let (profile_id, anchor_query, member_count, search_engine, queries_json) = connection
        .query_row(
            "SELECT profile_id, anchor_query, member_count, search_engine, queries_json
             FROM query_families
             WHERE family_id = ?1",
            [family_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                ))
            },
        )
        .optional()?
        .with_context(|| format!("query family {family_id} was not found"))?;
    let queries = serde_json::from_str::<Vec<String>>(&queries_json).unwrap_or_default();
    let normalized_queries =
        queries.iter().map(|query| normalize_query(query)).collect::<HashSet<_>>();
    let visit_ids = load_query_family_visit_ids(connection, &profile_id, &search_engine, &queries)?;
    Ok(Explanation {
        entity_type: "query_family".to_string(),
        entity_id: family_id.to_string(),
        trigger_rule: format!(
            "Queries were merged into one family because their Jaccard or containment similarity matched '{}'.",
            anchor_query
        ),
        factors: vec![
            explainability_factor("member_count", member_count as f32, 1.0, member_count as f32),
            explainability_factor(
                "distinct_query_count",
                normalized_queries.len() as f32,
                0.7,
                normalized_queries.len() as f32 * 0.7,
            ),
        ],
        participating_visit_ids: visit_ids,
    })
}

/// Explains why a deterministic reopened-investigation row fired, including the
/// repeated anchor evidence that pushed it over the threshold.
fn explain_reopened_investigation(
    connection: &Connection,
    investigation_id: &str,
) -> Result<Explanation> {
    let (anchor_type, anchor_id, occurrence_count, distinct_days, evidence_json) = connection
        .query_row(
            "SELECT anchor_type, anchor_id, occurrence_count, distinct_days, evidence_json
             FROM reopened_investigations
             WHERE investigation_id = ?1",
            [investigation_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, String>(4)?,
                ))
            },
        )
        .optional()?
        .with_context(|| format!("reopened investigation {investigation_id} was not found"))?;
    let participating_visit_ids = match anchor_type.as_str() {
        "query_family" => {
            let (profile_id, search_engine, queries_json) = connection.query_row(
                "SELECT profile_id, search_engine, queries_json
                 FROM query_families
                 WHERE family_id = ?1",
                [anchor_id.as_str()],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                    ))
                },
            )?;
            let queries = serde_json::from_str::<Vec<String>>(&queries_json).unwrap_or_default();
            load_query_family_visit_ids(connection, &profile_id, &search_engine, &queries)?
        }
        "reference_page" => extract_visit_ids_from_evidence_json(&evidence_json),
        _ => Vec::new(),
    };
    Ok(Explanation {
        entity_type: "reopened_investigation".to_string(),
        entity_id: investigation_id.to_string(),
        trigger_rule: "This investigation reopened because the same anchor reappeared across distinct days or repeated deterministic evidence."
            .to_string(),
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
    })
}

/// Explains the cadence evidence behind one habit-pattern row so the UI can
/// describe why a domain looks recurring instead of incidental.
fn explain_habit_pattern(connection: &Connection, entity_id: &str) -> Result<Explanation> {
    let (profile_id, registrable_domain) = parse_two_part_entity_id(entity_id, "habit_pattern")?;
    let (habit_type, mean_interval_days, cv, visit_count, is_interrupted) = connection
        .query_row(
            "SELECT habit_type, mean_interval_days, cv, visit_count, is_interrupted
             FROM habit_patterns
             WHERE profile_id = ?1
               AND registrable_domain = ?2",
            params![profile_id, registrable_domain],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, f32>(1)?,
                    row.get::<_, f32>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, i64>(4)? != 0,
                ))
            },
        )
        .optional()?
        .with_context(|| format!("habit pattern {entity_id} was not found"))?;
    let visit_ids = load_domain_visit_ids(connection, profile_id, registrable_domain)?;
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
        participating_visit_ids: visit_ids,
    })
}

/// Explains the latest sequence of visits that matched a persisted path-flow
/// pattern without rebuilding path-flow state on demand.
fn explain_path_flow(connection: &Connection, entity_id: &str) -> Result<Explanation> {
    let (profile_id, step_count, flow_pattern) = parse_path_flow_entity_id(entity_id)?;
    let (occurrence_count, _) = connection
        .query_row(
            "SELECT occurrence_count, last_seen_ms
             FROM path_flows
             WHERE profile_id = ?1
               AND step_count = ?2
               AND flow_pattern = ?3",
            params![profile_id, step_count, flow_pattern],
            |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)),
        )
        .optional()?
        .with_context(|| format!("path flow {entity_id} was not found"))?;
    let participating_visit_ids =
        load_recent_path_flow_visit_ids(connection, profile_id, step_count, flow_pattern)?;
    Ok(Explanation {
        entity_type: "path_flow".to_string(),
        entity_id: entity_id.to_string(),
        trigger_rule: "This flow pattern recurs across session-local domain n-grams.".to_string(),
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
    })
}

/// Explains why a compare-set candidate exists by surfacing alternating pages
/// inside one trail and the visit ids that participated in the comparison.
fn explain_compare_set(connection: &Connection, entity_id: &str) -> Result<Explanation> {
    let (trail_id, page_category) = parse_compare_set_entity_id(entity_id)?;
    let mut statement = connection.prepare(
        "SELECT search_trail_members.visit_id,
                visit_derived_facts.canonical_url,
                visit_derived_facts.registrable_domain
         FROM search_trail_members
         JOIN visit_derived_facts ON visit_derived_facts.visit_id = search_trail_members.visit_id
         WHERE search_trail_members.trail_id = ?1
           AND visit_derived_facts.page_category = ?2
         ORDER BY search_trail_members.ordinal ASC, search_trail_members.visit_id ASC",
    )?;
    let members = statement
        .query_map(params![trail_id, page_category], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    if members.is_empty() {
        anyhow::bail!("compare set {entity_id} was not found");
    }

    let mut canonical_urls = BTreeSet::new();
    let mut domains = BTreeSet::new();
    let mut alternations = 0_i64;
    let mut previous_url = None::<String>;
    let participating_visit_ids = members
        .iter()
        .map(|(visit_id, canonical_url, registrable_domain)| {
            canonical_urls.insert(canonical_url.clone());
            domains.insert(registrable_domain.clone());
            if previous_url.as_ref().is_some_and(|previous| previous != canonical_url) {
                alternations += 1;
            }
            previous_url = Some(canonical_url.clone());
            *visit_id
        })
        .collect::<Vec<_>>();

    Ok(Explanation {
        entity_type: "compare_set".to_string(),
        entity_id: entity_id.to_string(),
        trigger_rule:
            "This compare set alternated between multiple comparable pages within one search trail."
                .to_string(),
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
        participating_visit_ids,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seeded_explanation_connection() -> Connection {
        let connection = Connection::open_in_memory().expect("connection");
        ensure_core_intelligence_schema(&connection).expect("schema");
        connection
            .execute_batch(
                "
                ATTACH DATABASE ':memory:' AS archive;
                CREATE TABLE archive.visits (
                  id INTEGER PRIMARY KEY,
                  from_visit INTEGER,
                  visit_time_ms INTEGER NOT NULL
                );
                INSERT INTO archive.visits (id, from_visit, visit_time_ms) VALUES
                  (1, NULL, 1000),
                  (2, 1, 2000),
                  (3, 2, 3000),
                  (4, 3, 4000),
                  (5, NULL, 5000);

                INSERT INTO sessions
                  (session_id, profile_id, first_visit_ms, last_visit_ms, visit_count,
                   search_count, domain_count, is_deep_dive, auto_title, computed_at)
                VALUES
                  ('session-1', 'p1', 1000, 301000, 4, 2, 3, 1, 'Deep research', 'now'),
                  ('session-2', 'p1', 5000, 65000, 1, 0, 1, 0, 'Quick read', 'now');

                INSERT INTO visit_derived_facts
                  (visit_id, profile_id, session_id, trail_id, registrable_domain, canonical_url,
                   domain_category, page_category, search_engine, search_query, is_new_domain,
                   is_search_event, evidence_tier, taxonomy_source, computed_at)
                VALUES
                  (1, 'p1', 'session-1', 'trail-1', 'search.com', 'https://search.com/?q=pathkeep',
                   'search', 'search', 'google', 'pathkeep coverage', 1, 1, 'tier-a', 'fixture', 'now'),
                  (2, 'p1', 'session-1', 'trail-1', 'docs.com', 'https://docs.com/a',
                   'reference', 'article', NULL, NULL, 1, 0, 'tier-b', 'fixture', 'now'),
                  (3, 'p1', 'session-1', 'trail-1', 'alt.com', 'https://alt.com/b',
                   'reference', 'article', NULL, NULL, 1, 0, 'tier-b', 'fixture', 'now'),
                  (4, 'p1', 'session-1', NULL, 'docs.com', 'https://docs.com/c',
                   'reference', 'article', NULL, NULL, 0, 0, 'tier-c', 'fixture', 'now'),
                  (5, 'p1', 'session-2', NULL, 'quiet.example', 'https://quiet.example/',
                   'reference', 'article', NULL, NULL, 0, 0, 'tier-c', 'fixture', 'now');

                INSERT INTO search_trails
                  (trail_id, profile_id, session_id, initial_query, search_engine,
                   reformulation_count, visit_count, landing_url, landing_domain,
                   first_visit_ms, last_visit_ms, max_depth, queries_json, computed_at)
                VALUES
                  ('trail-1', 'p1', 'session-1', 'pathkeep coverage', 'google',
                   1, 3, 'https://docs.com/a', 'docs.com', 1000, 3000, 2,
                   '[\"pathkeep coverage\", \"pathkeep docs\"]', 'now');

                INSERT INTO search_trail_members (trail_id, profile_id, visit_id, ordinal, role)
                VALUES
                  ('trail-1', 'p1', 1, 0, 'search'),
                  ('trail-1', 'p1', 2, 1, 'landing'),
                  ('trail-1', 'p1', 3, 2, 'comparison');

                INSERT INTO search_events
                  (visit_id, profile_id, search_engine, raw_query, normalized_query,
                   query_kind, trail_id, computed_at)
                VALUES
                  (1, 'p1', 'google', 'PathKeep coverage', 'pathkeep coverage', 'keyword', 'trail-1', 'now'),
                  (4, 'p1', 'google', 'PathKeep docs', 'pathkeep docs', 'keyword', NULL, 'now');

                INSERT INTO query_families
                  (family_id, profile_id, anchor_query, member_count, search_engine,
                   first_seen_ms, last_seen_ms, queries_json, computed_at)
                VALUES
                  ('family-1', 'p1', 'pathkeep coverage', 2, 'google', 1000, 4000,
                   '[\"pathkeep coverage\", \"pathkeep docs\"]', 'now');

                INSERT INTO habit_patterns
                  (profile_id, registrable_domain, habit_type, mean_interval_days, cv,
                   visit_count, last_visited_ms, is_interrupted, computed_at)
                VALUES
                  ('p1', 'docs.com', 'weekly', 7.0, 0.25, 5, 4000, 1, 'now'),
                  ('p1', 'quiet.example', 'daily', 1.0, 0.1, 3, 5000, 0, 'now');

                INSERT INTO reopened_investigations
                  (investigation_id, profile_id, anchor_type, anchor_id, anchor_label,
                   occurrence_count, distinct_days, first_seen_ms, last_seen_ms, evidence_json, computed_at)
                VALUES
                  ('reopened-query', 'p1', 'query_family', 'family-1', 'pathkeep coverage',
                   2, 2, 1000, 4000, '{\"visitIds\":[1,4]}', 'now'),
                  ('reopened-reference', 'p1', 'reference_page', 'https://docs.com/a', 'Docs',
                   2, 2, 1000, 4000, '{\"visitIds\":[2,3,\"skip\"]}', 'now'),
                  ('reopened-other', 'p1', 'other', 'other', 'Other',
                   1, 1, 1000, 1000, '{}', 'now');

                INSERT INTO path_flows
                  (profile_id, flow_pattern, step_count, occurrence_count, last_seen_ms)
                VALUES ('p1', 'search.com → docs.com', 2, 2, 4000);
                ",
            )
            .expect("seed explanation tables");
        connection
    }

    #[test]
    fn explanation_builders_cover_all_supported_entity_shapes() {
        let connection = seeded_explanation_connection();

        let deep_session = explain_session(&connection, "session-1").expect("deep session");
        assert!(deep_session.trigger_rule.contains("Deep dive"));
        assert_eq!(deep_session.participating_visit_ids, vec![1, 2, 3, 4]);

        let shallow_session = explain_session(&connection, "session-2").expect("shallow session");
        assert!(shallow_session.trigger_rule.contains("30 minutes"));

        let trail = explain_search_trail(&connection, "trail-1").expect("trail explanation");
        assert_eq!(trail.participating_visit_ids, vec![1, 2, 3]);
        assert!(trail.factors.iter().any(|factor| factor.label == "landing_detected"));

        let family = explain_query_family(&connection, "family-1").expect("family explanation");
        assert_eq!(family.participating_visit_ids, vec![1, 4]);

        let reopened_query =
            explain_reopened_investigation(&connection, "reopened-query").expect("reopened query");
        assert_eq!(reopened_query.participating_visit_ids, vec![1, 4]);
        let reopened_reference = explain_reopened_investigation(&connection, "reopened-reference")
            .expect("reopened reference");
        assert_eq!(reopened_reference.participating_visit_ids, vec![2, 3]);
        let reopened_other =
            explain_reopened_investigation(&connection, "reopened-other").expect("reopened other");
        assert!(reopened_other.participating_visit_ids.is_empty());

        let interrupted =
            explain_habit_pattern(&connection, "p1::docs.com").expect("habit explanation");
        assert!(interrupted.trigger_rule.contains("interruption"));
        let active = explain_habit_pattern(&connection, "p1::quiet.example").expect("active habit");
        assert!(active.trigger_rule.contains("cross-day"));

        let path_flow =
            explain_path_flow(&connection, "p1::2::search.com → docs.com").expect("path flow");
        assert_eq!(path_flow.participating_visit_ids, vec![1, 2]);

        let compare =
            explain_compare_set(&connection, "compare:trail-1:article").expect("compare set");
        assert_eq!(compare.participating_visit_ids, vec![2, 3]);
        assert!(compare.factors.iter().any(|factor| factor.label == "alternation_count"));
        assert!(explain_compare_set(&connection, "compare:trail-1:missing").is_err());
    }

    #[test]
    fn explanation_builders_report_missing_or_malformed_entity_ids() {
        let connection = seeded_explanation_connection();

        assert!(explain_session(&connection, "missing").is_err());
        assert!(explain_search_trail(&connection, "missing").is_err());
        assert!(explain_query_family(&connection, "missing").is_err());
        assert!(explain_reopened_investigation(&connection, "missing").is_err());
        assert!(explain_habit_pattern(&connection, "malformed").is_err());
        assert!(explain_path_flow(&connection, "p1::two::docs.com").is_err());
        assert!(explain_path_flow(&connection, "p1::2::").is_err());
    }
}
