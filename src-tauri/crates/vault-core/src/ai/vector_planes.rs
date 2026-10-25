//! Responsibilities: atomically project the deduplicated `.pkvec` source into aligned sign-bit and
//! int8 files, and own the quantisation math.
//! Not responsible for: search eligibility, ranking or embedding generation.
//! Failure modes: the two files disagree on order; a half-written plane is read; a model or
//! dimension change leaves old planes looking current. Tests: projection tests below.
//!
//! Search maps the int8 plane and scores every record (`vector_index.rs`). The sign-bit plane is not
//! read by search; the scan benchmark compares it as a first stage.
//!
//! ```text
//! .pkbin  magic b"PKBIN\0\x01\0" | u32 len + JSON PlaneHeader | records [ key:u64 | bits: ceil(dim/8) B ]
//! .pki8   magic b"PKI8\0\0\x01\0" | u32 len + JSON PlaneHeader | records [ key:u64 | scale:f32 | codes: dim i8 ]
//! ```
//! Both are written from one stream in the same order and published by rename. Each rebuild gets a
//! new `projectionId`, so files derived from positions (`.pkpos`) can tell projections apart.

use crate::ai::EmbeddingFingerprint;
use crate::ai::vector_store::VectorStore;
use crate::ai_sidecar::provider_table_name;
use crate::config::{ProjectPaths, ensure_paths};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs::{self, File};
use std::io::{BufReader, BufWriter, Read, Seek, Write};
use std::path::{Path, PathBuf};

/// Magic + format-version prefix for a sign-bit plane file.
const BINARY_PLANE_MAGIC: [u8; 8] = *b"PKBIN\0\x01\0";
/// Magic + format-version prefix for an int8 score plane file.
const INT8_PLANE_MAGIC: [u8; 8] = *b"PKI8\0\0\x01\0";

/// File extension for the sign-bit plane (one per provider/model).
const BINARY_PLANE_EXTENSION: &str = "pkbin";
/// File extension for the int8 score plane (one per provider/model).
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
    /// Distinguishes rebuilds even when keys, dimension and count stay the same.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub projection_id: String,
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
/// step). Retained for quantisation diagnostics.
pub fn dequantize_int8(vector: &Int8Vector) -> Vec<f32> {
    vector.codes.iter().map(|&code| code as f32 * vector.scale).collect()
}

/// Dot product of two equal-length f32 vectors for projection diagnostics.
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

/// Resolves the sign-bit plane path for one provider/model pair under `derived/vectors/`.
fn binary_plane_path(paths: &ProjectPaths, provider_id: &str, model: &str) -> PathBuf {
    let file = format!("{}.{BINARY_PLANE_EXTENSION}", provider_table_name(provider_id, model));
    paths.vectors_dir.join(file)
}

/// Resolves the int8 score plane path for one provider/model pair under `derived/vectors/`.
fn int8_plane_path(paths: &ProjectPaths, provider_id: &str, model: &str) -> PathBuf {
    let file = format!("{}.{INT8_PLANE_EXTENSION}", provider_table_name(provider_id, model));
    paths.vectors_dir.join(file)
}

/// The sign-bit projection retained for diagnostics and scan benchmarks.
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

    /// Loads packed records for projection diagnostics and scan benchmarks.
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

/// An immutable int8 projection, mapped for contiguous scoring.
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

    /// Maps a complete projection without copying its payload into heap memory.
    pub(crate) fn map(&self) -> Result<Option<MappedInt8Plane>> {
        if !self.exists() {
            return Ok(None);
        }
        let (header, mut reader) = open_plane(&self.path, &INT8_PLANE_MAGIC)?;
        let start = reader.stream_position()? as usize;
        // SAFETY: projection writers replace files atomically and never modify mapped files.
        let bytes = unsafe { memmap2::Mmap::map(reader.get_ref())? };
        anyhow::ensure!(
            start <= bytes.len() && (bytes.len() - start) % (12 + header.dim) == 0,
            "truncated int8 plane"
        );
        Ok(Some(MappedInt8Plane { bytes, start, header }))
    }

    /// Keeps quantisation checks independent of the scoring kernel.
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

pub(crate) struct MappedInt8Plane {
    bytes: memmap2::Mmap,
    start: usize,
    pub header: PlaneHeader,
}
impl MappedInt8Plane {
    pub fn records(&self) -> &[u8] {
        &self.bytes[self.start..]
    }
    pub fn stride(&self) -> usize {
        12 + self.header.dim
    }
    pub fn len(&self) -> usize {
        self.records().len() / self.stride()
    }
    pub fn key(&self, position: usize) -> u64 {
        let offset = position * self.stride();
        u64::from_le_bytes(self.records()[offset..offset + 8].try_into().unwrap())
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

/// Streams the deduplicated source into aligned planes without retaining the vector corpus.
/// Each rebuild has a new identity so position caches cannot outlive a changed projection.
pub fn build_planes_from_store(
    paths: &ProjectPaths,
    provider_id: &str,
    model: &str,
) -> Result<PlaneBuildReport> {
    build_planes_into(paths, paths, provider_id, model)
}

/// Project mutable lineage sources into a new, unpublished generation.
pub(super) fn build_planes_into(
    source: &ProjectPaths,
    paths: &ProjectPaths,
    provider_id: &str,
    model: &str,
) -> Result<PlaneBuildReport> {
    build_planes_into_cancellable(
        source,
        paths,
        provider_id,
        model,
        &vault_store::CancelToken::new(),
    )
}
pub(super) fn build_planes_into_cancellable(
    source: &ProjectPaths,
    paths: &ProjectPaths,
    provider_id: &str,
    model: &str,
    cancel: &vault_store::CancelToken,
) -> Result<PlaneBuildReport> {
    cancel.check()?;
    ensure_paths(paths)?;
    let store = VectorStore::for_provider(source, provider_id, model);
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
            store.read_records_streaming_cancellable(
                |key, vector| {
                    if vectors % 1024 == 0 {
                        cancel.check()?;
                    }
                    vectors += 1;
                    project(key, vector)
                },
                cancel,
            )
        },
    )?;
    Ok(PlaneBuildReport { vectors, dim: (vectors != 0).then_some(header.dim) })
}

/// Deletes both derived planes for one provider/model, returning whether anything was removed.
///
/// Used by the index "clear" path; the planes are pure projections so a clean delete is the correct
/// reset (a later build re-projects from the surviving `.pkvec`).
pub fn delete_planes(paths: &ProjectPaths, provider_id: &str, model: &str) -> Result<bool> {
    let mut removed = super::visit_position_map::delete(paths, provider_id, model)?;
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

/// Writes both projections from one source stream, then publishes each complete file by rename.
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
    let mut id = [0; 16];
    getrandom::fill(&mut id)?;
    let header = PlaneHeader {
        dim,
        fingerprint_hash: fingerprint_hash.to_string(),
        projection_id: hex::encode(id),
    };
    {
        let mut binary_writer = open_plane_writer(&binary_tmp, &BINARY_PLANE_MAGIC, &header)?;
        let mut int8_writer = open_plane_writer(&int8_tmp, &INT8_PLANE_MAGIC, &header)?;
        // The lockstep sink: binarize + int8-quantize the SAME f32 record into both writers in the
        // SAME order, so the planes stay positionally aligned by content_key (the lockstep invariant).
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
    let mut file =
        File::open(path).with_context(|| format!("opening derived plane {}", path.display()))?;
    let mut found = [0u8; 8];
    file.read_exact(&mut found).context("reading derived plane magic")?;
    if &found != magic {
        anyhow::bail!(
            "derived plane {} has an unrecognized magic/format-version header",
            path.display()
        );
    }
    let mut len_bytes = [0u8; 4];
    file.read_exact(&mut len_bytes).context("reading derived plane header length")?;
    let len = u32::from_le_bytes(len_bytes) as usize;
    let mut json = vec![0u8; len];
    file.read_exact(&mut json).context("reading derived plane header body")?;
    let header: PlaneHeader =
        serde_json::from_slice(&json).context("parsing derived plane header")?;
    Ok((header, BufReader::new(file)))
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
    fn streaming_lockstep_projection_keeps_binary_and_int8_positionally_aligned() {
        // Projection order and last-writer-wins dedup must agree in both files.
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
        let reader = int8.map().unwrap().unwrap();
        let rows = int8.read_all().unwrap();
        assert_eq!(reader.header.dim, 4);

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
            // The mapped key and independently decoded codes agree with the source.
            let (int8_key, int8_vector) = &rows[position];
            assert_eq!(reader.key(position), *key);
            assert_eq!(*int8_key, *key, "int8 positions must align with sign-bit positions");
            assert_eq!(
                *int8_vector,
                quantize_int8(vector),
                "int8 codes must match the source vector"
            );
        }
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
}
