//! Codex writes a model item and a UI completion for the same message. Keep
//! both native addresses and present one row in the screen's display ranks.
use crate::{
    Archive, Result,
    read_model::{DisplayRow, JobControl, ReaderPublication},
};
use vault_store::{Db, OptionalRow, Session, params};

pub(super) fn publish(
    s: &Session<'_>,
    publication: &str,
    source: &str,
    projection: i64,
) -> vault_store::Result<()> {
    s.execute("INSERT INTO ah_chat_mirror(publication,source,ordinal,original)
        SELECT ?1,?2,ui.ordinal,item.ordinal FROM ah_events ui JOIN ah_events item
        ON item.session_id=ui.session_id AND item.projection_id=ui.projection_id AND item.ordinal=ui.ordinal-1
        WHERE ui.session_id=?2 AND ui.projection_id=?3 AND item.kind='response_item.message'
        AND ((item.role='user' AND ui.kind IN ('event_msg.item_completed.UserMessage','event_msg.user_message'))
        OR (item.role='assistant' AND ui.kind IN ('event_msg.item_completed.AgentMessage','event_msg.agent_message')))
        AND ui.text=item.text AND item.text<>'' AND (item.native_id IS NULL OR ui.native_id IS NULL OR item.native_id=ui.native_id)",
        params![publication,source,projection])?;
    Ok(())
}

/// Number of extra native-visible ranks up to a screen rank.
fn skipped(s: &Session<'_>, p: &str, b: &str, rank: i64) -> vault_store::Result<i64> {
    Ok(s.query_one("SELECT skipped FROM ah_chat_mirror_rank WHERE publication=?1 AND branch=?2 AND screen_rank<=?3 ORDER BY screen_rank DESC,skipped DESC LIMIT 1",
        params![p,b,rank],|r|Ok(r.get(0)?)).optional()?.unwrap_or(0))
}
impl Archive {
    pub(super) fn screen_mirrors(&self, h: &ReaderPublication, branch: &str) -> Result<u64> {
        let (p, b) = (h.publication.0.clone(), branch.to_owned());
        Ok(self
            .store
            .read(Db::AgentHistoryReadModel, move |s| Ok(skipped(s, &p, &b, i64::MAX)? as u64))?)
    }
    pub(super) fn without_mirrors(
        &self,
        h: &ReaderPublication,
        branch: &str,
        rank: u64,
    ) -> Result<u64> {
        let (p, b) = (h.publication.0.clone(), branch.to_owned());
        Ok(self.store.read(Db::AgentHistoryReadModel,move|s|{
            let skipped=s.query_one("SELECT skipped FROM ah_chat_mirror_rank WHERE publication=?1 AND branch=?2 AND native_rank<=?3 ORDER BY native_rank DESC LIMIT 1",params![p,b,rank as i64],|r|Ok(r.get::<i64>(0)?)).optional()?.unwrap_or(0);
            Ok(rank.saturating_sub(skipped as u64))
        })?)
    }
    pub(super) fn display_page(
        &self,
        h: &ReaderPublication,
        branch: &str,
        first: u64,
        count: usize,
        mirrors: u64,
        job: &JobControl,
    ) -> Result<Vec<DisplayRow>> {
        if mirrors == 0 {
            return self.reader_visible_window(h, Some(branch), first, count, job);
        }
        let (p, b) = (h.publication.0.clone(), branch.to_owned());
        let ranks=self.store.read(Db::AgentHistoryReadModel,move|s|{
            let initial=skipped(s,&p,&b,first as i64)?;
            let changes=s.query("SELECT screen_rank,max(skipped) FROM ah_chat_mirror_rank WHERE publication=?1 AND branch=?2 AND screen_rank>?3 AND screen_rank<?4 GROUP BY screen_rank ORDER BY screen_rank",
                params![p,b,first as i64,(first+count as u64) as i64],|r|Ok((r.get::<i64>(0)? as u64,r.get::<i64>(1)? as u64)))?;
            let mut changes=changes.into_iter().peekable();let mut skip=initial as u64;
            Ok((first..first+count as u64).map(|i|{
                if changes.peek().is_some_and(|(rank,_)|*rank==i){skip=changes.next().unwrap().1;}
                i+skip
            }).collect::<Vec<_>>())
        })?;
        let mut rest = ranks.as_slice();
        let mut out = Vec::with_capacity(count);
        while let Some(&lo) = rest.first() {
            let n = rest.partition_point(|rank| *rank - lo < 200);
            let (wanted, next) = rest.split_at(n);
            let rows = self.reader_visible_window(
                h,
                Some(branch),
                lo,
                (wanted[n - 1] - lo + 1) as usize,
                job,
            )?;
            out.extend(
                rows.into_iter()
                    .enumerate()
                    .filter(|(i, _)| wanted.binary_search(&(lo + *i as u64)).is_ok())
                    .map(|(_, r)| r),
            );
            rest = next;
        }
        Ok(out)
    }
}
