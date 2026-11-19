//! Responsibilities: stream retained bytes with per-chunk and whole-revision verification.
//! Not responsible for: native transport decoding, parsing or opening live source files.
use super::{Archive, RevisionInfo, SLICE_BYTES as CHUNK_BYTES, revisions::integrity};
use crate::{
    Error, Result,
    read_model::{JobControl, ProgressUnit, ReadPhase, ReadProgress},
};
use sha2::{Digest, Sha256};
use std::{
    io::{self, Read},
    sync::Arc,
};
use vault_store::{Db, OptionalRow, Store, params};

#[derive(Debug)]
struct ExpiredRevision;

impl std::fmt::Display for ExpiredRevision {
    fn fmt(&self, output: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        output.write_str("revision bytes expired by retention")
    }
}

impl std::error::Error for ExpiredRevision {}

/// Export must distinguish intentional expiry from a destination's NotFound I/O error.
pub(crate) fn expired_read(error: &io::Error) -> bool {
    error.get_ref().is_some_and(|cause| cause.is::<ExpiredRevision>())
}

fn expired() -> io::Error {
    io::Error::new(io::ErrorKind::NotFound, ExpiredRevision)
}

/// A bounded reader over a complete revision. Reading through EOF verifies the
/// whole digest; callers stopping early have verified only the chunks they read.
pub struct RawReader {
    store: Arc<Store>,
    jobs: Arc<super::lifetime::Jobs>,
    info: RevisionInfo,
    chunk: Vec<u8>,
    offset: usize,
    read_bytes: u64,
    digest: Sha256,
    verified: bool,
    failed: bool,
    scan: Option<JobControl>,
}

impl Archive {
    /// Returns a reader only for published evidence. Every consumed chunk is hash
    /// checked, including bytes reconstructed from a deduplicated older capture.
    pub fn raw_reader(&self, revision: i64) -> Result<RawReader> {
        let info = self.revision(revision)?;
        if !info.bytes_retained {
            return Err(Error::NotFound("revision bytes expired by retention".into()));
        }
        Ok(RawReader::new(self, info))
    }
}

impl Archive {
    /// Range consumers authenticate their slices; only a complete read proves the whole digest.
    pub(crate) fn raw_window(
        &self,
        info: &RevisionInfo,
        offset: u64,
        length: u64,
    ) -> std::io::Take<RawReader> {
        let mut reader = RawReader::new(self, info.clone());
        reader.read_bytes = offset;
        reader.take(length)
    }
}

impl RawReader {
    pub(crate) fn new(archive: &Archive, info: RevisionInfo) -> Self {
        Self {
            store: archive.store.clone(),
            jobs: archive.jobs.clone(),
            info,
            chunk: Vec::new(),
            offset: 0,
            read_bytes: 0,
            digest: Sha256::new(),
            verified: false,
            failed: false,
            scan: None,
        }
    }

    /// Seeks directly to a native range, retaining the authenticated slice when the
    /// new start is inside it. The bounded view never requests whole-stream EOF verification.
    pub(crate) fn range(&mut self, start: u64, length: u64) -> io::Take<&mut Self> {
        let chunk_start = self.read_bytes - self.offset as u64;
        match start.checked_sub(chunk_start).and_then(|n| usize::try_from(n).ok()) {
            Some(offset) if offset < self.chunk.len() => self.offset = offset,
            _ => {
                self.chunk.clear();
                self.offset = 0;
            }
        }
        self.read_bytes = start;
        self.take(length)
    }

    pub(crate) fn with_scan_control(mut self, job: &JobControl) -> Self {
        self.scan = Some(job.clone());
        self
    }

    fn slice_progress(&self, phase: ReadPhase, done: usize, total: Option<u64>) -> io::Result<()> {
        if let Some(job) = &self.scan {
            if let Some(report) = &job.progress {
                report(ReadProgress {
                    phase,
                    unit: ProgressUnit::SliceBytes,
                    done: done as u64,
                    total,
                });
            }
            job.cancel.check().map_err(|_| io::Error::other(Error::Cancelled))?;
        }
        Ok(())
    }

    fn load_chunk(&mut self) -> io::Result<bool> {
        if self.read_bytes == self.info.byte_len {
            let revision = self.info.id;
            let retained = self
                .store
                .read(Db::AgentHistoryEvidence, move |tx| {
                    tx.query_one(
                        "SELECT bytes_retained FROM ah_revisions WHERE id=?1",
                        [revision],
                        |row| Ok(row.get::<i64>(0)? != 0),
                    )
                })
                .map_err(io::Error::other)?;
            if !retained {
                return Err(expired());
            }
            if hex::encode(self.digest.clone().finalize()) != self.info.digest {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "revision length or digest does not match retained bytes",
                ));
            }
            self.verified = true;
            return Ok(false);
        }
        let revision = self.info.id;
        let position = self.read_bytes as i64;
        // A cold encrypted payload fetch, inflation and SHA verification used to be one
        // uninterrupted operation. Keep the full-slice authentication boundary while reporting
        // completed storage and verification blocks before the first native byte is released.
        const FETCH: usize = 256 * 1024;
        let observed = self.scan.as_ref().is_some_and(|job| job.progress.is_some());
        let (hash, data, length, codec, start) = loop {
            self.slice_progress(ReadPhase::FetchingSlice, 0, None)?;
            let loaded = self
            .store
            .read(Db::AgentHistoryEvidence, move |tx| {
                let (retained, manifest) = tx.query_one(
                    "SELECT bytes_retained, coalesce(manifest_revision_id,id) FROM ah_revisions WHERE id=?1",
                    [revision],
                    |row| Ok((row.get::<i64>(0)?, row.get::<i64>(1)?)),
                )?;
                if retained == 0 {
                    return Ok(None);
                }
                // Turso sorts preceding payloads if the blob join participates in ORDER BY.
                // Pin the existing offset index: the ordinal index also starts with revision_id.
                let (hash, length, start) = tx.query_one(
                    "SELECT chunk_hash, byte_len, byte_offset FROM ah_revision_chunks
                     INDEXED BY ah_revision_chunks_offset WHERE revision_id=?1 AND byte_offset<=?2
                     ORDER BY byte_offset DESC LIMIT 1",
                    params![manifest, position],
                    |row| Ok((row.get::<String>(0)?, row.get::<i64>(1)?, row.get::<i64>(2)?)),
                )
                .optional()?
                .ok_or_else(|| integrity("a retained revision chunk is missing"))?;
                let (data, codec) = if observed {
                    tx.query_one("SELECT substr(data,1,?2),codec FROM ah_chunks WHERE hash=?1",
                        params![hash.clone(),FETCH as i64],|r|Ok((r.get::<Vec<u8>>(0)?,r.get::<String>(1)?))).optional()?
                }else{
                    tx.query_one("SELECT data,codec FROM ah_chunks WHERE hash=?1",[hash.as_str()],
                        |r|Ok((r.get::<Vec<u8>>(0)?,r.get::<String>(1)?))).optional()?
                }.ok_or_else(||integrity("a retained revision chunk is missing"))?;
                Ok(Some((hash,data,length,codec,start)))
            }).map_err(io::Error::other)?;
            let (hash, mut data, length, codec, start) = loaded.ok_or_else(expired)?;
            let mut more = observed && data.len() == FETCH;
            let mut retargeted = false;
            while more {
                self.slice_progress(ReadPhase::FetchingSlice, data.len(), None)?;
                let key = hash.clone();
                let offset = data.len() as i64 + 1;
                let next = self
                    .store
                    .read(Db::AgentHistoryEvidence, move |tx| {
                        tx.query_one(
                            "SELECT substr(data,?2,?3) FROM ah_chunks WHERE hash=?1",
                            params![key, offset, FETCH as i64],
                            |r| Ok(r.get::<Vec<u8>>(0)?),
                        )
                        .optional()
                    })
                    .map_err(io::Error::other)?;
                let Some(piece) = next else {
                    // Retention can retarget a shared prefix between two bounded reads.
                    // Re-resolve its manifest; a genuinely missing slice fails that lookup.
                    retargeted = true;
                    break;
                };
                more = piece.len() == FETCH;
                data.extend(piece);
                if data.len() > 2 * CHUNK_BYTES {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "oversized stored slice",
                    ));
                }
            }
            if !retargeted {
                break (hash, data, length, codec, start);
            }
        };
        self.slice_progress(ReadPhase::FetchingSlice, data.len(), Some(data.len() as u64))?;
        let mut digest = Sha256::new();
        let bytes = match codec.as_str() {
            "zstd" => {
                let invalid = |e| io::Error::new(io::ErrorKind::InvalidData, e);
                let mut decoder =
                    zstd::stream::read::Decoder::new(data.as_slice()).map_err(invalid)?;
                decoder.window_log_max(26).map_err(invalid)?;
                let mut bytes = Vec::with_capacity(length.clamp(0, CHUNK_BYTES as i64) as usize);
                let mut buffer = vec![0; 64 * 1024];
                loop {
                    let n = decoder.read(&mut buffer).map_err(invalid)?;
                    if n == 0 {
                        break;
                    }
                    if bytes.len() + n > CHUNK_BYTES {
                        return Err(io::Error::new(
                            io::ErrorKind::InvalidData,
                            "oversized decoded slice",
                        ));
                    }
                    bytes.extend_from_slice(&buffer[..n]);
                    digest.update(&buffer[..n]);
                    self.slice_progress(
                        ReadPhase::VerifyingSlice,
                        bytes.len(),
                        Some(length as u64),
                    )?;
                }
                bytes
            }
            "native" => {
                let mut done = 0;
                for part in data.chunks(64 * 1024) {
                    digest.update(part);
                    done += part.len();
                    self.slice_progress(ReadPhase::VerifyingSlice, done, Some(length as u64))?;
                }
                data
            }
            _ => return Err(io::Error::new(io::ErrorKind::InvalidData, "unknown slice codec")),
        };
        if bytes.is_empty()
            || bytes.len() > CHUNK_BYTES
            || length != bytes.len() as i64
            || hex::encode(digest.finalize()) != hash
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "retained chunk length or digest is invalid",
            ));
        }
        let remaining = self.info.byte_len.checked_sub(self.read_bytes).ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidData, "slice position exceeds revision length")
        })?;
        self.chunk = bytes;
        let start = u64::try_from(start)
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "negative manifest offset"))?;
        self.offset = (self.read_bytes - start) as usize;
        if self.offset >= self.chunk.len() {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "slice manifest has a gap"));
        }
        self.chunk.truncate(self.offset.saturating_add(remaining.min(usize::MAX as u64) as usize));
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
        let jobs = self.jobs.clone();
        let _read = jobs.enter(None).map_err(io::Error::other)?;
        if !self.store.is_open() {
            return Err(io::Error::other(vault_store::Error::Closed));
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Artifact, ArtifactKind, Harness, SourceRoot};

    #[test]
    fn slice_seek_selects_a_manifest_offset_before_fetching_one_blob() {
        let temp = tempfile::tempdir().unwrap();
        let archive = Archive::create_unencrypted(temp.path()).unwrap();
        let root = SourceRoot { harness: Harness::Pi, root: "/fixture".into() };
        let artifact = Artifact {
            kind: ArtifactKind::Transcript,
            path: "/fixture/a.jsonl".into(),
            relative_path: "a.jsonl".into(),
            generation: None,
            current: true,
            project_hint: None,
        };
        let revision = archive.begin_revision("source", &root, &artifact).unwrap();
        for n in 0..8 {
            archive.append_chunk(revision, n, vec![n as u8; 256]).unwrap();
        }
        let bytes = (0u8..8).flat_map(|n| std::iter::repeat_n(n, 256)).collect::<Vec<_>>();
        archive.finish_revision(revision, &hex::encode(Sha256::digest(bytes)), 2048).unwrap();
        let info = archive.revision(revision).unwrap();
        archive.store.capture_query_plans(Db::AgentHistoryEvidence, true).unwrap();
        let mut result = Vec::new();
        archive.raw_window(&info, 2016, 32).read_to_end(&mut result).unwrap();
        assert_eq!(result, vec![7; 32]);
        let plans = archive.store.take_query_plans(Db::AgentHistoryEvidence).unwrap();
        let plan = plans.iter().find(|plan| plan.sql.contains("ah_revision_chunks")).unwrap();
        eprintln!("manifest lookup: {:?}", plan.steps);
        assert!(
            plan.steps
                .iter()
                .any(|s| s.contains("ah_revision_chunks_offset") && s.contains("byte_offset<=?")),
            "{plan:?}"
        );
        assert!(
            plan.steps.iter().all(|s| !s.contains("TEMP B-TREE") && !s.contains("SORT")),
            "{plan:?}"
        );
        assert!(
            archive.store.take_ephemeral_tables(Db::AgentHistoryEvidence).unwrap().is_empty(),
            "a slice seek materialized candidate rows"
        );
    }
}
