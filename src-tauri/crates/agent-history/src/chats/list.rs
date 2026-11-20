//! Live keyset paging over published root summaries. Marks are joined at read
//! time so rename, archive and restore take effect on the next request.
use crate::{Archive, Error, Harness, Result, query::operation};
use chrono::{Days, NaiveDate, TimeZone, Utc};
use chrono_tz::Tz;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use vault_store::{Db, OptionalRow, Session, Value as Sql};
const DB: Db = Db::AgentHistoryReadModel;

#[derive(Clone, Copy, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum Sort {
    #[default]
    LastActivity,
    Started,
    AgentTime,
    Tokens,
    Turns,
}
impl Sort {
    fn key(self) -> &'static str {
        match self {
            Self::LastActivity => "recent_key",
            Self::Started => "start_key",
            Self::AgentTime => "agent_key",
            Self::Tokens => "tokens_key",
            Self::Turns => "turns_key",
        }
    }
}
#[derive(Clone, Copy, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum Group {
    None,
    #[default]
    Day,
    Project,
}
#[derive(Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum Flag {
    Starred,
    OnlyInPathKeep,
    HasPr,
    HasSubagents,
    HasEdits,
    NoUsage,
    Archived,
    Trash,
}
#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "camelCase")]
pub struct ListQuery {
    pub harnesses: Vec<Harness>,
    pub project_id: Option<String>,
    pub flags: Vec<Flag>,
    pub sort: Sort,
    pub group: Group,
    pub range: Option<String>,
    pub day: Option<NaiveDate>,
    pub time_zone: Option<String>,
    pub cursor: Option<String>,
    pub limit: Option<usize>,
}

const JOIN: &str = " FROM ah_chat_summary s LEFT JOIN ah_user_state u ON u.conversation_id=s.id ";
const FIELDS:&str="s.id,s.harness,(SELECT json_object('id',p.project,'label',p.label,'kind',p.kind) FROM ah_project_folder p WHERE p.path=s.folder),coalesce(u.title,s.title),CASE WHEN u.title IS NULL THEN s.title_source ELSE 'user' END,
    s.started,s.last_activity,coalesce(u.starred,0),coalesce(u.put_away,0),u.trashed_at,s.missing,s.children,
    s.messages,s.turns,s.agent_ms,
    s.tokens_total,s.bytes,s.saved,s.publication";
pub(crate) fn project_chip(r: &vault_store::Row, column: usize) -> vault_store::Result<Value> {
    r.get::<Option<String>>(column)?
        .map(|raw| serde_json::from_str(&raw).map_err(operation))
        .transpose()
        .map(|p| p.unwrap_or(Value::Null))
}
fn row(r: &vault_store::Row) -> vault_store::Result<Value> {
    Ok(
        json!({"id":r.get::<String>(0)?,"harness":r.get::<String>(1)?,"project":project_chip(r,2)?,"title":r.get::<Option<String>>(3)?,
        "titleSource":r.get::<String>(4)?,"startedAt":r.get::<Option<i64>>(5)?,"lastActivityAt":r.get::<Option<i64>>(6)?,"starred":r.get::<i64>(7)?!=0,
        "archived":r.get::<i64>(8)?!=0,"trashed":r.get::<Option<i64>>(9)?.is_some(),"onlyInPathKeep":r.get::<i64>(10)?!=0,"childCount":r.get::<i64>(11)?,
        "messages":r.get::<i64>(12)?,"turns":r.get::<i64>(13)?,"agentMs":r.get::<Option<i64>>(14)?,"tokensTotal":r.get::<Option<i64>>(15)?,
        "sourceBytes":r.get::<i64>(16)?,"lastSavedAt":r.get::<Option<i64>>(17)?,"publication":r.get::<String>(18)?}),
    )
}
pub(super) fn summary(tx: &Session<'_>, id: &str) -> vault_store::Result<Option<Value>> {
    tx.query_one(&format!("SELECT {FIELDS}{JOIN} WHERE s.id=?1 AND s.live=1"), [id], row).optional()
}
pub(crate) fn recent_days(zone_name: Option<String>) -> Result<(i64, i64)> {
    let q = ListQuery { time_zone: zone_name, range: Some("30d".into()), ..Default::default() };
    Ok(interval(&q, zone(&q)?)?.expect("bounded range"))
}
fn zone(q: &ListQuery) -> Result<Tz> {
    q.time_zone
        .as_deref()
        .unwrap_or("UTC")
        .parse()
        .map_err(|_| Error::InvalidInput("unknown IANA time zone".into()))
}
fn midnight(day: NaiveDate, tz: Tz) -> Result<i64> {
    let start = day.and_hms_opt(0, 0, 0).unwrap();
    (0..=1440)
        .find_map(|m| tz.from_local_datetime(&(start + chrono::Duration::minutes(m))).earliest())
        .map(|t| t.timestamp_millis())
        .ok_or_else(|| Error::InvalidInput("local date outside range".into()))
}
fn interval(q: &ListQuery, tz: Tz) -> Result<Option<(i64, i64)>> {
    let today = Utc::now().with_timezone(&tz).date_naive();
    let (lo, hi) = match (q.day, q.range.as_deref().unwrap_or("all")) {
        (Some(d), "all") => (d, d),
        (None, "all") => return Ok(None),
        (None, r) => {
            let days = match r {
                "today" => 1,
                "7d" => 7,
                "30d" => 30,
                "90d" => 90,
                "1y" => 365,
                _ => return Err(Error::InvalidInput("invalid time range".into())),
            };
            (today - Days::new(days - 1), today)
        }
        _ => return Err(Error::InvalidInput("choose a range or day".into())),
    };
    Ok(Some((
        midnight(lo, tz)?,
        midnight(
            hi.succ_opt().ok_or_else(|| Error::InvalidInput("invalid local date".into()))?,
            tz,
        )?,
    )))
}
pub(crate) struct Filter {
    pub(crate) sql: String,
    pub(crate) args: Vec<Sql>,
}
impl Filter {
    pub(crate) fn search(q: &ListQuery, epoch: i64) -> Result<Self> {
        Ok(Self::build(q, interval(q, zone(q)?)?, Some(epoch)))
    }
    fn new(q: &ListQuery, bounds: Option<(i64, i64)>) -> Self {
        Self::build(q, bounds, None)
    }
    fn build(q: &ListQuery, bounds: Option<(i64, i64)>, search_epoch: Option<i64>) -> Self {
        let include_archived = search_epoch.is_some();
        let mut sql =
            String::from(if include_archived { "s.root=1" } else { "s.live=1 AND s.root=1" });
        let mut args = vec![];
        if !q.harnesses.is_empty() {
            sql.push_str(&format!(
                " AND s.harness IN ({})",
                vec!["?"; q.harnesses.len()].join(",")
            ));
            args.extend(q.harnesses.iter().map(|h| Sql::Text(h.to_string())));
        }
        if let Some(p) = &q.project_id {
            sql.push_str(" AND s.project_id=coalesce((SELECT project FROM ah_project_folder WHERE path=?),?)");
            args.extend([Sql::Text(p.clone()), Sql::Text(p.clone())]);
        }
        if q.flags.contains(&Flag::Trash) {
            sql.push_str(" AND u.trashed_at IS NOT NULL");
            if q.flags.contains(&Flag::Archived) {
                sql.push_str(" AND u.put_away=1");
            }
        } else {
            sql.push_str(" AND u.trashed_at IS NULL");
            sql.push_str(if q.flags.contains(&Flag::Archived) {
                " AND u.put_away=1"
            } else if include_archived {
                ""
            } else {
                " AND coalesce(u.put_away,0)=0"
            });
        }
        for flag in &q.flags {
            sql.push_str(match flag {
                Flag::Starred => " AND u.starred=1",
                Flag::OnlyInPathKeep => " AND s.missing=1",
                Flag::HasSubagents => " AND s.children>0",
                Flag::HasPr => " AND s.has_pr=1",
                Flag::HasEdits => " AND s.has_edits=1",
                Flag::NoUsage => " AND s.tokens_total IS NULL",
                _ => "",
            });
        }
        if let Some((lo, hi)) = bounds {
            // Each hour is shorter than the requested whole local days. Its
            // first/last observations answer the two boundary buckets exactly.
            let children = match search_epoch {
                None => {
                    "ah_chat_summary child ON child.parent=members.id AND child.live=1".to_owned()
                }
                Some(epoch) => format!(
                    "ah_search_card child ON child.parent=members.id AND child.born<={epoch} AND (child.retired IS NULL OR child.retired>{epoch})"
                ),
            };
            sql.push_str(&format!(" AND ((s.started>=? AND s.started<?) OR (s.last_activity>=? AND s.last_activity<?) OR EXISTS(
              WITH RECURSIVE members(id,pub) AS (SELECT s.id,s.publication UNION ALL
                SELECT child.id,child.publication FROM members JOIN {children})
              SELECT 1 FROM members WHERE EXISTS(SELECT 1 FROM ah_chat_activity a WHERE a.publication=members.pub AND a.hour BETWEEN ? AND ? AND a.last_ms>=? AND a.first_ms<?)
              OR EXISTS(SELECT 1 FROM ah_chat_turn t WHERE t.publication=members.pub AND t.source NOT LIKE 'leaf/%' AND t.started<? AND t.ended>?)))"));
            args.extend(
                [
                    lo,
                    hi,
                    lo,
                    hi,
                    lo.div_euclid(3_600_000),
                    (hi - 1).div_euclid(3_600_000),
                    lo,
                    hi,
                    hi,
                    lo,
                ]
                .map(Sql::Integer),
            );
        }
        Self { sql, args }
    }
}
#[derive(Serialize, Deserialize)]
struct Cursor {
    scope: String,
    key: String,
}
/// Counts are reusable only for the same summary and mark snapshot. Keys are
/// local days in the caller's zone, including the undated group.
#[derive(Default)]
pub(crate) struct DayCounts {
    version: (i64, i64),
    values: std::collections::HashMap<(String, Option<NaiveDate>), i64>,
}
pub(crate) fn view(tx: &Session<'_>) -> vault_store::Result<(i64, i64)> {
    tx.query_one("SELECT c.publication,m.generation FROM ah_chat_clock c JOIN ah_state_generation m ON m.id=1 WHERE c.id=1",(),|r|Ok((r.get(0)?,r.get(1)?)))
}
impl Archive {
    /// Current display title for a local Ask citation, including a user rename.
    pub fn citation_title(&self, id: &str) -> Result<Option<String>> {
        self.ensure_state_visible()?;
        let id = id.to_owned();
        Ok(self.store.read(DB, move |tx|tx.query_one(&format!(
            "SELECT coalesce(u.title,s.title){JOIN} WHERE s.live=1 AND s.id=?1 AND u.trashed_at IS NULL"
        ), [id], |r| Ok(r.get::<Option<String>>(0)?)).optional().map(Option::flatten))?)
    }
    /// Lets a backup notification and a list response identify the same committed screen view.
    pub fn chat_view_token(&self) -> Result<String> {
        let (heads, marks) = self.store.read(DB, view)?;
        Ok(format!("{heads}:{marks}"))
    }
    pub fn chat_list(&self, q: &ListQuery) -> Result<Value> {
        let _guard = self.jobs.enter(None)?;
        self.ensure_state_visible()?;
        self.ensure_projects()?;
        let tz = zone(q)?;
        let bounds = interval(q, tz)?;
        let limit = q.limit.unwrap_or(50);
        if !(1..=200).contains(&limit) {
            return Err(Error::InvalidInput("list limit must be 1..200".into()));
        }
        let mut shape = q.clone();
        shape.cursor = None;
        shape.limit = None;
        let scope = hex::encode(Sha256::digest(serde_json::to_vec(&(shape, bounds))?));
        let key = q
            .cursor
            .as_deref()
            .map(|c| -> Result<String> {
                let cursor = hex::decode(c)
                    .ok()
                    .and_then(|b| serde_json::from_slice::<Cursor>(&b).ok())
                    .filter(|c| c.scope == scope)
                    .ok_or_else(|| Error::InvalidInput("invalid list cursor".into()))?;
                Ok(cursor.key)
            })
            .transpose()?;
        let f = Filter::new(q, bounds);
        let column = q.sort.key();
        let group =
            if q.group == Group::Day && !matches!(q.sort, Sort::LastActivity | Sort::Started) {
                Group::None
            } else {
                q.group
            };
        let date_column = if q.sort == Sort::Started { "started" } else { "last_activity" };
        let day_cache = self.chat_days.clone();
        let count_scope = scope.clone();
        let encode = move |key: String| {
            hex::encode(
                serde_json::to_vec(&Cursor { scope: scope.clone(), key }).expect("scalar cursor"),
            )
        };
        Ok(self.store.read(DB,move|tx|{
            let version=view(tx)?;let view_token=format!("{}:{}",version.0,version.1);
            if group==Group::Project{
                let mut args=f.args.clone();
                let having=if let Some(k)=key{args.push(Sql::Text(k));" HAVING max(s.KEY)<?".replace("KEY",column)}else{String::new()};
                args.push(Sql::Integer(limit as i64+1));
                let mut groups=tx.query(&format!("SELECT s.project_id,count(*),max(s.{column}),(SELECT json_object('id',p.project,'label',p.label,'kind',p.kind) FROM ah_project_folder p WHERE p.path=s.folder){JOIN} WHERE {} GROUP BY s.project_id {having} ORDER BY max(s.{column}) DESC LIMIT ?",f.sql),args,|r|Ok((r.get::<Option<String>>(0)?,r.get::<i64>(1)?,r.get::<String>(2)?,project_chip(r,3)?)))?;
                let more=groups.len()>limit;groups.truncate(limit);
                let next=more.then(||encode(groups.last().unwrap().2.clone()));
                let mut out=vec![];
                for (folder,count,_,chip) in groups{
                    let mut args=f.args.clone();args.push(folder.clone().map(Sql::Text).unwrap_or(Sql::Null));
                    let items=tx.query(&format!("SELECT {FIELDS}{JOIN} WHERE {} AND s.project_id IS ? ORDER BY s.{column} DESC LIMIT 3",f.sql),args,row)?;
                    out.push(json!({"project":chip,"count":count,"remaining":count-items.len() as i64,"items":items}));
                }
                return Ok(json!({"groups":out,"items":[],"nextCursor":next,"viewToken":view_token}))
            }
            let mut args=f.args.clone();
            let seek=if let Some(k)=key{args.push(Sql::Text(k));format!(" AND s.{column}<?")}else{String::new()};
            args.push(Sql::Integer(limit as i64+1));
            let mut rows=tx.query(&format!("SELECT {FIELDS},s.{column}{JOIN} WHERE {}{seek} ORDER BY s.{column} DESC LIMIT ?",f.sql),args,|r|Ok((row(r)?,r.get::<String>(19)?)))?;
            let more=rows.len()>limit;rows.truncate(limit);
            let next=more.then(||encode(rows.last().unwrap().1.clone()));
            let mut groups=vec![];let mut items=vec![];
            for (mut item,_) in rows{
                if group==Group::Day{
                    let date=item[if date_column=="started"{"startedAt"}else{"lastActivityAt"}].as_i64().and_then(chrono::DateTime::from_timestamp_millis).map(|t|t.with_timezone(&tz).date_naive());
                    let value=json!(date);item["day"]=value.clone();
                    if !groups.iter().any(|g:&Value|g["day"]==value){
                        let mut args=f.args.clone();
                        let pred=if let Some(d)=date{let lo=midnight(d,tz).map_err(operation)?;let hi=midnight(d.succ_opt().unwrap(),tz).map_err(operation)?;args.extend([lo,hi].map(Sql::Integer));format!("s.{date_column}>=? AND s.{date_column}<?")}else{format!("s.{date_column} IS NULL")};
                        let key=(count_scope.clone(),date);
                        let cached={let cache=day_cache.lock().unwrap();(cache.version==version).then(||cache.values.get(&key).copied()).flatten()};
                        let count=match cached{Some(n)=>n,None=>{
                            let n=tx.query_one(&format!("SELECT count(*){JOIN} WHERE {} AND {pred}",f.sql),args,|r|Ok(r.get::<i64>(0)?))?;
                            let mut cache=day_cache.lock().unwrap();
                            if cache.version!=version || cache.values.len()>=1024 {cache.values.clear();cache.version=version;}
                            cache.values.insert(key,n);n
                        }};
                        groups.push(json!({"day":value,"count":count}));
                    }
                }
                items.push(item);
            }
            Ok(json!({"items":items,"groups":groups,"nextCursor":next,"viewToken":view_token}))
        })?)
    }

    pub fn chat_counts(&self, q: &ListQuery) -> Result<Value> {
        let _guard = self.jobs.enter(None)?;
        self.ensure_state_visible()?;
        self.ensure_projects()?;
        let f = Filter::new(q, interval(q, zone(q)?)?);
        Ok(self.store.read(DB,move|tx|{
            let (matching,children)=tx.query_one(&format!("SELECT count(*),coalesce(sum(s.children),0){JOIN} WHERE {}",f.sql),f.args,|r|Ok((r.get::<i64>(0)?,r.get::<i64>(1)?)))?;
            let (saved,space,missing)=tx.query_one(&format!("SELECT count(*),count(CASE WHEN u.trashed_at IS NULL THEN 1 END),count(CASE WHEN u.trashed_at IS NULL AND s.missing=1 THEN 1 END){JOIN} WHERE s.live=1 AND s.root=1"),(),|r|Ok((r.get::<i64>(0)?,r.get::<i64>(1)?,r.get::<i64>(2)?)))?;
            let (saved_children,only_in_archive,projects)=tx.query_one("SELECT count(CASE WHEN root=0 THEN 1 END),count(CASE WHEN root=1 AND missing=1 THEN 1 END),count(DISTINCT CASE WHEN s.root=1 AND u.trashed_at IS NULL THEN s.folder END) FROM ah_chat_summary s LEFT JOIN ah_user_state u ON u.conversation_id=s.id WHERE s.live=1",(),|r|Ok((r.get::<i64>(0)?,r.get::<i64>(1)?,r.get::<i64>(2)?)))?;
            let generation=view(tx)?;
            Ok(json!({"viewToken":format!("{}:{}",generation.0,generation.1),"savedChildren":saved_children,"savedOnlyInPathKeep":only_in_archive,"folderCount":projects,"matchingRoots":matching,"matchingChildren":children,"savedRoots":saved,"spaceRoots":space,"spaceOnlyInPathKeep":missing}))
        })?)
    }
}

impl Archive {
    /// Palette title matching uses effective titles for every saved root, in recency order.
    /// Only matching IDs are expanded into cards. A superseded search can leave between rows.
    pub fn palette_conversations(
        &self,
        text: &str,
        job: &crate::read_model::JobControl,
    ) -> Result<Vec<Value>> {
        self.ensure_projects()?;
        self.ensure_state_visible()?;
        let needle = text.trim().to_lowercase();
        let cancel = job.cancel.clone();
        Ok(self.store.read(Db::AgentHistoryReadModel,move|s|{
            let mut statement=s.prepare(&format!("SELECT s.id,coalesce(u.title,s.title,''){JOIN} WHERE s.live=1 AND s.root=1 AND u.trashed_at IS NULL AND (?1 OR coalesce(u.put_away,0)=0) ORDER BY s.recent_key DESC"))?;
            let mut rows=statement.query([i64::from(!needle.is_empty())])?;
            let mut matches=Vec::new();
            while let Some(r)=rows.next()?{
                cancel.check()?;
                let title=r.get::<String>(1)?;
                if needle.is_empty()||title.to_lowercase().contains(&needle){
                    matches.push(r.get::<String>(0)?);
                    if matches.len()==6{break;}
                }
            }
            matches.into_iter().filter_map(|id|summary(s,&id).transpose()).collect()
        })?)
    }
}

#[cfg(test)]
mod measurement {
    use super::*;
    use vault_store::{Priority, params};
    #[test]
    #[ignore = "Linux release list seek on 120,000 materialized roots"]
    fn list_keysets_at_120000_roots() {
        let tmp = tempfile::tempdir().unwrap();
        let a = Archive::create_unencrypted(tmp.path()).unwrap();
        let total = serde_json::to_string(&super::super::facts::Totals {
            messages: 30,
            turns: 30,
            calls: 30,
            usage_responses: 60,
            tokens: Some(super::super::facts::Tokens {
                input: 600,
                output: 120,
                ..Default::default()
            }),
            agent_ms: Some(720_000),
            ..Default::default()
        })
        .unwrap();
        for start in (0..120_000i64).step_by(500) {
            let total = total.clone();
            a.store.write(DB,Priority::Batch,move|s|{
                let mut insert=s.prepare("INSERT INTO ah_chat_summary(publication,id,live,root,harness,folder,title,title_source,started,last_activity,missing,bytes,saved,own,totals,detail,recent_key,start_key,agent_key,tokens_key,turns_key,messages,turns,agent_ms,tokens_total) VALUES(?1,?1,1,1,'pi',?2,?1,'tool',?3,?3,0,48000,?3,?4,?4,'{}',?5,?5,?6,?7,?8,30,30,720000,720)")?;
                for n in start..start+500{
                    let id=format!("pi-{n:06}");let at=1_790_000_000_000+(n/4)*60_000;
                    insert.execute(params![id.as_str(),format!("/projects/{}",n%120),at,total.as_str(),super::super::publish::order_key(Some(at),&id),super::super::publish::order_key(Some(720000),&id),super::super::publish::order_key(Some(720),&id),super::super::publish::order_key(Some(30),&id)])?;
                }
                Ok(())
            }).unwrap();
        }
        let q = ListQuery { limit: Some(50), ..Default::default() };
        let mut shape = q.clone();
        shape.limit = None;
        let scope =
            hex::encode(Sha256::digest(serde_json::to_vec(&(shape, None::<(i64, i64)>)).unwrap()));
        for (position, next) in [("first", 119999i64), ("deep", 11999)] {
            let mut q = q.clone();
            if position == "deep" {
                let prior = next + 1;
                q.cursor = Some(hex::encode(
                    serde_json::to_vec(&Cursor {
                        scope: scope.clone(),
                        key: super::super::publish::order_key(
                            Some(1_790_000_000_000 + (prior / 4) * 60_000),
                            &format!("pi-{prior:06}"),
                        ),
                    })
                    .unwrap(),
                ));
            }
            let mut times = vec![];
            for _ in 0..40 {
                let t = std::time::Instant::now();
                let page = a.chat_list(&q).unwrap();
                times.push(t.elapsed().as_micros());
                let ids = page["items"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|v| v["id"].as_str().unwrap().to_string())
                    .collect::<Vec<_>>();
                assert_eq!(ids, (0..50).map(|i| format!("pi-{:06}", next - i)).collect::<Vec<_>>());
            }
            a.chat_days.lock().unwrap().values.clear();
            a.store.capture_query_plans(DB, true).unwrap();
            a.chat_list(&q).unwrap();
            let plans = a.store.take_query_plans(DB).unwrap();
            let plan = plans.iter().find(|p| p.sql.contains("ORDER BY s.recent_key")).unwrap();
            let group = plans
                .iter()
                .find(|p| p.sql.contains("SELECT count(*)") && p.sql.contains("s.last_activity>="))
                .unwrap();
            let uncached = times[0];
            times.sort_unstable();
            println!(
                "{}",
                json!({"roots":120000,"page":position,"rows":50,"samples":40,"firstUs":uncached,"p50Us":times[19],"p95Us":times[37],"plan":plan.steps,"dayCountPlan":group.steps})
            );
            assert!(plan.steps.iter().any(|s| s.contains("ah_chat_recent")));
            if position == "deep" {
                assert!(plan.steps.iter().any(|s| s.contains("recent_key<")), "{:?}", plan.steps);
            }
        }
    }
}
