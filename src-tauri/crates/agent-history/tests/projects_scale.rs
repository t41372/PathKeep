//! Time the screen reads on 120,000 published roots. Fixture setup bypasses native parsing;
//! the measured queries, grouping, mark mirror and folder directory are the production paths.
use agent_history::{
    Archive,
    chats::{Group, ListQuery},
};
use serde_json::{Value, json};
use std::{path::Path, time::Instant};
use vault_layout::{ArtifactId, path};

fn seed(root: &Path) {
    drop(Archive::create_unencrypted(root).unwrap());
    let db = rusqlite::Connection::open(path(root, ArtifactId::AgentHistoryReadModel)).unwrap();
    db.execute_batch("WITH RECURSIVE n(i) AS (VALUES(0) UNION ALL SELECT i+1 FROM n WHERE i<119999),
        rows AS (SELECT printf('chat-%06d',i) id,i,
          CASE WHEN i>=60000 THEN printf('/projects/large/%02d',i%8) ELSE printf('/projects/small-%03d',i%600) END folder FROM n)
        INSERT INTO ah_chat_summary(publication,id,live,root,harness,folder,title,title_source,started,last_activity,missing,bytes,saved,own,totals,detail,recent_key,start_key,agent_key,tokens_key,turns_key,messages,turns,agent_ms,tokens_total)
        SELECT id,id,1,1,CASE i%2 WHEN 0 THEN 'pi' ELSE 'codex' END,folder,id,'tool',i,i,0,48000,i,'{}','{}','{}',id,id,id,id,id,30,30,720000,720 FROM rows;
        INSERT INTO ah_project_paths SELECT DISTINCT folder FROM ah_chat_summary; INSERT INTO ah_project_dirty SELECT path FROM ah_project_paths;").unwrap();
    let db = rusqlite::Connection::open(path(root, ArtifactId::AgentHistoryEvidence)).unwrap();
    db.execute("INSERT INTO ah_folder_observations(path,exists_on_disk,observed_at,git_json) VALUES('/projects/large',1,1,?1)",
        [json!({"top_level":"/projects/large","common_dir":"/projects/large/.git","branch":"main","remotes":[]}).to_string()]).unwrap();
}
fn measure(mut f: impl FnMut() -> Value, assertion: impl Fn(&Value)) -> Value {
    let mut us = Vec::new();
    let mut first = 0;
    for i in 0..31 {
        let start = Instant::now();
        let result = f();
        let t = start.elapsed().as_micros();
        if i == 0 {
            first = t
        } else {
            us.push(t)
        }
        assertion(&result);
    }
    us.sort_unstable();
    json!({"firstUs":first,"warmSamples":us.len(),"p50Us":us[14],"p95Us":us[28]})
}
fn open(root: &Path) -> (std::sync::Arc<vault_store::Store>, Archive) {
    use vault_store::{Config, Db, Store};
    let role = Db::AgentHistoryReadModel;
    let mut config = Config::new(root);
    config.databases = vec![role, Db::AgentHistoryEvidence, Db::Scratch];
    for db in [role, Db::AgentHistoryEvidence] {
        config.migrations.register(db, agent_history::schema_steps(db));
    }
    let (_, key) = vault_crypto::Envelope::create(
        b"ephemeral benchmark spill",
        vault_crypto::KdfParams { memory_kib: 32, iterations: 1, lanes: 1 },
    )
    .unwrap();
    config.io = Some(vault_crypto::TempCryptIo::new(&key).unwrap());
    let store = std::sync::Arc::new(Store::open(config).unwrap());
    let a = Archive::from_store(
        store.clone(),
        agent_history::AgentHistoryResources {
            search_path: path(root, ArtifactId::AgentHistorySearch),
            search_key: None,
        },
    )
    .unwrap();
    (store, a)
}
#[test]
#[ignore = "Linux release Projects screen timing"]
fn projects_at_120000_roots() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("archive");
    seed(&root);
    let prepared = Archive::open(&root, b"").unwrap();
    let cold = Instant::now();
    // This initial scoped read installs membership for the simulated pre-projects archive.
    prepared
        .chat_list(&ListQuery { group: Group::None, limit: Some(1), ..Default::default() })
        .unwrap();
    let membership = cold.elapsed();
    let all = prepared.project_list().unwrap();
    let initial = cold.elapsed();
    let id = all["items"][0]["id"].as_str().unwrap();
    drop(prepared);
    let (store, a) = open(&root);
    let role = vault_store::Db::AgentHistoryReadModel;
    let list = measure(
        || a.project_list().unwrap(),
        |v| {
            assert_eq!(v["items"].as_array().unwrap().len(), 601);
            assert_eq!(v["items"][0]["mainFolder"], "/projects/large");
            assert_eq!(v["items"][0]["conversationCount"], 60000);
            assert_eq!(v["items"][0]["tokensTotal"], 43200000);
        },
    );
    drop(a);
    drop(store);
    let (store, a) = open(&root);
    let q = ListQuery {
        project_id: Some(id.into()),
        limit: Some(50),
        group: Group::Day,
        ..Default::default()
    };
    let page = measure(
        || json!({"header":a.project_detail(id).unwrap(),"records":a.chat_list(&q).unwrap()}),
        |v| {
            assert_eq!(v["header"]["project"]["conversationCount"], 60000);
            let v = &v["records"];
            for (i, item) in v["items"].as_array().unwrap().iter().enumerate() {
                assert_eq!(item["id"], format!("chat-{:06}", 119999 - i));
                assert_eq!(item["project"]["id"], id);
            }
            assert_eq!(v["items"].as_array().unwrap().len(), 50);
            assert_eq!(v["groups"][0]["count"], 60000);
        },
    );
    store.capture_query_plans(role, true).unwrap();
    a.chat_list(&q).unwrap();
    let plans = store.take_query_plans(role).unwrap();
    let plan = plans
        .iter()
        .find(|p| {
            p.sql.contains("ORDER BY s.recent_key DESC LIMIT") && p.sql.contains("s.project_id=")
        })
        .unwrap();
    assert!(plan.steps.iter().any(|step| step.contains("ah_project_recent")), "{:?}", plan.steps);
    drop(a);
    drop(store);
    let (store, reopened) = open(&root);
    store.capture_query_plans(role, true).unwrap();
    reopened.project_list().unwrap();
    let list_plans = store.take_query_plans(role).unwrap();
    let list_plan = list_plans.iter().find(|p| p.sql.contains("FROM ah_project_rollup b")).unwrap();
    println!(
        "{}",
        json!({"roots":120000,"projects":601,"largeRoots":60000,"largeFoldersWithChats":8,"initialMembershipUs":membership.as_micros(),"initialRollupsAndListUs":(initial-membership).as_micros(),"listAll":list,"projectHeaderAndFirst50":page,"pagePlan":plan.steps,"projectsListPlan":list_plan.steps})
    );
}
