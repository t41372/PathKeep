//! Responsibilities: exercise complete agent report output through the native owner.
//! Not responsible for: LLM execution or search's candidate pools.
//! Failure modes: the 8 KB model budget drops domains, row cursors are ignored,
//! or a report mutation silently splices different fragments.
use serde_json::{Value, json};
use vault_core::{
    Vault,
    ai::{AgentToolContext, ToolRegistry},
};
use vault_store::{Db, Priority, params};
#[tokio::test]
async fn oversized_native_reports_page_through_every_domain() {
    for key in [None, Some([86; 32])] {
        let vault = Vault::open_temp_with_key(key).unwrap();
        vault.store().write(Db::Intelligence,Priority::Batch,|s| {
            s.execute("INSERT INTO intelligence_generations(profile_id) VALUES('profile')",())?;
            s.execute("INSERT INTO intelligence_counts(generation,profile_id,table_name,value) VALUES(0,'profile','sessions',1)",())?;
            for id in 1..=300_i64 {
                s.execute("INSERT INTO domain_daily_rollups_rows(generation,date_key,profile_id,registrable_domain,domain_category,visit_count,search_count,new_domain_visits,unique_urls) VALUES(0,'2024-04-01','profile',?1,?2,?3,0,0,1)",params![format!("site{id}.test"),"参考資料".repeat(10),id])?;
            }Ok(())
        }).unwrap();
        let context = AgentToolContext {
            vault: vault.clone(),
            paths: vault.paths().clone(),
            config: vault.config().clone(),
            database_key: None,
            embedding_provider: None,
            default_profile_id: None,
            default_domain: None,
            default_limit: 20,
            run_control: None,
            tz_offset: chrono::FixedOffset::east_opt(0).unwrap(),
        };
        let registry = ToolRegistry::with_default_search_tools();
        let mut cursor = None::<String>;
        let mut joined = String::new();
        let mut fragments = 0;
        loop {
            let result=registry.dispatch("intelligence_report",json!({"report":"top_sites","limit":300,"start_date":"2024-04-01","end_date":"2024-04-01","output_cursor":cursor}),&context).await.unwrap();
            assert!(result.model_text.len() <= 8000);
            let body = result.model_text.split_once("result follows.\n").unwrap().1;
            let page: Value = serde_json::from_str(body).unwrap();
            joined.push_str(page["jsonFragment"].as_str().unwrap());
            fragments += 1;
            cursor = page["nextOutputCursor"].as_str().map(str::to_owned);
            if cursor.is_none() {
                break;
            }
        }
        assert!(fragments > 1);
        let report: Value = serde_json::from_str(&joined).unwrap();
        assert_eq!(report["items"].as_array().unwrap().len(), 300);
        assert!(report["nextCursor"].is_null());
    }
}

#[tokio::test]
async fn agent_session_detail_forwards_the_independent_trail_cursor() {
    for key in [None, Some([108; 32])] {
        let vault = Vault::open_temp_with_key(key).unwrap();
        vault.store().write(Db::Intelligence,Priority::Batch,|s| {
            s.execute("INSERT INTO intelligence_generations(profile_id) VALUES('profile')",())?;
            s.execute("INSERT INTO intelligence_counts(generation,profile_id,table_name,value) VALUES(0,'profile','sessions',1)",())?;
            s.execute("INSERT INTO sessions_rows(session_id,profile_id,first_visit_ms,last_visit_ms,visit_count,search_count,domain_count,computed_at) VALUES('session','profile',1,9,9,9,1,'')",())?;
            for id in 1..=9_i64 {
                s.execute("INSERT INTO search_trails_rows(trail_id,profile_id,session_id,initial_query,search_engine,visit_count,first_visit_ms,last_visit_ms,queries_json,computed_at) VALUES(?1,'profile','session','query','google',1,?2,?2,'[]','')",params![format!("trail-{id}"),id])?;
            }Ok(())
        }).unwrap();
        let context = AgentToolContext {
            vault: vault.clone(),
            paths: vault.paths().clone(),
            config: vault.config().clone(),
            database_key: None,
            embedding_provider: None,
            default_profile_id: None,
            default_domain: None,
            default_limit: 20,
            run_control: None,
            tz_offset: chrono::FixedOffset::east_opt(0).unwrap(),
        };
        let registry = ToolRegistry::with_default_search_tools();
        let mut cursor = None::<String>;
        let mut seen = std::collections::BTreeSet::new();
        loop {
            let result=registry.dispatch("intelligence_report",json!({"report":"session_detail","session_id":"session","limit":2,"trails_cursor":cursor}),&context).await.unwrap();
            let body = result.model_text.split_once("result follows.\n").unwrap().1;
            let page: Value = serde_json::from_str(body).unwrap();
            let trails = page["trails"].as_array().unwrap();
            assert!(trails.len() <= 2);
            for trail in trails {
                assert!(seen.insert(trail["trailId"].as_str().unwrap().to_owned()));
            }
            cursor = page["trailsNextCursor"].as_str().map(str::to_owned);
            if cursor.is_none() {
                break;
            }
        }
        assert_eq!(seen.len(), 9);
    }
}
