//! Responsibilities: verify complete native generation persistence through the shipping writers.
//! Not responsible for: classification or the production rebuild scheduler.
//! Failure modes: a table is omitted, search terms lose query-kind filtering,
//! assignments are lost, or a partial replacement hides the published generation.
#[cfg(test)]
mod tests {
    use super::super::*;
    use vault_store::Session;

    fn count_rows(connection: &Session<'_>, table: &str) -> usize {
        connection
            .query_one(&format!("SELECT COUNT(*) FROM {table}"), (), |row| Ok(row.get::<i64>(0)?))
            .expect("count rows")
            .try_into()
            .expect("non-negative row count")
    }

    #[test]
    fn persist_core_state_replaces_every_full_rebuild_table() {
        let vault = crate::Vault::open_temp().unwrap();
        vault.write(vault_store::Db::Intelligence,vault_store::Priority::Batch,|connection| {
        connection.execute("INSERT INTO intelligence_generations(profile_id,facts,daily,navigation,aggregates) VALUES('p1',1,1,1,1)",())?;
        connection
            .execute(
                "INSERT INTO visit_derived_facts_rows
                 (generation,visit_id, profile_id, registrable_domain, canonical_url, domain_category,
                  page_category, is_new_domain, is_search_event, evidence_tier, taxonomy_source,
                  computed_at)
                 VALUES (1,99, 'p1', 'stale.example', 'https://stale.example/', 'unknown',
                         'unknown', 0, 0, 'tier-c', 'fixture', 'stale')",
                (),
            )
            .expect("seed stale row");

        let rollups = DailyRollupBundle {
            domain_rows: vec![super::super::DomainDayRow {
                date_key: "2026-04-25".to_string(),
                profile_id: "p1".to_string(),
                registrable_domain: "example.com".to_string(),
                domain_category: "reference".to_string(),
                visit_count: 2,
                search_count: 1,
                new_domain_visits: 1,
                unique_urls: 2,
                last_visit_ms: 0,
            }],
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
            summary_rows: vec![super::super::DaySummaryRow {
                date_key: "2026-04-25".to_string(),
                profile_id: "p1".to_string(),
                total_visits: 2,
                total_searches: 1,
                new_domains: 1,
                unique_domains: 1,
                hhi_score: 0.5,
                discovery_rate: 0.5,
                hours: Vec::new(),
            }],
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
            active_days: 2,
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

        let at="2026-04-25T00:00:00Z";
        let state=build_state::BuildState {profile_id:"p1".into(),facts_generation:2,..Default::default()};
        build_rows::persist_facts(connection,&state,&visits)?;
        build_rows::persist_daily(connection,2,&rollups)?;
        let mut persist=intelligence_structural_stream::StructuralTailPersist::new(connection,2,"p1",crate::intelligence::InsightsZone::system())?;
        for visit in &visits {if let Some(session)=visit.session_id.as_deref() {persist.persist_assignment(visit.visit_id,session,visit.trail_id.as_deref(),visit.visit_id,&visit.registrable_domain)?;}}
        for session in &sessions {persist.persist_session(session,at)?;}
        for trail in &trails {persist.persist_trail(trail,at)?;for member in &trail.members {persist.persist_trail_member(member)?;}}
        for event in &search_events {persist.persist_search_event(event,at)?;}
        drop(persist);
        build_rows::persist_families(connection,2,&query_families,at)?;
        build_rows::persist_aggregates(connection,2,&refind_pages,&reopened,&path_flows,at)?;
        assert_eq!(connection.query_one("SELECT visit_id FROM visit_derived_facts",(),|r|Ok(r.get::<i64>(0)?))?,99);
        connection.execute("UPDATE intelligence_generations SET facts=2,daily=2,navigation=2,aggregates=2 WHERE profile_id='p1'",())?;

        assert_eq!(count_rows(connection, "visit_derived_facts"), 1);
        assert_eq!(count_rows(connection, "domain_daily_rollups"), 1);
        assert_eq!(count_rows(connection, "category_daily_rollups"), 1);
        assert_eq!(count_rows(connection, "engine_daily_rollups"), 1);
        assert_eq!(count_rows(connection, "daily_summary_rollups"), 1);
        assert_eq!(count_rows(connection, "sessions"), 1);
        assert_eq!(count_rows(connection, "search_trails"), 1);
        assert_eq!(count_rows(connection, "search_trail_members"), 1);
        assert_eq!(count_rows(connection, "search_events"), 2);
        assert_eq!(count_rows(connection, "search_event_terms"), 2);
        assert_eq!(count_rows(connection, "query_families"), 1);
        assert_eq!(count_rows(connection, "refind_pages"), 1);
        assert_eq!(count_rows(connection, "reopened_investigations"), 1);
        assert_eq!(count_rows(connection, "path_flows"), 1);
        Ok(())
        }).unwrap();
    }
}
