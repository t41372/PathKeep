//! Responsibilities: persist vector planes and score every stored vector for search.
//! Not responsible for: canonical eligibility, page fusion, query transport or vector generation.
//! Failure modes: finite binary recall hides the true winner; facets hide eligible old keys;
//! vector dimensions drift; disk/resident planes disagree; ties vary between queries.
//! Tests: vector_index contracts, exact winner beyond 2,000, AI complete semantic pool tests.

use crate::ai::traits::VectorIndex;
use crate::ai::vector_planes::{
    BinaryPlane, Int8Plane, Int8Vector, binarize, dequantize_int8, dot_product, quantize_int8,
};
use crate::config::ProjectPaths;
use anyhow::Result;
use std::collections::HashSet;

/// A flat vector store with complete scoring over its int8 plane.
///
/// Holds the binary plane for key/position alignment; the int8 score plane is
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

    /// Scores every stored vector with bounded buffers. Search callers apply canonical
    /// filters before selecting a page, so a distant candidate can never be hidden by recall_k.
    pub(super) fn visit_scores(
        &self,
        query: &[f32],
        cancel: &vault_store::CancelToken,
        mut visit: impl FnMut(u64, f32) -> Result<()>,
    ) -> Result<()> {
        if self.binary.is_empty() {
            return Ok(());
        }
        if self.int8.is_empty() {
            let Some(planes) = &self.planes else { return Ok(()) };
            let mut reader = planes.int8.reader()?;
            for position in 0..self.binary.len() {
                if position % 256 == 0 {
                    cancel.check()?;
                }
                let (key, vector) = reader.record_at(position)?;
                visit(key, dot_product(query, &dequantize_int8(&vector)))?;
            }
        } else {
            for (position, (key, vector)) in self.int8.iter().enumerate() {
                if position % 256 == 0 {
                    cancel.check()?;
                }
                visit(*key, dot_product(query, &dequantize_int8(vector)))?;
            }
        }
        Ok(())
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

    /// Ranks the complete eligible vector set; k limits output, never candidates.
    fn search(
        &self,
        query: &[f32],
        k: usize,
        allowlist: Option<&[u64]>,
    ) -> Result<Vec<(u64, f32)>> {
        let allow: Option<HashSet<u64>> = allowlist.map(|ids| ids.iter().copied().collect());
        let mut scores = Vec::new();
        self.visit_scores(query, &vault_store::CancelToken::new(), |key, score| {
            if allow.as_ref().is_none_or(|set| set.contains(&key)) {
                scores.push((key, score));
            }
            Ok(())
        })?;
        scores
            .sort_unstable_by(|left, right| right.1.total_cmp(&left.1).then(left.0.cmp(&right.0)));
        scores.truncate(k);
        Ok(scores)
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
    const FORMER_RECALL_FLOOR: usize = 2000;
    const FORMER_RECALL_EXPANSION: usize = 8;
    const FORMER_ALLOWLIST_EXPANSION: usize = 64;
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
    fn exact_vector_search_reaches_a_winner_beyond_every_former_recall_pool() {
        let (corpus, query, winner) = winner_beyond_shallow_pool(16, 2207);
        let mut index = FlatVectorIndex::empty();
        index.build(&corpus).unwrap();
        assert_eq!(index.search(&query, 1, None).unwrap()[0].0, winner);
    }

    #[test]
    fn exact_scoring_recovers_a_winner_beyond_the_former_expansion() {
        const DECOYS: usize = 12;
        const _: () = assert!(DECOYS + 1 > FORMER_RECALL_EXPANSION);
        const _: () = assert!(DECOYS < FORMER_RECALL_FLOOR);
        let (corpus, query, winner_key) = winner_beyond_shallow_pool(16, DECOYS);
        let mut index = FlatVectorIndex::empty();
        index.build(&corpus).expect("build");
        let top = index.search(&query, 1, None).expect("search")[0].0;
        assert_eq!(top, winner_key, "complete scoring must reach the true winner");
    }

    #[test]
    fn exact_scoring_respects_the_allowlist_before_selection() {
        const DECOYS: usize = 70;
        const _: () = assert!(DECOYS + 1 > FORMER_ALLOWLIST_EXPANSION);
        const _: () = assert!(DECOYS < FORMER_RECALL_FLOOR);
        let (corpus, query, winner_key) = winner_beyond_shallow_pool(16, DECOYS);
        let allow: Vec<u64> = corpus.iter().map(|(key, _)| *key).collect();
        let mut index = FlatVectorIndex::empty();
        index.build(&corpus).expect("build");
        let top = index.search(&query, 1, Some(&allow)).expect("search")[0].0;
        assert_eq!(top, winner_key, "the allowlist must preserve the true winner");
    }

    #[test]
    fn exact_topk_is_stable_above_the_former_pool_size() {
        let dim = 16usize;
        let n = FORMER_RECALL_FLOOR + 200;
        let query = unit(vec![1.0; dim]);
        let winner_key = 1u64;
        let mut corpus: Vec<(u64, Vec<f32>)> = vec![(winner_key, query.clone())];
        for key in 2..=n as u64 {
            let mut vector = vec![0.05f32; dim];
            vector[(key as usize) % dim] += 1.0;
            corpus.push((key, unit(vector)));
        }

        let mut index = FlatVectorIndex::empty();
        index.build(&corpus).expect("build");
        assert!(index.len() > FORMER_RECALL_FLOOR, "the corpus exceeds the former recall pool");
        let first = index.search(&query, 1, None).expect("search");
        assert_eq!(first[0].0, winner_key, "the exact-query vector ranks first");
        for _ in 0..5 {
            assert_eq!(
                index.search(&query, 1, None).expect("repeat"),
                first,
                "stable complete ranking"
            );
        }
    }

    #[test]
    fn exact_topk_breaks_ties_deterministically() {
        let mut corpus: Vec<(u64, Vec<f32>)> = Vec::new();
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

#[cfg(test)]
mod complete_search_tests {
    use super::*;
    #[test]
    fn the_complete_score_stream_has_no_recall_pool_and_observes_cancellation() {
        let mut index = FlatVectorIndex::empty();
        index
            .build(
                &(1..=1207).map(|id| (id, vec![1., id as f32 / 1207., 0., 0.])).collect::<Vec<_>>(),
            )
            .unwrap();
        let cancel = vault_store::CancelToken::new();
        let mut keys = Vec::new();
        index
            .visit_scores(&[1., 0., 0., 0.], &cancel, |key, score| {
                assert!(score.is_finite());
                keys.push(key);
                Ok(())
            })
            .unwrap();
        assert_eq!(keys, (1..=1207).collect::<Vec<_>>());
        cancel.cancel();
        assert!(index.visit_scores(&[1., 0., 0., 0.], &cancel, |_, _| Ok(())).is_err());
    }
}
