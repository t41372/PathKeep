//! Flat vector sidecar store over `derived/vectors/` (the AI vector storage plane, 02 §A).
//!
//! ## Responsibilities
//! - persist computed embeddings to the rebuildable vector plane, keyed by the stable u64
//!   history/visit id, so W-AI-5's `VectorIndex` (Turbovec) can load them back
//! - stamp every store with the [`EmbeddingFingerprint`] (W-AI-0) so a model/dim/dtype change
//!   is DETECTED as stale (the seam W-AI-5 hooks rebuild into)
//! - support append + read-back without rewriting the whole store, so a resumable backfill can
//!   stream chunks in over many runs
//!
//! ## Not responsible for
//! - nearest-neighbour search / quantization (W-AI-5 owns the `VectorIndex` engine)
//! - the canonical archive (vectors NEVER live in canonical SQLite, 02 §A) or the SQLite
//!   `ai_embeddings` compatibility metadata (that stays in the intelligence plane)
//! - full re-embed migration on a fingerprint mismatch (4a only DETECTS staleness + leaves a
//!   clear seam; the rebuild flow lands with W-AI-5/§C.4)
//!
//! ## Format (documented so W-AI-5 / Turbovec can consume it)
//! One file per provider+model pair: `derived/vectors/<table>.pkvec`, where `<table>` is the
//! existing stable [`crate::ai_sidecar::provider_table_name`]. Layout, all little-endian:
//!
//! ```text
//! magic:    8 bytes  = b"PKVEC\0\x01\0"   (format tag + version byte 0x01)
//! header:   u32 len  + that many bytes of UTF-8 JSON (the `VectorStoreHeader` below)
//! records:  repeated [ id: u64 | vector: f32 * header.dim ] until EOF
//! ```
//!
//! Fixed-width records keyed by `u64` make this a flat, mmap-friendly, append-friendly store: a
//! reader strides by `8 + 4*dim` bytes and emits `(u64, Vec<f32>)` pairs — exactly the
//! `VectorIndex::build/append` input shape. The header carries the fingerprint HASH plus the dim
//! so a consumer can reject a stale or dimension-mismatched store before reading a single vector.
//!
//! ## Storage footprint — raw f32 is the REBUILD SOURCE plane, NOT the final on-disk rerank format
//! Records are raw little-endian `f32`, so the store is `(8 + 4*dim)` bytes/row. At the 14.4M-row
//! tail with a 1024-dim model that is ~14.4M × (8 + 4096) ≈ **59 GB** — too large to ship as the
//! final on-disk rerank tier and the reason this plane is EXCLUDED from the export bundle (it is
//! rebuildable derived state; see `migration.rs` `EXPORT_EXCLUSIONS_DOC`). Keeping raw f32 here is a
//! deliberate 4a choice: it is the lossless SOURCE the disk-rerank tier is built FROM. The project
//! research recommends an **int8 / MRL** on-disk format for the rerank tier; that quantization
//! decision (Turbovec quantization + the disk-rerank record format) is **explicitly deferred to
//! W-AI-5 / S2** and is NOT made here. Until then, do not treat 59 GB at the tail as acceptable
//! steady state — it is the rebuild source, sized to be regenerated, not retained, on the target.
//!
//! ## Performance notes
//! - append opens the file once, seeks to the end, and writes a contiguous buffer of records; it
//!   never rewrites prior records, so backfilling 14.4M rows is O(rows written), not O(store).
//! - reads stream record-by-record with a fixed stride, so memory is bounded by one record at a
//!   time plus the caller's collection. No JSON-per-vector overhead.

use crate::ai::EmbeddingFingerprint;
use crate::ai_sidecar::provider_table_name;
use crate::config::{ProjectPaths, ensure_paths};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs::{self, File, OpenOptions};
use std::io::{BufReader, BufWriter, Read, Seek, Write};
use std::path::PathBuf;

/// Magic + format-version prefix written at the head of every `.pkvec` file.
///
/// The trailing `0x01` is the on-disk format version; bump it (and the reader's check) if the
/// record layout ever changes so an old file is rejected rather than misread.
const VECTOR_STORE_MAGIC: [u8; 8] = *b"PKVEC\0\x01\0";

/// File extension for one provider/model vector store.
const VECTOR_STORE_EXTENSION: &str = "pkvec";

/// JSON header persisted after the magic; identifies how the store was embedded.
///
/// `fingerprint_hash` is the short [`EmbeddingFingerprint::hash`] token, the canonical identity
/// used to answer "is this store stale against the live embedding config?". `dim` is duplicated
/// out of the fingerprint as a first-class field because the reader needs the record stride
/// BEFORE trusting the (string) hash, and a dim mismatch is the one corruption that must fail
/// loudly rather than silently misalign records.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VectorStoreHeader {
    /// Provider identity that produced the vectors (mirrors the fingerprint field).
    pub provider_id: String,
    /// Model identifier (a runtime string, never a hardcoded product constant).
    pub model_id: String,
    /// Effective vector dimension = the record stride driver. Equals `fingerprint.effective_dim`.
    pub dim: usize,
    /// The embedding fingerprint hash this store was built under (stale-detection identity).
    pub fingerprint_hash: String,
}

/// A handle to one provider/model vector store on the `derived/vectors/` plane.
///
/// Cheap to construct (just resolves the path); all I/O happens in the explicit methods so a
/// caller can stamp/append/read across separate runs (the resumable-backfill use case).
pub struct VectorStore {
    path: PathBuf,
}

impl VectorStore {
    /// Resolves the store handle for one provider/model pair under `derived/vectors/`.
    ///
    /// Uses the existing stable [`provider_table_name`] derivation so the file name matches the
    /// `ai_index_ledger.sidecar_table` already recorded for this provider/model — no second
    /// naming scheme to keep in sync.
    pub fn for_provider(paths: &ProjectPaths, provider_id: &str, model: &str) -> Self {
        let file = format!("{}.{VECTOR_STORE_EXTENSION}", provider_table_name(provider_id, model));
        Self { path: paths.vectors_dir.join(file) }
    }

    /// Returns the on-disk path of this store (also used by storage-size reporting).
    pub fn path(&self) -> &std::path::Path {
        &self.path
    }

    /// Returns whether a store file already exists on disk.
    pub fn exists(&self) -> bool {
        self.path.exists()
    }

    /// Creates (or truncates) the store, writing the magic + header under the given fingerprint.
    ///
    /// Called once at the start of a full rebuild. The fingerprint MUST already carry the real
    /// observed `effective_dim` (the indexing loop derives it from the first returned vector), so
    /// the on-disk dim is the truth, not a config assumption (D4).
    pub fn create_stamped(
        paths: &ProjectPaths,
        fingerprint: &EmbeddingFingerprint,
    ) -> Result<Self> {
        ensure_paths(paths)?;
        let store = Self::for_provider(paths, &fingerprint.provider_id, &fingerprint.model_id);
        let header = VectorStoreHeader {
            provider_id: fingerprint.provider_id.clone(),
            model_id: fingerprint.model_id.clone(),
            dim: fingerprint.effective_dim,
            fingerprint_hash: fingerprint.hash(),
        };
        let file = File::create(&store.path)
            .with_context(|| format!("creating vector store {}", store.path.display()))?;
        let mut writer = BufWriter::new(file);
        write_header(&mut writer, &header)?;
        writer.flush().context("flushing new vector store header")?;
        Ok(store)
    }

    /// Reads the persisted header without reading any vectors.
    ///
    /// Returns `Ok(None)` when the file does not exist (a never-built store), so callers can
    /// distinguish "no store yet" from "stale store". A present-but-corrupt file errors.
    pub fn read_header(&self) -> Result<Option<VectorStoreHeader>> {
        if !self.exists() {
            return Ok(None);
        }
        let file = File::open(&self.path)
            .with_context(|| format!("opening vector store {}", self.path.display()))?;
        let mut reader = BufReader::new(file);
        let header = read_header(&mut reader)?;
        Ok(Some(header))
    }

    /// Answers "is the persisted store stale against the live embedding fingerprint?".
    ///
    /// `Ok(true)` when there is no store yet OR the stored fingerprint hash differs from the live
    /// one — the single seam W-AI-5 hooks the versioned-rebuild flow into (02 §C.4). It compares
    /// the stored HASH token (not the full struct) so only the short token has to be persisted.
    pub fn is_stale_against(&self, live: &EmbeddingFingerprint) -> Result<bool> {
        match self.read_header()? {
            None => Ok(true),
            Some(header) => Ok(header.fingerprint_hash != live.hash()),
        }
    }

    /// Appends a batch of `(history_id, vector)` records to the end of the store.
    ///
    /// Every vector's length MUST equal the header dim; a mismatch errors rather than writing a
    /// ragged record that would desync the fixed-stride reader. Appends are contiguous and never
    /// rewrite earlier records, which is what makes the resumable backfill O(rows written).
    pub fn append_vectors(&self, records: &[(u64, Vec<f32>)]) -> Result<()> {
        if records.is_empty() {
            return Ok(());
        }
        let header = self
            .read_header()?
            .with_context(|| format!("vector store {} is not stamped", self.path.display()))?;
        let mut buffer = Vec::with_capacity(records.len() * (8 + 4 * header.dim));
        for (id, vector) in records {
            if vector.len() != header.dim {
                anyhow::bail!(
                    "vector for id {id} has length {} but store dim is {}",
                    vector.len(),
                    header.dim
                );
            }
            buffer.extend_from_slice(&id.to_le_bytes());
            for component in vector {
                buffer.extend_from_slice(&component.to_le_bytes());
            }
        }
        let mut file = OpenOptions::new()
            .append(true)
            .open(&self.path)
            .with_context(|| format!("opening vector store {} for append", self.path.display()))?;
        file.write_all(&buffer).context("appending vectors")?;
        file.flush().context("flushing appended vectors")?;
        Ok(())
    }

    /// Reads every `(history_id, vector)` record back, deduplicated by id (last-writer-wins).
    ///
    /// CONTRACT W-AI-5 RELIES ON: the returned ids are a SET — each `history_id` appears exactly
    /// once, with the LAST persisted vector for that id (its most recent embedding). A resumable
    /// backfill that crashes between an append and its cursor-persist can leave a SECOND copy of an
    /// id on disk (the rows in the re-embedded boundary chunk); the indexing loop avoids most of
    /// this by skipping ids already on disk on resume, but this read-side dedup is the DEFENSIVE
    /// backstop so `VectorIndex::build/append` never ingests a duplicate id no matter how the file
    /// was torn. Records are emitted in first-seen order with each id carrying its latest vector.
    ///
    /// It strides by a fixed record width; a trailing partial record (an interrupted append) errors
    /// so a torn write is caught rather than silently dropped.
    pub fn read_all(&self) -> Result<Vec<(u64, Vec<f32>)>> {
        let file = File::open(&self.path)
            .with_context(|| format!("opening vector store {}", self.path.display()))?;
        let mut reader = BufReader::new(file);
        let header = read_header(&mut reader)?;
        let record_len = 8 + 4 * header.dim;
        // `order` preserves first-seen id order; `latest` maps id → index in `order` so a later
        // duplicate overwrites the earlier vector in place (last-writer-wins) without changing the
        // id's position. This keeps the output a set while staying O(records) time and memory.
        let mut order: Vec<(u64, Vec<f32>)> = Vec::new();
        let mut latest: std::collections::HashMap<u64, usize> = std::collections::HashMap::new();
        let mut record = vec![0u8; record_len];
        loop {
            match read_exact_or_eof(&mut reader, &mut record)? {
                ReadOutcome::Eof => break,
                ReadOutcome::Partial(read) => {
                    anyhow::bail!(
                        "vector store {} ends with a partial record ({read} of {record_len} bytes)",
                        self.path.display()
                    );
                }
                ReadOutcome::Full => {
                    let id = u64::from_le_bytes(record[..8].try_into().expect("8-byte id slice"));
                    let mut vector = Vec::with_capacity(header.dim);
                    for component in record[8..].chunks_exact(4) {
                        vector.push(f32::from_le_bytes(
                            component.try_into().expect("4-byte f32 slice"),
                        ));
                    }
                    match latest.get(&id) {
                        Some(&position) => order[position].1 = vector,
                        None => {
                            latest.insert(id, order.len());
                            order.push((id, vector));
                        }
                    }
                }
            }
        }
        Ok(order)
    }

    /// Streams the SET of `history_id`s currently persisted, without retaining any vectors.
    ///
    /// Used by the resumable backfill to skip re-appending an id whose vector is already on disk
    /// (the crash-between-append-and-upsert window, CRITICAL-2): the loop loads this once on resume
    /// and consults it before each append. Strides record-by-record like [`read_all`] but keeps only
    /// each id, so memory is one id-set plus a single reusable record buffer — never the whole store
    /// of vectors. A torn trailing record errors, mirroring [`read_all`], so a half-written body is
    /// caught rather than mistaken for a clean id.
    pub fn existing_ids(&self) -> Result<std::collections::HashSet<u64>> {
        let mut ids = std::collections::HashSet::new();
        if !self.exists() {
            return Ok(ids);
        }
        let file = File::open(&self.path)
            .with_context(|| format!("opening vector store {}", self.path.display()))?;
        let mut reader = BufReader::new(file);
        let header = read_header(&mut reader)?;
        let record_len = 8 + 4 * header.dim;
        let mut record = vec![0u8; record_len];
        loop {
            match read_exact_or_eof(&mut reader, &mut record)? {
                ReadOutcome::Eof => break,
                ReadOutcome::Partial(read) => {
                    anyhow::bail!(
                        "vector store {} ends with a partial record ({read} of {record_len} bytes)",
                        self.path.display()
                    );
                }
                ReadOutcome::Full => {
                    ids.insert(u64::from_le_bytes(
                        record[..8].try_into().expect("8-byte id slice"),
                    ));
                }
            }
        }
        Ok(ids)
    }

    /// Returns the number of vectors currently stored (header excluded).
    ///
    /// Computed from the file length and the header-derived record width, so it is O(1) and never
    /// reads the vectors — used by progress reporting during backfill. A data region that is NOT a
    /// whole multiple of the record stride is a torn/corrupt store; `count()` ERRORS in that case
    /// (mirroring [`read_all`]'s partial-record rejection) rather than silently reporting a
    /// truncated count, so a torn write surfaces consistently from both the O(1) and streaming
    /// paths.
    pub fn count(&self) -> Result<u64> {
        if !self.exists() {
            return Ok(0);
        }
        let file = File::open(&self.path)
            .with_context(|| format!("opening vector store {}", self.path.display()))?;
        let mut reader = BufReader::new(file);
        let header = read_header(&mut reader)?;
        let data_start = reader.stream_position().context("reading vector store data offset")?;
        let total = self.path.metadata().context("vector store metadata")?.len();
        // The record width is `8 (id) + 4*dim`, structurally >= 8, so this is never a zero divisor.
        let record_len = (8 + 4 * header.dim) as u64;
        let data_len = total.saturating_sub(data_start);
        if data_len % record_len != 0 {
            anyhow::bail!(
                "vector store {} data region ({data_len} bytes) is not a whole multiple of the {record_len}-byte record stride (torn write)",
                self.path.display()
            );
        }
        Ok(data_len / record_len)
    }

    /// Deletes the store file if it exists, returning whether a file was removed.
    ///
    /// Used by the index "clear" path; the vector plane is rebuildable so a clean delete is the
    /// correct reset (no partial state to preserve).
    pub fn delete(&self) -> Result<bool> {
        if !self.exists() {
            return Ok(false);
        }
        fs::remove_file(&self.path)
            .with_context(|| format!("removing vector store {}", self.path.display()))?;
        Ok(true)
    }
}

/// Writes the magic + length-prefixed JSON header to a fresh store writer.
fn write_header<W: Write>(writer: &mut W, header: &VectorStoreHeader) -> Result<()> {
    writer.write_all(&VECTOR_STORE_MAGIC).context("writing vector store magic")?;
    let json = serde_json::to_vec(header).context("serializing vector store header")?;
    let len = u32::try_from(json.len()).context("vector store header too large")?;
    writer.write_all(&len.to_le_bytes()).context("writing vector store header length")?;
    writer.write_all(&json).context("writing vector store header body")?;
    Ok(())
}

/// Reads and validates the magic + length-prefixed JSON header, leaving the reader at the first
/// record.
fn read_header<R: Read>(reader: &mut R) -> Result<VectorStoreHeader> {
    let mut magic = [0u8; 8];
    reader.read_exact(&mut magic).context("reading vector store magic")?;
    if magic != VECTOR_STORE_MAGIC {
        anyhow::bail!("vector store has an unrecognized magic/format-version header");
    }
    let mut len_bytes = [0u8; 4];
    reader.read_exact(&mut len_bytes).context("reading vector store header length")?;
    let len = u32::from_le_bytes(len_bytes) as usize;
    let mut json = vec![0u8; len];
    reader.read_exact(&mut json).context("reading vector store header body")?;
    serde_json::from_slice(&json).context("parsing vector store header")
}

/// Outcome of reading exactly one fixed-width record's worth of bytes.
enum ReadOutcome {
    /// A full record was read.
    Full,
    /// Clean end of file at a record boundary (zero bytes read).
    Eof,
    /// A short read mid-record (torn write); carries how many bytes were read.
    Partial(usize),
}

/// Reads exactly `buffer.len()` bytes, distinguishing clean EOF from a torn mid-record read.
fn read_exact_or_eof<R: Read>(reader: &mut R, buffer: &mut [u8]) -> Result<ReadOutcome> {
    let mut filled = 0;
    while filled < buffer.len() {
        match reader.read(&mut buffer[filled..]).context("reading vector store record")? {
            0 => {
                return Ok(if filled == 0 {
                    ReadOutcome::Eof
                } else {
                    ReadOutcome::Partial(filled)
                });
            }
            read => filled += read,
        }
    }
    Ok(ReadOutcome::Full)
}

/// Reports total bytes consumed by all `.pkvec` stores under the vector plane.
///
/// Filesystem-only so storage review can size the vector plane without loading any engine.
pub fn vector_plane_bytes(paths: &ProjectPaths) -> u64 {
    let Ok(entries) = fs::read_dir(&paths.vectors_dir) else {
        return 0;
    };
    entries
        .flatten()
        .filter(|entry| {
            entry.path().extension().and_then(|ext| ext.to_str()) == Some(VECTOR_STORE_EXTENSION)
        })
        .filter_map(|entry| entry.metadata().ok().map(|meta| meta.len()))
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::{EmbeddingDtype, EmbeddingPooling};
    use crate::config::project_paths_with_root;
    use tempfile::tempdir;

    fn fingerprint(dim: usize, model: &str) -> EmbeddingFingerprint {
        EmbeddingFingerprint::new(
            "lmstudio-embed",
            model,
            dim,
            EmbeddingDtype::Float32,
            true,
            EmbeddingPooling::Unknown,
            None,
        )
    }

    #[test]
    fn create_stamp_append_read_round_trips() {
        let dir = tempdir().expect("tempdir");
        let paths = project_paths_with_root(dir.path());
        let fp = fingerprint(3, "model-a");
        let store = VectorStore::create_stamped(&paths, &fp).expect("create");

        let header = store.read_header().expect("header").expect("present");
        assert_eq!(header.dim, 3);
        assert_eq!(header.fingerprint_hash, fp.hash());
        assert_eq!(header.provider_id, "lmstudio-embed");
        assert_eq!(header.model_id, "model-a");
        assert_eq!(store.count().expect("count"), 0);

        store
            .append_vectors(&[(10, vec![1.0, 0.0, 0.0]), (20, vec![0.0, 1.0, 0.0])])
            .expect("append batch 1");
        store.append_vectors(&[(30, vec![0.0, 0.0, 1.0])]).expect("append batch 2");

        assert_eq!(store.count().expect("count"), 3);
        let all = store.read_all().expect("read all");
        assert_eq!(
            all,
            vec![(10, vec![1.0, 0.0, 0.0]), (20, vec![0.0, 1.0, 0.0]), (30, vec![0.0, 0.0, 1.0]),]
        );
    }

    #[test]
    fn empty_append_is_noop() {
        let dir = tempdir().expect("tempdir");
        let paths = project_paths_with_root(dir.path());
        let store = VectorStore::create_stamped(&paths, &fingerprint(2, "m")).expect("create");
        store.append_vectors(&[]).expect("empty append");
        assert_eq!(store.count().expect("count"), 0);
    }

    #[test]
    fn append_rejects_dim_mismatch() {
        let dir = tempdir().expect("tempdir");
        let paths = project_paths_with_root(dir.path());
        let store = VectorStore::create_stamped(&paths, &fingerprint(3, "m")).expect("create");
        let error = store.append_vectors(&[(1, vec![1.0, 2.0])]).expect_err("dim mismatch");
        assert!(error.to_string().contains("length 2 but store dim is 3"));
    }

    #[test]
    fn append_requires_stamped_store() {
        let dir = tempdir().expect("tempdir");
        let paths = project_paths_with_root(dir.path());
        ensure_paths(&paths).expect("ensure");
        let store = VectorStore::for_provider(&paths, "p", "m");
        // No create_stamped → read_header returns None → append errors.
        let error = store.append_vectors(&[(1, vec![1.0])]).expect_err("not stamped");
        assert!(error.to_string().contains("is not stamped"));
    }

    #[test]
    fn is_stale_against_detects_missing_and_changed_fingerprint() {
        let dir = tempdir().expect("tempdir");
        let paths = project_paths_with_root(dir.path());
        let live = fingerprint(4, "model-a");

        // No store yet → stale (triggers initial build).
        let absent = VectorStore::for_provider(&paths, &live.provider_id, &live.model_id);
        assert!(absent.is_stale_against(&live).expect("absent stale"));

        let store = VectorStore::create_stamped(&paths, &live).expect("create");
        assert!(!store.is_stale_against(&live).expect("fresh not stale"));

        // A changed dim/model → different hash → stale.
        let changed = fingerprint(8, "model-a");
        assert!(store.is_stale_against(&changed).expect("changed stale"));
    }

    #[test]
    fn read_all_rejects_partial_trailing_record() {
        let dir = tempdir().expect("tempdir");
        let paths = project_paths_with_root(dir.path());
        let store = VectorStore::create_stamped(&paths, &fingerprint(3, "m")).expect("create");
        store.append_vectors(&[(1, vec![1.0, 2.0, 3.0])]).expect("append");
        // Simulate a torn append by truncating mid-record.
        let len = store.path().metadata().expect("meta").len();
        let file = OpenOptions::new().write(true).open(store.path()).expect("open");
        file.set_len(len - 3).expect("truncate");
        let error = store.read_all().expect_err("partial record");
        assert!(error.to_string().contains("partial record"));
    }

    #[test]
    fn read_header_rejects_bad_magic() {
        let dir = tempdir().expect("tempdir");
        let paths = project_paths_with_root(dir.path());
        ensure_paths(&paths).expect("ensure");
        let store = VectorStore::for_provider(&paths, "p", "m");
        fs::write(store.path(), b"NOTPKVECxxxx").expect("write garbage");
        let error = store.read_header().expect_err("bad magic");
        assert!(error.to_string().contains("unrecognized magic"));
    }

    #[test]
    fn count_and_read_header_none_for_absent_store() {
        let dir = tempdir().expect("tempdir");
        let paths = project_paths_with_root(dir.path());
        let store = VectorStore::for_provider(&paths, "p", "m");
        assert!(!store.exists());
        assert_eq!(store.count().expect("count"), 0);
        assert!(store.read_header().expect("header").is_none());
        assert!(store.is_stale_against(&fingerprint(2, "m")).expect("stale"));
    }

    #[test]
    fn delete_removes_store_and_reports_state() {
        let dir = tempdir().expect("tempdir");
        let paths = project_paths_with_root(dir.path());
        let store = VectorStore::create_stamped(&paths, &fingerprint(2, "m")).expect("create");
        assert!(store.exists());
        assert!(store.delete().expect("delete"));
        assert!(!store.exists());
        // Deleting an absent store is a harmless false.
        assert!(!store.delete().expect("second delete"));
    }

    #[test]
    fn vector_plane_bytes_sums_only_pkvec_files() {
        let dir = tempdir().expect("tempdir");
        let paths = project_paths_with_root(dir.path());
        assert_eq!(vector_plane_bytes(&paths), 0);

        let store = VectorStore::create_stamped(&paths, &fingerprint(2, "m")).expect("create");
        store.append_vectors(&[(1, vec![1.0, 2.0])]).expect("append");
        // A non-pkvec file in the plane is ignored.
        fs::write(paths.vectors_dir.join("notes.txt"), b"ignore me").expect("write txt");

        let bytes = vector_plane_bytes(&paths);
        assert_eq!(bytes, store.path().metadata().expect("meta").len());
        assert!(bytes > 0);
    }

    #[test]
    fn for_provider_path_uses_stable_table_name() {
        let dir = tempdir().expect("tempdir");
        let paths = project_paths_with_root(dir.path());
        let store =
            VectorStore::for_provider(&paths, "openai-compatible", "text-embedding-3-small");
        let name = store.path().file_name().expect("name").to_string_lossy().to_string();
        assert!(name.starts_with("pathkeep_openai_compatible_"));
        assert!(name.ends_with(".pkvec"));
    }

    #[test]
    fn record_bytes_are_little_endian_id_then_f32_components() {
        // MEDIUM-6: byte-pin the on-disk record so a symmetric write+read endianness mutant (which
        // round-trips through `read_all` undetected) is killed. We read the raw file and assert the
        // exact little-endian layout of a known record: id (u64 LE) then each component (f32 LE).
        let dir = tempdir().expect("tempdir");
        let paths = project_paths_with_root(dir.path());
        let store = VectorStore::create_stamped(&paths, &fingerprint(2, "m")).expect("create");
        store.append_vectors(&[(0x0102_0304_0506_0708, vec![1.0_f32, -2.0_f32])]).expect("append");

        let bytes = fs::read(store.path()).expect("read file");
        // The record region is the last `8 + 4*2 = 16` bytes (header precedes it).
        let record = &bytes[bytes.len() - 16..];
        assert_eq!(
            &record[..8],
            &0x0102_0304_0506_0708_u64.to_le_bytes(),
            "id must be little-endian u64",
        );
        assert_eq!(&record[8..12], &1.0_f32.to_le_bytes(), "first component little-endian f32");
        assert_eq!(
            &record[12..16],
            &(-2.0_f32).to_le_bytes(),
            "second component little-endian f32"
        );
        // Sanity: the symmetric round-trip still decodes the same record.
        assert_eq!(store.read_all().expect("read"), vec![(0x0102_0304_0506_0708, vec![1.0, -2.0])]);
    }

    #[test]
    fn read_all_dedupes_duplicate_ids_last_writer_wins() {
        // CRITICAL-2 backstop / W-AI-5 contract: a torn resume can append a second copy of an id;
        // `read_all` must return a SET, keeping the LAST vector for that id in first-seen order.
        let dir = tempdir().expect("tempdir");
        let paths = project_paths_with_root(dir.path());
        let store = VectorStore::create_stamped(&paths, &fingerprint(2, "m")).expect("create");
        store.append_vectors(&[(10, vec![1.0, 0.0]), (20, vec![0.0, 1.0])]).expect("append 1");
        // Re-append id 10 with a NEW vector (the resume re-embed) plus a fresh id 30.
        store.append_vectors(&[(10, vec![9.0, 9.0]), (30, vec![5.0, 5.0])]).expect("append 2");

        let all = store.read_all().expect("read all");
        let ids: Vec<u64> = all.iter().map(|(id, _)| *id).collect();
        assert_eq!(ids, vec![10, 20, 30], "ids are a set in first-seen order");
        // id 10 carries its LAST persisted vector.
        assert_eq!(all[0], (10, vec![9.0, 9.0]));
    }

    #[test]
    fn existing_ids_returns_persisted_id_set() {
        let dir = tempdir().expect("tempdir");
        let paths = project_paths_with_root(dir.path());
        let store = VectorStore::create_stamped(&paths, &fingerprint(2, "m")).expect("create");
        assert!(store.existing_ids().expect("ids on empty store").is_empty());
        store
            .append_vectors(&[(7, vec![1.0, 0.0]), (9, vec![0.0, 1.0]), (7, vec![2.0, 2.0])])
            .expect("append");
        let ids = store.existing_ids().expect("ids");
        assert_eq!(ids, std::collections::HashSet::from([7, 9]));
    }

    #[test]
    fn existing_ids_is_empty_for_absent_store() {
        let dir = tempdir().expect("tempdir");
        let paths = project_paths_with_root(dir.path());
        let store = VectorStore::for_provider(&paths, "p", "m");
        assert!(!store.exists());
        assert!(store.existing_ids().expect("absent ids").is_empty());
    }

    #[test]
    fn existing_ids_rejects_partial_trailing_record() {
        let dir = tempdir().expect("tempdir");
        let paths = project_paths_with_root(dir.path());
        let store = VectorStore::create_stamped(&paths, &fingerprint(3, "m")).expect("create");
        store.append_vectors(&[(1, vec![1.0, 2.0, 3.0])]).expect("append");
        let len = store.path().metadata().expect("meta").len();
        let file = OpenOptions::new().write(true).open(store.path()).expect("open");
        file.set_len(len - 3).expect("truncate");
        let error = store.existing_ids().expect_err("partial record");
        assert!(error.to_string().contains("partial record"));
    }

    #[test]
    fn count_rejects_torn_data_region() {
        // MEDIUM-8: a data region that is not a whole multiple of the record stride is a torn write;
        // `count()` must error (mirroring `read_all`) rather than silently truncating the count.
        let dir = tempdir().expect("tempdir");
        let paths = project_paths_with_root(dir.path());
        let store = VectorStore::create_stamped(&paths, &fingerprint(3, "m")).expect("create");
        store.append_vectors(&[(1, vec![1.0, 2.0, 3.0])]).expect("append");
        let len = store.path().metadata().expect("meta").len();
        let file = OpenOptions::new().write(true).open(store.path()).expect("open");
        // Drop 2 bytes so the data region is no longer a whole record stride.
        file.set_len(len - 2).expect("truncate");
        let error = store.count().expect_err("torn data region");
        assert!(error.to_string().contains("not a whole multiple"));
    }
}
