//! Responsibilities: attach bounded slices while compressing only absent content.
//! Not responsible for: choosing a native codec per source or publishing revisions.
use super::{Archive, SLICE_BYTES, revisions::integrity};
use crate::{Error, Result};
use sha2::{Digest, Sha256};
use vault_store::{Db, OptionalRow, Priority, params};

impl Archive {
    /// Reused slices cost a hash and manifest write, never another compression pass.
    pub fn append_chunk(&self, revision: i64, ordinal: u64, bytes: Vec<u8>) -> Result<()> {
        if bytes.is_empty() || bytes.len() > SLICE_BYTES || ordinal > i64::MAX as u64 {
            return Err(Error::InvalidInput("slice size or ordinal is outside its bounds".into()));
        }
        let hash = hex::encode(Sha256::digest(&bytes));
        let length = bytes.len() as i64;
        let mut encoded: Option<Vec<u8>> = None;
        loop {
            let id = hash.clone();
            let payload = encoded.clone();
            let attached = self.store.write(Db::SourceEvidence, Priority::Batch, move |tx| {
                let (state, next, codec) = tx.query_one(
                    "SELECT state, next_ordinal, codec FROM ah_revisions WHERE id=?1",
                    [revision],
                    |row| Ok((row.get::<String>(0)?, row.get::<i64>(1)?, row.get::<String>(2)?)),
                )?;
                if state != "staged" || next != ordinal as i64 {
                    return Err(integrity("slice does not continue an unpublished revision"));
                }
                let present = tx
                    .query_one("SELECT 1 FROM ah_chunks WHERE hash=?1", [id.as_str()], |row| {
                        Ok(row.get::<i64>(0)?)
                    })
                    .optional()?
                    .is_some();
                if !present {
                    let Some(payload) = payload else {
                        return Ok(Some(codec));
                    };
                    tx.execute(
                        "INSERT INTO ah_chunks(hash, data, codec) VALUES(?1, ?2, ?3)",
                        params![id.clone(), payload, codec],
                    )?;
                }
                tx.execute(
                    "INSERT INTO ah_revision_chunks( revision_id, ordinal, chunk_hash,
                        byte_len, byte_offset) SELECT ?1, ?2, ?3, ?4, staged_byte_len FROM
                        ah_revisions WHERE id=?1",
                    params![revision, ordinal as i64, id, length],
                )?;
                tx.execute(
                    "UPDATE ah_revisions SET next_ordinal=next_ordinal+1,
                        staged_byte_len=staged_byte_len+?1 WHERE id=?2",
                    params![length, revision],
                )?;
                Ok(None)
            })?;
            let Some(codec) = attached else {
                return Ok(());
            };
            // A missing slice is encoded outside the writer transaction. The next
            // atomic attach rechecks existence, covering concurrent reclamation.
            encoded = Some(if codec == "zstd" {
                zstd::stream::encode_all(bytes.as_slice(), 3)?
            } else {
                bytes.clone()
            });
        }
    }
}
