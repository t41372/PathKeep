//! Full deterministic-state persistence for scoped legacy rebuilds.
//!
//! ## Responsibilities
//! - Replace every deterministic table touched by the scoped full rebuild path
//!   inside one transaction.
//! - Preserve the existing full-rebuild contract without duplicating write-side
//!   logic across `intelligence_rebuild` and stage modules.
//! - Reuse shared tokenization and rollup validation helpers during writeback.
//!
//! ## Not responsible for
//! - Choosing rebuild strategy or stage ordering.
//! - Incremental stage-specific persistence such as structural tail replay.
//! - Route-level `/intelligence` read-model queries.
//!
//! ## Dependencies
//! - Parent-module deterministic record types and rollup bundle contract.
//! - Shared rollup uniqueness and query-token helpers.
//! - The `clear_core_tables_for_job_kind` boundary owned by schema/rebuild code.
//!
//! ## Performance notes
//! - The whole replacement happens inside one unchecked SQLite transaction.
//! - Search-event term expansion deduplicates per event before writing to avoid
//!   unnecessary term-row churn on repeated tokens.

use super::{
    DailyRollupBundle, HabitPatternRecord, PathFlowRecord, QueryFamilyRecord, RebuildMode,
    RefindPageRecord, ReopenedInvestigationRecord, SearchEventRecord, SessionRecord,
    SourceEffectivenessRecord, TrailRecord, VisitRecord, clear_core_tables_for_job_kind,
    ensure_unique_domain_rollup_rows, tokenize_query_terms,
};
use anyhow::Result;
use rusqlite::{Connection, params};
use std::collections::BTreeSet;

/// Replaces the full deterministic state for the requested job kind after the
/// scoped legacy rebuild path finishes computing every record in memory.
#[allow(clippy::too_many_arguments)]
pub(super) fn persist_core_state_for_job_kind(
    connection: &Connection,
    profile_id: Option<&str>,
    job_kind: RebuildMode,
    computed_at: &str,
    visits: &[VisitRecord],
    rollups: &DailyRollupBundle,
    sessions: &[SessionRecord],
    search_events: &[SearchEventRecord],
    trails: &[TrailRecord],
    query_families: &[QueryFamilyRecord],
    refind_pages: &[RefindPageRecord],
    source_effectiveness: &[SourceEffectivenessRecord],
    habits: &[HabitPatternRecord],
    reopened: &[ReopenedInvestigationRecord],
    path_flows: &[PathFlowRecord],
) -> Result<()> {
    let tx = connection.unchecked_transaction()?;
    clear_core_tables_for_job_kind(&tx, profile_id, job_kind)?;
    ensure_unique_domain_rollup_rows(&rollups.domain_rows)?;

    for visit in visits {
        tx.execute(
            "INSERT INTO visit_derived_facts (
               visit_id, profile_id, session_id, trail_id, registrable_domain, canonical_url,
               domain_category, page_category, search_engine, search_query, is_new_domain,
               is_search_event, evidence_tier, taxonomy_source, taxonomy_pack, taxonomy_version,
               computed_at
             )
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17)",
            params![
                visit.visit_id,
                visit.profile_id,
                visit.session_id,
                visit.trail_id,
                visit.registrable_domain,
                visit.canonical_url,
                visit.domain_category,
                visit.page_category,
                visit.search_engine,
                visit.search_query,
                i64::from(visit.is_new_domain),
                i64::from(visit.is_search_event),
                visit.evidence_tier,
                visit.taxonomy_source,
                visit.taxonomy_pack,
                visit.taxonomy_version,
                computed_at,
            ],
        )?;
    }

    for (
        date_key,
        profile_id,
        registrable_domain,
        domain_category,
        visit_count,
        search_count,
        new_domain_visits,
        unique_urls,
    ) in &rollups.domain_rows
    {
        tx.execute(
            "INSERT INTO domain_daily_rollups
             (date_key, profile_id, registrable_domain, domain_category, visit_count, search_count, new_domain_visits, unique_urls)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![date_key, profile_id, registrable_domain, domain_category, visit_count, search_count, new_domain_visits, unique_urls],
        )?;
    }
    for (date_key, profile_id, domain_category, visit_count, unique_domains) in
        &rollups.category_rows
    {
        tx.execute(
            "INSERT INTO category_daily_rollups
             (date_key, profile_id, domain_category, visit_count, unique_domains)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![date_key, profile_id, domain_category, visit_count, unique_domains],
        )?;
    }
    for (date_key, profile_id, search_engine, search_count) in &rollups.engine_rows {
        tx.execute(
            "INSERT INTO engine_daily_rollups (date_key, profile_id, search_engine, search_count)
             VALUES (?1, ?2, ?3, ?4)",
            params![date_key, profile_id, search_engine, search_count],
        )?;
    }
    for (
        date_key,
        profile_id,
        total_visits,
        total_searches,
        new_domains,
        unique_domains,
        hhi_score,
        discovery_rate,
    ) in &rollups.summary_rows
    {
        tx.execute(
            "INSERT INTO daily_summary_rollups
             (date_key, profile_id, total_visits, total_searches, new_domains, unique_domains, hhi_score, discovery_rate)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![date_key, profile_id, total_visits, total_searches, new_domains, unique_domains, hhi_score, discovery_rate],
        )?;
    }

    for session in sessions {
        tx.execute(
            "INSERT INTO sessions
             (session_id, profile_id, first_visit_ms, last_visit_ms, visit_count, search_count, domain_count, is_deep_dive, auto_title, computed_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                session.session_id,
                session.profile_id,
                session.first_visit_ms,
                session.last_visit_ms,
                session.visit_count,
                session.search_count,
                session.domain_count,
                i64::from(session.is_deep_dive),
                session.auto_title,
                computed_at,
            ],
        )?;
    }

    for trail in trails {
        tx.execute(
            "INSERT INTO search_trails
             (trail_id, profile_id, session_id, initial_query, search_engine, reformulation_count, visit_count,
              landing_url, landing_domain, first_visit_ms, last_visit_ms, max_depth, queries_json, computed_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
            params![
                trail.trail_id,
                trail.profile_id,
                trail.session_id,
                trail.initial_query,
                trail.search_engine,
                trail.reformulation_count,
                trail.visit_count,
                trail.landing_url,
                trail.landing_domain,
                trail.first_visit_ms,
                trail.last_visit_ms,
                trail.max_depth,
                serde_json::to_string(&trail.queries)?,
                computed_at,
            ],
        )?;
        for member in &trail.members {
            tx.execute(
                "INSERT INTO search_trail_members (trail_id, profile_id, visit_id, ordinal, role)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    member.trail_id,
                    member.profile_id,
                    member.visit_id,
                    member.ordinal,
                    member.role
                ],
            )?;
        }
    }

    for event in search_events {
        tx.execute(
            "INSERT INTO search_events
             (visit_id, profile_id, search_engine, raw_query, normalized_query, query_kind, trail_id, computed_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                event.visit_id,
                event.profile_id,
                event.search_engine,
                event.raw_query,
                event.normalized_query,
                event.query_kind.as_str(),
                event.trail_id,
                computed_at,
            ],
        )?;
        if event.query_kind.is_keyword() {
            for term in
                tokenize_query_terms(&event.normalized_query).into_iter().collect::<BTreeSet<_>>()
            {
                tx.execute(
                    "INSERT INTO search_event_terms (visit_id, profile_id, term) VALUES (?1, ?2, ?3)",
                    params![event.visit_id, event.profile_id, term],
                )?;
            }
        }
    }

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
                flow.last_seen_ms
            ],
        )?;
    }

    tx.commit()?;
    Ok(())
}
