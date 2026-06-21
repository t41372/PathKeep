//! Visit → content_key map sidecar over `derived/vectors/` (W-AI-4c dedup, 05 §1).
//!
//! ## Responsibilities
//! - persist the many-to-one mapping `history_id → content_key` so a single deduped vector (one per
//!   unique content, in the `.pkvec` store) fans back out to every visit that shares it. This is the
//!   join that makes "5000 gmail visits → 1 embedding" usable: search/heavy-tier resolve a content
//!   vector's visits through this map.
//! - support append + read-back without rewriting the whole map, so the resumable backfill streams
//!   chunks in over many runs and a restart knows which visits it already mapped (no-dup/no-miss).
//!
//! ## Not responsible for
//! - the vectors themselves (the `.pkvec` [`super::vector_store::VectorStore`] owns those) or the
//!   dedup hashing ([`super::dedup`]). This is purely the visit↔content adjacency.
//! - nearest-neighbour search (W-AI-5) or the SQLite `ai_embeddings` metadata.
//!
//! ## Format
//! One file per provider+model pair: `derived/vectors/<table>.pkmap` (the SAME stable
//! [`crate::ai_sidecar::provider_table_name`] as the `.pkvec`). Layout, all little-endian:
//!
//! ```text
//! magic:    8 bytes  = b"PKMAP\0\x01\0"   (format tag + version byte 0x01)
//! records:  repeated [ history_id: i64 | content_key: u64 ] until EOF
//! ```
//!
//! Fixed 16-byte records make this flat + mmap-friendly + append-friendly, exactly like the `.pkvec`
//! store. A reader strides by 16 bytes; a torn trailing record errors (a half-written body is caught,
//! not mistaken for a clean entry). `read_all` is last-writer-wins per history_id so a torn resume's
//! second copy collapses (the same defensive backstop the `.pkvec` store uses).

use crate::ai_sidecar::provider_table_name;
use crate::config::{ProjectPaths, ensure_paths};
use anyhow::{Context, Result};
use std::collections::{HashMap, HashSet};
use std::fs::{self, File, OpenOptions};
use std::io::{BufReader, Read, Write};
use std::path::PathBuf;

/// Magic + format-version prefix written at the head of every `.pkmap` file.
const VISIT_MAP_MAGIC: [u8; 8] = *b"PKMAP\0\x01\0";

/// File extension for one provider/model visit→content map.
const VISIT_MAP_EXTENSION: &str = "pkmap";

/// Fixed record width: `i64` history_id + `u64` content_key.
const RECORD_LEN: usize = 16;

/// A handle to one provider/model visit→content map on the `derived/vectors/` plane.
///
/// Cheap to construct (resolves the path only); all I/O is in the explicit methods so a caller can
/// append/read across separate runs (the resumable-backfill use case), mirroring [`super::vector_store::VectorStore`].
pub struct VisitContentMap {
    path: PathBuf,
}

impl VisitContentMap {
    /// Resolves the map handle for one provider/model pair under `derived/vectors/`.
    ///
    /// Uses the same stable [`provider_table_name`] as the `.pkvec` store so the `.pkmap` sits beside
    /// its vectors and no second naming scheme has to be kept in sync.
    pub fn for_provider(paths: &ProjectPaths, provider_id: &str, model: &str) -> Self {
        let file = format!("{}.{VISIT_MAP_EXTENSION}", provider_table_name(provider_id, model));
        Self { path: paths.vectors_dir.join(file) }
    }

    /// Returns the on-disk path of this map (also used by storage-size reporting).
    pub fn path(&self) -> &std::path::Path {
        &self.path
    }

    /// Returns whether a map file already exists on disk.
    pub fn exists(&self) -> bool {
        self.path.exists()
    }

    /// Ensures the map file exists with its magic header, creating it if absent (idempotent).
    ///
    /// Called by the backfill before the first append: on a full rebuild the caller deletes the map
    /// first so this writes a fresh header; on an incremental/resume pass an existing map is left
    /// untouched so prior mappings survive. `paths` is threaded so the vectors dir is ensured.
    pub fn ensure_created(&self, paths: &ProjectPaths) -> Result<()> {
        ensure_paths(paths)?;
        if self.exists() {
            return Ok(());
        }
        let mut file = File::create(&self.path)
            .with_context(|| format!("creating visit map {}", self.path.display()))?;
        file.write_all(&VISIT_MAP_MAGIC).context("writing visit map magic")?;
        file.flush().context("flushing new visit map header")?;
        Ok(())
    }

    /// Appends a batch of `(history_id, content_key)` records to the end of the map.
    ///
    /// Contiguous append, never rewrites earlier records, so backfilling 14.4M visits is O(rows
    /// written). The map must already exist (created at the true start of a build); a missing file is
    /// a clear error rather than a silent no-op.
    pub fn append(&self, records: &[(i64, u64)]) -> Result<()> {
        if records.is_empty() {
            return Ok(());
        }
        if !self.exists() {
            anyhow::bail!("visit map {} does not exist (create it first)", self.path.display());
        }
        let mut buffer = Vec::with_capacity(records.len() * RECORD_LEN);
        for (history_id, content_key) in records {
            buffer.extend_from_slice(&history_id.to_le_bytes());
            buffer.extend_from_slice(&content_key.to_le_bytes());
        }
        let mut file = OpenOptions::new()
            .append(true)
            .open(&self.path)
            .with_context(|| format!("opening visit map {} for append", self.path.display()))?;
        file.write_all(&buffer).context("appending visit map records")?;
        file.flush().context("flushing appended visit map records")?;
        Ok(())
    }

    /// Reads the full `history_id → content_key` map, last-writer-wins per history_id.
    ///
    /// A torn resume can append a second entry for one history_id; this returns a MAP (one entry per
    /// history_id, the last persisted content_key) so a re-mapped visit collapses, mirroring the
    /// `.pkvec` `read_all` dedup contract. A trailing partial record errors.
    pub fn read_all(&self) -> Result<HashMap<i64, u64>> {
        let mut map = HashMap::new();
        self.for_each_record(|history_id, content_key| {
            map.insert(history_id, content_key);
        })?;
        Ok(map)
    }

    /// Streams the SET of `history_id`s currently mapped, without retaining content keys.
    ///
    /// Used by the resumable backfill to skip re-appending a visit it already mapped (the
    /// crash-between-append-and-cursor window), so a resume never doubles a visit's mapping entry.
    pub fn mapped_history_ids(&self) -> Result<HashSet<i64>> {
        let mut ids = HashSet::new();
        self.for_each_record(|history_id, _| {
            ids.insert(history_id);
        })?;
        Ok(ids)
    }

    /// Streams the SET of `content_key`s referenced by at least one visit, without retaining ids.
    ///
    /// The inverse coverage view: which deduped vectors actually have a visit pointing at them. Used
    /// when validating that every embedded content_key is reachable (no orphan vector).
    pub fn referenced_content_keys(&self) -> Result<HashSet<u64>> {
        let mut keys = HashSet::new();
        self.for_each_record(|_, content_key| {
            keys.insert(content_key);
        })?;
        Ok(keys)
    }

    /// Collects, for each WANTED `content_key`, every `history_id` that maps to it (W-AI-5 hydration).
    ///
    /// The inverse the semantic-search hydration needs: a deduped result vector (a `content_key` the
    /// [`super::vector_index::FlatVectorIndex`] returned) must fan back out to its visits so search can
    /// pick a representative (most-recent) visit per result page. ONE streaming pass keeps only the
    /// requested keys' visits (the candidate set is bounded by `k`, never the 14.4M map), so memory is
    /// bounded by the result fan-out — never the whole map. A key with no mapped visit (an orphan
    /// vector) is simply absent from the returned map; the caller drops it. An absent map returns an
    /// empty map (a never-built index hydrates to nothing, not an error).
    pub fn history_ids_for_content_keys(
        &self,
        wanted: &HashSet<u64>,
    ) -> Result<HashMap<u64, Vec<i64>>> {
        let mut inverse: HashMap<u64, Vec<i64>> = HashMap::new();
        if wanted.is_empty() {
            return Ok(inverse);
        }
        self.for_each_record(|history_id, content_key| {
            if wanted.contains(&content_key) {
                inverse.entry(content_key).or_default().push(history_id);
            }
        })?;
        Ok(inverse)
    }

    /// Deletes the map file if it exists, returning whether a file was removed.
    pub fn delete(&self) -> Result<bool> {
        if !self.exists() {
            return Ok(false);
        }
        fs::remove_file(&self.path)
            .with_context(|| format!("removing visit map {}", self.path.display()))?;
        Ok(true)
    }

    /// Strides record-by-record, invoking `sink` for each `(history_id, content_key)`.
    ///
    /// One reusable 16-byte record buffer, so memory stays bounded regardless of map size. An absent
    /// file is an empty map (the caller's set/map simply stays empty); a torn trailing record errors.
    fn for_each_record<F: FnMut(i64, u64)>(&self, mut sink: F) -> Result<()> {
        if !self.exists() {
            return Ok(());
        }
        let file = File::open(&self.path)
            .with_context(|| format!("opening visit map {}", self.path.display()))?;
        let mut reader = BufReader::new(file);
        let mut magic = [0u8; 8];
        reader.read_exact(&mut magic).context("reading visit map magic")?;
        if magic != VISIT_MAP_MAGIC {
            anyhow::bail!(
                "visit map {} has an unrecognized magic/format-version header",
                self.path.display()
            );
        }
        let mut record = [0u8; RECORD_LEN];
        loop {
            match read_exact_or_eof(&mut reader, &mut record)? {
                ReadOutcome::Eof => break,
                ReadOutcome::Partial(read) => {
                    anyhow::bail!(
                        "visit map {} ends with a partial record ({read} of {RECORD_LEN} bytes)",
                        self.path.display()
                    );
                }
                ReadOutcome::Full => {
                    let history_id =
                        i64::from_le_bytes(record[..8].try_into().expect("8-byte id slice"));
                    let content_key =
                        u64::from_le_bytes(record[8..].try_into().expect("8-byte key slice"));
                    sink(history_id, content_key);
                }
            }
        }
        Ok(())
    }
}

/// Outcome of reading exactly one fixed-width record's worth of bytes.
enum ReadOutcome {
    Full,
    Eof,
    Partial(usize),
}

/// Reads exactly `buffer.len()` bytes, distinguishing clean EOF from a torn mid-record read.
fn read_exact_or_eof<R: Read>(reader: &mut R, buffer: &mut [u8]) -> Result<ReadOutcome> {
    let mut filled = 0;
    while filled < buffer.len() {
        match reader.read(&mut buffer[filled..]).context("reading visit map record")? {
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

/// Reports total bytes consumed by all `.pkmap` files under the vector plane.
pub fn visit_map_plane_bytes(paths: &ProjectPaths) -> u64 {
    let Ok(entries) = fs::read_dir(&paths.vectors_dir) else {
        return 0;
    };
    entries
        .flatten()
        .filter(|entry| {
            entry.path().extension().and_then(|ext| ext.to_str()) == Some(VISIT_MAP_EXTENSION)
        })
        .filter_map(|entry| entry.metadata().ok().map(|meta| meta.len()))
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::project_paths_with_root;
    use tempfile::tempdir;

    fn map(paths: &ProjectPaths) -> VisitContentMap {
        VisitContentMap::for_provider(paths, "static-embed", "model-a")
    }

    #[test]
    fn create_append_read_round_trips() {
        let dir = tempdir().expect("tempdir");
        let paths = project_paths_with_root(dir.path());
        let map = map(&paths);
        // `create` truncates the resolved path; use the provider-resolved handle's own create.
        let created = create_for(&paths, &map);
        created.append(&[(10, 0xAAAA), (20, 0xBBBB)]).expect("append 1");
        created.append(&[(30, 0xAAAA)]).expect("append 2");

        let all = created.read_all().expect("read all");
        assert_eq!(all.get(&10), Some(&0xAAAA));
        assert_eq!(all.get(&20), Some(&0xBBBB));
        // Two visits (10, 30) share content_key 0xAAAA — the dedup fan-out.
        assert_eq!(all.get(&30), Some(&0xAAAA));
        assert_eq!(all.len(), 3);

        let ids = created.mapped_history_ids().expect("ids");
        assert_eq!(ids, HashSet::from([10, 20, 30]));
        let keys = created.referenced_content_keys().expect("keys");
        assert_eq!(keys, HashSet::from([0xAAAA, 0xBBBB]));
    }

    #[test]
    fn history_ids_for_content_keys_returns_only_wanted_keys_fan_out() {
        let dir = tempdir().expect("tempdir");
        let paths = project_paths_with_root(dir.path());
        let created = create_for(&paths, &map(&paths));
        // Two visits share content_key 0xAAAA; one visit on 0xBBBB; one on an unwanted 0xCCCC.
        created.append(&[(10, 0xAAAA), (20, 0xBBBB), (30, 0xAAAA), (40, 0xCCCC)]).expect("append");

        let inverse = created
            .history_ids_for_content_keys(&HashSet::from([0xAAAA, 0xBBBB]))
            .expect("inverse");
        let mut aaaa = inverse.get(&0xAAAA).cloned().expect("aaaa present");
        aaaa.sort_unstable();
        assert_eq!(aaaa, vec![10, 30], "0xAAAA fans out to both its visits");
        assert_eq!(inverse.get(&0xBBBB), Some(&vec![20]));
        assert!(!inverse.contains_key(&0xCCCC), "unwanted keys are excluded");

        // An empty wanted set short-circuits to an empty map.
        assert!(created.history_ids_for_content_keys(&HashSet::new()).expect("empty").is_empty());
    }

    #[test]
    fn history_ids_for_content_keys_is_empty_for_absent_map() {
        let dir = tempdir().expect("tempdir");
        let paths = project_paths_with_root(dir.path());
        let handle = map(&paths);
        assert!(!handle.exists());
        assert!(
            handle
                .history_ids_for_content_keys(&HashSet::from([0x1]))
                .expect("absent inverse")
                .is_empty()
        );
    }

    #[test]
    fn empty_append_is_noop() {
        let dir = tempdir().expect("tempdir");
        let paths = project_paths_with_root(dir.path());
        let created = create_for(&paths, &map(&paths));
        created.append(&[]).expect("empty append");
        assert!(created.read_all().expect("read").is_empty());
    }

    #[test]
    fn append_requires_existing_map() {
        let dir = tempdir().expect("tempdir");
        let paths = project_paths_with_root(dir.path());
        ensure_paths(&paths).expect("ensure");
        let handle = map(&paths);
        let error = handle.append(&[(1, 1)]).expect_err("missing map");
        assert!(error.to_string().contains("does not exist"));
    }

    #[test]
    fn read_all_last_writer_wins_for_remapped_visit() {
        let dir = tempdir().expect("tempdir");
        let paths = project_paths_with_root(dir.path());
        let created = create_for(&paths, &map(&paths));
        created.append(&[(10, 0x1111)]).expect("append 1");
        // A torn resume re-maps visit 10 to a NEW content_key (title changed): last writer wins.
        created.append(&[(10, 0x2222)]).expect("append 2");
        assert_eq!(created.read_all().expect("read").get(&10), Some(&0x2222));
    }

    #[test]
    fn read_rejects_partial_trailing_record() {
        let dir = tempdir().expect("tempdir");
        let paths = project_paths_with_root(dir.path());
        let created = create_for(&paths, &map(&paths));
        created.append(&[(1, 1)]).expect("append");
        let len = created.path().metadata().expect("meta").len();
        let file = OpenOptions::new().write(true).open(created.path()).expect("open");
        file.set_len(len - 3).expect("truncate");
        let error = created.read_all().expect_err("partial record");
        assert!(error.to_string().contains("partial record"));
    }

    #[test]
    fn read_rejects_bad_magic() {
        let dir = tempdir().expect("tempdir");
        let paths = project_paths_with_root(dir.path());
        ensure_paths(&paths).expect("ensure");
        let handle = map(&paths);
        fs::write(handle.path(), b"NOTPKMAPxxxxxxxx").expect("write garbage");
        assert!(
            handle.read_all().expect_err("bad magic").to_string().contains("unrecognized magic")
        );
    }

    #[test]
    fn absent_map_reads_empty() {
        let dir = tempdir().expect("tempdir");
        let paths = project_paths_with_root(dir.path());
        let handle = map(&paths);
        assert!(!handle.exists());
        assert!(handle.read_all().expect("read").is_empty());
        assert!(handle.mapped_history_ids().expect("ids").is_empty());
        assert!(handle.referenced_content_keys().expect("keys").is_empty());
    }

    #[test]
    fn delete_removes_map_and_reports_state() {
        let dir = tempdir().expect("tempdir");
        let paths = project_paths_with_root(dir.path());
        let created = create_for(&paths, &map(&paths));
        assert!(created.exists());
        assert!(created.delete().expect("delete"));
        assert!(!created.exists());
        assert!(!created.delete().expect("second delete"));
    }

    #[test]
    fn plane_bytes_sums_only_pkmap_files() {
        let dir = tempdir().expect("tempdir");
        let paths = project_paths_with_root(dir.path());
        assert_eq!(visit_map_plane_bytes(&paths), 0);
        let created = create_for(&paths, &map(&paths));
        created.append(&[(1, 1)]).expect("append");
        fs::write(paths.vectors_dir.join("notes.txt"), b"ignore").expect("write txt");
        let bytes = visit_map_plane_bytes(&paths);
        assert_eq!(bytes, created.path().metadata().expect("meta").len());
        assert!(bytes > 0);
    }

    #[test]
    fn record_bytes_are_little_endian_id_then_key() {
        let dir = tempdir().expect("tempdir");
        let paths = project_paths_with_root(dir.path());
        let created = create_for(&paths, &map(&paths));
        created.append(&[(0x0102_0304_0506_0708, 0x1112_1314_1516_1718)]).expect("append");
        let bytes = fs::read(created.path()).expect("read file");
        let record = &bytes[bytes.len() - RECORD_LEN..];
        assert_eq!(&record[..8], &0x0102_0304_0506_0708_i64.to_le_bytes());
        assert_eq!(&record[8..], &0x1112_1314_1516_1718_u64.to_le_bytes());
    }

    #[test]
    fn for_provider_path_uses_stable_table_name() {
        let dir = tempdir().expect("tempdir");
        let paths = project_paths_with_root(dir.path());
        let handle = VisitContentMap::for_provider(&paths, "static-in-app", "minishlab/potion");
        let name = handle.path().file_name().expect("name").to_string_lossy().to_string();
        assert!(name.ends_with(".pkmap"));
    }

    /// Ensures the provider-resolved map exists via the real `ensure_created` (the production path),
    /// returning a fresh handle to the same path — mirrors how `indexing.rs` uses it.
    fn create_for(paths: &ProjectPaths, handle: &VisitContentMap) -> VisitContentMap {
        handle.ensure_created(paths).expect("ensure created");
        VisitContentMap::for_provider(paths, "static-embed", "model-a")
    }

    #[test]
    fn ensure_created_is_idempotent_and_preserves_prior_records() {
        let dir = tempdir().expect("tempdir");
        let paths = project_paths_with_root(dir.path());
        let handle = map(&paths);
        handle.ensure_created(&paths).expect("first create");
        handle.append(&[(1, 0x9)]).expect("append");
        // A second ensure_created on an existing map must NOT wipe the prior record (resume safety).
        handle.ensure_created(&paths).expect("idempotent create");
        assert_eq!(handle.read_all().expect("read").get(&1), Some(&0x9));
    }
}
