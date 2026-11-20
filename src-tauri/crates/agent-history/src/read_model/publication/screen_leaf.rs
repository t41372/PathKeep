//! Visible coordinates for the alternate Pi leaf selected in the reader.
//! The existing version-owned walk supplies the branch; the window only decodes its selected entries.
use super::*;
use crate::{Archive, Result};
use vault_store::{OptionalRow, Priority, params};
impl Archive {
    pub(super) fn leaf_visible_size(
        &self,
        owner: &str,
        leaf: u64,
        job: &JobControl,
    ) -> Result<u64> {
        let owner = owner.to_owned();
        let leaf = leaf as i64;
        let key = owner.clone();
        let existing = self.store.read(DB, move |s| {
            s.query_one(
                "SELECT records FROM ah_chat_leaf_size WHERE owner=?1 AND leaf=?2",
                params![key, leaf],
                |r| Ok(r.get::<i64>(0)? as u64),
            )
            .optional()
        })?;
        if let Some(n) = existing {
            return Ok(n);
        }
        let cancel = job.cancel.clone();
        Ok(self.store.write(DB,Priority::Interactive,move|s|{
            let mut stmt=s.prepare("SELECT first_ordinal,count FROM ah_reader_walk_run WHERE owner=?1 AND leaf=?2 ORDER BY reverse_rank DESC")?;
            let mut rows=stmt.query(params![owner.as_str(),leaf])?;
            let mut next=0;
            while let Some(row)=rows.next()?{
                cancel.check()?;
                let start:i64=row.get(0)?;let len:i64=row.get(1)?;
                let first=visible::prefix(s,&owner,start)?;
                let count=visible::prefix(s,&owner,start+len)?-first;
                if count!=0{s.execute("INSERT OR IGNORE INTO ah_chat_leaf_run VALUES(?1,?2,?3,?4,?5)",params![owner.as_str(),leaf,next,first,count])?;next+=count;}
            }
            s.execute("INSERT OR REPLACE INTO ah_chat_leaf_size VALUES(?1,?2,?3)",params![owner,leaf,next])?;
            Ok(next as u64)
        })?)
    }

    pub(crate) fn screen_leaf_rows(
        &self,
        h: &ReaderPublication,
        source: &str,
        leaf: u64,
        first: u64,
        n: usize,
        job: &JobControl,
    ) -> Result<Vec<DisplayRow>> {
        let _guard = self.jobs.enter_window(&job.cancel)?;
        self.at_head(h,||{
            let (owner,_)=self.member_version(h,source)?;
            let key=owner.clone();
            let runs=self.store.read(DB,move|s|s.query(
                "SELECT rank,start,records FROM ah_chat_leaf_run WHERE owner=?1 AND leaf=?2 AND rank>=
                 (SELECT max(rank) FROM ah_chat_leaf_run WHERE owner=?1 AND leaf=?2 AND rank<=?3) AND rank<?4 ORDER BY rank",
                params![key,leaf as i64,first as i64,(first+n as u64) as i64],
                |r|Ok((r.get::<i64>(0)? as u64,r.get::<i64>(1)? as u64,r.get::<i64>(2)? as u64))))?;
            let mut window=Window::default();
            for (rank,start,len) in runs{
                let lo=first.saturating_sub(rank);
                let hi=(first+n as u64-rank).min(len);
                self.append_visible(&owner,start+lo,start+hi,job,&mut window)?;
            }
            window.read(self,job)
        })
    }
    pub(crate) fn screen_leaf_outline(
        &self,
        h: &ReaderPublication,
        source: &str,
        leaf: u64,
        first: u64,
        n: usize,
        job: &JobControl,
    ) -> Result<Vec<(SourceId, OutlineEntry)>> {
        let _guard = self.jobs.enter_window(&job.cancel)?;
        self.at_head(h, || {
            let (owner, _) = self.member_version(h, source)?;
            let (info, _) = self.version_info(&owner)?;
            let mut out = vec![];
            for (start, len) in self.leaf_page(&owner, leaf, first, n, job)? {
                for e in self.source_outline(&owner, &info, start, len as usize, job)? {
                    out.push((SourceId(source.into()), e));
                }
            }
            Ok(out)
        })
    }
}
