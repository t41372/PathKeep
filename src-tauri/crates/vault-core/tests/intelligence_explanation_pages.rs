//! Responsibilities: keep entity evidence pages complete and bounded.
//! Not responsible for: classification or frontend presentation.
//! Failure modes: explanations load every ID, continuation loses ordinal ties,
//! or a page changes whole-entity factors.
use vault_core::{EntityExplanationRequest, Vault, explain_entity};
use vault_store::{Db, Priority, params};

#[test]
fn entity_evidence_pages_preserve_every_visit_and_whole_entity_factors() {
    for key in [None, Some([107; 32])] {
        let vault = Vault::open_temp_with_key(key).unwrap();
        vault.store().write(Db::Archive,Priority::Batch,|s| {
            s.execute("INSERT INTO source_profiles(id,browser_kind,profile_name,profile_path,profile_key,discovered_at) VALUES(1,'chrome','Default','/fixture','profile','')",())?;
            s.execute("INSERT INTO pages(id,url,canonical_url) VALUES(1,'https://example.com/','https://example.com/')",())?;
            s.execute("INSERT INTO urls(id,page_id,url,source_profile_id,created_by_run_id,first_visit_ms,first_visit_iso,last_visit_ms,last_visit_iso) VALUES(1,1,'https://example.com/',1,0,0,'',0,'')",())?;
            for id in 1..=113_i64 {s.execute("INSERT INTO visits(id,url_id,source_profile_id,created_by_run_id,visit_time_ms,visit_time_iso) VALUES(?1,1,1,0,?1,'')",[id])?;}Ok(())
        }).unwrap();
        vault.store().write(Db::Intelligence,Priority::Batch,|s| {
            s.execute("INSERT INTO intelligence_generations(profile_id) VALUES('profile')",())?;
            s.execute("INSERT INTO sessions_rows(session_id,profile_id,first_visit_ms,last_visit_ms,visit_count,search_count,domain_count,computed_at) VALUES('session','profile',1,113,113,113,1,'')",())?;
            s.execute("INSERT INTO search_trails_rows(trail_id,profile_id,session_id,initial_query,search_engine,visit_count,first_visit_ms,last_visit_ms,queries_json,computed_at) VALUES('trail','profile','session','query','google',113,1,113,'[]','')",())?;
            s.execute("INSERT INTO query_families_rows(family_id,profile_id,anchor_query,member_count,search_engine,first_seen_ms,last_seen_ms,queries_json,computed_at) VALUES('family','profile','query',113,'google',1,113,'[\"query\"]','')",())?;
            // A weekly habit: a habit is worked out from its site's daily rollups.
            for day in ["2020-01-01","2020-01-08","2020-01-15","2020-01-22","2020-01-29"] {
                s.execute("INSERT INTO domain_daily_rollups_rows(profile_id,date_key,registrable_domain,domain_category,visit_count,search_count,new_domain_visits,unique_urls,last_visit_ms) VALUES('profile',?1,'example.com','unknown',1,0,0,1,0)",[day])?;
            }
            s.execute("INSERT INTO reopened_investigations_rows(investigation_id,profile_id,anchor_type,anchor_id,anchor_label,occurrence_count,distinct_days,first_seen_ms,last_seen_ms,evidence_json,computed_at) VALUES('reopened','profile','query_family','family','query',113,113,1,113,'{}','')",())?;
            for id in 1..=113_i64 {
                s.execute("INSERT INTO visit_derived_facts_rows(visit_id,profile_id,canonical_url,registrable_domain,page_category,computed_at) VALUES(?1,'profile',?2,'example.com','article','')",params![id,format!("https://example.com/{}",id%3)])?;
                s.execute("INSERT INTO visit_assignments(visit_id,profile_id,session_id,trail_id,ordinal) VALUES(?1,'profile','session','trail',?2)",params![id,id/3])?;
                s.execute("INSERT INTO search_trail_members_rows(trail_id,profile_id,visit_id,ordinal,role) VALUES('trail','profile',?1,?2,'result')",params![id,id/3])?;
                s.execute("INSERT INTO search_events_rows(visit_id,profile_id,search_engine,raw_query,normalized_query,query_kind,trail_id,computed_at,visit_time_ms) VALUES(?1,'profile','google','query','query','keyword','trail','',?1)",[id])?;
            }Ok(())
        }).unwrap();
        vault.store().capture_query_plans(Db::Intelligence, true).unwrap();
        for (kind, id) in [
            ("session", "session"),
            ("search_trail", "trail"),
            ("query_family", "family"),
            ("habit_pattern", "profile::example.com"),
            ("reopened_investigation", "reopened"),
            ("compare_set", "compare:trail:article"),
        ] {
            let mut cursor = None;
            let mut seen = Vec::new();
            let mut factors = None;
            loop {
                let page = explain_entity(
                    &vault,
                    &EntityExplanationRequest {
                        entity_type: kind.into(),
                        entity_id: id.into(),
                        cursor,
                        page_size: Some(7),
                    },
                )
                .unwrap();
                assert!(page.participating_visit_ids.len() <= 7, "{kind}");
                for plan in vault.store().take_query_plans(Db::Intelligence).unwrap() {
                    assert!(
                        !plan.steps.iter().any(|step| step == "SCAN visits"
                            || step == "SCAN archive.visits"
                            || step == "SCAN v"),
                        "{kind}: {plan:?}"
                    );
                }
                let current = serde_json::to_value(&page.factors).unwrap();
                if let Some(previous) = &factors {
                    assert_eq!(previous, &current, "{kind}");
                }
                factors = Some(current);
                seen.extend(page.participating_visit_ids);
                cursor = page.participating_visit_ids_next_cursor;
                if cursor.is_none() {
                    break;
                }
            }
            assert_eq!(seen, (1..=113_i64).collect::<Vec<_>>(), "{kind}");
        }
    }
}
