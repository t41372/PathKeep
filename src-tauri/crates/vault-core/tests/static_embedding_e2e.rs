//! Env-gated real-machine tests for the in-app HAND-ROLLED static (model2vec) engine (W-AI-4c).
//!
//! These link `vault-core` as a normal dependency, so they exercise the REAL safetensors matrix
//! load, tokenizer, and hf-hub download path (NOT the `cfg(test)` stub). They are gated on env vars
//! so they NEVER run in the coverage gate / CI:
//!
//! - `PATHKEEP_STATIC_S1=1` → S1 throughput benchmark (docs/sec single + batched, plus the
//!   extrapolated 14.4M first-fill wall clock). The static tier is the WHOLE point of W-AI-4c: it
//!   must be ORDERS faster than candle's ~1.25 d/s (so the 14.4M first fill is minutes, not days).
//! - `PATHKEEP_STATIC_PARITY=1` → hand-roll-vs-reference cosine parity (THE static correctness
//!   proof): the SAME texts embedded by our hand-rolled engine and by the Python model2vec reference
//!   must have cosine > 0.999. The reference vectors are PINNED in a committed fixture
//!   (`tests/fixtures/static_parity_potion_multilingual.json`), generated ONCE from Python model2vec
//!   (`StaticModel.from_pretrained("minishlab/potion-multilingual-128M").encode(texts)`). The test is
//!   a STANDING gate: it does NOT depend on Python at test time, only on the committed reference and
//!   the (env-gated) downloaded model bytes. The fixture corpus deliberately includes CJK, emoji,
//!   URLs, percent-encoded paths, AND truly out-of-vocabulary rows (ancient scripts / exotic symbol
//!   planes) that the Unigram tokenizer maps to its `[UNK]` row. Those OOV rows PROVE the S1 unk
//!   handling is correct for THIS model: `potion-multilingual-128M` ships a Unigram tokenizer with no
//!   string `unk_token`, so model2vec POOLS `[UNK]` (it only drops unk for BPE/WordPiece models that
//!   declare a string `unk_token`). The engine mirrors that exactly — dropping the Unigram `[UNK]`
//!   would COLLAPSE those rows to ~0.80 cosine, which is precisely the divergence this gate catches.
//!
//! Both reuse a persistent model directory under `PATHKEEP_STATIC_MODELS_DIR` (default
//! `<repo>/artifacts/static-models`) so the matrix downloads once and is reused.
//!
//! ```sh
//! PATHKEEP_STATIC_S1=1 cargo test --manifest-path src-tauri/Cargo.toml \
//!   -p vault-core --test static_embedding_e2e --release -- --nocapture
//! PATHKEEP_STATIC_PARITY=1 \
//!   cargo test --manifest-path src-tauri/Cargo.toml \
//!   -p vault-core --test static_embedding_e2e --release -- --nocapture
//! ```
//!
//! Regenerating the pinned fixture (only when the model or input set changes), from a venv with
//! `pip install model2vec`:
//!
//! ```sh
//! python scripts/gen_static_parity_fixture.py \
//!   src-tauri/crates/vault-core/tests/fixtures/static_parity_potion_multilingual.json
//! ```
//!
//! Compiled out under the `coverage` cfg: these drive the REAL static load/download path
//! (`load_default`/`ensure_model_downloaded`), which is `cfg(not(any(test, coverage)))`. The coverage
//! gate exercises the in-crate stub instead.
#![cfg(not(coverage))]

use std::path::PathBuf;
use std::time::Instant;

use vault_core::{
    DEFAULT_STATIC_MODEL_FILES, DEFAULT_STATIC_MODEL_REPO, EmbeddingProvider, EmbeddingRole,
    NoopDownloadProgress, StaticEmbeddingProvider, ensure_model_downloaded,
};

/// Resolves a persistent models root so the matrix downloads once across runs.
fn models_root() -> PathBuf {
    if let Ok(dir) = std::env::var("PATHKEEP_STATIC_MODELS_DIR") {
        return PathBuf::from(dir);
    }
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../artifacts/static-models")
}

/// Builds a `ProjectPaths`-shaped root whose `models_dir` is the persistent cache.
fn project_paths() -> vault_core::ProjectPaths {
    let root = models_root();
    std::fs::create_dir_all(root.join("models")).expect("create models dir");
    vault_core::project_paths_with_root(&root)
}

/// Downloads (with consent) + loads the real static engine.
///
/// NOTE: requires the real SHA-256 digests pinned in `DEFAULT_STATIC_MODEL_FILES`. On the FIRST real
/// download, capture the printed digests (verification will fail with the expected-vs-got values) and
/// paste them into the manifest, then re-run — the same one-time pinning flow W-AI-4b used.
fn load_static_default() -> StaticEmbeddingProvider {
    let paths = project_paths();
    let mut progress = NoopDownloadProgress;
    ensure_model_downloaded(
        &paths,
        DEFAULT_STATIC_MODEL_REPO,
        DEFAULT_STATIC_MODEL_FILES,
        true,
        &mut progress,
    )
    .expect("download + verify potion-multilingual-128M");
    StaticEmbeddingProvider::load_default(&paths).expect("load static engine")
}

fn cosine(a: &[f32], b: &[f32]) -> f32 {
    assert_eq!(a.len(), b.len(), "vectors must share a dimension");
    let dot: f32 = a.iter().zip(b).map(|(x, y)| x * y).sum();
    let na: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let nb: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    dot / (na * nb)
}

#[tokio::test]
async fn static_s1_throughput_benchmark() {
    if std::env::var("PATHKEEP_STATIC_S1").as_deref() != Ok("1") {
        eprintln!(
            "skipping static S1: set PATHKEEP_STATIC_S1=1 (downloads ~512 MB potion matrix on first run)"
        );
        return;
    }
    let provider = load_static_default();
    let corpus: Vec<String> = (0..512)
        .map(|i| {
            format!(
                "Profile: default\nVisited at: 2026-06-{:02}T10:00:00Z\nURL: https://example.com/page-{i}\nDomain: example.com\nTitle: Example article number {i} about rust embeddings and history search",
                (i % 28) + 1
            )
        })
        .collect();

    // Warm up.
    let _ = provider.embed(&corpus[..1], EmbeddingRole::Document).await.expect("warmup");

    let single_count = 64usize;
    let single_start = Instant::now();
    for text in corpus.iter().take(single_count) {
        let _ = provider
            .embed(std::slice::from_ref(text), EmbeddingRole::Document)
            .await
            .expect("single embed");
    }
    let single_dps = single_count as f64 / single_start.elapsed().as_secs_f64();

    let batch_start = Instant::now();
    let vectors = provider.embed(&corpus, EmbeddingRole::Document).await.expect("batch embed");
    let batch_dps = corpus.len() as f64 / batch_start.elapsed().as_secs_f64();

    let dim = vectors[0].len();
    let eta_14m_minutes = 14_400_000.0 / batch_dps / 60.0;
    eprintln!("\n=== W-AI-4c S1 throughput (HAND-ROLLED static potion-multilingual-128M, CPU) ===");
    eprintln!(
        "| static | dim {dim} | single {single_dps:.0} d/s | batched {batch_dps:.0} d/s | 14.4M ETA {eta_14m_minutes:.1} min |"
    );
    eprintln!("(candle Q8_0 baseline: ~1.25 d/s → 14.4M ≈ days; static should be ORDERS faster)");
    eprintln!("=== end S1 ===\n");
    assert!(batch_dps > 100.0, "static throughput must be ORDERS faster than candle (>100 d/s)");
}

/// Path to the COMMITTED Python-model2vec reference fixture (the standing parity gate).
fn parity_fixture_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/static_parity_potion_multilingual.json")
}

#[tokio::test]
async fn static_vs_reference_cosine_parity() {
    if std::env::var("PATHKEEP_STATIC_PARITY").as_deref() != Ok("1") {
        eprintln!(
            "skipping static parity: set PATHKEEP_STATIC_PARITY=1 (downloads the model on first run)"
        );
        return;
    }
    // THE correctness proof: compare the hand-rolled engine against PINNED Python-model2vec reference
    // vectors (committed fixture). The corpus includes CJK/emoji/URL/percent-encoded AND truly-OOV
    // ([UNK]-producing) inputs — so this FAILS (cosine ~0.80 on the OOV rows) unless the engine
    // handles unk EXACTLY like model2vec: for potion's Unigram tokenizer (no string `unk_token`),
    // model2vec POOLS [UNK], so the engine must too (S1).
    let bytes = std::fs::read(parity_fixture_path()).expect("read committed parity fixture");
    let reference: serde_json::Value =
        serde_json::from_slice(&bytes).expect("parse parity fixture json");
    let texts: Vec<String> = reference["texts"]
        .as_array()
        .expect("texts array")
        .iter()
        .map(|t| t.as_str().expect("text string").to_string())
        .collect();
    let ref_vectors = reference["vectors"].as_array().expect("vectors array");
    assert_eq!(texts.len(), ref_vectors.len(), "fixture texts and vectors must align");

    let provider = load_static_default();
    let ours = provider.embed(&texts, EmbeddingRole::Document).await.expect("static embed");

    eprintln!("\n=== W-AI-4c static parity (hand-rolled vs PINNED model2vec reference) ===");
    eprintln!("static dim = {} | fixture rows = {}", ours[0].len(), texts.len());
    let mut min_cosine = f32::INFINITY;
    for (i, text) in texts.iter().enumerate() {
        let ref_vec: Vec<f32> = ref_vectors[i]
            .as_array()
            .expect("ref vector")
            .iter()
            .map(|v| v.as_f64().expect("f64") as f32)
            .collect();
        let c = cosine(&ours[i], &ref_vec);
        min_cosine = min_cosine.min(c);
        let preview: String = text.chars().take(38).collect();
        eprintln!("[{i:>2}] cosine = {c:.6}  ({preview})");
    }
    eprintln!("min reference cosine = {min_cosine:.6}");
    eprintln!("=== end parity ===\n");
    assert!(
        min_cosine > 0.999,
        "hand-rolled static pooling must match the model2vec reference on EVERY fixture text (incl. OOV/CJK/emoji/URL): min cosine {min_cosine} <= 0.999"
    );

    // SYMMETRY: query and document encodings match (static has no instruction asymmetry).
    let as_query = provider.embed(&texts, EmbeddingRole::Query).await.expect("query embed");
    assert_eq!(as_query, ours, "static is symmetric: query == document encoding");
}
