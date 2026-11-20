//! Selective project/tool scopes seek their member publications. Broad date ranges seek
//! interval ends, retaining intervals that started before the range. Both release each batch.
use super::{Query, catalog::Catalog};
use crate::{Archive, Error, Result};
use vault_store::{CancelToken, Db, Row, Value as Sql, params};
const DB: Db = Db::AgentHistoryReadModel;
pub(super) struct Page {
    pub id: i64,
    pub publication: String,
    pub lo: Option<i64>,
    pub hi: Option<i64>,
    pub undated: bool,
    pub brief: Vec<u8>,
}
const FIELDS: &str = "rowid,publication,lo,hi,undated,brief";
fn row(r: &Row) -> vault_store::Result<Page> {
    Ok(Page {
        id: r.get(0)?,
        publication: r.get(1)?,
        lo: r.get(2)?,
        hi: r.get(3)?,
        undated: r.get::<i64>(4)? != 0,
        brief: r.get(5)?,
    })
}
impl Archive {
    pub(super) fn insight_batches(
        &self,
        cat: &Catalog,
        q: &Query,
        bounds: (i64, i64),
        all: bool,
        cancel: &CancelToken,
        mut visit: impl FnMut(Vec<Page>) -> Result<()>,
    ) -> Result<()> {
        let (lo, hi) = bounds;
        if !q.harnesses.is_empty() || q.project_id.is_some() {
            let pubs: Vec<_> = cat
                .publications
                .iter()
                .filter(|(_, i)| cat.eligible(**i, q))
                .map(|(p, _)| p)
                .collect();
            if pubs.len() <= cat.publications.len() / 4 + 1 {
                for group in pubs.chunks(64) {
                    let mut after = (String::new(), String::new(), -1_i64);
                    loop {
                        cancel.check().map_err(|_| Error::Cancelled)?;
                        let tail = after.clone();
                        let mut args: Vec<_> =
                            group.iter().map(|p| Sql::Text((***p).to_owned())).collect();
                        let slots = vec!["?"; args.len()].join(",");
                        args.extend([
                            Sql::Text(tail.0),
                            Sql::Text(tail.1),
                            Sql::Integer(tail.2),
                            Sql::Integer(lo),
                            Sql::Integer(hi),
                            Sql::Integer(i64::from(all)),
                        ]);
                        let batch=self.store.read(DB,move|s|s.query(&format!("SELECT {FIELDS},source,page FROM ah_insight_page WHERE publication IN ({slots})
                            AND (publication,source,page)>(?,?,?) AND ((hi>=? AND lo<?) OR (? AND undated=1)) ORDER BY publication,source,page LIMIT 512"),args,|r|Ok((row(r)?,r.get::<String>(6)?,r.get::<i64>(7)?))))?;
                        let Some((p, src, n)) = batch.last() else { break };
                        after = (p.publication.clone(), src.clone(), *n);
                        let done = batch.len() < 512;
                        visit(batch.into_iter().map(|(p, _, _)| p).collect())?;
                        if done {
                            break;
                        }
                    }
                }
                return Ok(());
            }
        }
        let mut cursor = (lo, 0);
        loop {
            cancel.check().map_err(|_| Error::Cancelled)?;
            let batch = self.store.read(DB, move |s| {
                s.query(
                    &format!(
                        "SELECT {FIELDS} FROM ah_insight_page INDEXED BY ah_insight_time
                WHERE hi>=?1 AND (hi,rowid)>(?1,?2) AND lo<?3 ORDER BY hi,rowid LIMIT 512"
                    ),
                    params![cursor.0, cursor.1, hi],
                    row,
                )
            })?;
            let Some(p) = batch.last() else { break };
            cursor = (p.hi.unwrap(), p.id);
            visit(batch)?;
        }
        if all {
            let mut after = 0;
            loop {
                cancel.check().map_err(|_| Error::Cancelled)?;
                let batch=self.store.read(DB,move|s|s.query(&format!("SELECT {FIELDS} FROM ah_insight_page WHERE hi IS NULL AND rowid>?1 ORDER BY rowid LIMIT 512"),[after],row))?;
                let Some(p) = batch.last() else { break };
                after = p.id;
                visit(batch)?;
            }
        }
        Ok(())
    }
}
