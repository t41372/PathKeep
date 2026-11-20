//! Join facts to the requested display addresses in batches. Reader bodies
//! come from the existing evidence/rendered-page window.
use crate::{query::operation, read_model::DisplayRow};
use serde_json::{Value, json};
use std::collections::{BTreeMap, HashMap};
use vault_store::{Session, Value as Sql, params};
fn json_text(raw: Option<String>) -> vault_store::Result<Value> {
    raw.as_deref()
        .map(serde_json::from_str)
        .transpose()
        .map_err(operation)
        .map(|v| v.unwrap_or(Value::Null))
}

pub(super) fn annotate(
    tx: &Session<'_>,
    publication: &str,
    records: Vec<DisplayRow>,
    branch: &str,
    leaf: Option<&str>,
) -> vault_store::Result<Vec<Value>> {
    let mut sources: BTreeMap<&str, Vec<i64>> = BTreeMap::new();
    for r in &records {
        sources.entry(&r.source_id).or_default().push(r.ordinal.0 as i64);
    }
    let mut annotations = HashMap::new();
    let mut attachments: HashMap<(String, i64), Vec<Value>> = HashMap::new();
    let mut call_results = HashMap::new();
    let mut notices = HashMap::<(String, i64), Vec<Value>>::new();
    for (source, ordinals) in sources {
        // An inherited prefix points to its producing conversation. A redundant
        // copy shares the original source's accounting addresses.
        let owner=tx.query_one("SELECT coalesce((SELECT ?1 FROM ah_reader_member WHERE publication=?1 AND source=?2),
            (SELECT head.publication FROM ah_sessions s JOIN ah_reader_head head ON head.conversation=s.conversation_id WHERE s.id=?2)),
            coalesce((SELECT copy_of_source_id FROM ah_sessions WHERE id=?2),?2)",params![publication,source],|r|Ok((r.get::<Option<String>>(0)?,r.get::<String>(1)?)))?;
        let Some(owner_id) = owner.0 else { continue };
        let mut args = vec![
            Sql::Text(owner_id),
            Sql::Text(leaf.unwrap_or(&owner.1).into()),
            Sql::Text(owner.1),
            Sql::Text(publication.into()),
            Sql::Text(leaf.unwrap_or(branch).into()),
            Sql::Text(source.into()),
        ];
        let values = ordinals
            .iter()
            .map(|n| {
                args.push(Sql::Integer(*n));
                format!("(?{})", args.len())
            })
            .collect::<Vec<_>>()
            .join(",");
        let refs=format!("WITH wanted(n) AS (VALUES {values}) SELECT a.display_ordinal,a.ordinal,a.reference_key,a.name,a.kind,a.reason
            FROM wanted w JOIN ah_chat_attachment a ON a.publication=?1 AND a.source=?3 AND a.display_ordinal=w.n");
        tx.for_each(&refs,args.clone(),|r|{
            let reason=r.get::<Option<String>>(5)?;
            attachments.entry((source.to_owned(),r.get::<i64>(0)?)).or_default().push(json!({"sourceId":source,"ordinal":r.get::<i64>(1)?,"key":r.get::<String>(2)?,"name":r.get::<Option<String>>(3)?,"kind":r.get::<Option<String>>(4)?,"availability":if reason.is_some(){"missing"}else{"retained"},"reason":reason}));
            Ok(())
        })?;
        let banners=format!("WITH wanted(n) AS (VALUES {values}) SELECT a.display_ordinal,a.ordinal,a.code,a.tone,a.at
            FROM wanted w JOIN ah_chat_notice a ON a.publication=?1 AND a.source=?3 AND a.display_ordinal=w.n");
        tx.for_each(&banners,args.clone(),|r|{
            notices.entry((source.into(),r.get::<i64>(0)?)).or_default().push(json!({"ordinal":r.get::<i64>(1)?,"code":r.get::<String>(2)?,"tone":r.get::<String>(3)?,"at":r.get::<Option<i64>>(4)?}));
            Ok(())
        })?;
        let calls = format!(
            "WITH wanted(n) AS (VALUES {values}) SELECT c.call_id,c.result_ordinal,c.status,c.duration FROM wanted w JOIN ah_chat_call c ON c.publication=?1 AND c.source=?3 AND c.ordinal=w.n"
        );
        tx.for_each(&calls, args.clone(), |r| {
            call_results.insert(
                (source.to_owned(), r.get::<String>(0)?),
                (r.get::<Option<i64>>(1)?, r.get::<Option<String>>(2)?, r.get::<Option<i64>>(3)?),
            );
            Ok(())
        })?;
        let sql=format!("WITH wanted(n) AS (VALUES {values}) SELECT w.n,t.ordinal,
              CASE WHEN t.footer=w.n THEN t.facts END,o.facts,response.facts,CASE WHEN variant.id IS NOT NULL THEN
                (SELECT count(*) FROM ah_chat_variant peer WHERE peer.publication=variant.publication AND peer.source=variant.source AND peer.parent IS variant.parent) END,prompt.rank
              FROM wanted w LEFT JOIN ah_chat_turn t ON t.publication=?1 AND t.source=?2 AND t.ordinal=(
                SELECT ordinal FROM ah_chat_turn WHERE publication=?1 AND source=?2 AND ordinal<=w.n ORDER BY ordinal DESC LIMIT 1)
                AND t.last_ordinal>=w.n
              LEFT JOIN ah_chat_operation o ON o.publication=?1 AND o.source=?3 AND o.ordinal=w.n
              LEFT JOIN ah_chat_prompt prompt ON prompt.publication=?4 AND prompt.branch=?5 AND prompt.source=?6 AND prompt.ordinal=t.ordinal
              LEFT JOIN ah_chat_variant variant ON variant.publication=?1 AND variant.source=?3 AND variant.ordinal=w.n
              LEFT JOIN ah_chat_response response ON response.publication=?1 AND response.source=?3 AND response.ordinal=w.n");
        tx.for_each(&sql, args, |r| {
            annotations.insert(
                (source.to_owned(), r.get::<i64>(0)?),
                (
                    r.get::<Option<i64>>(1)?,
                    json_text(r.get(2)?)?,
                    json_text(r.get(3)?)?,
                    json_text(r.get(4)?)?,
                    r.get::<Option<i64>>(5)?,
                    r.get::<Option<i64>>(6)?.map(|n| n + 1),
                ),
            );
            Ok(())
        })?;
    }
    let mut out = Vec::with_capacity(records.len());
    for r in records {
        let (turn, footer, ops, response, versions, turn_number) =
            annotations.remove(&(r.source_id.clone(), r.ordinal.0 as i64)).unwrap_or_default();
        let mut chips =
            attachments.remove(&(r.source_id.clone(), r.ordinal.0 as i64)).unwrap_or_default();
        let mut operations = vec![];
        for mut op in ops.as_array().cloned().unwrap_or_default() {
            if op["kind"] == "attachment" {
                op["sourceId"] = json!(r.source_id);
                op["ordinal"] = json!(r.ordinal);
                chips.push(op);
                continue;
            }
            if op["kind"] == "call"
                && let Some(id) = op["callId"].as_str()
                && let Some((ordinal, status, duration)) =
                    call_results.get(&(r.source_id.clone(), id.to_owned()))
            {
                op["resultOrdinal"] = json!(ordinal);
                op["status"] = json!(status);
                op["durationMs"] = json!(duration);
            }
            operations.push(op);
        }
        let banners =
            notices.remove(&(r.source_id.clone(), r.ordinal.0 as i64)).unwrap_or_default();
        let parts=r.parts.into_iter().map(|p|{
            use crate::read_model::PartKind::*;
            json!({"kind":match p.kind{Text=>"text",Thinking=>"thinking",CallInput=>"callInput",CallResult=>"callResult"},"text":p.text})
        }).collect::<Vec<_>>();
        out.push(json!({"sourceId":r.source_id,"ordinal":r.ordinal,"nativeId":r.native_id,"parentId":r.parent_id,"kind":r.kind,"role":r.role,"at":r.timestamp,
            "parts":parts,"notices":banners,"bytes":r.record_bytes,"truncated":r.truncated,"boundary":r.boundary,"turnOrdinal":turn,"turnNumber":turn_number,"versionCount":versions,"footer":footer,"response":response,"operations":operations,"attachments":chips}));
    }
    Ok(out)
}
