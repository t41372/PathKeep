//! Env-gated real-machine tests for the in-app HAND-ROLLED static (model2vec) engine (W-AI-4c).
//!
//! These link `vault-core` as a normal dependency, so they exercise the REAL safetensors matrix
//! load, tokenizer, and download path (NOT the `cfg(test)` stub). They are gated on env vars
//! so they NEVER run in the coverage gate / CI:
//!
//! - `PATHKEEP_STATIC_S1=1` → S1 throughput benchmark (docs/sec single + batched, plus the
//!   extrapolated 14.4M first-fill wall clock). The static tier is the WHOLE point of W-AI-4c: it
//!   must be orders of magnitude faster than a transformer forward pass (~1.25 d/s measured for a
//!   quantized Qwen3 on the target), so the 14.4M first fill is minutes, not days.
//! - `PATHKEEP_STATIC_PARITY=1` → hand-roll-vs-reference cosine parity (THE static correctness
//!   proof): the SAME texts embedded by our hand-rolled engine and by the Python model2vec reference
//!   must have cosine >= 0.999999. The reference vectors are PINNED in a committed fixture
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
//! `<repo>/src-tauri/artifacts/static-models`) so the matrix downloads once and is reused.
//! `PATHKEEP_STATIC_MEMORY=1` runs the macOS physical-footprint benchmark in fresh child processes
//! for the engine and tokenizer separately. `PATHKEEP_STATIC_MEMORY_LABEL` names its JSON artifact
//! under that same models root (default `latest`). Run only `static_memory_footprint_benchmark`
//! when measuring, so other tests cannot warm the engine cache or allocator in the child.
//! Model preparation warms the filesystem cache; each child starts with an empty engine cache.
//! To measure optimized loader/pooling code while reusing existing dependency builds, add
//! `--config 'profile.test.package.vault-core.opt-level=3'`. This is not a full release build.
//!
//! ```sh
//! PATHKEEP_STATIC_S1=1 cargo test --manifest-path src-tauri/Cargo.toml \
//!   -p vault-core --test static_embedding_e2e --release -- --nocapture
//! PATHKEEP_STATIC_PARITY=1 \
//!   cargo test --manifest-path src-tauri/Cargo.toml \
//!   -p vault-core --test static_embedding_e2e --release -- --nocapture
//! PATHKEEP_STATIC_MEMORY=1 PATHKEEP_STATIC_MEMORY_LABEL=after \
//!   cargo test --manifest-path src-tauri/Cargo.toml \
//!   --config 'profile.test.package.vault-core.opt-level=3' \
//!   -p vault-core --test static_embedding_e2e static_memory_footprint_benchmark \
//!   -- --exact --nocapture --test-threads=1
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
                "Example article number {i} about rust embeddings and history search\nexample.com · page"
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
    eprintln!("(a quantized transformer did ~1.25 d/s → 14.4M ≈ days; static must be far faster)");
    eprintln!("=== end S1 ===\n");
    assert!(
        batch_dps > 100.0,
        "static throughput must be far faster than a transformer (>100 d/s)"
    );
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
        eprintln!("[{i:>2}] cosine = {c:.9}  ({preview})");
    }
    eprintln!("min reference cosine = {min_cosine:.9}");
    eprintln!("=== end parity ===\n");
    assert!(
        min_cosine >= 0.999999,
        "static pooling must match the model2vec reference on every fixture text: min cosine {min_cosine} < 0.999999"
    );

    // SYMMETRY: query and document encodings match (static has no instruction asymmetry).
    let as_query = provider.embed(&texts, EmbeddingRole::Query).await.expect("query embed");
    assert_eq!(as_query, ours, "static is symmetric: query == document encoding");
}

/// Physical footprint includes anonymous and compressed memory charged to this process. File-backed
/// clean pages are reclaimable and are not charged like a heap copy of the matrix. This is not RSS.
#[cfg(target_os = "macos")]
fn physical_footprint() -> u64 {
    let mut usage = std::mem::MaybeUninit::<libc::rusage_info_v0>::uninit();
    // SAFETY: the buffer has the size and alignment required by RUSAGE_INFO_V0. The kernel writes
    // it synchronously; we only read the initialized result after a successful call.
    let result = unsafe {
        libc::proc_pid_rusage(
            std::process::id() as libc::pid_t,
            libc::RUSAGE_INFO_V0,
            usage.as_mut_ptr().cast(),
        )
    };
    assert_eq!(result, 0, "proc_pid_rusage failed: {}", std::io::Error::last_os_error());
    // SAFETY: proc_pid_rusage succeeded and initialized the RUSAGE_INFO_V0 buffer.
    unsafe { usage.assume_init() }.ri_phys_footprint
}

/// A bounded batch of page text in the backfill format, with varied topics and languages. Each
/// body is a few hundred characters; numeric page IDs make the 10,000 inputs distinct.
#[cfg(target_os = "macos")]
fn memory_benchmark_pages(start: usize, end: usize) -> Vec<String> {
    const PAGES: &[(&str, &str, &str, &str)] = &[
        (
            "Rust database transactions",
            "docs.rs",
            "storage transaction recovery",
            "A local database keeps committed records after a crash. This article explains write ahead logs, durable checkpoints, concurrent readers, encrypted storage and recovery. The examples compare a sequential scan with an indexed lookup and show how to keep a desktop application responsive during a large import.",
        ),
        (
            "Walking through Kyoto",
            "travel.example",
            "japan kyoto walking gardens",
            "The route starts near the railway station and follows a quiet river toward the temple gardens. There are small cafes, covered markets and places to rest along the way. This guide includes public transport connections, rainy day alternatives, seasonal flowers and practical notes for a relaxed afternoon walk.",
        ),
        (
            "家庭麵包製作筆記",
            "cooking.example",
            "recipes bread sourdough",
            "這篇筆記記錄了家庭製作酸種麵包的流程，從培養酵母、調整含水量，到揉麵、摺疊與低溫發酵。麵粉種類、室內溫度與等待時間都會影響口感。烘烤前先預熱鑄鐵鍋，入爐時保留蒸氣，最後開蓋讓表皮上色。文章也整理了常見問題：麵團太黏、發酵不足、切口不展開，以及如何保存隔夜麵包。每次只調整一項條件，記下結果，下一次才能知道哪個改變有效。",
        ),
        (
            "Solar panels and home energy",
            "energy.example",
            "guides solar battery efficiency",
            "A home energy system balances generation, battery storage and daily electricity demand. Roof direction and shade affect the output across the seasons. The guide compares installation costs, inverter efficiency, maintenance, household consumption and ways to measure savings from actual meter readings rather than estimates.",
        ),
        (
            "Understanding musical harmony",
            "music.example",
            "lessons harmony chords piano",
            "These piano exercises connect scales, chord progressions and voice leading. Listen for the tension before a cadence and practise moving each voice by a small interval. The lesson covers major and minor keys, rhythm, ear training and arranging a melody with simple accompaniment, with short examples for daily practice.",
        ),
        (
            "星空攝影入門",
            "photo.example",
            "photography night sky exposure",
            "拍攝星空需要選擇光害較少的地點，先確認天氣、月相和交通路線。使用穩固腳架與手動對焦，從短曝光開始，逐步調整感光度與光圈。這份指南比較廣角鏡頭、赤道儀、降噪和多張疊合的方法，也提醒拍攝者保留原始檔案與現場筆記。回家後檢查星點是否拖線，觀察暗部細節，按實際結果調整下次的設定。後製時保持自然的色彩，避免過度銳化，讓照片呈現當晚真正看到的天空。",
        ),
    ];
    (start..end)
        .map(|index| {
            let (title, site, path, body) = PAGES[index % PAGES.len()];
            format!("{title} {index}\n{site} · {path}\n{body}")
        })
        .collect()
}

/// Separate processes prevent tokenizer-only allocations and retained cached engines from
/// contaminating each other's measurements. No timing threshold: this records a real workload.
#[cfg(target_os = "macos")]
#[tokio::test]
async fn static_memory_footprint_benchmark() {
    if std::env::var("PATHKEEP_STATIC_MEMORY").as_deref() != Ok("1") {
        eprintln!("skipping static memory: set PATHKEEP_STATIC_MEMORY=1 on macOS");
        return;
    }
    if let Ok(phase) = std::env::var("PATHKEEP_STATIC_MEMORY_CHILD") {
        let paths = project_paths();
        let baseline = physical_footprint();
        let started = Instant::now();
        let report = if phase == "tokenizer" {
            let directory = vault_core::model_dir_for_repo(&paths, DEFAULT_STATIC_MODEL_REPO);
            let bytes = std::fs::read(directory.join("tokenizer.json")).expect("read tokenizer");
            let tokenizer =
                vault_core::ai::StaticTokenizer::from_bytes(&bytes).expect("load tokenizer");
            drop(bytes);
            let load_ms = started.elapsed().as_secs_f64() * 1000.0;
            let footprint = physical_footprint();
            std::hint::black_box(&tokenizer);
            serde_json::json!({"baseline_bytes": baseline, "loaded_bytes": footprint,
                "load_ms": load_ms})
        } else {
            assert_eq!(phase, "engine");
            // Includes the production cache-miss SHA-256 verification, but excludes download setup.
            let provider = StaticEmbeddingProvider::load_default(&paths).expect("load engine");
            let load_ms = started.elapsed().as_secs_f64() * 1000.0;
            let loaded = physical_footprint();
            let query = ["find articles about local first encrypted browser history".to_string()];
            let vector = provider.embed(&query, EmbeddingRole::Query).await.expect("query");
            std::hint::black_box(&vector);
            drop(vector);
            let queried = physical_footprint();
            // Match the backfill's bounded 64-text batches; retain neither all texts nor vectors.
            let started = Instant::now();
            for start in (0..10_000).step_by(64) {
                let texts = memory_benchmark_pages(start, (start + 64).min(10_000));
                let vectors = provider.embed(&texts, EmbeddingRole::Document).await.expect("pages");
                assert_eq!(vectors.len(), texts.len());
                std::hint::black_box(vectors);
            }
            let seconds = started.elapsed().as_secs_f64();
            let built = physical_footprint();
            serde_json::json!({"baseline_bytes": baseline, "loaded_bytes": loaded,
                "after_query_bytes": queried, "after_10000_bytes": built,
                "load_ms": load_ms, "texts": 10_000, "batch_size": 64,
                "build_seconds": seconds, "texts_per_second": 10_000.0 / seconds})
        };
        println!("STATIC_MEMORY_REPORT={report}");
        return;
    }

    let paths = project_paths();
    ensure_model_downloaded(
        &paths,
        DEFAULT_STATIC_MODEL_REPO,
        DEFAULT_STATIC_MODEL_FILES,
        true,
        &mut NoopDownloadProgress,
    )
    .expect("prepare model once before measurement");
    let mut report = serde_json::Map::new();
    for phase in ["tokenizer", "engine"] {
        let output = std::process::Command::new(std::env::current_exe().expect("test executable"))
            .args([
                "--exact",
                "static_memory_footprint_benchmark",
                "--nocapture",
                "--test-threads=1",
            ])
            .env("PATHKEEP_STATIC_MEMORY_CHILD", phase)
            .output()
            .expect("run isolated measurement");
        assert!(
            output.status.success(),
            "measurement failed: {}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        let stdout = String::from_utf8(output.stdout).expect("measurement UTF-8");
        let json = stdout
            .lines()
            .find_map(|line| line.split_once("STATIC_MEMORY_REPORT=").map(|(_, json)| json))
            .expect("child measurement report");
        report.insert(phase.into(), serde_json::from_str(json).expect("measurement JSON"));
    }
    let label = std::env::var("PATHKEEP_STATIC_MEMORY_LABEL").unwrap_or_else(|_| "latest".into());
    assert!(label.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'));
    let artifact = models_root().join(format!("memory-{label}.json"));
    let json = serde_json::to_string_pretty(&report).expect("serialize measurement");
    std::fs::write(&artifact, &json).expect("write measurement artifact");
    eprintln!("{json}\nMeasurement artifact: {}", artifact.display());
}
