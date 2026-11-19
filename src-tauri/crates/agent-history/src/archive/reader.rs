//! Responsibilities: stream retained bytes with per-chunk and whole-revision verification.
//! Not responsible for: native transport decoding, parsing or opening live source files.
use super::{Archive, RevisionInfo, SLICE_BYTES as CHUNK_BYTES};
use crate::{Error, Result};
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
    pub(super) fn new(archive: &Archive, info: RevisionInfo) -> Self {
        Self::from_evidence(archive.store.clone(), archive.jobs.clone(), info)
    }

    fn from_evidence(
        store: Arc<Store>,
        jobs: Arc<super::lifetime::Jobs>,
        info: RevisionInfo,
    ) -> Self {
        Self {
            store,
            jobs,
            info,
            chunk: Vec::new(),
            offset: 0,
            read_bytes: 0,
            digest: Sha256::new(),
            verified: false,
            failed: false,
        }
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
        let loaded = self
            .store
            .read(Db::AgentHistoryEvidence, move |tx| {
                let retained: i64 = tx.query_one(
                    "SELECT bytes_retained FROM ah_revisions WHERE id=?1",
                    [revision],
                    |row| Ok(row.get(0)?),
                )?;
                if retained == 0 {
                    return Ok(None);
                }
                let chunk = raw_slice(tx, revision, position)?;
                Ok(Some(chunk))
            })
            .map_err(io::Error::other)?;
        let chunk = loaded.ok_or_else(expired)?.ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidData, "a retained revision chunk is missing")
        })?;
        let bytes = decode(&chunk.0, chunk.1, chunk.2, &chunk.3)?;
        let remaining = self.info.byte_len - self.read_bytes;
        self.chunk = bytes;
        let start = u64::try_from(chunk.4).map_err(invalid_data)?;
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

fn invalid_data(e: impl std::fmt::Display) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, e.to_string())
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
        if let Err(e) = &result
            && e.kind() == io::ErrorKind::InvalidData
        {
            let failure = Arc::new(vault_store::StoreOpenFailure {
                role: Db::AgentHistoryEvidence,
                stage: vault_store::OpenStage::Read,
                cause: vault_store::Error::RetainedBytes {
                    revision: self.info.id,
                    detail: e.to_string(),
                },
            });
            self.store.remember_conversation_failure(failure.clone());
            self.failed = true;
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                vault_store::Error::Open(failure),
            ));
        }
        self.failed = result.is_err();
        result
    }
}

/// Read every complete retained revision using the export reader and a bounded
/// page of identities. A concurrent retention expiry is an observed transition.
pub(crate) fn verify_retained(
    store: &Arc<Store>,
    jobs: &Arc<super::lifetime::Jobs>,
    cancel: &vault_store::CancelToken,
) -> Result<(u64, u64)> {
    let (mut after, mut bytes, mut revisions) = (0i64, 0u64, 0u64);
    loop {
        cancel.check()?;
        let page=store.read(Db::AgentHistoryEvidence,move |s| s.query(
            "SELECT id,source_id,digest,byte_len,captured_at,capture_format,bytes_retained FROM ah_revisions
             WHERE state='complete' AND bytes_retained=1 AND id>?1 ORDER BY id LIMIT 128",[after],super::revisions::map_revision))?;
        if page.is_empty() {
            return Ok((bytes, revisions));
        }
        for info in page {
            cancel.check()?;
            after = info.id;
            let mut reader = RawReader::from_evidence(store.clone(), jobs.clone(), info);
            match io::copy(&mut reader, &mut io::sink()) {
                Ok(n) => {
                    bytes += n;
                    revisions += 1;
                }
                Err(e) if expired_read(&e) => {}
                Err(e) => return Err(e.into()),
            }
        }
    }
}

impl Archive {
    /// Verify canonical bytes on a private evidence worker even if its model cannot open.
    pub fn verify_retained_evidence(
        store: &Arc<Store>,
        cancel: &vault_store::CancelToken,
    ) -> Result<()> {
        verify_retained(store, &Default::default(), cancel).map(drop)
    }
}

type Slice = (String, Vec<u8>, i64, String, i64);
fn raw_slice(
    tx: &vault_store::Session<'_>,
    revision: i64,
    position: i64,
) -> vault_store::Result<Option<Slice>> {
    tx.query_one(
        "SELECT c.hash, c.data, m.byte_len, c.codec, m.byte_offset FROM
                            ah_revisions r JOIN ah_revision_chunks m ON
                            m.revision_id=coalesce(r.manifest_revision_id, r.id) JOIN ah_chunks c
                            ON c.hash=m.chunk_hash WHERE r.id=?1 AND m.byte_offset<=?2 ORDER BY
                            m.byte_offset DESC LIMIT 1",
        params![revision, position],
        |row| {
            Ok((
                row.get::<String>(0)?,
                row.get::<Vec<u8>>(1)?,
                row.get::<i64>(2)?,
                row.get::<String>(3)?,
                row.get::<i64>(4)?,
            ))
        },
    )
    .optional()
}
fn decode(hash: &str, encoded: Vec<u8>, length: i64, codec: &str) -> io::Result<Vec<u8>> {
    let bytes = if codec == "zstd" {
        use std::io::Read;
        let mut bytes = Vec::new();
        zstd::stream::read::Decoder::new(encoded.as_slice())
            .map_err(invalid_data)?
            .take((CHUNK_BYTES + 1) as u64)
            .read_to_end(&mut bytes)
            .map_err(invalid_data)?;
        bytes
    } else if codec == "native" {
        encoded
    } else {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "unknown slice codec"));
    };
    if bytes.is_empty()
        || bytes.len() > CHUNK_BYTES
        || length != bytes.len() as i64
        || hex::encode(Sha256::digest(&bytes)) != hash
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "retained chunk length or digest is invalid",
        ));
    }
    Ok(bytes)
}

/// Verify a single immutable manifest without touching any other revision. Salvage
/// and retry share the codec, chunk bounds and hash checks with ordinary raw export.
pub(super) struct VerifiedSlice {
    pub hash: String,
    pub encoded: Vec<u8>,
    pub codec: String,
    pub len: i64,
    pub offset: i64,
}
pub(super) fn inspect_slices(
    tx: &vault_store::Session<'_>,
    info: &RevisionInfo,
    mut slice: impl FnMut(VerifiedSlice) -> vault_store::Result<()>,
) -> vault_store::Result<()> {
    let damage = |detail: String| {
        vault_store::Error::RetainedBytes { revision: info.id, detail }
            .at(Db::AgentHistoryEvidence, vault_store::OpenStage::Read)
    };
    let mut hash = Sha256::new();
    let mut position = 0u64;
    while position < info.byte_len {
        let (id, data, len, codec, offset) = raw_slice(tx, info.id, position as i64)?
            .ok_or_else(|| damage("missing manifest chunk".into()))?;
        if offset != position as i64 {
            return Err(damage("noncontiguous manifest".into()));
        }
        let mut bytes =
            decode(&id, data.clone(), len, &codec).map_err(|e| damage(e.to_string()))?;
        bytes.truncate((info.byte_len - position).min(bytes.len() as u64) as usize);
        position += bytes.len() as u64;
        hash.update(&bytes);
        let size = bytes.len() as i64;
        let verified = if size == len {
            VerifiedSlice { hash: id, encoded: data, codec, len, offset }
        } else {
            // The final slice of an inherited manifest can extend beyond this
            // revision. Copy only the prefix included in its verified digest.
            VerifiedSlice {
                hash: hex::encode(Sha256::digest(&bytes)),
                encoded: bytes,
                codec: "native".into(),
                len: size,
                offset,
            }
        };
        slice(verified)?;
    }
    if hex::encode(hash.finalize()) != info.digest {
        return Err(damage("revision digest mismatch".into()));
    }
    Ok(())
}

impl Archive {
    pub fn verify_recorded_revision(
        config: &vault_store::Config,
        id: i64,
    ) -> vault_store::Result<()> {
        Store::inspect_existing(config,Db::AgentHistoryEvidence,|s|{
            let info=s.query_one("SELECT id,source_id,digest,byte_len,captured_at,capture_format,bytes_retained FROM ah_revisions WHERE state='complete' AND bytes_retained=1 AND id=?1",[id],super::revisions::map_revision)?;
            inspect_slices(s,&info,|_|Ok(()))
        }).map_err(|e|e.at(Db::AgentHistoryEvidence,vault_store::OpenStage::Read))
    }
}
