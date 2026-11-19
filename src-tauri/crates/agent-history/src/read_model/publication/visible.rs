//! Sparse rank/select bitmaps for native records with reader parts and structural boundaries.
//! Turn ownership and token accounting do not participate in these ranks.
use super::*;
use crate::Archive;
use vault_store::{OptionalRow, Priority, Session, params};
#[derive(Default)]
pub(super) struct Builder {
    block: u64,
    before: u64,
    mask: [u8; 32],
}
impl Builder {
    pub fn add(&mut self, a: &Archive, owner: &str, entry: &Entry) -> Result<()> {
        let ordinal = entry.locator.ordinal.0;
        if ordinal / 256 != self.block {
            self.flush(a, owner)?;
            self.block = ordinal / 256;
        }
        if entry.visible {
            let bit = ordinal as usize % 256;
            self.mask[bit / 8] |= 1 << (bit % 8);
        }
        Ok(())
    }
    pub fn flush(&mut self, a: &Archive, owner: &str) -> Result<()> {
        let n: u64 = self.mask.iter().map(|b| u64::from(b.count_ones())).sum();
        if n == 0 {
            return Ok(());
        }
        let mask = std::mem::take(&mut self.mask).to_vec();
        let first = self.block as i64 * 256;
        let before = self.before as i64;
        let token = owner.to_string();
        a.store.write(DB, Priority::Batch, move |tx| {
            build::require_staging(tx, &token)?;
            tx.execute(
                "INSERT INTO ah_reader_rank VALUES(?1,?2,?3,?4)",
                params![token, first, before, mask],
            )?;
            Ok(())
        })?;
        self.before += n;
        Ok(())
    }
}

pub(super) fn prefix(tx: &Session<'_>, owner: &str, end: i64) -> vault_store::Result<i64> {
    let block = tx
        .query_one(
            "SELECT first_rank,visible_before,mask FROM ah_reader_rank WHERE owner=?1 AND
                 first_rank<?2 ORDER BY first_rank DESC LIMIT 1",
            params![owner, end],
            |r| Ok((r.get::<i64>(0)?, r.get::<i64>(1)?, r.get::<Vec<u8>>(2)?)),
        )
        .optional()?;
    let Some((first, before, mask)) = block else { return Ok(0) };
    if mask.len() != 32 {
        return Err(crate::query::operation(pages::invalid("rank bitmap")));
    }
    Ok(before
        + (0..(end - first).min(256) as usize).filter(|b| mask[b / 8] & (1 << (b % 8)) != 0).count()
            as i64)
}

impl Archive {
    /// Seeks in displayable-record order. Structural compaction and context-edit markers
    /// remain reachable even when their native representation has no readable text.
    pub fn reader_visible_window(
        &self,
        h: &ReaderPublication,
        branch: Option<&str>,
        first: u64,
        count: usize,
        job: &JobControl,
    ) -> Result<Vec<DisplayRow>> {
        let _guard = self.jobs.enter_reader(&job.cancel)?;
        read::window_limit(first, count)?;
        self.check_head(h)?;
        let mut result = Vec::new();
        for (owner, start, n) in self.line_runs(h, branch, first, count, true)? {
            let lo = first.saturating_sub(start);
            let hi = (first + count as u64 - start).min(n);
            for (lo, hi) in self.selected_visible_ranges(&owner, lo, hi)? {
                self.append_visible(&owner, lo, hi, job, &mut result)?;
            }
        }
        self.check_head(h)?;
        Ok(result)
    }
    fn append_visible(
        &self,
        owner: &str,
        lo: u64,
        hi: u64,
        job: &JobControl,
        result: &mut Vec<DisplayRow>,
    ) -> Result<()> {
        let token = owner.to_owned();
        let blocks = self.store.read(DB, move |tx| {
            let base = tx.query_one(
                "SELECT visible_before FROM ah_reader_rank WHERE owner=?1 AND visible_before<=?2
                 ORDER BY visible_before DESC LIMIT 1",
                params![token.clone(), lo as i64],
                |r| Ok(r.get::<i64>(0)?),
            )?;
            tx.query(
                "SELECT first_rank,visible_before,mask FROM ah_reader_rank WHERE owner=?1 AND
                 visible_before>=?2 AND visible_before<?3 ORDER BY visible_before",
                params![token, base, hi as i64],
                |r| Ok((r.get::<i64>(0)? as u64, r.get::<i64>(1)? as u64, r.get::<Vec<u8>>(2)?)),
            )
        })?;
        let mut runs: Vec<(u64, usize)> = Vec::new();
        for (base, mut visible, mask) in blocks {
            if mask.len() != 32 {
                return Err(pages::invalid("visible rank bitmap"));
            }
            for bit in 0..256 {
                if mask[bit / 8] & (1 << (bit % 8)) != 0 {
                    if (lo..hi).contains(&visible) {
                        let ordinal = base + bit as u64;
                        if let Some((first, n)) = runs.last_mut()
                            && *first + *n as u64 == ordinal
                        {
                            *n += 1
                        } else {
                            runs.push((ordinal, 1))
                        }
                    }
                    visible += 1;
                }
            }
        }
        for (ordinal, n) in runs {
            result.extend(self.display_source(owner, ordinal, n, job)?);
        }

        Ok(())
    }
}
