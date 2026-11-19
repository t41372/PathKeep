//! Pi leaf selection follows native parent edges. The latest recorded leaf is
//! distinct from a live UI choice, which the evidence cannot establish.
use super::*;
use crate::Archive;
use vault_store::{OptionalRow, Priority, params};

/// A page of recorded Pi alternatives, with the most recent terminal entry as a reader default.
#[derive(Clone, Debug, Serialize)]
pub struct ReaderLeaves {
    pub ordinals: Vec<u64>,
    pub total: u64,
    pub latest_recorded: Option<u64>,
}

impl Archive {
    /// Enumerates leaf addresses without walking every parent chain or scanning raw events.
    pub fn reader_leaves(
        &self,
        h: &ReaderPublication,
        source: &str,
        first: u64,
        count: usize,
    ) -> Result<ReaderLeaves> {
        read::window_limit(first, count)?;
        let job = JobControl::default();
        let _worker = self.jobs.enter_window(&job.cancel)?;
        let (owner, _) = self.member_version(h, source)?;
        self.version_info(&owner)?;
        let key = owner.clone();
        let (total, latest_recorded) = self
            .store
            .read(DB, move |tx| {
                tx.query_one("SELECT total,latest FROM ah_reader_leaf WHERE owner=?1", [key], |r| {
                    Ok((r.get::<i64>(0)? as u64, r.get::<Option<i64>>(1)?.map(|n| n as u64)))
                })
                .optional()
            })?
            .unwrap_or_default();
        let n = count.min(total.saturating_sub(first) as usize);
        let ordinals = read::range(self, &owner, LEAVES, first, n, &job)?;
        self.check_head(h)?;
        Ok(ReaderLeaves { ordinals, total, latest_recorded })
    }

    pub(super) fn stage_selected_leaf(
        &self,
        owner: &str,
        total: u64,
        latest: Option<u64>,
        job: &JobControl,
    ) -> Result<()> {
        let mut reverse_rank = 0i64;
        let mut resolved = true;
        if let Some(leaf) = latest {
            let walked = self.walk_leaf(owner, leaf, job, |lo, hi| {
                let token = owner.to_owned();
                let n = self.store.write(DB, Priority::Batch, move |tx| {
                    build::require_staging(tx, &token)?;
                    let start = visible::prefix(tx, &token, lo as i64)?;
                    let count = visible::prefix(tx, &token, hi as i64)? - start;
                    if count > 0 {
                        tx.execute(
                            "INSERT INTO ah_reader_leaf_line VALUES(?1,?2,?3,?4)",
                            params![token, reverse_rank, start, count],
                        )?;
                    }
                    Ok(count)
                })?;
                reverse_rank += n;
                Ok(true)
            });
            match walked {
                Ok(()) => {}
                Err(Error::InvalidInput(_)) => resolved = false,
                Err(error) => return Err(error),
            }
        }
        let token = owner.to_owned();
        self.store.write(DB, Priority::Batch, move |tx| {
            build::require_staging(tx, &token)?;
            tx.execute(
                "INSERT INTO ah_reader_leaf VALUES(?1,?2,?3,?4)",
                params![
                    token,
                    total as i64,
                    latest.map(|n| n as i64),
                    resolved.then_some(reverse_rank)
                ],
            )?;
            Ok(())
        })?;
        Ok(())
    }

    pub(super) fn selected_visible_ranges(
        &self,
        owner: &str,
        lo: u64,
        hi: u64,
    ) -> Result<Vec<(u64, u64)>> {
        let token = owner.to_owned();
        Ok(self.store.read(DB,move|tx|{
            let total=tx.query_one("SELECT selected_visible FROM ah_reader_leaf WHERE owner=?1",[token.as_str()],|r|Ok(r.get::<Option<i64>>(0)?)).optional()?.flatten();
            let Some(total)=total else{return Ok(vec![(lo,hi)])};
            let total=total as u64;
            let lower=total.checked_sub(hi).ok_or_else(||crate::query::operation(pages::invalid("branch rank")))?;
            let first=tx.query_one("SELECT reverse_rank FROM ah_reader_leaf_line WHERE owner=?1 AND reverse_rank<=?2 ORDER BY reverse_rank DESC LIMIT 1",params![token.clone(),lower as i64],|r|Ok(r.get::<i64>(0)?))?;
            tx.query("SELECT reverse_rank,visible_start,count FROM ah_reader_leaf_line WHERE owner=?1 AND reverse_rank>=?2 AND reverse_rank<?3 ORDER BY reverse_rank DESC",
                params![token,first,(total-lo) as i64],|r|{
                    let reverse=r.get::<i64>(0)? as u64;let base=r.get::<i64>(1)? as u64;let count=r.get::<i64>(2)? as u64;
                    let start=total-reverse-count;Ok((base+lo.saturating_sub(start),base+(hi-start).min(count)))
                })
        })?)
    }
}
