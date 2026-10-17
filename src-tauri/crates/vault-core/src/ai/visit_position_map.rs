//! Responsibilities: map each visit to its vector's int8 plane position, bound to the projections
//! it was built from.
//! Not responsible for: eligibility, scoring or repairing source files during search.
//! Failure modes: positions from another projection attribute scores to the wrong page; a torn
//! file is read; a visit beyond the array is looked up. Tests: `ai/tests.rs`
//! `semantic_search_rejects_a_position_map_from_another_projection` and `ai/related_pages.rs`.
//!
//! ```text
//! .pkpos  magic b"PKPOS\0\x01\0" | u32 len + JSON Header | u32 position per history id (u32::MAX: none)
//! ```
//! Written after the reverse sidecars at the end of a pass. When it is missing or does not match the
//! current planes and forward sidecar, search builds the same array in memory and writes nothing.
//! The detail panel uses PointLookup instead and reports updating until the writer publishes it.
use super::{FlatVectorIndex, ReverseVisitMap, VisitContentMap};
use crate::{ProjectPaths, ai_sidecar::provider_table_name};
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fs::{self, File},
    io::{BufWriter, Read, Seek, SeekFrom, Write},
    path::PathBuf,
};
use vault_store::CancelToken;

const MAGIC: &[u8; 8] = b"PKPOS\0\x01\0";
pub(super) const ABSENT: u32 = u32::MAX;

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct Header {
    fingerprint_hash: String,
    int8_projection: String,
    int8_records: usize,
    forward_projection: String,
    forward_records: u64,
    positions: usize,
}

/// Detail-panel reads must never rebuild or copy a visit-sized array on a click.
pub(super) struct PointLookup {
    file: File,
    start: u64,
    positions: usize,
    records: usize,
    header: Header,
}

impl PointLookup {
    /// A missing or mismatched projection means updating, rather than an unindexed page.
    pub(super) fn open(
        paths: &ProjectPaths,
        provider: &str,
        model: &str,
        index: &FlatVectorIndex,
    ) -> Result<Option<Self>> {
        let read = || -> Result<Option<Self>> {
            let (mut expected, _) = source_header(paths, provider, model, index)?;
            if expected.int8_projection.is_empty() || expected.forward_projection.is_empty() {
                return Ok(None);
            }
            let mut file = File::open(path(paths, provider, model))?;
            let found = read_header(&mut file)?;
            expected.positions = found.positions;
            if found != expected {
                return Ok(None);
            }
            Ok(Some(Self {
                start: file.stream_position()?,
                positions: found.positions,
                records: index.len(),
                header: found,
                file,
            }))
        };
        // Writers replace these files separately; any incomplete header is transient unavailability.
        Ok(read().ok().flatten())
    }

    /// Bounds fallback lookups by the published visit prefix.
    pub(super) fn max_visit(&self) -> i64 {
        self.positions.saturating_sub(1) as i64
    }

    /// Reject a reverse-map replacement that raced candidate hydration.
    pub(super) fn is_current(
        &self,
        paths: &ProjectPaths,
        provider: &str,
        model: &str,
        index: &FlatVectorIndex,
    ) -> Result<bool> {
        let (mut current, _) = source_header(paths, provider, model, index)?;
        current.positions = self.positions;
        Ok(current == self.header)
    }

    /// Reads just the selected visit's u32 from the validated, pinned file.
    pub(super) fn get(&mut self, visit: i64) -> Result<Option<usize>> {
        let Ok(visit) = usize::try_from(visit) else { return Ok(None) };
        if visit >= self.positions {
            return Ok(None);
        }
        self.file.seek(SeekFrom::Start(self.start + visit as u64 * 4))?;
        let mut bytes = [0; 4];
        self.file.read_exact(&mut bytes)?;
        let position = u32::from_le_bytes(bytes);
        ensure!(position == ABSENT || (position as usize) < self.records, "invalid visit position");
        Ok((position != ABSENT).then_some(position as usize))
    }
}

pub(super) fn path(paths: &ProjectPaths, provider: &str, model: &str) -> PathBuf {
    paths.vectors_dir.join(format!("{}.pkpos", provider_table_name(provider, model)))
}

/// Missing or invalid caches cost a linear mapping pass, never incorrect attribution.
pub(super) fn load(
    paths: &ProjectPaths,
    provider: &str,
    model: &str,
    index: &FlatVectorIndex,
    cancel: &CancelToken,
) -> Result<Vec<u32>> {
    let (mut header, forward) = source_header(paths, provider, model, index)?;
    if !header.int8_projection.is_empty()
        && !header.forward_projection.is_empty()
        && let Ok((found, positions)) = read(&path(paths, provider, model))
    {
        header.positions = found.positions;
        if found == header && positions.iter().all(|p| *p == ABSENT || (*p as usize) < index.len())
        {
            return Ok(positions);
        }
    }
    build(paths, provider, model, index, forward, cancel)
}

fn source_header(
    paths: &ProjectPaths,
    provider: &str,
    model: &str,
    index: &FlatVectorIndex,
) -> Result<(Header, bool)> {
    let reverse = ReverseVisitMap::for_provider(paths, provider, model);
    let forward = reverse.projection_header(index.fingerprint_hash())?;
    let usable = forward.is_some();
    let (forward_projection, forward_records) = forward.unwrap_or_default();
    Ok((
        Header {
            fingerprint_hash: index.fingerprint_hash().into(),
            int8_projection: index.projection_id().into(),
            int8_records: index.len(),
            forward_projection,
            forward_records,
            positions: 0,
        },
        usable,
    ))
}

// Both writer and fallback use this last-writer-wins join, including keys not yet embedded.
fn build(
    paths: &ProjectPaths,
    provider: &str,
    model: &str,
    index: &FlatVectorIndex,
    forward: bool,
    cancel: &CancelToken,
) -> Result<Vec<u32>> {
    ensure!(index.len() < ABSENT as usize, "too many vectors for visit positions");
    let mut keys = HashMap::with_capacity(index.len());
    for position in 0..index.len() {
        if position % 1024 == 0 {
            cancel.check()?;
        }
        keys.insert(index.key(position), position as u32);
    }
    let mut positions = Vec::new();
    let mut insert = |id: i64, key: u64| -> Result<()> {
        cancel.check()?;
        let id = usize::try_from(id)?;
        if positions.len() <= id {
            positions.resize(id + 1, ABSENT);
        }
        positions[id] = keys.get(&key).copied().unwrap_or(ABSENT);
        Ok(())
    };
    if forward {
        ReverseVisitMap::for_provider(paths, provider, model).visit_forward(cancel, &mut insert)?;
    } else {
        for (id, key) in VisitContentMap::for_provider(paths, provider, model).read_all()? {
            insert(id, key)?;
        }
    }
    Ok(positions)
}

/// Projection writers publish the cache only after its complete payload has been flushed.
pub(super) fn rebuild(paths: &ProjectPaths, provider: &str, model: &str) -> Result<()> {
    let index = FlatVectorIndex::open(paths, provider, model)?;
    let (mut header, forward) = source_header(paths, provider, model, &index)?;
    let positions = build(paths, provider, model, &index, forward, &CancelToken::new())?;
    header.positions = positions.len();
    let destination = path(paths, provider, model);
    let temporary = destination.with_extension("pkpos.tmp");
    let mut writer = BufWriter::new(File::create(&temporary)?);
    let json = serde_json::to_vec(&header)?;
    writer.write_all(MAGIC)?;
    writer.write_all(&(u32::try_from(json.len())?).to_le_bytes())?;
    writer.write_all(&json)?;
    for position in positions {
        writer.write_all(&position.to_le_bytes())?;
    }
    writer.flush()?;
    drop(writer);
    fs::rename(temporary, destination)?;
    Ok(())
}

fn read(path: &std::path::Path) -> Result<(Header, Vec<u32>)> {
    let mut file = File::open(path)?;
    let header = read_header(&mut file)?;
    let mut bytes = vec![0; header.positions * 4];
    file.read_exact(&mut bytes)?;
    let positions =
        bytes.chunks_exact(4).map(|b| u32::from_le_bytes(b.try_into().unwrap())).collect();
    Ok((header, positions))
}

fn read_header(file: &mut File) -> Result<Header> {
    let mut prefix = [0; 12];
    file.read_exact(&mut prefix)?;
    ensure!(&prefix[..8] == MAGIC, "invalid visit position magic");
    let length = u32::from_le_bytes(prefix[8..].try_into().unwrap()) as usize;
    let file_bytes = file.metadata()?.len() as usize;
    ensure!(
        length <= 65_536 && length <= file_bytes.saturating_sub(12),
        "invalid visit position header"
    );
    let mut json = vec![0; length];
    file.read_exact(&mut json)?;
    let header: Header = serde_json::from_slice(&json)?;
    ensure!(
        header.positions.checked_mul(4).and_then(|n| n.checked_add(12 + length))
            == Some(file_bytes),
        "truncated visit positions"
    );
    Ok(header)
}

pub(super) fn delete(paths: &ProjectPaths, provider: &str, model: &str) -> Result<bool> {
    let path = path(paths, provider, model);
    if !path.exists() {
        return Ok(false);
    }
    fs::remove_file(path)?;
    Ok(true)
}
