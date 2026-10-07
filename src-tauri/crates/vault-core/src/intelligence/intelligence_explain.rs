//! Failure modes: reads write schemas, a query escapes its owner, stale scopes leak,
//! or bounded results lose continuation. Native render-path and paging tests cover these.
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
    intelligence_explain_helpers::{
        explainability_factor, load_domain_visit_ids, load_membership_visit_ids,
        load_query_family_visit_ids, load_recent_path_flow_visit_ids, parse_compare_set_entity_id,
        parse_path_flow_entity_id, parse_two_part_entity_id,
    },
    normalize_query,
};
use crate::models::{EntityExplanationRequest, ExplainabilityFactor, Explanation};
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
               AND visits.from_visit IS NOT NULL",
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
    let (habit_type, mean_interval_days, cv, visit_count, is_interrupted) = connection
        .query_one(
            "SELECT habit_type, mean_interval_days, cv, visit_count, is_interrupted
             FROM habit_patterns
             WHERE profile_id = ?1
               AND registrable_domain = ?2",
            params![profile_id, registrable_domain],
            |row| {
                Ok((
                    row.get::<String>(0)?,
                    row.get::<f64>(1)? as f32,
                    row.get::<f64>(2)? as f32,
                    row.get::<i64>(3)?,
                    row.get::<i64>(4)? != 0,
                ))
            },
        )
        .optional()?
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::native::TestDatabase;
    use vault_store::{Db, Priority, params_owned as params};

    fn seeded_explanation_connection() -> TestDatabase {
        let vault = crate::Vault::open_temp().unwrap();
        vault.write(Db::Archive,Priority::Batch,|s| {
            s.execute("INSERT INTO source_profiles(id,browser_kind,profile_name,profile_path,profile_key,discovered_at) VALUES(1,'chrome','Default','/fixture','p1','')",())?;
            for (id,url,domain) in [(1_i64,"https://search.com/?q=pathkeep","search.com"),(2,"https://docs.com/a","docs.com"),(3,"https://alt.com/b","alt.com"),(4,"https://docs.com/c","docs.com"),(5,"https://quiet.example/","quiet.example")] {
                s.execute("INSERT INTO pages(id,url,canonical_url,registrable_domain) VALUES(?1,?2,?2,?3)",params![id,url,domain])?;
                s.execute("INSERT INTO urls(id,page_id,url,title,source_profile_id,created_by_run_id,first_visit_ms,first_visit_iso,last_visit_ms,last_visit_iso) VALUES(?1,?1,?2,'Fixture',1,0,?3,'',?3,'')",params![id,url,id*1000])?;
                s.execute("INSERT INTO visits(id,url_id,source_profile_id,created_by_run_id,visit_time_ms,visit_time_iso,from_visit) VALUES(?1,?1,1,0,?2,'',?3)",params![id,id*1000,if id==1||id==5 {None}else{Some(id-1)}])?;
            }
            s.execute("INSERT INTO visits(id,url_id,source_profile_id,created_by_run_id,visit_time_ms,visit_time_iso) VALUES(6,2,1,0,6000,'')",())?;
            Ok(())
        }).unwrap();
        let connection = TestDatabase::new(&vault, Db::Intelligence);
        connection
            .execute("INSERT INTO intelligence_generations(profile_id) VALUES('p1')", ())
            .unwrap();
        connection.execute_batch("
                INSERT INTO sessions_rows
                  (session_id, profile_id, first_visit_ms, last_visit_ms, visit_count,
                   search_count, domain_count, is_deep_dive, auto_title, computed_at)
                VALUES
                  ('session-1', 'p1', 1000, 301000, 4, 2, 3, 1, 'Deep research', 'now'),
                  ('session-2', 'p1', 5000, 65000, 1, 0, 1, 0, 'Quick read', 'now');

INSERT INTO visit_derived_facts_rows(visit_id,profile_id,registrable_domain,canonical_url,domain_category,page_category,search_engine,search_query,is_new_domain,is_search_event,evidence_tier,taxonomy_source,computed_at) VALUES (1,'p1','search.com','https://search.com/?q=pathkeep','search','search','google','pathkeep coverage',1,1,'tier-a','fixture','now'),(2,'p1','docs.com','https://docs.com/a','reference','article',NULL,NULL,1,0,'tier-b','fixture','now'),(3,'p1','alt.com','https://alt.com/b','reference','article',NULL,NULL,1,0,'tier-b','fixture','now'),(4,'p1','docs.com','https://docs.com/c','reference','article',NULL,NULL,0,0,'tier-c','fixture','now'),(5,'p1','quiet.example','https://quiet.example/','reference','article',NULL,NULL,0,0,'tier-c','fixture','now');
INSERT INTO visit_derived_facts_rows(visit_id,profile_id,registrable_domain,canonical_url,computed_at) VALUES(6,'p1','docs.com','https://docs.com/a','');
INSERT INTO visit_assignments(visit_id,profile_id,session_id,trail_id) VALUES (1,'p1','session-1','trail-1'),(2,'p1','session-1','trail-1'),(3,'p1','session-1','trail-1'),(4,'p1','session-1',NULL),(5,'p1','session-2',NULL);

                INSERT INTO search_trails_rows
                  (trail_id, profile_id, session_id, initial_query, search_engine,
                   reformulation_count, visit_count, landing_url, landing_domain,
                   first_visit_ms, last_visit_ms, max_depth, queries_json, computed_at)
                VALUES
                  ('trail-1', 'p1', 'session-1', 'pathkeep coverage', 'google',
                   1, 3, 'https://docs.com/a', 'docs.com', 1000, 3000, 2,
                   '[\"pathkeep coverage\", \"pathkeep docs\"]', 'now');

                INSERT INTO search_trail_members_rows (trail_id, profile_id, visit_id, ordinal, role)
                VALUES
                  ('trail-1', 'p1', 1, 0, 'search'),
                  ('trail-1', 'p1', 2, 1, 'landing'),
                  ('trail-1', 'p1', 3, 2, 'comparison');

                INSERT INTO search_events_rows
                  (visit_id, profile_id, search_engine, raw_query, normalized_query,
                   query_kind, trail_id, computed_at)
                VALUES
                  (1, 'p1', 'google', 'PathKeep coverage', 'pathkeep coverage', 'keyword', 'trail-1', 'now'),
                  (4, 'p1', 'google', 'PathKeep docs', 'pathkeep docs', 'keyword', NULL, 'now');

                INSERT INTO query_families_rows
                  (family_id, profile_id, anchor_query, member_count, search_engine,
                   first_seen_ms, last_seen_ms, queries_json, computed_at)
                VALUES
                  ('family-1', 'p1', 'pathkeep coverage', 2, 'google', 1000, 4000,
                   '[\"pathkeep coverage\", \"pathkeep docs\"]', 'now');

                INSERT INTO habit_patterns_rows(profile_id,registrable_domain,habit_type,mean_interval_days,cv,visit_count,last_visited_ms,is_interrupted,computed_at) VALUES ('p1','docs.com','weekly',7.0,0.25,5,4000,1,'now'),
('p1','quiet.example','daily',1.0,0.1,3,5000,0,'now');

                INSERT INTO refind_pages_rows(profile_id,canonical_url,url,title,registrable_domain,cross_day_count,trail_count,search_arrival_count,typed_revisit_count,refind_score,evidence_json,first_seen_ms,last_seen_ms,computed_at)
                VALUES('p1','https://docs.com/a','https://docs.com/a','Docs','docs.com',2,2,1,0,1,'{\"visitIds\":[2,3,\"skip\"]}',1000,4000,'');

                INSERT INTO reopened_investigations_rows
                  (investigation_id, profile_id, anchor_type, anchor_id, anchor_label,
                   occurrence_count, distinct_days, first_seen_ms, last_seen_ms, evidence_json, computed_at)
                VALUES
                  ('reopened-query', 'p1', 'query_family', 'family-1', 'pathkeep coverage',
                   2, 2, 1000, 4000, '{\"visitIds\":[1,4]}', 'now'),
                  ('reopened-reference', 'p1', 'reference_page', 'https://docs.com/a', 'Docs',
                   2, 2, 1000, 4000, '{\"visitIds\":[2,3,\"skip\"]}', 'now'),
                  ('reopened-other', 'p1', 'other', 'other', 'Other',
                   1, 1, 1000, 1000, '{}', 'now');

                INSERT INTO path_flows_rows
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

        let deep_session =
            { connection.read(move |session| explain_session(session, "session-1", None, 50)) }
                .expect("deep session");
        assert!(deep_session.trigger_rule.contains("Deep dive"));
        assert_eq!(deep_session.participating_visit_ids, vec![1, 2, 3, 4]);
        // The stable CODE is what the localized panel renders from; the deep-dive rule names no
        // threshold, so it must NOT carry the gap param.
        assert_eq!(
            deep_session.trigger_rule_code.as_deref(),
            Some(Explanation::TRIGGER_RULE_SESSION_DEEP_DIVE)
        );
        assert_eq!(deep_session.trigger_rule_gap_minutes, None);

        let shallow_session =
            { connection.read(move |session| explain_session(session, "session-2", None, 50)) }
                .expect("shallow session");
        assert!(shallow_session.trigger_rule.contains("30 minutes"));
        assert_eq!(
            shallow_session.trigger_rule_code.as_deref(),
            Some(Explanation::TRIGGER_RULE_SESSION_GAP)
        );
        // The gap threshold travels as a NUMBER derived from `SESSION_GAP_MS`, so the copy never
        // hard-codes a value the grouper could change out from under it.
        assert_eq!(
            shallow_session.trigger_rule_gap_minutes,
            Some(super::super::SESSION_GAP_MS / 60_000)
        );

        let trail =
            { connection.read(move |session| explain_search_trail(session, "trail-1", None, 50)) }
                .expect("trail explanation");
        assert_eq!(trail.participating_visit_ids, vec![1, 2, 3]);
        assert!(trail.factors.iter().any(|factor| factor.label == "landing_detected"));
        assert_eq!(
            trail.trigger_rule_code.as_deref(),
            Some(Explanation::TRIGGER_RULE_SEARCH_TRAIL)
        );
        // The anchor query travels structurally, so the panel never re-parses quote characters.
        assert_eq!(trail.trigger_rule_query.as_deref(), Some("pathkeep coverage"));

        let family =
            { connection.read(move |session| explain_query_family(session, "family-1", None, 50)) }
                .expect("family explanation");
        assert_eq!(family.participating_visit_ids, vec![1, 4]);
        assert_eq!(
            family.trigger_rule_code.as_deref(),
            Some(Explanation::TRIGGER_RULE_QUERY_FAMILY)
        );
        assert_eq!(family.trigger_rule_query.as_deref(), Some("pathkeep coverage"));

        let reopened_query = {
            connection.read(move |session| {
                explain_reopened_investigation(session, "reopened-query", None, 50)
            })
        }
        .expect("reopened query");
        assert_eq!(reopened_query.participating_visit_ids, vec![1, 4]);
        let reopened_reference = {
            connection.read(move |session| {
                explain_reopened_investigation(session, "reopened-reference", None, 50)
            })
        }
        .expect("reopened reference");
        assert_eq!(reopened_reference.participating_visit_ids, vec![2, 6]);
        let reopened_other = {
            connection.read(move |session| {
                explain_reopened_investigation(session, "reopened-other", None, 50)
            })
        }
        .expect("reopened other");
        assert!(reopened_other.participating_visit_ids.is_empty());
        // Every reopened-investigation shape shares one parameterless rule code.
        for reopened in [&reopened_query, &reopened_reference, &reopened_other] {
            assert_eq!(
                reopened.trigger_rule_code.as_deref(),
                Some(Explanation::TRIGGER_RULE_REOPENED_INVESTIGATION)
            );
        }

        let interrupted = {
            connection.read(move |session| explain_habit_pattern(session, "p1::docs.com", None, 50))
        }
        .expect("habit explanation");
        assert!(interrupted.trigger_rule.contains("interruption"));
        // The two habit variants are DISTINCT codes, and the habit TYPE travels as its persisted code
        // so the front end never has to capture the name out of the sentence.
        assert_eq!(
            interrupted.trigger_rule_code.as_deref(),
            Some(Explanation::TRIGGER_RULE_HABIT_PATTERN_INTERRUPTED)
        );
        assert_eq!(interrupted.trigger_rule_habit_type.as_deref(), Some("weekly"));
        let active = {
            connection
                .read(move |session| explain_habit_pattern(session, "p1::quiet.example", None, 50))
        }
        .expect("active habit");
        assert!(active.trigger_rule.contains("cross-day"));
        assert_eq!(
            active.trigger_rule_code.as_deref(),
            Some(Explanation::TRIGGER_RULE_HABIT_PATTERN)
        );
        assert_eq!(active.trigger_rule_habit_type.as_deref(), Some("daily"));

        let path_flow = {
            connection
                .read(move |session| explain_path_flow(session, "p1::2::search.com → docs.com"))
        }
        .expect("path flow");
        assert_eq!(path_flow.participating_visit_ids, vec![1, 2]);
        assert_eq!(
            path_flow.trigger_rule_code.as_deref(),
            Some(Explanation::TRIGGER_RULE_PATH_FLOW)
        );

        let compare = {
            connection.read(move |session| {
                explain_compare_set(session, "compare:trail-1:article", None, 50)
            })
        }
        .expect("compare set");
        assert_eq!(compare.participating_visit_ids, vec![2, 3]);
        assert_eq!(
            compare.trigger_rule_code.as_deref(),
            Some(Explanation::TRIGGER_RULE_COMPARE_SET)
        );
        assert!(compare.factors.iter().any(|factor| factor.label == "alternation_count"));
        assert!(
            {
                connection.read(move |session| {
                    explain_compare_set(session, "compare:trail-1:missing", None, 50)
                })
            }
            .is_err()
        );
    }

    #[test]
    fn explanation_builders_report_missing_or_malformed_entity_ids() {
        let connection = seeded_explanation_connection();

        assert!(
            { connection.read(move |session| explain_session(session, "missing", None, 50)) }
                .is_err()
        );
        assert!(
            { connection.read(move |session| explain_search_trail(session, "missing", None, 50)) }
                .is_err()
        );
        assert!(
            { connection.read(move |session| explain_query_family(session, "missing", None, 50)) }
                .is_err()
        );
        assert!(
            {
                connection.read(move |session| {
                    explain_reopened_investigation(session, "missing", None, 50)
                })
            }
            .is_err()
        );
        assert!(
            {
                connection
                    .read(move |session| explain_habit_pattern(session, "malformed", None, 50))
            }
            .is_err()
        );
        assert!(
            { connection.read(move |session| explain_path_flow(session, "p1::two::docs.com")) }
                .is_err()
        );
        assert!({ connection.read(move |session| explain_path_flow(session, "p1::2::")) }.is_err());
    }
}
