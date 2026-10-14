//! Responsibilities: prove candidate coverage and canonical identity on native pages.
//! Not responsible for: providers, network fetches or model ranking.
//! Failure modes: the old 12,500/50,000 pools omit pages, URL variants duplicate
//! candidates, or an old starred/noted page is excluded by recency.

use vault_core::{Vault, WorkingSetConfig, select_working_set};
use vault_store::{Db, Priority, params};

#[test]
fn every_page_is_reachable_past_both_old_pool_limits() {
    for key in [None, Some([66; 32])] {
        let vault = Vault::open_temp_with_key(key).unwrap();
        vault.store().write(Db::Archive,Priority::Batch,|s| {
            s.execute("INSERT INTO source_profiles(id,browser_kind,profile_name,profile_path,discovered_at,enabled,profile_key) VALUES(1,'chrome','Default','/fixture','',1,'p')",())?;
            Ok(())
        }).unwrap();
        for first in (1..=50_021).step_by(1000) {
            vault.store().write(Db::Archive,Priority::Batch,move |s| {
                for id in first..(first+1000).min(50_022) {
                    let url=format!("https://fixture.test/{id}");
                    s.execute("INSERT INTO pages(id,url,canonical_url,registrable_domain) VALUES(?1,?2,?2,'fixture.test')",params![id,url.as_str()])?;
                    s.execute("INSERT INTO urls(id,page_id,url,title,first_visit_ms,first_visit_iso,last_visit_ms,last_visit_iso,source_profile_id,created_by_run_id) VALUES(?1,?1,?2,'Page',0,'',0,'',1,0)",params![id,url.as_str()])?;
                    s.execute("INSERT INTO visits(id,url_id,source_profile_id,visit_time_ms,visit_time_iso,created_by_run_id) VALUES(?1,?1,1,0,'',0)",[id])?;
                    s.execute("INSERT INTO search_documents(page_id,url,title,search_terms,normalized_url,normalized_title,normalized_search_terms,cjk_grams,compact_text,newest_visit_ms,updated_at) VALUES(?1,?2,'Page','',?2,'page','','','',0,'')",params![id,url])?;
                    s.execute("INSERT INTO search_document_profiles VALUES(?1,1,1,0)",[id])?;
                }
                Ok(())
            }).unwrap();
        }
        vault
            .store()
            .write(Db::Archive, Priority::Interactive, |s| {
                s.execute(
                    "INSERT INTO star VALUES('url','https://fixture.test/50021','',NULL)",
                    (),
                )?;
                Ok(())
            })
            .unwrap();
        let mut cursor = None;
        let mut keys = std::collections::BTreeSet::new();
        let mut found_old_star = false;
        loop {
            let page = select_working_set(
                &vault,
                &WorkingSetConfig::default(),
                1_800_000_000_000,
                997,
                cursor,
            )
            .unwrap();
            for row in page.items {
                if row.canonical_url == "https://fixture.test/50021" {
                    found_old_star = row.signals.starred && !row.signals.recent;
                }
                assert!(keys.insert(row.canonical_url));
            }
            cursor = page.next_cursor;
            if cursor.is_none() {
                break;
            }
        }
        assert_eq!(keys.len(), 50_021);
        assert!(found_old_star);
    }
}

#[test]
fn candidate_page_does_not_group_the_entire_archive_for_each_scalar_signal() {
    let vault = vault_core::Vault::open_temp().unwrap();
    vault.store().capture_query_plans(vault_store::Db::Archive, true).unwrap();
    vault_core::select_working_set(
        &vault,
        &Default::default(),
        1_700_000_000_000,
        127,
        Some(50_000),
    )
    .unwrap();
    let plans = vault.store().take_query_plans(vault_store::Db::Archive).unwrap();
    assert!(!plans.is_empty());
    for plan in plans {
        eprintln!("working-set plan: {:?}", plan.steps);
        for step in &plan.steps {
            assert!(
                !["SCAN variant", "SCAN f", "SCAN pages", "SCAN p"]
                    .iter()
                    .any(|prefix| step == prefix || step.starts_with(&format!("{prefix} "))),
                "{:?}\n{}",
                plan.steps,
                plan.sql
            );
        }
    }
}
