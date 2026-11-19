//! Version-owned ranked Pi runs. The default selection uses visible coordinates;
//! a leaf opened explicitly uses physical coordinates. A run can be used only at its issued rank.
use super::*;
use crate::{Archive, query::operation};
use vault_store::{OptionalRow, Session, params};
pub(super) const SELECTED: i64 = -1;
const RUN_BINDING: i64 = 8;

fn stamp(owner: &str, leaf: i64, reverse: i64, start: i64, count: i64) -> Vec<u8> {
    pages::binding(owner, RUN_BINDING, leaf, reverse, count, &start.to_le_bytes())
}
pub(super) fn save(
    tx: &Session<'_>,
    owner: &str,
    leaf: i64,
    reverse: i64,
    start: i64,
    count: i64,
) -> vault_store::Result<()> {
    let stamp = stamp(owner, leaf, reverse, start, count);
    tx.execute(
        "INSERT INTO ah_reader_walk_run VALUES(?1,?2,?3,?4,?5,?6)
        ON CONFLICT(owner,leaf,reverse_rank) DO UPDATE SET first_ordinal=excluded.first_ordinal,
        count=excluded.count,stamp=excluded.stamp WHERE stamp<>excluded.stamp OR
        first_ordinal<>excluded.first_ordinal OR count<>excluded.count",
        params![owner, leaf, reverse, start, count, stamp],
    )?;
    Ok(())
}
pub(super) fn depth_stamp(owner: &str, leaf: i64, depth: i64) -> Vec<u8> {
    stamp(owner, leaf, -1, 0, depth)
}

pub(super) fn page(
    tx: &Session<'_>,
    owner: &str,
    leaf: i64,
    depth: i64,
    first: i64,
    end: i64,
) -> vault_store::Result<Vec<(u64, u64)>> {
    let invalid = || operation(Error::ReaderRebuildRequired);
    if !(0..=depth).contains(&first) || !(first..=depth).contains(&end) || end - first > 200 {
        return Err(invalid());
    }
    if first == end {
        return Ok(Vec::new());
    }
    let lower = tx
        .query_one(
            "SELECT reverse_rank FROM ah_reader_walk_run WHERE owner=?1 AND leaf=?2
        AND reverse_rank<=?3 ORDER BY reverse_rank DESC LIMIT 1",
            params![owner, leaf, depth - end],
            |r| Ok(r.get::<i64>(0)?),
        )
        .optional()?
        .ok_or_else(invalid)?;
    let mut cursor = first;
    let runs = tx.query(
        "SELECT reverse_rank, first_ordinal, count, stamp FROM ah_reader_walk_run
         WHERE owner=?1 AND leaf=?2 AND reverse_rank>=?3 AND reverse_rank<?4
         ORDER BY reverse_rank DESC LIMIT 201",
        params![owner, leaf, lower, depth - first],
        |row| {
            let reverse: i64 = row.get(0)?;
            let start: i64 = row.get(1)?;
            let count: i64 = row.get(2)?;
            let binding: Vec<u8> = row.get(3)?;
            if reverse < 0
                || start < 0
                || count <= 0
                || reverse > depth - count
                || start.checked_add(count).is_none()
                || binding != stamp(owner, leaf, reverse, start, count)
            {
                return Err(invalid());
            }
            let rank = depth - reverse - count;
            let lo = rank.max(first);
            let hi = (rank + count).min(end);
            if lo != cursor || hi <= lo {
                return Err(invalid());
            }
            cursor = hi;
            Ok(((start + (lo - rank)) as u64, (hi - lo) as u64))
        },
    )?;
    if cursor != end {
        return Err(invalid());
    }
    Ok(runs)
}
impl Archive {
    /// Validate previously materialized walks during explicit offline repair. Unpublished
    /// fragments have no depth record and cannot serve a request.
    pub(super) fn walks_intact(&self, owner: &str, job: &JobControl) -> Result<bool> {
        let mut after = -1;
        loop {
            let token = owner.to_string();
            let head = self.store.read(DB, move |tx| {
                tx.query_one(
                    "SELECT leaf,depth,stamp FROM ah_reader_walk
                     WHERE owner=?1 AND leaf>?2 ORDER BY leaf LIMIT 1",
                    params![token, after],
                    |row| Ok((row.get::<i64>(0)?, row.get::<i64>(1)?, row.get::<Vec<u8>>(2)?)),
                )
                .optional()
            })?;
            let Some((leaf, depth, stamp)) = head else { return Ok(true) };
            if depth <= 0 || stamp != depth_stamp(owner, leaf, depth) {
                return Ok(false);
            }
            for start in (0..depth).step_by(200) {
                job.cancel.check().map_err(|_| Error::Cancelled)?;
                let key = owner.to_string();
                let read = self
                    .store
                    .read(DB, move |tx| page(tx, &key, leaf, depth, start, depth.min(start + 200)));
                match read.map_err(Error::from) {
                    Ok(_) => {}
                    Err(Error::ReaderRebuildRequired) => return Ok(false),
                    Err(e) => return Err(e),
                }
            }
            after = leaf;
        }
    }
}
