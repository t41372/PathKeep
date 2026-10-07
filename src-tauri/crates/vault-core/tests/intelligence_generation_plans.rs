//! Responsibilities: protect indexed generation and assignment joins.
//! Not responsible for: rebuild math or archive ingest.
//! Failure modes: joining published generations multiplies work per fact.
use vault_core::Vault;
use vault_store::Db;
#[test]
fn generation_views_seek_assignment_keys() {
    let vault = Vault::open_temp().unwrap();
    let plan=vault.store().read(Db::Intelligence,|s| {
        s.query("EXPLAIN QUERY PLAN SELECT visit_id,profile_id,session_id,trail_id,canonical_url,is_new_domain,is_search_event,search_query FROM visit_derived_facts ORDER BY visit_id",(),|r|Ok(r.get::<String>(3)?))
    }).unwrap();
    eprintln!("generation plan: {plan:?}");
    assert!(plan.iter().any(|s| s.contains("SEARCH a") && s.contains("visit_id=?")), "{plan:?}");
}

#[test]
fn derived_batch_looks_up_only_requested_visit_ids() {
    let vault = Vault::open_temp().unwrap();
    vault.store().write(Db::Archive,vault_store::Priority::Batch,|s| {
        s.execute("INSERT INTO source_profiles(id,browser_kind,profile_name,profile_path,profile_key,discovered_at) VALUES(1,'chrome','Default','/fixture','profile','')",())?;
        s.execute("INSERT INTO pages(id,url,canonical_url,registrable_domain) VALUES(1,'https://example.test/','https://example.test/','example.test')",())?;
        s.execute("INSERT INTO urls(id,page_id,url,source_profile_id,created_by_run_id,first_visit_ms,first_visit_iso,last_visit_ms,last_visit_iso) VALUES(1,1,'https://example.test/',1,0,0,'',0,'')",())?;
        for id in 1..=7_i64 {s.execute("INSERT INTO visits(id,url_id,source_profile_id,created_by_run_id,visit_time_ms,visit_time_iso) VALUES(?1,1,1,0,1700000000000+?1,'')",[id])?;}
        s.execute("UPDATE archive_totals SET value=7 WHERE name='visits'",())?;
        s.execute("INSERT INTO archive_totals(name,value) VALUES('sourceVisits:1',7)",())?;Ok(())
    }).unwrap();
    vault.store().capture_query_plans(Db::Intelligence, true).unwrap();
    vault_core::run_core_intelligence(
        &vault,
        &vault_core::CoreIntelligenceRebuildRequest { limit: Some(3), ..Default::default() },
    )
    .unwrap();
    let plans = vault.store().take_query_plans(Db::Intelligence).unwrap();
    let plans =
        plans.iter().filter(|p| p.sql.contains("WITH requested(visit_id)")).collect::<Vec<_>>();
    assert!(!plans.is_empty());
    for plan in plans {
        eprintln!("derived batch plan: {:?}", plan.steps);
        assert!(
            plan.steps.iter().any(|s| s.contains("SEARCH f") && s.contains("visit_id=?")),
            "{:?}",
            plan.steps
        );
    }
}
