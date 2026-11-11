//! Responsibilities: stream retained bytes with per-chunk and whole-revision verification.
//! Not responsible for: decompression, parsing or opening live source files.
use super::{
    Archive, RevisionInfo,
    revisions::{CHUNK_BYTES, integrity},
};
use crate::{Error, Result};
use sha2::{Digest, Sha256};
use std::{
    io::{self, Read},
    sync::Arc,
};
use vault_store::{Db, OptionalRow, Store, params};

/// A bounded reader over a complete revision. Reading through EOF verifies the
/// whole digest; callers stopping early have verified only the chunks they read.
pub struct RawReader {
    store: Arc<Store>,
    info: RevisionInfo,
    expected_chunks: u64,
    next_ordinal: u64,
    chunk: Vec<u8>,
    offset: usize,
    read_bytes: u64,
    digest: Sha256,
    verified: bool,
    failed: bool,
}

impl Archive {
    /// Returns a reader only for published evidence. Every consumed chunk is hash
    /// checked, including bytes reconstructed from a deduplicated older capture.
    pub fn raw_reader(&self, revision: i64) -> Result<RawReader> {
        let info = self.revision(revision)?;
        let chunks: i64 = self.store.read(Db::SourceEvidence, move |tx| {
            tx.query_one(
                "SELECT next_ordinal FROM ah_revisions WHERE id=?1 AND state='complete'",
                [revision],
                |r| Ok(r.get(0)?),
            )
        })?;
        if chunks < 0 {
            return Err(Error::Integrity("negative revision chunk count".into()));
        }
        Ok(RawReader::new(self.store.clone(), info, chunks as u64))
    }
}

impl RawReader {
    pub(super) fn new(store: Arc<Store>, info: RevisionInfo, expected_chunks: u64) -> Self {
        Self {
            store,
            info,
            expected_chunks,
            next_ordinal: 0,
            chunk: Vec::new(),
            offset: 0,
            read_bytes: 0,
            digest: Sha256::new(),
            verified: false,
            failed: false,
        }
    }

    fn load_chunk(&mut self) -> io::Result<bool> {
        if self.next_ordinal == self.expected_chunks {
            if self.read_bytes != self.info.byte_len
                || hex::encode(self.digest.clone().finalize()) != self.info.digest
            {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "revision length or digest does not match retained bytes",
                ));
            }
            self.verified = true;
            return Ok(false);
        }
        let revision = self.info.id;
        let ordinal = self.next_ordinal as i64;
        let chunk = self.store.read(Db::SourceEvidence, move |tx| {
            tx.query_one("SELECT c.hash,c.data,m.byte_len FROM ah_revision_chunks m JOIN ah_chunks c ON c.hash=m.chunk_hash WHERE m.revision_id=?1 AND m.ordinal=?2",
                params![revision,ordinal], |row| Ok((row.get::<String>(0)?,row.get::<Vec<u8>>(1)?,row.get::<i64>(2)?))).optional()?
                .ok_or_else(|| integrity("a retained revision chunk is missing"))
        }).map_err(io::Error::other)?;
        if chunk.1.is_empty()
            || chunk.1.len() > CHUNK_BYTES
            || chunk.2 != chunk.1.len() as i64
            || hex::encode(Sha256::digest(&chunk.1)) != chunk.0
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "retained chunk length or digest is invalid",
            ));
        }
        if self.read_bytes.saturating_add(chunk.1.len() as u64) > self.info.byte_len {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "retained chunks exceed the revision boundary",
            ));
        }
        self.chunk = chunk.1;
        self.offset = 0;
        self.next_ordinal += 1;
        Ok(true)
    }

    fn read_checked(&mut self, output: &mut [u8]) -> io::Result<usize> {
        if self.verified || output.is_empty() {
            return Ok(0);
        }
        if self.offset == self.chunk.len() && !self.load_chunk()? {
            return Ok(0);
        }
        let count = output.len().min(self.chunk.len() - self.offset);
        let bytes = &self.chunk[self.offset..self.offset + count];
        output[..count].copy_from_slice(bytes);
        self.digest.update(bytes);
        self.read_bytes += count as u64;
        self.offset += count;
        Ok(count)
    }
}

impl Read for RawReader {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        if self.failed {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "revision reader failed integrity verification",
            ));
        }
        let result = self.read_checked(output);
        self.failed = result.is_err();
        result
    }
}
