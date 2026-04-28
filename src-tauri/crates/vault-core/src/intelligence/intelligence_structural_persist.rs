//! Structural rebuild write-side persistence helpers for Core Intelligence.
//!
//! ## Responsibilities
//! - Scope dirty-window cleanup before streamed structural replay starts.
//! - Replace profile-scoped query-family and structural aggregate tables after a
//!   rebuild finishes.
//! - Keep transaction-heavy write orchestration out of stage planning and
//!   stream-state code.
//!
//! ## Not responsible for
//! - Deciding when a structural rebuild should run.
//! - Streaming visits or maintaining in-progress session/trail state.
//! - Building refind, habit, or query-family records.
//!
//! ## Dependencies
//! - Parent-module structural record types and aggregate row types.
//! - SQLite transaction helpers and the derived intelligence tables.
//!
//! ## Performance notes
//! - Dirty-range deletes stay profile-scoped and bounded by `start_ms` when an
//!   incremental replay is possible.
//! - Replacement helpers batch all writes inside one transaction so rebuild
//!   callers do not pay per-row commit costs.

use super::{
    PathFlowRecord, QueryFamilyRecord, RefindPageRecord, ReopenedInvestigationRecord,
    SourceEffectivenessRecord,
};
use anyhow::Result;
use rusqlite::{Connection, params};

/// Deletes only the structural rows that overlap the dirty replay window
/// before streamed replay starts writing replacements.
pub(super) fn clear_structural_tail_state(
    tx: &rusqlite::Transaction<'_>,
    profile_id: &str,
    start_ms: Option<i64>,
) -> Result<()> {
    if let Some(start_ms) = start_ms {
        tx.execute(
            "DELETE FROM sessions WHERE profile_id = ?1 AND last_visit_ms >= ?2",
            params![profile_id, start_ms],
        )?;
        tx.execute(
            "DELETE FROM search_trails WHERE profile_id = ?1 AND last_visit_ms >= ?2",
            params![profile_id, start_ms],
        )?;
        delete_structural_memberships_in_range(tx, profile_id, start_ms)?;
    } else {
        tx.execute("DELETE FROM sessions WHERE profile_id = ?1", [profile_id])?;
        tx.execute("DELETE FROM search_trails WHERE profile_id = ?1", [profile_id])?;
        tx.execute("DELETE FROM search_trail_members WHERE profile_id = ?1", [profile_id])?;
        tx.execute("DELETE FROM search_event_terms WHERE profile_id = ?1", [profile_id])?;
        tx.execute("DELETE FROM search_events WHERE profile_id = ?1", [profile_id])?;
    }
    Ok(())
}

/// Replaces all query-family rows for one profile after the rebuild has
/// computed the new deterministic families.
pub(super) fn replace_query_families(
    connection: &Connection,
    profile_id: &str,
    query_families: &[QueryFamilyRecord],
    computed_at: &str,
) -> Result<()> {
    let tx = connection.unchecked_transaction()?;
    tx.execute("DELETE FROM query_families WHERE profile_id = ?1", [profile_id])?;
    for family in query_families {
        tx.execute(
            "INSERT INTO query_families
             (family_id, profile_id, anchor_query, member_count, search_engine, first_seen_ms, last_seen_ms, queries_json, computed_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                family.family_id,
                family.profile_id,
                family.anchor_query,
                family.member_count,
                family.search_engine,
                family.first_seen_ms,
                family.last_seen_ms,
                serde_json::to_string(&family.queries)?,
                computed_at,
            ],
        )?;
    }
    tx.commit()?;
    Ok(())
}

/// Replaces all profile-scoped structural aggregate tables once a rebuild has
/// produced the fresh aggregate rows.
pub(super) fn replace_structural_profile_aggregates(
    connection: &Connection,
    profile_id: &str,
    refind_pages: &[RefindPageRecord],
    source_effectiveness: &[SourceEffectivenessRecord],
    habits: &[super::HabitPatternRecord],
    reopened: &[ReopenedInvestigationRecord],
    path_flows: &[PathFlowRecord],
    computed_at: &str,
) -> Result<()> {
    let tx = connection.unchecked_transaction()?;
    tx.execute("DELETE FROM refind_pages WHERE profile_id = ?1", [profile_id])?;
    tx.execute("DELETE FROM source_effectiveness WHERE profile_id = ?1", [profile_id])?;
    tx.execute("DELETE FROM habit_patterns WHERE profile_id = ?1", [profile_id])?;
    tx.execute("DELETE FROM reopened_investigations WHERE profile_id = ?1", [profile_id])?;
    tx.execute("DELETE FROM path_flows WHERE profile_id = ?1", [profile_id])?;

    for page in refind_pages {
        tx.execute(
            "INSERT INTO refind_pages
             (profile_id, canonical_url, url, title, registrable_domain, cross_day_count, trail_count, search_arrival_count,
              typed_revisit_count, refind_score, evidence_json, first_seen_ms, last_seen_ms, computed_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
            params![
                page.profile_id,
                page.canonical_url,
                page.url,
                page.title,
                page.registrable_domain,
                page.cross_day_count,
                page.trail_count,
                page.search_arrival_count,
                page.typed_revisit_count,
                page.refind_score,
                page.evidence_json,
                page.first_seen_ms,
                page.last_seen_ms,
                computed_at,
            ],
        )?;
    }
    for source in source_effectiveness {
        tx.execute(
            "INSERT INTO source_effectiveness
             (profile_id, registrable_domain, source_role, trail_count, stable_landing_count, effectiveness_score,
              evidence_json, first_seen_ms, last_seen_ms, computed_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                source.profile_id,
                source.registrable_domain,
                source.source_role,
                source.trail_count,
                source.stable_landing_count,
                source.effectiveness_score,
                source.evidence_json,
                source.first_seen_ms,
                source.last_seen_ms,
                computed_at,
            ],
        )?;
    }
    for habit in habits {
        tx.execute(
            "INSERT INTO habit_patterns
             (profile_id, registrable_domain, habit_type, mean_interval_days, cv, visit_count, last_visited_ms, is_interrupted, computed_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                habit.profile_id,
                habit.registrable_domain,
                habit.habit_type,
                habit.mean_interval_days,
                habit.cv,
                habit.visit_count,
                habit.last_visited_ms,
                i64::from(habit.is_interrupted),
                computed_at,
            ],
        )?;
    }
    for record in reopened {
        tx.execute(
            "INSERT INTO reopened_investigations
             (investigation_id, profile_id, anchor_type, anchor_id, anchor_label, occurrence_count, distinct_days,
              first_seen_ms, last_seen_ms, evidence_json, computed_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                record.investigation_id,
                record.profile_id,
                record.anchor_type,
                record.anchor_id,
                record.anchor_label,
                record.occurrence_count,
                record.distinct_days,
                record.first_seen_ms,
                record.last_seen_ms,
                record.evidence_json,
                computed_at,
            ],
        )?;
    }
    for flow in path_flows {
        tx.execute(
            "INSERT INTO path_flows
             (profile_id, flow_pattern, step_count, occurrence_count, last_seen_ms)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                flow.profile_id,
                flow.flow_pattern,
                flow.step_count,
                flow.occurrence_count,
                flow.last_seen_ms,
            ],
        )?;
    }
    tx.commit()?;
    Ok(())
}

/// Deletes trail-member and search-event rows touched by the dirty structural
/// replay window so streamed replay can replace only the affected suffix.
fn delete_structural_memberships_in_range(
    tx: &rusqlite::Transaction<'_>,
    profile_id: &str,
    start_ms: i64,
) -> Result<()> {
    let membership_filter = "SELECT visit_derived_facts.visit_id
         FROM visit_derived_facts
         JOIN archive.visits ON archive.visits.id = visit_derived_facts.visit_id
         WHERE visit_derived_facts.profile_id = ?1
           AND archive.visits.reverted_at IS NULL
           AND archive.visits.visit_time_ms >= ?2";
    tx.execute(
        &format!(
            "DELETE FROM search_trail_members
             WHERE profile_id = ?1
               AND visit_id IN ({membership_filter})"
        ),
        params![profile_id, start_ms],
    )?;
    tx.execute(
        &format!(
            "DELETE FROM search_event_terms
             WHERE profile_id = ?1
               AND visit_id IN ({membership_filter})"
        ),
        params![profile_id, start_ms],
    )?;
    tx.execute(
        &format!(
            "DELETE FROM search_events
             WHERE profile_id = ?1
               AND visit_id IN ({membership_filter})"
        ),
        params![profile_id, start_ms],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn structural_profile_aggregate_replace_persists_habit_patterns() {
        let connection = Connection::open_in_memory().expect("sqlite");
        crate::intelligence::ensure_core_intelligence_schema(&connection).expect("schema");
        let habits = vec![super::super::HabitPatternRecord {
            profile_id: "p1".to_string(),
            registrable_domain: "example.com".to_string(),
            habit_type: "weekly".to_string(),
            mean_interval_days: 7.0,
            cv: 0.15,
            visit_count: 8,
            last_visited_ms: 1767225600000,
            is_interrupted: true,
        }];

        replace_structural_profile_aggregates(
            &connection,
            "p1",
            &[],
            &[],
            &habits,
            &[],
            &[],
            "2026-01-01T00:00:00Z",
        )
        .expect("replace aggregates");

        let row: (String, i64) = connection
            .query_row(
                "SELECT habit_type, is_interrupted FROM habit_patterns WHERE profile_id = 'p1'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("habit row");
        assert_eq!(row, ("weekly".to_string(), 1));
    }
}
