//! Disk-backed index and interactive stream timings. Seed the post-backup projection outside
//! the timed section, so the build figure excludes fixture generation and native discovery.
use agent_history::{Archive, Harness, SourceRoot, capture::BackupOptions, search::ScreenQuery};
use serde_json::json;
use std::{
    path::Path,
    sync::{Arc, Mutex},
    time::Instant,
};

fn seed(root: &Path, roots: i64) {
    drop(Archive::create_unencrypted(root).unwrap());
    let db = rusqlite::Connection::open(vault_layout::path(
        root,
        vault_layout::ArtifactId::AgentHistoryReadModel,
    ))
    .unwrap();
    // The archive is closed while seeding the normalized inputs. All measured operations use
    // the production store, tokenizer, directory, commit, query and stream.
    db.execute_batch("CREATE TEMP TABLE ranks(n INTEGER PRIMARY KEY);
        WITH RECURSIVE n(x) AS (VALUES(0) UNION ALL SELECT x+1 FROM n WHERE x<119) INSERT INTO ranks SELECT x FROM n;
        CREATE TEMP TABLE roots(n INTEGER PRIMARY KEY,id TEXT);").unwrap();
    for start in (0..roots).step_by(250) {
        let tx = db.unchecked_transaction().unwrap();
        tx.execute("DELETE FROM roots", []).unwrap();
        tx.execute(
            "WITH RECURSIVE n(x) AS (VALUES(?1) UNION ALL SELECT x+1 FROM n WHERE x+1<?2)
            INSERT INTO roots SELECT x,printf('pi-%06d',x) FROM n",
            [start, roots.min(start + 250)],
        )
        .unwrap();
        tx.execute("INSERT INTO ah_sessions(id,source_id,native_id,harness,origin,revision_id,complete,projection_id,parser_version,conversation_id,origin_mark,first_record_digest,source_root,relative_path,relation_json,title_source,conversation_current)
            SELECT id,id,id,'pi','cli',n+1,1,n+1,'bench',id,'cli','','/bench',id,'{}','tool',1 FROM roots",[]).unwrap();
        tx.execute("INSERT INTO ah_chat_summary(publication,id,live,root,harness,title,title_source,started,last_activity,missing,bytes,detail,own,totals,recent_key,start_key,agent_key,tokens_key,turns_key)
            SELECT id,id,1,1,'pi',id,'tool',n,n,0,48000,'{}','{}','{}',id,id,id,id,id FROM roots",[]).unwrap();
        tx.execute("INSERT INTO ah_outbox(session_id,revision_id,projection_id) SELECT id,n+1,n+1 FROM roots",[]).unwrap();
        tx.execute("INSERT INTO ah_events(session_id,revision_id,projection_id,ordinal,native_id,kind,role,text,payload)
            SELECT id,n+1,n+1,rank,printf('entry-%d',rank),'message',role,body,
            json_object('type','message','id',printf('entry-%d',rank),'message',json_object('role',role,'content',body))
            FROM (SELECT roots.id,roots.n,ranks.n AS rank,
                CASE ranks.n%4 WHEN 0 THEN 'user' WHEN 2 THEN 'toolResult' ELSE 'assistant' END AS role,
                CASE WHEN roots.n=0 AND ranks.n=119 THEN 'the singularneedle at the oldest tail'
                WHEN ranks.n%4=0 THEN 'please inspect the commonword in this saved conversation'
                ELSE 'synthetic response with a reproducible transcript body' END AS body
                FROM roots CROSS JOIN ranks)",[]).unwrap();
        tx.commit().unwrap();
    }
    db.execute("UPDATE ah_meta SET value=?1 WHERE key='next_projection_id'", [roots + 2]).unwrap();
}
#[test]
#[ignore = "Linux release conversation search, 120,000 roots x 120 records"]
fn global_stream_at_scale() {
    let roots = std::env::var("AH_SEARCH_ROOTS").map_or(120_000, |n| n.parse().unwrap());
    let tmp = tempfile::tempdir().unwrap();
    let root = std::env::var_os("AH_SEARCH_CORPUS")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| tmp.path().join("archive"));
    if !root.exists() {
        seed(&root, roots);
    }
    let a = Arc::new(Archive::open(&root, b"").unwrap());
    let started = Instant::now();
    let built = a.rebuild_search().unwrap();
    assert_eq!(built.indexed_events, (roots * 120) as u64);
    println!(
        "{}",
        json!({"operation":"indexBuild","roots":roots,"records":built.indexed_events,"elapsedMs":started.elapsed().as_millis()})
    );
    for (term, groups, expected) in [("commonword", roots, roots * 30), ("singularneedle", 1, 1)] {
        let start = Instant::now();
        let mut first = None;
        let mut last = None;
        let mut previous = None;
        let mut distinct = 0;
        a.stream_conversations(
            None,
            &ScreenQuery::all(term.into()),
            &Default::default(),
            |ty, value| {
                if ty == "groups" {
                    first.get_or_insert_with(|| start.elapsed());
                    for card in value["items"].as_array().unwrap() {
                        let id = card["id"].as_str().unwrap();
                        if previous.as_deref() != Some(id) {
                            let rank = if groups == 1 { 0 } else { groups - distinct - 1 };
                            assert_eq!(id, format!("pi-{rank:06}"));
                            distinct += 1;
                            previous = Some(id.to_owned());
                        }
                        if groups == 1 {
                            assert_eq!(card["snippets"][0]["address"]["ordinal"], 119);
                        }
                    }
                }
                if ty == "finished" {
                    last = Some(value)
                }
                Ok(())
            },
        )
        .unwrap();
        let last = last.unwrap();
        assert_eq!(distinct, groups);
        assert_eq!(last["hits"], expected);
        assert_eq!(last["searchedCount"], roots);
        println!(
            "{}",
            json!({"operation":"search","query":term,"firstResultMs":first.unwrap().as_secs_f64()*1000.,"endMs":start.elapsed().as_secs_f64()*1000.,"hits":expected,"conversations":groups,"searched":roots})
        );
    }
    let path = tmp.path().join("pi");
    std::fs::create_dir_all(path.join("sessions")).unwrap();
    std::fs::write(path.join("sessions/new.jsonl"),b"{\"type\":\"session\",\"id\":\"added\",\"version\":3}\n{\"type\":\"message\",\"id\":\"u\",\"parentId\":null,\"message\":{\"role\":\"user\",\"content\":\"a new question\"}}\n{\"type\":\"message\",\"id\":\"a\",\"parentId\":\"u\",\"message\":{\"role\":\"assistant\",\"content\":\"deltaonly\"}}\n").unwrap();
    let elapsed = Arc::new(Mutex::new(None));
    let observed = elapsed.clone();
    let owner = a.clone();
    let start = Instant::now();
    a.backup(
        &[SourceRoot { harness: Harness::Pi, root: path }],
        &BackupOptions {
            progress: Some(Arc::new(move |p| {
                if p.phase == "index" {
                    let start = Instant::now();
                    let delta = owner.synchronize_search().unwrap();
                    *observed.lock().unwrap() = Some((start.elapsed(), delta.indexed_events));
                }
            })),
            ..Default::default()
        },
    )
    .unwrap();
    let (index_time, events) = elapsed.lock().unwrap().unwrap();
    let backup = start.elapsed();
    let mut hits = None;
    a.stream_conversations(
        None,
        &ScreenQuery::all("deltaonly".into()),
        &Default::default(),
        |ty, p| {
            if ty == "finished" {
                hits = p["hits"].as_i64()
            }
            Ok(())
        },
    )
    .unwrap();
    assert_eq!(hits, Some(1));
    println!(
        "{}",
        json!({"operation":"oneNewRoot","indexedEvents":events,"indexUpdateMs":index_time.as_secs_f64()*1000.,"backupMs":backup.as_secs_f64()*1000.})
    );
}
