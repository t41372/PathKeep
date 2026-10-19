//! Responsibilities: verify native candidate coverage and parked-job re-offering.
//! Not responsible for: network fetching or extractor behavior.
//! Failure modes: URL variants miss their page, absent pages are forgotten, stale visit IDs
//! survive re-offering, or a candidate pass stops at its page size.
use vault_core::{
    ContentFetchNowRequest, Vault, enqueue_content_fetch_now, enqueue_content_fetch_working_set,
};
use vault_store::{Db, Priority, params};

#[test]
fn native_pages_offer_all_candidates_and_reoffer_a_prearchive_job() {
    for key in [None, Some([62; 32])] {
        let owner = Vault::open_temp_with_key(key).unwrap();
        let mut config = owner.config().clone();
        config.ai.content_fetch_enabled = true;
        let vault = owner.with_config(config);
        let pending = enqueue_content_fetch_now(
            &vault,
            &ContentFetchNowRequest {
                history_id: Some(999),
                profile_id: Some("old-profile".into()),
                url: "https://github.com/owner/repo?utm_source=mail".into(),
                title: None,
            },
        )
        .unwrap();
        vault
            .store()
            .write(Db::Intelligence, Priority::Interactive, move |s| {
                s.execute(
                    "UPDATE intelligence_jobs SET state='awaiting_archive' WHERE id=?1",
                    [pending.job_id],
                )?;
                Ok(())
            })
            .unwrap();
        assert_eq!(vault_core::reoffer_archived_candidates(&vault).unwrap(), 0);
        vault.store().write(Db::Archive,Priority::Batch,|s| {
            s.execute("INSERT INTO source_profiles(id,browser_kind,profile_name,profile_path,discovered_at,profile_key) VALUES(1,'chrome','Default','/fixture','','chrome:Default')",())?;
            for id in 1..=23_i64 {
                let url=if id==1 {"https://github.com/owner/repo".to_owned()} else {format!("https://github.com/owner/repo{id}")};
                s.execute("INSERT INTO pages(id,url,canonical_url,registrable_domain) VALUES(?1,?2,?2,'github.com')",params![id,url.clone()])?;
                s.execute("INSERT INTO urls(id,page_id,url,source_profile_id,created_by_run_id,first_visit_ms,first_visit_iso,last_visit_ms,last_visit_iso) VALUES(?1,?1,?2,1,0,1,'',1,'')",params![id,url.clone()])?;
                s.execute("INSERT INTO visits(id,url_id,source_profile_id,created_by_run_id,visit_time_ms,visit_time_iso) VALUES(?1,?1,1,0,1,'')",[id])?;
                s.execute("INSERT INTO star(entity_kind,entity_key,starred_at) VALUES('url',?1,'')",[url])?;
                s.execute("INSERT INTO search_documents(page_id,url,title,search_terms,normalized_url,normalized_title,normalized_search_terms,cjk_grams,compact_text,updated_at) VALUES(?1,?2,'','',?2,'','','','','')",params![id,if id==1 {"https://github.com/owner/repo".to_owned()} else {format!("https://github.com/owner/repo{id}")}])?;
                s.execute("INSERT INTO search_document_profiles(page_id,source_profile_id,visible_visit_count,newest_visit_ms) VALUES(?1,1,1,1)",[id])?;
            }
            Ok(())
        }).unwrap();
        assert_eq!(vault_core::reoffer_archived_candidates(&vault).unwrap(), 1);
        let payload = vault
            .store()
            .read(Db::Intelligence, move |s| {
                s.query_one(
                    "SELECT state,payload_json FROM intelligence_jobs WHERE id=?1",
                    [pending.job_id],
                    |r| Ok((r.get::<String>(0)?, r.get::<String>(1)?)),
                )
            })
            .unwrap();
        assert_eq!(payload.0, "queued");
        let json: serde_json::Value = serde_json::from_str(&payload.1).unwrap();
        assert_eq!(json["historyId"], 1);
        assert_eq!(json["profileId"], "chrome:Default");
        enqueue_content_fetch_working_set(&vault, 4).unwrap();
        let count=vault.store().read(Db::Intelligence,|s| s.query_one("SELECT COUNT(*) FROM intelligence_jobs WHERE job_type='content-fetch' AND state='queued'",(),|r|Ok(r.get::<i64>(0)?))).unwrap();
        assert_eq!(count, 23);
    }
}

#[test]
fn a_page_without_visits_reoffers_a_parked_candidate() {
    for key in [None, Some([87; 32])] {
        let owner = Vault::open_temp_with_key(key).unwrap();
        let mut config = owner.config().clone();
        config.ai.content_fetch_enabled = true;
        let vault = owner.with_config(config);
        let job = enqueue_content_fetch_now(
            &vault,
            &ContentFetchNowRequest {
                history_id: None,
                profile_id: None,
                url: "https://github.com/owner/page".into(),
                title: None,
            },
        )
        .unwrap();
        vault
            .store()
            .write(Db::Intelligence, Priority::Interactive, move |s| {
                s.execute(
                    "UPDATE intelligence_jobs SET state='awaiting_archive' WHERE id=?1",
                    [job.job_id],
                )?;
                Ok(())
            })
            .unwrap();
        vault.store().write(Db::Archive,Priority::Batch,|s| {s.execute("INSERT INTO pages(url,canonical_url,registrable_domain) VALUES('https://github.com/owner/page','https://github.com/owner/page','github.com')",())?;Ok(())}).unwrap();
        assert_eq!(vault_core::reoffer_archived_candidates(&vault).unwrap(), 1);
        let state = vault
            .store()
            .read(Db::Intelligence, move |s| {
                s.query_one(
                    "SELECT state,payload_json FROM intelligence_jobs WHERE id=?1",
                    [job.job_id],
                    |r| Ok((r.get::<String>(0)?, r.get::<String>(1)?)),
                )
            })
            .unwrap();
        assert_eq!(state.0, "queued");
        let payload: serde_json::Value = serde_json::from_str(&state.1).unwrap();
        assert!(payload["historyId"].is_null());
    }
}
