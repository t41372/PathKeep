//! Derived recall/rescore planes projected from the `.pkvec` f32 source (W-AI-5, 05 §3).
//!
//! ## Responsibilities
//! - project the lossless f32 [`super::vector_store::VectorStore`] (`.pkvec`) into the two
//!   query-time planes the flat [`super::vector_index::FlatVectorIndex`] reads:
//!   - a **binary plane** (`.pkbin`): one sign-bit per dim (bit = 1 iff component ≥ 0), packed into
//!     `ceil(dim/8)` bytes/vector, loaded RAM-resident for SIMD-friendly Hamming recall;
//!   - an **int8 plane** (`.pki8`): per-vector symmetric max-abs quantization to `[-127, 127]` plus
//!     the f32 scale, kept ON DISK and read by seek-by-position (NOT resident) for high-fidelity
//!     rescore of only the recalled candidates (C1/C2, 05 §10) — so resident RAM = the binary plane.
//! - own the pure quantization/binarization math (sign-bit pack, int8 quantize/dequantize) so the
//!   error bounds are unit-tested directly, NOT only through the index.
//! - stamp each plane with the SAME [`EmbeddingFingerprint`] as the `.pkvec` source so a model/dim
//!   change marks the planes stale and they are rebuilt (zero training — pure re-projection).
//!
//! ## Not responsible for
//! - nearest-neighbour ranking (the [`super::vector_index`] owns recall + rescore + allowlist);
//! - producing the f32 vectors (the embed loop + `.pkvec` store own that — these planes are a
//!   downstream projection, never a source of truth);
//! - export policy (both planes live under `derived/vectors/` and are excluded from the bundle like
//!   `.pkvec`/`.pkmap`, see `migration.rs`).
//!
//! ## Why two derived planes instead of querying f32 directly
//! At the 14.4M tail the raw f32 plane is ~14.7 GB @256-dim — too large to hold resident and too slow
//! to brute-force per query (05 §3). The binary plane packs 32 B/vec (~460 MB packed; ~0.9 GB once the
//! Rust `(u64, Vec<u8>)` resident overhead is counted, S2 §10) so it fits in RAM for a SIMD
//! popcount(xor) Hamming sweep. The int8 plane (~3.7 GB @256-dim, 260 B/vec incl. scale) stays ON DISK
//! and is read by seek-by-position for only the few thousand recalled candidates — so resident RAM is
//! binary-only and the 8 GB envelope holds (C1/C2, 05 §10). The f32 `.pkvec` stays the lossless rebuild
//! SOURCE (never deleted); whether f32 is ALSO used as a final exact tier is the S2 benchmark's call.
//!
//! ## Format (little-endian; both planes mirror the `.pkvec` magic+header discipline)
//! ```text
//! .pkbin  magic 8 = b"PKBIN\0\x01\0" | u32 len + JSON PlaneHeader | records [ key:u64 | bits: ceil(dim/8) B ]
//! .pki8   magic 8 = b"PKI8\0\0\x01\0" | u32 len + JSON PlaneHeader | records [ key:u64 | scale:f32 | codes: dim i8 ]
//! ```
//! Fixed-width records keyed by the same `content_key` u64 the `.pkvec` uses, so both planes stride
//! cleanly and a reader pairs a key to its row without a side index.

use crate::ai::EmbeddingFingerprint;
use crate::ai::vector_store::VectorStore;
use crate::ai_sidecar::provider_table_name;
use crate::config::{ProjectPaths, ensure_paths};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs::{self, File};
use std::io::{BufReader, BufWriter, Read, Seek, Write};
use std::path::{Path, PathBuf};

/// Magic + format-version prefix for a binary recall plane file.
const BINARY_PLANE_MAGIC: [u8; 8] = *b"PKBIN\0\x01\0";
/// Magic + format-version prefix for an int8 rescore plane file.
const INT8_PLANE_MAGIC: [u8; 8] = *b"PKI8\0\0\x01\0";

/// File extension for the binary recall plane (one per provider/model).
const BINARY_PLANE_EXTENSION: &str = "pkbin";
/// File extension for the int8 rescore plane (one per provider/model).
const INT8_PLANE_EXTENSION: &str = "pki8";

/// JSON header persisted after each plane's magic; ties the plane to its source fingerprint + dim.
///
/// `dim` is duplicated as a first-class field (like the `.pkvec` header) because the reader needs the
/// record stride BEFORE trusting the hash, and a dim mismatch must fail loudly rather than misalign
/// records. `fingerprint_hash` is the [`EmbeddingFingerprint::hash`] of the `.pkvec` source so a
/// staleness check is a string compare.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaneHeader {
    /// Effective vector dimension this plane was projected at (the record stride driver).
    pub dim: usize,
    /// The embedding fingerprint hash the source `.pkvec` carried when this plane was projected.
    pub fingerprint_hash: String,
}

/// Number of bytes one binarized vector occupies: one sign bit per dim, packed.
///
/// `ceil(dim / 8)` — at 256 dim this is the 32 B/vec the RAM budget (460 MB @14.4M) is sized for.
pub fn binary_bytes_for_dim(dim: usize) -> usize {
    dim.div_ceil(8)
}

/// Binarizes one f32 vector into packed sign bits (bit = 1 iff component ≥ 0).
///
/// PURE. Bit `i` lives in byte `i / 8` at position `i % 8` (LSB-first within a byte). A component of
/// exactly 0.0 maps to 1 (the `≥ 0` rule, 05 §3) so a deterministic boundary value never flips with
/// floating noise. Trailing bits of the final byte (when `dim` is not a multiple of 8) stay 0, which
/// is the same on both the indexed vector and the binarized query, so they cancel in the xor and
/// never bias the Hamming distance.
pub fn binarize(vector: &[f32]) -> Vec<u8> {
    let mut bits = vec![0u8; binary_bytes_for_dim(vector.len())];
    for (index, &component) in vector.iter().enumerate() {
        if component >= 0.0 {
            bits[index / 8] |= 1 << (index % 8);
        }
    }
    bits
}

/// Hamming distance between two equal-length packed bit vectors, via SIMD-friendly popcount.
///
/// PURE. Chunks the bytes into `u64` words and `count_ones`-pops the xor: the compiler vectorizes
/// `u64::count_ones` over a contiguous slice into a hardware popcount sweep (the 14.4M-vector recall
/// hot path). A ragged tail (`len % 8 != 0`) is popped byte-by-byte. Caller guarantees equal length
/// (both come from the same `dim`); a length mismatch counts only the shared prefix, which never
/// happens on the index path but keeps the function total.
pub fn hamming_distance(left: &[u8], right: &[u8]) -> u32 {
    // Equal length is the contract on every production call (both sides share `dim`); a future caller
    // that prefix-compares a dim-mismatched pair would silently produce meaningless distances (the D1
    // class of bug), so trip a debug assertion. Release builds stay total via the `min` below.
    debug_assert_eq!(left.len(), right.len(), "hamming_distance requires equal-length bit vectors");
    let len = left.len().min(right.len());
    let mut distance = 0u32;
    let mut offset = 0;
    while offset + 8 <= len {
        let a = u64::from_le_bytes(left[offset..offset + 8].try_into().expect("8-byte word"));
        let b = u64::from_le_bytes(right[offset..offset + 8].try_into().expect("8-byte word"));
        distance += (a ^ b).count_ones();
        offset += 8;
    }
    while offset < len {
        distance += (left[offset] ^ right[offset]).count_ones();
        offset += 1;
    }
    distance
}

/// One int8-quantized vector: a per-vector f32 scale plus the `i8` codes.
///
/// The scale dequantizes a code back to its approximate f32: `f ≈ code * scale`. Symmetric max-abs
/// quantization keeps zero at zero (no asymmetric offset to store) and bounds the per-component error
/// to `scale / 2`.
#[derive(Debug, Clone, PartialEq)]
pub struct Int8Vector {
    /// Per-vector dequantization scale (`f ≈ code * scale`); `0.0` for an all-zero source vector.
    pub scale: f32,
    /// Signed 8-bit codes in `[-127, 127]`, one per dim.
    pub codes: Vec<i8>,
}

/// Quantizes one f32 vector with per-vector symmetric max-abs scaling to `[-127, 127]`.
///
/// PURE. `scale = max|component| / 127`; each code is `round(component / scale)` clamped to
/// `[-127, 127]` (127, not 128, keeps the range symmetric so `-x` and `x` quantize to opposite codes).
/// An all-zero vector (max-abs 0) yields `scale = 0.0` and all-zero codes — dequantizing it returns
/// zeros, the exact original. `+128` is deliberately excluded so no code overflows `i8`.
pub fn quantize_int8(vector: &[f32]) -> Int8Vector {
    let max_abs = vector.iter().fold(0.0f32, |acc, &value| acc.max(value.abs()));
    if max_abs == 0.0 {
        return Int8Vector { scale: 0.0, codes: vec![0i8; vector.len()] };
    }
    let scale = max_abs / 127.0;
    let codes =
        vector.iter().map(|&value| (value / scale).round().clamp(-127.0, 127.0) as i8).collect();
    Int8Vector { scale, codes }
}

/// Dequantizes an int8 vector back to approximate f32 (`f ≈ code * scale`).
///
/// PURE. Inverse of [`quantize_int8`]; the per-component error is bounded by `scale / 2` (the rounding
/// step). Used by the rescore stage to recover enough fidelity to re-rank the binary-recall candidates.
pub fn dequantize_int8(vector: &Int8Vector) -> Vec<f32> {
    vector.codes.iter().map(|&code| code as f32 * vector.scale).collect()
}

/// Dot product of two equal-length f32 vectors (the rescore similarity).
///
/// PURE. The stored vectors are L2-normalized by the embed path (the provider normalizes
/// defensively), so a dot product IS cosine similarity — no per-query renormalization needed. A
/// length mismatch dots only the shared prefix, keeping the function total (never hit on the index
/// path, where both sides share `dim`).
pub fn dot_product(left: &[f32], right: &[f32]) -> f32 {
    // Equal length is the contract on every production call (both sides share `dim`); a future caller
    // that prefix-dots a dim-mismatched pair would silently produce meaningless scores (the D1 class of
    // bug), so trip a debug assertion. Release builds stay total via the `min` below.
    debug_assert_eq!(left.len(), right.len(), "dot_product requires equal-length vectors");
    let len = left.len().min(right.len());
    let mut sum = 0.0f32;
    for index in 0..len {
        sum += left[index] * right[index];
    }
    sum
}

/// Resolves the binary recall plane path for one provider/model pair under `derived/vectors/`.
fn binary_plane_path(paths: &ProjectPaths, provider_id: &str, model: &str) -> PathBuf {
    let file = format!("{}.{BINARY_PLANE_EXTENSION}", provider_table_name(provider_id, model));
    paths.vectors_dir.join(file)
}

/// Resolves the int8 rescore plane path for one provider/model pair under `derived/vectors/`.
fn int8_plane_path(paths: &ProjectPaths, provider_id: &str, model: &str) -> PathBuf {
    let file = format!("{}.{INT8_PLANE_EXTENSION}", provider_table_name(provider_id, model));
    paths.vectors_dir.join(file)
}

/// One binary recall plane on the `derived/vectors/` plane (RAM-resident at query time).
///
/// Cheap to construct (resolves the path only); the I/O is in explicit methods so the projection step
/// and the index loader stay separate concerns.
pub struct BinaryPlane {
    path: PathBuf,
}

impl BinaryPlane {
    /// Resolves the binary plane handle for one provider/model pair.
    pub fn for_provider(paths: &ProjectPaths, provider_id: &str, model: &str) -> Self {
        Self { path: binary_plane_path(paths, provider_id, model) }
    }

    /// Returns the on-disk path of this plane (used by storage-size reporting + the index loader).
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Returns whether the plane file exists on disk.
    pub fn exists(&self) -> bool {
        self.path.exists()
    }

    /// Reads the persisted header without reading any records (`None` when never built).
    pub fn read_header(&self) -> Result<Option<PlaneHeader>> {
        read_plane_header(&self.path, &BINARY_PLANE_MAGIC)
    }

    /// Loads every `(content_key, packed_bits)` record into RAM for the recall sweep.
    ///
    /// The whole binary plane is resident by design (460 MB @14.4M/256-dim, within the 8 GB envelope);
    /// the int8 plane is the one that stays on disk. A torn trailing record errors (mirroring the
    /// `.pkvec` reader) so a half-written projection is caught, not misread.
    pub fn read_all(&self) -> Result<Vec<(u64, Vec<u8>)>> {
        let (header, mut reader) = open_plane(&self.path, &BINARY_PLANE_MAGIC)?;
        let record_len = 8 + binary_bytes_for_dim(header.dim);
        let raw = read_records(&mut reader, record_len, &self.path)?;
        // `read_records` returns the FULL record (leading 8-byte key + body); slice off the key so a
        // caller gets just the packed sign bits keyed by the u64.
        Ok(raw.into_iter().map(|(key, bytes)| (key, bytes[8..].to_vec())).collect())
    }
}

/// One int8 rescore plane on the `derived/vectors/` plane — ON-DISK, read by position per candidate.
///
/// NOT resident (C1/C2, 05 §10): the int8 plane stays on disk and the rescore stage seeks each
/// recalled candidate's record by its position via [`Int8Plane::reader`] + [`Int8PlaneReader::record_at`].
/// Only the binary recall plane is held in RAM (~0.9 GB @14.4M/256-dim incl. Rust overhead); pulling
/// the few thousand recalled int8 records per query off disk is cheap vs the O(n) binary sweep and warms
/// the OS page cache, so the "≤8 GB envelope, binary-only resident" claim is TRUE as the doc states. The
/// per-record seek offset is `header_len + position * int8_record_stride` (fixed width: 8-byte key +
/// 4-byte scale + `dim` i8 codes), and `position` is the SAME index the binary recall returned — the
/// two planes are positionally aligned by the lockstep projection ([`write_planes_atomic`]).
pub struct Int8Plane {
    path: PathBuf,
}

impl Int8Plane {
    /// Resolves the int8 plane handle for one provider/model pair.
    pub fn for_provider(paths: &ProjectPaths, provider_id: &str, model: &str) -> Self {
        Self { path: int8_plane_path(paths, provider_id, model) }
    }

    /// Returns the on-disk path of this plane.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Returns whether the plane file exists on disk.
    pub fn exists(&self) -> bool {
        self.path.exists()
    }

    /// Reads the persisted header without reading any records (`None` when never built).
    pub fn read_header(&self) -> Result<Option<PlaneHeader>> {
        read_plane_header(&self.path, &INT8_PLANE_MAGIC)
    }

    /// Opens a seekable reader over the int8 plane for position-addressed candidate rescore (C1).
    ///
    /// The query path opens this ONCE per search and calls [`Int8PlaneReader::record_at`] for each
    /// recalled candidate position, so the int8 plane is never held resident. Returns the header (for
    /// the record stride + dim) bundled with the open file handle. Errors when the plane is absent —
    /// the caller never reaches here for a never-built index (that path is gated on the binary plane
    /// being empty).
    pub fn reader(&self) -> Result<Int8PlaneReader> {
        let header = read_plane_header(&self.path, &INT8_PLANE_MAGIC)?
            .with_context(|| format!("int8 plane {} is absent", self.path.display()))?;
        let file = File::open(&self.path)
            .with_context(|| format!("opening int8 plane {}", self.path.display()))?;
        // Bytes consumed by magic + length-prefixed JSON header = the first record's file offset.
        let data_start = 8 + 4 + serde_json::to_vec(&header)?.len() as u64;
        let record_len = (8 + 4 + header.dim) as u64; // key + f32 scale + dim i8 codes
        Ok(Int8PlaneReader { file, data_start, record_len, dim: header.dim })
    }

    /// Loads every `(content_key, Int8Vector)` record (TEST/diagnostic only — never the query path).
    ///
    /// Retained so the projection round-trip + alignment can be asserted in one read; the query path
    /// uses [`Int8Plane::reader`] seek-by-position instead so the plane is never held resident (C1).
    #[cfg(test)]
    pub fn read_all(&self) -> Result<Vec<(u64, Int8Vector)>> {
        let (header, mut reader) = open_plane(&self.path, &INT8_PLANE_MAGIC)?;
        let record_len = 8 + 4 + header.dim; // key + f32 scale + dim i8 codes
        let raw = read_records(&mut reader, record_len, &self.path)?;
        let mut out = Vec::with_capacity(raw.len());
        for (key, bytes) in raw {
            let scale = f32::from_le_bytes(bytes[8..12].try_into().expect("4-byte scale"));
            let codes = bytes[12..].iter().map(|&byte| byte as i8).collect();
            out.push((key, Int8Vector { scale, codes }));
        }
        Ok(out)
    }
}

/// A seekable, on-disk reader over one int8 rescore plane (resident = nothing but this handle, C1).
///
/// Holds the open file plus the fixed geometry (data start, record stride, dim) so the rescore stage
/// can read any candidate's int8 record by its POSITION (the index the binary recall returned) without
/// loading the plane. Reading record `position` seeks to `data_start + position * record_len` and reads
/// exactly one fixed-width record — O(1) per candidate, a few thousand per query.
pub struct Int8PlaneReader {
    file: File,
    data_start: u64,
    record_len: u64,
    dim: usize,
}

impl Int8PlaneReader {
    /// Reads the `(content_key, Int8Vector)` record at the given position (binary-recall position).
    ///
    /// The position MUST be the index the binary recall returned: the lockstep projection guarantees
    /// int8 position i is the SAME content_key as binary position i, so this recovers exactly the
    /// candidate's codes. Returns the key too so the caller can assert the alignment in tests and key
    /// the rescore result without a second lookup.
    pub fn record_at(&mut self, position: usize) -> Result<(u64, Int8Vector)> {
        let offset = self.data_start + position as u64 * self.record_len;
        self.file.seek(std::io::SeekFrom::Start(offset)).context("seeking int8 plane record")?;
        let mut record = vec![0u8; self.record_len as usize];
        self.file.read_exact(&mut record).context("reading int8 plane record")?;
        let key = u64::from_le_bytes(record[..8].try_into().expect("8-byte key"));
        let scale = f32::from_le_bytes(record[8..12].try_into().expect("4-byte scale"));
        let codes = record[12..].iter().map(|&byte| byte as i8).collect();
        Ok((key, Int8Vector { scale, codes }))
    }

    /// Returns the effective dim of the records in this plane (the int8 code count per record).
    pub fn dim(&self) -> usize {
        self.dim
    }
}

/// Result of a plane (re)projection pass: how many vectors were projected and the dim observed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PlaneBuildReport {
    /// Distinct content keys projected into both planes.
    pub vectors: usize,
    /// The effective dim the planes were projected at (`None` when the source had no vectors).
    pub dim: Option<usize>,
}

/// Whether the derived planes are stale against the live `.pkvec` source (need (re)projection).
///
/// Stale when EITHER plane is missing OR a plane's stamped fingerprint hash differs from `live`. This
/// is the single seam the index build hooks rebuild into: the planes are pure projections, so a
/// fingerprint change (model/dim/dtype) just re-projects from the f32 source — zero training.
pub fn planes_are_stale(
    paths: &ProjectPaths,
    provider_id: &str,
    model: &str,
    live: &EmbeddingFingerprint,
) -> Result<bool> {
    let binary = BinaryPlane::for_provider(paths, provider_id, model);
    let int8 = Int8Plane::for_provider(paths, provider_id, model);
    let live_hash = live.hash();
    for header in [binary.read_header()?, int8.read_header()?] {
        match header {
            None => return Ok(true),
            Some(header) if header.fingerprint_hash != live_hash => return Ok(true),
            Some(_) => {}
        }
    }
    Ok(false)
}

/// Projects the f32 `.pkvec` source into the binary + int8 planes for one provider/model (05 §3).
///
/// STREAMING + LOCKSTEP (C2, 05 §10): strides the deduped `(content_key, f32)` SET out of the
/// [`VectorStore`] one record at a time via [`VectorStore::read_records_streaming`] (which preserves
/// the last-writer-wins SET contract), binarizing + int8-quantizing each vector straight into BOTH
/// plane writers in lockstep, never materializing the full binary/int8 Vecs. Peak build RAM is two
/// write buffers + one f32 record + the streamer's dedup key-set (well under 1 GB at any scale) — the
/// old `read_all` path held the entire f32 SET (~14.7 GB @14.4M/256) plus a full binary Vec plus a
/// full int8 Vec resident (~19 GB peak), blowing the 8 GB envelope (Principle 3).
///
/// **LOAD-BEARING INVARIANT — binary position i ↔ int8 position i ↔ same content_key.** Both writers
/// are advanced by the SAME streaming callback, in the SAME order, so the n-th binary record and the
/// n-th int8 record are the SAME content_key. The query path's int8 seek-by-position
/// ([`Int8Plane::read_record_at`]) relies on this: it reads the int8 record at the position the binary
/// recall returned, so a single stream emitting one binary + one int8 record per source key is what
/// keeps the two planes aligned. Do NOT reorder, filter, or skip one writer without the other.
///
/// Both planes are written atomically (temp file + rename) so an interrupted projection never leaves a
/// torn plane the loader would reject, and both are stamped with the SOURCE store's fingerprint hash so
/// [`planes_are_stale`] flips false right after. Returns the count + dim. A missing/empty `.pkvec`
/// writes empty (header-only) planes so the index loads cleanly to "no results" rather than erroring.
pub fn build_planes_from_store(
    paths: &ProjectPaths,
    provider_id: &str,
    model: &str,
) -> Result<PlaneBuildReport> {
    ensure_paths(paths)?;
    let store = VectorStore::for_provider(paths, provider_id, model);
    let Some(header) = store.read_header()? else {
        // No source store yet: write empty, dim-0 planes under an empty fingerprint so the loader
        // sees "present but empty" and search returns no semantic hits with an honest note.
        write_planes_atomic(paths, provider_id, model, 0, "", |_| Ok(()))?;
        return Ok(PlaneBuildReport::default());
    };
    let mut vectors = 0usize;
    write_planes_atomic(
        paths,
        provider_id,
        model,
        header.dim,
        &header.fingerprint_hash,
        |project| {
            store.read_records_streaming(|key, vector| {
                vectors += 1;
                project(key, vector)
            })
        },
    )?;
    Ok(PlaneBuildReport { vectors, dim: (vectors != 0).then_some(header.dim) })
}

/// Deletes both derived planes for one provider/model, returning whether anything was removed.
///
/// Used by the index "clear" path; the planes are pure projections so a clean delete is the correct
/// reset (a later build re-projects from the surviving `.pkvec`).
pub fn delete_planes(paths: &ProjectPaths, provider_id: &str, model: &str) -> Result<bool> {
    let mut removed = false;
    for path in
        [binary_plane_path(paths, provider_id, model), int8_plane_path(paths, provider_id, model)]
    {
        if path.exists() {
            fs::remove_file(&path)
                .with_context(|| format!("removing derived plane {}", path.display()))?;
            removed = true;
        }
    }
    Ok(removed)
}

/// Reports total bytes consumed by all derived recall/rescore planes under the vector plane dir.
///
/// Filesystem-only so storage review can size the derived planes without loading any engine.
pub fn derived_plane_bytes(paths: &ProjectPaths) -> u64 {
    let Ok(entries) = fs::read_dir(&paths.vectors_dir) else {
        return 0;
    };
    entries
        .flatten()
        .filter(|entry| {
            matches!(
                entry.path().extension().and_then(|ext| ext.to_str()),
                Some(BINARY_PLANE_EXTENSION) | Some(INT8_PLANE_EXTENSION)
            )
        })
        .filter_map(|entry| entry.metadata().ok().map(|meta| meta.len()))
        .sum()
}

/// Writes BOTH derived planes atomically + in lockstep from one streaming projection (C2, 05 §10).
///
/// Opens the binary + int8 temp files, writes both magic + JSON headers, then invokes `project` with a
/// single closure that — per source `(content_key, f32)` — appends ONE binary record and ONE int8
/// record, advancing both writers together so binary position i and int8 position i are the SAME key
/// (the load-bearing alignment invariant the int8 seek-by-position depends on). After `project`
/// returns, both temp files are flushed and renamed over their destinations, so a reader never sees a
/// partially-written or half-aligned plane. Streaming the projection here (rather than buffering full
/// binary/int8 Vecs) is what keeps build peak RAM bounded at 14.4M (Principle 3).
///
/// `project` receives the per-record writer closure and is expected to drive the source stream through
/// it exactly once per content_key in projection order. An empty `.pkvec` (the no-source case) passes a
/// `project` that emits nothing, yielding header-only planes.
fn write_planes_atomic(
    paths: &ProjectPaths,
    provider_id: &str,
    model: &str,
    dim: usize,
    fingerprint_hash: &str,
    project: impl FnOnce(&mut dyn FnMut(u64, &[f32]) -> Result<()>) -> Result<()>,
) -> Result<()> {
    let binary_path = binary_plane_path(paths, provider_id, model);
    let int8_path = int8_plane_path(paths, provider_id, model);
    // The two planes share the same base file name and differ only by extension, so a plain
    // `.with_extension("tmp")` would collide on ONE temp path; keep the original extension and append
    // `.tmp` so each plane gets a distinct, atomic-rename temp (`<name>.pkbin.tmp` / `<name>.pki8.tmp`).
    let binary_tmp = plane_tmp_path(&binary_path);
    let int8_tmp = plane_tmp_path(&int8_path);
    let header = PlaneHeader { dim, fingerprint_hash: fingerprint_hash.to_string() };
    {
        let mut binary_writer = open_plane_writer(&binary_tmp, &BINARY_PLANE_MAGIC, &header)?;
        let mut int8_writer = open_plane_writer(&int8_tmp, &INT8_PLANE_MAGIC, &header)?;
        // The lockstep sink: binarize + int8-quantize the SAME f32 record into both writers in the
        // SAME order, so the planes stay positionally aligned by content_key (C1/C2 invariant).
        let mut record_buf: Vec<u8> = Vec::new();
        let mut sink = |key: u64, vector: &[f32]| -> Result<()> {
            let bits = binarize(vector);
            record_buf.clear();
            record_buf.extend_from_slice(&key.to_le_bytes());
            record_buf.extend_from_slice(&bits);
            binary_writer.write_all(&record_buf).context("writing binary plane record")?;

            let int8 = quantize_int8(vector);
            record_buf.clear();
            record_buf.extend_from_slice(&key.to_le_bytes());
            record_buf.extend_from_slice(&int8.scale.to_le_bytes());
            record_buf.extend(int8.codes.iter().map(|&code| code as u8));
            int8_writer.write_all(&record_buf).context("writing int8 plane record")?;
            Ok(())
        };
        project(&mut sink)?;
        binary_writer.flush().context("flushing binary plane")?;
        int8_writer.flush().context("flushing int8 plane")?;
    }
    fs::rename(&binary_tmp, &binary_path)
        .with_context(|| format!("finalizing derived plane {}", binary_path.display()))?;
    fs::rename(&int8_tmp, &int8_path)
        .with_context(|| format!("finalizing derived plane {}", int8_path.display()))?;
    Ok(())
}

/// Resolves the per-plane temp path by appending `.tmp` to the FULL file name (extension included).
///
/// Both planes share a base name and differ only by extension, so appending (rather than replacing
/// the extension) keeps each temp distinct: `<name>.pkbin` → `<name>.pkbin.tmp`, `<name>.pki8` →
/// `<name>.pki8.tmp`. `derived_plane_bytes` ignores `.tmp` (it only sums `pkbin`/`pki8`), so a stray
/// temp never inflates the storage report.
fn plane_tmp_path(path: &Path) -> PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(".tmp");
    path.with_file_name(name)
}

/// Creates a plane temp file and writes its magic + length-prefixed JSON header, returning the writer
/// positioned at the first record.
fn open_plane_writer(tmp: &Path, magic: &[u8; 8], header: &PlaneHeader) -> Result<BufWriter<File>> {
    let file = File::create(tmp)
        .with_context(|| format!("creating derived plane temp {}", tmp.display()))?;
    let mut writer = BufWriter::new(file);
    writer.write_all(magic).context("writing derived plane magic")?;
    let json = serde_json::to_vec(header).context("serializing derived plane header")?;
    let len = u32::try_from(json.len()).context("derived plane header too large")?;
    writer.write_all(&len.to_le_bytes()).context("writing derived plane header length")?;
    writer.write_all(&json).context("writing derived plane header body")?;
    Ok(writer)
}

/// Reads + validates a plane's magic + JSON header, returning `None` when the file is absent.
fn read_plane_header(path: &Path, magic: &[u8; 8]) -> Result<Option<PlaneHeader>> {
    if !path.exists() {
        return Ok(None);
    }
    let (header, _) = open_plane(path, magic)?;
    Ok(Some(header))
}

/// Opens a plane file, validates its magic, and returns its header + a reader positioned at the
/// first record.
fn open_plane(path: &Path, magic: &[u8; 8]) -> Result<(PlaneHeader, BufReader<File>)> {
    let file =
        File::open(path).with_context(|| format!("opening derived plane {}", path.display()))?;
    let mut reader = BufReader::new(file);
    let mut found = [0u8; 8];
    reader.read_exact(&mut found).context("reading derived plane magic")?;
    if &found != magic {
        anyhow::bail!(
            "derived plane {} has an unrecognized magic/format-version header",
            path.display()
        );
    }
    let mut len_bytes = [0u8; 4];
    reader.read_exact(&mut len_bytes).context("reading derived plane header length")?;
    let len = u32::from_le_bytes(len_bytes) as usize;
    let mut json = vec![0u8; len];
    reader.read_exact(&mut json).context("reading derived plane header body")?;
    let header: PlaneHeader =
        serde_json::from_slice(&json).context("parsing derived plane header")?;
    Ok((header, reader))
}

/// Strides a reader by a fixed `record_len`, returning each `(key, full_record_bytes)`.
///
/// The returned byte buffer includes the leading 8-byte key (callers slice past it); a torn trailing
/// record errors so an interrupted projection is caught rather than silently dropped.
fn read_records(
    reader: &mut impl Read,
    record_len: usize,
    path: &Path,
) -> Result<Vec<(u64, Vec<u8>)>> {
    let mut out = Vec::new();
    let mut record = vec![0u8; record_len];
    loop {
        let mut filled = 0;
        while filled < record_len {
            match reader.read(&mut record[filled..]).context("reading derived plane record")? {
                0 => break,
                read => filled += read,
            }
        }
        if filled == 0 {
            break;
        }
        if filled < record_len {
            anyhow::bail!(
                "derived plane {} ends with a partial record ({filled} of {record_len} bytes)",
                path.display()
            );
        }
        let key = u64::from_le_bytes(record[..8].try_into().expect("8-byte key"));
        out.push((key, record.clone()));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::{EmbeddingDtype, EmbeddingPooling};
    use crate::config::project_paths_with_root;
    use tempfile::tempdir;

    fn fingerprint(dim: usize, model: &str) -> EmbeddingFingerprint {
        EmbeddingFingerprint::new(
            "static-embed",
            model,
            dim,
            EmbeddingDtype::Float32,
            true,
            EmbeddingPooling::Mean,
            None,
        )
    }

    #[test]
    fn binary_bytes_rounds_up_to_whole_bytes() {
        assert_eq!(binary_bytes_for_dim(0), 0);
        assert_eq!(binary_bytes_for_dim(1), 1);
        assert_eq!(binary_bytes_for_dim(8), 1);
        assert_eq!(binary_bytes_for_dim(9), 2);
        assert_eq!(binary_bytes_for_dim(256), 32);
    }

    #[test]
    fn binarize_uses_sign_bit_with_zero_as_positive() {
        // dim 10: bits laid LSB-first within each byte; component >= 0 -> 1.
        let vector = vec![1.0, -1.0, 0.0, -0.001, 5.0, -5.0, 0.0, 2.0, -3.0, 4.0];
        let bits = binarize(&vector);
        assert_eq!(bits.len(), 2);
        // byte 0 bits for indices 0..8: +,-,+(0.0),-,+,-,+(0.0),+  => 1,0,1,0,1,0,1,1
        assert_eq!(bits[0], 0b1101_0101);
        // byte 1 bits for indices 8,9: -,+ => 0,1
        assert_eq!(bits[1], 0b0000_0010);
    }

    #[test]
    fn hamming_distance_counts_differing_bits_over_words_and_tail() {
        // 9 dims -> 2 bytes, so the second byte exercises the ragged tail path.
        let a = binarize(&[1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0]);
        let b = binarize(&[-1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, -1.0]);
        // Two bits differ (index 0 and index 8).
        assert_eq!(hamming_distance(&a, &b), 2);
        assert_eq!(hamming_distance(&a, &a), 0);
    }

    #[test]
    fn hamming_distance_uses_full_u64_word_path() {
        // 64 dims -> exactly one u64 word, no tail; flip 3 bits.
        let mut left = vec![1.0f32; 64];
        let mut right = vec![1.0f32; 64];
        right[0] = -1.0;
        right[31] = -1.0;
        right[63] = -1.0;
        let _ = &mut left;
        assert_eq!(hamming_distance(&binarize(&left), &binarize(&right)), 3);
    }

    #[test]
    #[should_panic(expected = "equal-length bit vectors")]
    fn hamming_distance_debug_asserts_equal_length() {
        // The equal-length contract is now a debug assertion (D1 hardening): a future caller that
        // prefix-compares a dim-mismatched pair trips here instead of silently scoring garbage. The
        // release path stays total via the `min` fallback (compiled-out assertion).
        let a = vec![0xFFu8, 0x00u8];
        let b = vec![0x00u8];
        let _ = hamming_distance(&a, &b);
    }

    #[test]
    fn quantize_dequantize_round_trips_within_error_bound() {
        let vector = vec![0.9, -0.3, 0.0, 0.5, -0.7, 0.123, -0.456];
        let int8 = quantize_int8(&vector);
        let restored = dequantize_int8(&int8);
        // Per-component error must be bounded by scale/2 (the rounding step).
        let bound = int8.scale / 2.0 + f32::EPSILON;
        for (original, recovered) in vector.iter().zip(restored.iter()) {
            assert!(
                (original - recovered).abs() <= bound,
                "component {original} recovered as {recovered}, bound {bound}"
            );
        }
        // The max-abs component maps to exactly +/-127.
        assert!(int8.codes.contains(&127) || int8.codes.contains(&-127));
    }

    #[test]
    fn quantize_is_symmetric_and_excludes_negative_128() {
        let int8 = quantize_int8(&[1.0, -1.0, 0.5, -0.5]);
        // x and -x quantize to opposite codes; no code reaches -128.
        assert_eq!(int8.codes[0], 127);
        assert_eq!(int8.codes[1], -127);
        assert_eq!(int8.codes[2], 64);
        assert_eq!(int8.codes[3], -64);
        assert!(int8.codes.iter().all(|&code| code >= -127));
    }

    #[test]
    fn quantize_all_zero_vector_is_lossless() {
        let int8 = quantize_int8(&[0.0, 0.0, 0.0]);
        assert_eq!(int8.scale, 0.0);
        assert_eq!(int8.codes, vec![0, 0, 0]);
        assert_eq!(dequantize_int8(&int8), vec![0.0, 0.0, 0.0]);
    }

    #[test]
    fn dot_product_matches_manual() {
        assert_eq!(dot_product(&[1.0, 2.0, 3.0], &[4.0, 5.0, 6.0]), 32.0);
    }

    #[test]
    #[should_panic(expected = "equal-length vectors")]
    fn dot_product_debug_asserts_equal_length() {
        // The equal-length contract is now a debug assertion (D1 hardening): a future caller that
        // prefix-dots a dim-mismatched pair trips here instead of silently scoring garbage. The
        // release path stays total via the `min` fallback (compiled-out assertion).
        let _ = dot_product(&[1.0, 2.0, 3.0], &[4.0, 5.0]);
    }

    /// Seeds a `.pkvec` store with the given records and returns its paths.
    fn seed_store(records: &[(u64, Vec<f32>)], dim: usize) -> (ProjectPaths, tempfile::TempDir) {
        let dir = tempdir().expect("tempdir");
        let paths = project_paths_with_root(dir.path());
        let store =
            VectorStore::create_stamped(&paths, &fingerprint(dim, "model-a")).expect("create");
        store.append_vectors(records).expect("append");
        (paths, dir)
    }

    #[test]
    fn build_planes_projects_store_and_round_trips_both_planes() {
        let records = vec![(10u64, vec![1.0, -1.0, 0.5]), (20u64, vec![-1.0, 1.0, -0.5])];
        let (paths, _dir) = seed_store(&records, 3);

        let report =
            build_planes_from_store(&paths, "static-embed", "model-a").expect("build planes");
        assert_eq!(report.vectors, 2);
        assert_eq!(report.dim, Some(3));

        let binary = BinaryPlane::for_provider(&paths, "static-embed", "model-a");
        let bin_records = binary.read_all().expect("binary read");
        assert_eq!(bin_records.len(), 2);
        assert_eq!(bin_records[0].0, 10);
        assert_eq!(bin_records[0].1, binarize(&records[0].1));

        let int8 = Int8Plane::for_provider(&paths, "static-embed", "model-a");
        let i8_records = int8.read_all().expect("int8 read");
        assert_eq!(i8_records.len(), 2);
        assert_eq!(i8_records[1].0, 20);
        assert_eq!(i8_records[1].1, quantize_int8(&records[1].1));

        // Headers carry the source fingerprint hash.
        let live = fingerprint(3, "model-a");
        assert_eq!(
            binary.read_header().expect("hdr").expect("present").fingerprint_hash,
            live.hash()
        );
        assert!(!planes_are_stale(&paths, "static-embed", "model-a", &live).expect("not stale"));
    }

    #[test]
    fn streaming_lockstep_projection_keeps_binary_and_int8_positionally_aligned() {
        // C1/C2: the load-bearing invariant. After the streaming lockstep projection, int8 position i
        // must be the SAME content_key as binary position i, and `Int8PlaneReader::record_at(i)` must
        // recover exactly that key's quantized vector — this is what makes int8 seek-by-position (rather
        // than a resident int8 plane) correct. A torn-resume duplicate of key 10 also proves the
        // streaming source preserves last-writer-wins (one record per key, the LATER vector).
        let records = vec![
            (10u64, vec![1.0, -1.0, 0.5, 0.25]),
            (20u64, vec![-1.0, 1.0, -0.5, 0.75]),
            (30u64, vec![0.3, 0.3, -0.9, -0.1]),
            (10u64, vec![0.9, 0.9, 0.9, 0.9]), // a torn-resume re-append; LAST wins, no second row
        ];
        let (paths, _dir) = seed_store(&records, 4);
        let report = build_planes_from_store(&paths, "static-embed", "model-a").expect("build");
        assert_eq!(report.vectors, 3, "dedup SET collapses the duplicate key");

        let binary = BinaryPlane::for_provider(&paths, "static-embed", "model-a");
        let bin = binary.read_all().expect("binary read");
        let int8 = Int8Plane::for_provider(&paths, "static-embed", "model-a");
        let mut reader = int8.reader().expect("int8 reader");
        assert_eq!(reader.dim(), 4);

        // The deduped source in first-seen order, each key carrying its LAST vector.
        let expected: Vec<(u64, Vec<f32>)> = vec![
            (10, vec![0.9, 0.9, 0.9, 0.9]),
            (20, records[1].1.clone()),
            (30, records[2].1.clone()),
        ];
        assert_eq!(bin.len(), expected.len());
        for (position, (key, vector)) in expected.iter().enumerate() {
            // Binary position i keyed correctly...
            assert_eq!(bin[position].0, *key, "binary key at {position}");
            // ...and the int8 record SEEKED at the same position is the SAME key + the exact codes.
            let (int8_key, int8_vector) = reader.record_at(position).expect("seek int8");
            assert_eq!(int8_key, *key, "int8 seek-by-position must align with binary position");
            assert_eq!(
                int8_vector,
                quantize_int8(vector),
                "int8 codes must match the source vector"
            );
        }
    }

    #[test]
    fn int8_plane_reader_errors_when_plane_is_absent() {
        // Opening a seek reader over a never-built plane is a clear error (the query path never reaches
        // here — it gates on the binary plane being empty first — but the boundary stays honest).
        let dir = tempdir().expect("tempdir");
        let paths = project_paths_with_root(dir.path());
        let int8 = Int8Plane::for_provider(&paths, "static-embed", "never-built");
        let error = match int8.reader() {
            Ok(_) => panic!("expected an absent-plane error"),
            Err(error) => error,
        };
        assert!(error.to_string().contains("absent"), "got: {error}");
    }

    #[test]
    fn build_planes_without_store_writes_empty_planes() {
        let dir = tempdir().expect("tempdir");
        let paths = project_paths_with_root(dir.path());
        let report =
            build_planes_from_store(&paths, "static-embed", "model-a").expect("build empty");
        assert_eq!(report.vectors, 0);
        assert_eq!(report.dim, None);
        let binary = BinaryPlane::for_provider(&paths, "static-embed", "model-a");
        assert!(binary.exists());
        assert!(binary.read_all().expect("empty read").is_empty());
        let int8 = Int8Plane::for_provider(&paths, "static-embed", "model-a");
        assert!(int8.read_all().expect("empty read").is_empty());
    }

    #[test]
    fn planes_are_stale_when_missing_or_fingerprint_changes() {
        let live = fingerprint(3, "model-a");
        let dir = tempdir().expect("tempdir");
        let paths = project_paths_with_root(dir.path());
        // No planes yet -> stale.
        assert!(planes_are_stale(&paths, "static-embed", "model-a", &live).expect("absent stale"));

        let records = vec![(1u64, vec![1.0, 0.0, 0.0])];
        let (paths, _dir) = seed_store(&records, 3);
        build_planes_from_store(&paths, "static-embed", "model-a").expect("build");
        assert!(!planes_are_stale(&paths, "static-embed", "model-a", &live).expect("fresh"));

        // A changed dim/model -> different hash -> stale.
        let changed = fingerprint(8, "model-a");
        assert!(planes_are_stale(&paths, "static-embed", "model-a", &changed).expect("changed"));
    }

    #[test]
    fn delete_planes_removes_both_and_reports_state() {
        let records = vec![(1u64, vec![1.0, 0.0, 0.0])];
        let (paths, _dir) = seed_store(&records, 3);
        build_planes_from_store(&paths, "static-embed", "model-a").expect("build");
        assert!(delete_planes(&paths, "static-embed", "model-a").expect("delete"));
        assert!(!BinaryPlane::for_provider(&paths, "static-embed", "model-a").exists());
        assert!(!Int8Plane::for_provider(&paths, "static-embed", "model-a").exists());
        // Deleting absent planes is a harmless false.
        assert!(!delete_planes(&paths, "static-embed", "model-a").expect("second delete"));
    }

    #[test]
    fn derived_plane_bytes_sums_only_plane_files() {
        let dir = tempdir().expect("tempdir");
        let paths = project_paths_with_root(dir.path());
        assert_eq!(derived_plane_bytes(&paths), 0);
        let records = vec![(1u64, vec![1.0, 0.0, 0.0])];
        let (paths, _dir) = seed_store(&records, 3);
        build_planes_from_store(&paths, "static-embed", "model-a").expect("build");
        fs::write(paths.vectors_dir.join("notes.txt"), b"ignore").expect("write txt");
        let bytes = derived_plane_bytes(&paths);
        let binary = BinaryPlane::for_provider(&paths, "static-embed", "model-a");
        let int8 = Int8Plane::for_provider(&paths, "static-embed", "model-a");
        let expected = binary.path().metadata().expect("meta").len()
            + int8.path().metadata().expect("meta").len();
        assert_eq!(bytes, expected);
    }

    #[test]
    fn read_rejects_bad_magic_and_partial_record() {
        let records = vec![(1u64, vec![1.0, 0.0, 0.0]), (2u64, vec![0.0, 1.0, 0.0])];
        let (paths, _dir) = seed_store(&records, 3);
        build_planes_from_store(&paths, "static-embed", "model-a").expect("build");

        let binary = BinaryPlane::for_provider(&paths, "static-embed", "model-a");
        // Truncate mid-record to force the partial-record error.
        let len = binary.path().metadata().expect("meta").len();
        let file = std::fs::OpenOptions::new().write(true).open(binary.path()).expect("open");
        file.set_len(len - 1).expect("truncate");
        assert!(binary.read_all().expect_err("partial").to_string().contains("partial record"));

        // Bad magic.
        let int8 = Int8Plane::for_provider(&paths, "static-embed", "model-a");
        fs::write(int8.path(), b"NOTPLANExxxx").expect("garbage");
        assert!(
            int8.read_header().expect_err("bad magic").to_string().contains("unrecognized magic")
        );
    }

    #[test]
    fn read_header_none_for_absent_plane() {
        let dir = tempdir().expect("tempdir");
        let paths = project_paths_with_root(dir.path());
        let binary = BinaryPlane::for_provider(&paths, "static-embed", "model-a");
        assert!(binary.read_header().expect("hdr").is_none());
        assert!(!binary.exists());
    }
}
