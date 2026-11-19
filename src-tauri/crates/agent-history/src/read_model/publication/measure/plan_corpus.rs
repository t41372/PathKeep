//! Query-plan population for the four native phases of generate_pi in scripts/agent-history-verify.py.
//! This fixture writes complete retained documents and populates the indexes touched by the measured SQL.
use super::*;

fn document(root: i64, roots: i64, times: &[String]) -> Vec<u8> {
    let mut raw = Vec::with_capacity(48 * 1024);
    let header = json!({"type":"session","version":3,"id":format!("bench-{root:06}"),"timestamp":times[0],"cwd":"/synthetic/project"});
    serde_json::to_writer(&mut raw, &header).unwrap();
    raw.push(b'\n');
    for (ordinal, time) in times.iter().enumerate() {
        let index = (root - 1) * 120 + ordinal as i64;
        let mut text = format!("Record {index}: 歷史備份 Café Δ local evidence and tool output.");
        if index + 1 == roots * 120 {
            text.push_str(" pathkeepverificationtailquartz");
        }
        let mut message = json!({"role":"user","content":[{"type":"text","text":text}],"timestamp":1_794_312_000_000_i64+ordinal as i64});
        match ordinal % 4 {
            phase @ (1 | 3) => {
                message["role"] = "assistant".into();
                message["api"] = "anthropic-messages".into();
                message["provider"] = "synthetic".into();
                message["model"] = "synthetic-model".into();
                message["stopReason"] = if phase == 1 { "toolUse" } else { "stop" }.into();
                message["usage"] = json!({"input":10,"output":2,"cacheRead":0,"cacheWrite":0,"totalTokens":12,
                    "cost":{"input":0,"output":0,"cacheRead":0,"cacheWrite":0,"total":0}});
                if phase == 1 {
                    message["content"].as_array_mut().unwrap().push(json!({"type":"toolCall","id":format!("call-{}",index/4),"name":"read","arguments":{"path":"fixture.txt"}}));
                }
            }
            2 => {
                message["role"] = "toolResult".into();
                message["toolCallId"] = format!("call-{}", index / 4).into();
                message["toolName"] = "read".into();
                message["isError"] = false.into();
            }
            _ => {}
        }
        let v = json!({"type":"message","id":format!("event-{index}"),"parentId":(ordinal>0).then(||format!("event-{}",index-1)),"timestamp":time,"message":message});
        serde_json::to_writer(&mut raw, &v).unwrap();
        raw.push(b'\n');
    }
    raw
}

pub(super) fn seed(a: &Archive, roots: i64) {
    use sha2::{Digest, Sha256};
    let times: Vec<_> = (0..120)
        .map(|n| {
            chrono::DateTime::from_timestamp_millis(1_794_312_000_000 + n)
                .unwrap()
                .to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
        })
        .collect();
    for base in (1..=roots).step_by(128) {
        let batch: Vec<_> = (base..(base + 128).min(roots + 1))
            .map(|id| {
                let raw = document(id, roots, &times);
                (
                    id,
                    format!("bench-{id:06}"),
                    hex::encode(Sha256::digest(&raw)),
                    raw.len() as i64,
                    zstd::stream::encode_all(raw.as_slice(), 1).unwrap(),
                )
            })
            .collect();
        let summaries=a.store.write(Db::AgentHistoryEvidence,Priority::Batch,move|tx|{
            let mut src=tx.prepare("INSERT INTO ah_sources(id,harness,root,relative_path,kind,retention,observed_at,latest_revision_id)
                VALUES(?1,'pi',?1,'sessions/chat.jsonl','transcript','appended',1,?2)")?;
            let mut revision=tx.prepare("INSERT INTO ah_revisions(id,source_id,digest,byte_len,captured_at,state,codec,next_ordinal,staged_byte_len)
                VALUES(?1,?2,?3,?4,1,'complete','zstd',1,?4)")?;
            let mut blob=tx.prepare("INSERT INTO ah_chunks(hash,codec,data) VALUES(?1,'zstd',?2)")?;
            let mut slice=tx.prepare("INSERT INTO ah_revision_chunks(revision_id,ordinal,chunk_hash,byte_len,byte_offset) VALUES(?1,0,?2,?3,0)")?;
            let mut summaries=Vec::new();
            for (id,root,digest,size,compressed) in batch{
                src.execute(params![root.clone(),id])?;
                revision.execute(params![id,root.clone(),digest.clone(),size])?;
                blob.execute(params![digest.clone(),compressed])?;
                slice.execute(params![id,digest.clone(),size])?;
                summaries.push((id,root,digest,size));
            }
            Ok(summaries)
        }).unwrap();
        a.store.write(DB,Priority::Batch,move|tx|{
            let mut session=tx.prepare("INSERT INTO ah_sessions(id,source_id,native_id,harness,origin,revision_id,complete,projection_id,
                parser_version,conversation_id,origin_mark,first_record_digest,source_root,relative_path,relation_json,title_source,conversation_current,updated_at)
                VALUES(?1,?1,?1,'pi','fixture',?2,1,?2,'fixture',?1,'fixture','fixture',?1,'sessions/chat.jsonl','{}','native',1,1)")?;
            let mut version=tx.prepare("INSERT INTO ah_reader_version(token,attempt,source,revision,digest,byte_len,codec,renderer,records,selective,state,record_limit,decoded_limit)
                VALUES(?1,?2,?1,?2,?5,?6,?3,?4,121,0,'published',16777216,1000000000)")?;
            let mut head=tx.prepare("INSERT INTO ah_reader_head(conversation,publication,current_source,input_digest) VALUES(?1,?1,?1,'fixture')")?;
            let mut receipt=tx.prepare("INSERT INTO ah_reader_serving(conversation,publication) VALUES(?1,?1)")?;
            for (id,root,digest,size) in summaries{
                session.execute(params![root.clone(),id])?;
                version.execute(params![root.clone(),id,CODEC,RENDERER,digest,size])?;
                receipt.execute([root.as_str()])?;
                head.execute([root])?;
            }
            Ok(())
        }).unwrap();
    }
    assert_eq!(
        a.store
            .read(Db::AgentHistoryEvidence, |tx| tx.query_one(
                "SELECT count(*) FROM ah_sources",
                (),
                |r| Ok(r.get::<i64>(0)?)
            ))
            .unwrap(),
        roots
    );
    for id in [1, (roots + 1) / 2, roots] {
        let mut tail = 0;
        let report = a
            .scan_revision(id, &Default::default(), &Default::default(), &mut |r, _, v| {
                let value = v.unwrap();
                if r.ordinal.0 == 0 {
                    assert_eq!(value["id"], format!("bench-{id:06}"));
                } else {
                    let n = (id - 1) * 120 + r.ordinal.0 as i64 - 1;
                    assert_eq!(value["id"], format!("event-{n}"));
                    tail += usize::from(
                        value["message"]["content"][0]["text"]
                            .as_str()
                            .unwrap()
                            .contains("pathkeepverificationtailquartz"),
                    );
                }
                Ok(())
            })
            .unwrap();
        assert_eq!((report.objects, tail), (121, usize::from(id == roots)));
    }
}
