//! Synthetic post-backup projection shared by lexical and semantic Linux measurements.
use agent_history::Archive;
use std::path::Path;
pub fn seed(root: &Path, roots: i64) {
    drop(Archive::create_unencrypted(root).unwrap());
    seed_projection(root, roots, false);
}
pub fn seed_projection(root: &Path, roots: i64, distinct_prompts: bool) {
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
                WHEN ranks.n%4=0 THEN 'please inspect the commonword in this saved conversation' ||
                    CASE WHEN ?1 THEN printf(' for task %d in project %s',ranks.n,roots.id) ELSE '' END
                ELSE 'synthetic response with a reproducible transcript body' END AS body
                FROM roots CROSS JOIN ranks)",[distinct_prompts]).unwrap();
        tx.commit().unwrap();
    }
    db.execute("UPDATE ah_meta SET value=?1 WHERE key='next_projection_id'", [roots + 2]).unwrap();
}
