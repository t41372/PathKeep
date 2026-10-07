//! Responsibilities: verify deterministic builds through the native owner.
//! Not responsible for: provider calls, frontend rendering or scale measurements.
//! Failure modes: batch boundaries change facts, restart duplicates work, a rebuild
//! monopolizes interactive writes, incremental results diverge, or cancellation loses its cursor.

use vault_core::{
    CoreIntelligenceRebuildRequest, Vault, run_core_intelligence,
    run_core_intelligence_with_progress,
};
use vault_store::{Db, Priority, params};

fn seed(vault: &Vault, first: i64, last: i64) {
    vault.store().write(Db::Archive,Priority::Batch,move |s| {
        s.execute("INSERT INTO source_profiles(id,browser_kind,profile_name,profile_path,discovered_at,enabled,profile_key) VALUES(1,'chrome','Default','/fixture','',1,'chrome:Default') ON CONFLICT(id) DO NOTHING",())?;
        let mut page=s.prepare("INSERT INTO pages(id,url,canonical_url,registrable_domain) VALUES(?1,?2,?2,?3) ON CONFLICT(id) DO NOTHING")?;
        let mut url=s.prepare("INSERT INTO urls(id,page_id,url,title,first_visit_ms,first_visit_iso,last_visit_ms,last_visit_iso,source_profile_id,created_by_run_id,source_url_id) VALUES(?1,?1,?2,'Reference',0,'',0,'',1,0,?1) ON CONFLICT(id) DO NOTHING")?;
        for id in 1..=101 {
            let address=if id%10==0 {format!("https://www.google.com/search?q=topic+{}",id/10)} else {format!("https://docs.example.com/page/{id}")};
            let domain=if id%10==0 {"google.com"} else {"example.com"};
            page.execute(params![id,address.as_str(),domain])?;
            url.execute(params![id,address])?;
        }
        let mut visit=s.prepare("INSERT INTO visits(id,url_id,source_visit_id,visit_time_ms,visit_time_iso,source_profile_id,created_by_run_id,from_visit,transition_type) VALUES(?1,?2,?3,?4,'',1,0,?5,?6)")?;
        for id in first..=last {
            let time=1_700_000_000_000+(id/10)*3_600_000+(id%10)*60_000;
            visit.execute(params![id,1+(id%101),id.to_string(),time,if id%10==0 {None} else {Some(id-1)},if id%7==0 {1} else {0}])?;
        }
        s.execute("UPDATE archive_totals SET value=(SELECT COUNT(*) FROM visits) WHERE name='visits'",())?;
        s.execute("UPDATE archive_totals SET value=101 WHERE name='urls'",())?;
        s.execute("INSERT INTO archive_totals(name,value) VALUES('sourceVisits:1',(SELECT COUNT(*) FROM visits)) ON CONFLICT(name) DO UPDATE SET value=excluded.value",())?;
        Ok(())
    }).unwrap();
}

fn facts(vault: &Vault) -> Vec<Vec<String>> {
    eprintln!("fixture facts");
    vault.store().read(Db::Intelligence,|s| {
        s.query("SELECT visit_id,profile_id,session_id,trail_id,canonical_url,is_new_domain,is_search_event,search_query FROM visit_derived_facts ORDER BY visit_id",(),|r| {
            (0..8).map(|i|r.get_value(i).map(|v|format!("{v:?}")).map_err(Into::into)).collect()
        })
    }).unwrap()
}

fn derived_state(vault: &Vault) -> Vec<Vec<Vec<String>>> {
    vault.store().read(Db::Intelligence,|s| {
        let mut tables=Vec::new();
        for (table,columns) in [
            ("domain_daily_rollups","date_key,profile_id,registrable_domain,domain_category,visit_count,search_count,new_domain_visits,unique_urls"),
            ("category_daily_rollups","date_key,profile_id,domain_category,visit_count,unique_domains"),
            ("engine_daily_rollups","date_key,profile_id,search_engine,search_count"),
            ("daily_summary_rollups","date_key,profile_id,total_visits,total_searches,new_domains,unique_domains,hhi_score,discovery_rate"),
            ("sessions","session_id,profile_id,first_visit_ms,last_visit_ms,visit_count,search_count,domain_count,is_deep_dive,auto_title"),
            ("search_trails","trail_id,profile_id,session_id,initial_query,search_engine,reformulation_count,visit_count,landing_url,landing_domain,first_visit_ms,last_visit_ms,max_depth,queries_json"),
            ("search_trail_members","trail_id,profile_id,visit_id,ordinal,role"),
            ("search_events","visit_id,profile_id,search_engine,raw_query,normalized_query,query_kind,trail_id,visit_time_ms"),
            ("search_event_terms","visit_id,profile_id,term"),
            ("query_families","family_id,profile_id,anchor_query,member_count,search_engine,first_seen_ms,last_seen_ms,queries_json"),
            ("refind_pages","profile_id,canonical_url,url,title,registrable_domain,cross_day_count,trail_count,search_arrival_count,typed_revisit_count,refind_score,evidence_json,first_seen_ms,last_seen_ms"),
            ("source_effectiveness","profile_id,registrable_domain,source_role,trail_count,stable_landing_count,effectiveness_score,evidence_json,first_seen_ms,last_seen_ms"),
            ("habit_patterns","profile_id,registrable_domain,habit_type,mean_interval_days,cv,visit_count,last_visited_ms,is_interrupted"),
            ("reopened_investigations","investigation_id,profile_id,anchor_type,anchor_id,anchor_label,occurrence_count,distinct_days,first_seen_ms,last_seen_ms,evidence_json"),
            ("path_flows","profile_id,flow_pattern,step_count,occurrence_count,last_seen_ms"),
        ] {
            eprintln!("fixture table={table}");
            let count=columns.split(',').count();
            let order=(1..=count).map(|n|n.to_string()).collect::<Vec<_>>().join(",");
            tables.push(s.query(&format!("SELECT {columns} FROM {table} ORDER BY {order}"),(),|r| {
                (0..count).map(|i|r.get_value(i).map(|v|format!("{v:?}")).map_err(Into::into)).collect()
            })?);
        }
        Ok(tables)
    }).unwrap()
}

fn build(vault: &Vault, request: &CoreIntelligenceRebuildRequest) {
    let mut phase = String::new();
    run_core_intelligence_with_progress(vault, request, |p| {
        if phase != p.phase {
            eprintln!("fixture phase={} processed={:?}", p.phase, p.processed_items);
            phase = p.phase;
        }
        Ok(())
    })
    .unwrap();
}

#[test]
fn many_batches_equal_one_pass_and_incremental_equals_full() {
    for key in [None, Some([67; 32])] {
        let batched = Vault::open_temp_with_key(key).unwrap();
        let reference = Vault::open_temp_with_key(key).unwrap();
        seed(&batched, 1, 1313);
        seed(&reference, 1, 1313);
        build(
            &batched,
            &CoreIntelligenceRebuildRequest {
                limit: Some(47),
                full_rebuild: true,
                ..Default::default()
            },
        );
        build(
            &reference,
            &CoreIntelligenceRebuildRequest {
                limit: Some(4096),
                full_rebuild: true,
                ..Default::default()
            },
        );
        assert_eq!(facts(&batched).len(), 1313);
        assert_eq!(facts(&batched), facts(&reference));
        assert_eq!(derived_state(&batched), derived_state(&reference));
        seed(&batched, 1314, 11_313);
        seed(&reference, 1314, 11_313);
        build(&batched, &CoreIntelligenceRebuildRequest { limit: Some(113), ..Default::default() });
        build(
            &reference,
            &CoreIntelligenceRebuildRequest { full_rebuild: true, ..Default::default() },
        );
        assert_eq!(facts(&batched).len(), 11_313);
        assert_eq!(facts(&batched), facts(&reference));
        assert_eq!(derived_state(&batched), derived_state(&reference));
    }
}

#[test]
fn an_interrupted_build_resumes_and_an_interactive_star_commits_before_completion() {
    for key in [None, Some([68; 32])] {
        let vault = Vault::open_temp_with_key(key).unwrap();
        seed(&vault, 1, 1313);
        let request = CoreIntelligenceRebuildRequest {
            limit: Some(47),
            full_rebuild: true,
            ..Default::default()
        };
        let stopped = run_core_intelligence_with_progress(&vault, &request, |p| {
            if p.phase == "visit-derived-facts" && p.processed_items.is_some_and(|n| n >= 141) {
                anyhow::bail!("fixture interruption after committed batches");
            }
            Ok(())
        });
        assert!(stopped.is_err());
        let paths = vault.paths().clone();
        let config = vault.config().clone();
        vault.store().close().unwrap();
        let reopened = Vault::open(paths, config, key).unwrap();
        let (entered, ready) = std::sync::mpsc::channel();
        let (release, wait) = std::sync::mpsc::channel();
        let worker = reopened.clone();
        let handle = std::thread::spawn(move || {
            let mut paused = false;
            run_core_intelligence_with_progress(&worker, &request, |p| {
                if p.phase == "visit-derived-facts" && !paused {
                    assert!(p.processed_items.unwrap() >= 141);
                    paused = true;
                    entered.send(()).unwrap();
                    wait.recv().unwrap();
                }
                Ok(())
            })
            .unwrap()
        });
        ready.recv_timeout(std::time::Duration::from_secs(30)).unwrap();
        vault_core::set_star(
            &reopened,
            vault_core::SetStarRequest {
                entity_kind: vault_core::StarEntityKind::Url,
                entity_key: "https://docs.example.com/page/1".into(),
                source_profile: None,
            },
        )
        .unwrap();
        assert!(!handle.is_finished());
        release.send(()).unwrap();
        handle.join().unwrap();
        assert_eq!(facts(&reopened).len(), 1313);
    }
}

#[test]
fn process_exit_mid_rebuild_resumes_without_duplicate_facts() {
    if let Ok(root) = std::env::var("PATHKEEP_INTELLIGENCE_CRASH_CHILD") {
        let key = (std::env::var("PATHKEEP_INTELLIGENCE_CRASH_KEY").as_deref() == Ok("1"))
            .then_some([74; 32]);
        let vault = Vault::open(
            vault_core::project_paths_with_root(std::path::Path::new(&root)),
            Default::default(),
            key,
        )
        .unwrap();
        run_core_intelligence_with_progress(
            &vault,
            &CoreIntelligenceRebuildRequest {
                limit: Some(47),
                full_rebuild: true,
                ..Default::default()
            },
            |p| {
                if p.phase == "visit-derived-facts" && p.processed_items.is_some_and(|n| n >= 141) {
                    std::process::exit(23);
                }
                Ok(())
            },
        )
        .unwrap();
        panic!("child should exit after a committed batch");
    }
    for key in [None, Some([74; 32])] {
        let vault = Vault::open_temp_with_key(key).unwrap();
        seed(&vault, 1, 1313);
        let paths = vault.paths().clone();
        let config = vault.config().clone();
        vault.store().close().unwrap();
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "process_exit_mid_rebuild_resumes_without_duplicate_facts",
                "--nocapture",
            ])
            .env("PATHKEEP_INTELLIGENCE_CRASH_CHILD", &paths.app_root)
            .env("PATHKEEP_INTELLIGENCE_CRASH_KEY", if key.is_some() { "1" } else { "0" })
            .status()
            .unwrap();
        assert_eq!(status.code(), Some(23));
        let reopened = Vault::open(paths, config, key).unwrap();
        let first = std::cell::Cell::new(None);
        run_core_intelligence_with_progress(
            &reopened,
            &CoreIntelligenceRebuildRequest {
                limit: Some(47),
                full_rebuild: true,
                ..Default::default()
            },
            |p| {
                if first.get().is_none() && p.phase == "visit-derived-facts" {
                    first.set(p.processed_items);
                }
                Ok(())
            },
        )
        .unwrap();
        assert_eq!(first.get(), Some(141));
        let rows = facts(&reopened);
        assert_eq!(rows.len(), 1313);
        let ids = rows.iter().map(|r| r[0].clone()).collect::<std::collections::BTreeSet<_>>();
        assert_eq!(ids.len(), 1313);
    }
}

#[test]
fn sessions_that_started_more_than_a_day_earlier_still_overlap_the_requested_day() {
    for key in [None, Some([75; 32])] {
        let vault = Vault::open_temp_with_key(key).unwrap();
        seed(&vault, 1, 1);
        vault.store().write(Db::Intelligence,Priority::Batch,|s| {
            s.execute("INSERT INTO intelligence_generations(profile_id) VALUES('chrome:Default')",())?;
            s.execute("INSERT INTO sessions_rows(generation,session_id,profile_id,first_visit_ms,last_visit_ms,visit_count,search_count,domain_count,is_deep_dive,auto_title,computed_at) VALUES(0,'long-session','chrome:Default',1700000000000,1700432000000,1000,0,1,1,'Long session','')",())?;Ok(())
        }).unwrap();
        let result = vault_core::get_sessions(
            &vault,
            &vault_core::PagedDateRangeRequest {
                cursor: None,
                date_range: vault_core::DateRange {
                    start: "2023-11-18".into(),
                    end: "2023-11-18".into(),
                },
                profile_id: Some("chrome:Default".into()),

                page_size: 10,
            },
        )
        .unwrap();
        assert_eq!(result.total, 1);
        assert_eq!(result.sessions[0].session_id, "long-session");
    }
}

#[path = "intelligence_turso/render_plans.rs"]
mod render_plans;

#[test]
fn malformed_job_claim_commits_its_quarantine() {
    for key in [None, Some([84; 32])] {
        let vault = Vault::open_temp_with_key(key).unwrap();
        vault.store().write(Db::Intelligence,Priority::Interactive,|s| {
            s.execute("INSERT INTO intelligence_jobs(id,job_type,state,dedupe_key,payload_json,created_at,scheduled_at,updated_at) VALUES(1,'full-rebuild','queued','bad','{','','','')",())?;Ok(())
        }).unwrap();
        assert!(vault_core::intelligence_runtime::claim_core_intelligence_job(&vault, 1).is_err());
        let state = vault
            .store()
            .read(Db::Intelligence, |s| {
                s.query_one("SELECT state FROM intelligence_jobs WHERE id=1", (), |r| {
                    Ok(r.get::<String>(0)?)
                })
            })
            .unwrap();
        assert_eq!(state, "failed");
        assert!(
            vault_core::intelligence_runtime::next_queued_intelligence_job(&vault)
                .unwrap()
                .is_none()
        );
    }
}

#[test]
fn a_changed_archive_retargets_an_interrupted_generation() {
    for key in [None, Some([91; 32])] {
        let vault = Vault::open_temp_with_key(key).unwrap();
        let reference = Vault::open_temp_with_key(key).unwrap();
        seed(&vault, 1, 131);
        seed(&reference, 1, 250);
        let stopped = run_core_intelligence_with_progress(
            &vault,
            &CoreIntelligenceRebuildRequest { limit: Some(47), ..Default::default() },
            |p| {
                if p.phase == "visit-derived-facts" && p.processed_items.is_some_and(|n| n >= 47) {
                    anyhow::bail!("interrupt before a backup");
                }
                Ok(())
            },
        );
        assert!(stopped.is_err());
        seed(&vault, 132, 250);
        run_core_intelligence(&vault, &CoreIntelligenceRebuildRequest::default()).unwrap();
        run_core_intelligence(&reference, &CoreIntelligenceRebuildRequest::default()).unwrap();
        assert_eq!(facts(&vault), facts(&reference));
        assert_eq!(derived_state(&vault), derived_state(&reference));
        // A completed facts stage also must not pin an obsolete source watermark.
        seed(&vault, 251, 260);
        vault_core::intelligence::run_core_intelligence_job_type_with_progress(
            &vault,
            "visit-derive",
            &CoreIntelligenceRebuildRequest::default(),
            |_| Ok(()),
        )
        .unwrap();
        vault.store().write(Db::Archive,Priority::Batch,|s| {
            s.execute("UPDATE visits SET reverted_at='now'",())?;
            s.execute("UPDATE archive_totals SET value=0 WHERE name='visits' OR name LIKE 'sourceVisits:%'",())?;Ok(())
        }).unwrap();
        let report =
            run_core_intelligence(&vault, &CoreIntelligenceRebuildRequest::default()).unwrap();
        assert_eq!(report.execution_mode.as_deref(), Some("noop"));
        assert!(facts(&vault).is_empty());
    }
}

#[test]
fn a_long_session_resumes_from_durable_domain_counts() {
    for key in [None, Some([97; 32])] {
        let vault = Vault::open_temp_with_key(key).unwrap();
        seed(&vault, 1, 1313);
        vault
            .store()
            .write(Db::Archive, Priority::Batch, |s| {
                s.execute("UPDATE visits SET visit_time_ms=1700000000000+id*10000", ())?;
                Ok(())
            })
            .unwrap();
        run_core_intelligence(&vault, &CoreIntelligenceRebuildRequest::default()).unwrap();
        seed(&vault, 1314, 1323);
        vault
            .store()
            .write(Db::Archive, Priority::Batch, |s| {
                s.execute(
                    "UPDATE visits SET visit_time_ms=1700000000000+id*10000 WHERE id>1313",
                    (),
                )?;
                Ok(())
            })
            .unwrap();
        vault.store().capture_query_plans(Db::Intelligence, true).unwrap();
        run_core_intelligence(&vault, &CoreIntelligenceRebuildRequest::default()).unwrap();
        let plans = vault.store().take_query_plans(Db::Intelligence).unwrap();
        assert!(plans.iter().any(|p| p.sql.contains("FROM session_domains")));
        assert!(!plans.iter().any(|p| {
            p.sql.contains("SELECT a.visit_id,f.registrable_domain FROM visit_assignments")
        }));
        vault.store().read(Db::Intelligence,|s| {
            assert_eq!(s.query_one("SELECT SUM(visit_count) FROM session_domains WHERE generation=(SELECT navigation FROM intelligence_generations WHERE profile_id='chrome:Default')",(),|r|Ok(r.get::<i64>(0)?))?,1323);
            assert_eq!(s.query_one("SELECT domain_count FROM sessions",(),|r|Ok(r.get::<i64>(0)?))?,2);
            Ok(())
        }).unwrap();
    }
}

#[test]
fn work_requested_during_a_claim_is_not_lost_at_completion() {
    use vault_core::intelligence_runtime::*;
    for key in [None, Some([98; 32])] {
        let vault = Vault::open_temp_with_key(key).unwrap();
        let request = CoreIntelligenceRebuildRequest::default();
        let id = enqueue_core_intelligence_job(&vault, "full-rebuild", &request, "first backup")
            .unwrap();
        assert!(claim_core_intelligence_job(&vault, id).unwrap().is_some());
        assert_eq!(
            enqueue_core_intelligence_job(&vault, "full-rebuild", &request, "second backup")
                .unwrap(),
            id
        );
        assert!(mark_intelligence_job_succeeded(&vault, id, &serde_json::json!({})).unwrap());
        assert_eq!(next_queued_intelligence_job(&vault).unwrap().unwrap().id, id);
        assert!(claim_core_intelligence_job(&vault, id).unwrap().is_some());
        assert!(mark_intelligence_job_succeeded(&vault, id, &serde_json::json!({})).unwrap());
        assert!(next_queued_intelligence_job(&vault).unwrap().is_none());
    }
}

#[test]
fn a_rebuild_does_not_overwrite_a_newer_invalidation() {
    for key in [None, Some([99; 32])] {
        let vault = Vault::open_temp_with_key(key).unwrap();
        seed(&vault, 1, 131);
        let mut invalidated = false;
        run_core_intelligence_with_progress(
            &vault,
            &CoreIntelligenceRebuildRequest { limit: Some(47), ..Default::default() },
            |p| {
                if !invalidated
                    && p.phase == "visit-derived-facts"
                    && p.processed_items.is_some_and(|n| n >= 47)
                {
                    invalidated = true;
                    vault_core::intelligence_runtime::mark_all_deterministic_modules_stale(
                        &vault,
                        "fixture-newer-change",
                        "A newer source change awaits the next build.",
                    )?;
                }
                Ok(())
            },
        )
        .unwrap();
        let runtime = vault_core::intelligence_runtime::load_intelligence_runtime(&vault).unwrap();
        let sessions = runtime.modules.iter().find(|m| m.module_id == "sessions").unwrap();
        assert_eq!(sessions.status, "stale");
        run_core_intelligence(&vault, &CoreIntelligenceRebuildRequest::default()).unwrap();
        let runtime = vault_core::intelligence_runtime::load_intelligence_runtime(&vault).unwrap();
        assert_eq!(
            runtime.modules.iter().find(|m| m.module_id == "sessions").unwrap().status,
            "ready"
        );
    }
}
