//! Sparse rank/select bitmaps for native records with reader parts and structural boundaries.
//! Turn ownership and token accounting do not participate in these ranks.
use super::*;
use crate::Archive;
use vault_store::Session;
#[derive(Serialize, Deserialize)]
struct RankBlock {
    first: u64,
    before: u64,
    mask: [u8; 32],
}

pub(super) struct Builder {
    blocks: pages::Buffer,
    before: u64,
    mask: [u8; 32],
    used: usize,
}
impl Default for Builder {
    fn default() -> Self {
        Self {
            blocks: pages::Buffer::new(RANK, pages::META_BYTES),
            before: 0,
            mask: [0; 32],
            used: 0,
        }
    }
}
impl Builder {
    pub fn add(&mut self, a: &Archive, owner: &str, entry: &Entry) -> Result<()> {
        if self.used == 256 {
            self.flush(a, owner)?;
        }
        if entry.visible {
            self.mask[self.used / 8] |= 1 << (self.used % 8);
        }
        self.used += 1;
        Ok(())
    }
    pub fn flush(&mut self, a: &Archive, owner: &str) -> Result<()> {
        if self.used == 0 {
            return Ok(());
        }
        let first = self.blocks.len() * 256;
        let mask = std::mem::take(&mut self.mask);
        self.blocks.push(a, owner, &RankBlock { first, before: self.before, mask })?;
        self.blocks.flush(a, owner)?;
        self.before += mask.iter().map(|b| u64::from(b.count_ones())).sum::<u64>();
        self.used = 0;
        Ok(())
    }
    pub fn extent(&self) -> (i64, i64) {
        (self.blocks.len() as i64, self.before as i64)
    }
}
pub(super) fn prefix(tx: &Session<'_>, owner: &str, end: i64) -> vault_store::Result<i64> {
    if end == 0 {
        return Ok(0);
    }
    let rank = (end as u64 - 1) / 256;
    let (_, blocks) = pages::in_snapshot::<RankBlock>(tx, owner, RANK, rank)?;
    let block = blocks
        .first()
        .filter(|b| b.first == rank * 256)
        .ok_or_else(|| crate::query::operation(pages::invalid("rank block")))?;
    Ok((block.before
        + (0..end as u64 - block.first)
            .filter(|&n| block.mask[n as usize / 8] & (1 << (n % 8)) != 0)
            .count() as u64) as i64)
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
        let _guard = self.jobs.enter_window(&job.cancel)?;
        self.at_head(h, || {
            read::window_limit(first, count)?;
            let mut window = Window::default();
            for (owner, start, n) in self.line_runs(h, branch, first, count, true)? {
                let lo = first.saturating_sub(start);
                let hi = (first + count as u64 - start).min(n);
                let (marker, ranges) = self.selected_visible_ranges(&owner, lo, hi)?;
                if let Some(n) = marker {
                    let (info, _) = self.version_info(&owner)?;
                    window.marker(DisplayRow {
                        source_id: info.source_id,
                        ordinal: PhysicalOrdinal(n),
                        native_id: None,
                        parent_id: None,
                        kind: "unresolved_parent".into(),
                        role: None,
                        timestamp: None,
                        parts: vec![],
                        record_bytes: 0,
                        truncated: false,
                        tokens_before: None,
                        boundary: Some(StructuralBoundary {
                            kind: BoundaryKind::UnresolvedParent,
                            tokens_before: None,
                        }),
                    });
                }
                for (lo, hi) in ranges {
                    self.append_visible(&owner, lo, hi, job, &mut window)?;
                }
            }
            let result = window.read(self, job)?;
            Ok(result)
        })
    }
    fn append_visible(
        &self,
        owner: &str,
        lo: u64,
        hi: u64,
        job: &JobControl,
        window: &mut Window,
    ) -> Result<()> {
        let token = owner.to_owned();
        let (extent, total, records) = self.store.read(DB, move |tx| {
            tx.query_one(
                "SELECT rank_pages,visible_total,records FROM ah_reader_version WHERE token=?1",
                [token],
                |r| Ok((r.get::<i64>(0)? as u64, r.get::<i64>(1)? as u64, r.get::<i64>(2)? as u64)),
            )
        })?;
        if hi > total
            || lo >= hi
            || extent == 0
            || extent != records.div_ceil(256)
            || total > records
        {
            return Err(Error::ReaderRebuildRequired);
        }
        let load = |n: u64| -> Result<RankBlock> {
            let (_, mut rows) =
                pages::load::<RankBlock>(self, owner, RANK, n, job).map_err(navigation_error)?;
            rows.pop().filter(|b| b.first == 256 * n).ok_or(Error::ReaderRebuildRequired)
        };
        let (mut low, mut high) = (0, extent);
        while low < high {
            let mid = low + (high - low) / 2;
            if load(mid)?.before <= lo { low = mid + 1 } else { high = mid }
        }
        let start = low.checked_sub(1).ok_or(Error::ReaderRebuildRequired)?;
        let mut ordinals = Vec::with_capacity((hi - lo) as usize);
        for n in start..extent {
            let RankBlock { first: base, before: mut visible, mask } = load(n)?;
            if visible >= hi {
                break;
            }

            for bit in 0..256 {
                if mask[bit / 8] & (1 << (bit % 8)) != 0 {
                    if (lo..hi).contains(&visible) {
                        ordinals.push(base + bit as u64);
                    }
                    visible += 1;
                }
            }
        }
        if ordinals.len() as u64 != hi - lo {
            return Err(Error::ReaderRebuildRequired);
        }
        window.physical(owner, ordinals);

        Ok(())
    }
}
