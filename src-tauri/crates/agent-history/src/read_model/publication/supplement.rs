//! Image-only messages and native notices may have no text part. Insert these
//! sparse physical addresses into the screen ranks without changing the CLI outline.
use super::*;
use crate::{Archive, Result};
use std::collections::{BTreeMap, HashMap};
use vault_store::{OptionalRow, Session, params};

pub(crate) fn save_extra(
    s: &Session<'_>,
    publication: &str,
    branch: &str,
    source: &str,
    n: i64,
    base: i64,
    inserted: &mut i64,
) -> vault_store::Result<()> {
    s.execute(
        "INSERT INTO ah_chat_insert_rank VALUES(?1,?2,?3,?4,?5,?6,?7)",
        params![publication, branch, base + *inserted, base, *inserted + 1, source, n],
    )?;
    *inserted += 1;
    Ok(())
}
pub(crate) fn added_before_native(
    s: &Session<'_>,
    p: &str,
    b: &str,
    n: i64,
) -> vault_store::Result<i64> {
    Ok(s.query_one("SELECT added FROM ah_chat_insert_rank WHERE publication=?1 AND branch=?2 AND base_rank<=?3 ORDER BY base_rank DESC,added DESC LIMIT 1",params![p,b,n],|r|Ok(r.get(0)?)).optional()?.unwrap_or(0))
}

/// Each selected physical Pi run supplies its own prefix of visible records.
pub(super) fn build(s: &Session<'_>, p: &str) -> vault_store::Result<()> {
    let segments=s.query("SELECT branch,source,version,records,first_visible FROM ah_reader_line l WHERE publication=?1 AND EXISTS(
        SELECT 1 FROM ah_chat_extra e WHERE e.publication=?1 AND e.source=l.source) ORDER BY branch,first_rank",[p],
        |r|Ok((r.get::<String>(0)?,r.get::<String>(1)?,r.get::<String>(2)?,r.get::<i64>(3)?,r.get::<i64>(4)?)))?;
    let mut added = BTreeMap::new();
    for (branch, source, v, records, mut base) in segments {
        let ranges=match leaves::selection(s,&v)?{
            None=>vec![(0,records)],
            Some(sel) if sel.boundary.is_none() && sel.latest.is_some()=>s.query("SELECT first_ordinal,count FROM ah_reader_walk_run WHERE owner=?1 AND leaf=?2 ORDER BY reverse_rank DESC",params![v.as_str(),sel.latest.map(|n|n as i64)],|r|Ok((r.get(0)?,r.get(1)?)))?,
            Some(_)=>continue,
        };
        let mut mask = visible::RankCursor::default();
        for (lo, count) in ranges {
            let end = (lo + count).min(records);
            if end <= lo {
                continue;
            }
            let previous = visible::prefix(s, &v, lo)?;
            let mut stmt=s.prepare("SELECT ordinal FROM ah_chat_extra WHERE publication=?1 AND source=?2 AND ordinal>=?3 AND ordinal<?4 ORDER BY ordinal")?;
            let mut rows = stmt.query(params![p, source.as_str(), lo, end])?;
            while let Some(r) = rows.next()? {
                let n = r.get::<i64>(0)?;
                if mask.at(s, &v, n as u64)?.is_some() {
                    continue;
                }
                let native = base + visible::prefix(s, &v, n)? - previous;
                let skipped=s.query_one("SELECT skipped FROM ah_chat_mirror_rank WHERE publication=?1 AND branch=?2 AND native_rank<?3 ORDER BY native_rank DESC LIMIT 1",params![p,branch.as_str(),native],|r|Ok(r.get::<i64>(0)?)).optional()?.unwrap_or(0);
                save_extra(
                    s,
                    p,
                    &branch,
                    &source,
                    n,
                    native - skipped,
                    added.entry(branch.clone()).or_default(),
                )?;
            }
            base += visible::prefix(s, &v, end)? - previous;
        }
    }
    Ok(())
}
impl Archive {
    pub(super) fn prepare_extra_walks(&self, p: &str, job: &JobControl) -> Result<()> {
        let p = p.to_owned();
        let tips=self.store.read(DB,move|s|s.query("SELECT DISTINCT m.version,l.latest FROM ah_reader_member m JOIN ah_reader_leaf l ON l.owner=m.version WHERE m.publication=?1 AND l.latest IS NOT NULL AND l.unresolved_at IS NULL AND (EXISTS(SELECT 1 FROM ah_chat_extra e WHERE e.publication=?1 AND e.source=m.source) OR EXISTS(SELECT 1 FROM ah_chat_response u WHERE u.publication=?1 AND u.source=m.source))",[p],|r|Ok((r.get::<String>(0)?,r.get::<i64>(1)? as u64))))?;
        for (owner, tip) in tips {
            self.leaf_page(&owner, tip, 0, 1, job)?;
        }
        Ok(())
    }
    pub(crate) fn extra_count(&self, h: &ReaderPublication, key: &str) -> Result<u64> {
        let (p, key) = (h.publication.0.clone(), key.to_owned());
        Ok(self.store.read(DB, move |s| Ok(added_before_native(s, &p, &key, i64::MAX)? as u64))?)
    }
    pub(crate) fn added_rank(
        &self,
        h: &ReaderPublication,
        key: &str,
        source: &str,
        n: u64,
    ) -> Result<Option<u64>> {
        let (p, key, src) = (h.publication.0.clone(), key.to_owned(), source.to_owned());
        Ok(self.store.read(DB,move|s|s.query_one("SELECT rank FROM ah_chat_insert_rank WHERE publication=?1 AND branch=?2 AND source=?3 AND ordinal=?4",params![p,key,src,n as i64],|r|Ok(r.get::<i64>(0)? as u64)).optional())?)
    }
    pub(crate) fn with_extras(&self, h: &ReaderPublication, key: &str, rank: u64) -> Result<u64> {
        let (p, key) = (h.publication.0.clone(), key.to_owned());
        Ok(rank
            + self.store.read(DB, move |s| added_before_native(s, &p, &key, rank as i64))? as u64)
    }
    pub(crate) fn extra_page(
        &self,
        h: &ReaderPublication,
        key: &str,
        first: u64,
        n: usize,
        job: &JobControl,
    ) -> Result<(u64, BTreeMap<u64, DisplayRow>)> {
        let (p, key) = (h.publication.0.clone(), key.to_owned());
        let(prior,wanted)=self.store.read(DB,move|s|{
            let prior=s.query_one("SELECT added FROM ah_chat_insert_rank WHERE publication=?1 AND branch=?2 AND rank<?3 ORDER BY rank DESC LIMIT 1",params![p.as_str(),key.as_str(),first as i64],|r|Ok(r.get::<i64>(0)? as u64)).optional()?.unwrap_or(0);
            let rows=s.query("SELECT rank,source,ordinal FROM ah_chat_insert_rank WHERE publication=?1 AND branch=?2 AND rank>=?3 AND rank<?4 ORDER BY rank",params![p,key,first as i64,(first+n as u64) as i64],|r|Ok((r.get::<i64>(0)? as u64,r.get::<String>(1)?,r.get::<i64>(2)? as u64)))?;
            Ok((prior,rows))
        })?;
        if wanted.is_empty() {
            return Ok((prior, BTreeMap::new()));
        }
        let _guard = self.jobs.enter_window(&job.cancel)?;
        self.at_head(h, || {
            let mut owners = HashMap::new();
            let mut window = Window::default();
            let mut ranks = vec![];
            for (rank, source, n) in wanted {
                if !owners.contains_key(&source) {
                    owners.insert(source.clone(), self.member_version(h, &source)?.0);
                }
                window.physical(&owners[&source], [n]);
                ranks.push(rank);
            }
            Ok((prior, ranks.into_iter().zip(window.read(self, job)?).collect()))
        })
    }
}
