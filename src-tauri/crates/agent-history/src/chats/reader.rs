//! Window coordinates and presentation facts for one published conversation.
//! Body decoding, branches and raw byte ranges stay with the archive reader.
use super::list::summary;
use crate::{
    Archive, Error, Result,
    read_model::{JobControl, ReaderPublication},
};
use serde::Deserialize;
use serde_json::{Value, json};
use vault_store::{Db, OptionalRow, params};
const DB: Db = Db::AgentHistoryReadModel;
#[derive(Clone, Default, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum Anchor {
    #[default]
    Start,
    End,
    Rank {
        rank: u64,
    },
    Record {
        source_id: String,
        ordinal: u64,
    },
    Native {
        source_id: String,
        native_id: String,
    },
}
#[derive(Clone, Copy, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum View {
    #[default]
    Visible,
    Raw,
    Prompts,
}
#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WindowQuery {
    pub id: String,
    pub publication: Option<String>,
    pub branch: Option<String>,
    pub leaf: Option<u64>,
    #[serde(default)]
    pub view: View,
    #[serde(default)]
    pub anchor: Anchor,
    pub before: Option<usize>,
    pub limit: Option<usize>,
    pub request_id: Option<String>,
    pub show_cost: Option<bool>,
    pub estimate_cost: Option<bool>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BranchesQuery {
    pub request_id: Option<String>,
    pub id: String,
    pub publication: Option<String>,
    pub after: Option<String>,
    pub children_after: Option<String>,
    pub source_id: Option<String>,
    pub leaf_start: Option<u64>,
    pub prompt_ordinal: Option<u64>,
    pub version_after: Option<u64>,
    pub limit: Option<usize>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RawQuery {
    pub request_id: Option<String>,
    pub id: String,
    pub publication: String,
    pub source_id: String,
    pub ordinal: u64,
    pub offset: u64,
    pub limit: usize,
}

fn page(first: u64, total: u64, n: usize, rows: Vec<Value>, h: &ReaderPublication) -> Value {
    let end = first + rows.len() as u64;
    json!({"publication":h.publication,"first":first,"total":total,"previous":(first>0).then_some(first.saturating_sub(n as u64)),"next":(end<total).then_some(end),"rows":rows})
}
impl Archive {
    pub(super) fn screen_head(
        &self,
        id: &str,
        expected: Option<&str>,
    ) -> Result<ReaderPublication> {
        let h = self.reader_header(id)?;
        if expected.is_some_and(|p| p != h.publication.0) {
            return Err(Error::StaleCursor);
        }
        Ok(h)
    }
    pub fn chat_header(&self, id: &str) -> Result<Value> {
        self.ensure_projects()?;
        self.ensure_state_visible()?;
        let h = self.reader_header(id)?;
        let key = id.to_owned();
        let pub_id = h.publication.0.clone();
        let src = h.current_source.clone();
        let parent = h.parent_id.clone();
        let (summary,unparsed,prompt_count,parent_summary,view,(native,cwd))=self.store.read(DB,move|s|{
            let summary=summary(s,&key)?;
            let bad=s.query_one("SELECT unparsed FROM ah_chat_line_stats WHERE publication=?1 AND branch=?2",params![pub_id.as_str(),src.as_str()],|r|Ok(r.get::<i64>(0)?)).optional()?.unwrap_or(0);
            let prompts=s.query_one("SELECT coalesce(max(rank)+1,0) FROM ah_chat_prompt WHERE publication=?1 AND branch=?2",params![pub_id,src.as_str()],|r|Ok(r.get::<i64>(0)?))?;
            let parent=parent.map(|id|super::list::summary(s,&id)).transpose()?.flatten();
            let native=s.query_one("SELECT native_id,project_path FROM ah_sessions WHERE id=?1",[src],|r|Ok((r.get::<String>(0)?,r.get::<Option<String>>(1)?)))?;
            Ok((summary,bad,prompts,parent,super::list::view(s)?,native))
        })?;
        let summary = summary.ok_or(Error::ConversationNotFound)?;
        if summary["publication"] != h.publication.0 {
            return Err(Error::StaleCursor);
        }
        let folder = self.screen_folder(cwd.as_deref())?;
        let visible = h.visible_records.saturating_sub(self.screen_mirrors(&h, &h.current_source)?)
            + self.extra_count(&h, &h.current_source)?;
        Ok(
            json!({"summary":summary,"viewToken":format!("{}:{}",view.0,view.1),"publication":h.publication,"currentSource":h.current_source,"parentId":h.parent_id,"parentStatus":h.parent_status,
            "folderState":folder["state"],"nativeSessionId":(!native.is_empty()).then_some(native),"resumeCommand":null,"unparsedRecords":unparsed,"promptCount":prompt_count,"parent":parent_summary,"physicalRecords":h.physical_records,"visibleRecords":visible,"coverage":{"complete":h.complete,"reason":h.coverage_reason},"attachmentsNotKept":h.attachments_not_kept}),
        )
    }
    pub fn chat_raw(&self, q: &RawQuery, job: &JobControl) -> Result<Value> {
        let h = self.screen_head(&q.id, Some(&q.publication))?;
        if !(1..=65536).contains(&q.limit) {
            return Err(Error::InvalidInput("raw limit must be 1..65536 bytes".into()));
        }
        Ok(serde_json::to_value(self.reader_record_bytes(
            &h,
            &q.source_id,
            q.ordinal,
            q.offset,
            q.limit,
            job,
        )?)?)
    }
    pub fn chat_window(&self, q: &WindowQuery, job: &JobControl) -> Result<Value> {
        let h = self.screen_head(&q.id, q.publication.as_deref())?;
        let branch = q.branch.as_deref().unwrap_or(&h.current_source);
        let n = q.limit.unwrap_or(50);
        if !(1..=200).contains(&n) || q.before.unwrap_or(0) >= n {
            return Err(Error::InvalidInput("invalid window size or anchor position".into()));
        }
        let leaf_facts = if matches!(q.view, View::Raw) && q.leaf.is_none() {
            None
        } else {
            self.selected_leaf_facts(&h.publication.0, branch, q.leaf, job)?
        };
        if matches!(q.view, View::Prompts) {
            return self.prompt_window(&h, q, leaf_facts.as_deref().unwrap_or(branch), n);
        }
        let raw = matches!(q.view, View::Raw);
        let mirrors = if !raw && q.leaf.is_none() { self.screen_mirrors(&h, branch)? } else { 0 };
        let display_key =
            if q.leaf.is_some() { leaf_facts.as_deref().unwrap_or(branch) } else { branch };
        let additions = if raw { 0 } else { self.extra_count(&h, display_key)? };
        let total =
            additions + self.screen_length(&h, branch, q.leaf, raw, job)?.saturating_sub(mirrors);
        let first = match &q.anchor {
            Anchor::Start => 0,
            Anchor::End => total.saturating_sub(n as u64),
            Anchor::Rank { rank } => *rank,
            anchor => {
                let (source, ordinal) = match anchor {
                    Anchor::Record { source_id, ordinal } => (source_id, *ordinal),
                    Anchor::Native { source_id, native_id } => (
                        source_id,
                        *self
                            .reader_lookup(&h, source_id, native_id, None, 1, job)?
                            .first()
                            .ok_or_else(|| Error::NotFound("native record".into()))?,
                    ),
                    _ => unreachable!(),
                };
                let extra = if additions > 0 {
                    self.added_rank(&h, display_key, source, ordinal)?
                } else {
                    None
                };
                let rank = match extra {
                    Some(n) => n,
                    None => {
                        let rank = self.screen_rank(&h, branch, q.leaf, raw, source, ordinal)?;
                        let rank = if mirrors > 0 {
                            self.without_mirrors(&h, branch, rank)?
                        } else {
                            rank
                        };
                        if additions > 0 { self.with_extras(&h, display_key, rank)? } else { rank }
                    }
                };
                rank.saturating_sub(q.before.unwrap_or(n / 2) as u64)
            }
        }
        .min(total);
        if first == total {
            return Ok(page(first, total, n, vec![], &h));
        }
        if raw {
            let outline = if let Some(leaf) = q.leaf {
                self.screen_leaf_outline(&h, branch, leaf, first, n, job)?
            } else {
                self.reader_outline(&h, Some(branch), first, n, job)?
            };
            let rows = self.screen_raw_metadata(&h, branch, q.leaf, outline, job)?;
            return Ok(page(first, total, n, rows, &h));
        }
        let take = n.min((total - first) as usize);
        let (prior, mut extra) = if additions > 0 {
            self.extra_page(&h, display_key, first, take, job)?
        } else {
            (0, Default::default())
        };
        let plain = take - extra.len();
        let regular = if plain == 0 {
            vec![]
        } else if let Some(leaf) = q.leaf {
            self.screen_leaf_rows(&h, branch, leaf, first - prior, plain, job)?
        } else {
            self.display_page(&h, branch, first - prior, plain, mirrors, job)?
        };
        let mut regular = regular.into_iter();
        let rows = (first..first + take as u64)
            .filter_map(|rank| extra.remove(&rank).or_else(|| regular.next()))
            .collect();
        let p = h.publication.0.clone();
        let selected_branch = branch.to_owned();
        let rows = self.store.read(DB, move |s| {
            super::annotations::annotate(s, &p, rows, &selected_branch, leaf_facts.as_deref())
        })?;
        let mut reply = page(first, total, n, rows, &h);
        if q.show_cost == Some(false) || q.estimate_cost == Some(false) {
            for row in reply["rows"].as_array_mut().unwrap() {
                if let Some(response) = row["response"].as_object_mut() {
                    response.remove("estimatedUsd");
                    if q.show_cost == Some(false) {
                        response.remove("recordedUsd");
                    }
                }
                if let Some(facts) = row.pointer_mut("/footer/facts").and_then(Value::as_object_mut)
                {
                    facts.remove("estimatedUsd");
                    if q.show_cost == Some(false) {
                        facts.remove("recordedUsd");
                    }
                }
            }
        }
        Ok(reply)
    }
    fn prompt_window(
        &self,
        h: &ReaderPublication,
        q: &WindowQuery,
        branch: &str,
        n: usize,
    ) -> Result<Value> {
        let p = h.publication.0.clone();
        let branch = branch.to_owned();
        let anchor = q.anchor.clone();
        let before = q.before.unwrap_or(0) as u64;
        let (total,first,rows)=self.store.read(DB,move|s|{
            let total=s.query_one("SELECT coalesce(max(rank)+1,0) FROM ah_chat_prompt WHERE publication=?1 AND branch=?2",params![p.as_str(),branch.as_str()],|r|Ok(r.get::<i64>(0)? as u64))?;
            let first=match anchor{
                Anchor::Start=>0, Anchor::End=>total.saturating_sub(n as u64), Anchor::Rank{rank}=>rank.min(total),
                Anchor::Record{source_id,ordinal}=>s.query_one("SELECT rank FROM ah_chat_prompt WHERE publication=?1 AND branch=?2 AND source=?3 AND ordinal=?4",
                    params![p.as_str(),branch.as_str(),source_id,ordinal as i64],|r|Ok(r.get::<i64>(0)? as u64)).optional()?.ok_or_else(||crate::query::operation(Error::NotFound("prompt".into())))?.saturating_sub(before),
                Anchor::Native{..}=>return Err(crate::query::operation(Error::InvalidInput("use a prompt rank or record".into()))),
            };
            let rows=s.query("SELECT rank,source,ordinal,visible_rank,preview FROM ah_chat_prompt WHERE publication=?1 AND branch=?2 AND rank>=?3 ORDER BY rank LIMIT ?4",params![p,branch,first as i64,n as i64],|r|Ok(json!({"turn":r.get::<i64>(0)?+1,"sourceId":r.get::<String>(1)?,"ordinal":r.get::<i64>(2)?,"visibleRank":r.get::<i64>(3)?,"preview":r.get::<String>(4)?})))?;
            Ok((total,first,rows))
        })?;
        Ok(page(first, total, n, rows, h))
    }
}
