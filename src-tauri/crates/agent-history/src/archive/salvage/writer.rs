//! Bound writes to the private salvage target by both row count and encoded
//! bytes. Only the final, verified digest can advance a retained source head.
use super::{Archive, EVIDENCE, VerifiedSlice};
use crate::archive::{RevisionInfo, SourceInfo};
use vault_store::{Priority, Result, Session, params};

enum Change {
    Begin(Box<(RevisionInfo, Option<SourceInfo>)>),
    Slice(i64, i64, VerifiedSlice),
    Finish(RevisionInfo, Option<i64>, i64),
}
impl Change {
    fn apply(self, tx: &Session<'_>) -> Result<()> {
        match self {
            Self::Begin(header) => {
                let (r, descriptor) = *header;
                if let Some(d) = descriptor {
                    tx.execute("INSERT OR IGNORE INTO ah_sources
                        (id,harness,root,relative_path,kind,retention,observed_at,last_seen_at,missing_since)
                        VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9)",
                        params![r.source_id.clone(),d.harness,d.root,d.relative_path,d.kind,d.retention,d.observed_at,d.last_seen_at,d.missing_since])?;
                } else {
                    tx.execute("INSERT OR IGNORE INTO ah_sources(id,harness,root,relative_path,kind,retention,observed_at)
                        VALUES(?1,'','','','metadata','state',?2)",params![r.source_id.clone(),r.captured_at])?;
                }
                tx.execute(
                    "INSERT INTO ah_revisions(id,source_id,captured_at,state,capture_format,codec)
                    VALUES(?1,?2,?3,'staged',?4,'zstd')",
                    params![r.id, r.source_id, r.captured_at, r.capture_format],
                )?;
            }
            Self::Slice(id, ordinal, slice) => {
                tx.execute(
                    "INSERT OR IGNORE INTO ah_chunks(hash,data,codec) VALUES(?1,?2,?3)",
                    params![slice.hash.clone(), slice.encoded, slice.codec],
                )?;
                tx.execute("INSERT INTO ah_revision_chunks(revision_id,ordinal,chunk_hash,byte_len,byte_offset)
                    VALUES(?1,?2,?3,?4,?5)",params![id,ordinal,slice.hash,slice.len,slice.offset])?;
            }
            Self::Finish(r, original_head, ordinal) => {
                tx.execute(
                    "UPDATE ah_revisions SET state='complete',digest=?1,byte_len=?2,
                    staged_byte_len=?2,next_ordinal=?3 WHERE id=?4",
                    params![r.digest, r.byte_len as i64, ordinal, r.id],
                )?;
                tx.execute(
                    "UPDATE ah_sources SET latest_revision_id=?1 WHERE id=?2 AND
                    (latest_revision_id IS NULL OR latest_revision_id IS NOT ?3)",
                    params![r.id, r.source_id, original_head],
                )?;
            }
        }
        Ok(())
    }
}
#[derive(Default)]
pub(super) struct Batch {
    changes: Vec<Change>,
    encoded_bytes: usize,
}
impl Batch {
    pub fn begin(
        &mut self,
        archive: &Archive,
        info: &RevisionInfo,
        descriptor: Option<SourceInfo>,
    ) -> Result<()> {
        self.push(archive, Change::Begin(Box::new((info.clone(), descriptor))))
    }
    pub fn slice(
        &mut self,
        archive: &Archive,
        id: i64,
        ordinal: i64,
        slice: VerifiedSlice,
    ) -> Result<()> {
        self.encoded_bytes += slice.encoded.len();
        self.push(archive, Change::Slice(id, ordinal, slice))
    }
    pub fn finish(
        &mut self,
        archive: &Archive,
        info: RevisionInfo,
        head: Option<i64>,
        ordinal: i64,
    ) -> Result<()> {
        self.push(archive, Change::Finish(info, head, ordinal))
    }
    fn push(&mut self, archive: &Archive, change: Change) -> Result<()> {
        self.changes.push(change);
        if self.encoded_bytes >= 4 * 1024 * 1024 || self.changes.len() >= 128 {
            self.flush(archive)?;
        }
        Ok(())
    }
    pub fn flush(&mut self, archive: &Archive) -> Result<()> {
        if self.changes.is_empty() {
            return Ok(());
        }
        let changes = std::mem::take(&mut self.changes);
        self.encoded_bytes = 0;
        archive.store.write(EVIDENCE, Priority::Batch, move |tx| {
            for change in changes {
                change.apply(tx)?;
            }
            Ok(())
        })
    }
}
