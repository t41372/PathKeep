//! In-app candle Qwen3-Embedding inference engine (QUANTIZED GGUF) + consent-gated download (W-AI-4b).
//!
//! ## Responsibilities
//! - own the ONE place candle/tokenizers touch a forward pass: load a QUANTIZED decoder embedding
//!   model (Qwen3-Embedding family by default) from a GGUF checkpoint in
//!   [`crate::config::ProjectPaths::models_dir`] and turn text into vectors with the model's real
//!   pooling + normalization (D2/§C.1)
//! - implement the rig-free [`EmbeddingProvider`] boundary against that engine, honoring the
//!   02 §C.3 correctness 鐵律: read the ACTUAL hidden size as `effective_dim`, defensively
//!   L2-normalize, and thread [`EmbeddingRole`] through as a REAL instruction prefix (this is where
//!   the role stops being a no-op — 4a wired it; 4b makes it apply)
//! - download the model on demand via `hf-hub`, but ONLY behind explicit consent, with SHA-256
//!   verification of every file, offline-first reuse, off-thread execution, cancellation, and
//!   progress (§C.5)
//! - register the `Candle` arm of [`super::embedding_external::AnyEmbeddingProvider`] so the
//!   index/search loops dispatch to the in-app engine without a single call-site change (W-AI-0 §8)
//!
//! ## Not responsible for
//! - provider config/secret resolution (the worker's `AiProviderRuntime` carries those)
//! - vector persistence, the embed loop, or queue bookkeeping (those stay engine-agnostic in
//!   `vector_store`/`indexing`)
//! - chat/LLM inference (D1: chat is external-only; candle does embedding + optional rerank)
//!
//! ## Why this module exists & the quantized model
//! D2/D4 say the in-app engine is candle and PathKeep makes zero model assumptions. Qwen3-Embedding
//! is normally run QUANTIZED (that is how LM Studio serves it); the prior F32 path materialized
//! ~2.4 GB of weights in RAM and was impractical on the 4-core/8 GB target. This engine therefore
//! loads a **GGUF quantized checkpoint** (default Q8_0) into candle's quantized tensors.
//!
//! candle-transformers ships `quantized_qwen3::ModelWeights`, but its public `forward` returns
//! LOGITS (it applies the `lm_head` after narrowing to the last token) and its layer/embedding
//! fields are private — so it CANNOT yield the last-token HIDDEN STATE an embedding model needs.
//! We therefore build [`QuantizedQwen3Embedding`] from candle's PUBLIC quantized building blocks
//! (`gguf_file::Content`, `QMatMul::from_weights`, quantized `RmsNorm::from_qtensor`, the
//! `ConcatKvCache`, RoPE, and `repeat_kv`) — a faithful Qwen3 decoder that stops at the final
//! RMSNorm and returns hidden states. Because the KV cache is resettable in-place
//! (`clear_kv_cache`), one model instance is reused across texts (NO per-text rebuild — the F32
//! path could not do this because the base model's `clear_kv_cache` is private).
//!
//! The real candle forward pass + GGUF load + hf-hub download are `#[cfg(not(any(test, coverage)))]`;
//! a deterministic same-call-graph stub backs `#[cfg(any(test, coverage))]` so the 100% coverage
//! gate is met without weights or a network. The PURE helpers (instruction templating, last-token
//! pooling tensor math, config parsing, the model-file manifest, SHA-256 verification, the engine
//! selector + download decision) are un-gated and unit-tested so a tokenization/pooling/manifest
//! /selection regression is caught by the unit + mutation gates rather than shipping silently. The
//! S1 throughput benchmark and the candle-vs-LM-Studio parity correctness gate live in env-gated
//! integration tests (never CI).

use super::embedding_external::{AnyEmbeddingProvider, ExternalEmbeddingProvider};
use super::provider::l2_normalize;
use super::traits::{
    EmbeddingDescriptor, EmbeddingDtype, EmbeddingPooling, EmbeddingProvider, EmbeddingRole,
};
use crate::AiProviderRuntime;
use crate::config::ProjectPaths;
use anyhow::{Context, Result};

#[cfg(not(any(test, coverage)))]
use candle_core::{Device, Tensor};
#[cfg(not(any(test, coverage)))]
use std::sync::Mutex;

// ---------------------------------------------------------------------------
// Model-file manifest (always compiled — pure data + verification helpers).
// ---------------------------------------------------------------------------

/// The default in-app embedding model's Hugging Face repository id (the GGUF weights repo).
///
/// This is the NO-CONFIG convenience default (D3): the OFFICIAL `Qwen/Qwen3-Embedding-0.6B-GGUF`
/// repo, which ships the quantized weights LM Studio also serves. It drives a download path, NOT any
/// dim/pooling assumption (those are read at runtime, D4). A future swap (e.g. a Qwen4 embedding
/// model) changes this one constant plus the pinned hashes below; nothing else in the engine assumes
/// this specific model.
pub const DEFAULT_CANDLE_MODEL_REPO: &str = "Qwen/Qwen3-Embedding-0.6B-GGUF";

/// The Hugging Face repo that carries the `tokenizer.json` + `config.json` for the model.
///
/// The GGUF weights repo (above) ships ONLY `.gguf` files — no `tokenizer.json`. The base
/// (non-GGUF) `Qwen/Qwen3-Embedding-0.6B` repo carries the tokenizer + config we need alongside the
/// quantized weights, so the manifest fetches each file from its own repo (see [`ModelFile::repo`]).
pub const DEFAULT_CANDLE_TOKENIZER_REPO: &str = "Qwen/Qwen3-Embedding-0.6B";

/// The default GGUF quantization level the engine ships with.
///
/// Q8_0 is the safe high-fidelity default: it is published in the OFFICIAL Qwen GGUF repo and its
/// parity cosine vs LM Studio stays > 0.99 (see the S1 artifact). The quant is recorded in the
/// engine's `model_id` so the embedding fingerprint changes when the quant changes — switching quant
/// levels re-embeds the index rather than silently mixing encodings.
pub const DEFAULT_CANDLE_QUANT: &str = "Q8_0";

/// One required file in the model bundle, with the exact SHA-256 it must hash to and its source repo.
///
/// Pinning the digest (not just the name) is the §C.5 integrity contract: a downloaded or
/// sideloaded file that does not match is rejected before it is ever loaded into the engine, so a
/// corrupted or substituted weight file fails loudly rather than producing silent garbage vectors.
/// `repo` lets one manifest pull files from two HF repos (GGUF weights vs base tokenizer/config).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModelFile {
    /// File name within `models_dir/<repo-dir>/` (the local layout flattens everything into one dir).
    pub name: &'static str,
    /// Lowercase hex SHA-256 the file must match.
    pub sha256: &'static str,
    /// Hugging Face repo the file is fetched from.
    pub repo: &'static str,
}

/// The files the candle engine needs, with their pinned SHA-256 digests.
///
/// Digests are the verified hashes of the real artifacts on the Hugging Face `main` revision
/// (computed from the downloaded files during W-AI-4b — see `artifacts/benchmarks`). The forward
/// pass needs `config.json` (architecture sanity-check), `tokenizer.json` (BPE), and the
/// `*-Q8_0.gguf` quantized weights. The tokenizer + config come from the BASE repo; the GGUF comes
/// from the GGUF repo.
///
/// SHA-256 BOUNDARY NOTE (F2): these are NOT placeholders. Each digest is the real, recomputed hash
/// of the file downloaded on 2026-06-21:
/// - `config.json`     = `b5bf1f51…` (base repo `Qwen/Qwen3-Embedding-0.6B`)
/// - `tokenizer.json`  = `def76fb0…` (base repo)
/// - Q8_0 GGUF         = `06507c7b…` (official `Qwen/Qwen3-Embedding-0.6B-GGUF`)
///
/// A fat-fingered digest is caught by the e2e download (`PATHKEEP_CANDLE_S1`/`PARITY`) failing
/// SHA-256 verification, and the manifest unit test pins these exact values, not just hex-shape.
pub const DEFAULT_CANDLE_MODEL_FILES: &[ModelFile] = &[
    ModelFile {
        name: "config.json",
        sha256: "b5bf1f51fc45be473a54718cef92448d90a1be001bf9b9a44b8c7f10a19feaa9",
        repo: DEFAULT_CANDLE_TOKENIZER_REPO,
    },
    ModelFile {
        name: "tokenizer.json",
        sha256: "def76fb086971c7867b829c23a26261e38d9d74e02139253b38aeb9df8b4b50a",
        repo: DEFAULT_CANDLE_TOKENIZER_REPO,
    },
    ModelFile {
        name: "Qwen3-Embedding-0.6B-Q8_0.gguf",
        sha256: "06507c7b42688469c4e7298b0a1e16deff06caf291cf0a5b278c308249c3e439",
        repo: DEFAULT_CANDLE_MODEL_REPO,
    },
];

/// The manifest's GGUF weight file name (the one `*.gguf` entry the loader feeds to candle).
///
/// Pure selector over [`DEFAULT_CANDLE_MODEL_FILES`] so the loader never hardcodes the file name in
/// two places (the manifest is the single source of truth) and a missing/duplicate GGUF entry is a
/// loud error rather than a silent wrong-file load.
pub fn gguf_file_name(files: &[ModelFile]) -> Result<&str> {
    let mut ggufs = files.iter().filter(|file| file.name.ends_with(".gguf"));
    let first = ggufs.next().context("model manifest has no .gguf weight file")?;
    if ggufs.next().is_some() {
        anyhow::bail!(
            "model manifest has more than one .gguf weight file; the loader needs exactly one"
        );
    }
    Ok(first.name)
}

/// Reserved `base_url` sentinel that marks an embedding provider as the in-app candle engine.
///
/// Selecting candle vs the external `/v1/embeddings` path is config-driven WITHOUT a new request
/// format (candle is not a wire shape) and WITHOUT a schema change: a provider whose `base_url` is
/// this sentinel routes to the in-app engine; any other base URL routes to the external adapter.
/// `default_model` still carries the model id (D4: never hardcoded into the selector), so a future
/// in-app model is a config change, not a code change.
pub const CANDLE_INAPP_BASE_URL: &str = "candle:in-app";

/// Resolves the on-disk directory for one model repo under `models_dir`.
///
/// One directory per repo id, with the repo's `org/name` flattened to a single safe segment so a
/// slash in the id never escapes `models_dir`. Pure (path math only) so it is unit-tested.
pub fn model_dir_for_repo(paths: &ProjectPaths, repo: &str) -> std::path::PathBuf {
    paths.models_dir.join(crate::utils::filesystem_safe_path_segment(repo))
}

/// Whether a resolved provider runtime is configured to use the in-app candle engine.
///
/// Pure (reads only the config) so the routing decision is unit-tested without loading a model.
pub fn runtime_uses_candle(runtime: &AiProviderRuntime) -> bool {
    runtime.config.base_url.as_deref() == Some(CANDLE_INAPP_BASE_URL)
}

/// Resolves the GGUF weights repo id for a candle runtime (config `default_model`, else the default).
///
/// Pure so the model-id resolution is unit-tested. An empty `default_model` falls back to
/// [`DEFAULT_CANDLE_MODEL_REPO`] (D4: the default is a constant, never a baked-in selector
/// assumption); a configured value wins so a future in-app model is a config change.
pub fn candle_repo_for_runtime(runtime: &AiProviderRuntime) -> &str {
    if runtime.config.default_model.is_empty() {
        DEFAULT_CANDLE_MODEL_REPO
    } else {
        runtime.config.default_model.as_str()
    }
}

/// Selects the embedding engine for a resolved provider runtime (config-driven, W-AI-4b).
///
/// Routing + DEGRADATION contract (S3 — never hard-fail a backfill because the model is absent, per
/// AGENTS.md principle 4 "intelligence is optional / degrade not break"):
/// - A runtime marked for the in-app engine ([`runtime_uses_candle`]) selects candle ONLY when the
///   model is present + verified on disk ([`model_is_present_and_verified`] = the consent gate, since
///   the model only lands there after a consented download).
/// - When candle is requested but the model is NOT present/verified, this DOES NOT error: it falls
///   back to the external `/v1/embeddings` adapter when the runtime also carries an external base URL
///   that supports it; only if no external fallback is configured does it return a clear
///   "model not downloaded" error (so the caller can surface a download prompt, not crash the job).
/// - Any other runtime returns the external adapter.
///
/// The model id used to load comes from the provider config (`default_model`) when set, falling back
/// to the default repo — never a hardcoded assumption baked into the selector.
#[cfg(not(any(test, coverage)))]
pub fn select_embedding_provider(
    paths: &ProjectPaths,
    runtime: &AiProviderRuntime,
) -> Result<AnyEmbeddingProvider> {
    if runtime_uses_candle(runtime) {
        let repo = candle_repo_for_runtime(runtime);
        let model_dir = model_dir_for_repo(paths, repo);
        if model_is_present_and_verified(&model_dir, DEFAULT_CANDLE_MODEL_FILES) {
            let provider = CandleEmbeddingProvider::load(
                paths,
                repo,
                DEFAULT_CANDLE_MODEL_FILES,
                DEFAULT_CANDLE_QUANT,
                QWEN3_QUERY_TASK,
            )?;
            return Ok(AnyEmbeddingProvider::Candle(Box::new(provider)));
        }
        // Model not present: degrade rather than abort the backfill (S3). Prefer an external
        // fallback if the runtime can serve one; otherwise surface a clear, actionable error.
        return degrade_candle_to_external(runtime, repo);
    }
    Ok(AnyEmbeddingProvider::External(ExternalEmbeddingProvider::new(runtime.clone())?))
}

/// Test/coverage selector: builds the deterministic candle stub or the external stub by config.
///
/// Same routing + degradation decision as the real selector (so the backfill exercises both arms,
/// AND the degrade-to-external / clear-error arms, at 100% coverage) but uses the no-weights candle
/// stub for the in-app path. The stub is treated as "present" so the candle arm is reachable; the
/// degrade arms are exercised via [`degrade_candle_to_external`] directly in unit tests.
#[cfg(any(test, coverage))]
pub fn select_embedding_provider(
    _paths: &ProjectPaths,
    runtime: &AiProviderRuntime,
) -> Result<AnyEmbeddingProvider> {
    if runtime_uses_candle(runtime) {
        let repo = candle_repo_for_runtime(runtime);
        Ok(AnyEmbeddingProvider::Candle(Box::new(CandleEmbeddingProvider::new_stub(
            &format!("candle:{repo}"),
            repo,
            DEFAULT_CANDLE_QUANT,
        ))))
    } else {
        Ok(AnyEmbeddingProvider::External(ExternalEmbeddingProvider::new(runtime.clone())?))
    }
}

/// Degrades a candle runtime whose model is absent to the external adapter, or a clear error (S3).
///
/// Pure decision logic (no model load, no network): if the runtime carries a usable external base
/// URL — i.e. NOT the candle sentinel and an OpenAI-compatible request format — it builds the
/// external provider so the backfill can proceed against `/v1/embeddings`. Otherwise it returns an
/// actionable "model not downloaded" error the caller turns into a download prompt. Either way the
/// backfill is NEVER hard-failed merely because the in-app model has not been fetched yet.
pub fn degrade_candle_to_external(
    runtime: &AiProviderRuntime,
    repo: &str,
) -> Result<AnyEmbeddingProvider> {
    let has_external_base =
        runtime.config.base_url.as_deref().is_some_and(|base| base != CANDLE_INAPP_BASE_URL);
    if has_external_base {
        return Ok(AnyEmbeddingProvider::External(ExternalEmbeddingProvider::new(
            runtime.clone(),
        )?));
    }
    anyhow::bail!(
        "in-app candle model {repo} is not downloaded; fetch it (with consent) before embedding, or configure an external embedding provider"
    )
}

/// Returns `true` only when every manifest file is present AND matches its pinned SHA-256.
///
/// This is the offline-first + integrity gate (§C.5): a model is "available" without any network
/// iff its files are already on disk and verified. A present-but-corrupt file makes the model
/// UNAVAILABLE (returns `false`), so the engine degrades rather than loading bad weights. Streams
/// each file through [`crate::utils::file_sha256_hex`] so a multi-hundred-MB weight file is never
/// read into memory whole.
///
/// This full re-hash is the AUTHORITATIVE check used at download time and by the selector's
/// availability gate. The per-load hot path uses the cheaper [`model_is_loadable`] presence/marker
/// check (S5) so the full SHA-256 is not recomputed on every engine load.
pub fn model_is_present_and_verified(model_dir: &std::path::Path, files: &[ModelFile]) -> bool {
    files.iter().all(|file| {
        let path = model_dir.join(file.name);
        match crate::utils::file_sha256_hex(&path) {
            Ok(digest) => digest == file.sha256,
            Err(_) => false,
        }
    })
}

/// Name of the marker file written next to the weights once they pass full SHA-256 verification.
///
/// Its presence means "every manifest file in this dir verified at download time"; the per-load
/// path trusts it (plus a cheap presence/size check) instead of re-hashing hundreds of MB on every
/// engine load (S5). Removing the weights (or the marker) forces a re-verify/re-download.
const VERIFIED_MARKER_FILE: &str = ".pathkeep-verified";

/// Records that a model directory passed full SHA-256 verification (writes the verified marker).
///
/// Called once after a download verifies every file, so subsequent loads can use the cheap
/// presence check instead of re-hashing the weights (S5).
fn write_verified_marker(model_dir: &std::path::Path) -> Result<()> {
    std::fs::write(model_dir.join(VERIFIED_MARKER_FILE), b"ok")
        .with_context(|| format!("writing verified marker in {}", model_dir.display()))
}

/// Cheap per-load readiness check: every manifest file is present + non-empty AND the dir is marked
/// verified (S5 — avoids re-hashing the weights on each load).
///
/// Pure apart from `stat`-ing each file + the marker. Used by the engine load path so a model that
/// was verified at download time loads without recomputing its SHA-256. The authoritative full
/// re-hash ([`model_is_present_and_verified`]) still runs at download time and in the selector's
/// availability gate, so a model is never LOADED unless it was verified at least once.
pub fn model_is_loadable(model_dir: &std::path::Path, files: &[ModelFile]) -> bool {
    if !model_dir.join(VERIFIED_MARKER_FILE).exists() {
        return false;
    }
    files.iter().all(|file| match std::fs::metadata(model_dir.join(file.name)) {
        Ok(meta) => meta.is_file() && meta.len() > 0,
        Err(_) => false,
    })
}

/// Verifies one already-downloaded file against its expected digest, erroring on mismatch.
///
/// Used right after a download writes a file: a digest mismatch is a hard error (the file is
/// removed by the caller) so a truncated or tampered download is never accepted. Pure apart from
/// reading the file it is asked to verify.
pub fn verify_file_sha256(path: &std::path::Path, expected_sha256: &str) -> Result<()> {
    let digest = crate::utils::file_sha256_hex(path)
        .with_context(|| format!("hashing downloaded model file {}", path.display()))?;
    if digest != expected_sha256 {
        anyhow::bail!(
            "model file {} failed SHA-256 verification (expected {expected_sha256}, got {digest})",
            path.display()
        );
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Consent-gated, SHA-256-verified, off-thread, cancelable, progress-reporting download (§C.5).
// ---------------------------------------------------------------------------

/// Progress sink for a model download (PathKeep-owned so vault-core stays decoupled from hf-hub).
///
/// The worker implements this to forward per-file byte progress to the UI's `pathkeep://`-style
/// event channel; a no-op default lets the foreground/offline path skip reporting. `cancelled`
/// lets the caller abort BETWEEN files at a consistent boundary (a half-written file is discarded,
/// never placed into `models_dir`), satisfying the "cancelable" requirement without tearing a
/// partially-verified blob into the model directory.
pub trait ModelDownloadProgress: Send {
    /// Announces the start of one file's download with its total size in bytes (0 if unknown).
    fn file_started(&mut self, _file: &str, _total_bytes: u64) {}
    /// Announces that one file finished downloading + verifying.
    fn file_finished(&mut self, _file: &str) {}
    /// Returns whether the caller has asked to cancel; checked between files.
    fn cancelled(&self) -> bool {
        false
    }
}

/// A do-nothing progress sink for the offline/foreground path.
#[derive(Debug, Default)]
pub struct NoopDownloadProgress;
impl ModelDownloadProgress for NoopDownloadProgress {}

/// Decides, for one manifest file, whether it must be (re)downloaded or is already satisfied.
///
/// Pure (one stat + hash of the existing file): returns `false` when the file is already present
/// AND matches its pinned digest (offline-first reuse — no network when the bytes are already
/// correct), `true` otherwise (missing or corrupt → must fetch). Factored out so the offline-first
/// decision is unit-tested without a network.
pub fn download_needed(model_dir: &std::path::Path, file: &ModelFile) -> bool {
    let path = model_dir.join(file.name);
    match crate::utils::file_sha256_hex(&path) {
        Ok(digest) => digest != file.sha256,
        Err(_) => true,
    }
}

/// Ensures the model is present + verified in `models_dir`, downloading only what is missing.
///
/// The §C.5 contract:
/// - **consent-gated**: with `consented == false` it NEVER touches the network; it returns `Ok` iff
///   the model is ALREADY present + verified (offline/sideloaded), else an error telling the caller
///   consent is required. No auto-download ever happens without explicit consent.
/// - **offline-first**: a file already on disk that matches its digest is reused untouched; with
///   every file satisfied the function returns without any network call even when `consented`.
/// - **SHA-256 verified**: every freshly downloaded file is hashed and a mismatch is a hard error
///   (the bad blob is removed), so a truncated/tampered download is never accepted. On full success
///   a verified marker is written so later loads skip the full re-hash (S5).
/// - **cancelable**: `progress.cancelled()` is checked before each file; a cancel returns an error
///   without leaving a partial file in `models_dir`.
/// - **progress**: `progress.file_started/finished` bracket each downloaded file.
///
/// Runs synchronously (blocking hf-hub `ureq`); the worker calls it on a blocking/off-thread task
/// so the UI never blocks (the off-thread hop is the worker's job, mirroring the import/backup
/// pattern). Returns the resolved `models_dir/<repo-segment>` directory on success.
pub fn ensure_model_downloaded(
    paths: &ProjectPaths,
    repo: &str,
    files: &[ModelFile],
    consented: bool,
    progress: &mut dyn ModelDownloadProgress,
) -> Result<std::path::PathBuf> {
    let model_dir = model_dir_for_repo(paths, repo);

    // Offline-first: if everything is already present + verified, we are done with NO network and
    // regardless of consent (the user already has the bytes). Refresh the verified marker so a
    // sideloaded (manually-placed) model is also fast to load thereafter.
    if model_is_present_and_verified(&model_dir, files) {
        write_verified_marker(&model_dir)?;
        return Ok(model_dir);
    }

    if !consented {
        anyhow::bail!(
            "candle model {repo} is not present and downloading it requires explicit consent; enable the in-app model download to fetch it"
        );
    }

    std::fs::create_dir_all(&model_dir)
        .with_context(|| format!("creating model directory {}", model_dir.display()))?;

    for file in files {
        if progress.cancelled() {
            anyhow::bail!("candle model download for {repo} was cancelled");
        }
        if !download_needed(&model_dir, file) {
            continue; // Already present + verified (offline-first per-file reuse).
        }
        progress.file_started(file.name, 0);
        let dest = model_dir.join(file.name);
        fetch_one_file(file, &dest)?;
        // Verify the freshly written file; a mismatch removes the bad blob and errors.
        if let Err(error) = verify_file_sha256(&dest, file.sha256) {
            let _ = std::fs::remove_file(&dest);
            return Err(error);
        }
        progress.file_finished(file.name);
    }
    // Every file is downloaded + verified once: record the marker so subsequent loads are cheap (S5).
    write_verified_marker(&model_dir)?;
    Ok(model_dir)
}

/// Downloads ONE file from its HF repo into `dest` (real network path).
///
/// Uses hf-hub's blocking `ureq` (rustls) transport, then copies the cached blob into PathKeep's
/// flat `models_dir/<repo-segment>/<file>` layout so the loader finds it by name. Each file declares
/// its own source repo, so the tokenizer/config (base repo) and the GGUF weights (GGUF repo) are
/// fetched from the right place. Kept tiny and gated so the surrounding consent/offline/cancel/
/// verify logic is testable without a network.
#[cfg(not(any(test, coverage)))]
fn fetch_one_file(file: &ModelFile, dest: &std::path::Path) -> Result<()> {
    let api = hf_hub::api::sync::ApiBuilder::new().build().context("building hf-hub api client")?;
    let cached = api.model(file.repo.to_string()).get(file.name).map_err(|error| {
        anyhow::anyhow!("downloading {} from {}: {error}", file.name, file.repo)
    })?;
    std::fs::copy(&cached, dest)
        .with_context(|| format!("placing downloaded {} into {}", file.name, dest.display()))?;
    Ok(())
}

/// Deterministic offline stub for the download fetch (tests/coverage): writes the expected bytes.
///
/// The tests drive `ensure_model_downloaded` with a manifest whose digests match these synthetic
/// bytes, so the consent/offline/cancel/verify/progress logic runs end-to-end with no network. The
/// real hf-hub path is compiled out under test/coverage.
#[cfg(any(test, coverage))]
fn fetch_one_file(file: &ModelFile, dest: &std::path::Path) -> Result<()> {
    std::fs::write(dest, stub_file_bytes(file.name))
        .with_context(|| format!("writing stub model file {}", dest.display()))?;
    Ok(())
}

/// The synthetic bytes the download stub writes for a given file name.
#[cfg(any(test, coverage))]
fn stub_file_bytes(name: &str) -> Vec<u8> {
    format!("stub-model-file::{name}").into_bytes()
}

// ---------------------------------------------------------------------------
// Instruction templating (pure — the role becomes REAL here, W-AI-0 §8 / §C.2).
// ---------------------------------------------------------------------------

/// The Qwen3-Embedding query-instruction task description.
///
/// Qwen3-Embedding encodes a query as `Instruct: {task}\nQuery:{text}` and a document as the bare
/// text (no instruction). The default task is the generic retrieval instruction from the model
/// card. It is a runtime string, not a model assumption — a different instruction-tuned model can
/// override it without touching the engine.
pub const QWEN3_QUERY_TASK: &str =
    "Given a web search query, retrieve relevant browser history entries that answer the query";

/// The instruction TEMPLATE recorded in the descriptor/fingerprint (with a `{text}` placeholder).
///
/// Stored in the [`EmbeddingDescriptor::instruction_template`] so the fingerprint changes if the
/// instruction changes (a different instruction = a different encoding = a stale index). Uses a
/// placeholder rather than a concrete query so the fingerprint is text-independent.
///
/// FORMAT (S2): the Qwen3-Embedding query format is `Instruct: {task}\nQuery:{text}` with NO space
/// after `Query:` — per the model card's `get_detailed_instruct` and `config_sentence_transformers.json`.
pub fn query_instruction_template(task: &str) -> String {
    format!("Instruct: {task}\nQuery:{{text}}")
}

/// Applies the role-specific instruction to one input text (the §C.2 query/document asymmetry).
///
/// `Query` → `Instruct: {task}\nQuery:{text}` (the model encodes queries with an instruction, with
/// NO space after `Query:` per the model card, S2); `Document` → the bare text (documents carry no
/// instruction). This is the function that makes [`EmbeddingRole`] load-bearing for the in-app
/// engine. Pure → unit-tested so a swap of the two arms (or a dropped instruction, or a stray space)
/// is caught.
pub fn apply_role_instruction(text: &str, role: EmbeddingRole, task: &str) -> String {
    match role {
        EmbeddingRole::Query => format!("Instruct: {task}\nQuery:{text}"),
        EmbeddingRole::Document => text.to_string(),
    }
}

// ---------------------------------------------------------------------------
// Last-token pooling (pure tensor math — testable with a tiny synthetic input).
// ---------------------------------------------------------------------------

/// Last-token pooling over one sequence's hidden states.
///
/// Qwen3-Embedding is a causal decoder, so the LAST non-pad token's hidden state summarizes the
/// whole input (every later position can attend to it). Inputs are embedded ONE sequence per
/// forward with NO padding (each text is its own forward), so "last token" is simply the final row
/// of the `[seq_len, hidden]` hidden-state matrix — there is no pad to skip. This pure helper takes
/// that matrix as `seq_len` rows of `hidden` components and returns the final row, erroring on an
/// empty sequence (a zero-token input must be rejected upstream). Keeping it pure lets a tiny
/// synthetic matrix prove the math without loading a model.
pub fn last_token_pool(rows: &[Vec<f32>]) -> Result<Vec<f32>> {
    rows.last().cloned().context("last-token pooling requires at least one token in the sequence")
}

// ---------------------------------------------------------------------------
// Config parsing (pure — reads the model's real architecture, never assumed).
// ---------------------------------------------------------------------------

/// The subset of `config.json` PathKeep reads to size the engine, plus the dtype hint.
///
/// Deserialized from the model's own `config.json` (D4: the architecture is READ, never assumed).
/// `hidden_size` is the model's native embedding dimension BEFORE any pooling/MRL — the descriptor
/// still records the ACTUAL returned length, but this lets a test assert the parsed dim without a
/// forward pass and cross-checks the GGUF's own metadata. (The quantized GGUF carries the same
/// architecture in its own metadata, which the loader reads directly; this config.json is the
/// human-readable sibling kept for the manifest + a sanity cross-check.)
#[derive(Debug, Clone, serde::Deserialize)]
pub struct CandleModelMetadata {
    /// Hidden size = the native per-token embedding width (the pooled vector's length).
    pub hidden_size: usize,
    /// Maximum positions the model was trained for (the model's context window).
    pub max_position_embeddings: usize,
}

/// Parses the model's `config.json` bytes into the engine's metadata view.
///
/// Pure (no I/O) so a malformed/short config is caught by a unit test rather than at model load.
/// Ignores fields the engine does not need; a missing `hidden_size`/`max_position_embeddings`
/// (the two it does need) is a hard error.
pub fn parse_model_metadata(config_json: &[u8]) -> Result<CandleModelMetadata> {
    serde_json::from_slice(config_json).context("parsing candle model config.json")
}

/// Max input tokens the engine keeps before truncation.
///
/// Qwen3-Embedding content rows are short (a URL/title/snippet), so a bounded window keeps each
/// forward fast. This is an inference window, NOT a model assumption — it only truncates the input
/// the same way the reference embedding pipeline does.
pub const CANDLE_MAX_INPUT_TOKENS: usize = 1024;

// ---------------------------------------------------------------------------
// The vendored quantized Qwen3 embedding forward (real build only).
// ---------------------------------------------------------------------------
//
// Built from candle-transformers' PUBLIC quantized building blocks because the shipped
// `quantized_qwen3::ModelWeights` only exposes a LOGITS forward (it applies lm_head + has private
// fields) — an embedding model needs the last-token HIDDEN STATE instead. This is a faithful copy
// of candle's `quantized_qwen3` decoder that stops at the final RMSNorm and returns hidden states.

#[cfg(not(any(test, coverage)))]
mod quant_engine {
    use super::*;
    use candle_core::quantized::gguf_file;
    use candle_nn::kv_cache::ConcatKvCache;
    use candle_nn::{Embedding, Module};
    use candle_transformers::models::with_tracing::QMatMul;
    use candle_transformers::quantized_nn::RmsNorm;
    use candle_transformers::utils::repeat_kv;
    use std::sync::Arc;

    /// candle's own `Result` (the tensor ops return `candle_core::Error`). The internal forward
    /// methods use it natively and only the public boundary methods lift into `anyhow`.
    type CResult<T> = candle_core::Result<T>;

    /// RoPE tables for the quantized Qwen3 attention (cos/sin precomputed for the context window).
    struct RotaryEmbedding {
        sin: Tensor,
        cos: Tensor,
    }

    impl RotaryEmbedding {
        fn new(
            head_dim: usize,
            max_seq_len: usize,
            rope_theta: f64,
            dev: &Device,
        ) -> CResult<Self> {
            let inv_freq: Vec<f32> = (0..head_dim)
                .step_by(2)
                .map(|i| 1f32 / rope_theta.powf(i as f64 / head_dim as f64) as f32)
                .collect();
            let inv_freq_len = inv_freq.len();
            let inv_freq = Tensor::from_vec(inv_freq, (1, inv_freq_len), dev)?;
            let t = Tensor::arange(0u32, max_seq_len as u32, dev)?
                .to_dtype(candle_core::DType::F32)?
                .reshape((max_seq_len, 1))?;
            let freqs = t.matmul(&inv_freq)?;
            Ok(Self { sin: freqs.sin()?, cos: freqs.cos()? })
        }

        fn apply(&self, q: &Tensor, k: &Tensor, offset: usize) -> CResult<(Tensor, Tensor)> {
            let (_, _, seq_len, _) = q.dims4()?;
            let cos = self.cos.narrow(0, offset, seq_len)?;
            let sin = self.sin.narrow(0, offset, seq_len)?;
            let q_embed = candle_nn::rotary_emb::rope(&q.contiguous()?, &cos, &sin)?;
            let k_embed = candle_nn::rotary_emb::rope(&k.contiguous()?, &cos, &sin)?;
            Ok((q_embed, k_embed))
        }
    }

    /// One quantized attention block (q/k/v/o projections + per-head q/k RMSNorm + KV cache).
    struct Attention {
        q_proj: QMatMul,
        k_proj: QMatMul,
        v_proj: QMatMul,
        o_proj: QMatMul,
        q_norm: RmsNorm,
        k_norm: RmsNorm,
        num_heads: usize,
        num_kv_heads: usize,
        num_kv_groups: usize,
        head_dim: usize,
        rotary: Arc<RotaryEmbedding>,
        kv_cache: ConcatKvCache,
    }

    impl Attention {
        fn forward(&mut self, x: &Tensor, mask: Option<&Tensor>, offset: usize) -> CResult<Tensor> {
            let (b, l, _) = x.dims3()?;
            let q = self.q_proj.forward(x)?;
            let k = self.k_proj.forward(x)?;
            let v = self.v_proj.forward(x)?;

            let q = q.reshape((b, l, self.num_heads, self.head_dim))?.transpose(1, 2)?;
            let k = k.reshape((b, l, self.num_kv_heads, self.head_dim))?.transpose(1, 2)?;
            let v = v.reshape((b, l, self.num_kv_heads, self.head_dim))?.transpose(1, 2)?;

            let q_flat = self.q_norm.forward(&q.flatten(0, 2)?)?;
            let k_flat = self.k_norm.forward(&k.flatten(0, 2)?)?;
            let q = q_flat.reshape((b, self.num_heads, l, self.head_dim))?;
            let k = k_flat.reshape((b, self.num_kv_heads, l, self.head_dim))?;

            let (q, k) = self.rotary.apply(&q, &k, offset)?;
            let (k, v) = self.kv_cache.append(&k, &v)?;

            let k = repeat_kv(k, self.num_kv_groups)?.contiguous()?;
            let v = repeat_kv(v, self.num_kv_groups)?.contiguous()?;

            let scale = 1.0 / (self.head_dim as f64).sqrt();
            let mut scores = (q.matmul(&k.transpose(2, 3)?)? * scale)?;
            if let Some(m) = mask {
                scores = scores.broadcast_add(m)?;
            }
            let probs = candle_nn::ops::softmax_last_dim(&scores)?;
            let ctx = probs.matmul(&v)?;
            let ctx = ctx.transpose(1, 2)?.reshape((b, l, self.num_heads * self.head_dim))?;
            self.o_proj.forward(&ctx)
        }

        fn clear_kv_cache(&mut self) {
            self.kv_cache.reset();
        }
    }

    /// One decoder layer (pre-norm attention + pre-norm SwiGLU MLP, residual around each).
    struct Layer {
        attn: Attention,
        ln1: RmsNorm,
        ln2: RmsNorm,
        gate_proj: QMatMul,
        up_proj: QMatMul,
        down_proj: QMatMul,
    }

    impl Layer {
        fn forward(&mut self, x: &Tensor, mask: Option<&Tensor>, offset: usize) -> CResult<Tensor> {
            let h = self.ln1.forward(x)?;
            let h = self.attn.forward(&h, mask, offset)?;
            let x = (x + h)?;
            let h2 = self.ln2.forward(&x)?;
            let gate = self.gate_proj.forward(&h2)?;
            let gate = candle_nn::ops::silu(&gate)?;
            let up = self.up_proj.forward(&h2)?;
            let h2 = self.down_proj.forward(&(gate * up)?)?;
            x + h2
        }

        fn clear_kv_cache(&mut self) {
            self.attn.clear_kv_cache();
        }
    }

    /// A quantized Qwen3 decoder that returns LAST-TOKEN HIDDEN STATES (not logits).
    ///
    /// Loads a GGUF checkpoint and runs the standard Qwen3 forward up to and including the final
    /// RMSNorm, then returns the last token's hidden state — exactly the input to the last-token
    /// pooling and L2-normalize that Qwen3-Embedding requires. The model is reused across texts:
    /// [`Self::clear_kv_cache`] resets the cache so each text is independent without rebuilding.
    pub struct QuantizedQwen3Embedding {
        embed_tokens: Embedding,
        layers: Vec<Layer>,
        norm: RmsNorm,
        device: Device,
        /// The model's native hidden size (the pooled vector length), read from GGUF metadata.
        pub hidden_size: usize,
        /// The model's trained context window, read from GGUF metadata.
        pub max_position_embeddings: usize,
    }

    impl QuantizedQwen3Embedding {
        /// Loads the engine from a GGUF file path on disk (offline).
        pub fn from_gguf_path(path: &std::path::Path, device: &Device) -> Result<Self> {
            let mut file = std::fs::File::open(path)
                .with_context(|| format!("opening GGUF weights {}", path.display()))?;
            let content = gguf_file::Content::read(&mut file).map_err(|error| {
                anyhow::anyhow!("reading GGUF header from {}: {error}", path.display())
            })?;

            let md = |key: &str| -> Result<&gguf_file::Value> {
                content.metadata.get(key).with_context(|| format!("GGUF metadata missing {key}"))
            };
            let num_heads = md("qwen3.attention.head_count")?.to_u32()? as usize;
            let num_kv_heads = md("qwen3.attention.head_count_kv")?.to_u32()? as usize;
            let head_dim = md("qwen3.attention.key_length")?.to_u32()? as usize;
            let num_layers = md("qwen3.block_count")?.to_u32()? as usize;
            let hidden_size = md("qwen3.embedding_length")?.to_u32()? as usize;
            let max_position_embeddings = md("qwen3.context_length")?.to_u32()? as usize;
            let rms_eps = md("qwen3.attention.layer_norm_rms_epsilon")?.to_f32()? as f64;
            let rope_theta = md("qwen3.rope.freq_base")?.to_f32()? as f64;
            if num_kv_heads == 0 || num_heads % num_kv_heads != 0 {
                anyhow::bail!(
                    "GGUF head_count {num_heads} is not a multiple of head_count_kv {num_kv_heads}"
                );
            }
            let num_kv_groups = num_heads / num_kv_heads;

            // Bound the RoPE precompute to our input window (the full 32k table is a multi-MB
            // allocation we never need for short rows; inputs are truncated to the same bound).
            let rope_window = CANDLE_MAX_INPUT_TOKENS.min(max_position_embeddings);
            let rotary = Arc::new(RotaryEmbedding::new(head_dim, rope_window, rope_theta, device)?);

            let mut tensor = |name: &str| -> Result<candle_core::quantized::QTensor> {
                content
                    .tensor(&mut file, name, device)
                    .map_err(|error| anyhow::anyhow!("reading GGUF tensor {name}: {error}"))
            };

            let embed_q = tensor("token_embd.weight")?;
            let embed_tokens = Embedding::new(
                embed_q.dequantize(device).context("dequantizing token embeddings")?,
                hidden_size,
            );

            let mut layers = Vec::with_capacity(num_layers);
            for i in 0..num_layers {
                let prefix = format!("blk.{i}");
                let q_proj =
                    QMatMul::from_weights(tensor(&format!("{prefix}.attn_q.weight"))?.into())?;
                let k_proj =
                    QMatMul::from_weights(tensor(&format!("{prefix}.attn_k.weight"))?.into())?;
                let v_proj =
                    QMatMul::from_weights(tensor(&format!("{prefix}.attn_v.weight"))?.into())?;
                let o_proj =
                    QMatMul::from_weights(tensor(&format!("{prefix}.attn_output.weight"))?.into())?;
                let q_norm = RmsNorm::from_qtensor(
                    tensor(&format!("{prefix}.attn_q_norm.weight"))?,
                    rms_eps,
                )?;
                let k_norm = RmsNorm::from_qtensor(
                    tensor(&format!("{prefix}.attn_k_norm.weight"))?,
                    rms_eps,
                )?;
                let ln1 =
                    RmsNorm::from_qtensor(tensor(&format!("{prefix}.attn_norm.weight"))?, rms_eps)?;
                let ln2 =
                    RmsNorm::from_qtensor(tensor(&format!("{prefix}.ffn_norm.weight"))?, rms_eps)?;
                let gate_proj =
                    QMatMul::from_weights(tensor(&format!("{prefix}.ffn_gate.weight"))?.into())?;
                let up_proj =
                    QMatMul::from_weights(tensor(&format!("{prefix}.ffn_up.weight"))?.into())?;
                let down_proj =
                    QMatMul::from_weights(tensor(&format!("{prefix}.ffn_down.weight"))?.into())?;
                layers.push(Layer {
                    attn: Attention {
                        q_proj,
                        k_proj,
                        v_proj,
                        o_proj,
                        q_norm,
                        k_norm,
                        num_heads,
                        num_kv_heads,
                        num_kv_groups,
                        head_dim,
                        rotary: rotary.clone(),
                        kv_cache: ConcatKvCache::new(2),
                    },
                    ln1,
                    ln2,
                    gate_proj,
                    up_proj,
                    down_proj,
                });
            }
            let norm = RmsNorm::from_qtensor(tensor("output_norm.weight")?, rms_eps)?;

            Ok(Self {
                embed_tokens,
                layers,
                norm,
                device: device.clone(),
                hidden_size,
                max_position_embeddings,
            })
        }

        /// Resets every layer's KV cache so the next forward starts a fresh, independent sequence.
        pub fn clear_kv_cache(&mut self) {
            for layer in &mut self.layers {
                layer.clear_kv_cache();
            }
        }

        /// Builds a causal attention mask for a prompt of `seq_len` tokens at `offset`.
        fn causal_mask(&self, seq_len: usize) -> Result<Tensor> {
            let minf = f32::NEG_INFINITY;
            let mask: Vec<f32> = (0..seq_len)
                .flat_map(|i| (0..seq_len).map(move |j| if j <= i { 0.0 } else { minf }))
                .collect();
            Tensor::from_slice(&mask, (1, 1, seq_len, seq_len), &self.device)
                .context("building causal mask")
        }

        /// Runs one token id sequence through the decoder and returns the LAST token's hidden state.
        ///
        /// `[1, seq_len]` ids → final RMSNorm hidden states `[1, seq_len, hidden]` → the last row as
        /// `Vec<f32>` of length `hidden_size`. This is exactly last-token pooling for a causal decoder
        /// with no padding (one text per forward). The KV cache must be clear before each call.
        pub fn last_token_hidden(&mut self, ids: &[u32]) -> Result<Vec<f32>> {
            let seq_len = ids.len();
            if seq_len == 0 {
                anyhow::bail!("quantized forward requires at least one token");
            }
            let input = Tensor::from_slice(ids, (1, seq_len), &self.device)
                .context("building input id tensor")?;
            let mut h = self.embed_tokens.forward(&input)?;
            let mask = if seq_len == 1 { None } else { Some(self.causal_mask(seq_len)?) };
            for layer in &mut self.layers {
                h = layer.forward(&h, mask.as_ref(), 0)?;
            }
            let h = self.norm.forward(&h)?;
            // [1, seq_len, hidden] → last token → [hidden].
            let last = h
                .narrow(1, seq_len - 1, 1)?
                .squeeze(1)?
                .squeeze(0)?
                .to_dtype(candle_core::DType::F32)?;
            last.to_vec1::<f32>().context("reading last-token hidden vector")
        }
    }
}

// ---------------------------------------------------------------------------
// The provider.
// ---------------------------------------------------------------------------

/// In-app candle embedding provider implementing [`EmbeddingProvider`].
///
/// Holds the loaded quantized model + tokenizer (real build) or the deterministic descriptor
/// (test/coverage build). The model is behind a [`Mutex`] because the forward mutates a KV cache
/// (it is `&mut self`) while the trait's `embed` is `&self`; the lock serializes forwards, which is
/// correct for a single CPU-bound engine on a 4-core box (concurrency would only thrash cache +
/// cores). The real engine is gated behind `cfg(not(any(test, coverage)))`; the stub returns
/// deterministic, role-aware, normalized vectors with the same public call graph so the `Candle`
/// enum arm and the embed loop are exercised at 100% coverage without weights.
pub struct CandleEmbeddingProvider {
    /// Provider identity recorded in the descriptor/fingerprint.
    provider_id: String,
    /// Model identifier recorded in the descriptor/fingerprint. It is the repo id WITH the quant
    /// level appended (`<repo>:<quant>`), so the fingerprint changes when the quant changes — a
    /// quant swap re-embeds the index instead of silently mixing Q4 and Q8 vectors (D4 / §C.4).
    model_id: String,
    /// The query-instruction task description threaded into the `Query` role prefix.
    query_task: String,
    /// The native hidden size read from the model (the pooled vector length).
    hidden_size: usize,
    /// The real engine state (loaded quantized model + tokenizer), only present in non-test builds.
    #[cfg(not(any(test, coverage)))]
    engine: Mutex<CandleEngine>,
}

/// The loaded candle engine: the quantized model + tokenizer (real build).
///
/// One [`quant_engine::QuantizedQwen3Embedding`] is loaded ONCE from the GGUF and reused for every
/// text (its KV cache is reset between texts via `clear_kv_cache`), so there is no per-text model
/// rebuild — the quantized weights live in candle's quantized tensors (far smaller in RAM than the
/// old F32 weight map) and are never reconverted on the hot path. The `tokenizers::Tokenizer` is
/// reused across texts (it is stateless).
#[cfg(not(any(test, coverage)))]
struct CandleEngine {
    model: quant_engine::QuantizedQwen3Embedding,
    tokenizer: tokenizers::Tokenizer,
}

impl CandleEmbeddingProvider {
    /// Composes the fingerprint-bearing model id from a repo and quant level (`<repo>:<quant>`).
    ///
    /// Pure so the id composition is unit-tested. Recording the quant in the model id is how a quant
    /// change invalidates the embedding fingerprint (§C.4): two indexes built at different quant
    /// levels never share a fingerprint, so switching quant re-embeds.
    pub fn model_id_for(repo: &str, quant: &str) -> String {
        format!("{repo}:{quant}")
    }

    /// Loads the default-model candle engine from `models_dir`, requiring a verified model on disk.
    ///
    /// This is the production entrypoint. It is consent-gated by CONSTRUCTION: the caller only ever
    /// calls it after the model is present + verified (the selector checks the availability gate,
    /// itself gated on a consented download, before selecting candle). Loading is offline (no
    /// network): it reads only the files on disk.
    #[cfg(not(any(test, coverage)))]
    pub fn load_default(paths: &ProjectPaths) -> Result<Self> {
        Self::load(
            paths,
            DEFAULT_CANDLE_MODEL_REPO,
            DEFAULT_CANDLE_MODEL_FILES,
            DEFAULT_CANDLE_QUANT,
            QWEN3_QUERY_TASK,
        )
    }

    /// Loads a candle engine for one repo from `models_dir`, verifying readiness first (S5: a cheap
    /// presence + verified-marker check, NOT a full re-hash of the weights on every load).
    #[cfg(not(any(test, coverage)))]
    pub fn load(
        paths: &ProjectPaths,
        repo: &str,
        files: &[ModelFile],
        quant: &str,
        query_task: &str,
    ) -> Result<Self> {
        let model_dir = model_dir_for_repo(paths, repo);
        if !model_is_loadable(&model_dir, files) {
            anyhow::bail!(
                "candle model {repo} is not present or not verified in {}; download it (with consent) before selecting the in-app engine",
                model_dir.display()
            );
        }

        let gguf_name = gguf_file_name(files)?;
        let device = Device::Cpu;
        let model = quant_engine::QuantizedQwen3Embedding::from_gguf_path(
            &model_dir.join(gguf_name),
            &device,
        )?;
        let hidden_size = model.hidden_size;

        // Cross-check the GGUF's architecture against the sibling `config.json` so a mismatched
        // tokenizer/config bundle (e.g. a config for a different model size) fails LOUDLY at load
        // rather than producing wrong-dim vectors. The GGUF metadata is authoritative for the
        // forward; config.json is the human-readable sibling we verify against (D4: read, not assume).
        let config_bytes = std::fs::read(model_dir.join("config.json"))
            .with_context(|| format!("reading config.json for {repo}"))?;
        let metadata = parse_model_metadata(&config_bytes)?;
        if metadata.hidden_size != hidden_size {
            anyhow::bail!(
                "candle model {repo} config.json hidden_size {} disagrees with GGUF embedding_length {hidden_size}; the bundle is inconsistent",
                metadata.hidden_size
            );
        }
        if model.max_position_embeddings != metadata.max_position_embeddings {
            anyhow::bail!(
                "candle model {repo} config.json max_position_embeddings {} disagrees with GGUF context_length {}; the bundle is inconsistent",
                metadata.max_position_embeddings,
                model.max_position_embeddings
            );
        }

        let tokenizer = tokenizers::Tokenizer::from_file(model_dir.join("tokenizer.json"))
            .map_err(|error| anyhow::anyhow!("loading tokenizer for {repo}: {error}"))?;

        Ok(Self {
            provider_id: format!("candle:{repo}"),
            model_id: Self::model_id_for(repo, quant),
            query_task: query_task.to_string(),
            hidden_size,
            engine: Mutex::new(CandleEngine { model, tokenizer }),
        })
    }

    /// Builds a deterministic stub provider for tests/coverage (no weights, no network).
    ///
    /// Mirrors the real provider's public shape so [`super::embedding_external::AnyEmbeddingProvider`]
    /// `Candle` dispatch and the embed loop run at 100% coverage. The `hidden_size` is a small
    /// synthetic number with NO relationship to any real model (picking 1024 here would be the D4
    /// truth-assumption this design removes).
    #[cfg(any(test, coverage))]
    pub fn new_stub(provider_id: &str, repo: &str, quant: &str) -> Self {
        Self {
            provider_id: provider_id.to_string(),
            model_id: Self::model_id_for(repo, quant),
            query_task: QWEN3_QUERY_TASK.to_string(),
            hidden_size: STUB_CANDLE_DIM,
        }
    }

    /// Builds this engine's runtime descriptor once the dim is known.
    ///
    /// Sets the per-adapter truth the W-AI-0 A-S2 carryover requires for the candle path:
    /// dtype=Float32 (the engine emits float32 vector COMPONENTS — quantization is a weight-storage
    /// property, not an output-component dtype), normalized=true (we L2-normalize every vector
    /// below), pooling=LastToken (the model's real pooling), and a populated instruction_template
    /// (the query asymmetry). `effective_dim` is the ACTUAL returned length when observed, else the
    /// native hidden size — never a hardcoded constant (D4). The quant level is carried in
    /// `model_id`, so it is already part of the fingerprint.
    fn descriptor_with(&self, effective_dim: Option<usize>) -> EmbeddingDescriptor {
        EmbeddingDescriptor {
            provider_id: self.provider_id.clone(),
            model_id: self.model_id.clone(),
            effective_dim: effective_dim.or(Some(self.hidden_size)),
            dtype: EmbeddingDtype::Float32,
            normalized: true,
            pooling: EmbeddingPooling::LastToken,
            instruction_template: Some(query_instruction_template(&self.query_task)),
        }
    }
}

impl EmbeddingProvider for CandleEmbeddingProvider {
    /// Embeds a batch under one role: applies the role instruction, runs a per-text forward,
    /// last-token-pools, and L2-normalizes — one vector per input, in order.
    fn embed(
        &self,
        texts: &[String],
        role: EmbeddingRole,
    ) -> impl std::future::Future<Output = Result<Vec<Vec<f32>>>> + Send {
        embed_impl(self, texts, role)
    }

    fn model_id(&self) -> &str {
        &self.model_id
    }

    fn descriptor(&self) -> EmbeddingDescriptor {
        // `effective_dim` is the native hidden size here; the indexing loop re-derives the
        // descriptor from the actual returned length when it stamps the fingerprint.
        self.descriptor_with(None)
    }
}

/// Real candle embed: per-text forward → last-token pool → L2-normalize.
///
/// One text per forward with NO padding, so the pooled token is unambiguously the final row and
/// never a pad. The single quantized model is reused across texts under the engine lock: each text
/// resets the KV cache (`clear_kv_cache`) so sequences stay independent without rebuilding the model
/// (the throughput win over the F32 path, whose base model could not reset its cache). All forwards
/// run under the engine lock because they share one mutable model; serializing them is correct for a
/// single CPU-bound engine (concurrent forwards would only contend for the same cores).
#[cfg(not(any(test, coverage)))]
async fn embed_impl(
    provider: &CandleEmbeddingProvider,
    texts: &[String],
    role: EmbeddingRole,
) -> Result<Vec<Vec<f32>>> {
    if texts.is_empty() {
        return Ok(Vec::new());
    }
    let mut engine =
        provider.engine.lock().map_err(|_| anyhow::anyhow!("candle engine lock poisoned"))?;
    let CandleEngine { model, tokenizer } = &mut *engine;
    let mut out = Vec::with_capacity(texts.len());
    for text in texts {
        let prepared = apply_role_instruction(text, role, &provider.query_task);
        let encoding = tokenizer
            .encode(prepared, true)
            .map_err(|error| anyhow::anyhow!("tokenizing input: {error}"))?;
        let mut ids: Vec<u32> = encoding.get_ids().to_vec();
        if ids.is_empty() {
            anyhow::bail!("tokenizer produced no tokens for the input");
        }
        ids.truncate(CANDLE_MAX_INPUT_TOKENS);
        // Reset the cache so this text is independent of the previous one, then forward.
        model.clear_kv_cache();
        let mut vector = model.last_token_hidden(&ids)?;
        l2_normalize(&mut vector);
        out.push(vector);
    }
    Ok(out)
}

/// Synthetic dimension the stub emits — an arbitrary small number unrelated to any real model.
#[cfg(any(test, coverage))]
const STUB_CANDLE_DIM: usize = 6;

/// Deterministic, role-aware, normalized stub embed for tests and coverage builds.
///
/// Same call graph as the real path (apply role → per-text → normalize) but derives each vector
/// from a per-(provider, role, text) digest so two different texts or roles differ, reuses the
/// SAME pure helpers (`apply_role_instruction`, `last_token_pool`, `l2_normalize`), and produces a
/// near-unit-norm vector. No weights, no network.
#[cfg(any(test, coverage))]
async fn embed_impl(
    provider: &CandleEmbeddingProvider,
    texts: &[String],
    role: EmbeddingRole,
) -> Result<Vec<Vec<f32>>> {
    if texts.is_empty() {
        return Ok(Vec::new());
    }
    let mut out = Vec::with_capacity(texts.len());
    for text in texts {
        let prepared = apply_role_instruction(text, role, &provider.query_task);
        let digest = crate::utils::sha256_hex(
            format!("{}::{}::{prepared}", provider.provider_id, role.as_str()).as_bytes(),
        );
        let bytes = digest.as_bytes();
        // Build a tiny synthetic [seq_len=2, hidden] hidden-state matrix and last-token pool it, so
        // the pure pooling helper is exercised on the same path the real engine uses.
        let rows: Vec<Vec<f32>> = (0..2)
            .map(|row| {
                (0..provider.hidden_size)
                    .map(|component| {
                        let index = (row * provider.hidden_size + component) % bytes.len();
                        (bytes[index] % 13) as f32 + 1.0
                    })
                    .collect()
            })
            .collect();
        let mut vector = last_token_pool(&rows)?;
        l2_normalize(&mut vector);
        out.push(vector);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{AiProviderConfig, AiProviderPurpose, AiRequestFormat};
    use secrecy::SecretString;

    fn near_unit_norm(vector: &[f32]) -> f32 {
        vector.iter().map(|value| value * value).sum::<f32>().sqrt()
    }

    fn candle_runtime(default_model: &str) -> AiProviderRuntime {
        AiProviderRuntime {
            config: AiProviderConfig {
                id: "candle-provider".to_string(),
                name: "In-app Candle".to_string(),
                purpose: AiProviderPurpose::Embedding,
                request_format: AiRequestFormat::OpenAi,
                enabled: true,
                base_url: Some(CANDLE_INAPP_BASE_URL.to_string()),
                default_model: default_model.to_string(),
                dimensions: None,
                ..AiProviderConfig::default()
            },
            api_key: SecretString::from(String::new()),
        }
    }

    fn external_runtime(format: AiRequestFormat, base_url: Option<&str>) -> AiProviderRuntime {
        AiProviderRuntime {
            config: AiProviderConfig {
                id: "external-provider".to_string(),
                name: "External".to_string(),
                purpose: AiProviderPurpose::Embedding,
                request_format: format,
                enabled: true,
                base_url: base_url.map(str::to_string),
                default_model: "text-embedding-test".to_string(),
                dimensions: None,
                ..AiProviderConfig::default()
            },
            api_key: SecretString::from("key".to_string()),
        }
    }

    #[test]
    fn apply_role_instruction_prefixes_query_only_with_no_space_after_query() {
        let task = "find stuff";
        let query = apply_role_instruction("how to bake bread", EmbeddingRole::Query, task);
        // S2: NO space after `Query:` (per the Qwen3-Embedding model card get_detailed_instruct).
        assert_eq!(query, "Instruct: find stuff\nQuery:how to bake bread");
        assert!(!query.contains("Query: "), "there must be NO space after 'Query:'");
        // Documents carry NO instruction (the asymmetry must be preserved).
        let document = apply_role_instruction("how to bake bread", EmbeddingRole::Document, task);
        assert_eq!(document, "how to bake bread");
        assert_ne!(query, document);
    }

    #[test]
    fn query_instruction_template_uses_placeholder_with_no_space() {
        let template = query_instruction_template("retrieve docs");
        // S2: NO space after `Query:`.
        assert_eq!(template, "Instruct: retrieve docs\nQuery:{text}");
        assert!(!template.contains("Query: "), "template must have NO space after 'Query:'");
        // The template is text-independent (placeholder), so the fingerprint is stable per task.
        assert!(template.contains("{text}"));
    }

    #[test]
    fn last_token_pool_returns_final_row() {
        let rows = vec![vec![1.0, 2.0], vec![3.0, 4.0], vec![5.0, 6.0]];
        assert_eq!(last_token_pool(&rows).expect("pool"), vec![5.0, 6.0]);
    }

    #[test]
    fn last_token_pool_rejects_empty_sequence() {
        let rows: Vec<Vec<f32>> = Vec::new();
        let error = last_token_pool(&rows).expect_err("empty sequence");
        assert!(error.to_string().contains("at least one token"));
    }

    #[test]
    fn parse_model_metadata_reads_required_fields_and_ignores_extras() {
        let config = br#"{
            "hidden_size": 1024,
            "max_position_embeddings": 32768,
            "num_hidden_layers": 28,
            "torch_dtype": "bfloat16"
        }"#;
        let metadata = parse_model_metadata(config).expect("parse");
        assert_eq!(metadata.hidden_size, 1024);
        assert_eq!(metadata.max_position_embeddings, 32768);
    }

    #[test]
    fn parse_model_metadata_rejects_missing_field() {
        let config = br#"{ "max_position_embeddings": 32768 }"#;
        assert!(parse_model_metadata(config).is_err());
    }

    #[test]
    fn parse_model_metadata_rejects_malformed_json() {
        assert!(parse_model_metadata(b"{not json").is_err());
    }

    #[test]
    fn model_dir_for_repo_flattens_slash_into_one_segment() {
        let dir = tempfile::tempdir().expect("tempdir");
        let paths = crate::config::project_paths_with_root(dir.path());
        let model_dir = model_dir_for_repo(&paths, "Qwen/Qwen3-Embedding-0.6B-GGUF");
        // The repo's slash must NOT create a nested directory that could escape models_dir.
        assert_eq!(model_dir.parent().expect("parent"), paths.models_dir);
        let segment = model_dir.file_name().expect("segment").to_string_lossy().to_string();
        assert!(!segment.contains('/'));
        assert!(segment.contains("Qwen3-Embedding-0.6B-GGUF"));
    }

    #[test]
    fn gguf_file_name_returns_the_single_gguf() {
        assert_eq!(
            gguf_file_name(DEFAULT_CANDLE_MODEL_FILES).expect("gguf"),
            "Qwen3-Embedding-0.6B-Q8_0.gguf"
        );
    }

    #[test]
    fn gguf_file_name_rejects_zero_or_multiple_ggufs() {
        let none = [ModelFile { name: "tokenizer.json", sha256: "x", repo: "r" }];
        assert!(gguf_file_name(&none).is_err());
        let two = [
            ModelFile { name: "a.gguf", sha256: "x", repo: "r" },
            ModelFile { name: "b.gguf", sha256: "y", repo: "r" },
        ];
        assert!(gguf_file_name(&two).is_err());
    }

    #[test]
    fn model_id_for_appends_quant_so_fingerprint_changes_on_quant_swap() {
        assert_eq!(
            CandleEmbeddingProvider::model_id_for("Qwen/Qwen3-Embedding-0.6B-GGUF", "Q8_0"),
            "Qwen/Qwen3-Embedding-0.6B-GGUF:Q8_0"
        );
        // A different quant produces a different model id (so the fingerprint differs, §C.4).
        assert_ne!(
            CandleEmbeddingProvider::model_id_for("repo", "Q8_0"),
            CandleEmbeddingProvider::model_id_for("repo", "Q4_K_M")
        );
    }

    #[test]
    fn candle_repo_for_runtime_prefers_config_then_default() {
        assert_eq!(candle_repo_for_runtime(&candle_runtime("")), DEFAULT_CANDLE_MODEL_REPO);
        assert_eq!(candle_repo_for_runtime(&candle_runtime("custom/repo")), "custom/repo");
    }

    #[test]
    fn runtime_uses_candle_detects_sentinel() {
        assert!(runtime_uses_candle(&candle_runtime("")));
        assert!(!runtime_uses_candle(&external_runtime(
            AiRequestFormat::OpenAi,
            Some("http://localhost:1234/v1")
        )));
        // A runtime with no base url at all is not candle.
        assert!(!runtime_uses_candle(&external_runtime(AiRequestFormat::OpenAi, None)));
    }

    #[test]
    fn verify_file_sha256_accepts_match_and_rejects_mismatch() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("blob.bin");
        std::fs::write(&path, b"hello candle").expect("write");
        let digest = crate::utils::sha256_hex(b"hello candle");
        verify_file_sha256(&path, &digest).expect("matching digest");
        let error = verify_file_sha256(&path, &"0".repeat(64)).expect_err("mismatch");
        assert!(error.to_string().contains("failed SHA-256 verification"));
    }

    #[test]
    fn verify_file_sha256_errors_on_missing_file() {
        let dir = tempfile::tempdir().expect("tempdir");
        let missing = dir.path().join("nope.bin");
        assert!(verify_file_sha256(&missing, &"0".repeat(64)).is_err());
    }

    fn leak_manifest(entries: &[(&'static str, &[u8])]) -> Vec<ModelFile> {
        entries
            .iter()
            .map(|(name, bytes)| {
                let digest = crate::utils::sha256_hex(bytes);
                ModelFile { name, sha256: Box::leak(digest.into_boxed_str()), repo: "Stub/Repo" }
            })
            .collect()
    }

    #[test]
    fn model_is_present_and_verified_requires_all_files_and_matching_digests() {
        let dir = tempfile::tempdir().expect("tempdir");
        let model_dir = dir.path();
        let files = leak_manifest(&[("a.txt", b"alpha"), ("b.txt", b"beta")]);

        // Missing files → not present.
        assert!(!model_is_present_and_verified(model_dir, &files));

        std::fs::write(model_dir.join("a.txt"), b"alpha").expect("write a");
        // Still missing b → not present.
        assert!(!model_is_present_and_verified(model_dir, &files));

        std::fs::write(model_dir.join("b.txt"), b"WRONG").expect("write b wrong");
        // b present but wrong digest → not verified.
        assert!(!model_is_present_and_verified(model_dir, &files));

        std::fs::write(model_dir.join("b.txt"), b"beta").expect("write b right");
        // All present + verified.
        assert!(model_is_present_and_verified(model_dir, &files));
    }

    #[test]
    fn model_is_loadable_requires_marker_and_present_nonempty_files() {
        let dir = tempfile::tempdir().expect("tempdir");
        let model_dir = dir.path();
        let files = leak_manifest(&[("w.gguf", b"weights"), ("tokenizer.json", b"tok")]);
        // No marker, no files → not loadable.
        assert!(!model_is_loadable(model_dir, &files));
        std::fs::write(model_dir.join("w.gguf"), b"weights").expect("write w");
        std::fs::write(model_dir.join("tokenizer.json"), b"tok").expect("write tok");
        // Files present but no verified marker → not loadable (never load an unverified model).
        assert!(!model_is_loadable(model_dir, &files));
        write_verified_marker(model_dir).expect("marker");
        // Marker + present non-empty files → loadable.
        assert!(model_is_loadable(model_dir, &files));
        // Marker present but a file MISSING (metadata errors) → not loadable (covers the Err arm).
        std::fs::remove_file(model_dir.join("w.gguf")).expect("remove w");
        assert!(!model_is_loadable(model_dir, &files));
        std::fs::write(model_dir.join("w.gguf"), b"weights").expect("rewrite w");
        // An empty file is not loadable even with the marker.
        std::fs::write(model_dir.join("w.gguf"), b"").expect("truncate");
        assert!(!model_is_loadable(model_dir, &files));
    }

    #[test]
    fn default_manifest_pins_exact_digests_and_repos() {
        // F2: pin the EXACT SHA-256 values (not just hex-shape) so a fat-fingered digest fails the
        // gate, not just a length typo. These are the real 2026-06-21 download hashes.
        let by_name = |name: &str| {
            DEFAULT_CANDLE_MODEL_FILES.iter().find(|f| f.name == name).expect("file present")
        };
        assert_eq!(
            by_name("config.json").sha256,
            "b5bf1f51fc45be473a54718cef92448d90a1be001bf9b9a44b8c7f10a19feaa9"
        );
        assert_eq!(
            by_name("tokenizer.json").sha256,
            "def76fb086971c7867b829c23a26261e38d9d74e02139253b38aeb9df8b4b50a"
        );
        assert_eq!(
            by_name("Qwen3-Embedding-0.6B-Q8_0.gguf").sha256,
            "06507c7b42688469c4e7298b0a1e16deff06caf291cf0a5b278c308249c3e439"
        );
        // The GGUF weights come from the GGUF repo; tokenizer + config from the base repo.
        assert_eq!(by_name("Qwen3-Embedding-0.6B-Q8_0.gguf").repo, DEFAULT_CANDLE_MODEL_REPO);
        assert_eq!(by_name("config.json").repo, DEFAULT_CANDLE_TOKENIZER_REPO);
        assert_eq!(by_name("tokenizer.json").repo, DEFAULT_CANDLE_TOKENIZER_REPO);
        // Every digest is still a 64-hex-char string (shape guard kept).
        for file in DEFAULT_CANDLE_MODEL_FILES {
            assert_eq!(file.sha256.len(), 64, "{} digest must be 64 hex chars", file.name);
            assert!(file.sha256.chars().all(|c| c.is_ascii_hexdigit()));
        }
        assert_eq!(DEFAULT_CANDLE_MODEL_REPO, "Qwen/Qwen3-Embedding-0.6B-GGUF");
        assert_eq!(DEFAULT_CANDLE_QUANT, "Q8_0");
    }

    #[tokio::test]
    async fn stub_embed_is_role_aware_normalized_and_distinct() {
        let provider =
            CandleEmbeddingProvider::new_stub("candle:test", "test-model", DEFAULT_CANDLE_QUANT);
        let docs = provider
            .embed(&["alpha".to_string(), "beta".to_string()], EmbeddingRole::Document)
            .await
            .expect("embed docs");
        assert_eq!(docs.len(), 2);
        assert_eq!(docs[0].len(), STUB_CANDLE_DIM);
        assert!((near_unit_norm(&docs[0]) - 1.0).abs() < 1e-6);
        assert_ne!(docs[0], docs[1], "different texts differ");

        // Same text under a different role differs (the role instruction is applied in the stub).
        let query = provider
            .embed(&["alpha".to_string()], EmbeddingRole::Query)
            .await
            .expect("embed query");
        assert_ne!(query[0], docs[0]);
    }

    #[tokio::test]
    async fn stub_embed_empty_input_short_circuits() {
        let provider =
            CandleEmbeddingProvider::new_stub("candle:test", "test-model", DEFAULT_CANDLE_QUANT);
        assert!(provider.embed(&[], EmbeddingRole::Document).await.expect("empty").is_empty());
    }

    #[test]
    fn descriptor_records_candle_truth_including_quant_in_model_id() {
        let provider = CandleEmbeddingProvider::new_stub("candle:test", "repo", "Q8_0");
        let descriptor = provider.descriptor();
        assert_eq!(descriptor.provider_id, "candle:test");
        // The model id carries the quant so the fingerprint changes on a quant swap.
        assert_eq!(descriptor.model_id, "repo:Q8_0");
        assert_eq!(descriptor.dtype, EmbeddingDtype::Float32);
        assert!(descriptor.normalized);
        assert_eq!(descriptor.pooling, EmbeddingPooling::LastToken);
        let template = descriptor.instruction_template.expect("template");
        assert!(template.contains("Instruct:"));
        assert!(!template.contains("Query: "), "S2: no space after Query:");
        // effective_dim defaults to the native hidden size when no real vector observed yet.
        assert_eq!(descriptor.effective_dim, Some(STUB_CANDLE_DIM));
        assert_eq!(provider.model_id(), "repo:Q8_0");
    }

    #[test]
    fn descriptor_with_prefers_observed_dim() {
        let provider = CandleEmbeddingProvider::new_stub("candle:test", "repo", "Q8_0");
        // An observed length wins over the native hidden size (D4: the real returned length).
        assert_eq!(provider.descriptor_with(Some(42)).effective_dim, Some(42));
    }

    // ---- selector + graceful degradation (S3, coverage-F1) ----

    #[test]
    fn select_embedding_provider_routes_candle_arm() {
        let dir = tempfile::tempdir().expect("tempdir");
        let paths = crate::config::project_paths_with_root(dir.path());
        // Default model empty → the candle stub for the DEFAULT repo, with the quant in the id.
        let provider = select_embedding_provider(&paths, &candle_runtime("")).expect("candle");
        match provider {
            AnyEmbeddingProvider::Candle(candle) => {
                assert_eq!(
                    candle.model_id(),
                    CandleEmbeddingProvider::model_id_for(
                        DEFAULT_CANDLE_MODEL_REPO,
                        DEFAULT_CANDLE_QUANT
                    )
                );
            }
            AnyEmbeddingProvider::External(_) => panic!("expected the candle arm"),
        }
    }

    #[test]
    fn select_embedding_provider_uses_configured_repo() {
        let dir = tempfile::tempdir().expect("tempdir");
        let paths = crate::config::project_paths_with_root(dir.path());
        let provider =
            select_embedding_provider(&paths, &candle_runtime("custom/repo")).expect("candle");
        match provider {
            AnyEmbeddingProvider::Candle(candle) => {
                assert_eq!(candle.model_id(), "custom/repo:Q8_0");
            }
            AnyEmbeddingProvider::External(_) => panic!("expected the candle arm"),
        }
    }

    #[test]
    fn select_embedding_provider_routes_external_arm() {
        let dir = tempfile::tempdir().expect("tempdir");
        let paths = crate::config::project_paths_with_root(dir.path());
        let provider = select_embedding_provider(
            &paths,
            &external_runtime(AiRequestFormat::LmStudio, Some("http://localhost:1234/v1")),
        )
        .expect("external");
        match provider {
            AnyEmbeddingProvider::External(external) => {
                assert_eq!(external.model_id(), "text-embedding-test");
            }
            AnyEmbeddingProvider::Candle(_) => panic!("expected the external arm"),
        }
    }

    #[test]
    fn degrade_candle_to_external_falls_back_when_external_base_present() {
        // S3: a candle runtime whose model is absent but which ALSO carries a usable external base
        // URL degrades to the external adapter rather than aborting the backfill.
        let runtime = external_runtime(AiRequestFormat::OpenAi, Some("http://localhost:1234/v1"));
        let provider = degrade_candle_to_external(&runtime, "repo").expect("degrade");
        assert!(matches!(provider, AnyEmbeddingProvider::External(_)));
    }

    #[test]
    fn degrade_candle_to_external_errors_clearly_when_no_external_fallback() {
        // S3: a pure candle runtime (only the candle sentinel base url) with the model absent gets a
        // clear, actionable "model not downloaded" error — NOT a silent abort.
        let runtime = candle_runtime("");
        let error = degrade_candle_to_external(&runtime, "Qwen/Qwen3-Embedding-0.6B-GGUF")
            .err()
            .expect("no external fallback should error");
        assert!(error.to_string().contains("is not downloaded"));
    }

    #[test]
    fn degrade_candle_to_external_errors_when_no_base_url() {
        // A candle runtime with NO base url at all also cannot degrade → clear error.
        let runtime = external_runtime(AiRequestFormat::OpenAi, None);
        assert!(degrade_candle_to_external(&runtime, "repo").is_err());
    }

    // ---- download path (consent / offline-first / cancel / verify / progress) ----

    /// A test manifest whose pinned digests match the download stub's synthetic bytes.
    fn stub_manifest() -> Vec<ModelFile> {
        // The digests must equal the SHA-256 of `stub_file_bytes(name)` so a stub download verifies.
        ["config.json", "tokenizer.json", "weights.gguf"]
            .into_iter()
            .map(|name| {
                let digest = crate::utils::sha256_hex(&stub_file_bytes(name));
                ModelFile { name, sha256: Box::leak(digest.into_boxed_str()), repo: "Stub/Repo" }
            })
            .collect()
    }

    #[derive(Default)]
    struct RecordingProgress {
        started: Vec<String>,
        finished: Vec<String>,
        cancel_after: Option<usize>,
    }

    impl ModelDownloadProgress for RecordingProgress {
        fn file_started(&mut self, file: &str, _total_bytes: u64) {
            self.started.push(file.to_string());
        }
        fn file_finished(&mut self, file: &str) {
            self.finished.push(file.to_string());
        }
        fn cancelled(&self) -> bool {
            self.cancel_after.is_some_and(|threshold| self.started.len() >= threshold)
        }
    }

    #[test]
    fn ensure_download_refuses_without_consent_when_absent() {
        let dir = tempfile::tempdir().expect("tempdir");
        let paths = crate::config::project_paths_with_root(dir.path());
        let manifest = stub_manifest();
        let mut progress = NoopDownloadProgress;
        let error = ensure_model_downloaded(&paths, "Stub/Repo", &manifest, false, &mut progress)
            .expect_err("no consent");
        assert!(error.to_string().contains("requires explicit consent"));
    }

    #[test]
    fn ensure_download_fetches_verifies_reports_progress_and_marks_loadable() {
        let dir = tempfile::tempdir().expect("tempdir");
        let paths = crate::config::project_paths_with_root(dir.path());
        let manifest = stub_manifest();
        let mut progress = RecordingProgress::default();
        let model_dir =
            ensure_model_downloaded(&paths, "Stub/Repo", &manifest, true, &mut progress)
                .expect("download");
        // Every manifest file is now present + verified, and the cheap loadable check passes (S5).
        assert!(model_is_present_and_verified(&model_dir, &manifest));
        assert!(model_is_loadable(&model_dir, &manifest));
        assert_eq!(progress.started.len(), 3);
        assert_eq!(progress.finished.len(), 3);
    }

    #[test]
    fn ensure_download_is_offline_first_when_already_verified() {
        let dir = tempfile::tempdir().expect("tempdir");
        let paths = crate::config::project_paths_with_root(dir.path());
        let manifest = stub_manifest();
        // First download places + verifies the files.
        let mut first = RecordingProgress::default();
        ensure_model_downloaded(&paths, "Stub/Repo", &manifest, true, &mut first).expect("first");
        // A second call WITHOUT consent succeeds (already present) and reports no new progress —
        // proving the offline-first reuse path runs without touching the fetch.
        let mut second = RecordingProgress::default();
        let model_dir = ensure_model_downloaded(&paths, "Stub/Repo", &manifest, false, &mut second)
            .expect("offline reuse");
        assert!(model_is_present_and_verified(&model_dir, &manifest));
        assert!(second.started.is_empty(), "no file should be re-fetched");
    }

    #[test]
    fn ensure_download_cancels_between_files_without_partial_install() {
        let dir = tempfile::tempdir().expect("tempdir");
        let paths = crate::config::project_paths_with_root(dir.path());
        let manifest = stub_manifest();
        // Cancel as soon as the first file has started: the loop must stop and report cancellation.
        let mut progress =
            RecordingProgress { cancel_after: Some(1), ..RecordingProgress::default() };
        let error = ensure_model_downloaded(&paths, "Stub/Repo", &manifest, true, &mut progress)
            .expect_err("cancelled");
        assert!(error.to_string().contains("cancelled"));
        // Not all files were installed (the model is not fully present), and it is not loadable.
        let model_dir = model_dir_for_repo(&paths, "Stub/Repo");
        assert!(!model_is_present_and_verified(&model_dir, &manifest));
        assert!(!model_is_loadable(&model_dir, &manifest));
    }

    #[test]
    fn ensure_download_skips_already_present_file_inside_the_loop() {
        // Partial install: one manifest file is already on disk + valid, the rest are missing. The
        // download loop must SKIP the present file (offline-first per-file reuse) and fetch only the
        // rest — exercising the in-loop `continue` branch (not the top-level all-present shortcut).
        let dir = tempfile::tempdir().expect("tempdir");
        let paths = crate::config::project_paths_with_root(dir.path());
        let manifest = stub_manifest();
        let model_dir = model_dir_for_repo(&paths, "Stub/Repo");
        std::fs::create_dir_all(&model_dir).expect("mkdir");
        // Pre-place the FIRST file with its correct stub bytes so it is already verified.
        let present = &manifest[0];
        std::fs::write(model_dir.join(present.name), stub_file_bytes(present.name)).expect("seed");

        let mut progress = RecordingProgress::default();
        ensure_model_downloaded(&paths, "Stub/Repo", &manifest, true, &mut progress)
            .expect("download remainder");
        assert!(model_is_present_and_verified(&model_dir, &manifest));
        // The already-present file was skipped (not re-fetched); only the other two were fetched.
        assert!(!progress.started.contains(&present.name.to_string()));
        assert_eq!(progress.started.len(), manifest.len() - 1);
    }

    #[test]
    fn ensure_download_rejects_digest_mismatch_and_removes_blob() {
        let dir = tempfile::tempdir().expect("tempdir");
        let paths = crate::config::project_paths_with_root(dir.path());
        // A manifest whose digest does NOT match the stub bytes → verification fails, blob removed.
        let wrong: &'static str = Box::leak("a".repeat(64).into_boxed_str());
        let manifest = vec![ModelFile { name: "config.json", sha256: wrong, repo: "Stub/Repo" }];
        let mut progress = NoopDownloadProgress;
        let error = ensure_model_downloaded(&paths, "Stub/Repo", &manifest, true, &mut progress)
            .expect_err("digest mismatch");
        assert!(error.to_string().contains("failed SHA-256 verification"));
        let model_dir = model_dir_for_repo(&paths, "Stub/Repo");
        assert!(!model_dir.join("config.json").exists(), "bad blob must be removed");
    }

    #[test]
    fn ensure_download_marks_loadable_for_sideloaded_model() {
        // A model whose files are manually placed (sideloaded) and verify offline gets the verified
        // marker on the first offline-first call, so it becomes loadable without a download (S5).
        let dir = tempfile::tempdir().expect("tempdir");
        let paths = crate::config::project_paths_with_root(dir.path());
        let manifest = stub_manifest();
        let model_dir = model_dir_for_repo(&paths, "Stub/Repo");
        std::fs::create_dir_all(&model_dir).expect("mkdir");
        for file in &manifest {
            std::fs::write(model_dir.join(file.name), stub_file_bytes(file.name)).expect("seed");
        }
        assert!(!model_is_loadable(&model_dir, &manifest), "no marker yet");
        // Offline-first call (no consent needed) writes the marker.
        let mut progress = NoopDownloadProgress;
        ensure_model_downloaded(&paths, "Stub/Repo", &manifest, false, &mut progress)
            .expect("sideloaded offline");
        assert!(model_is_loadable(&model_dir, &manifest));
    }

    #[test]
    fn download_needed_is_false_only_for_present_and_matching_file() {
        let dir = tempfile::tempdir().expect("tempdir");
        let model_dir = dir.path();
        let digest = crate::utils::sha256_hex(b"payload");
        let file =
            ModelFile { name: "f.bin", sha256: Box::leak(digest.into_boxed_str()), repo: "r" };
        // Missing → needed.
        assert!(download_needed(model_dir, &file));
        std::fs::write(model_dir.join("f.bin"), b"payload").expect("write");
        // Present + matching → not needed.
        assert!(!download_needed(model_dir, &file));
        std::fs::write(model_dir.join("f.bin"), b"corrupt").expect("rewrite");
        // Present but wrong digest → needed.
        assert!(download_needed(model_dir, &file));
    }

    #[test]
    fn noop_progress_defaults_are_inert() {
        let mut progress = NoopDownloadProgress;
        progress.file_started("x", 10);
        progress.file_finished("x");
        assert!(!progress.cancelled());
    }
}
