//! Quantities on the displayed line. The summary separately counts original
//! work on abandoned suffixes and the root's child conversations.
use super::{Entry, OUTLINE, leaves, pages};
use crate::{
    Archive, Result,
    chats::facts::{Interpreter, Tokens, Turn},
    query::operation,
    read_model::{JobControl, RecordCoverage},
};
use vault_store::{OptionalRow, Priority, Session, params};

pub(crate) fn leaf_key(source: &str, leaf: u64) -> String {
    format!("leaf/{source}/{leaf}")
}
pub(crate) fn line_facts(s: &Session<'_>, publication: &str) -> vault_store::Result<()> {
    let lines=s.query("SELECT l.branch,l.source,l.version,l.records,json_extract(m.descriptor,'$.copy_of') FROM ah_reader_line l LEFT JOIN ah_reader_member m ON m.publication=l.publication AND m.source=l.source WHERE l.publication=?1 ORDER BY l.branch,l.first_rank",
        [publication],|r|Ok((r.get::<String>(0)?,r.get::<String>(1)?,r.get::<String>(2)?,r.get::<i64>(3)?,r.get::<Option<String>>(4)?)))?;
    let mut totals = std::collections::BTreeMap::<String, (Option<Tokens>, i64)>::new();
    let mut skipped = std::collections::HashMap::<String, i64>::new();
    for (branch, source, version, records, copy) in lines {
        let first_visible=s.query_one("SELECT first_visible FROM ah_reader_line WHERE publication=?1 AND branch=?2 AND source=?3",params![publication,branch.as_str(),source.as_str()],|r|Ok(r.get::<i64>(0)?))?;
        let mut mask = super::visible::RankCursor::default();
        let mut mirrors=s.prepare("SELECT ordinal FROM ah_chat_mirror WHERE publication=?1 AND source=?2 AND ordinal<?3 ORDER BY ordinal")?;
        let mut mirrors = mirrors.query(params![publication, source.as_str(), records])?;
        let count = skipped.entry(branch.clone()).or_default();
        while let Some(row) = mirrors.next()? {
            if let Some(rank) = mask.at(s, &version, row.get::<i64>(0)? as u64)? {
                let legacy = first_visible + rank as i64;
                let screen = legacy - *count;
                *count += 1;
                s.execute(
                    "INSERT INTO ah_chat_mirror_rank VALUES(?1,?2,?3,?4,?5)",
                    params![publication, branch.as_str(), legacy, screen, *count],
                )?;
            }
        }
        let (tokens, bad) = totals.entry(branch).or_default();
        let mut pos = 0;
        while pos < records as u64 {
            let (first, page) = pages::in_snapshot::<Entry>(s, &version, OUTLINE, pos)?;
            for e in page.into_iter().skip((pos - first) as usize) {
                if pos >= records as u64 {
                    break;
                }
                *bad += i64::from(!matches!(
                    e.locator.coverage,
                    RecordCoverage::Object | RecordCoverage::Blank
                ));
                pos += 1;
            }
        }
        let producing = copy.as_deref().unwrap_or(&source);
        let owner=s.query_one("SELECT coalesce((SELECT ?1 FROM ah_reader_member WHERE publication=?1 AND source=?2),(SELECT h.publication FROM ah_sessions m JOIN ah_reader_head h ON h.conversation=m.conversation_id WHERE m.id=?2))",params![publication,producing],|r|Ok(r.get::<Option<String>>(0)?))?;
        let selected = leaves::selection(s, &version)?;
        let mut ranks = super::visible::RankCursor::default();
        let mut stmt=s.prepare("SELECT ordinal,json_extract(facts,'$.tokens') FROM ah_chat_response WHERE publication=?1 AND source=?2 AND ordinal<?3 ORDER BY ordinal")?;
        let mut rows = stmt.query(params![owner, producing, records])?;
        while let Some(row) = rows.next()? {
            if let Some(sel) = &selected
                && sel.boundary.is_none()
            {
                if !s.query_one("SELECT EXISTS(SELECT 1 FROM ah_reader_walk_run WHERE owner=?1 AND leaf=?2 AND first_ordinal<=?3 AND first_ordinal+count>?3)",
                    params![version.as_str(),sel.latest.map(|n|n as i64),row.get::<i64>(0)?],|r|Ok(r.get::<i64>(0)?!=0))?{continue;}
            } else if selected.is_some() {
                let Some(n) = ranks.at(s, &version, row.get::<i64>(0)? as u64)? else { continue };
                if !s.query_one("SELECT EXISTS(SELECT 1 FROM ah_reader_walk_run WHERE owner=?1 AND leaf=-1 AND first_ordinal<=?2 AND first_ordinal+count>?2)",params![version.as_str(),n as i64],|r|Ok(r.get::<i64>(0)?!=0))?{continue}
            }
            tokens
                .get_or_insert_with(Default::default)
                .add(serde_json::from_str(&row.get::<String>(1)?).map_err(operation)?);
        }
    }
    for (branch, (tokens, bad)) in totals {
        s.execute(
            "INSERT INTO ah_chat_line_stats VALUES(?1,?2,?3,?4)",
            params![
                publication,
                branch,
                tokens.map(|t| serde_json::to_string(&t)).transpose().map_err(operation)?,
                bad
            ],
        )?;
    }
    Ok(())
}

impl Archive {
    /// Precompute the ordinary selected branch while publishing. An earlier
    /// leaf acquires its own footers and prompt directory on first selection.
    pub(crate) fn ensure_leaf_facts(
        &self,
        pub_id: &str,
        source: &str,
        version: &str,
        leaf: u64,
        job: &JobControl,
    ) -> Result<String> {
        let key = leaf_key(source, leaf);
        let (p, k) = (pub_id.to_owned(), key.clone());
        if self.store.read(super::DB,move|s|s.query_one("SELECT EXISTS(SELECT 1 FROM ah_chat_line_stats WHERE publication=?1 AND branch=?2)",params![p,k],|r|Ok(r.get::<i64>(0)?!=0)))?{return Ok(key)}
        self.leaf_page(version, leaf, 0, 1, job)?;
        let (p, src, v, k) =
            (pub_id.to_owned(), source.to_owned(), version.to_owned(), key.clone());
        let cancel = job.cancel.clone();
        self.store.write(super::DB,Priority::Batch,move|s|{
            if s.query_one("SELECT EXISTS(SELECT 1 FROM ah_chat_line_stats WHERE publication=?1 AND branch=?2)",params![p.as_str(),k.as_str()],|r|Ok(r.get::<i64>(0)?!=0))?{return Ok(())}
            let (projection,child)=s.query_one("SELECT projection_id,parent_session_id IS NOT NULL OR parent_path IS NOT NULL FROM ah_sessions WHERE id=?1",[src.as_str()],|r|Ok((r.get::<i64>(0)?,r.get::<i64>(1)?!=0)))?;
            let mut parser=Interpreter::new("pi".into(),child,Default::default());
            let mut number=0;let mut visible=0;let mut added=0;let mut ranks=super::visible::RankCursor::default();let mut total:Option<Tokens>=None;
            let mut ranges=s.prepare("SELECT first_ordinal,count FROM ah_reader_walk_run WHERE owner=?1 AND leaf=?2 ORDER BY reverse_rank DESC")?;
            let mut ranges=ranges.query(params![v.as_str(),leaf as i64])?;
            let save=|t:Turn,total:&mut Option<Tokens>|->vault_store::Result<()>{
                if let Some(n)=t.facts.tokens{total.get_or_insert_with(Default::default).add(n);}
                s.execute("INSERT INTO ah_chat_turn VALUES(?1,?2,?3,?4,?5,?6,?7,?8)",params![p.as_str(),k.as_str(),t.ordinal,t.last_ordinal,t.started_at,t.ended_at,t.footer_ordinal,t.wire().to_string()])?;
                Ok(())
            };
            while let Some(run)=ranges.next()?{
                let lo:i64=run.get(0)?;let end=lo+run.get::<i64>(1)?;
                let mut events=s.prepare("SELECT ordinal,kind,role,timestamp,substr(text,1,1024),payload FROM ah_events WHERE session_id=?1 AND projection_id=?2 AND ordinal>=?3 AND ordinal<?4 ORDER BY ordinal")?;
                let mut rows=events.query(params![src.as_str(),projection,lo,end])?;
                while let Some(row)=rows.next()?{
                    cancel.check()?;
                    let n:i64=row.get(0)?;
                    let (finished,_)=parser.record(n,&row.get::<String>(1)?,row.get::<Option<String>>(2)?.as_deref(),row.get(3)?,&row.get::<String>(4)?,&serde_json::from_str(&row.get::<String>(5)?).map_err(operation)?);
                    if let Some(t)=finished{save(t,&mut total)?}
                    let ordinary=ranks.at(s,&v,n as u64)?.is_some();
                    let extra=!ordinary && s.query_one("SELECT EXISTS(SELECT 1 FROM ah_chat_extra WHERE publication=?1 AND source=?2 AND ordinal=?3)",params![p.as_str(),src.as_str(),n],|r|Ok(r.get::<i64>(0)?!=0))?;
                    if extra {super::supplement::save_extra(s,&p,&k,&src,n,visible-added,&mut added)?;}
                    if ordinary||extra{
                        if let Some(prompt)=parser.turn.as_ref().filter(|t|t.ordinal==n).and_then(|t|t.prompt.as_deref()){
                            s.execute("INSERT INTO ah_chat_prompt VALUES(?1,?2,?3,?4,?5,?6,?7)",params![p.as_str(),k.as_str(),number,src.as_str(),n,visible,prompt])?;
                            number+=1;
                        }
                        visible+=1;
                    }
                }
            }
            if let Some(t)=parser.turn.take(){save(t.finish(),&mut total)?}
            s.execute("INSERT INTO ah_chat_line_stats VALUES(?1,?2,?3,0)",params![p,k,total.map(|t|serde_json::to_string(&t)).transpose().map_err(operation)?])?;
            Ok(())
        })?;
        Ok(key)
    }

    pub(crate) fn selected_leaf_facts(
        &self,
        pub_id: &str,
        source: &str,
        leaf: Option<u64>,
        job: &JobControl,
    ) -> Result<Option<String>> {
        let (p, src) = (pub_id.to_owned(), source.to_owned());
        let version=self.store.read(super::DB,move|s|s.query_one("SELECT m.version,v.leaf_count,l.latest,l.unresolved_at FROM ah_reader_member m JOIN ah_reader_version v ON v.token=m.version LEFT JOIN ah_reader_leaf l ON l.owner=v.token WHERE m.publication=?1 AND m.source=?2",params![p,src],|r|Ok((r.get::<String>(0)?,r.get::<i64>(1)?,r.get::<Option<i64>>(2)?,r.get::<Option<i64>>(3)?))).optional())?;
        let Some((v, count, latest, boundary)) = version else { return Ok(None) };
        let selection =
            leaf.or_else(|| latest.filter(|_| count > 1 && boundary.is_none()).map(|n| n as u64));
        selection.map(|n| self.ensure_leaf_facts(pub_id, source, &v, n, job)).transpose()
    }
}
