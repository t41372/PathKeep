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

#[cfg(test)]
mod tests {
    use super::super::{SearchQueryKind, TrailMemberRecord, ensure_core_intelligence_schema};
    use super::*;

    fn count_rows(connection: &Connection, table: &str) -> usize {
        connection
            .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| row.get::<_, i64>(0))
            .expect("count rows")
            .try_into()
            .expect("non-negative row count")
    }

    #[test]
    fn persist_core_state_replaces_every_full_rebuild_table() {
        let connection = Connection::open_in_memory().expect("connection");
        ensure_core_intelligence_schema(&connection).expect("schema");
        connection
            .execute(
                "INSERT INTO visit_derived_facts
                 (visit_id, profile_id, registrable_domain, canonical_url, domain_category,
                  page_category, is_new_domain, is_search_event, evidence_tier, taxonomy_source,
                  computed_at)
                 VALUES (99, 'p1', 'stale.example', 'https://stale.example/', 'unknown',
                         'unknown', 0, 0, 'tier-c', 'fixture', 'stale')",
                [],
            )
            .expect("seed stale row");

        let rollups = DailyRollupBundle {
            domain_rows: vec![(
                "2026-04-25".to_string(),
                "p1".to_string(),
                "example.com".to_string(),
                "reference".to_string(),
                2,
                1,
                1,
                2,
            )],
            category_rows: vec![(
                "2026-04-25".to_string(),
                "p1".to_string(),
                "reference".to_string(),
                2,
                1,
            )],
            engine_rows: vec![(
                "2026-04-25".to_string(),
                "p1".to_string(),
                "google".to_string(),
                1,
            )],
            summary_rows: vec![("2026-04-25".to_string(), "p1".to_string(), 2, 1, 1, 1, 0.5, 0.5)],
        };
        let visits = vec![VisitRecord {
            visit_id: 1,
            profile_id: "p1".to_string(),
            source_profile_id: 7,
            source_visit_id: 10,
            source_url_id: 11,
            url: "https://example.com/search?q=pathkeep".to_string(),
            title: Some("PathKeep".to_string()),
            visit_time_ms: 1_777_000_000_000,
            from_visit: None,
            transition_type: Some(805_306_368),
            external_referrer_url: None,
            canonical_url: "https://example.com/search".to_string(),
            registrable_domain: "example.com".to_string(),
            domain_category: "reference".to_string(),
            page_category: "search".to_string(),
            search_engine: Some("google".to_string()),
            search_query: Some("pathkeep coverage pathkeep".to_string()),
            is_new_domain: true,
            is_search_event: true,
            evidence_tier: "tier-a".to_string(),
            taxonomy_source: "fixture".to_string(),
            taxonomy_pack: Some("test-pack".to_string()),
            taxonomy_version: Some("v1".to_string()),
            display_name: Some("Example".to_string()),
            session_id: Some("session-1".to_string()),
            trail_id: Some("trail-1".to_string()),
        }];
        let sessions = vec![SessionRecord {
            session_id: "session-1".to_string(),
            profile_id: "p1".to_string(),
            first_visit_ms: 1,
            last_visit_ms: 2,
            visit_count: 2,
            search_count: 1,
            domain_count: 1,
            is_deep_dive: true,
            auto_title: Some("Research".to_string()),
        }];
        let search_events = vec![
            SearchEventRecord {
                visit_id: 1,
                profile_id: "p1".to_string(),
                search_engine: "google".to_string(),
                raw_query: "PathKeep coverage PathKeep".to_string(),
                normalized_query: "pathkeep coverage pathkeep".to_string(),
                query_kind: SearchQueryKind::Keyword,
                trail_id: Some("trail-1".to_string()),
                visit_time_ms: 1,
            },
            SearchEventRecord {
                visit_id: 2,
                profile_id: "p1".to_string(),
                search_engine: "google".to_string(),
                raw_query: "example.com".to_string(),
                normalized_query: "example.com".to_string(),
                query_kind: SearchQueryKind::Navigational,
                trail_id: None,
                visit_time_ms: 2,
            },
        ];
        let trails = vec![TrailRecord {
            trail_id: "trail-1".to_string(),
            profile_id: "p1".to_string(),
            session_id: "session-1".to_string(),
            initial_query: "pathkeep".to_string(),
            search_engine: "google".to_string(),
            reformulation_count: 1,
            visit_count: 2,
            landing_url: Some("https://example.com/landing".to_string()),
            landing_domain: Some("example.com".to_string()),
            first_visit_ms: 1,
            last_visit_ms: 2,
            max_depth: 2,
            queries: vec!["pathkeep".to_string(), "coverage".to_string()],
            members: vec![TrailMemberRecord {
                trail_id: "trail-1".to_string(),
                profile_id: "p1".to_string(),
                visit_id: 1,
                ordinal: 0,
                role: "search".to_string(),
            }],
        }];
        let query_families = vec![QueryFamilyRecord {
            family_id: "family-1".to_string(),
            profile_id: "p1".to_string(),
            anchor_query: "pathkeep".to_string(),
            member_count: 2,
            search_engine: "google".to_string(),
            first_seen_ms: 1,
            last_seen_ms: 2,
            queries: vec!["pathkeep".to_string(), "pathkeep coverage".to_string()],
        }];
        let refind_pages = vec![RefindPageRecord {
            profile_id: "p1".to_string(),
            canonical_url: "https://example.com/landing".to_string(),
            url: "https://example.com/landing?from=search".to_string(),
            title: Some("Landing".to_string()),
            registrable_domain: "example.com".to_string(),
            cross_day_count: 2,
            trail_count: 1,
            search_arrival_count: 1,
            typed_revisit_count: 1,
            refind_score: 0.75,
            evidence_json: "[1,2]".to_string(),
            first_seen_ms: 1,
            last_seen_ms: 2,
        }];
        let source_effectiveness = vec![SourceEffectivenessRecord {
            profile_id: "p1".to_string(),
            registrable_domain: "example.com".to_string(),
            source_role: "landing".to_string(),
            trail_count: 1,
            stable_landing_count: 1,
            effectiveness_score: 0.8,
            evidence_json: "[1]".to_string(),
            first_seen_ms: 1,
            last_seen_ms: 2,
        }];
        let habits = vec![HabitPatternRecord {
            profile_id: "p1".to_string(),
            registrable_domain: "example.com".to_string(),
            habit_type: "weekly".to_string(),
            mean_interval_days: 7.0,
            cv: 0.1,
            visit_count: 5,
            last_visited_ms: 2,
            is_interrupted: false,
        }];
        let reopened = vec![ReopenedInvestigationRecord {
            investigation_id: "reopened-1".to_string(),
            profile_id: "p1".to_string(),
            anchor_type: "query-family".to_string(),
            anchor_id: "family-1".to_string(),
            anchor_label: "pathkeep".to_string(),
            occurrence_count: 2,
            distinct_days: 2,
            first_seen_ms: 1,
            last_seen_ms: 2,
            evidence_json: "[1,2]".to_string(),
        }];
        let path_flows = vec![PathFlowRecord {
            profile_id: "p1".to_string(),
            flow_pattern: "search -> docs".to_string(),
            step_count: 2,
            occurrence_count: 3,
            last_seen_ms: 2,
        }];

        persist_core_state_for_job_kind(
            &connection,
            None,
            RebuildMode::FullRebuild,
            "2026-04-25T00:00:00Z",
            &visits,
            &rollups,
            &sessions,
            &search_events,
            &trails,
            &query_families,
            &refind_pages,
            &source_effectiveness,
            &habits,
            &reopened,
            &path_flows,
        )
        .expect("persist full rebuild");

        assert_eq!(count_rows(&connection, "visit_derived_facts"), 1);
        assert_eq!(count_rows(&connection, "domain_daily_rollups"), 1);
        assert_eq!(count_rows(&connection, "category_daily_rollups"), 1);
        assert_eq!(count_rows(&connection, "engine_daily_rollups"), 1);
        assert_eq!(count_rows(&connection, "daily_summary_rollups"), 1);
        assert_eq!(count_rows(&connection, "sessions"), 1);
        assert_eq!(count_rows(&connection, "search_trails"), 1);
        assert_eq!(count_rows(&connection, "search_trail_members"), 1);
        assert_eq!(count_rows(&connection, "search_events"), 2);
        assert_eq!(count_rows(&connection, "search_event_terms"), 2);
        assert_eq!(count_rows(&connection, "query_families"), 1);
        assert_eq!(count_rows(&connection, "refind_pages"), 1);
        assert_eq!(count_rows(&connection, "source_effectiveness"), 1);
        assert_eq!(count_rows(&connection, "habit_patterns"), 1);
        assert_eq!(count_rows(&connection, "reopened_investigations"), 1);
        assert_eq!(count_rows(&connection, "path_flows"), 1);
    }
}
