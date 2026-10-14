//! Responsibilities: persist vector planes and score every stored vector for search.
//! Not responsible for: canonical eligibility, page fusion, query transport or vector generation.
//! Failure modes: finite binary recall hides the true winner; facets hide eligible old keys;
//! vector dimensions drift; disk/resident planes disagree; ties vary between queries.
//! Tests: the complete score stream here; semantic pool tests in `ai/tests.rs`.

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
/// seeked off disk by position for a plane-backed index (resident = binary only), or held
/// resident for an in-memory index (tests/bench, no on-disk plane). Construct via
/// [`FlatVectorIndex::open`] (reads the on-disk binary plane, seeks int8) or [`FlatVectorIndex::empty`]
/// (an in-memory index a caller `build`s); either way it implements the [`VectorIndex`] trait so a
/// future indexed engine is a drop-in replacement.
pub struct FlatVectorIndex {
    /// `(content_key, packed_sign_bits)` for the Hamming recall sweep, in projection order (the same
    /// order the int8 plane was written, so a binary position indexes the aligned int8 record).
    binary: Vec<(u64, Vec<u8>)>,
    /// `(content_key, int8_vector)` for the rescore stage — populated ONLY for an in-memory index;
    /// EMPTY for a plane-backed index (which seeks int8 records off disk by position instead).
    int8: Vec<(u64, Int8Vector)>,
    /// Effective dim of the held vectors; `0` until the first vector is ingested (D4: never assumed).
    dim: usize,
    /// On-disk plane handles for `load` + int8 seek-by-position; `None` in-memory.
    planes: Option<PlaneHandles>,
}

/// On-disk plane handles backing an index built from a provider/model's projected planes.
struct PlaneHandles {
    binary: BinaryPlane,
    int8: Int8Plane,
}

impl FlatVectorIndex {
    /// Builds an empty, purely in-memory index (no on-disk planes; `load` is a no-op).
    ///
    /// Used by the projection-free `build`-from-vectors path (the vector index benchmark). The `dim` is `0` until the
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

    /// Reloads the RESIDENT BINARY plane from disk (the int8 plane stays on disk, seeked per query).
    ///
    /// For an in-memory index (no plane handles) this is a no-op that leaves the held vectors intact.
    /// For a plane-backed index it replaces the resident binary copy with the on-disk projection,
    /// picking up a fresh build; the int8 plane is NOT loaded resident (resident = binary only),
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::vector_planes::build_planes_from_store;
    use crate::ai::vector_store::VectorStore;
    use crate::ai::{EmbeddingDtype, EmbeddingFingerprint, EmbeddingPooling};
    use crate::config::project_paths_with_root;

    #[test]
    fn the_complete_score_stream_has_no_recall_pool_and_observes_cancellation() {
        // Built the way search opens it: `.pkvec` projected to planes, binary resident, int8 read
        // off disk by position.
        let dir = tempfile::tempdir().expect("tempdir");
        let paths = project_paths_with_root(dir.path());
        let fingerprint = EmbeddingFingerprint::new(
            "static-embed",
            "model-a",
            4,
            EmbeddingDtype::Float32,
            true,
            EmbeddingPooling::Mean,
            None,
        );
        let store = VectorStore::create_stamped(&paths, &fingerprint).expect("create");
        store
            .append_vectors(
                &(1..=1207).map(|id| (id, vec![1., id as f32 / 1207., 0., 0.])).collect::<Vec<_>>(),
            )
            .expect("append");
        build_planes_from_store(&paths, "static-embed", "model-a").expect("build planes");
        let index = FlatVectorIndex::open(&paths, "static-embed", "model-a").expect("open");

        let cancel = vault_store::CancelToken::new();
        let mut keys = Vec::new();
        index
            .visit_scores(&[1., 0., 0., 0.], &cancel, |key, score| {
                assert!(score.is_finite());
                keys.push(key);
                Ok(())
            })
            .unwrap();
        keys.sort_unstable();
        assert_eq!(keys, (1..=1207).collect::<Vec<_>>());
        cancel.cancel();
        assert!(index.visit_scores(&[1., 0., 0., 0.], &cancel, |_, _| Ok(())).is_err());
    }
}
