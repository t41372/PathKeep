//! Responsibilities: settle prefix comparisons from retained evidence before reading bytes.
//! Not responsible for: trusting source timestamps or selecting a conversation's current source.
use super::{Archive, RevisionInfo};
use crate::{Error, Result, adapters::common};
use std::io::{BufRead, BufReader, Read};
use vault_store::{Db, params};

struct Slice {
    offset: u64,
    length: u64,
    hash: String,
}

// A retained prefix may end inside its owner's last slice.
fn covering_slice<'a>(
    manifest: &'a [Slice],
    index: usize,
    position: u64,
    revision: &RevisionInfo,
) -> Result<(&'a Slice, u64)> {
    if let Some(slice) = manifest.get(index)
        && let Some(end) = slice.offset.checked_add(slice.length)
        && (slice.offset..end).contains(&position)
    {
        return Ok((slice, end));
    }
    Err(Error::Integrity(format!(
        "source {} revision {} has no valid slice at byte {position}",
        revision.source_id, revision.id,
    )))
}

impl Archive {
    /// Equal whole digests or aligned slice addresses prove equality without decoding again.
    pub(crate) fn revisions_prefix(&self, first: i64, second: i64) -> Result<(bool, bool)> {
        let a = self.revision(first)?;
        let b = self.revision(second)?;
        self.revision_prefix(&a, &b)
    }

    pub(crate) fn revision_prefix(
        &self,
        a: &RevisionInfo,
        b: &RevisionInfo,
    ) -> Result<(bool, bool)> {
        self.revision_prefix_cancellable(a, b, &vault_store::CancelToken::new())
    }

    pub(crate) fn revision_prefix_cancellable(
        &self,
        a: &RevisionInfo,
        b: &RevisionInfo,
        cancel: &vault_store::CancelToken,
    ) -> Result<(bool, bool)> {
        if a.byte_len > b.byte_len {
            return Ok((false, false));
        }
        if a.byte_len == b.byte_len && a.digest == b.digest {
            return Ok((true, true));
        }
        let left = self.manifest(a)?;
        let right = self.manifest(b)?;
        let (mut i, mut j, mut offset) = (0, 0, 0);
        while offset < a.byte_len {
            cancel.check().map_err(|_| Error::Cancelled)?;
            let (x, x_end) = covering_slice(&left, i, offset, a)?;
            let (y, y_end) = covering_slice(&right, j, offset, b)?;
            let end = x_end.min(y_end).min(a.byte_len);
            if x.offset != y.offset || x.hash != y.hash {
                let mut first = self.raw_window(a, offset, end - offset);
                let mut second = self.raw_window(b, offset, end - offset);
                let mut left = [0; 64 * 1024];
                let mut right = [0; 64 * 1024];
                let mut remaining = end - offset;
                while remaining > 0 {
                    cancel.check().map_err(|_| Error::Cancelled)?;
                    let n = remaining.min(left.len() as u64) as usize;
                    first.read_exact(&mut left[..n])?;
                    second.read_exact(&mut right[..n])?;
                    if left[..n] != right[..n] {
                        return Ok((false, false));
                    }
                    remaining -= n as u64;
                }
            }
            offset = end;
            if offset == x_end {
                i += 1;
            }
            if offset == y_end {
                j += 1;
            }
        }
        Ok((true, a.byte_len == b.byte_len))
    }

    fn manifest(&self, revision: &RevisionInfo) -> Result<Vec<Slice>> {
        let id = revision.id;
        let length = revision.byte_len as i64;
        Ok(self.store.read(Db::AgentHistoryEvidence, move |tx| {
            tx.query(
                "SELECT m.byte_offset, m.byte_len, m.chunk_hash FROM ah_revisions r JOIN
                    ah_revision_chunks m ON m.revision_id=coalesce(r.manifest_revision_id, r.id)
                    WHERE r.id=?1 AND m.byte_offset<?2 ORDER BY m.byte_offset",
                params![id, length],
                |row| {
                    Ok(Slice {
                        offset: row.get::<i64>(0)? as u64,
                        length: row.get::<i64>(1)? as u64,
                        hash: row.get(2)?,
                    })
                },
            )
        })?)
    }

    /// Only native compressed transports require a decoded comparison when raw metadata differs.
    pub(crate) fn sources_prefix(
        &self,
        first: i64,
        first_path: &str,
        second: i64,
        second_path: &str,
    ) -> Result<(bool, bool)> {
        let compressed = |path: &str| path.ends_with(".zst") || path.ends_with(".zstd");
        if !compressed(first_path) && !compressed(second_path) {
            return self.revisions_prefix(first, second);
        }
        let a = self.revision(first)?;
        let b = self.revision(second)?;
        if a.digest == b.digest && a.byte_len == b.byte_len {
            return Ok((true, true));
        }
        let a = common::decoded_reader(Box::new(self.raw_reader(first)?), first_path)?;
        let b = common::decoded_reader(Box::new(self.raw_reader(second)?), second_path)?;
        let (mut a, mut b) = (BufReader::new(a), BufReader::new(b));
        loop {
            let left = a.fill_buf()?;
            let right = b.fill_buf()?;
            if left.is_empty() {
                return Ok((true, right.is_empty()));
            }
            if right.is_empty() {
                return Ok((false, false));
            }
            let n = left.len().min(right.len());
            if left[..n] != right[..n] {
                return Ok((false, false));
            }
            a.consume(n);
            b.consume(n);
        }
    }
}
