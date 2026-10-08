//! Responsibilities: run production render queries under plan capture.
//! Not responsible for: aggregate equivalence or benchmark timings.
//! Failure modes: an endpoint scans canonical facts or detail/continuation plans regress.
use super::*;

#[test]
fn render_reads_use_indexed_archive_and_membership_access() {
    use vault_core::intelligence::*;
    use vault_core::*;
    for key in [None, Some([76; 32])] {
        let vault = Vault::open_temp_with_key(key).unwrap();
        seed(&vault, 1, 1313);
        // A second site besides example.com, so Top sites has a next page: the fixture's
        // google.com visits are all searches, which Top sites leaves out.
        vault.store().write(Db::Archive, Priority::Batch, |s| {
            s.execute("INSERT INTO pages(id,url,canonical_url,registrable_domain) VALUES(102,'https://notes.example.org/a','https://notes.example.org/a','example.org')", ())?;
            s.execute("INSERT INTO urls(id,page_id,url,title,first_visit_ms,first_visit_iso,last_visit_ms,last_visit_iso,source_profile_id,created_by_run_id,source_url_id) VALUES(102,102,'https://notes.example.org/a','Notes',0,'',0,'',1,0,102)", ())?;
            for id in 1314..=1316_i64 {
                s.execute("INSERT INTO visits(id,url_id,source_visit_id,visit_time_ms,visit_time_iso,source_profile_id,created_by_run_id,transition_type) VALUES(?1,102,?2,?3,'',1,0,0)", vault_store::params![id, id.to_string(), 1_700_000_000_000_i64 + id * 60_000])?;
            }
            s.execute("UPDATE archive_totals SET value=(SELECT COUNT(*) FROM visits) WHERE name IN ('visits','sourceVisits:1')", ())?;
            Ok(())
        }).unwrap();
        run_core_intelligence(&vault, &Default::default()).unwrap();
        let range = DateRange { start: "2023-11-14".into(), end: "2023-11-20".into() };
        let scope = ScopedDateRangeRequest {
            date_range: range.clone(),
            profile_id: Some("chrome:Default".into()),
        };
        let page = PagedDateRangeRequest {
            cursor: None,
            date_range: range.clone(),
            profile_id: scope.profile_id.clone(),

            page_size: 10,
        };
        vault.store().write(Db::SourceEvidence, Priority::Batch, |s| {
            s.execute("INSERT INTO visit_engagement_evidence(source_batch_id,source_profile_id,source_visit_id,metric_key,metric_value_int,source_field,recorded_at) VALUES(0,1,'1313','engagement.total_foreground_duration_ms',42,'fixture','')", ())?;
            Ok(())
        }).unwrap();
        vault.store().capture_query_plans(Db::Intelligence, true).unwrap();
        vault.store().capture_query_plans(Db::Archive, true).unwrap();
        vault.store().capture_query_plans(Db::SourceEvidence, true).unwrap();
        macro_rules! check {($label:literal,$expression:expr)=> {{
            let result=$expression;
            let mut plans=vault.store().take_query_plans(Db::Intelligence).unwrap();
            plans.extend(vault.store().take_query_plans(Db::Archive).unwrap());
            plans.extend(vault.store().take_query_plans(Db::SourceEvidence).unwrap());
            assert!(!plans.is_empty(),"{} did not execute its production read",$label);
            for plan in plans {
                for step in &plan.steps {
                    assert!(!["SCAN visits","SCAN archive.visits","SCAN v","SCAN urls","SCAN archive.urls","SCAN u"].iter().any(|prefix|step==prefix || step.starts_with(&format!("{prefix} "))),"{}: unbounded canonical scan: {:?}\n{}",$label,plan.steps,plan.sql);
                }
            }
            result.unwrap()
        }};}
        check!("overview", get_intelligence_primary_overview(&vault, &scope));
        check!("overview secondary", get_intelligence_secondary_overview(&vault, &scope));
        check!(
            "domain",
            get_domain_deep_dive(
                &vault,
                &DomainDeepDiveRequest {
                    pages_cursor: None,
                    searches_cursor: None,
                    page_size: None,
                    registrable_domain: "example.com".into(),
                    date_range: range.clone(),
                    profile_id: scope.profile_id.clone()
                }
            )
        );
        let sessions = check!("sessions", get_sessions(&vault, &page));
        check!(
            "session detail",
            get_session_detail(
                &vault,
                &SessionDetailRequest {
                    session_id: sessions.sessions[0].session_id.clone(),
                    ..Default::default()
                }
            )
        );
        let trails = check!(
            "trails",
            get_search_trails(
                &vault,
                &SearchTrailQueryRequest {
                    cursor: None,
                    date_range: range.clone(),
                    profile_id: scope.profile_id.clone(),
                    engine: None,

                    page_size: 10
                }
            )
        );
        assert!(!trails.trails.is_empty());
        if let Some(trail) = trails.trails.first() {
            check!(
                "trail detail",
                get_trail_detail(
                    &vault,
                    &TrailDetailRequest { trail_id: trail.trail_id.clone(), ..Default::default() }
                )
            );
        }
        check!("navigation", get_navigation_path(&vault, 11));
        let refind = check!(
            "refind",
            get_refind_pages(
                &vault,
                &RefindPagesRequest {
                    cursor: None,
                    date_range: range.clone(),
                    profile_id: scope.profile_id.clone(),
                    limit: Some(10)
                }
            )
        );
        assert!(!refind.is_empty());
        if let Some(item) = refind.first() {
            check!(
                "refind detail",
                get_refind_page_detail(
                    &vault,
                    &RefindPageDetailRequest {
                        canonical_url: item.canonical_url.clone(),
                        date_range: range.clone(),
                        profile_id: scope.profile_id.clone(),
                        ..Default::default()
                    }
                )
            );
            check!(
                "explain refind",
                explain_refind(
                    &vault,
                    &ExplainRefindRequest {
                        profile_id: None,
                        cursor: None,
                        page_size: None,
                        canonical_url: item.canonical_url.clone()
                    }
                )
            );
        }
        check!(
            "explain session",
            explain_entity(
                &vault,
                &EntityExplanationRequest {
                    cursor: None,
                    page_size: None,
                    entity_type: "session".into(),
                    entity_id: sessions.sessions[0].session_id.clone()
                }
            )
        );
        check!(
            "daily",
            get_day_insights(
                &vault,
                &DayInsightsRequest {
                    date: "2023-11-16".into(),
                    profile_id: scope.profile_id.clone()
                }
            )
        );
        check!(
            "search query list",
            get_search_queries(
                &vault,
                &SearchQueryListRequest {
                    date_range: range.clone(),
                    profile_id: scope.profile_id.clone(),
                    ..Default::default()
                }
            )
        );
        let families = check!("query families", get_query_families(&vault, &page));
        assert!(!families.families.is_empty());
        check!(
            "family detail",
            get_query_family_detail(
                &vault,
                &QueryFamilyDetailRequest {
                    family_id: families.families[0].family_id.clone(),
                    date_range: range.clone(),
                    profile_id: scope.profile_id.clone(),
                    ..Default::default()
                }
            )
        );
        let lists = ScopedPageRequest::from(&scope);
        check!("stable sources", get_stable_sources(&vault, &lists));
        check!("friction", get_friction_signals(&vault, &lists));
        check!("reopened", get_reopened_investigations(&vault, &lists));
        check!("observed interactions", get_observed_interactions(&vault, &lists));
        check!("breadth", get_breadth_index(&vault, &scope));
        check!("habits", get_habit_patterns(&vault, &scope));
        check!(
            "interrupted habits",
            get_interrupted_habits(
                &vault,
                &ProfileScopedRequest { profile_id: scope.profile_id.clone() }
            )
        );
        check!("digest", get_digest_summary(&vault, &scope));
        check!("activity", get_activity_mix(&vault, &scope));
        check!("engine ranking", get_search_engine_ranking(&vault, &scope));
        check!(
            "effectiveness",
            get_search_effectiveness(
                &vault,
                &SearchEffectivenessRequest {
                    date_range: range.clone(),
                    profile_id: scope.profile_id.clone(),
                    ..Default::default()
                }
            )
        );
        let granularity = GranularityDateRangeRequest {
            date_range: range.clone(),
            profile_id: scope.profile_id.clone(),
            granularity: "day".into(),
        };
        check!("activity trend", get_activity_mix_trend(&vault, &granularity));
        check!("discovery trend", get_discovery_trend(&vault, &granularity));
        check!(
            "rhythm",
            get_browsing_rhythm(
                &vault,
                &CategoryFilteredDateRangeRequest {
                    date_range: range.clone(),
                    profile_id: scope.profile_id.clone(),
                    ..Default::default()
                }
            )
        );
        check!("anniversaries", get_on_this_day(&vault, scope.profile_id.as_deref()));
        let sites_request = TopSitesRequest {
            date_range: range.clone(),
            profile_id: scope.profile_id.clone(),
            profile_ids: Vec::new(),
            limit: Some(1),
            ..Default::default()
        };
        let sites = check!("top sites", get_top_sites(&vault, &sites_request));
        assert!(sites.next_cursor.is_some());
        check!(
            "top sites continuation",
            get_top_sites(
                &vault,
                &TopSitesRequest { cursor: sites.next_cursor, ..sites_request.clone() }
            )
        );
        check!("hub pages", get_hub_pages(&vault, &sites_request));
        check!(
            "frequent queries",
            get_frequent_searches(
                &vault,
                &FrequentSearchesRequest {
                    date_range: range.clone(),
                    profile_id: scope.profile_id.clone(),
                    ..Default::default()
                }
            )
        );
        check!(
            "concepts",
            get_top_search_concepts(
                &vault,
                &TopSearchConceptsRequest {
                    date_range: range.clone(),
                    profile_id: scope.profile_id.clone(),
                    ..Default::default()
                }
            )
        );
        check!(
            "path flows",
            get_path_flows(
                &vault,
                &PathFlowRequest {
                    date_range: range.clone(),
                    profile_id: scope.profile_id.clone(),
                    step_count: 2,
                    ..Default::default()
                }
            )
        );
        let compare = check!("compare sets", get_compare_sets(&vault, &scope));
        if let Some(item) = compare.first() {
            check!(
                "compare detail",
                get_compare_set_detail(
                    &vault,
                    &CompareSetDetailRequest {
                        compare_set_id: item.compare_set_id.clone(),
                        date_range: range.clone(),
                        profile_id: scope.profile_id.clone()
                    }
                )
            );
        }
        check!("browser difference", get_multi_browser_diff(&vault, &scope));
        let cards = IntelligenceEmbedCardsRequest {
            date_range: range.clone(),
            profile_id: scope.profile_id.clone(),
            ..Default::default()
        };
        check!("embed cards", get_intelligence_embed_cards(&vault, &cards));
        check!("widget snapshot", get_intelligence_widget_snapshot(&vault, &cards));
        check!("public snapshot", get_intelligence_public_snapshot(&vault, &scope));
        check!("search settings", list_search_engine_rules_for_settings(&vault));
        check!(
            "domain trend",
            get_domain_trend(
                &vault,
                &DomainTrendRequest { date_range: range, registrable_domain: "example.com".into() }
            )
        );
    }
}
