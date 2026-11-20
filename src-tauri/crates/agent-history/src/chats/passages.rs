//! Bounded reads of normalized transcript text for optional retrieval. Native payloads,
//! attachment bytes and working-directory metadata never enter these passages.
use crate::{Archive, Error, Result, query::parents::hidden_query};
use serde::Serialize;
use vault_store::{Db, OptionalRow, params};

/// Offsets in this API count Unicode scalars, matching SQLite substr.
pub const PASSAGE_CHARS: usize = 1024;
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Passage {
    pub event_id: i64,
    pub conversation_id: String,
    pub source_id: String,
    pub ordinal: i64,
    pub role: String,
    pub text: String,
}
pub struct EmbeddingRow {
    pub id: i64,
    pub chunks: Vec<Chunk>,
}
pub struct Chunk {
    pub role: crate::search::Role,
    pub part: usize,
    pub start: usize,
    pub text: String,
}

// Use exactly the same native content slots and speaker rules as lexical Records search.
const BODY:&str="m.harness,e.kind,e.role,e.text,e.payload,CASE WHEN m.harness='deepseek' THEN
    (SELECT json_extract(header.payload,'$.version') FROM ah_events header WHERE header.session_id=m.id
    AND header.projection_id=m.projection_id AND header.ordinal=0) END";
fn chunks(row: &vault_store::Row, col: usize) -> vault_store::Result<Vec<Chunk>> {
    let harness = row.get::<String>(col)?.parse().map_err(crate::query::operation)?;
    let event = crate::NativeEvent {
        ordinal: 0,
        references: vec![],
        native_id: None,
        parent_id: None,
        timestamp: None,
        kind: row.get(col + 1)?,
        role: row.get(col + 2)?,
        text: row.get(col + 3)?,
        payload: serde_json::from_str(&row.get::<String>(col + 4)?)
            .map_err(crate::query::operation)?,
    };
    let mut out = Vec::new();
    for (part, p) in
        crate::search::readable(&event, harness, row.get(col + 5)?).into_iter().enumerate()
    {
        let mut chars = p.text.chars();
        let mut start = 0;
        loop {
            let text: String = chars.by_ref().take(PASSAGE_CHARS).collect();
            if text.is_empty() {
                break;
            }
            out.push(Chunk { role: p.role, part, start, text });
            start += PASSAGE_CHARS;
        }
    }
    Ok(out)
}
impl Archive {
    pub fn ensure_ai_visible(&self, id: &str) -> Result<()> {
        self.ensure_state_visible()?;
        let id = id.to_owned();
        let exists=self.store.read(Db::AgentHistoryReadModel,move|s|s.query_one(&format!(
            "SELECT EXISTS(SELECT 1 FROM ah_sessions WHERE conversation_id=?1 AND conversation_current=1 AND conversation_id NOT IN ({}))",hidden_query()),[id],|r|Ok(r.get::<i64>(0)?!=0)))?;
        if exists { Ok(()) } else { Err(Error::ConversationNotFound) }
    }
    /// Keyset scan keeps each database read bounded even when a single tool result is huge.
    pub fn embedding_rows(&self, after: i64) -> Result<Vec<EmbeddingRow>> {
        let sql = format!(
            "SELECT e.id,{BODY} FROM ah_events e CROSS JOIN ah_sessions m
            ON m.id=e.session_id AND m.projection_id=e.projection_id WHERE e.id>?1 AND e.text<>''
            AND m.conversation_id NOT IN ({}) ORDER BY e.id LIMIT 512",
            hidden_query()
        );
        Ok(self.store.read(Db::AgentHistoryReadModel, move |s| {
            let mut statement = s.prepare(&sql)?;
            let mut rows = statement.query([after])?;
            let mut page = Vec::new();
            let mut bytes = 0;
            while let Some(r) = rows.next()? {
                let id = r.get(0)?;
                let chunks = chunks(&r, 1)?;
                bytes += chunks.iter().map(|c| c.text.len()).sum::<usize>();
                page.push(EmbeddingRow { id, chunks });
                if bytes >= 1024 * 1024 {
                    break;
                }
            }
            Ok(page)
        })?)
    }
    pub fn ai_passage(&self, id: i64, chunk: u32) -> Result<Option<Passage>> {
        let sql=format!("SELECT m.conversation_id,m.id,e.ordinal,coalesce(e.role,'system'),substr(e.text,?2,?3)
            FROM ah_events e JOIN ah_sessions m ON m.id=e.session_id AND m.projection_id=e.projection_id
            WHERE e.id=?1 AND m.conversation_id NOT IN ({})",hidden_query());
        let start = i64::from(chunk) * PASSAGE_CHARS as i64 + 1;
        Ok(self.store.read(Db::AgentHistoryReadModel, move |s| {
            s.query_one(&sql, params![id, start, PASSAGE_CHARS as i64], |r| {
                Ok(Passage {
                    event_id: id,
                    conversation_id: r.get(0)?,
                    source_id: r.get(1)?,
                    ordinal: r.get(2)?,
                    role: r.get(3)?,
                    text: r.get(4)?,
                })
            })
            .optional()
        })?)
    }
}

/// Resolve ranked addresses through the current source projection and the Records root filter.
/// The ancestor walk starts only at the requested rows, including subagents in root filters.
impl Archive {
    pub fn semantic_page(
        &self,
        addresses: &[(i64, u32)],
        q: &crate::search::ScreenQuery,
    ) -> Result<Vec<(usize, serde_json::Value)>> {
        use serde_json::json;
        if addresses.is_empty() {
            return Ok(vec![]);
        }
        let wanted = addresses
            .iter()
            .enumerate()
            .map(|(i, (event, chunk))| format!("({i},{event},{})", *chunk))
            .collect::<Vec<_>>()
            .join(",");
        let filters = q.filters.clone();
        let roles = q.roles.clone();
        let hidden = hidden_query();
        Ok(self.store.read(Db::AgentHistoryReadModel,move|s|{
            let epoch=s.query_one("SELECT CAST(value AS INTEGER) FROM ah_meta WHERE key='search_epoch'",(),|r|Ok(r.get::<i64>(0)?))?;
            let filter=super::list::Filter::search(&filters,epoch).map_err(crate::query::operation)?;
            let sql=format!("WITH RECURSIVE wanted(pos,event,chunk) AS (VALUES {wanted}),
                messages AS (SELECT w.pos,w.chunk,e.id,e.ordinal,e.native_id,e.role,e.session_id,m.conversation_id FROM wanted w
                JOIN ah_events e ON e.id=w.event JOIN ah_sessions m ON m.id=e.session_id AND m.projection_id=e.projection_id
                WHERE m.conversation_id NOT IN ({hidden})),
                ancestors(pos,id) AS (SELECT pos,conversation_id FROM messages UNION SELECT a.pos,c.parent FROM ancestors a
                  JOIN ah_chat_summary c ON c.id=a.id AND c.live=1 WHERE c.parent IS NOT NULL)
                SELECT m.pos,s.id,coalesce(u.title,s.title),s.harness,m.conversation_id,m.session_id,m.ordinal,m.native_id,c.publication,m.chunk,s.last_activity,coalesce(u.put_away,0),s.missing,{body}
                FROM ancestors a JOIN ah_chat_summary s ON s.id=a.id AND s.live=1 AND s.root=1
                JOIN messages m ON m.pos=a.pos JOIN ah_events e ON e.id=m.id
                JOIN ah_sessions source ON source.id=m.session_id
                JOIN ah_chat_summary c ON c.id=m.conversation_id AND c.live=1
                LEFT JOIN ah_user_state u ON u.conversation_id=s.id WHERE {} ORDER BY m.pos",filter.sql,body=BODY.replace("m.","source."));
            let mut out=Vec::new();
            s.for_each(&sql,filter.args,|r|{
                let chunk=r.get::<i64>(9)? as usize;
                if let Some(p)=chunks(r,13)?.into_iter().nth(chunk) && roles.contains(&p.role){
                    out.push((r.get::<i64>(0)? as usize,json!({"id":r.get::<String>(1)?,"title":r.get::<Option<String>>(2)?,"harness":r.get::<String>(3)?,
                        "address":{"conversationId":r.get::<String>(4)?,"sourceId":r.get::<String>(5)?,"ordinal":r.get::<i64>(6)?,"nativeId":r.get::<Option<String>>(7)?,"publication":r.get::<String>(8)?},
                        "role":p.role,"part":p.part,"start":p.start,"text":p.text,"lastActivityAt":r.get::<Option<i64>>(10)?,"archived":r.get::<i64>(11)?!=0,"onlyInPathKeep":r.get::<i64>(12)?!=0})));
                }
                Ok(())
            })?;
            Ok(out)
        })?)
    }
}

impl Archive {
    /// Continue a long reader row as typed text without sharing its raw JSON or attachments.
    pub fn ai_record_chunks(
        &self,
        id: &str,
        publication: Option<&str>,
        source: &str,
        ordinal: u64,
        first: usize,
        limit: usize,
    ) -> Result<serde_json::Value> {
        use serde_json::json;
        self.ensure_ai_visible(id)?;
        let head = self.screen_head(id, publication)?;
        let (version, end) = self.member_version(&head, source)?;
        if ordinal >= end {
            return Err(Error::NotFound("record".into()));
        }
        let sql=format!("SELECT {BODY} FROM ah_reader_version v JOIN ah_events e ON e.session_id=v.source AND e.projection_id=v.attempt
            JOIN ah_sessions m ON m.id=v.source WHERE v.token=?1 AND e.ordinal=?2");
        let parts = self.store.read(Db::AgentHistoryReadModel, move |s| {
            s.query_one(&sql, params![version, ordinal as i64], |r| chunks(r, 0))
        })?;
        let total = parts.len();
        let first = first.min(total);
        let items = parts
            .into_iter()
            .skip(first)
            .take(limit)
            .map(|c| json!({"role":c.role,"part":c.part,"start":c.start,"text":c.text}))
            .collect::<Vec<_>>();
        let end = first + items.len();
        self.screen_head(id, Some(&head.publication.0))?;
        Ok(
            json!({"id":id,"publication":head.publication,"sourceId":source,"ordinal":ordinal,"chunks":items,"nextChunk":(end<total).then_some(end),"totalChunks":total}),
        )
    }
}
