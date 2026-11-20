//! Reader details and branch/child cards. Folder facts come from archived observations.
use super::{BranchesQuery, facts::Totals, list::summary};
use crate::{Archive, Error, Result, read_model::JobControl};
use serde_json::{Value, json};
use vault_store::{Db, OptionalRow, params};
const DB: Db = Db::AgentHistoryReadModel;
impl Archive {
    pub(super) fn screen_folder(&self, path: Option<&str>) -> Result<Value> {
        let Some(path) = path else { return Ok(Value::Null) };
        let path = path.to_owned();
        Ok(self.store.read(Db::AgentHistoryEvidence,move|s|{
            let row=s.query_one(
                "SELECT f.exists_on_disk,f.observed_at,coalesce(f.git_json,(SELECT prior.git_json
                 FROM ah_folder_observations prior WHERE f.exists_on_disk=0 AND prior.path=f.path AND prior.git_json IS NOT NULL
                 ORDER BY prior.id DESC LIMIT 1)) FROM ah_folder_observations f WHERE f.path=?1 ORDER BY f.id DESC LIMIT 1",
                [path.as_str()],|r|Ok((r.get::<i64>(0)?!=0,r.get::<i64>(1)?,r.get::<Option<String>>(2)?))
            ).optional()?;
            let Some((exists,at,raw))=row else{return Ok(Value::Null)};
            let git=raw.map(|s|serde_json::from_str::<Value>(&s)).transpose().map_err(crate::query::operation)?;
            let git=git.map_or(Value::Null,|g|json!({"topLevel":g["top_level"],"commonDir":g["common_dir"],"branch":g["branch"],"remotes":g["remotes"]}));
            let worktree=git["topLevel"].as_str().zip(git["commonDir"].as_str()).is_some_and(|(top,common)|
                format!("{}/.git",top.replace('\\',"/").trim_end_matches('/'))!=common.replace('\\',"/"));
            Ok(json!({"state":if exists{"present"}else if worktree{"worktreeGone"}else{"gone"},"git":git,"observation":{"path":path,"exists":exists,"observedAt":at}}))
        })?)
    }
    pub fn chat_info(&self, id: &str) -> Result<Value> {
        let h = self.reader_header(id)?;
        let p = h.publication.0.clone();
        let (own,total,detail,folder,native,origin,version,paths)=self.store.read(DB,move|s|{
            let (own,total,detail,folder,native,origin,version)=s.query_one("SELECT c.own,c.totals,c.detail,c.folder,m.native_id,m.origin,m.harness_version FROM ah_chat_summary c JOIN ah_reader_head h ON h.publication=c.publication JOIN ah_sessions m ON m.id=h.current_source WHERE c.publication=?1",[p.as_str()],|r|Ok((r.get::<String>(0)?,r.get::<String>(1)?,r.get::<String>(2)?,r.get::<Option<String>>(3)?,r.get::<String>(4)?,r.get::<String>(5)?,r.get::<Option<String>>(6)?)))?;
            let paths=s.query_one("SELECT count(*) FROM ah_reader_member WHERE publication=?1",[p],|r|Ok(r.get::<i64>(0)?))?;
            Ok((own,total,detail,folder,native,origin,version,paths))
        })?;
        let own: Totals = serde_json::from_str(&own)?;
        let total: Totals = serde_json::from_str(&total)?;
        let observation = self.screen_folder(folder.as_deref())?;
        let mut models = total.models.iter().collect::<Vec<_>>();
        models.sort_by(|(a, x), (b, y)| y.cmp(x).then(a.cmp(b)));
        let primary =
            models.iter().find(|(k, _)| total.measured_models.contains(*k)).map(|(k, _)| *k);
        let model_rows = models
            .iter()
            .map(|(model, n)| json!({"model":model,"outputTokens":total.measured_models.contains(*model).then_some(*n)}))
            .collect::<Vec<_>>();
        let child_tokens = total
            .tokens
            .filter(|_| total.usage_conversations > own.usage_conversations)
            .map(|n| n.total() - own.tokens.map_or(0, |v| v.total()));
        let mut info = json!({"publication":h.publication,"own":own.wire(),"total":total.wire(),"subagentTokens":child_tokens,"usageCoverage":{"known":total.usage_conversations,"conversations":total.conversations},
            "models":model_rows,"primaryModel":primary,"nativeSessionId":(!native.is_empty()).then_some(native),"launchOrigin":origin,"toolVersion":version,"folder":folder,"folderObservation":observation["observation"],
            "sourceCount":paths,"physicalRecords":h.physical_records,"visibleRecords":h.visible_records,"resumeCommand":self.resume_command(&h)?,"estimatedUsd":total.estimated_usd,"priceTable":super::pricing::VERSION});
        let key = h.publication.0.clone();
        let src = h.current_source.clone();
        let (tokens,bad)=self.store.read(DB,move|s|s.query_one("SELECT tokens,unparsed FROM ah_chat_line_stats WHERE publication=?1 AND branch=?2",params![key,src],|r|Ok((r.get::<Option<String>>(0)?,r.get::<i64>(1)?))).optional())?.unwrap_or_default();
        info["currentLineTokens"] =
            tokens.map(|s| serde_json::from_str(&s)).transpose()?.unwrap_or(Value::Null);
        info["unparsedRecords"] = json!(bad);
        let visible = h.visible_records.saturating_sub(self.screen_mirrors(&h, &h.current_source)?)
            + self.extra_count(&h, &h.current_source)?;
        info["visibleRecords"] = json!(visible);
        info["hiddenRecords"] = json!(h.physical_records.saturating_sub(visible));
        let detail: Value = serde_json::from_str(&detail)?;
        for key in ["permissionMode", "startCommit", "effort"] {
            info[key] = detail[key].clone();
        }
        let p = h.publication.0.clone();
        let src = h.current_source.clone();
        let id = id.to_owned();
        let (root,path,bytes,saved,subfiles)=self.store.read(DB,move|s|{
            let (root,path,bytes,saved)=s.query_one("SELECT m.descriptor,c.bytes,c.saved FROM ah_reader_member m JOIN ah_chat_summary c ON c.publication=m.publication WHERE m.publication=?1 AND m.source=?2",params![p,src],|r|{
                let d:Value=serde_json::from_str(&r.get::<String>(0)?).map_err(crate::query::operation)?;
                Ok((d["root"].as_str().unwrap_or("").to_owned(),d["relative_path"].as_str().unwrap_or("").to_owned(),r.get::<i64>(1)?,r.get::<Option<i64>>(2)?))
            })?;
            let subfiles=s.query_one("WITH RECURSIVE children(id) AS (SELECT conversation FROM ah_reader_head WHERE parent=?1 UNION SELECT c.conversation FROM children JOIN ah_reader_head c ON c.parent=children.id)
                SELECT coalesce(sum(h.members),0) FROM children JOIN ah_reader_head h ON h.conversation=children.id",[id],|r|Ok(r.get::<i64>(0)?))?;
            Ok((root,path,bytes,saved,subfiles))
        })?;
        info["sourcePath"] = json!(std::path::Path::new(&root).join(path));
        info["sourceBytes"] = json!(bytes);
        info["lastSavedAt"] = json!(saved);
        info["subagentSourceCount"] = json!(subfiles);
        info["git"] = observation["git"].clone();
        info["folderState"] = observation["state"].clone();
        info["editedFileCount"] = json!(total.edited_files.len());
        info["timeSource"] = json!(match (own.recorded_turns > 0, own.derived_turns > 0) {
            (true, true) => "mixed",
            (true, false) => "recorded",
            (false, true) => "derived",
            _ => "unknown",
        });
        info["agentType"] = detail["agentType"].clone();
        info["allErrorsRetried"] = json!(
            (total.api_errors > 0 && total.retry_unknown == 0)
                .then_some(total.retried_errors == total.api_errors)
        );
        Ok(info)
    }
    pub fn chat_branches(&self, q: &BranchesQuery, job: &JobControl) -> Result<Value> {
        let h = self.screen_head(&q.id, q.publication.as_deref())?;
        let count = q.limit.unwrap_or(50);
        if !(1..=200).contains(&count) {
            return Err(Error::InvalidInput("branch limit must be 1..200".into()));
        }
        let mut members = self.reader_sources(&h, q.after.as_deref(), count + 1)?;
        let more = members.len() > count;
        members.truncate(count);
        let mut children = self.reader_children(&h, q.children_after.as_deref(), count + 1)?;
        let more_children = children.len() > count;
        children.truncate(count);
        let first = q.leaf_start.unwrap_or(0);
        let src = q.source_id.as_deref().unwrap_or(&h.current_source);
        let leaves = self.reader_leaves(&h, src, first, count, job)?;
        let versions = if let Some(ordinal) = q.prompt_ordinal {
            let p = h.publication.0.clone();
            let source = src.to_owned();
            let after = q.version_after.map_or(-1, |n| n as i64);
            self.store.read(DB,move|s|{
                let parent=s.query_one("SELECT parent FROM ah_chat_variant WHERE publication=?1 AND source=?2 AND ordinal=?3",params![p.as_str(),source.as_str(),ordinal as i64],|r|Ok(r.get::<Option<String>>(0)?)).optional()?;
                let Some(parent)=parent else{return Ok(json!({"items":[],"count":0,"next":null}))};
                let total=s.query_one("SELECT count(*) FROM ah_chat_variant WHERE publication=?1 AND source=?2 AND parent IS ?3",params![p.as_str(),source.as_str(),parent.as_deref()],|r|Ok(r.get::<i64>(0)?))?;
                let mut items=s.query("SELECT v.ordinal,v.id,v.leaf,json_extract(t.facts,'$.prompt') FROM ah_chat_variant v LEFT JOIN ah_chat_turn t ON t.publication=v.publication AND t.source=v.source AND t.ordinal=v.ordinal WHERE v.publication=?1 AND v.source=?2 AND v.parent IS ?3 AND v.ordinal>?4 ORDER BY v.ordinal LIMIT ?5",params![p,source,parent,after,count as i64+1],|r|Ok(json!({"ordinal":r.get::<i64>(0)?,"nativeId":r.get::<String>(1)?,"leaf":r.get::<i64>(2)?,"preview":r.get::<Option<String>>(3)?})))?;
                let more=items.len()>count;items.truncate(count);let next=more.then(||items.last().unwrap()["ordinal"].clone());
                Ok(json!({"items":items,"count":total,"next":next}))
            })?
        } else {
            Value::Null
        };
        let end_children = more_children.then(|| children.last().cloned()).flatten();
        let parent = h.publication.0.clone();
        let cards=self.store.read(DB,move|s|{
            let mut cards=vec![];
            for id in children {
                if let Some(mut card)=summary(s,&id)?{
                    let(detail,totals)=s.query_one("SELECT detail,totals FROM ah_chat_summary WHERE id=?1 AND live=1",[id],|r|Ok((r.get::<String>(0)?,r.get::<String>(1)?)))?;
                    let detail:Value=serde_json::from_str(&detail).map_err(crate::query::operation)?;
                    let totals:Totals=serde_json::from_str(&totals).map_err(crate::query::operation)?;
                    let launch=if let Some(call)=detail["launchCallId"].as_str(){
                        s.query_one("SELECT source,ordinal FROM ah_chat_call WHERE publication=?1 AND call_id=?2 AND ordinal IS NOT NULL LIMIT 1",params![parent.as_str(),call],|r|Ok(json!({"sourceId":r.get::<String>(0)?,"ordinal":r.get::<i64>(1)?,"callId":call}))).optional()?
                    }else if let Some(agent)=detail["agentId"].as_str(){
                        s.query_one("SELECT min(source),min(ordinal),min(call_id) FROM ah_chat_call WHERE publication=?1 AND child_ref=?2 AND ordinal IS NOT NULL HAVING count(*)=1",params![parent.as_str(),agent],|r|Ok(json!({"sourceId":r.get::<String>(0)?,"ordinal":r.get::<i64>(1)?,"callId":r.get::<String>(2)?}))).optional()?
                    }else{None};
                    card["launch"]=json!(launch);card["agentType"]=detail["agentType"].clone();
                    card["model"]=json!(totals.models.iter().max_by_key(|(_,n)|*n).map(|(m,_)|m));
                    cards.push(card);
                }
            }
            Ok(cards)
        })?;
        let end_sources = more.then(|| members.last().map(|m| m.source_id.clone())).flatten();
        let sources=members.into_iter().map(|m|json!({"id":m.source_id,"nativeId":m.native_id,"path":std::path::Path::new(&m.root).join(&m.relative_path),
            "bytes":m.bytes,"savedAt":m.observation.captured_at,"lastSeenAt":m.observation.last_seen_at,"missingSince":m.observation.missing_since,
            "current":m.source_id==h.current_source,"copyOf":m.copy_of,"base":m.resolved_base,"forkId":m.fork_id,"physicalRecords":m.line_records,"visibleRecords":m.line_visible_records,"awaitingRebuild":m.awaiting_rebuild,"rawHeadCurrent":m.raw_head_current})).collect::<Vec<_>>();
        Ok(
            json!({"publication":h.publication,"sources":sources,"sourcesAfter":end_sources,"children":cards,"versions":versions,"childrenAfter":end_children,"leaves":{"ordinals":leaves.ordinals,"total":leaves.total,"latestRecorded":leaves.latest_recorded},"leafNext":(first+ (leaves.ordinals.len() as u64)<leaves.total).then_some(first+leaves.ordinals.len() as u64)}),
        )
    }
}
