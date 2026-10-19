//! Responsibilities: map the int8 projection and score every vector in plane order.
//! Not responsible for: canonical eligibility, page fusion or embedding generation.
//! Failure modes: a candidate pool hides a distant vector; a query of another dimension is scored;
//! cancellation is ignored on a large plane. Tests: below, and the complete-pool suites in
//! `ai/tests.rs`; `tests/vector_scan_bench` for cost.
use crate::ai::traits::VectorIndex;
use crate::ai::vector_planes::{Int8Plane, MappedInt8Plane};
use crate::config::ProjectPaths;
use anyhow::Result;
use std::collections::HashSet;

/// Keeps query memory bounded by mapping the immutable score plane.
pub struct FlatVectorIndex {
    mapped: Option<MappedInt8Plane>,
    plane: Int8Plane,
}
impl FlatVectorIndex {
    /// Opens headers and maps the int8 file; sign bits are not needed for complete scoring.
    pub fn open(paths: &ProjectPaths, provider: &str, model: &str) -> Result<Self> {
        let mut index =
            Self { plane: Int8Plane::for_provider(paths, provider, model), mapped: None };
        index.load()?;
        Ok(index)
    }
    pub fn len(&self) -> usize {
        self.mapped.as_ref().map_or(0, |plane| plane.len())
    }
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    pub fn dim(&self) -> usize {
        self.mapped.as_ref().map_or(0, |plane| plane.header.dim)
    }
    /// Positions bind scores to the same immutable projection throughout a query.
    pub fn key(&self, position: usize) -> u64 {
        self.mapped.as_ref().expect("positions require a mapped plane").key(position)
    }
    /// A zero record is missing evidence, rather than a valid neighbour with similarity zero.
    pub(super) fn has_vector(&self, position: usize) -> bool {
        let plane = self.mapped.as_ref().expect("positions require a mapped plane");
        let start = position * plane.stride();
        plane.records()[start + 12..start + plane.stride()].iter().any(|code| *code != 0)
    }
    /// Period topics reuse the serving projection instead of loading the f32 source corpus.
    pub(super) fn vector_at(&self, position: usize) -> Vec<f32> {
        let plane = self.mapped.as_ref().expect("positions require a mapped plane");
        let start = position * plane.stride();
        let record = &plane.records()[start..start + plane.stride()];
        let scale = f32::from_le_bytes(record[8..12].try_into().unwrap());
        record[12..].iter().map(|byte| *byte as i8 as f32 * scale).collect()
    }
    /// Samples the same quantised dot-product scale used by the complete scan.
    pub(super) fn score_pair(&self, a: usize, b: usize) -> f32 {
        let query = self.vector_at(a);
        let plane = self.mapped.as_ref().expect("positions require a mapped plane");
        let start = b * plane.stride();
        let record = &plane.records()[start..start + plane.stride()];
        scaled_dot(&query, &record[12..], f32::from_le_bytes(record[8..12].try_into().unwrap()))
    }
    pub(super) fn projection_id(&self) -> &str {
        self.mapped.as_ref().map_or("", |plane| plane.header.projection_id.as_str())
    }
    pub(super) fn fingerprint_hash(&self) -> &str {
        self.mapped.as_ref().map_or("", |plane| plane.header.fingerprint_hash.as_str())
    }
    /// Scores the complete plane with the original f32 query, without query quantisation.
    pub fn score_all(&self, query: &[f32], cancel: &vault_store::CancelToken) -> Result<Vec<f32>> {
        cancel.check()?;
        if self.is_empty() {
            return Ok(Vec::new());
        }
        anyhow::ensure!(query.len() == self.dim(), "query dimension does not match index");
        let plane = self.mapped.as_ref().expect("nonempty index has a mapped plane");
        let mut scores = vec![0.; self.len()];
        let threads = std::thread::available_parallelism().map_or(1, usize::from).min(4);
        let chunk = self.len().div_ceil(threads);
        std::thread::scope(|scope| -> Result<()> {
            let handles = scores
                .chunks_mut(chunk)
                .enumerate()
                .map(|(part, output)| {
                    scope.spawn(move || -> Result<()> {
                        let stride = plane.stride();
                        let start = part * chunk * stride;
                        let records = &plane.records()[start..start + output.len() * stride];
                        for (block, out) in records.chunks(256 * stride).zip(output.chunks_mut(256))
                        {
                            cancel.check()?;
                            for (record, score) in block.chunks_exact(stride).zip(out) {
                                let scale = f32::from_le_bytes(record[8..12].try_into().unwrap());
                                *score = scaled_dot(query, &record[12..], scale);
                            }
                        }
                        Ok(())
                    })
                })
                .collect::<Vec<_>>();
            for handle in handles {
                handle.join().expect("vector scoring worker panicked")?;
            }
            Ok(())
        })?;
        Ok(scores)
    }
}

// Independent lanes let the compiler vectorise without a loop-carried f32 reduction.
fn scaled_dot(query: &[f32], codes: &[u8], scale: f32) -> f32 {
    let mut sums = [0f32; 8];
    let mut q = query.chunks_exact(8);
    let mut c = codes.chunks_exact(8);
    for (q, c) in q.by_ref().zip(c.by_ref()) {
        for lane in 0..8 {
            sums[lane] += q[lane] * c[lane] as i8 as f32;
        }
    }
    let mut sum = sums.into_iter().sum::<f32>();
    for (q, c) in q.remainder().iter().zip(c.remainder()) {
        sum += q * *c as i8 as f32;
    }
    scale * sum
}
impl VectorIndex for FlatVectorIndex {
    fn search(
        &self,
        query: &[f32],
        k: usize,
        allowlist: Option<&[u64]>,
    ) -> Result<Vec<(u64, f32)>> {
        let allow = allowlist.map(|ids| ids.iter().copied().collect::<HashSet<_>>());
        let mut scores = self
            .score_all(query, &vault_store::CancelToken::new())?
            .into_iter()
            .enumerate()
            .filter_map(|(position, score)| {
                let key = self.key(position);
                allow.as_ref().is_none_or(|set| set.contains(&key)).then_some((key, score))
            })
            .collect::<Vec<_>>();
        scores.sort_unstable_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
        scores.truncate(k);
        Ok(scores)
    }
    fn load(&mut self) -> Result<()> {
        self.mapped = self.plane.map()?;
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
        // Exercise the mapped production index, including complete coverage and cancellation.
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
        let scores = index.score_all(&[1., 0., 0., 0.], &cancel).unwrap();
        assert!(scores.iter().all(|score| score.is_finite()));
        let mut keys = (0..scores.len()).map(|position| index.key(position)).collect::<Vec<_>>();
        keys.sort_unstable();
        assert_eq!(keys, (1..=1207).collect::<Vec<_>>());
        cancel.cancel();
        assert!(index.score_all(&[1., 0., 0., 0.], &cancel).is_err());
    }
}
