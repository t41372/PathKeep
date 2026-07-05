//! Env-gated real-machine tests for the in-app QUANTIZED candle Qwen3-Embedding engine (W-AI-4b).
//!
//! These link `vault-core` as a normal dependency, so they exercise the REAL candle forward pass +
//! GGUF model load + hf-hub download path (NOT the `cfg(test)` stub). They are gated on env vars so
//! they NEVER run in the coverage gate / CI:
//!
//! - `PATHKEEP_CANDLE_S1=1` → S1 throughput benchmark (docs/sec single + batched, plus the
//!   extrapolated 14.4M first-fill wall clock). Runs for BOTH the shipped Q8_0 default AND a Q4_K_M
//!   comparison when its files are sideloaded (see the quant fixtures below) so the artifact carries
//!   a quant comparison table.
//! - `PATHKEEP_CANDLE_PARITY=1` → candle-vs-LM-Studio cosine parity (the candle correctness proof):
//!   the SAME text embedded by the in-app candle engine and by LM Studio's
//!   `text-embedding-qwen3-embedding-0.6b` must have cosine ≈ 1.0 — for BOTH the document role and
//!   the QUERY role (the query role exercises the S2 instruction template against a reference).
//!
//! Both reuse a persistent model directory under `PATHKEEP_CANDLE_MODELS_DIR` (default
//! `<repo>/artifacts/candle-models`) so the quantized weights download once and are reused.
//!
//! ```sh
//! PATHKEEP_CANDLE_S1=1 cargo test --manifest-path src-tauri/Cargo.toml \
//!   -p vault-core --test candle_embedding_e2e --release -- --nocapture
//! PATHKEEP_CANDLE_PARITY=1 cargo test --manifest-path src-tauri/Cargo.toml \
//!   -p vault-core --test candle_embedding_e2e --release -- --nocapture
//! ```
//!
//! Compiled out under the `coverage` cfg: these drive the REAL candle load/download path
//! (`load_default`/`ensure_model_downloaded`), which is `cfg(not(any(test, coverage)))`, so the
//! coverage build does not expose it. The coverage gate exercises the in-crate stub instead.
#![cfg(not(coverage))]

use std::path::PathBuf;
use std::time::Instant;

use vault_core::{
    AiProviderConfig, AiProviderPurpose, AiProviderRuntime, AiRequestFormat,
    CandleEmbeddingProvider, DEFAULT_CANDLE_MODEL_FILES, DEFAULT_CANDLE_MODEL_REPO,
    DEFAULT_CANDLE_QUANT, EmbeddingProvider, EmbeddingRole, ExternalEmbeddingProvider,
    NoopDownloadProgress, QWEN3_QUERY_TASK, SecretString, apply_role_instruction,
    ensure_model_downloaded,
};

/// Resolves a persistent models root so the weights download once across runs.
fn models_root() -> PathBuf {
    if let Ok(dir) = std::env::var("PATHKEEP_CANDLE_MODELS_DIR") {
        return PathBuf::from(dir);
    }
    // Default: <repo>/artifacts/candle-models. The integration test runs with CWD = src-tauri.
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../artifacts/candle-models")
}

/// Builds a `ProjectPaths`-shaped root whose `models_dir` is the persistent cache.
fn project_paths() -> vault_core::ProjectPaths {
    // `models_dir = <root>/models`, so point root at the parent of our cache's `models` dir.
    let root = models_root();
    std::fs::create_dir_all(root.join("models")).expect("create models dir");
    vault_core::project_paths_with_root(&root)
}

/// Downloads (with consent) + loads the real Q8_0 candle engine.
fn load_candle_default() -> CandleEmbeddingProvider {
    let paths = project_paths();
    let mut progress = NoopDownloadProgress;
    ensure_model_downloaded(
        &paths,
        DEFAULT_CANDLE_MODEL_REPO,
        DEFAULT_CANDLE_MODEL_FILES,
        true,
        &mut progress,
    )
    .expect("download + verify Qwen3-Embedding-0.6B-GGUF (Q8_0)");
    // `false`: the parity/throughput e2e measures the CPU baseline (S1); the Metal GPU opt-in
    // (W-AI-9-D) is a separate build concern and does not change the vectors this parity test asserts.
    CandleEmbeddingProvider::load_default(&paths, false).expect("load candle Q8_0 engine")
}

/// Loads a sideloaded quant for the comparison table (skips if its GGUF is not present).
///
/// Looks for `<models_dir>/quant-<quant>/{<gguf>, tokenizer.json, config.json}` where the GGUF +
/// tokenizer + config were placed by the benchmark harness. Returns `None` when the files are
/// absent so the default-only run still works. The repo arg names a synthetic per-quant directory.
fn try_load_sideloaded_quant(quant: &str, gguf: &'static str) -> Option<CandleEmbeddingProvider> {
    let paths = project_paths();
    let repo = format!("quant-{quant}");
    let dir = vault_core::model_dir_for_repo(&paths, &repo);
    if !dir.join(gguf).exists() {
        return None;
    }
    // The loader requires the verified marker (S5); the harness placed the files, so mark it loadable.
    std::fs::write(dir.join(".pathkeep-verified"), b"ok").ok()?;
    CandleEmbeddingProvider::load(
        &paths,
        &repo,
        leak_quant_manifest(gguf),
        quant,
        QWEN3_QUERY_TASK,
        false,
    )
    .ok()
}

/// Builds a leaked 'static manifest for a sideloaded quant (digests unused by the loadable check).
fn leak_quant_manifest(gguf: &'static str) -> &'static [vault_core::ModelFile] {
    let manifest = vec![
        vault_core::ModelFile { name: "config.json", sha256: "x", repo: "local" },
        vault_core::ModelFile { name: "tokenizer.json", sha256: "x", repo: "local" },
        vault_core::ModelFile { name: gguf, sha256: "x", repo: "local" },
    ];
    Box::leak(manifest.into_boxed_slice())
}

fn cosine(a: &[f32], b: &[f32]) -> f32 {
    assert_eq!(a.len(), b.len(), "vectors must share a dimension");
    let dot: f32 = a.iter().zip(b).map(|(x, y)| x * y).sum();
    let na: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let nb: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    dot / (na * nb)
}

fn lmstudio_runtime() -> AiProviderRuntime {
    AiProviderRuntime {
        config: AiProviderConfig {
            id: "lmstudio-embed-parity".to_string(),
            name: "LM Studio Embedding (parity)".to_string(),
            purpose: AiProviderPurpose::Embedding,
            request_format: AiRequestFormat::LmStudio,
            enabled: true,
            base_url: Some("http://localhost:1234/v1".to_string()),
            default_model: "text-embedding-qwen3-embedding-0.6b".to_string(),
            dimensions: None,
            ..AiProviderConfig::default()
        },
        api_key: Some(SecretString::from("lm-studio".to_string())),
    }
}

/// Measures single + batched throughput for one already-loaded provider and prints the row.
async fn bench_throughput(label: &str, provider: &CandleEmbeddingProvider) {
    let corpus: Vec<String> = (0..64)
        .map(|i| {
            format!(
                "Profile: default\nVisited at: 2026-06-{:02}T10:00:00Z\nURL: https://example.com/page-{i}\nDomain: example.com\nTitle: Example article number {i} about rust embeddings and history search",
                (i % 28) + 1
            )
        })
        .collect();

    // Warm up (first forward pays one-time allocation/JIT-ish costs).
    let _ = provider.embed(&corpus[..1], EmbeddingRole::Document).await.expect("warmup");

    let single_count = 16usize;
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
    let eta_14m_hours = 14_400_000.0 / batch_dps / 3600.0;
    eprintln!(
        "| {label:<8} | dim {dim} | single {single_dps:.2} d/s | batched {batch_dps:.2} d/s | 14.4M ETA {eta_14m_hours:.1} h |"
    );
    assert!(batch_dps > 0.0, "throughput must be positive");
}

#[tokio::test]
async fn candle_s1_throughput_benchmark() {
    if std::env::var("PATHKEEP_CANDLE_S1").as_deref() != Ok("1") {
        eprintln!(
            "skipping candle S1: set PATHKEEP_CANDLE_S1=1 (downloads ~640 MB Q8_0 on first run)"
        );
        return;
    }
    eprintln!("\n=== W-AI-4b S1 throughput (candle QUANTIZED Qwen3-Embedding-0.6B, CPU) ===");
    eprintln!("| quant    | dim  | single d/s | batched d/s | 14.4M ETA |");
    // Q4_K_M comparison (sideloaded under <models>/quant-Q4_K_M/) when present.
    if let Some(q4) = try_load_sideloaded_quant("Q4_K_M", "Qwen3-Embedding-0.6B-q4_k_m.gguf") {
        bench_throughput("Q4_K_M", &q4).await;
    } else {
        eprintln!(
            "| Q4_K_M   | (sideload <models>/quant-Q4_K_M/Qwen3-Embedding-0.6B-q4_k_m.gguf to benchmark) |"
        );
    }
    let q8 = load_candle_default();
    bench_throughput(DEFAULT_CANDLE_QUANT, &q8).await;
    eprintln!("=== end S1 ===\n");
}

#[tokio::test]
async fn candle_vs_lmstudio_cosine_parity() {
    if std::env::var("PATHKEEP_CANDLE_PARITY").as_deref() != Ok("1") {
        eprintln!(
            "skipping candle parity: set PATHKEEP_CANDLE_PARITY=1 with LM Studio on :1234 (qwen3-0.6b embedding)"
        );
        return;
    }
    let candle = load_candle_default();
    let lmstudio = ExternalEmbeddingProvider::new(lmstudio_runtime()).expect("lm studio provider");

    let texts = vec![
        "the quick brown fox jumps over the lazy dog".to_string(),
        "a treatise on the macroeconomics of central banking".to_string(),
        "rust programming language memory safety without garbage collection".to_string(),
    ];

    // Document role (no instruction) on both engines — same model, so cosine should be ~1.0.
    let candle_docs = candle.embed(&texts, EmbeddingRole::Document).await.expect("candle docs");
    let lm_docs = lmstudio.embed(&texts, EmbeddingRole::Document).await.expect("lm studio docs");

    // QUERY-role parity, validated against a REFERENCE (S2). LM Studio's `/v1/embeddings` does NOT
    // auto-apply the query instruction — it embeds whatever string it is given (proven during
    // W-AI-4b: raw-query vs instruction-formatted differ by cosine ~0.73). So the correct reference
    // for candle's query embedding is LM Studio embedding the SAME explicitly-formatted instruction
    // string `Instruct: {task}\nQuery:{text}` (NO space — S2) as a DOCUMENT. If candle applied the
    // wrong template (a stray space, or no instruction) this cosine would drop below the gate, so
    // this validates the corrected S2 template against an external reference, not against itself.
    let candle_queries = candle.embed(&texts, EmbeddingRole::Query).await.expect("candle queries");
    let formatted: Vec<String> = texts
        .iter()
        .map(|text| apply_role_instruction(text, EmbeddingRole::Query, QWEN3_QUERY_TASK))
        .collect();
    let lm_query_ref =
        lmstudio.embed(&formatted, EmbeddingRole::Document).await.expect("lm studio query ref");

    eprintln!("\n=== W-AI-4b parity (candle Q8_0 vs LM Studio) ===");
    eprintln!("candle dim = {}, lm studio dim = {}", candle_docs[0].len(), lm_docs[0].len());

    let mut min_doc = f32::INFINITY;
    let mut min_query = f32::INFINITY;
    for (i, text) in texts.iter().enumerate() {
        let dc = cosine(&candle_docs[i], &lm_docs[i]);
        let qc = cosine(&candle_queries[i], &lm_query_ref[i]);
        min_doc = min_doc.min(dc);
        min_query = min_query.min(qc);
        eprintln!(
            "[{i}] doc cosine = {dc:.6}  query(role) cosine = {qc:.6}  ({})",
            &text[..text.len().min(40)]
        );
    }
    eprintln!("min document cosine   = {min_doc:.6}");
    eprintln!(
        "min query-role cosine = {min_query:.6}  (candle query vs LM Studio doc-embed of the formatted instruction string)"
    );
    eprintln!("=== end parity ===\n");

    assert_eq!(candle_docs[0].len(), lm_docs[0].len(), "same model → same dim");
    assert!(
        min_doc > 0.99,
        "candle document parity must match LM Studio: min cosine {min_doc} <= 0.99"
    );
    assert!(
        min_query > 0.99,
        "candle QUERY parity must match LM Studio reference (S2 instruction template): min cosine {min_query} <= 0.99"
    );

    // Q4_K_M document parity (sideloaded), REPORTED ONLY for the quant comparison table — NOT a
    // shipped-default gate. Q4_K_M is the lower-fidelity comparison point, not the default: the
    // W-AI-4b measurement found its parity ≈ 0.983 (< the 0.99 gate), which is exactly WHY the
    // default ships Q8_0 (parity ≈ 0.9995). We print it so the artifact's table is grounded; we do
    // NOT assert > 0.99 here because Q4_K_M is not the shipped quant and failing on it would block a
    // benchmark-only sideload. The default Q8_0's parity IS asserted above.
    if let Some(q4) = try_load_sideloaded_quant("Q4_K_M", "Qwen3-Embedding-0.6B-q4_k_m.gguf") {
        let q4_docs = q4.embed(&texts, EmbeddingRole::Document).await.expect("q4 docs");
        let mut min_q4 = f32::INFINITY;
        for i in 0..texts.len() {
            min_q4 = min_q4.min(cosine(&q4_docs[i], &lm_docs[i]));
        }
        eprintln!(
            "Q4_K_M min document cosine = {min_q4:.6}  (comparison only; below the 0.99 gate → NOT the default)"
        );
    } else {
        eprintln!("Q4_K_M parity skipped (sideload <models>/quant-Q4_K_M/ to measure)");
    }
}
