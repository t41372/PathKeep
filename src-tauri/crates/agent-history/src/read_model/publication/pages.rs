//! Independently authenticated, length-delimited binary records in rank order.
//! Pages never interpret a native payload. Large metadata entries can span several bounded blobs.
use super::*;
use crate::Archive;
use serde::de::DeserializeOwned;
use sha2::{Digest, Sha256};
use std::io::Read;
use vault_store::{OptionalRow, Priority, params};

pub(super) const META_BYTES: usize = 64 * 1024;
pub(super) const TEXT_BYTES: usize = 256 * 1024;
const MAX_LOGICAL_PAGE: usize = 192 * 1024 * 1024;

pub(super) struct Buffer {
    family: i64,
    limit: usize,
    first: u64,
    count: usize,
    bytes: Vec<u8>,
}
impl Buffer {
    pub fn len(&self) -> u64 {
        self.first + self.count as u64
    }
    pub fn new(family: i64, limit: usize) -> Self {
        Self { family, limit, first: 0, count: 0, bytes: Vec::new() }
    }
    pub fn push<T: Serialize>(&mut self, a: &Archive, owner: &str, value: &T) -> Result<()> {
        let data = postcard::to_allocvec(value).map_err(invalid)?;
        if data.len() + 4 > MAX_LOGICAL_PAGE {
            return Err(Error::LimitExceeded("reader entry".into()));
        }
        if self.count == 256 || self.bytes.len() + 4 + data.len() > self.limit {
            self.flush(a, owner)?
        }
        self.bytes.extend_from_slice(&(data.len() as u32).to_le_bytes());
        self.bytes.extend(data);
        self.count += 1;
        Ok(())
    }
    pub fn flush(&mut self, a: &Archive, owner: &str) -> Result<()> {
        if self.count == 0 {
            return Ok(());
        }
        let mut pieces = Vec::new();
        for raw in self.bytes.chunks(self.limit) {
            pieces.push((
                raw.len() as i64,
                Sha256::digest(raw).to_vec(),
                zstd::stream::encode_all(raw, 3)?,
            ));
        }
        let owner = owner.to_string();
        let family = self.family;
        let first = self.first as i64;
        a.store.write(DB, Priority::Batch, move |tx| {
            super::build::require_staging(tx, &owner)?;
            // A damaged compressed payload can survive under its original key.
            // Repair that payload without rewriting healthy shared blobs.
            let mut blobs = tx.prepare(
                "INSERT INTO ah_reader_blob(hash,data) VALUES(?1,?2) ON CONFLICT(hash)
                 DO UPDATE SET data=excluded.data WHERE ah_reader_blob.data<>excluded.data",
            )?;
            let mut insert =
                tx.prepare("INSERT INTO ah_reader_page VALUES(?1,?2,?3,?4,?5,?6,?7)")?;
            for (part, (len, hash, data)) in pieces.into_iter().enumerate() {
                let binding = binding(&owner, family, first, part as i64, len, &hash);
                blobs.execute(params![hash.clone(), data])?;
                insert.execute(params![
                    owner.clone(),
                    family,
                    first,
                    part as i64,
                    len,
                    hash,
                    binding
                ])?;
            }
            Ok(())
        })?;
        self.first += self.count as u64;
        self.count = 0;
        self.bytes.clear();
        Ok(())
    }
}

pub(super) fn binding(
    owner: &str,
    family: i64,
    first: i64,
    part: i64,
    len: i64,
    hash: &[u8],
) -> Vec<u8> {
    let mut sum = Sha256::new();
    sum.update(owner.as_bytes());
    for n in [family, first, part, len] {
        sum.update(n.to_le_bytes())
    }
    sum.update(hash);
    sum.finalize().to_vec()
}

pub(super) fn invalid(e: impl std::fmt::Display) -> Error {
    Error::Integrity(format!("invalid reader page: {e}"))
}
pub(super) fn load<T: DeserializeOwned>(
    a: &Archive,
    owner: &str,
    family: i64,
    rank: u64,
    job: &JobControl,
) -> Result<(u64, Vec<T>)> {
    let key = owner.to_owned();
    let data = a.store.read(DB, move |tx| fetch(tx, key, family, rank))?;
    decode(data, owner, family, rank, job)
}

type Pieces = (Option<i64>, Vec<(i64, i64, Vec<u8>, Vec<u8>, Vec<u8>)>);
fn fetch(
    tx: &vault_store::Session<'_>,
    key: String,
    family: i64,
    rank: u64,
) -> vault_store::Result<Pieces> {
    let first = tx
        .query_one(
            "SELECT first_rank FROM ah_reader_page WHERE owner=?1 AND family=?2 AND
                 first_rank<=?3 ORDER BY first_rank DESC LIMIT 1",
            params![key.clone(), family, rank as i64],
            |r| Ok(r.get::<i64>(0)?),
        )
        .optional()?;
    // Check metadata before retrieving BLOBs, even when an imported directory is damaged.
    if let Some(first) = first {
        let (n, bytes) = tx.query_one(
            "SELECT count(*),sum(nbytes) FROM ah_reader_page
                WHERE owner=?1 AND family=?2 AND first_rank=?3",
            params![key.clone(), family, first],
            |r| Ok((r.get::<i64>(0)?, r.get::<i64>(1)?)),
        )?;
        if first < 0
            || n > MAX_LOGICAL_PAGE.div_ceil(META_BYTES) as i64
            || !(1..=MAX_LOGICAL_PAGE as i64).contains(&bytes)
        {
            return Ok((None, Vec::new()));
        }
    }
    let pieces = match first {
            Some(first) => tx.query(
                "SELECT p.part,p.nbytes,p.hash,p.binding,b.data FROM ah_reader_page p JOIN
                 ah_reader_blob b ON b.hash=p.hash WHERE p.owner=?1 AND p.family=?2 AND
                 p.first_rank=?3 AND p.nbytes BETWEEN 1 AND ?4 AND length(b.data)<=2*p.nbytes+256 ORDER BY p.part",
                params![key, family, first,TEXT_BYTES as i64],
                |r| {
                    Ok((
                        r.get::<i64>(0)?,
                        r.get::<i64>(1)?,
                        r.get::<Vec<u8>>(2)?,
                        r.get::<Vec<u8>>(3)?,
                        r.get::<Vec<u8>>(4)?,
                    ))
                },
            )?,
            None => Vec::new(),
        };
    Ok((first, pieces))
}

pub(super) fn in_snapshot<T: DeserializeOwned>(
    tx: &vault_store::Session<'_>,
    owner: &str,
    family: i64,
    rank: u64,
) -> vault_store::Result<(u64, Vec<T>)> {
    let data = fetch(tx, owner.to_string(), family, rank)?;
    decode(data, owner, family, rank, &JobControl::default()).map_err(crate::query::operation)
}
fn decode<T: DeserializeOwned>(
    (first, pieces): Pieces,
    owner: &str,
    family: i64,
    rank: u64,
    job: &JobControl,
) -> Result<(u64, Vec<T>)> {
    let first = first.ok_or_else(|| invalid("missing rank"))? as u64;
    let mut bytes = Vec::new();
    for (index, (part, len, hash, stamp, blob)) in pieces.into_iter().enumerate() {
        job.cancel.check().map_err(|_| Error::Cancelled)?;
        if stamp != binding(owner, family, first as i64, part, len, &hash) {
            return Err(invalid("stale page binding"));
        }
        if part != index as i64
            || !(1..=TEXT_BYTES as i64).contains(&len)
            || bytes.len() + len as usize > MAX_LOGICAL_PAGE
        {
            return Err(invalid("piece length"));
        }
        let start = bytes.len();
        let mut decoder = zstd::stream::Decoder::new(blob.as_slice()).map_err(invalid)?;
        decoder.window_log_max(23).map_err(invalid)?;
        decoder.take(len as u64 + 1).read_to_end(&mut bytes).map_err(invalid)?;
        if bytes.len() - start != len as usize || Sha256::digest(&bytes[start..])[..] != hash {
            return Err(invalid("checksum"));
        }
    }
    let mut input = &bytes[..];
    let mut rows = Vec::new();
    while !input.is_empty() {
        if rows.len() == 256 {
            return Err(invalid("entry count"));
        }
        let (len, rest) = input.split_at_checked(4).ok_or_else(|| invalid("length prefix"))?;
        let length = u32::from_le_bytes(len.try_into().unwrap()) as usize;
        let (entry, rest) = rest.split_at_checked(length).ok_or_else(|| invalid("short entry"))?;
        rows.push(postcard::from_bytes(entry).map_err(invalid)?);
        input = rest;
    }
    if rank - first >= rows.len() as u64 {
        return Err(invalid("rank gap"));
    }
    Ok((first, rows))
}
