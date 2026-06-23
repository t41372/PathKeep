//! Keyed reverse/forward visit↔content sidecars over `derived/vectors/` (M-11, 05 §10).
//!
//! ## Responsibilities
//! - turn the append-ordered `.pkmap` ([`super::visit_content_map::VisitContentMap`]) into two
//!   SORTED, fixed-width sidecars that answer the two query-time joins in BOUNDED time instead of a
//!   full corpus stride:
//!   - the **reverse sidecar** (`.pkrev`): `(content_key, history_id)` records sorted by
//!     `(content_key, history_id)`, so semantic hydration resolves the ~k' result content_keys to
//!     their visits by BINARY SEARCH on disk — O(k' · log n) seeks, NOT the O(n) `.pkmap` stride the
//!     always-on hydration path used (M-11). A content_key's visits form ONE contiguous run, so a
//!     single binary search to the run start plus a forward scan collects every history_id.
//!   - the **forward sidecar** (`.pkfwd`): `(history_id, content_key)` records sorted by
//!     `(history_id, content_key)`, so the `is:starred` facet resolves a BOUNDED starred history_id
//!     set to its content_keys by binary search — O(starred · log n), closing XA-PERF-4 (the
//!     acknowledged O(n) forward stride in `resolve_starred_content_keys`).
//! - stay DERIVED + REBUILDABLE: both sidecars are pure projections of the `.pkmap`, stamped with the
//!   SAME [`EmbeddingFingerprint`] hash as the `.pkvec`/planes so a model/dim change marks them stale
//!   and they re-project (zero training), and EXPORT-EXCLUDED like every `derived/vectors/` plane.
//!
//! ## Not responsible for
//! - the append-ordered authoritative `.pkmap` (this is a downstream sorted projection of it, never a
//!   source of truth — a torn/missing sidecar re-projects from the `.pkmap`);
//! - the vectors / dedup hashing / nearest-neighbour search (those stay with their owners).
//!
//! ## Why a persisted SORTED sidecar instead of a resident `HashMap`
//! At the 14.4M-visit tail the `.pkmap` is per-VISIT (it does NOT shrink with dedup), so a resident
//! `HashMap<content_key, Vec<history_id>>` is ~300–460 MB stacked on top of the ~879 MB binary plane —
//! a meaningful bite of the 8 GB envelope when the WebView + SQLCipher cache + a running embedder also
//! need room (Principle 3). The whole retrieval plane's posture is "binary-plane-only resident; int8
//! seeked on disk" (05 §10 C1); a resident reverse map would break it. So these sidecars MIRROR the
//! int8 plane exactly: fixed-width records, sorted, BINARY-SEARCHED by file position with ~0 added
//! resident RAM. Hydration touches only the k' result keys' records; is:starred only the starred set's.
//!
//! ## Format (little-endian; both sidecars mirror the `.pkvec` magic+header discipline)
//! ```text
//! .pkrev  magic 8 = b"PKREV\0\x01\0" | u32 len + JSON ReverseMapHeader | records [ content_key:u64 | history_id:i64 ]  sorted by (content_key, history_id)
//! .pkfwd  magic 8 = b"PKFWD\0\x01\0" | u32 len + JSON ReverseMapHeader | records [ history_id:i64  | content_key:u64 ]  sorted by (history_id, content_key)
//! ```
//! Each record is a fixed 16 bytes (two u64/i64). The leading 8 bytes are the SORT KEY of that
//! sidecar (content_key for `.pkrev`, history_id for `.pkfwd`), so a binary search compares only the
//! first 8 bytes of each probed record. The header carries the source fingerprint hash + the record
//! COUNT (so the binary search bound is read without a metadata round-trip per probe).

use super::visit_content_map::VisitContentMap;
use crate::ai_sidecar::provider_table_name;
use crate::config::{ProjectPaths, ensure_paths};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs::{self, File};
use std::io::{BufWriter, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

/// Magic + format-version prefix for the content_key-sorted reverse sidecar.
const REVERSE_MAP_MAGIC: [u8; 8] = *b"PKREV\0\x01\0";
/// Magic + format-version prefix for the history_id-sorted forward sidecar.
const FORWARD_MAP_MAGIC: [u8; 8] = *b"PKFWD\0\x01\0";

/// File extension for the content_key-sorted reverse sidecar (hydration).
const REVERSE_MAP_EXTENSION: &str = "pkrev";
/// File extension for the history_id-sorted forward sidecar (`is:starred`).
const FORWARD_MAP_EXTENSION: &str = "pkfwd";

/// Fixed record width: an 8-byte sort key + an 8-byte payload (u64 content_key + i64 history_id).
const RECORD_LEN: usize = 16;

/// JSON header persisted after each sidecar's magic; ties it to its source fingerprint + record count.
///
/// `fingerprint_hash` is the [`EmbeddingFingerprint::hash`] the `.pkvec`/`.pkmap` carried when this
/// sidecar was projected, so a staleness check is a string compare (mirrors the `.pkvec`/plane
/// headers). `record_count` is the number of fixed-width records so a binary search knows its upper
/// bound from the header alone — no `metadata()` round-trip per probe.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReverseMapHeader {
    /// The embedding fingerprint hash the source carried when this sidecar was projected.
    pub fingerprint_hash: String,
    /// Number of fixed-width records (the binary-search upper bound).
    pub record_count: u64,
}

/// Resolves the reverse (`.pkrev`) sidecar path for one provider/model pair under `derived/vectors/`.
fn reverse_map_path(paths: &ProjectPaths, provider_id: &str, model: &str) -> PathBuf {
    let file = format!("{}.{REVERSE_MAP_EXTENSION}", provider_table_name(provider_id, model));
    paths.vectors_dir.join(file)
}

/// Resolves the forward (`.pkfwd`) sidecar path for one provider/model pair under `derived/vectors/`.
fn forward_map_path(paths: &ProjectPaths, provider_id: &str, model: &str) -> PathBuf {
    let file = format!("{}.{FORWARD_MAP_EXTENSION}", provider_table_name(provider_id, model));
    paths.vectors_dir.join(file)
}

/// A handle to one provider/model keyed reverse/forward sidecar pair on the `derived/vectors/` plane.
///
/// Cheap to construct (resolves both paths only); all I/O is in the explicit methods so the build step
/// (project from `.pkmap`) and the query-time binary searches stay separate concerns, mirroring the
/// [`super::vector_planes`] handles.
pub struct ReverseVisitMap {
    reverse_path: PathBuf,
    forward_path: PathBuf,
}

impl ReverseVisitMap {
    /// Resolves the sidecar-pair handle for one provider/model pair.
    pub fn for_provider(paths: &ProjectPaths, provider_id: &str, model: &str) -> Self {
        Self {
            reverse_path: reverse_map_path(paths, provider_id, model),
            forward_path: forward_map_path(paths, provider_id, model),
        }
    }

    /// Returns the on-disk path of the content_key-sorted reverse (`.pkrev`) sidecar.
    pub fn reverse_path(&self) -> &Path {
        &self.reverse_path
    }

    /// Returns the on-disk path of the history_id-sorted forward (`.pkfwd`) sidecar.
    pub fn forward_path(&self) -> &Path {
        &self.forward_path
    }

    /// Returns whether BOTH sidecars exist on disk.
    ///
    /// Both are written atomically in one [`Self::build_from_visit_map`] pass, so "one present, the
    /// other absent" is a half-finished/torn projection — treated as not-present so the read-path guard
    /// re-projects rather than serving a half-built pair.
    pub fn exists(&self) -> bool {
        self.reverse_path.exists() && self.forward_path.exists()
    }

    /// Whether the keyed sidecars are stale against the live `.pkvec` fingerprint (need re-projection).
    ///
    /// Stale when EITHER sidecar is missing, OR the reverse sidecar's stamped fingerprint hash differs
    /// from `live`, OR the two sidecars carry different record counts (a torn/mismatched pair). This is
    /// the read-path guard's seam: a keyed lookup is only trusted when both sidecars are present, in
    /// lockstep, and stamped for the current embedding config — otherwise the caller re-projects from
    /// the authoritative `.pkmap` (consistent with the `.pkvec`/plane fingerprint-staleness handling).
    pub fn is_stale_against(&self, live_fingerprint_hash: &str) -> Result<bool> {
        let (Some(reverse), Some(forward)) = (
            read_sidecar_header(&self.reverse_path, &REVERSE_MAP_MAGIC)?,
            read_sidecar_header(&self.forward_path, &FORWARD_MAP_MAGIC)?,
        ) else {
            return Ok(true);
        };
        Ok(reverse.fingerprint_hash != live_fingerprint_hash
            || reverse.record_count != forward.record_count)
    }

    /// Projects BOTH sorted sidecars from the authoritative append-ordered `.pkmap` (M-11).
    ///
    /// Reads the `.pkmap`'s last-writer-wins `history_id → content_key` map (so a torn-resume duplicate
    /// is already collapsed — the SAME dedup the `.pkvec` reader applies), then writes:
    /// - `.pkrev`: every `(content_key, history_id)` sorted by `(content_key, history_id)`;
    /// - `.pkfwd`: every `(history_id, content_key)` sorted by `(history_id, content_key)`.
    ///
    /// Both are written to a temp file + atomically renamed so an interrupted projection never leaves a
    /// torn sidecar a binary search would mis-probe, and both are stamped with `fingerprint_hash` so
    /// [`Self::is_stale_against`] flips false right after. RESUMABLE/RESTART-SAFE: the `.pkmap` is the
    /// single source of truth, so re-projecting at any point (a fresh build, a resumed build, a
    /// stale-sidecar rebuild) reads the CURRENT `.pkmap` state and produces the SAME correct sidecars —
    /// no dup (the `.pkmap` map collapses duplicates; one record per history_id ⇒ one `.pkfwd` row and
    /// exactly one `.pkrev` row per visit), no miss (every mapped visit is projected).
    pub fn build_from_visit_map(
        &self,
        paths: &ProjectPaths,
        visit_map: &VisitContentMap,
        fingerprint_hash: &str,
    ) -> Result<u64> {
        ensure_paths(paths)?;
        // `read_all` is last-writer-wins per history_id, so a torn-resume re-map is already collapsed:
        // one (history_id, content_key) pair per visit — the dedup parity the `.pkvec` reader holds.
        let map = visit_map.read_all()?;
        let mut pairs: Vec<(i64, u64)> = map.into_iter().collect();
        let record_count = pairs.len() as u64;

        // `.pkrev` sorted by (content_key, history_id): a content_key's visits form ONE contiguous run
        // so hydration binary-searches the run start then forward-scans it. The history_id sub-sort
        // makes the on-disk byte layout deterministic (stable across rebuilds, golden-vector testable).
        pairs.sort_unstable_by(|left, right| left.1.cmp(&right.1).then(left.0.cmp(&right.0)));
        write_sidecar_atomic(
            &self.reverse_path,
            &REVERSE_MAP_MAGIC,
            fingerprint_hash,
            record_count,
            pairs.iter().map(|&(history_id, content_key)| (content_key, history_id as u64)),
        )?;

        // `.pkfwd` sorted by (history_id, content_key): the bounded starred history_id set binary-
        // searches here. history_id is unique in the `.pkmap` map (last-writer-wins), so each is one row.
        // CRUCIAL: sort by the SAME u64-reinterpretation `lower_bound` compares with (the leading 8
        // bytes are the history_id stored bit-for-bit as a u64), so the on-disk order is exactly the
        // search's comparison order — correct UNCONDITIONALLY, not just because `visits.id` ROWIDs
        // happen to be positive. (`.pkrev`'s history_id is only a SUB-sort within a content_key run, so
        // its order never affects the search, which keys solely on the leading content_key.)
        pairs.sort_unstable_by(|left, right| {
            (left.0 as u64).cmp(&(right.0 as u64)).then(left.1.cmp(&right.1))
        });
        write_sidecar_atomic(
            &self.forward_path,
            &FORWARD_MAP_MAGIC,
            fingerprint_hash,
            record_count,
            pairs.iter().map(|&(history_id, content_key)| (history_id as u64, content_key)),
        )?;
        Ok(record_count)
    }

    /// Collects, for each WANTED `content_key`, every `history_id` that maps to it (M-11 hydration).
    ///
    /// The bounded replacement for `VisitContentMap::history_ids_for_content_keys`'s O(n) stride: for
    /// each of the ~k' wanted keys it BINARY-SEARCHES the `.pkrev` to the first record carrying that key,
    /// then forward-reads the contiguous run of that key's records (sorted layout) collecting their
    /// history_ids — O(k' · log n + matched_visits) seeks/reads, never the corpus. Returns EXACTLY the
    /// history_ids the old full scan returned for the same key set (the sorted projection is the SAME
    /// `(content_key, history_id)` multiset as the `.pkmap`'s last-writer-wins map). A key with no
    /// mapped visit is simply absent from the result; an empty wanted set short-circuits.
    pub fn history_ids_for_content_keys(
        &self,
        wanted: &HashSet<u64>,
    ) -> Result<std::collections::HashMap<u64, Vec<i64>>> {
        let mut inverse: std::collections::HashMap<u64, Vec<i64>> =
            std::collections::HashMap::new();
        if wanted.is_empty() {
            return Ok(inverse);
        }
        let Some((mut file, header)) = open_sidecar(&self.reverse_path, &REVERSE_MAP_MAGIC)? else {
            return Ok(inverse);
        };
        let data_start = data_start_offset(&header)?;
        for &key in wanted {
            // The reverse sidecar's leading 8 bytes are the content_key (the sort key); binary-search
            // for the FIRST record with this key, then forward-scan its contiguous run.
            let mut position = lower_bound(&mut file, data_start, header.record_count, key)?;
            while position < header.record_count {
                let record = read_record_at(&mut file, data_start, position)?;
                let record_key = u64::from_le_bytes(record[..8].try_into().expect("8-byte key"));
                if record_key != key {
                    break; // Past this content_key's contiguous run.
                }
                let history_id = i64::from_le_bytes(record[8..].try_into().expect("8-byte id"));
                inverse.entry(key).or_default().push(history_id);
                position += 1;
            }
        }
        Ok(inverse)
    }

    /// Collects the SET of `content_key`s for a BOUNDED set of `history_id`s (XA-PERF-4, `is:starred`).
    ///
    /// The bounded replacement for `VisitContentMap::content_keys_for_history_ids`'s O(n) stride: for
    /// each of the (tiny) starred history_ids it BINARY-SEARCHES the `.pkfwd` to that history_id's row
    /// and reads its content_key — O(starred · log n) seeks, never the corpus. history_id is unique in
    /// the `.pkmap` map, so each starred visit contributes at most one content_key; a starred visit not
    /// in the map (never embedded) contributes nothing. An empty wanted set short-circuits.
    pub fn content_keys_for_history_ids(&self, wanted: &HashSet<i64>) -> Result<HashSet<u64>> {
        let mut keys = HashSet::new();
        if wanted.is_empty() {
            return Ok(keys);
        }
        let Some((mut file, header)) = open_sidecar(&self.forward_path, &FORWARD_MAP_MAGIC)? else {
            return Ok(keys);
        };
        let data_start = data_start_offset(&header)?;
        for &history_id in wanted {
            // The forward sidecar's leading 8 bytes are the history_id (the sort key, reinterpreted as
            // a u64 for the byte compare — see `lower_bound`'s ordering note). Binary-search to the
            // first matching row; a unique history_id has exactly one row, so read it and take its key.
            let probe = history_id as u64;
            let position = lower_bound(&mut file, data_start, header.record_count, probe)?;
            if position < header.record_count {
                let record = read_record_at(&mut file, data_start, position)?;
                let record_id = i64::from_le_bytes(record[..8].try_into().expect("8-byte id"));
                if record_id == history_id {
                    let content_key =
                        u64::from_le_bytes(record[8..].try_into().expect("8-byte key"));
                    keys.insert(content_key);
                }
            }
        }
        Ok(keys)
    }

    /// Deletes both sidecars if present, returning whether anything was removed.
    ///
    /// Used by the index "clear"/full-rebuild path; the sidecars are pure projections so a clean delete
    /// is the correct reset (a later build re-projects them from the surviving `.pkmap`).
    pub fn delete(&self) -> Result<bool> {
        let mut removed = false;
        for path in [&self.reverse_path, &self.forward_path] {
            if path.exists() {
                fs::remove_file(path)
                    .with_context(|| format!("removing reverse sidecar {}", path.display()))?;
                removed = true;
            }
        }
        Ok(removed)
    }
}

/// Reports total bytes consumed by all `.pkrev` + `.pkfwd` sidecars under the vector plane dir.
///
/// Filesystem-only so storage review can size the keyed sidecars without loading any engine.
pub fn reverse_map_plane_bytes(paths: &ProjectPaths) -> u64 {
    let Ok(entries) = fs::read_dir(&paths.vectors_dir) else {
        return 0;
    };
    entries
        .flatten()
        .filter(|entry| {
            matches!(
                entry.path().extension().and_then(|ext| ext.to_str()),
                Some(REVERSE_MAP_EXTENSION) | Some(FORWARD_MAP_EXTENSION)
            )
        })
        .filter_map(|entry| entry.metadata().ok().map(|meta| meta.len()))
        .sum()
}

/// Writes one sorted sidecar to a temp file + atomically renames it over the destination.
///
/// `records` MUST already be in the sidecar's sort order (the caller sorts once per direction). Each
/// `(sort_key, payload)` is written as two little-endian 8-byte words (the sort key first, so a binary
/// search compares only the leading 8 bytes). The atomic temp+rename means a reader never sees a
/// half-written sidecar a binary search would mis-probe.
fn write_sidecar_atomic(
    path: &Path,
    magic: &[u8; 8],
    fingerprint_hash: &str,
    record_count: u64,
    records: impl Iterator<Item = (u64, u64)>,
) -> Result<()> {
    let tmp = sidecar_tmp_path(path);
    {
        let file = File::create(&tmp)
            .with_context(|| format!("creating reverse sidecar temp {}", tmp.display()))?;
        let mut writer = BufWriter::new(file);
        writer.write_all(magic).context("writing reverse sidecar magic")?;
        let header =
            ReverseMapHeader { fingerprint_hash: fingerprint_hash.to_string(), record_count };
        let json = serde_json::to_vec(&header).context("serializing reverse sidecar header")?;
        let len = u32::try_from(json.len()).context("reverse sidecar header too large")?;
        writer.write_all(&len.to_le_bytes()).context("writing reverse sidecar header length")?;
        writer.write_all(&json).context("writing reverse sidecar header body")?;
        let mut buffer = [0u8; RECORD_LEN];
        for (sort_key, payload) in records {
            buffer[..8].copy_from_slice(&sort_key.to_le_bytes());
            buffer[8..].copy_from_slice(&payload.to_le_bytes());
            writer.write_all(&buffer).context("writing reverse sidecar record")?;
        }
        writer.flush().context("flushing reverse sidecar")?;
    }
    fs::rename(&tmp, path)
        .with_context(|| format!("finalizing reverse sidecar {}", path.display()))?;
    Ok(())
}

/// Resolves the per-sidecar temp path by appending `.tmp` to the FULL file name (extension included).
///
/// Appending (rather than `with_extension`) keeps each sidecar's temp distinct from the other's even
/// though they share a base name and differ only by extension (`<name>.pkrev` → `<name>.pkrev.tmp`).
/// `reverse_map_plane_bytes` ignores `.tmp`, so a stray temp never inflates the storage report.
fn sidecar_tmp_path(path: &Path) -> PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(".tmp");
    path.with_file_name(name)
}

/// Reads + validates a sidecar's magic + JSON header, returning `None` when the file is absent.
fn read_sidecar_header(path: &Path, magic: &[u8; 8]) -> Result<Option<ReverseMapHeader>> {
    if !path.exists() {
        return Ok(None);
    }
    let (_, header) = open_sidecar(path, magic)?.expect("present file opens");
    Ok(Some(header))
}

/// Opens a sidecar, validates its magic, and returns its open file + header (`None` when absent).
///
/// The returned [`File`] is positioned anywhere (every read seeks absolutely), so callers use
/// [`read_record_at`] / [`lower_bound`] which seek by computed offset. An absent file is `None` (a
/// never-built sidecar reads as empty), a present-but-corrupt file errors.
#[allow(clippy::type_complexity)]
fn open_sidecar(path: &Path, magic: &[u8; 8]) -> Result<Option<(File, ReverseMapHeader)>> {
    if !path.exists() {
        return Ok(None);
    }
    let mut file =
        File::open(path).with_context(|| format!("opening reverse sidecar {}", path.display()))?;
    let mut found = [0u8; 8];
    file.read_exact(&mut found).context("reading reverse sidecar magic")?;
    if &found != magic {
        anyhow::bail!(
            "reverse sidecar {} has an unrecognized magic/format-version header",
            path.display()
        );
    }
    let mut len_bytes = [0u8; 4];
    file.read_exact(&mut len_bytes).context("reading reverse sidecar header length")?;
    let len = u32::from_le_bytes(len_bytes) as usize;
    let mut json = vec![0u8; len];
    file.read_exact(&mut json).context("reading reverse sidecar header body")?;
    let header: ReverseMapHeader =
        serde_json::from_slice(&json).context("parsing reverse sidecar header")?;
    Ok(Some((file, header)))
}

/// Byte offset of the first record = magic (8) + length prefix (4) + the JSON header bytes.
///
/// Recomputed from the header (re-serialized to the SAME bytes `write_sidecar_atomic` wrote) so a
/// binary-search probe seeks straight to `data_start + position * RECORD_LEN` without re-reading the
/// header each time.
fn data_start_offset(header: &ReverseMapHeader) -> Result<u64> {
    let json = serde_json::to_vec(header).context("re-serializing reverse sidecar header")?;
    Ok(8 + 4 + json.len() as u64)
}

/// Reads the fixed-width record at `position` (0-based) by absolute seek.
fn read_record_at(file: &mut File, data_start: u64, position: u64) -> Result<[u8; RECORD_LEN]> {
    let offset = data_start + position * RECORD_LEN as u64;
    file.seek(SeekFrom::Start(offset)).context("seeking reverse sidecar record")?;
    let mut record = [0u8; RECORD_LEN];
    file.read_exact(&mut record).context("reading reverse sidecar record")?;
    Ok(record)
}

/// Returns the position of the FIRST record whose leading-8-byte sort key is `>= probe`.
///
/// A classic on-disk binary search over `record_count` fixed-width records: O(log n) seeks, each
/// reading one record's leading 8 bytes. Returns `record_count` when every key is `< probe` (the probe
/// is past the end). The sort key is compared as a `u64`: for `.pkrev` it IS the u64 content_key; for
/// `.pkfwd` the i64 history_id was stored bit-reinterpreted as a u64 AND the file was SORTED by that
/// SAME `history_id as u64` order ([`ReverseVisitMap::build_from_visit_map`]), so probing with
/// `history_id as u64` is correct regardless of sign — the on-disk order and the probe order are the
/// same total order by construction. The caller still equality-checks the located record before
/// trusting it (a probe with no matching row lands on the next-greater key).
fn lower_bound(file: &mut File, data_start: u64, record_count: u64, probe: u64) -> Result<u64> {
    let mut low = 0u64;
    let mut high = record_count;
    while low < high {
        let mid = low + (high - low) / 2;
        let record = read_record_at(file, data_start, mid)?;
        let key = u64::from_le_bytes(record[..8].try_into().expect("8-byte key"));
        if key < probe {
            low = mid + 1;
        } else {
            high = mid;
        }
    }
    Ok(low)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::project_paths_with_root;
    use tempfile::tempdir;

    fn seeded_visit_map(paths: &ProjectPaths, records: &[(i64, u64)]) -> VisitContentMap {
        let map = VisitContentMap::for_provider(paths, "static-embed", "model-a");
        map.ensure_created(paths).expect("ensure created");
        map.append(records).expect("append");
        map
    }

    fn build(paths: &ProjectPaths, records: &[(i64, u64)], fingerprint: &str) -> ReverseVisitMap {
        let visit_map = seeded_visit_map(paths, records);
        let sidecars = ReverseVisitMap::for_provider(paths, "static-embed", "model-a");
        sidecars.build_from_visit_map(paths, &visit_map, fingerprint).expect("build sidecars");
        sidecars
    }

    #[test]
    fn hydration_returns_exactly_the_old_full_scan_result() {
        // M-11 correctness: the binary-search hydration must return EXACTLY the history_ids the old
        // O(n) `.pkmap` stride returned for the same content_key set — a page with multiple visits fans
        // out to ALL of them (no miss), and an unwanted key is excluded (no extra).
        let dir = tempdir().expect("tempdir");
        let paths = project_paths_with_root(dir.path());
        // Two visits share content_key 0xAAAA; one on 0xBBBB; one on an unwanted 0xCCCC.
        let records = [(10, 0xAAAA), (20, 0xBBBB), (30, 0xAAAA), (40, 0xCCCC)];
        let sidecars = build(&paths, &records, "fp-1");

        let inverse = sidecars
            .history_ids_for_content_keys(&HashSet::from([0xAAAA, 0xBBBB]))
            .expect("inverse");
        let mut aaaa = inverse.get(&0xAAAA).cloned().expect("aaaa present");
        aaaa.sort_unstable();
        assert_eq!(aaaa, vec![10, 30], "0xAAAA fans out to BOTH its visits (no miss)");
        assert_eq!(inverse.get(&0xBBBB), Some(&vec![20]));
        assert!(!inverse.contains_key(&0xCCCC), "an unwanted key is excluded");

        // The keyed result is identical to the authoritative full-scan `.pkmap` result.
        let visit_map = VisitContentMap::for_provider(&paths, "static-embed", "model-a");
        let scan =
            visit_map.history_ids_for_content_keys(&HashSet::from([0xAAAA, 0xBBBB])).expect("scan");
        let mut keyed_sorted: Vec<(u64, Vec<i64>)> = inverse
            .into_iter()
            .map(|(key, mut ids)| {
                ids.sort_unstable();
                (key, ids)
            })
            .collect();
        keyed_sorted.sort_unstable_by_key(|(key, _)| *key);
        let mut scan_sorted: Vec<(u64, Vec<i64>)> = scan
            .into_iter()
            .map(|(key, mut ids)| {
                ids.sort_unstable();
                (key, ids)
            })
            .collect();
        scan_sorted.sort_unstable_by_key(|(key, _)| *key);
        assert_eq!(keyed_sorted, scan_sorted, "keyed hydration must equal the full scan exactly");
    }

    #[test]
    fn hydration_empty_wanted_short_circuits_and_missing_key_absent() {
        let dir = tempdir().expect("tempdir");
        let paths = project_paths_with_root(dir.path());
        let sidecars = build(&paths, &[(1, 0x1), (2, 0x2)], "fp-1");
        // Empty wanted set short-circuits to an empty map (no file read).
        assert!(sidecars.history_ids_for_content_keys(&HashSet::new()).expect("empty").is_empty());
        // A content_key with no mapped visit is simply absent from the result.
        let inverse =
            sidecars.history_ids_for_content_keys(&HashSet::from([0xDEAD])).expect("missing key");
        assert!(inverse.is_empty(), "a never-mapped content_key yields no entry");
    }

    #[test]
    fn forward_resolution_is_bounded_and_matches_full_scan() {
        // XA-PERF-4: the `is:starred` forward direction must return the same content_key SET the old
        // O(n) `content_keys_for_history_ids` stride returned for the same bounded history_id set.
        let dir = tempdir().expect("tempdir");
        let paths = project_paths_with_root(dir.path());
        let records = [(10, 0xAAAA), (20, 0xBBBB), (30, 0xAAAA), (40, 0xCCCC)];
        let sidecars = build(&paths, &records, "fp-1");

        let keys =
            sidecars.content_keys_for_history_ids(&HashSet::from([10, 20, 30])).expect("forward");
        assert_eq!(keys, HashSet::from([0xAAAA, 0xBBBB]), "0xAAAA deduped across visits 10+30");
        assert!(!keys.contains(&0xCCCC), "visit 40's key is excluded (not wanted)");

        // Identical to the authoritative full-scan `.pkmap` forward result.
        let visit_map = VisitContentMap::for_provider(&paths, "static-embed", "model-a");
        let scan =
            visit_map.content_keys_for_history_ids(&HashSet::from([10, 20, 30])).expect("scan");
        assert_eq!(keys, scan, "keyed forward must equal the full scan exactly");

        // Empty wanted set short-circuits; a never-mapped history_id contributes nothing.
        assert!(sidecars.content_keys_for_history_ids(&HashSet::new()).expect("empty").is_empty());
        assert!(
            sidecars
                .content_keys_for_history_ids(&HashSet::from([999]))
                .expect("absent id")
                .is_empty()
        );
    }

    #[test]
    fn binary_search_holds_across_many_keys_and_runs() {
        // A larger corpus with multi-visit runs of varying length, so the binary search + run-scan is
        // exercised at non-trivial positions (not just the 4-record toy). Every key's fan-out and every
        // history_id's key must round-trip exactly.
        let dir = tempdir().expect("tempdir");
        let paths = project_paths_with_root(dir.path());
        let mut records: Vec<(i64, u64)> = Vec::new();
        let mut history_id = 1i64;
        // content_key k has k visits (k = 1..=20), so runs vary in length and positions interleave.
        for key in 1u64..=20 {
            for _ in 0..key {
                records.push((history_id, key));
                history_id += 1;
            }
        }
        let sidecars = build(&paths, &records, "fp-1");

        let wanted: HashSet<u64> = (1u64..=20).collect();
        let inverse = sidecars.history_ids_for_content_keys(&wanted).expect("inverse");
        for key in 1u64..=20 {
            assert_eq!(
                inverse.get(&key).map(|ids| ids.len()),
                Some(key as usize),
                "content_key {key} must fan out to exactly {key} visits"
            );
        }
        // Every history_id resolves to its content_key via the forward sidecar.
        let all_ids: HashSet<i64> = records.iter().map(|&(id, _)| id).collect();
        let keys = sidecars.content_keys_for_history_ids(&all_ids).expect("forward");
        assert_eq!(keys, (1u64..=20).collect::<HashSet<u64>>());
    }

    #[test]
    fn torn_resume_duplicate_collapses_via_last_writer_wins() {
        // RESUMABLE parity: a torn resume re-maps visit 10 to a NEW content_key; the `.pkmap`'s
        // last-writer-wins map collapses it, so the sidecar projects ONE record for visit 10 (the LAST
        // key), not two — no dup, mirroring the `.pkvec` read_all contract.
        let dir = tempdir().expect("tempdir");
        let paths = project_paths_with_root(dir.path());
        let visit_map = VisitContentMap::for_provider(&paths, "static-embed", "model-a");
        visit_map.ensure_created(&paths).expect("ensure");
        visit_map.append(&[(10, 0x1111)]).expect("append 1");
        visit_map.append(&[(10, 0x2222)]).expect("append 2 (re-map)");
        let sidecars = ReverseVisitMap::for_provider(&paths, "static-embed", "model-a");
        let count = sidecars.build_from_visit_map(&paths, &visit_map, "fp-1").expect("build");
        assert_eq!(count, 1, "the re-mapped visit collapses to one record");

        // Visit 10 resolves to the LAST content_key only; the stale 0x1111 has no visits.
        let keys = sidecars.content_keys_for_history_ids(&HashSet::from([10])).expect("forward");
        assert_eq!(keys, HashSet::from([0x2222]));
        let stale =
            sidecars.history_ids_for_content_keys(&HashSet::from([0x1111])).expect("stale key");
        assert!(stale.is_empty(), "the superseded content_key fans out to nothing");
        let live =
            sidecars.history_ids_for_content_keys(&HashSet::from([0x2222])).expect("live key");
        assert_eq!(live.get(&0x2222), Some(&vec![10]));
    }

    #[test]
    fn rebuild_from_current_visit_map_is_idempotent_no_dup_no_miss() {
        // RESTART-SAFE: re-projecting from the SAME `.pkmap` (a stale-sidecar rebuild) produces the
        // SAME sidecars — re-running the build twice never doubles or drops a record.
        let dir = tempdir().expect("tempdir");
        let paths = project_paths_with_root(dir.path());
        let records = [(10, 0xAAAA), (20, 0xBBBB), (30, 0xAAAA)];
        let visit_map = seeded_visit_map(&paths, &records);
        let sidecars = ReverseVisitMap::for_provider(&paths, "static-embed", "model-a");
        let first = sidecars.build_from_visit_map(&paths, &visit_map, "fp-1").expect("build 1");
        let first_bytes = fs::read(sidecars.reverse_path()).expect("read 1");
        let second = sidecars.build_from_visit_map(&paths, &visit_map, "fp-1").expect("build 2");
        let second_bytes = fs::read(sidecars.reverse_path()).expect("read 2");
        assert_eq!(first, second, "record count stable across rebuilds");
        assert_eq!(first_bytes, second_bytes, "the projected bytes are deterministic");
    }

    #[test]
    fn staleness_flips_on_fingerprint_change_or_missing_sidecar() {
        let dir = tempdir().expect("tempdir");
        let paths = project_paths_with_root(dir.path());
        let sidecars = ReverseVisitMap::for_provider(&paths, "static-embed", "model-a");
        // Never built → stale (missing).
        assert!(sidecars.is_stale_against("fp-1").expect("absent stale"));
        assert!(!sidecars.exists());

        build(&paths, &[(1, 0x1), (2, 0x2)], "fp-1");
        assert!(sidecars.exists());
        assert!(
            !sidecars.is_stale_against("fp-1").expect("fresh"),
            "matching fingerprint not stale"
        );
        // A changed fingerprint → stale (model/dim/dtype changed upstream).
        assert!(sidecars.is_stale_against("fp-2").expect("changed"), "different fingerprint stale");

        // Removing ONE sidecar (a torn pair) reads as not-present → stale.
        fs::remove_file(sidecars.forward_path()).expect("remove forward");
        assert!(!sidecars.exists(), "a half-present pair is treated as not present");
        assert!(sidecars.is_stale_against("fp-1").expect("half pair stale"));
    }

    #[test]
    fn staleness_flips_when_sidecar_record_counts_diverge() {
        // A mismatched pair (same fingerprint but different record counts — a torn/partial reprojection
        // where one sidecar finished and the other did not) is stale, so the read path re-projects
        // rather than trusting a pair that cannot be positionally consistent.
        let dir = tempdir().expect("tempdir");
        let paths = project_paths_with_root(dir.path());
        let sidecars = build(&paths, &[(1, 0x1), (2, 0x2)], "fp-1");
        assert!(!sidecars.is_stale_against("fp-1").expect("fresh pair"));
        // Re-project ONLY the forward sidecar from a DIFFERENT (smaller) map, so the two counts diverge
        // while the reverse keeps its stamp. The mismatch must read as stale.
        let smaller = VisitContentMap::for_provider(&paths, "static-embed", "smaller");
        smaller.ensure_created(&paths).expect("ensure");
        smaller.append(&[(1, 0x1)]).expect("append one");
        let only_forward = ReverseVisitMap::for_provider(&paths, "static-embed", "model-a");
        write_sidecar_atomic(
            only_forward.forward_path(),
            &FORWARD_MAP_MAGIC,
            "fp-1",
            1, // a count that does NOT match the reverse sidecar's 2
            std::iter::once((1u64, 0x1u64)),
        )
        .expect("rewrite forward with a diverging count");
        assert!(
            sidecars.is_stale_against("fp-1").expect("count mismatch"),
            "diverging record counts mark the pair stale"
        );
    }

    #[test]
    fn absent_sidecars_read_empty_not_error() {
        // The read-path guard relies on an absent sidecar reading as empty (so the caller re-projects
        // or falls back) rather than erroring.
        let dir = tempdir().expect("tempdir");
        let paths = project_paths_with_root(dir.path());
        let sidecars = ReverseVisitMap::for_provider(&paths, "static-embed", "never-built");
        assert!(
            sidecars
                .history_ids_for_content_keys(&HashSet::from([0x1]))
                .expect("absent reverse")
                .is_empty()
        );
        assert!(
            sidecars
                .content_keys_for_history_ids(&HashSet::from([1]))
                .expect("absent forward")
                .is_empty()
        );
    }

    #[test]
    fn empty_visit_map_projects_empty_sidecars() {
        let dir = tempdir().expect("tempdir");
        let paths = project_paths_with_root(dir.path());
        let visit_map = VisitContentMap::for_provider(&paths, "static-embed", "model-a");
        visit_map.ensure_created(&paths).expect("ensure");
        let sidecars = ReverseVisitMap::for_provider(&paths, "static-embed", "model-a");
        let count = sidecars.build_from_visit_map(&paths, &visit_map, "fp-1").expect("build empty");
        assert_eq!(count, 0);
        assert!(sidecars.exists(), "empty (header-only) sidecars still exist");
        assert!(
            sidecars
                .history_ids_for_content_keys(&HashSet::from([0x1]))
                .expect("empty reverse")
                .is_empty()
        );
        assert!(!sidecars.is_stale_against("fp-1").expect("empty fresh"));
    }

    #[test]
    fn delete_removes_both_sidecars_and_reports_state() {
        let dir = tempdir().expect("tempdir");
        let paths = project_paths_with_root(dir.path());
        let sidecars = build(&paths, &[(1, 0x1)], "fp-1");
        assert!(sidecars.exists());
        assert!(sidecars.delete().expect("delete"));
        assert!(!sidecars.reverse_path().exists());
        assert!(!sidecars.forward_path().exists());
        // Deleting absent sidecars is a harmless false.
        assert!(!sidecars.delete().expect("second delete"));
    }

    #[test]
    fn plane_bytes_sums_only_reverse_and_forward_sidecars() {
        let dir = tempdir().expect("tempdir");
        let paths = project_paths_with_root(dir.path());
        assert_eq!(reverse_map_plane_bytes(&paths), 0);
        let sidecars = build(&paths, &[(1, 0x1), (2, 0x2)], "fp-1");
        fs::write(paths.vectors_dir.join("notes.txt"), b"ignore").expect("write txt");
        let bytes = reverse_map_plane_bytes(&paths);
        let expected = sidecars.reverse_path().metadata().expect("rev meta").len()
            + sidecars.forward_path().metadata().expect("fwd meta").len();
        assert_eq!(bytes, expected);
        assert!(bytes > 0);
    }

    #[test]
    fn read_rejects_bad_magic() {
        let dir = tempdir().expect("tempdir");
        let paths = project_paths_with_root(dir.path());
        ensure_paths(&paths).expect("ensure");
        let sidecars = ReverseVisitMap::for_provider(&paths, "static-embed", "model-a");
        fs::write(sidecars.reverse_path(), b"NOTPKREVxxxxxxxx").expect("write garbage");
        let error =
            sidecars.history_ids_for_content_keys(&HashSet::from([0x1])).expect_err("bad magic");
        assert!(error.to_string().contains("unrecognized magic"));
    }

    #[test]
    fn record_bytes_are_little_endian_sort_key_then_payload() {
        // Byte-pin the on-disk layout so a symmetric write+read endianness mutant (which round-trips
        // undetected) is killed: `.pkrev` record = content_key (u64 LE) then history_id (i64 LE).
        let dir = tempdir().expect("tempdir");
        let paths = project_paths_with_root(dir.path());
        let sidecars = build(&paths, &[(0x0102_0304_0506_0708, 0x1112_1314_1516_1718)], "fp-1");
        let bytes = fs::read(sidecars.reverse_path()).expect("read file");
        let record = &bytes[bytes.len() - RECORD_LEN..];
        // Sort key first: the content_key u64.
        assert_eq!(&record[..8], &0x1112_1314_1516_1718_u64.to_le_bytes());
        // Payload: the history_id i64.
        assert_eq!(&record[8..], &0x0102_0304_0506_0708_i64.to_le_bytes());
    }
}
