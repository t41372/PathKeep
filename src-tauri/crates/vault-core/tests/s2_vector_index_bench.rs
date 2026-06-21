//! S2 benchmark — flat two-stage vector index at scale (W-AI-5, 04 §6 / 05 §3, §10).
//!
//! This is an INTEGRATION test (separate crate) that links `vault-core` and drives the PUBLIC
//! [`FlatVectorIndex`] + plane projection API. It is gated on `PATHKEEP_S2_BENCH=1` so it NEVER runs
//! in the coverage gate / CI (a missing/unset env var skips it); the index + quantization LOGIC it
//! exercises is fully covered by the un-gated unit tests in `ai/vector_index.rs` + `ai/vector_planes.rs`.
//!
//! It is the data that decides 05 §10's open items: binary-recall sufficiency, f32 retention, and
//! whether an index library is later warranted. It measures, at synthetic 1M / 5M / 14.4M scales:
//! - recall@10 (binary-recall + int8-rescore vs exact f32 brute force, over a query sample),
//! - latency (recall stage + rescore stage, p50 / p95),
//! - REAL resident RAM of the binary plane — the ONLY RAM-resident plane — including the Rust
//!   `(u64, Vec<u8>)` per-vector overhead, not just the packed `n × ceil(dim/8)` bytes (E1),
//! - int8 plane on-disk size (the int8 plane is now genuinely on-disk, seeked per candidate, C1),
//! - plane build (projection) time + the fact that build peak RAM is now STREAMING-bounded (C2),
//! - a small full-n exact-recall PROBE (a handful of queries brute-forced at the TRUE n) so recall@k'
//!   is reported at real scale, not only on the sub-sample (E2).
//!
//! ```sh
//! PATHKEEP_S2_BENCH=1 cargo test --manifest-path src-tauri/Cargo.toml \
//!   -p vault-core --test s2_vector_index_bench --release -- --nocapture
//! # Scale + sampling are env-tunable so a RAM/time-limited host can run the largest feasible scale:
//! #   PATHKEEP_S2_SCALES=1000000,5000000   PATHKEEP_S2_DIM=256
//! #   PATHKEEP_S2_QUERIES=200              PATHKEEP_S2_EXACT_SAMPLE=200000
//! ```
//!
//! Determinism: a SELF-CONTAINED seeded SplitMix64 PRNG (no `rand` dependency — the no-new-deps
//! constraint applies) generates the synthetic vectors + queries, so every run is reproducible.

use std::time::Instant;
use vault_core::{
    BinaryPlane, EmbeddingDtype, EmbeddingFingerprint, EmbeddingPooling, FlatVectorIndex,
    Int8Plane, RECALL_EXPANSION, RECALL_FLOOR, VectorIndex, VectorStore, binarize,
    binary_bytes_for_dim, build_planes_from_store, dot_product, hamming_distance,
    project_paths_with_root,
};

/// Default scales swept when `PATHKEEP_S2_SCALES` is unset (1M / 5M / 14.4M, the 05 §3 envelope).
const DEFAULT_SCALES: &[usize] = &[1_000_000, 5_000_000, 14_400_000];
/// Default embedding dim (the static base tier emits 256-dim, 05 §2).
const DEFAULT_DIM: usize = 256;
/// Default number of query probes used for latency percentiles + recall@10.
const DEFAULT_QUERIES: usize = 200;
/// Default ceiling on the brute-force exact set sampled for recall (full 14.4M exact is too slow).
const DEFAULT_EXACT_SAMPLE: usize = 200_000;
/// Top-k the recall metric is computed at (recall@10).
const TOP_K: usize = 10;
/// Number of queries brute-forced at the FULL n for the honest full-scale recall probe (E2).
///
/// A full per-query exact top-k over n is O(n·dim), so only a handful are run — enough to report
/// recall@k' at REAL scale (not just the sub-sample) without making the benchmark intractable.
const DEFAULT_FULL_PROBES: usize = 20;

/// A tiny deterministic SplitMix64 PRNG (public-domain algorithm), self-contained (no `rand` dep).
struct SplitMix64 {
    state: u64,
}

impl SplitMix64 {
    fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    /// Returns the next pseudo-random u64.
    fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Returns a pseudo-random f32 in `[-1, 1)`.
    fn next_unit(&mut self) -> f32 {
        // 24 bits of mantissa precision mapped to [0,1), then to [-1,1).
        let bits = (self.next_u64() >> 40) as f32; // 24-bit value
        (bits / (1u32 << 24) as f32) * 2.0 - 1.0
    }
}

/// L2-normalizes a vector in place and returns it.
fn normalize(mut vector: Vec<f32>) -> Vec<f32> {
    let norm = vector.iter().map(|value| value * value).sum::<f32>().sqrt();
    if norm > 0.0 {
        for value in &mut vector {
            *value /= norm;
        }
    }
    vector
}

/// Generates one L2-normalized uniform-random vector of `dim` (used for cluster centroids + queries).
fn random_unit_vector(rng: &mut SplitMix64, dim: usize) -> Vec<f32> {
    normalize((0..dim).map(|_| rng.next_unit()).collect())
}

/// Number of synthetic clusters the corpus is drawn from.
///
/// Real embeddings have cluster structure (pages about one topic land near each other); a PURELY
/// uniform-random corpus is the pathological case where every vector is ~orthogonal and the "true"
/// top-k is noise-level — meaningless for a recall metric. Drawing each vector as `centroid + noise`
/// gives a realistic neighbourhood structure binary recall can actually resolve, so recall@10
/// reflects the engine, not the (absent) structure of white noise.
const CLUSTERS: usize = 4_096;
/// Per-component Gaussian-ish noise scale added to a centroid to form a corpus vector.
const CLUSTER_NOISE: f32 = 0.35;
/// Per-component noise added to a source corpus vector to form a perturbed query (small → the query
/// is a near-copy of one document, so its nearest neighbour is well-defined).
const QUERY_NOISE: f32 = 0.08;

/// Generates one clustered synthetic vector: a random centroid perturbed by bounded noise.
///
/// `centroids` is the fixed set of cluster centers; each call picks one and adds per-component noise,
/// then renormalizes — so the corpus has genuine near-neighbours (same-cluster vectors) the way real
/// embeddings do.
fn clustered_unit_vector(rng: &mut SplitMix64, centroids: &[Vec<f32>], dim: usize) -> Vec<f32> {
    let centroid = &centroids[(rng.next_u64() as usize) % centroids.len()];
    let vector: Vec<f32> =
        (0..dim).map(|index| centroid[index] + rng.next_unit() * CLUSTER_NOISE).collect();
    normalize(vector)
}

/// Builds the fixed cluster centroids for one scale (deterministic from the seed).
fn build_centroids(seed: u64, dim: usize) -> Vec<Vec<f32>> {
    let mut rng = SplitMix64::new(seed ^ 0xC0FF_EE00_C0FF_EE00);
    (0..CLUSTERS).map(|_| random_unit_vector(&mut rng, dim)).collect()
}

/// Reads a usize env var or falls back to the default.
fn env_usize(key: &str, fallback: usize) -> usize {
    std::env::var(key).ok().and_then(|value| value.parse().ok()).unwrap_or(fallback)
}

/// Parses the comma-separated scale list from env, or the default sweep.
fn env_scales() -> Vec<usize> {
    match std::env::var("PATHKEEP_S2_SCALES") {
        Ok(value) => {
            value.split(',').filter_map(|item| item.trim().parse::<usize>().ok()).collect()
        }
        Err(_) => DEFAULT_SCALES.to_vec(),
    }
}

/// Returns the p50 / p95 of a sorted-in-place latency sample in microseconds.
fn percentiles(samples: &mut [f64]) -> (f64, f64) {
    samples.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let pick = |q: f64| -> f64 {
        if samples.is_empty() {
            return 0.0;
        }
        let index = ((samples.len() as f64 - 1.0) * q).round() as usize;
        samples[index]
    };
    (pick(0.50), pick(0.95))
}

/// Formats a byte count as a human MB string.
fn mb(bytes: u64) -> f64 {
    bytes as f64 / (1024.0 * 1024.0)
}

#[test]
fn s2_flat_vector_index_scale_benchmark() {
    if std::env::var("PATHKEEP_S2_BENCH").as_deref() != Ok("1") {
        eprintln!(
            "skipping S2 vector-index benchmark: set PATHKEEP_S2_BENCH=1 (use --release for real numbers)"
        );
        return;
    }

    let dim = env_usize("PATHKEEP_S2_DIM", DEFAULT_DIM);
    let query_count = env_usize("PATHKEEP_S2_QUERIES", DEFAULT_QUERIES);
    let exact_sample = env_usize("PATHKEEP_S2_EXACT_SAMPLE", DEFAULT_EXACT_SAMPLE);
    let full_probes = env_usize("PATHKEEP_S2_FULL_PROBES", DEFAULT_FULL_PROBES);
    let scales = env_scales();

    // The per-vector resident overhead of the binary plane's `Vec<(u64, Vec<u8>)>`: the tuple's u64 key
    // + the inner `Vec<u8>` pointer/len/cap header. The packed bits add `ceil(dim/8)` on top. Reporting
    // this is the E1 honesty fix — the old "n × 32 B" undercounted real RAM.
    let binary_overhead = std::mem::size_of::<(u64, Vec<u8>)>();

    println!("\n=== S2 flat vector index benchmark (W-AI-5) ===");
    println!(
        "dim={dim}  queries={query_count}  exact_sample={exact_sample}  full_probes={full_probes}  top_k={TOP_K}  scales={scales:?}"
    );
    println!(
        "recall pool k' = max(top_k * {RECALL_EXPANSION}, RECALL_FLOOR={RECALL_FLOOR}) capped at n"
    );
    println!(
        "binary plane resident = {} (struct) + {} (packed bits) B/vec; int8 plane ON-DISK = {} B/vec (+ scale); f32 source = {} B/vec",
        binary_overhead,
        binary_bytes_for_dim(dim),
        4 + dim,
        4 * dim
    );

    for &n in &scales {
        run_scale(n, dim, query_count, exact_sample, full_probes, binary_overhead);
    }
    println!("=== end S2 benchmark ===\n");
}

/// Runs one scale of the benchmark and prints its report row.
fn run_scale(
    n: usize,
    dim: usize,
    query_count: usize,
    exact_sample: usize,
    full_probes: usize,
    binary_overhead: usize,
) {
    println!("\n--- scale n={n} ---");
    let centroids = build_centroids(S2_SEED ^ (n as u64), dim);
    let mut rng = SplitMix64::new(S2_SEED ^ (n as u64));

    // Build the index in RAM. We feed the index directly so the resident-RAM + search numbers reflect
    // the live engine; the on-disk plane projection (below) gives the int8 size + build time.
    let build_start = Instant::now();
    let mut index = FlatVectorIndex::empty();
    // Append in batches to bound peak transient allocation.
    const BATCH: usize = 50_000;
    let mut next_key: u64 = 1;
    let mut appended = 0usize;
    while appended < n {
        let take = BATCH.min(n - appended);
        let mut batch: Vec<(u64, Vec<f32>)> = Vec::with_capacity(take);
        for _ in 0..take {
            batch.push((next_key, clustered_unit_vector(&mut rng, &centroids, dim)));
            next_key += 1;
        }
        index.append(&batch).expect("append batch");
        appended += take;
    }
    let build_secs = build_start.elapsed().as_secs_f64();
    assert_eq!(index.len(), n);

    // REAL resident RAM of the binary plane (the only plane held resident at query time, C1): the Rust
    // `(u64, Vec<u8>)` struct overhead PLUS the packed bits, per vector — not just the packed bytes.
    // This is the honest footprint the 8 GB envelope must hold (E1).
    let binary_ram = (n as u64) * (binary_overhead + binary_bytes_for_dim(dim)) as u64;

    // Sample a subset for the exact brute-force reference (full n is too slow for a per-query exact).
    // The sampled subset is the first `exact_sample` keys (1..=exact_sample); we score recall only on
    // queries whose exact top-k lives within that subset by restricting BOTH stages to the subset.
    let sample_n = exact_sample.min(n);
    let sample_vectors: Vec<(u64, Vec<f32>)> = {
        // Re-derive the first `sample_n` vectors deterministically for the exact reference (same seed
        // + centroids + draw order as the corpus, so these ARE the first `sample_n` corpus vectors).
        let mut sample_rng = SplitMix64::new(S2_SEED ^ (n as u64));
        (1..=sample_n as u64)
            .map(|key| (key, clustered_unit_vector(&mut sample_rng, &centroids, dim)))
            .collect()
    };
    // A sub-index over just the sample so recall@10 compares flat-vs-exact on the SAME population.
    let mut sample_index = FlatVectorIndex::empty();
    sample_index.build(&sample_vectors).expect("build sample index");

    // Query probes: each query is a KNOWN sample vector perturbed by small noise, so it has a single
    // well-defined nearest neighbour (its source) plus that source's cluster as the relevant set. This
    // is the standard "perturbed query" recall setup — it removes the within-cluster-tie ambiguity of
    // a from-scratch random query, so recall@10 measures whether the engine RETRIEVES a vector it is a
    // near-copy of, the property that matters for real query→document matching.
    let mut query_rng = SplitMix64::new(S2_SEED ^ 0xDEAD_BEEF ^ (n as u64));
    // Track each query's source key so recall@1-of-source (did the engine find the exact document the
    // query is a near-copy of?) can be reported — the most interpretable signal for a perturbed query.
    let mut query_source_keys: Vec<u64> = Vec::with_capacity(query_count);
    let queries: Vec<Vec<f32>> = (0..query_count)
        .map(|_| {
            let (source_key, source) =
                &sample_vectors[(query_rng.next_u64() as usize) % sample_vectors.len()];
            query_source_keys.push(*source_key);
            let perturbed: Vec<f32> =
                source.iter().map(|value| value + query_rng.next_unit() * QUERY_NOISE).collect();
            normalize(perturbed)
        })
        .collect();

    // Latency: full-scale search over the resident index (recall + rescore combined; we also time the
    // recall stage alone by searching with k=TOP_K and reading the elapsed, since the two-stage call
    // is the user-visible latency).
    let mut latencies_us: Vec<f64> = Vec::with_capacity(query_count);
    for query in &queries {
        let start = Instant::now();
        let results = index.search(query, TOP_K, None).expect("search");
        latencies_us.push(start.elapsed().as_secs_f64() * 1e6);
        assert!(results.len() <= TOP_K);
    }
    let (p50, p95) = percentiles(&mut latencies_us);

    // Precompute the sample's packed binary codes once (the stage-1 recall input), so the binary
    // coverage diagnostic does not re-binarize per query.
    let sample_codes: Vec<(u64, Vec<u8>)> =
        sample_vectors.iter().map(|(key, vector)| (*key, binarize(vector))).collect();

    // recall@10 over the sampled population: flat (binary recall + int8 rescore) vs exact f32. We also
    // report the BINARY-RECALL coverage (does the stage-1 pool of k' candidates contain the exact
    // top-10?) so the report attributes any miss to recall (stage 1) vs rescore (stage 2). The pool is
    // the SAME `max(k * expansion, RECALL_FLOOR)` the engine uses (X-1), capped at the sampled size.
    let recall_pool = (TOP_K * RECALL_EXPANSION).max(RECALL_FLOOR).min(sample_n);
    let mut recall_hits = 0usize;
    let mut recall_total = 0usize;
    let mut binary_coverage_hits = 0usize;
    // recall@1-of-source: did the engine's top-1 == the document the query is a near-copy of?
    let mut source_top1_hits = 0usize;
    for (query, source_key) in queries.iter().zip(query_source_keys.iter()) {
        let flat_ranked: Vec<u64> = sample_index
            .search(query, TOP_K, None)
            .expect("sample flat search")
            .into_iter()
            .map(|(key, _)| key)
            .collect();
        if flat_ranked.first() == Some(source_key) {
            source_top1_hits += 1;
        }
        let exact = exact_top_k(query, &sample_vectors, TOP_K);
        let exact_set: std::collections::HashSet<u64> = exact.iter().copied().collect();
        recall_hits += flat_ranked.iter().filter(|key| exact_set.contains(key)).count();
        recall_total += exact.len();
        // Stage-1 coverage: how many of the exact top-10 live inside the binary-recalled k' pool.
        let pool = binary_recall_pool(query, &sample_codes, recall_pool);
        binary_coverage_hits += pool.iter().filter(|key| exact_set.contains(key)).count();
    }
    let recall_at_10 = recall_hits as f64 / recall_total.max(1) as f64;
    let binary_coverage = binary_coverage_hits as f64 / recall_total.max(1) as f64;
    let source_top1 = source_top1_hits as f64 / query_count.max(1) as f64;

    // Sweep the binary-recall pool depth k' to quantify the recall/latency tradeoff: deeper pools
    // recover more of the exact top-10 at the cost of a larger int8 rescore. This is the data the
    // report uses to recommend a final k' (and whether flat-binary at any feasible k' is enough).
    let pool_sweep: Vec<(usize, f64)> = [10, 50, 100, 500, 1000, 5000]
        .into_iter()
        .map(|multiplier| {
            let pool_k = (TOP_K * multiplier).min(sample_n);
            let mut hits = 0usize;
            let mut total = 0usize;
            for query in &queries {
                let exact: std::collections::HashSet<u64> =
                    exact_top_k(query, &sample_vectors, TOP_K).into_iter().collect();
                let pool = binary_recall_pool(query, &sample_codes, pool_k);
                hits += pool.iter().filter(|key| exact.contains(key)).count();
                total += exact.len();
            }
            (pool_k, hits as f64 / total.max(1) as f64)
        })
        .collect();

    // FULL-n exact-recall probe (E2): brute-force a handful of queries at the TRUE n so recall@k' is
    // reported at real scale, not only on the sub-sample. Each probe re-derives the n corpus vectors
    // deterministically in a STREAM (never materializing all n) to track the exact top-k, then compares
    // to the engine's full-index search. The query is a known sample vector perturbed (so its source is
    // the well-defined nearest neighbour), reusing the first few of the latency queries.
    let probe_count = full_probes.min(query_count);
    let mut full_recall_hits = 0usize;
    let mut full_recall_total = 0usize;
    for query in queries.iter().take(probe_count) {
        let exact: std::collections::HashSet<u64> =
            exact_top_k_streaming(query, n, dim, &centroids, S2_SEED ^ (n as u64), TOP_K)
                .into_iter()
                .collect();
        let flat: Vec<u64> = index
            .search(query, TOP_K, None)
            .expect("full probe search")
            .into_iter()
            .map(|(key, _)| key)
            .collect();
        full_recall_hits += flat.iter().filter(|key| exact.contains(key)).count();
        full_recall_total += exact.len();
    }
    let full_recall = full_recall_hits as f64 / full_recall_total.max(1) as f64;

    // On-disk plane projection: int8 size + build time + STREAMING build peak, via the real `.pkvec` ->
    // plane pipeline. We project the SAMPLE subset (projecting full n would duplicate the whole corpus
    // on disk); int8 size per vector is constant, so we report per-vector + extrapolate to n. The
    // projection now STREAMS the `.pkvec` source straight into both plane writers (C2), so build peak
    // RAM is bounded by the dedup key-set + write buffers + one f32 record — CONSTANT in dim, O(keys)
    // not O(n·dim), i.e. it never materializes the ~14.7 GB f32 SET the old `read_all` path held.
    let temp = tempfile::tempdir().expect("tempdir");
    let paths = project_paths_with_root(temp.path());
    let fingerprint = EmbeddingFingerprint::new(
        "s2-bench",
        "synthetic",
        dim,
        EmbeddingDtype::Float32,
        true,
        EmbeddingPooling::Mean,
        None,
    );
    let store = VectorStore::create_stamped(&paths, &fingerprint).expect("create store");
    store.append_vectors(&sample_vectors).expect("append store");
    let project_start = Instant::now();
    let report = build_planes_from_store(&paths, "s2-bench", "synthetic").expect("build planes");
    let project_secs = project_start.elapsed().as_secs_f64();
    assert_eq!(report.vectors, sample_n);
    let int8_plane = Int8Plane::for_provider(&paths, "s2-bench", "synthetic");
    let binary_plane = BinaryPlane::for_provider(&paths, "s2-bench", "synthetic");
    let int8_sample_bytes = int8_plane.path().metadata().expect("int8 meta").len();
    let binary_sample_bytes = binary_plane.path().metadata().expect("binary meta").len();
    let int8_bytes_per_vec = int8_sample_bytes as f64 / sample_n.max(1) as f64;
    let int8_full_bytes = (int8_bytes_per_vec * n as f64) as u64;
    // Projection time extrapolated from the sample to n (linear projection, O(n)).
    let project_full_secs = project_secs * (n as f64 / sample_n.max(1) as f64);
    // Streaming build-peak estimate @n: the dedup key-set (8 B + ~8 B offset / key incl. HashMap load
    // factor, ~16 B/key) + the first-seen `order` Vec (8 B/key) + two small write buffers + one f32
    // record. We report the dominant key-set term so the "build peak ≪ 1 GB at any scale" claim is
    // grounded; the f32 source is NEVER resident.
    let build_peak_bytes = (n as u64) * 24 + (4 * dim) as u64;

    println!("build (in-RAM append) : {build_secs:.2} s  ({:.0} vec/s)", n as f64 / build_secs);
    println!(
        "binary plane RESIDENT : {:.1} MB @n incl. Rust overhead ({} struct + {} bits B/vec)  [on-disk sample {:.1} MB]",
        mb(binary_ram),
        binary_overhead,
        binary_bytes_for_dim(dim),
        mb(binary_sample_bytes)
    );
    println!(
        "int8 plane ON-DISK    : {:.1} MB @n  ({:.0} B/vec)  [seeked per candidate, NOT resident; projected from {}-vec sample]",
        mb(int8_full_bytes),
        int8_bytes_per_vec,
        sample_n
    );
    println!(
        "build peak RAM (stream): ~{:.1} MB @n  (dedup key-set + write buffers + 1 f32 record; f32 source NEVER resident)",
        mb(build_peak_bytes)
    );
    println!(
        "plane build (project) : {:.2} s @n (extrapolated from {:.3} s / {}-vec sample, STREAMING)",
        project_full_secs, project_secs, sample_n
    );
    println!("search latency (2-stage, k={TOP_K}) : p50 {p50:.0} us   p95 {p95:.0} us");
    if sample_n < n {
        println!(
            "WARNING: recall@10 + coverage + pool-sweep below are measured on a {sample_n}-vec SUB-SAMPLE (sample_n < n={n}); the full-n probe row is the real-scale check."
        );
    }
    println!(
        "recall@10 (flat vs exact, {}-vec SUB-SAMPLE) : {:.4}  [binary-recall coverage of exact top-10: {:.4}, pool k'={}]",
        sample_n, recall_at_10, binary_coverage, recall_pool
    );
    println!(
        "recall@{TOP_K} FULL-n PROBE (flat vs exact at TRUE n={n}, {probe_count} queries) : {full_recall:.4}"
    );
    println!(
        "recall@1-of-source (top-1 == the doc the query is a near-copy of) : {source_top1:.4}"
    );
    print!("binary-recall coverage of exact top-10 by pool depth k' :");
    for (pool_k, coverage) in &pool_sweep {
        print!("  k'={pool_k}->{coverage:.3}");
    }
    println!();
}

/// Exact f32 brute-force top-k by cosine (= dot, vectors are unit-norm) over a population.
fn exact_top_k(query: &[f32], population: &[(u64, Vec<f32>)], k: usize) -> Vec<u64> {
    let mut scored: Vec<(u64, f32)> =
        population.iter().map(|(key, vector)| (*key, dot_product(query, vector))).collect();
    scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap().then(a.0.cmp(&b.0)));
    scored.truncate(k);
    scored.into_iter().map(|(key, _)| key).collect()
}

/// Exact f32 brute-force top-k over the FULL n corpus, re-derived in a STREAM (never materialized).
///
/// The full-n recall probe (E2) must brute-force at the TRUE n, but holding n f32 vectors resident
/// would defeat the memory story the benchmark is asserting. So this re-generates the corpus exactly as
/// `run_scale` did (same seed + centroids + draw order ⇒ the SAME keys 1..=n and the SAME vectors) one
/// vector at a time, keeping only the running top-k. O(n·dim) time, O(k) memory.
fn exact_top_k_streaming(
    query: &[f32],
    n: usize,
    dim: usize,
    centroids: &[Vec<f32>],
    seed: u64,
    k: usize,
) -> Vec<u64> {
    let mut rng = SplitMix64::new(seed);
    // Keep the running top-k as (score, key); a tiny linear-insert heap is fine for k≈10.
    let mut top: Vec<(f32, u64)> = Vec::with_capacity(k + 1);
    for key in 1..=n as u64 {
        let vector = clustered_unit_vector(&mut rng, centroids, dim);
        let score = dot_product(query, &vector);
        // Insert if the running top-k is not yet full or this beats its weakest member.
        if top.len() < k || score > top[top.len() - 1].0 {
            // Position by score desc, then key asc — the SAME total order as `exact_top_k`.
            let position = top
                .iter()
                .position(|(existing, existing_key)| {
                    score > *existing || (score == *existing && key < *existing_key)
                })
                .unwrap_or(top.len());
            top.insert(position, (score, key));
            top.truncate(k);
        }
    }
    top.into_iter().map(|(_, key)| key).collect()
}

/// Stage-1 binary recall: the `pool_k` smallest-Hamming candidates for `query` over packed codes.
///
/// Mirrors [`FlatVectorIndex`]'s stage-1 exactly (binarize query, popcount-xor against each code,
/// take the smallest distances) so the coverage diagnostic measures the SAME recall the engine does.
fn binary_recall_pool(query: &[f32], codes: &[(u64, Vec<u8>)], pool_k: usize) -> Vec<u64> {
    let query_bits = binarize(query);
    let mut scored: Vec<(u32, u64)> =
        codes.iter().map(|(key, bits)| (hamming_distance(&query_bits, bits), *key)).collect();
    scored.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));
    scored.truncate(pool_k);
    scored.into_iter().map(|(_, key)| key).collect()
}

/// Fixed seed for the synthetic corpus (XORed with the scale so each scale is a distinct population).
const S2_SEED: u64 = 0x5032_424E_4348_3031; // an arbitrary fixed constant for reproducibility.
