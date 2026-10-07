//! Responsibilities: verify every ranked intelligence list exposes complete continuation.
//! Not responsible for: frontend rendering or search's candidate pools.
//! Failure modes: defaults become total caps, ties skip records, filters drift across pages,
//! or a last page advertises an empty continuation.
use vault_core::{DateRange, TopSitesRequest, Vault, intelligence::get_top_sites};
use vault_store::{Db, Priority, params};
#[test]
fn top_sites_page_through_every_domain_in_each_sort_order() {
    for key in [None, Some([80; 32])] {
        let vault = Vault::open_temp_with_key(key).unwrap();
        vault.store().write(Db::Intelligence,Priority::Batch,|s| {
            s.execute("INSERT INTO intelligence_generations(profile_id) VALUES('profile')",())?;
            for id in 0..113_i64 {
                s.execute("INSERT INTO domain_daily_rollups_rows(generation,date_key,profile_id,registrable_domain,domain_category,visit_count,search_count,new_domain_visits,unique_urls) VALUES(0,'2026-01-01','profile',?1,'reference',?2,0,0,1)",params![format!("site{id}.test"),id%5+1])?;
            }
            Ok(())
        }).unwrap();
        for sort in ["visit_count", "unique_days", "average_daily_visits"] {
            let mut cursor = None;
            let mut seen = std::collections::BTreeSet::new();
            let mut previous = None;
            loop {
                let page = get_top_sites(
                    &vault,
                    &TopSitesRequest {
                        date_range: DateRange {
                            start: "2026-01-01".into(),
                            end: "2026-01-01".into(),
                        },
                        profile_id: Some("profile".into()),
                        profile_ids: Vec::new(),
                        sort_by: Some(sort.into()),
                        limit: Some(7),
                        cursor,
                    },
                )
                .unwrap();
                assert!(page.items.len() <= 7);
                for site in page.items {
                    assert!(seen.insert(site.registrable_domain.clone()));
                    let key = (-site.visit_count, site.registrable_domain);
                    if let Some(old) = previous.as_ref() {
                        assert!(old <= &key);
                    }
                    previous = Some(key);
                }
                cursor = page.next_cursor;
                if cursor.is_none() {
                    break;
                }
            }
            assert_eq!(seen.len(), 113);
        }
    }
}

#[test]
fn refind_pages_reach_tied_results_and_choose_the_best_profile() {
    for key in [None, Some([81; 32])] {
        let vault = Vault::open_temp_with_key(key).unwrap();
        vault.store().write(Db::Intelligence,Priority::Batch,|s| {
            for (profile,generation) in [("a",1_i64),("b",2)] {
                s.execute("INSERT INTO intelligence_generations(profile_id,aggregates) VALUES(?1,?2)",params![profile,generation])?;
                for id in 0..113_i64 {
                    s.execute("INSERT INTO refind_pages_rows(generation,profile_id,canonical_url,url,title,registrable_domain,cross_day_count,trail_count,search_arrival_count,typed_revisit_count,refind_score,evidence_json,first_seen_ms,last_seen_ms,computed_at) VALUES(?1,?2,?3,?3,'Page','example.com',3,2,2,2,?4,'{}',1767268800000,1767268800000,'')",params![generation,profile,format!("https://example.com/{id}"),(id%3) as f64+generation as f64])?;
                }
            }
            Ok(())
        }).unwrap();
        for profile in [None, Some("a".to_owned())] {
            let mut cursor = None;
            let mut seen = std::collections::BTreeSet::new();
            loop {
                let page = vault_core::get_refind_pages(
                    &vault,
                    &vault_core::RefindPagesRequest {
                        date_range: DateRange {
                            start: "2026-01-01".into(),
                            end: "2026-01-02".into(),
                        },
                        profile_id: profile.clone(),
                        limit: Some(7),
                        cursor,
                    },
                )
                .unwrap();
                for item in page.items {
                    assert!(seen.insert(item.canonical_url));
                    assert_eq!(item.profile_id, if profile.is_none() { "b" } else { "a" });
                    assert_eq!(item.profile_count, if profile.is_none() { 2 } else { 1 });
                }
                cursor = page.next_cursor;
                if cursor.is_none() {
                    break;
                }
            }
            assert_eq!(seen.len(), 113);
        }
    }
}

#[test]
fn frequent_queries_and_concepts_continue_beyond_their_former_limits() {
    for key in [None, Some([82; 32])] {
        let vault = Vault::open_temp_with_key(key).unwrap();
        vault.store().write(Db::Intelligence,Priority::Batch,|s| {
            s.execute("INSERT INTO intelligence_generations(profile_id) VALUES('profile')",())?;
            for id in 1..=113_i64 {
                s.execute("INSERT INTO search_events_rows(generation,visit_id,profile_id,search_engine,raw_query,normalized_query,query_kind,trail_id,computed_at,visit_time_ms) VALUES(0,?1,'profile','engine',?2,?2,'keyword',NULL,'',1767268800000)",params![id,format!("query{id}")])?;
                s.execute("INSERT INTO search_event_terms_rows(generation,visit_id,profile_id,term) VALUES(0,?1,'profile',?2)",params![id,format!("term{id}")])?;
            }
            Ok(())
        }).unwrap();
        let range = DateRange { start: "2026-01-01".into(), end: "2026-01-02".into() };
        let mut cursor = None;
        let mut seen = std::collections::BTreeSet::new();
        loop {
            let page = vault_core::get_frequent_searches(
                &vault,
                &vault_core::FrequentSearchesRequest {
                    date_range: range.clone(),
                    profile_id: Some("profile".into()),
                    limit: Some(9),
                    cursor,
                },
            )
            .unwrap();
            for item in page.items {
                assert!(seen.insert(item.normalized_query));
            }
            cursor = page.next_cursor;
            if cursor.is_none() {
                break;
            }
        }
        assert_eq!(seen.len(), 113);
        let mut cursor = None;
        let mut seen = std::collections::BTreeSet::new();
        loop {
            let page = vault_core::get_top_search_concepts(
                &vault,
                &vault_core::TopSearchConceptsRequest {
                    date_range: range.clone(),
                    profile_id: Some("profile".into()),
                    limit: Some(9),
                    cursor,
                },
            )
            .unwrap();
            for item in page.items {
                assert!(seen.insert(item.term));
            }
            cursor = page.next_cursor;
            if cursor.is_none() {
                break;
            }
        }
        assert_eq!(seen.len(), 113);
    }
}

#[test]
fn sessions_trails_and_families_continue_across_timestamp_ties() {
    for key in [None, Some([85; 32])] {
        let vault = Vault::open_temp_with_key(key).unwrap();
        vault.store().write(Db::Archive,Priority::Batch,|s| {
            s.execute("INSERT INTO source_profiles(id,browser_kind,profile_name,profile_path,profile_key,discovered_at) VALUES(1,'chrome','Profile','/fixture','profile','')",())?;Ok(())
        }).unwrap();
        vault.store().write(Db::Intelligence,Priority::Batch,|s| {
            s.execute("INSERT INTO intelligence_generations(profile_id) VALUES('profile')",())?;
            for id in 1..=37_i64 {
                s.execute("INSERT INTO sessions_rows(generation,session_id,profile_id,first_visit_ms,last_visit_ms,visit_count,search_count,domain_count,is_deep_dive,auto_title,computed_at) VALUES(0,?1,'profile',1767268800000,1767268800000,1,1,1,0,'Session','')",[format!("session:profile:{id}")])?;
                s.execute("INSERT INTO search_trails_rows(generation,trail_id,profile_id,session_id,initial_query,search_engine,reformulation_count,visit_count,landing_url,landing_domain,first_visit_ms,last_visit_ms,max_depth,queries_json,computed_at) VALUES(0,?1,'profile',?2,'query','google',0,1,NULL,NULL,1767268800000,1767268800000,1,'[\"query\"]','')",params![format!("trail:profile:{id}"),format!("session:profile:{id}")])?;
                s.execute("INSERT INTO query_families_rows(generation,family_id,profile_id,anchor_query,member_count,search_engine,first_seen_ms,last_seen_ms,queries_json,computed_at) VALUES(0,?1,'profile','query',1,'google',1767268800000,1767268800000,'[\"query\"]','')",[format!("family:profile:{id}")])?;
            }Ok(())
        }).unwrap();
        for kind in ["sessions", "trails", "families"] {
            let mut cursor = None;
            let mut seen = std::collections::BTreeSet::new();
            loop {
                let range = DateRange { start: "2026-01-01".into(), end: "2026-01-02".into() };
                let (ids, next, total) = match kind {
                    "sessions" => {
                        let p = vault_core::get_sessions(
                            &vault,
                            &vault_core::PagedDateRangeRequest {
                                date_range: range,
                                profile_id: Some("profile".into()),
                                page_size: 7,
                                cursor,
                            },
                        )
                        .unwrap();
                        (
                            p.sessions.into_iter().map(|r| r.session_id).collect::<Vec<_>>(),
                            p.next_cursor,
                            p.total,
                        )
                    }
                    "trails" => {
                        let p = vault_core::get_search_trails(
                            &vault,
                            &vault_core::SearchTrailQueryRequest {
                                date_range: range,
                                profile_id: Some("profile".into()),
                                engine: None,
                                page_size: 7,
                                cursor,
                            },
                        )
                        .unwrap();
                        (
                            p.trails.into_iter().map(|r| r.trail_id).collect::<Vec<_>>(),
                            p.next_cursor,
                            p.total,
                        )
                    }
                    _ => {
                        let p = vault_core::get_query_families(
                            &vault,
                            &vault_core::PagedDateRangeRequest {
                                date_range: range,
                                profile_id: Some("profile".into()),
                                page_size: 7,
                                cursor,
                            },
                        )
                        .unwrap();
                        (
                            p.families.into_iter().map(|r| r.family_id).collect::<Vec<_>>(),
                            p.next_cursor,
                            p.total,
                        )
                    }
                };
                assert_eq!(total, 37);
                for id in ids {
                    assert!(seen.insert(id));
                }
                cursor = next;
                if cursor.is_none() {
                    break;
                }
            }
            assert_eq!(seen.len(), 37);
        }
    }
}

#[test]
fn hub_pages_continue_beyond_the_former_ten_page_limit() {
    for key in [None, Some([88; 32])] {
        let vault = Vault::open_temp_with_key(key).unwrap();
        vault.store().write(Db::Archive,Priority::Batch,|s| {
            s.execute("INSERT INTO source_profiles(id,browser_kind,profile_name,profile_path,profile_key,discovered_at) VALUES(1,'chrome','Default','/fixture','profile','')",())?;
            for id in 1..=113_i64 {
                let url=format!("https://example.com/{id}");
                s.execute("INSERT INTO pages(id,url,canonical_url) VALUES(?1,?2,?2)",params![id,url.clone()])?;
                s.execute("INSERT INTO urls(id,page_id,url,source_profile_id,created_by_run_id,first_visit_ms,first_visit_iso,last_visit_ms,last_visit_iso) VALUES(?1,?1,?2,1,0,1767268800000,'',1767268800000,'')",params![id,url])?;
                s.execute("INSERT INTO visits(id,url_id,source_profile_id,created_by_run_id,visit_time_ms,visit_time_iso) VALUES(?1,?1,1,0,1767268800000,'')",[id])?;
            }Ok(())
        }).unwrap();
        vault.store().write(Db::Intelligence,Priority::Batch,|s| {
            s.execute("INSERT INTO intelligence_generations(profile_id) VALUES('profile')",())?;
            for id in 1..=113_i64 {
                s.execute("INSERT INTO visit_derived_facts_rows(visit_id,profile_id,canonical_url,registrable_domain,computed_at) VALUES(?1,'profile',?2,'example.com','')",params![id,format!("https://example.com/{id}")])?;
                s.execute("INSERT INTO search_trail_members_rows(trail_id,profile_id,visit_id,ordinal,role) VALUES(?1,'profile',?2,0,'landing')",params![format!("trail:{id}"),id])?;
            }Ok(())
        }).unwrap();
        let mut cursor = None;
        let mut seen = std::collections::BTreeSet::new();
        loop {
            let page = vault_core::get_hub_pages(
                &vault,
                &TopSitesRequest {
                    date_range: DateRange { start: "2026-01-01".into(), end: "2026-01-02".into() },
                    profile_id: None,
                    profile_ids: Vec::new(),
                    sort_by: None,
                    limit: Some(7),
                    cursor,
                },
            )
            .unwrap();
            for item in page.items {
                assert!(seen.insert(item.url));
            }
            cursor = page.next_cursor;
            if cursor.is_none() {
                break;
            }
        }
        assert_eq!(seen.len(), 113);
    }
}

#[test]
fn summary_lists_keep_every_source_friction_signal_and_reopened_investigation() {
    for key in [None, Some([89; 32])] {
        let vault = Vault::open_temp_with_key(key).unwrap();
        vault.store().write(Db::Intelligence,Priority::Batch,|s| {
            s.execute("INSERT INTO intelligence_generations(profile_id) VALUES('profile')",())?;
            for id in 1..=113_i64 {
                s.execute("INSERT INTO source_effectiveness_rows(profile_id,registrable_domain,source_role,trail_count,stable_landing_count,effectiveness_score,evidence_json,first_seen_ms,last_seen_ms,computed_at) VALUES('profile',?1,'reference',3,2,?2,'{}',1767268800000,1767268800000,'')",params![format!("site{id}.test"),(id%3) as f64])?;
                s.execute("INSERT INTO search_trails_rows(trail_id,profile_id,initial_query,search_engine,reformulation_count,visit_count,first_visit_ms,last_visit_ms,queries_json,computed_at) VALUES(?1,'profile',?2,'google',2,3,1767268800000,1767268800000,'[]','')",params![format!("trail:{id}"),format!("query{id}")])?;
                s.execute("INSERT INTO reopened_investigations_rows(investigation_id,profile_id,anchor_type,anchor_id,anchor_label,occurrence_count,distinct_days,first_seen_ms,last_seen_ms,evidence_json,computed_at) VALUES(?1,'profile','query_family',?1,?1,3,2,1767268800000,1767268800000,'{}','')",[format!("investigation:{id}")])?;
            }Ok(())
        }).unwrap();
        for kind in ["stable", "friction", "reopened"] {
            let mut cursor = None;
            let mut seen = std::collections::BTreeSet::new();
            loop {
                let request = vault_core::ScopedPageRequest {
                    date_range: DateRange { start: "2026-01-01".into(), end: "2026-01-02".into() },
                    profile_id: None,
                    limit: Some(7),
                    cursor,
                };
                let (ids, next) = match kind {
                    "stable" => {
                        let p = vault_core::get_stable_sources(&vault, &request).unwrap();
                        (
                            p.items.into_iter().map(|r| r.registrable_domain).collect::<Vec<_>>(),
                            p.next_cursor,
                        )
                    }
                    "friction" => {
                        let p = vault_core::get_friction_signals(&vault, &request).unwrap();
                        (
                            p.items.into_iter().map(|r| r.description).collect::<Vec<_>>(),
                            p.next_cursor,
                        )
                    }
                    _ => {
                        let p = vault_core::get_reopened_investigations(&vault, &request).unwrap();
                        (
                            p.items.into_iter().map(|r| r.investigation_id).collect::<Vec<_>>(),
                            p.next_cursor,
                        )
                    }
                };
                for id in ids {
                    assert!(seen.insert(id));
                }
                cursor = next;
                if cursor.is_none() {
                    break;
                }
            }
            assert_eq!(seen.len(), 113, "{kind}");
        }
    }
}

#[test]
fn materialized_all_time_flows_page_past_twenty_results() {
    for key in [None, Some([90; 32])] {
        let vault = Vault::open_temp_with_key(key).unwrap();
        vault.store().write(Db::Intelligence,Priority::Batch,|s| {
            s.execute("INSERT INTO intelligence_generations(profile_id) VALUES('profile')",())?;
            for id in 1..=113_i64 {
                s.execute("INSERT INTO path_flows_rows(profile_id,flow_pattern,step_count,occurrence_count,last_seen_ms) VALUES('profile',?1,2,?2,1767268800000)",params![format!("site{id}.test → example.com"),id%3+1])?;
            }Ok(())
        }).unwrap();
        let mut cursor = None;
        let mut seen = std::collections::BTreeSet::new();
        loop {
            let page = vault_core::get_path_flows(
                &vault,
                &vault_core::PathFlowRequest {
                    date_range: DateRange { start: "1900-01-01".into(), end: "2100-01-01".into() },
                    profile_id: None,
                    step_count: 2,
                    limit: Some(7),
                    cursor,
                },
            )
            .unwrap();
            for item in page.items {
                assert!(seen.insert(item.flow_id));
            }
            cursor = page.next_cursor;
            if cursor.is_none() {
                break;
            }
        }
        assert_eq!(seen.len(), 113);
    }
}

#[test]
fn sparse_observed_evidence_has_continuation_even_when_a_page_is_empty() {
    for key in [None, Some([92; 32])] {
        let vault = Vault::open_temp_with_key(key).unwrap();
        vault.store().write(Db::Archive,Priority::Batch,|s| {
            s.execute("INSERT INTO source_profiles(id,browser_kind,profile_name,profile_path,profile_key,discovered_at) VALUES(1,'chrome','Default','/fixture','profile','')",())?;
            s.execute("INSERT INTO pages(id,url,canonical_url) VALUES(1,'https://example.com/','https://example.com/')",())?;
            s.execute("INSERT INTO urls(id,page_id,url,source_profile_id,created_by_run_id,first_visit_ms,first_visit_iso,last_visit_ms,last_visit_iso) VALUES(1,1,'https://example.com/',1,0,1767268800000,'',1767268800000,'')",())?;
            for id in 1..=113_i64 {s.execute("INSERT INTO visits(id,url_id,source_visit_id,source_profile_id,created_by_run_id,visit_time_ms,visit_time_iso) VALUES(?1,1,?2,1,0,1767268800000,'')",params![id,id.to_string()])?;}Ok(())
        }).unwrap();
        vault.store().write(Db::SourceEvidence,Priority::Batch,|s| {
            for id in [1_i64,57] {s.execute("INSERT INTO visit_engagement_evidence(source_batch_id,source_profile_id,source_visit_id,metric_key,metric_value_int,source_field,recorded_at) VALUES(0,1,?1,'engagement.total_foreground_duration_ms',42,'fixture','')",[id.to_string()])?;}Ok(())
        }).unwrap();
        let mut cursor = None;
        let mut seen = std::collections::BTreeSet::new();
        let mut empty_continuations = 0;
        loop {
            let page = vault_core::get_observed_interactions(
                &vault,
                &vault_core::ScopedPageRequest {
                    date_range: DateRange { start: "2026-01-01".into(), end: "2026-01-02".into() },
                    profile_id: None,
                    limit: Some(7),
                    cursor,
                },
            )
            .unwrap();
            if page.items.is_empty() && page.next_cursor.is_some() {
                empty_continuations += 1;
            }
            for item in page.items {
                assert!(seen.insert(item.visit_id));
                assert_eq!(item.foreground_duration_ms, Some(42));
            }
            cursor = page.next_cursor;
            if cursor.is_none() {
                break;
            }
        }
        assert!(empty_continuations > 0);
        assert_eq!(seen, std::collections::BTreeSet::from([1, 57]));
    }
}

#[test]
fn search_effectiveness_pages_through_every_hard_topic() {
    for key in [None, Some([93; 32])] {
        let vault = Vault::open_temp_with_key(key).unwrap();
        vault.store().write(Db::Intelligence,Priority::Batch,|s| {
            s.execute("INSERT INTO intelligence_generations(profile_id) VALUES('profile')",())?;
            for id in 1..=37_i64 {s.execute("INSERT INTO query_families_rows(family_id,profile_id,anchor_query,member_count,search_engine,first_seen_ms,last_seen_ms,queries_json,computed_at) VALUES(?1,'profile',?1,?2,'google',1767268800000,1767268800000,'[]','')",params![format!("family:{id}"),id%3+1])?;}Ok(())
        }).unwrap();
        let mut cursor = None;
        let mut seen = std::collections::BTreeSet::new();
        loop {
            let result = vault_core::get_search_effectiveness(
                &vault,
                &vault_core::SearchEffectivenessRequest {
                    date_range: DateRange { start: "2026-01-01".into(), end: "2026-01-02".into() },
                    profile_id: None,
                    engine: Some("google".into()),
                    topics_limit: Some(7),
                    topics_cursor: cursor,
                },
            )
            .unwrap();
            for topic in result.hardest_topics.items {
                assert!(seen.insert(topic.family_id));
            }
            cursor = result.hardest_topics.next_cursor;
            if cursor.is_none() {
                break;
            }
        }
        assert_eq!(seen.len(), 37);
    }
}

#[test]
fn domain_details_page_through_all_pages_and_landing_queries() {
    for key in [None, Some([94; 32])] {
        let vault = Vault::open_temp_with_key(key).unwrap();
        vault.store().write(Db::Archive,Priority::Batch,|s| {
            s.execute("INSERT INTO source_profiles(id,browser_kind,profile_name,profile_path,profile_key,discovered_at) VALUES(1,'chrome','Default','/fixture','profile','')",())?;
            for id in 1..=113_i64 {
                let url=format!("https://example.com/{id}");
                s.execute("INSERT INTO pages(id,url,canonical_url,registrable_domain) VALUES(?1,?2,?2,'example.com')",params![id,url.clone()])?;
                s.execute("INSERT INTO urls(id,page_id,url,title,source_profile_id,created_by_run_id,first_visit_ms,first_visit_iso,last_visit_ms,last_visit_iso) VALUES(?1,?1,?2,'Page',1,0,1767268800000,'',1767268800000,'')",params![id,url])?;
                s.execute("INSERT INTO visits(id,url_id,source_profile_id,created_by_run_id,visit_time_ms,visit_time_iso) VALUES(?1,?1,1,0,1767268800000,'')",[id])?;
            }Ok(())
        }).unwrap();
        vault.store().write(Db::Intelligence,Priority::Batch,|s| {
            s.execute("INSERT INTO intelligence_generations(profile_id) VALUES('profile')",())?;
            s.execute("INSERT INTO domain_daily_rollups_rows(profile_id,date_key,registrable_domain,domain_category,visit_count,search_count,new_domain_visits,unique_urls) VALUES('profile','2026-01-01','example.com','reference',113,0,0,113)",())?;
            for id in 1..=113_i64 {s.execute("INSERT INTO search_trails_rows(trail_id,profile_id,initial_query,search_engine,visit_count,landing_domain,first_visit_ms,last_visit_ms,queries_json,computed_at) VALUES(?1,'profile',?2,'google',1,'example.com',1767268800000,1767268800000,'[]','')",params![format!("trail:{id}"),format!("query{id}")])?;}Ok(())
        }).unwrap();
        let mut pages_cursor = None;
        let mut searches_cursor = None;
        let mut pages = std::collections::BTreeSet::new();
        let mut queries = std::collections::BTreeSet::new();
        loop {
            let detail = vault_core::get_domain_deep_dive(
                &vault,
                &vault_core::DomainDeepDiveRequest {
                    registrable_domain: "example.com".into(),
                    date_range: DateRange { start: "2026-01-01".into(), end: "2026-01-02".into() },
                    profile_id: None,
                    page_size: Some(7),
                    pages_cursor,
                    searches_cursor,
                },
            )
            .unwrap();
            assert_eq!(detail.page_count, 113);
            assert_eq!(detail.landing_search_count, 113);
            for page in detail.top_pages.items {
                assert!(pages.insert(page.url));
            }
            for query in detail.landing_searches.items {
                assert!(queries.insert(query.query));
            }
            pages_cursor = detail.top_pages.next_cursor;
            searches_cursor = detail.landing_searches.next_cursor;
            if pages_cursor.is_none() && searches_cursor.is_none() {
                break;
            }
        }
        assert_eq!(pages.len(), 113);
        assert_eq!(queries.len(), 113);
    }
}

#[test]
fn refind_detail_reaches_all_visits_days_and_related_trails() {
    for key in [None, Some([95; 32])] {
        let vault = Vault::open_temp_with_key(key).unwrap();
        vault.store().write(Db::Archive,Priority::Batch,|s| {
            s.execute("INSERT INTO source_profiles(id,browser_kind,profile_name,profile_path,profile_key,discovered_at) VALUES(1,'chrome','Default','/fixture','profile','')",())?;
            s.execute("INSERT INTO pages(id,url,canonical_url,registrable_domain) VALUES(1,'https://example.com/','https://example.com/','example.com')",())?;
            s.execute("INSERT INTO urls(id,page_id,url,source_profile_id,created_by_run_id,first_visit_ms,first_visit_iso,last_visit_ms,last_visit_iso) VALUES(1,1,'https://example.com/',1,0,1767268800000,'',1777032000000,'')",())?;
            for id in 1..=113_i64 {s.execute("INSERT INTO visits(id,url_id,source_profile_id,created_by_run_id,visit_time_ms,visit_time_iso) VALUES(?1,1,1,0,?2,'')",params![id,1767268800000+id*86400000])?;}Ok(())
        }).unwrap();
        vault.store().write(Db::Intelligence,Priority::Batch,|s| {
            s.execute("INSERT INTO intelligence_generations(profile_id) VALUES('profile')",())?;
            s.execute("INSERT INTO refind_pages_rows(profile_id,canonical_url,url,title,registrable_domain,cross_day_count,trail_count,search_arrival_count,typed_revisit_count,refind_score,evidence_json,first_seen_ms,last_seen_ms,computed_at) VALUES('profile','https://example.com/','https://example.com/','Page','example.com',113,113,113,0,50,'{\"visitIds\":[1],\"totalVisitCount\":113}',1767268800000,1777032000000,'')",())?;
            for id in 1..=113_i64 {
                let trail=format!("trail:{id}");let time=1767268800000+id*86400000;
                s.execute("INSERT INTO visit_derived_facts_rows(visit_id,profile_id,canonical_url,registrable_domain,computed_at) VALUES(?1,'profile','https://example.com/','example.com','')",[id])?;
                s.execute("INSERT INTO visit_assignments(visit_id,profile_id,session_id,trail_id) VALUES(?1,'profile','session',?2)",params![id,trail.clone()])?;
                s.execute("INSERT INTO search_trails_rows(trail_id,profile_id,initial_query,search_engine,visit_count,first_visit_ms,last_visit_ms,queries_json,computed_at) VALUES(?1,'profile','query','google',1,?2,?2,'[]','')",params![trail,time])?;
            }Ok(())
        }).unwrap();
        let mut visits_cursor = None;
        let mut trails_cursor = None;
        let mut visits = std::collections::BTreeSet::new();
        let mut trails = std::collections::BTreeSet::new();
        loop {
            let detail = vault_core::get_refind_page_detail(
                &vault,
                &vault_core::RefindPageDetailRequest {
                    canonical_url: "https://example.com/".into(),
                    date_range: DateRange { start: "2026-01-01".into(), end: "2026-05-01".into() },
                    profile_id: Some("profile".into()),
                    page_size: Some(7),
                    visits_cursor,
                    trails_cursor,
                },
            )
            .unwrap();
            assert_eq!(detail.recent_days.len(), 113);
            for id in detail.explanation.visit_ids {
                assert!(visits.insert(id));
            }
            for trail in detail.related_trails.items {
                assert!(trails.insert(trail.trail_id));
            }
            visits_cursor = detail.explanation.visit_ids_next_cursor;
            trails_cursor = detail.related_trails.next_cursor;
            if visits_cursor.is_none() && trails_cursor.is_none() {
                break;
            }
        }
        assert_eq!(visits.len(), 113);
        assert_eq!(trails.len(), 113);
    }
}

#[test]
fn query_family_details_page_through_every_related_trail() {
    for key in [None, Some([96; 32])] {
        let vault = Vault::open_temp_with_key(key).unwrap();
        vault.store().write(Db::Intelligence,Priority::Batch,|s| {
            s.execute("INSERT INTO intelligence_generations(profile_id) VALUES('profile')",())?;
            s.execute("INSERT INTO query_families_rows(family_id,profile_id,anchor_query,member_count,search_engine,first_seen_ms,last_seen_ms,queries_json,computed_at) VALUES('family','profile','query',37,'google',1767268800000,1767268800000,'[\"query\"]','')",())?;
            for id in 1..=37_i64 {
                let trail=format!("trail:{id}");
                s.execute("INSERT INTO search_trails_rows(trail_id,profile_id,initial_query,search_engine,visit_count,first_visit_ms,last_visit_ms,queries_json,computed_at) VALUES(?1,'profile','query','google',1,1767268800000,1767268800000,'[]','')",[trail.clone()])?;
                s.execute("INSERT INTO search_events_rows(visit_id,profile_id,search_engine,raw_query,normalized_query,query_kind,trail_id,computed_at,visit_time_ms) VALUES(?1,'profile','google','query','query','keyword',?2,'',1767268800000)",params![id,trail])?;
            }Ok(())
        }).unwrap();
        let mut cursor = None;
        let mut seen = std::collections::BTreeSet::new();
        loop {
            let detail = vault_core::get_query_family_detail(
                &vault,
                &vault_core::QueryFamilyDetailRequest {
                    family_id: "family".into(),
                    date_range: DateRange { start: "2026-01-01".into(), end: "2026-01-02".into() },
                    profile_id: None,
                    page_size: Some(7),
                    cursor,
                },
            )
            .unwrap();
            for item in detail.related_trails.items {
                assert!(seen.insert(item.trail_id));
            }
            cursor = detail.related_trails.next_cursor;
            if cursor.is_none() {
                break;
            }
        }
        assert_eq!(seen.len(), 37);
    }
}

#[test]
fn recent_queries_have_complete_cursors_for_every_sort() {
    for key in [None, Some([103; 32])] {
        let vault = Vault::open_temp_with_key(key).unwrap();
        vault.store().write(Db::Archive,Priority::Batch,|s| {
            s.execute("INSERT INTO source_profiles(id,browser_kind,profile_name,profile_path,profile_key,discovered_at) VALUES(1,'chrome','Default','/fixture','profile','')",())?;
            for id in 1..=113_i64 {
                let url=format!("https://google.com/search?q=query{id}");
                s.execute("INSERT INTO pages(id,url,canonical_url,registrable_domain) VALUES(?1,?2,?2,'google.com')",params![id,url.clone()])?;
                s.execute("INSERT INTO urls(id,page_id,url,source_profile_id,created_by_run_id,first_visit_ms,first_visit_iso,last_visit_ms,last_visit_iso) VALUES(?1,?1,?2,1,0,1767268800000,'',1767268800000,'')",params![id,url])?;
                s.execute("INSERT INTO visits(id,url_id,source_profile_id,created_by_run_id,visit_time_ms,visit_time_iso) VALUES(?1,?1,1,0,?2,'')",params![id,1767268800000+id%7])?;
            }Ok(())
        }).unwrap();
        vault.store().write(Db::Intelligence,Priority::Batch,|s| {
            s.execute("INSERT INTO intelligence_generations(profile_id) VALUES('profile')",())?;
            for id in 1..=113_i64 {
                let query=format!("query{id}");
                s.execute("INSERT INTO visit_derived_facts_rows(visit_id,profile_id,canonical_url,registrable_domain,is_search_event,computed_at) VALUES(?1,'profile',?2,'google.com',1,'')",params![id,format!("https://google.com/search?q={query}")])?;
                s.execute("INSERT INTO search_events_rows(visit_id,profile_id,search_engine,raw_query,normalized_query,query_kind,computed_at,visit_time_ms) VALUES(?1,'profile','google',?2,?2,'keyword','',?3)",params![id,query.clone(),1767268800000+id%7])?;
                s.execute("INSERT INTO query_families_rows(family_id,profile_id,anchor_query,member_count,search_engine,first_seen_ms,last_seen_ms,queries_json,computed_at) VALUES(?1,'profile',?2,?3,'google',1767268800000,1767268800000,?4,'')",params![format!("family:{id}"),query.clone(),id%5+1,serde_json::json!([query]).to_string()])?;
            }Ok(())
        }).unwrap();
        for sort in ["newest", "exact-frequency", "family-frequency", "alphabetical"] {
            let mut cursor = None;
            let mut seen = std::collections::BTreeSet::new();
            loop {
                let page = vault_core::get_search_queries(
                    &vault,
                    &vault_core::SearchQueryListRequest {
                        date_range: DateRange {
                            start: "2026-01-01".into(),
                            end: "2026-01-02".into(),
                        },
                        sort: Some(sort.into()),
                        cursor,
                        page_size: 7,
                        ..Default::default()
                    },
                )
                .unwrap();
                assert_eq!(page.total, 113);
                assert!(page.rows.len() <= 7);
                for row in page.rows {
                    assert!(seen.insert(row.normalized_query));
                }
                cursor = page.next_cursor;
                if cursor.is_none() {
                    break;
                }
            }
            assert_eq!(seen.len(), 113, "{sort}");
        }
    }
}

#[test]
fn session_and_trail_details_page_in_navigation_order() {
    for key in [None, Some([104; 32])] {
        let vault = Vault::open_temp_with_key(key).unwrap();
        vault.store().write(Db::Archive,Priority::Batch,|s| {
            s.execute("INSERT INTO source_profiles(id,browser_kind,profile_name,profile_path,profile_key,discovered_at) VALUES(1,'chrome','Default','/fixture','profile','')",())?;
            s.execute("INSERT INTO pages(id,url,canonical_url) VALUES(1,'https://example.com/','https://example.com/')",())?;
            s.execute("INSERT INTO urls(id,page_id,url,source_profile_id,created_by_run_id,first_visit_ms,first_visit_iso,last_visit_ms,last_visit_iso) VALUES(1,1,'https://example.com/',1,0,1767268800000,'',1767268913000,'')",())?;
            for id in 1..=113_i64 {s.execute("INSERT INTO visits(id,url_id,source_profile_id,created_by_run_id,visit_time_ms,visit_time_iso) VALUES(?1,1,1,0,?2,'')",params![id,1767268800000+(114-id)*1000])?;}Ok(())
        }).unwrap();
        vault.store().write(Db::Intelligence,Priority::Batch,|s| {
            s.execute("INSERT INTO intelligence_generations(profile_id) VALUES('profile')",())?;
            s.execute("INSERT INTO sessions_rows(session_id,profile_id,first_visit_ms,last_visit_ms,visit_count,search_count,domain_count,computed_at) VALUES('session','profile',1767268800000,1767268913000,113,1,1,'')",())?;
            s.execute("INSERT INTO search_trails_rows(trail_id,profile_id,session_id,initial_query,search_engine,visit_count,first_visit_ms,last_visit_ms,queries_json,computed_at) VALUES('trail','profile','session','query','google',113,1767268800000,1767268913000,'[]','')",())?;
            for id in 1..=113_i64 {
                s.execute("INSERT INTO visit_derived_facts_rows(visit_id,profile_id,canonical_url,registrable_domain,computed_at) VALUES(?1,'profile','https://example.com/','example.com','')",[id])?;
                s.execute("INSERT INTO visit_assignments(visit_id,profile_id,session_id,trail_id,ordinal) VALUES(?1,'profile','session','trail',?2)",params![id,113-id])?;
                s.execute("INSERT INTO search_trail_members_rows(trail_id,profile_id,visit_id,ordinal,role) VALUES('trail','profile',?1,?2,'result')",params![id,113-id])?;
            }Ok(())
        }).unwrap();
        for kind in ["session", "trail"] {
            let mut cursor = None;
            let mut ids = Vec::new();
            loop {
                let (page, next) = if kind == "session" {
                    let page = vault_core::get_session_detail(
                        &vault,
                        &vault_core::SessionDetailRequest {
                            session_id: "session".into(),
                            visits_cursor: cursor,
                            page_size: Some(7),
                            ..Default::default()
                        },
                    )
                    .unwrap();
                    (
                        page.visits.into_iter().map(|r| r.visit_id).collect::<Vec<_>>(),
                        page.visits_next_cursor,
                    )
                } else {
                    let page = vault_core::get_trail_detail(
                        &vault,
                        &vault_core::TrailDetailRequest {
                            trail_id: "trail".into(),
                            cursor,
                            page_size: Some(7),
                        },
                    )
                    .unwrap();
                    (
                        page.members.into_iter().map(|r| r.visit_id).collect::<Vec<_>>(),
                        page.members_next_cursor,
                    )
                };
                assert!(page.len() <= 7);
                ids.extend(page);
                cursor = next;
                if cursor.is_none() {
                    break;
                }
            }
            assert_eq!(ids, (1..=113).rev().collect::<Vec<_>>());
        }
    }
}

#[test]
fn anniversary_domains_continue_and_deep_sessions_are_counted_once() {
    use chrono::Datelike;
    for key in [None, Some([105; 32])] {
        let vault = Vault::open_temp_with_key(key).unwrap();
        let date =
            chrono::Local::now().date_naive().with_year(chrono::Local::now().year() - 4).unwrap();
        let date_key = date.to_string();
        let seed_date = date_key.clone();
        let noon = date.and_hms_opt(12, 0, 0).unwrap().and_utc().timestamp_millis();
        vault.store().write(Db::Intelligence,Priority::Batch,move |s| {
            s.execute("INSERT INTO intelligence_generations(profile_id) VALUES('profile')",())?;
            s.execute("INSERT INTO daily_summary_rollups_rows(date_key,profile_id,total_visits,total_searches,new_domains,unique_domains) VALUES(?1,'profile',113,0,0,113)",[seed_date.clone()])?;
            s.execute("INSERT INTO sessions_rows(session_id,profile_id,first_visit_ms,last_visit_ms,visit_count,search_count,domain_count,is_deep_dive,computed_at) VALUES('deep','profile',?1,?2,1000,0,113,1,'')",params![noon-86400000,noon+86400000])?;
            for id in 1..=113_i64 {s.execute("INSERT INTO domain_daily_rollups_rows(date_key,profile_id,registrable_domain,domain_category,visit_count,search_count,new_domain_visits,unique_urls) VALUES(?1,'profile',?2,'reference',1,0,0,1)",params![seed_date.clone(),format!("site{id}.test")])?;}Ok(())
        }).unwrap();
        let anniversaries = vault_core::get_on_this_day(&vault, None).unwrap();
        assert_eq!(anniversaries.len(), 1);
        let entry = &anniversaries[0];
        assert_eq!(entry.total_visits, 113);
        assert_eq!(entry.deep_dive_sessions, 1);
        let mut seen = entry.top_domains.iter().cloned().collect::<std::collections::BTreeSet<_>>();
        let mut cursor = entry.top_domains_next_cursor.clone();
        while cursor.is_some() {
            let page = get_top_sites(
                &vault,
                &TopSitesRequest {
                    date_range: DateRange { start: date_key.clone(), end: date_key.clone() },
                    profile_id: None,
                    profile_ids: Vec::new(),
                    sort_by: Some("visit_count".into()),
                    limit: Some(7),
                    cursor,
                },
            )
            .unwrap();
            for item in page.items {
                assert!(seen.insert(item.registrable_domain));
            }
            cursor = page.next_cursor;
        }
        assert_eq!(seen.len(), 113);
    }
}

#[test]
fn curated_cards_and_widgets_expose_every_selected_card() {
    for key in [None, Some([106; 32])] {
        let vault = Vault::open_temp_with_key(key).unwrap();
        let mut cursor = None;
        let mut cards = Vec::new();
        loop {
            let page = vault_core::get_intelligence_embed_cards(
                &vault,
                &vault_core::IntelligenceEmbedCardsRequest {
                    date_range: DateRange { start: "2026-01-01".into(), end: "2026-01-02".into() },
                    profile_id: None,
                    limit: Some(1),
                    cursor,
                },
            )
            .unwrap();
            cards.extend(page.items.into_iter().map(|r| r.card_id));
            cursor = page.next_cursor;
            if cursor.is_none() {
                break;
            }
        }
        assert_eq!(cards, vec!["digest:visits", "digest:searches"]);
        let widget = vault_core::get_intelligence_widget_snapshot(
            &vault,
            &vault_core::IntelligenceEmbedCardsRequest {
                date_range: DateRange { start: "2026-01-01".into(), end: "2026-01-02".into() },
                profile_id: None,
                limit: Some(1),
                cursor: None,
            },
        )
        .unwrap();
        assert_eq!(widget.highlights.len(), 1);
        assert!(widget.highlights_next_cursor.is_some());
    }
}
