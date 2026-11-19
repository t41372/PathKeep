//! Merges bounded sorted native-ID runs in the encrypted-or-plaintext R role.
//! IDs are verified against the outline after seeking by digest.
use super::{
    build::{key, require_staging},
    pages::{self, Buffer},
    *,
};
use crate::Archive;
use std::{
    cmp::Reverse,
    collections::{BinaryHeap, VecDeque},
};
use vault_store::{OptionalRow, Priority, params};
type Posting = (String, u64);
const PAGE: usize = 256;
const FAN_IN: usize = 16;

#[derive(Default)]
pub(super) struct SortedRuns {
    entries: Vec<Posting>,
    runs: usize,
}
impl SortedRuns {
    pub fn push(&mut self, a: &Archive, token: &str, id: &str, ordinal: u64) -> Result<()> {
        self.entries.push((key(id), ordinal));
        if self.entries.len() == PAGE {
            self.spill(a, token)?
        }
        Ok(())
    }
    fn spill(&mut self, a: &Archive, token: &str) -> Result<()> {
        if self.entries.is_empty() {
            return Ok(());
        }
        self.entries.sort_unstable();
        self.entries.dedup();
        save(a, token, 0, self.runs, 0, &self.entries)?;
        self.runs += 1;
        self.entries.clear();
        Ok(())
    }
    pub fn finish(
        mut self,
        a: &Archive,
        token: &str,
        job: &JobControl,
        pi: bool,
        header: Option<u64>,
        last_entries: (Option<u64>, Option<u64>),
    ) -> Result<()> {
        self.spill(a, token)?;
        let mut level = 0;
        while self.runs > FAN_IN {
            for (out, first) in (0..self.runs).step_by(FAN_IN).enumerate() {
                let mut page = 0;
                merge(a, token, level, first..(first + FAN_IN).min(self.runs), job, |rows| {
                    save(a, token, level + 1, out, page, rows)?;
                    page += 1;
                    Ok(())
                })?;
            }
            self.runs = self.runs.div_ceil(FAN_IN);
            level += 1;
        }
        let mut writer = Buffer::new(LOOKUP, pages::META_BYTES);
        let mut directory = Buffer::new(DIRECTORY, pages::META_BYTES);
        let mut leaves = Buffer::new(LEAVES, pages::META_BYTES);
        let mut latest = None;
        // Events with one ID are ordered by physical position. The reference at n sorts
        // before the definition at n: only a strictly earlier occurrence is eligible.
        let mut group = String::new();
        let mut first_id = None;
        let mut multiple = false;
        let mut used = false;
        let mut duplicates = false;
        let mut emit_leaf = |n: u64| -> Result<()> {
            if pi && Some(n) != header {
                leaves.push(a, token, &n)?;
                latest = Some(latest.map_or(n, |old: u64| old.max(n)));
            }
            Ok(())
        };
        merge(a, token, level, 0..self.runs, job, |rows| {
            let mut first = None;
            let rank = writer.len();
            let mut uncertain = Vec::new();
            for (key, position) in rows {
                if key != &group {
                    if let Some(n) = first_id
                        && !multiple
                        && !used
                    {
                        emit_leaf(n)?
                    }
                    group.clone_from(key);
                    first_id = None;
                    multiple = false;
                    used = false;
                }
                let ordinal = position / 2;
                if position % 2 == 0 {
                    if multiple {
                        uncertain.push(ordinal)
                    } else if first_id.is_some() {
                        used = true;
                    }
                    continue;
                }
                if let Some(first) = first_id {
                    if !multiple && !used {
                        emit_leaf(first)?;
                    }
                    multiple = true;
                    duplicates = true;
                    // A second or subsequent occurrence can never be the unique earlier match.
                    emit_leaf(ordinal)?;
                } else {
                    first_id = Some(ordinal);
                }
                let posting = (key.clone(), ordinal);
                first.get_or_insert_with(|| posting.clone());
                writer.push(a, token, &posting)?;
            }
            if !uncertain.is_empty() {
                let owner = token.to_owned();
                a.store.write(DB, Priority::Batch, move |tx| {
                    require_staging(tx, &owner)?;
                    let mut insert =
                        tx.prepare("INSERT INTO ah_reader_ambiguous_stage VALUES(?1,?2)")?;
                    for n in uncertain {
                        insert.execute(params![owner.clone(), n as i64])?;
                    }
                    Ok(())
                })?;
            }
            let Some(first) = first else { return Ok(()) };
            writer.flush(a, token)?;
            directory.push(a, token, &(first.0, first.1, rank))?;
            Ok(())
        })?;
        if let Some(n) = first_id
            && !multiple
            && !used
        {
            emit_leaf(n)?
        }
        directory.flush(a, token)?;
        let mut ambiguous = Buffer::new(AMBIGUOUS, pages::META_BYTES);
        let mut after = -1i64;
        loop {
            job.cancel.check().map_err(|_| Error::Cancelled)?;
            let owner = token.to_owned();
            let rows=a.store.write(DB,Priority::Batch,move|tx|{
                let rows=tx.query("SELECT ordinal FROM ah_reader_ambiguous_stage WHERE owner=?1 AND ordinal>?2 ORDER BY ordinal LIMIT 256",params![owner.clone(),after],|r|Ok(r.get::<i64>(0)?))?;
                if let Some(last)=rows.last(){tx.execute("DELETE FROM ah_reader_ambiguous_stage WHERE owner=?1 AND ordinal<=?2",params![owner,*last])?;}
                Ok(rows)
            })?;
            if rows.is_empty() {
                break;
            }
            for n in rows {
                after = n;
                ambiguous.push(a, token, &(n as u64))?;
            }
        }
        ambiguous.flush(a, token)?;
        let ambiguous_count = ambiguous.len() as i64;
        let extent = directory.len() as i64;
        let postings = writer.len() as i64;
        let leaf_count = if pi { leaves.len() as i64 } else { -1 };
        let owner = token.to_owned();
        a.store.write(DB, Priority::Batch, move |tx| {
            tx.execute(
                "UPDATE ah_reader_version SET lookup_pages=?1, postings=?2, header_ordinal=?3,
                 leaf_count=?5, directory_pages=?6, duplicate_ids=?7,ambiguous_edges=?8 WHERE token=?4",
                params![
                    extent,
                    postings,
                    header.map(|n| n as i64),
                    owner,
                    leaf_count,
                    (extent + 255) / 256,
                    i64::from(duplicates),ambiguous_count
                ],
            )
        })?;
        Extents::seal(a, token)?;
        if pi {
            leaves.flush(a, token)?;
            a.stage_selected_leaf(
                token,
                leaves.len(),
                latest,
                if latest.is_none() { last_entries.0 } else { last_entries.1 },
                job,
            )?;
        }
        Ok(())
    }
}

fn save(
    a: &Archive,
    token: &str,
    level: usize,
    run: usize,
    page: usize,
    rows: &[Posting],
) -> Result<()> {
    let bytes = postcard::to_allocvec(rows).map_err(pages::invalid)?;
    let owner = token.to_owned();
    a.store.write(DB, Priority::Batch, move |tx| {
        require_staging(tx, &owner)?;
        tx.execute(
            "INSERT INTO ah_reader_sort VALUES(?1,?2,?3,?4,?5)",
            params![owner, level as i64, run as i64, page as i64, bytes],
        )?;
        Ok(())
    })?;
    Ok(())
}
struct Run {
    index: usize,
    page: usize,
    pending: VecDeque<Posting>,
}
impl Run {
    fn next(&mut self, a: &Archive, token: &str, level: usize) -> Result<Option<Posting>> {
        if let Some(value) = self.pending.pop_front() {
            return Ok(Some(value));
        }
        let owner = token.to_owned();
        let (run, page) = (self.index as i64, self.page as i64);
        let bytes = a.store.write(DB, Priority::Batch, move |tx| {
            require_staging(tx, &owner)?;
            let bytes = tx.query_one("SELECT data FROM ah_reader_sort WHERE owner=?1 AND level=?2 AND run=?3 AND page=?4", params![owner.clone(), level as i64, run, page], |r| Ok(r.get::<Vec<u8>>(0)?)).optional()?;
            if bytes.is_some() {
                tx.execute("DELETE FROM ah_reader_sort WHERE owner=?1 AND level=?2 AND run=?3 AND page=?4", params![owner, level as i64, run, page])?;
            }
            Ok(bytes)
        })?;
        let Some(bytes) = bytes else { return Ok(None) };
        if bytes.len() > pages::META_BYTES {
            return Err(pages::invalid("lookup run length"));
        }
        self.pending = postcard::from_bytes(&bytes).map_err(pages::invalid)?;
        self.page += 1;
        Ok(self.pending.pop_front())
    }
}
fn merge(
    a: &Archive,
    token: &str,
    level: usize,
    indices: std::ops::Range<usize>,
    job: &JobControl,
    mut output: impl FnMut(&[Posting]) -> Result<()>,
) -> Result<()> {
    let mut runs: Vec<_> =
        indices.map(|index| Run { index, page: 0, pending: VecDeque::new() }).collect();
    let mut heap = BinaryHeap::new();
    for (i, run) in runs.iter_mut().enumerate() {
        if let Some(value) = run.next(a, token, level)? {
            heap.push(Reverse((value, i)))
        }
    }
    let mut page = Vec::with_capacity(PAGE);
    while let Some(Reverse((value, i))) = heap.pop() {
        page.push(value);
        if page.len() == PAGE {
            job.cancel.check().map_err(|_| Error::Cancelled)?;
            output(&page)?;
            page.clear();
        }
        if let Some(next) = runs[i].next(a, token, level)? {
            heap.push(Reverse((next, i)))
        }
    }
    if !page.is_empty() {
        output(&page)?
    }
    Ok(())
}
