//! Flat two-stage vector index: binary Hamming recall → int8 rescore (W-AI-5, 05 §3/§6).
//!
//! ## Responsibilities
//! - implement the [`VectorIndex`] boundary trait with a HAND-ROLLED flat engine (no vector-index
//!   crate — 05 §6 / 04 §6 start flat-binary-in-RAM; an indexed backend can drop in behind this same
//!   trait LATER if the S2 benchmark proves flat insufficient at 14.4M).
//! - run the two-stage search: stage 1 is a SIMD-friendly popcount Hamming sweep over the
//!   RAM-resident binary plane to recall the top `k'` candidates; stage 2 dequantizes only those
//!   candidates from the int8 plane and re-ranks by dot product (= cosine, vectors are L2-normalized),
//!   returning the top `k`.
//! - apply an allowlist as a POST-filter (04 §6 / 02 §D.1): recall an EXPANDED `k'`, drop ids not in
//!   the allowlist, then take `k`, so a tight facet filter still surfaces enough true matches.
//! - keep tie-breaking deterministic (score desc, then content_key asc) so identical-score results
//!   rank stably across runs and platforms.
//!
//! ## Not responsible for
//! - producing/persisting the planes (the [`super::vector_planes`] projection owns that; this index
//!   only READS them) or the f32 `.pkvec` source (the [`super::vector_store`] owns it);
//! - embedding the query (the caller embeds + binarizes before calling [`VectorIndex::search`]);
//! - hydrating content_key → visit (the search layer joins through the `.pkmap`).
//!
//! ## Why flat (no HNSW/IVF yet)
//! Binary Hamming over 14.4M × 32 B packed (~0.9 GB resident incl. Rust overhead, hardware popcount) is
//! fast enough flat at this scale (S2: p50 ~105ms @14.4M, 05 §10); HNSW/IVF add a graph/centroid
//! structure + (for IVF) periodic retraining that only
//! pay off past ~1–2M when a flat sweep stops being interactive (05 §6). Hiding the engine behind
//! [`VectorIndex`] means that upgrade is a swap, not a rewrite. The S2 benchmark is the data that
//! decides whether/when the swap is warranted.
//!
//! ## Performance notes
//! - ONLY the binary plane is held resident (`Vec<(u64, Vec<u8>)>`); the recall sweep is O(n) popcount,
//!   the compiler vectorizes [`super::vector_planes::hamming_distance`]'s `u64::count_ones` chunking.
//!   At 14.4M/256-dim that is ~0.9 GB incl. Rust `(u64, Vec<u8>)` overhead — within the 8 GB envelope.
//! - the int8 rescore plane is NOT resident (C1/C2, 05 §10): a plane-backed index seeks each recalled
//!   candidate's int8 record off disk by its POSITION (the binary-recall position, the two planes are
//!   positionally aligned by the lockstep projection), so resident RAM = binary only. The ~3.7 GB int8
//!   plane stays on disk; a few thousand random reads/query is cheap vs the O(n) binary sweep and warms
//!   the page cache. An in-memory index (no on-disk planes, e.g. tests/bench) keeps int8 resident.
//! - rescore touches only `k'` int8 vectors, dequantizing each once — O(k'·dim), independent of n.
//! - `append`/`remove` mutate the resident binary (and, for an in-memory index, int8) plane in place;
//!   `save`/`load` are deferred to the projection step (the planes ARE the persistence), so this
//!   engine's `save` is a no-op and `load` re-reads the on-disk binary plane (int8 stays on disk).

use crate::ai::traits::VectorIndex;
use crate::ai::vector_planes::{
    BinaryPlane, Int8Plane, Int8Vector, binarize, dequantize_int8, dot_product, hamming_distance,
    quantize_int8,
};
use crate::config::ProjectPaths;
use anyhow::Result;
use std::collections::HashSet;

/// Floor on the binary-recall candidate pool `k'` regardless of `k` (X-1, 05 §10).
///
/// The stage-1 Hamming sweep is O(n) whether `k'` is 64 or 2000, and the stage-2 int8 rescore of a
/// few thousand candidates is sub-millisecond, so a DEEPER recall floor is ~free latency-wise and a
/// real robustness win: a shallow pool (`k=8 → k'=64`) leaves stage 2 with too few candidates to
/// recover binary-recall's mis-orderings on dense near-duplicate neighbourhoods, and made the no-facet
/// pool SHALLOWER than the faceted one (the inversion this floor removes). It is NOT a "3× quality"
/// claim — the S2 recall@10-vs-exact is partly a synthetic near-duplicate artifact — but a tuning win
/// at ~zero cost. Capped at the index size, so a small index simply rescores everything it has.
pub const RECALL_FLOOR: usize = 2000;

/// Multiplier applied to `k` to size the binary-recall candidate pool `k'` (05 §3, 02 §D.1).
///
/// Stage 1 (Hamming) is a coarse filter: binary distance correlates with but does not equal cosine,
/// so recalling more candidates than the final `k` gives stage 2 (int8 rescore) room to recover the
/// true top-`k` that binary alone would have mis-ordered. The pool is `max(k * RECALL_EXPANSION,
/// RECALL_FLOOR)` capped at the index size (X-1): this expansion scales the pool with large `k`, while
/// [`RECALL_FLOOR`] guarantees a deep-enough floor for small `k` (the interactive default `k=8` would
/// otherwise recall only 64). Both are nearly free because stage 1 is O(n) regardless of `k'` and the
/// int8 rescore of a few thousand candidates is sub-millisecond.
pub const RECALL_EXPANSION: usize = 8;

/// Extra expansion applied to `k'` when an allowlist post-filter is in effect (04 §6).
///
/// A post-filter drops recalled ids that are not permitted, so a tight facet (e.g. one starred
/// domain) could leave fewer than `k` survivors from the base pool. This larger factor recalls deeper
/// so the filter still yields a full `k`. It is a CEILING on work, not a guarantee: an allowlist with
/// fewer than `k` total members simply returns everything it has.
pub const ALLOWLIST_EXPANSION: usize = 64;

/// A hand-rolled flat two-stage vector index over the derived binary + int8 planes.
///
/// Holds the BINARY plane resident (it drives the Hamming recall sweep); the int8 rescore plane is
/// seeked off disk by position for a plane-backed index (C1/C2 — resident = binary only), or held
/// resident for an in-memory index (tests/bench, no on-disk plane). Construct via
/// [`FlatVectorIndex::open`] (reads the on-disk binary plane, seeks int8) or [`FlatVectorIndex::empty`]
/// (an in-memory index a caller `build`s); either way it implements the [`VectorIndex`] trait so a
/// future indexed engine is a drop-in replacement.
pub struct FlatVectorIndex {
    /// `(content_key, packed_sign_bits)` for the Hamming recall sweep, in projection order (the same
    /// order the int8 plane was written, so a binary position indexes the aligned int8 record).
    binary: Vec<(u64, Vec<u8>)>,
    /// `(content_key, int8_vector)` for the rescore stage — populated ONLY for an in-memory index;
    /// EMPTY for a plane-backed index (which seeks int8 records off disk by position instead, C1).
    int8: Vec<(u64, Int8Vector)>,
    /// Effective dim of the held vectors; `0` until the first vector is ingested (D4: never assumed).
    dim: usize,
    /// On-disk plane handles for `save`/`load`/`clear` + int8 seek-by-position; `None` in-memory.
    planes: Option<PlaneHandles>,
}

/// On-disk plane handles backing an index built from a provider/model's projected planes.
struct PlaneHandles {
    binary: BinaryPlane,
    int8: Int8Plane,
}

impl FlatVectorIndex {
    /// Builds an empty, purely in-memory index (no on-disk planes; `save`/`load` are no-ops).
    ///
    /// Used by tests and by the projection-free `build`-from-vectors path. The `dim` is `0` until the
    /// first vector arrives via [`VectorIndex::build`]/[`VectorIndex::append`].
    pub fn empty() -> Self {
        Self { binary: Vec::new(), int8: Vec::new(), dim: 0, planes: None }
    }

    /// Opens the index over one provider/model's derived planes, loading them into RAM.
    ///
    /// Reads the binary + int8 planes (projected from `.pkvec` by [`super::vector_planes`]). Missing
    /// planes load as EMPTY (never an error): a never-built index yields zero results with an honest
    /// note from the search layer, never a panic. The dim is taken from the binary plane header so the
    /// recall stride is the truth, not an assumption.
    pub fn open(paths: &ProjectPaths, provider_id: &str, model: &str) -> Result<Self> {
        let binary = BinaryPlane::for_provider(paths, provider_id, model);
        let int8 = Int8Plane::for_provider(paths, provider_id, model);
        let mut index = Self {
            binary: Vec::new(),
            int8: Vec::new(),
            dim: 0,
            planes: Some(PlaneHandles { binary, int8 }),
        };
        index.load()?;
        Ok(index)
    }

    /// Returns the number of vectors held by the index.
    pub fn len(&self) -> usize {
        self.binary.len()
    }

    /// Returns whether the index holds no vectors.
    pub fn is_empty(&self) -> bool {
        self.binary.is_empty()
    }

    /// Returns the effective dim of the held vectors (`0` when empty).
    pub fn dim(&self) -> usize {
        self.dim
    }

    /// Ingests one f32 vector into both resident planes, binarizing + int8-quantizing it.
    ///
    /// The first ingested vector fixes the dim (D4); a later vector of a different length errors
    /// rather than corrupting the fixed-stride planes. A repeated key is NOT deduped here — callers
    /// feed the deduped `.pkvec` set (its `read_all` is already a last-writer-wins SET), so this stays
    /// a thin ingest. Returns the projected `(binary, int8)` rows so a caller can persist them too.
    fn ingest(&mut self, key: u64, vector: &[f32]) -> Result<()> {
        if self.dim == 0 {
            self.dim = vector.len();
        } else if vector.len() != self.dim {
            anyhow::bail!(
                "vector for key {key} has length {} but index dim is {}",
                vector.len(),
                self.dim
            );
        }
        self.binary.push((key, binarize(vector)));
        self.int8.push((key, quantize_int8(vector)));
        Ok(())
    }

    /// Runs the two-stage flat search for a BINARIZED+int8 query, returning top-`k` `(key, score)`.
    ///
    /// `query_bits` is the sign-bit binarization of the query (stage 1 input); `query_int8` is its
    /// dequantized-f32 form used as the rescore reference (stage 2). The caller binarizes/quantizes
    /// the embedded query once via [`prepare_query`]. Splitting the prepared inputs from
    /// [`VectorIndex::search`] keeps the hot path allocation-free per call and unit-testable directly.
    fn search_prepared(
        &self,
        query_bits: &[u8],
        query_ref: &[f32],
        k: usize,
        allowlist: Option<&HashSet<u64>>,
    ) -> Result<Vec<(u64, f32)>> {
        if k == 0 || self.binary.is_empty() {
            return Ok(Vec::new());
        }
        // Stage 1 — binary Hamming recall. Size k' as `max(k * expansion, RECALL_FLOOR)` (X-1): the
        // expansion scales the pool with large `k`, the floor guarantees a deep-enough pool for the
        // interactive default `k=8`. Expand FURTHER when an allowlist post-filter will drop recalled
        // ids (04 §6). Both branches apply the floor so the no-facet pool is never shallower than the
        // faceted one (removing the prior inversion). Capped at the index size.
        let expansion = if allowlist.is_some() { ALLOWLIST_EXPANSION } else { RECALL_EXPANSION };
        let recall_k = k.saturating_mul(expansion).max(RECALL_FLOOR).min(self.binary.len());
        let candidates = self.recall(query_bits, recall_k, allowlist);

        // Stage 2 — int8 rescore over only the recalled candidates. When the int8 plane is NOT resident
        // (a plane-backed index loaded via `open`/`load`, the production query path), seek each
        // candidate's int8 record off disk by its POSITION (resident = binary only, C1). When int8 IS
        // resident (an in-memory `empty`+`build`/`append` index, tests/bench), read it directly. Either
        // way `position` is the binary-recall position and the planes are positionally aligned, so the
        // record fetched is exactly that candidate's. (`self.int8` is empty for a plane-backed index
        // even when the binary plane has rows, so the seek branch owns that case.)
        let mut scored: Vec<(u64, f32)> = if self.int8.is_empty() {
            let handles = self
                .planes
                .as_ref()
                .expect("a non-empty binary plane with no resident int8 is always plane-backed");
            let mut reader = handles.int8.reader()?;
            let mut out = Vec::with_capacity(candidates.len());
            for position in candidates {
                let (key, int8) = reader.record_at(position)?;
                out.push((key, dot_product(query_ref, &dequantize_int8(&int8))));
            }
            out
        } else {
            candidates
                .into_iter()
                .map(|position| {
                    let (key, int8) = &self.int8[position];
                    (*key, dot_product(query_ref, &dequantize_int8(int8)))
                })
                .collect()
        };
        // Deterministic tie-break: score desc, then content_key asc.
        scored.sort_by(|left, right| {
            right
                .1
                .partial_cmp(&left.1)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(left.0.cmp(&right.0))
        });
        scored.truncate(k);
        Ok(scored)
    }

    /// Stage 1: returns the binary-plane POSITIONS of the `recall_k` smallest-Hamming candidates.
    ///
    /// Applies the allowlist as a post-filter (skips ids not permitted) during the sweep so the pool is
    /// already filtered before truncation. Uses a BOUNDED top-k selection (`select_nth_unstable_by` then
    /// truncate, C3) instead of a full sort of all n: it partitions the n candidates around the k'-th in
    /// O(n) average rather than O(n log n), then sorts only the kept k' for deterministic stage-2 order.
    /// The comparator (Hamming asc, then content_key asc) is a STRICT TOTAL ORDER because content_key is
    /// unique within the pool, so the selection is deterministic across runs/platforms. Positions (not
    /// keys) are returned so stage 2 fetches the aligned int8 record by position.
    fn recall(
        &self,
        query_bits: &[u8],
        recall_k: usize,
        allowlist: Option<&HashSet<u64>>,
    ) -> Vec<usize> {
        // (distance, key, position) so the order tie-breaks deterministically on key.
        let mut scored: Vec<(u32, u64, usize)> = self
            .binary
            .iter()
            .enumerate()
            .filter(|(_, (key, _))| allowlist.is_none_or(|allow| allow.contains(key)))
            .map(|(position, (key, bits))| (hamming_distance(query_bits, bits), *key, position))
            .collect();
        // Hamming asc, then content_key asc — a strict total order (unique key) so selection + the
        // tail sort are both deterministic.
        let cmp = |left: &(u32, u64, usize), right: &(u32, u64, usize)| {
            left.0.cmp(&right.0).then(left.1.cmp(&right.1))
        };
        if recall_k < scored.len() {
            // Partition so the smallest `recall_k` by `cmp` occupy `[0, recall_k)`, then drop the rest.
            scored.select_nth_unstable_by(recall_k - 1, cmp);
            scored.truncate(recall_k);
        }
        // Sort the kept pool so stage 2 sees candidates in deterministic best-first order.
        scored.sort_unstable_by(cmp);
        scored.into_iter().map(|(_, _, position)| position).collect()
    }
}

/// Binarizes + keeps an f32 reference copy of an embedded query for [`FlatVectorIndex`] search.
///
/// The binary form drives stage-1 Hamming; the f32 reference is the stage-2 rescore reference. We
/// rescore against the QUERY's f32 (not its int8) so query quantization error never compounds with
/// the stored vectors' int8 error — only the indexed side is lossy, matching how the recall@10
/// benchmark is framed (binary-recall + int8-rescore vs exact f32). Returned as an owned pair the
/// caller passes to [`VectorIndex::search`] via the trait's f32 `query` slice.
pub fn prepare_query(query: &[f32]) -> (Vec<u8>, Vec<f32>) {
    (binarize(query), query.to_vec())
}

impl VectorIndex for FlatVectorIndex {
    /// Builds a fresh index from `(content_key, f32)` pairs, replacing any held vectors.
    fn build(&mut self, items: &[(u64, Vec<f32>)]) -> Result<()> {
        self.binary.clear();
        self.int8.clear();
        self.dim = 0;
        for (key, vector) in items {
            self.ingest(*key, vector)?;
        }
        Ok(())
    }

    /// Appends `(content_key, f32)` pairs to the held vectors (dim must match once fixed).
    fn append(&mut self, items: &[(u64, Vec<f32>)]) -> Result<()> {
        for (key, vector) in items {
            self.ingest(*key, vector)?;
        }
        Ok(())
    }

    /// Removes every resident vector with the given content_key (a key may repeat at the storage
    /// boundary on a rare u64 collision; all copies are dropped together).
    fn remove(&mut self, external_id: u64) -> Result<()> {
        self.binary.retain(|(key, _)| *key != external_id);
        self.int8.retain(|(key, _)| *key != external_id);
        Ok(())
    }

    /// Returns the top-`k` `(content_key, score)` matches for the embedded f32 `query`.
    ///
    /// `query` is the f32 query embedding; this binarizes it for stage-1 recall and rescores against
    /// its f32 in stage 2. `allowlist`, when `Some`, restricts results to those content keys via a
    /// post-filter over an EXPANDED recall pool (04 §6). An empty index returns no results (never an
    /// error) so a never-built index degrades gracefully.
    fn search(
        &self,
        query: &[f32],
        k: usize,
        allowlist: Option<&[u64]>,
    ) -> Result<Vec<(u64, f32)>> {
        let (query_bits, query_ref) = prepare_query(query);
        let allow_set: Option<HashSet<u64>> = allowlist.map(|ids| ids.iter().copied().collect());
        self.search_prepared(&query_bits, &query_ref, k, allow_set.as_ref())
    }

    /// No-op: the projected planes ARE the persistence (written by [`super::vector_planes`]).
    ///
    /// The index reads planes; it never owns their on-disk lifecycle. Mutations made via
    /// `append`/`remove` are in-RAM only and are reconciled by re-projecting the planes from the
    /// `.pkvec` source, not by a write-back here. Kept as a satisfied trait method so the boundary
    /// stays uniform.
    fn save(&self) -> Result<()> {
        Ok(())
    }

    /// Reloads the RESIDENT BINARY plane from disk (the int8 plane stays on disk, seeked per query).
    ///
    /// For an in-memory index (no plane handles) this is a no-op that leaves the held vectors intact.
    /// For a plane-backed index it replaces the resident binary copy with the on-disk projection,
    /// picking up a fresh build; the int8 plane is NOT loaded resident (C1 — resident = binary only),
    /// it is read by seek-by-position during rescore. The resident int8 Vec stays empty for a
    /// plane-backed index so a stale copy can never be read instead of the on-disk plane.
    fn load(&mut self) -> Result<()> {
        let Some(handles) = self.planes.as_ref() else {
            return Ok(());
        };
        // Missing planes load as empty rather than erroring (never-built index path).
        self.binary = if handles.binary.exists() { handles.binary.read_all()? } else { Vec::new() };
        self.int8 = Vec::new();
        self.dim = handles.binary.read_header()?.map(|header| header.dim).unwrap_or(0);
        Ok(())
    }

    /// Drops all resident vectors (the on-disk planes are cleared by the projection's delete path).
    fn clear(&mut self) -> Result<()> {
        self.binary.clear();
        self.int8.clear();
        self.dim = 0;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::vector_planes::build_planes_from_store;
    use crate::ai::vector_store::VectorStore;
    use crate::ai::{EmbeddingDtype, EmbeddingFingerprint, EmbeddingPooling};
    use crate::config::project_paths_with_root;
    use tempfile::tempdir;

    /// L2-normalizes a vector so dot product equals cosine (matches the embed path's invariant).
    fn unit(mut vector: Vec<f32>) -> Vec<f32> {
        let norm = vector.iter().map(|value| value * value).sum::<f32>().sqrt();
        if norm > 0.0 {
            for value in &mut vector {
                *value /= norm;
            }
        }
        vector
    }

    #[test]
    fn build_search_returns_nearest_by_cosine() {
        let mut index = FlatVectorIndex::empty();
        index
            .build(&[
                (1, unit(vec![1.0, 0.0, 0.0])),
                (2, unit(vec![0.0, 1.0, 0.0])),
                (3, unit(vec![0.9, 0.1, 0.0])),
            ])
            .expect("build");
        assert_eq!(index.len(), 3);
        assert!(!index.is_empty());
        assert_eq!(index.dim(), 3);

        let results = index.search(&unit(vec![1.0, 0.05, 0.0]), 2, None).expect("search");
        assert_eq!(results.len(), 2);
        // key 1 and key 3 are the two closest to an x-axis-ish query.
        let keys: Vec<u64> = results.iter().map(|(key, _)| *key).collect();
        assert!(keys.contains(&1));
        assert!(keys.contains(&3));
        assert!(!keys.contains(&2));
        // Scores are descending.
        assert!(results[0].1 >= results[1].1);
    }

    #[test]
    fn empty_index_and_zero_k_return_no_results() {
        let index = FlatVectorIndex::empty();
        assert!(index.is_empty());
        assert!(index.search(&[1.0, 0.0], 5, None).expect("empty search").is_empty());

        let mut populated = FlatVectorIndex::empty();
        populated.build(&[(1, unit(vec![1.0, 0.0]))]).expect("build");
        assert!(populated.search(&[1.0, 0.0], 0, None).expect("zero k").is_empty());
    }

    #[test]
    fn allowlist_post_filter_restricts_results() {
        let mut index = FlatVectorIndex::empty();
        index
            .build(&[
                (1, unit(vec![1.0, 0.0, 0.0])),
                (2, unit(vec![0.95, 0.05, 0.0])),
                (3, unit(vec![0.9, 0.1, 0.0])),
            ])
            .expect("build");
        // Without the allowlist key 1 would top; restrict to {2,3} and only those survive.
        let results = index.search(&unit(vec![1.0, 0.0, 0.0]), 5, Some(&[2, 3])).expect("filtered");
        let keys: Vec<u64> = results.iter().map(|(key, _)| *key).collect();
        assert_eq!(keys.len(), 2);
        assert!(keys.contains(&2));
        assert!(keys.contains(&3));
        assert!(!keys.contains(&1));
    }

    #[test]
    fn allowlist_with_fewer_members_than_k_returns_all_members() {
        let mut index = FlatVectorIndex::empty();
        index
            .build(&[
                (1, unit(vec![1.0, 0.0])),
                (2, unit(vec![0.0, 1.0])),
                (3, unit(vec![0.5, 0.5])),
            ])
            .expect("build");
        let results = index.search(&unit(vec![1.0, 0.0]), 10, Some(&[2])).expect("single allow");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0, 2);
    }

    #[test]
    fn empty_allowlist_returns_no_results() {
        let mut index = FlatVectorIndex::empty();
        index.build(&[(1, unit(vec![1.0, 0.0]))]).expect("build");
        let results = index.search(&unit(vec![1.0, 0.0]), 5, Some(&[])).expect("empty allow");
        assert!(results.is_empty());
    }

    #[test]
    fn tie_break_is_deterministic_on_content_key() {
        // Two vectors identical to the query produce identical scores; the lower key must rank first.
        let mut index = FlatVectorIndex::empty();
        index
            .build(&[
                (7, unit(vec![1.0, 0.0])),
                (3, unit(vec![1.0, 0.0])),
                (9, unit(vec![0.0, 1.0])),
            ])
            .expect("build");
        let results = index.search(&unit(vec![1.0, 0.0]), 2, None).expect("search");
        assert_eq!(results[0].0, 3, "tie resolves to the lower content_key first");
        assert_eq!(results[1].0, 7);
    }

    #[test]
    fn append_and_remove_mutate_in_place() {
        let mut index = FlatVectorIndex::empty();
        index.build(&[(1, unit(vec![1.0, 0.0]))]).expect("build");
        index.append(&[(2, unit(vec![0.0, 1.0]))]).expect("append");
        assert_eq!(index.len(), 2);
        index.remove(1).expect("remove");
        assert_eq!(index.len(), 1);
        let results = index.search(&unit(vec![0.0, 1.0]), 5, None).expect("search");
        assert_eq!(results[0].0, 2);
        index.clear().expect("clear");
        assert!(index.is_empty());
        assert_eq!(index.dim(), 0);
    }

    #[test]
    fn append_rejects_dim_mismatch() {
        let mut index = FlatVectorIndex::empty();
        index.build(&[(1, unit(vec![1.0, 0.0]))]).expect("build");
        let error = index.append(&[(2, vec![1.0, 0.0, 0.0])]).expect_err("dim mismatch");
        assert!(error.to_string().contains("length 3 but index dim is 2"));
    }

    #[test]
    fn save_is_noop_and_load_reads_planes() {
        let dir = tempdir().expect("tempdir");
        let paths = project_paths_with_root(dir.path());
        let fingerprint = EmbeddingFingerprint::new(
            "static-embed",
            "model-a",
            3,
            EmbeddingDtype::Float32,
            true,
            EmbeddingPooling::Mean,
            None,
        );
        let store = VectorStore::create_stamped(&paths, &fingerprint).expect("create");
        store
            .append_vectors(&[(10, unit(vec![1.0, 0.0, 0.0])), (20, unit(vec![0.0, 1.0, 0.0]))])
            .expect("append store");
        build_planes_from_store(&paths, "static-embed", "model-a").expect("build planes");

        let mut index = FlatVectorIndex::open(&paths, "static-embed", "model-a").expect("open");
        assert_eq!(index.len(), 2);
        assert_eq!(index.dim(), 3);
        index.save().expect("save noop");
        index.load().expect("reload");
        assert_eq!(index.len(), 2);

        let results = index.search(&unit(vec![1.0, 0.0, 0.0]), 1, None).expect("search");
        assert_eq!(results[0].0, 10);
    }

    #[test]
    fn open_missing_planes_loads_empty() {
        let dir = tempdir().expect("tempdir");
        let paths = project_paths_with_root(dir.path());
        let index = FlatVectorIndex::open(&paths, "static-embed", "never-built").expect("open");
        assert!(index.is_empty());
        assert!(index.search(&[1.0, 0.0, 0.0], 5, None).expect("search").is_empty());
    }

    #[test]
    fn in_memory_load_is_noop_and_keeps_vectors() {
        let mut index = FlatVectorIndex::empty();
        index.build(&[(1, unit(vec![1.0, 0.0]))]).expect("build");
        index.load().expect("in-memory load noop");
        assert_eq!(index.len(), 1);
    }

    /// Builds a corpus where the true int8-cosine winner sits at binary-Hamming RANK > `shallow`.
    ///
    /// The query is all-positive (binarizes to all-ones). `decoys` vectors are all-positive but point
    /// in a near-orthogonal direction (Hamming distance 0 to the query, yet LOW cosine). One winner
    /// vector flips a single sign bit (Hamming distance 1, so it ranks AFTER every Hamming-0 decoy) but
    /// is otherwise aligned with the query (HIGH cosine). A binary-recall pool shallower than
    /// `decoys + 1` never reaches the winner; only a pool deep enough to include rank `decoys` does.
    fn winner_beyond_shallow_pool(
        dim: usize,
        decoys: usize,
    ) -> (Vec<(u64, Vec<f32>)>, Vec<f32>, u64) {
        assert!(dim >= 2);
        let query = unit(vec![1.0; dim]);
        let mut corpus: Vec<(u64, Vec<f32>)> = Vec::with_capacity(decoys + 1);
        // Hamming-0 decoys: all components positive (so every sign bit matches the query) but a spiky
        // direction (one big component, the rest tiny) so cosine to the uniform query is low. Distinct
        // spike positions keep them distinct vectors.
        for index in 0..decoys {
            let mut vector = vec![0.01f32; dim];
            vector[index % dim] = 1.0;
            corpus.push((index as u64 + 1, unit(vector)));
        }
        // Winner: aligned with the query (all large positive) EXCEPT one tiny negative component, so it
        // is Hamming-1 (ranked after all Hamming-0 decoys) yet has the highest cosine to the query.
        let winner_key = decoys as u64 + 1;
        let mut winner = vec![1.0f32; dim];
        winner[dim - 1] = -0.001;
        corpus.push((winner_key, unit(winner)));
        (corpus, query, winner_key)
    }

    #[test]
    fn recall_floor_lifts_the_no_facet_pool_past_k_times_expansion() {
        // X-1: with k=1 the no-facet pool would be k*RECALL_EXPANSION = 8 without the floor. Place the
        // true winner at Hamming rank 12 (12 Hamming-0 decoys ahead of it): a pool of 8 misses it, but
        // RECALL_FLOOR (capped at the 13-vector index) recalls all 13 so the int8 winner is found.
        const DECOYS: usize = 12;
        // Compile-time invariants the scenario depends on: the winner sits BEYOND the un-floored pool
        // (k=1 → RECALL_EXPANSION) yet WITHIN the floor (capped at len=DECOYS+1), so only the floor
        // recalls it.
        const _: () = assert!(DECOYS + 1 > RECALL_EXPANSION);
        const _: () = assert!(DECOYS < RECALL_FLOOR);
        let (corpus, query, winner_key) = winner_beyond_shallow_pool(16, DECOYS);
        let mut index = FlatVectorIndex::empty();
        index.build(&corpus).expect("build");
        let top = index.search(&query, 1, None).expect("search")[0].0;
        assert_eq!(top, winner_key, "the floor must recall deep enough to rescore the true winner");
    }

    #[test]
    fn recall_floor_lifts_the_faceted_pool_past_k_times_allowlist_expansion() {
        // X-1 (facet branch): with k=1 the faceted pool would be k*ALLOWLIST_EXPANSION = 64 without the
        // floor. Place the winner at Hamming rank 70 (70 Hamming-0 decoys ahead): a pool of 64 misses
        // it, RECALL_FLOOR (capped at the 71-vector index) recalls all so the int8 winner is found. The
        // allowlist admits every key so the post-filter itself never drops the winner — only pool depth.
        const DECOYS: usize = 70;
        // Compile-time invariants: the winner sits BEYOND the un-floored faceted pool (k=1 →
        // ALLOWLIST_EXPANSION) yet WITHIN the floor (capped at len=DECOYS+1), so only the floor reaches.
        const _: () = assert!(DECOYS + 1 > ALLOWLIST_EXPANSION);
        const _: () = assert!(DECOYS < RECALL_FLOOR);
        let (corpus, query, winner_key) = winner_beyond_shallow_pool(16, DECOYS);
        let allow: Vec<u64> = corpus.iter().map(|(key, _)| *key).collect();
        let mut index = FlatVectorIndex::empty();
        index.build(&corpus).expect("build");
        let top = index.search(&query, 1, Some(&allow)).expect("search")[0].0;
        assert_eq!(top, winner_key, "the floor must lift the faceted pool to the true winner too");
    }

    #[test]
    fn bounded_selection_partitions_when_index_exceeds_the_recall_floor() {
        // C3: when the index has MORE than RECALL_FLOOR vectors and k=1, recall_k = RECALL_FLOOR < n, so
        // the `recall_k < scored.len()` branch fires and `select_nth_unstable_by` + truncate actually
        // partition the pool below n (the small-index tests recall the whole index, never reaching this
        // branch). The result must still be a correct, deterministic top-1 across repeated runs.
        let dim = 16usize;
        let n = RECALL_FLOOR + 200;
        let query = unit(vec![1.0; dim]);
        // The winner is the exact query at the LOWEST content_key, so even though every vector here is
        // Hamming-0 to the all-positive query (the tie-break that orders the partition is content_key
        // asc), the winner is kept in the floored pool AND its exact-cosine rescore tops the result.
        let winner_key = 1u64;
        let mut corpus: Vec<(u64, Vec<f32>)> = vec![(winner_key, query.clone())];
        // Distinct positive decoys (Hamming-0, spiky direction → lower cosine) at higher keys.
        for key in 2..=n as u64 {
            let mut vector = vec![0.05f32; dim];
            vector[(key as usize) % dim] += 1.0;
            corpus.push((key, unit(vector)));
        }

        let mut index = FlatVectorIndex::empty();
        index.build(&corpus).expect("build");
        assert!(
            index.len() > RECALL_FLOOR,
            "index must exceed the floor to exercise the partition"
        );
        let first = index.search(&query, 1, None).expect("search");
        assert_eq!(
            first[0].0, winner_key,
            "the exact-query vector tops even through the partition"
        );
        for _ in 0..5 {
            assert_eq!(
                index.search(&query, 1, None).expect("repeat"),
                first,
                "stable under partition"
            );
        }
    }

    #[test]
    fn bounded_selection_is_deterministic_across_runs() {
        // C3: `recall` now uses `select_nth_unstable_by` (a partial, unstable partition) instead of a
        // full sort, then sorts the kept pool. The comparator is a strict total order (Hamming asc,
        // then UNIQUE content_key asc), so even with equal Hamming distances the selected pool — and
        // thus the final ranking — is identical on every run. Many tied-Hamming vectors + a deep enough
        // pool exercise the partition path; the result must be byte-identical run to run.
        let mut corpus: Vec<(u64, Vec<f32>)> = Vec::new();
        // 40 vectors all at the SAME Hamming distance from the query (all-positive, distinct directions)
        // so the tie-break (content_key) is the only thing ordering them — the determinism stress.
        for key in 0..40u64 {
            let mut vector = vec![0.1f32; 8];
            vector[(key % 8) as usize] += key as f32 * 0.01 + 0.5;
            corpus.push((key + 1, unit(vector)));
        }
        let query = unit(vec![1.0; 8]);
        let mut index = FlatVectorIndex::empty();
        index.build(&corpus).expect("build");
        let first = index.search(&query, 10, None).expect("search 1");
        for _ in 0..8 {
            assert_eq!(
                index.search(&query, 10, None).expect("repeat"),
                first,
                "ranking must be stable"
            );
        }
        // The kept pool is sorted, so scores are non-increasing and the tie-break is content_key asc.
        for window in first.windows(2) {
            assert!(window[0].1 >= window[1].1, "scores descend");
        }
    }

    #[test]
    fn two_stage_recall_matches_exact_f32_on_a_small_set() {
        // Compare flat (binary recall + int8 rescore) ranking to exact f32 cosine on a tiny set with
        // well-SEPARATED clusters (so int8 rounding can't flip near-ties): the top result must agree
        // (the property the S2 benchmark measures at scale). Each vector points strongly along one
        // axis; the query aligns with the first.
        let vectors = vec![
            (1u64, unit(vec![0.95, 0.1, 0.1, 0.1])),
            (2u64, unit(vec![0.1, 0.95, 0.1, 0.1])),
            (3u64, unit(vec![0.1, 0.1, 0.95, 0.1])),
            (4u64, unit(vec![0.1, 0.1, 0.1, 0.95])),
        ];
        let query = unit(vec![0.9, 0.15, 0.12, 0.1]);

        let mut index = FlatVectorIndex::empty();
        index.build(&vectors).expect("build");
        let flat_top = index.search(&query, 1, None).expect("flat search")[0].0;

        // Exact f32 brute force.
        let exact_top = vectors
            .iter()
            .map(|(key, vector)| (*key, dot_product(&query, vector)))
            .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap())
            .expect("exact top")
            .0;
        assert_eq!(flat_top, exact_top);
        assert_eq!(flat_top, 1);
    }
}
