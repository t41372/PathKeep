//! In-app HAND-ROLLED model2vec / static embedding engine (Tier 0 fast base, W-AI-4c).
//!
//! ## Responsibilities
//! - own the ONE place a static (model2vec-style) embedding is computed: tokenize → look up each
//!   token's row in a static embedding matrix → (optional per-token weight) mean-pool → optional
//!   L2-normalize. No transformer forward, no KV cache — O(sequence length) per text.
//! - implement the rig-free [`EmbeddingProvider`] boundary against that engine, honoring the
//!   02 §C.3 correctness 鐵律: read the ACTUAL pooled length as `effective_dim`, set the descriptor
//!   dtype/normalized/pooling to THIS adapter's reality (Float32 / `config.normalize` / Mean), and
//!   thread [`EmbeddingRole`] through (static models are symmetric — query == document — so the role
//!   is a DOCUMENTED no-op here, mirrored in the descriptor's empty instruction template).
//! - load the model files (safetensors embedding matrix + tokenizer.json + config.json) via OUR OWN
//!   code (reusing the in-tree `tokenizers` + candle's bundled `safetensors`), download on demand via
//!   the SHARED W-AI-4b consent-gated, SHA-256-verified [`super::embedding_candle::ensure_model_downloaded`].
//!
//! ## Not responsible for
//! - provider config/secret resolution (the worker's `AiProviderRuntime` carries those)
//! - vector persistence, the embed loop, dedup, or queue bookkeeping (engine-agnostic, elsewhere)
//! - the heavy Qwen3 tier (candle) or chat/LLM inference
//!
//! ## Supply-chain decision (W-AI-4c): HAND-ROLLED, no new crate
//! `model2vec-rs` is MIT on GitHub but ships a "Non-standard" license field on crates.io (would trip
//! `cargo deny check licenses`, whose allowlist is SPDX-only) and is a single-org crate at ~193 stars
//! — an order of magnitude under the >6k supply-chain gate. model2vec inference is, by design,
//! trivial (table lookup + mean-pool + normalize: the PCA/zipf/SIF regularization is BAKED INTO the
//! distilled matrix at distillation time, NOT re-applied at inference). So we hand-roll it from the
//! crates already in-tree from the candle path (`tokenizers`, candle's `safetensors`, `hf-hub`),
//! adding ZERO new dependencies and keeping `cargo deny`/`audit` green. The pooling/lookup/normalize
//! math is PURE → un-gated + unit-tested against a tiny synthetic matrix (far more testable than the
//! candle forward); only the real safetensors/tokenizer load + download are `cfg`-gated.

use super::embedding_candle::ModelFile;
#[cfg(not(any(test, coverage)))]
use super::embedding_candle::model_dir_for_repo;
use super::embedding_external::{AnyEmbeddingProvider, ExternalEmbeddingProvider};
use super::traits::{
    EmbeddingDescriptor, EmbeddingDtype, EmbeddingPooling, EmbeddingProvider, EmbeddingRole,
};
use crate::AiProviderRuntime;
use crate::config::ProjectPaths;
use anyhow::{Context, Result};
use std::sync::Arc;

// ---------------------------------------------------------------------------
// Model manifest (always compiled — pure data; load/download live in candle's shared helpers).
// ---------------------------------------------------------------------------

/// The default static model's Hugging Face repository id.
///
/// `minishlab/potion-multilingual-128M` is a model2vec STATIC multilingual model (256-dim, 101
/// languages, distilled from bge-m3) — the Tier-0 base from 05 §2. This drives a download path, NOT
/// any dim/pooling assumption (those are read at runtime, D4). A future swap changes this one
/// constant plus the pinned hashes; nothing else in the engine assumes this specific model.
pub const DEFAULT_STATIC_MODEL_REPO: &str = "minishlab/potion-multilingual-128M";

/// Reserved `base_url` sentinel marking an embedding provider as the in-app STATIC engine.
///
/// Selecting static vs candle vs external is config-driven by `base_url` with no schema change:
/// this sentinel routes to the static engine, [`super::embedding_candle::CANDLE_INAPP_BASE_URL`]
/// routes to candle, any other URL routes to the external `/v1/embeddings` adapter. `default_model`
/// still carries the repo id (D4), so a future static model is a config change, not a code change.
pub const STATIC_INAPP_BASE_URL: &str = "static:in-app";

/// The files the static engine needs, with their pinned SHA-256 digests.
///
/// model2vec ships everything in ONE repo (unlike the candle GGUF/tokenizer split): the
/// `model.safetensors` embedding matrix, the `tokenizer.json` (a Unigram tokenizer derived from
/// bge-m3, unk token `[UNK]` id 1), and `config.json` (carries `normalize` + `hidden_dim`). All three
/// come from [`DEFAULT_STATIC_MODEL_REPO`].
///
/// SHA-256 BOUNDARY NOTE (F2): these are the REAL recomputed hashes of the files downloaded on
/// 2026-06-21 from `minishlab/potion-multilingual-128M` (HF `main`), captured via the env-gated e2e
/// download (config.json `595e4cab…`, tokenizer.json `19f19090…`, model.safetensors `14b5eb39…`). A
/// fat-fingered or substituted file is caught by [`super::embedding_candle::verify_file_sha256`]
/// failing rather than loading a wrong matrix; the manifest test pins the 64-hex shape.
pub const DEFAULT_STATIC_MODEL_FILES: &[ModelFile] = &[
    ModelFile {
        name: "config.json",
        sha256: "595e4cab2093732efd5dbe084fd5c1826b5eea693b73b4c1fd971672867d2e54",
        repo: DEFAULT_STATIC_MODEL_REPO,
    },
    ModelFile {
        name: "tokenizer.json",
        sha256: "19f1909063da3cfe3bd83a782381f040dccea475f4816de11116444a73e1b6a1",
        repo: DEFAULT_STATIC_MODEL_REPO,
    },
    ModelFile {
        name: "model.safetensors",
        sha256: "14b5eb39cb4ce5666da8ad1f3dc6be4346e9b2d601c073302fa0a31bf7943397",
        repo: DEFAULT_STATIC_MODEL_REPO,
    },
];

/// Max input tokens the static engine keeps before truncation.
///
/// DIVERGENCE FROM model2vec (documented, finding S7): model2vec's `StaticModel.encode` truncates at
/// a DEFAULT `max_length=512` tokens (NOT unbounded — verified against model2vec 0.8.2 source). This
/// engine uses a larger 2048-token cap purely as a DoS guard: it bounds the mean-pool work per text so
/// a pathological multi-megabyte input cannot stall the 14.4M backfill. For any input UNDER 512 tokens
/// the two truncations coincide and the pooling is identical to the reference (mean-pool is
/// order-independent), so the parity fixture inputs are kept well under 512 tokens and compare exactly
/// (cosine 1.0). The 512–2048 band is the only divergence (our mean spans more tokens than the
/// reference's), which the corpus's short URL/title/summary rows never reach in practice; a future
/// alignment could lower this to 512 to match the reference at the cost of the wider DoS headroom.
/// This is an inference-side input bound, NOT a model/distillation assumption.
pub const STATIC_MAX_INPUT_TOKENS: usize = 2048;

// ---------------------------------------------------------------------------
// Routing + selection (pure decision logic — config-driven, mirrors the candle selector).
// ---------------------------------------------------------------------------

/// Whether a resolved provider runtime is configured to use the in-app static engine.
///
/// Pure (reads only the config) so the routing decision is unit-tested without loading a matrix.
pub fn runtime_uses_static(runtime: &AiProviderRuntime) -> bool {
    runtime.config.base_url.as_deref() == Some(STATIC_INAPP_BASE_URL)
}

/// Resolves the static model repo id for a static runtime (config `default_model`, else the default).
///
/// Pure so the model-id resolution is unit-tested. An empty `default_model` falls back to
/// [`DEFAULT_STATIC_MODEL_REPO`] (D4: the default is a constant, never baked into the selector); a
/// configured value wins so a future static model is a config change, not a code change.
pub fn static_repo_for_runtime(runtime: &AiProviderRuntime) -> &str {
    if runtime.config.default_model.is_empty() {
        DEFAULT_STATIC_MODEL_REPO
    } else {
        runtime.config.default_model.as_str()
    }
}

/// Degrades a static runtime whose model is absent to the external adapter, or a clear error (S3).
///
/// Pure decision logic (no matrix load, no network), mirroring the candle degradation contract: if
/// the runtime carries a usable external base URL (NOT the static sentinel), build the external
/// provider so the backfill proceeds against `/v1/embeddings`; otherwise return an actionable "model
/// not downloaded" error the caller turns into a download prompt. The backfill is NEVER hard-failed
/// merely because the static model has not been fetched yet (AGENTS.md principle 4).
pub fn degrade_static_to_external(
    runtime: &AiProviderRuntime,
    repo: &str,
) -> Result<AnyEmbeddingProvider> {
    let has_external_base =
        runtime.config.base_url.as_deref().is_some_and(|base| base != STATIC_INAPP_BASE_URL);
    if has_external_base {
        return Ok(AnyEmbeddingProvider::External(ExternalEmbeddingProvider::new(
            runtime.clone(),
        )?));
    }
    anyhow::bail!(
        "in-app static model {repo} is not downloaded; fetch it (with consent) before embedding, or configure an external embedding provider"
    )
}

/// Selects the static engine for a static-routed runtime (config-driven, W-AI-4c).
///
/// Routing + DEGRADATION contract (mirrors the candle selector's S3): selects the static engine ONLY
/// when the model is present + verified on disk (the consent gate); otherwise degrades to the
/// external adapter when one is configured, else returns a clear "not downloaded" error. Called only
/// after [`runtime_uses_static`] returns true.
#[cfg(not(any(test, coverage)))]
pub fn select_static_embedding_provider(
    paths: &ProjectPaths,
    runtime: &AiProviderRuntime,
) -> Result<AnyEmbeddingProvider> {
    let repo = static_repo_for_runtime(runtime);
    let model_dir = model_dir_for_repo(paths, repo);
    if super::embedding_candle::model_is_present_and_verified(
        &model_dir,
        DEFAULT_STATIC_MODEL_FILES,
    ) {
        let provider = StaticEmbeddingProvider::load(paths, repo, DEFAULT_STATIC_MODEL_FILES)?;
        return Ok(AnyEmbeddingProvider::Static(Box::new(provider)));
    }
    degrade_static_to_external(runtime, repo)
}

/// Test/coverage selector: builds the deterministic static stub (treated as "present").
///
/// Same routing decision as the real selector so the backfill exercises the static arm at 100%
/// coverage; the degrade arms are exercised via [`degrade_static_to_external`] directly in tests.
#[cfg(any(test, coverage))]
pub fn select_static_embedding_provider(
    _paths: &ProjectPaths,
    runtime: &AiProviderRuntime,
) -> Result<AnyEmbeddingProvider> {
    let repo = static_repo_for_runtime(runtime);
    Ok(AnyEmbeddingProvider::Static(Box::new(StaticEmbeddingProvider::new_stub(
        &format!("static:{repo}"),
        repo,
    ))))
}

// ---------------------------------------------------------------------------
// The static embedding matrix + config (pure data — testable with a tiny synthetic table).
// ---------------------------------------------------------------------------

/// The subset of model2vec `config.json` the static engine reads.
///
/// Deserialized from the model's own `config.json` (D4: read, never assumed). `normalize` decides
/// whether the pooled vector is L2-normalized (model2vec default `true`); `hidden_dim` is the
/// declared output width used only to CROSS-CHECK the loaded matrix's real column count (the
/// descriptor still records the ACTUAL pooled length). Unknown fields (`apply_pca`, `apply_zipf`,
/// `sif_coefficient`, …) are ignored: they describe DISTILLATION-time regularization already baked
/// into the matrix, not an inference-time step.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct StaticModelConfig {
    /// Declared embedding dimension (matrix column count); cross-checked against the loaded matrix.
    pub hidden_dim: usize,
    /// Whether to L2-normalize the pooled vector. model2vec defaults to `true` when absent.
    #[serde(default = "default_normalize")]
    pub normalize: bool,
}

/// model2vec's default for `normalize` when the field is absent from `config.json`.
fn default_normalize() -> bool {
    true
}

/// Parses the model's `config.json` bytes into the engine's config view.
///
/// Pure (no I/O) so a malformed/short config is caught by a unit test rather than at model load. A
/// missing `hidden_dim` (the one field the cross-check needs) is a hard error; a missing `normalize`
/// defaults to model2vec's `true`.
pub fn parse_static_config(config_json: &[u8]) -> Result<StaticModelConfig> {
    serde_json::from_slice(config_json).context("parsing static model config.json")
}

/// The static token→vector matrix: `vocab_size` rows of `dim` columns, plus optional per-token weights.
///
/// This is the pure, in-memory shape the pooling operates on — equally constructible from a real
/// safetensors load or a tiny synthetic table in a unit test. `weights`, when present, is one scalar
/// per row applied before mean-pooling (model2vec's optional pre-baked per-token weighting; POTION
/// folds SIF into the matrix and ships NO weights, so this is `None` for the default model).
/// `unk_token_id`, when present, is the unk row the reference DROPS before pooling (see
/// [`Self::mean_pool`]); set ONLY for tokenizers model2vec drops unk for — i.e. BPE/WordPiece with a
/// string `unk_token`. `None` (the default `potion-multilingual-128M` Unigram case, and synthetic
/// test matrices) means no unk filtering, matching model2vec pooling everything for those models.
#[derive(Debug, Clone, PartialEq)]
pub struct StaticEmbeddingMatrix {
    /// Flat row-major matrix: row `i` occupies `rows[i*dim .. (i+1)*dim]`.
    rows: Vec<f32>,
    /// Number of vocabulary rows (`rows.len() / dim`).
    vocab_size: usize,
    /// Per-token embedding width (the column count).
    dim: usize,
    /// Optional per-token scalar weight (`vocab_size` entries) applied before mean-pooling.
    weights: Option<Vec<f32>>,
    /// The tokenizer's `<unk>` token id, excluded from pooling to match the reference. `None` when
    /// the source had no unk token (e.g. a synthetic test matrix).
    unk_token_id: Option<u32>,
}

impl StaticEmbeddingMatrix {
    /// Builds a matrix from a flat row-major buffer, validating the shape (no unk filtering).
    ///
    /// Pure constructor so the lookup/pool math is unit-tested from a synthetic table. Errors when
    /// `dim == 0`, the buffer is not a whole multiple of `dim`, or `weights` (when present) does not
    /// have exactly one entry per row — any of which would desync the lookup and silently corrupt
    /// every vector. Use [`Self::with_unk_token`] to attach the reference's `<unk>`-drop behavior.
    pub fn new(rows: Vec<f32>, dim: usize, weights: Option<Vec<f32>>) -> Result<Self> {
        if dim == 0 {
            anyhow::bail!("static embedding matrix dim must be non-zero");
        }
        if rows.len() % dim != 0 {
            anyhow::bail!(
                "static embedding matrix buffer length {} is not a multiple of dim {dim}",
                rows.len()
            );
        }
        let vocab_size = rows.len() / dim;
        if vocab_size == 0 {
            anyhow::bail!("static embedding matrix has zero rows");
        }
        if let Some(weights) = &weights {
            if weights.len() != vocab_size {
                anyhow::bail!(
                    "static embedding weights length {} does not match vocab size {vocab_size}",
                    weights.len()
                );
            }
        }
        Ok(Self { rows, vocab_size, dim, weights, unk_token_id: None })
    }

    /// Returns this matrix with its unk token id set, so [`Self::mean_pool`] drops that token.
    ///
    /// model2vec's reference (Python `model.py` `tokenize`: `[id for id in ids if id != unk_id]` when
    /// `unk_token_id is not None`; model2vec-rs `encode_with_args` likewise) EXCLUDES the unk token
    /// from pooling — but ONLY for tokenizers that declare a string `unk_token` (BPE/WordPiece), where
    /// keeping it would pull every OOV input toward one row. The default `potion-multilingual-128M`
    /// ships a Unigram tokenizer with NO string `unk_token`, so model2vec leaves it `None` and pools
    /// `[UNK]` — so this is `None` there too (no dropping), keeping the engine bit-for-bit with the
    /// reference. Resolved from the tokenizer.json at load (see [`resolve_unk_to_drop`]).
    pub fn with_unk_token(mut self, unk_token_id: Option<u32>) -> Self {
        self.unk_token_id = unk_token_id;
        self
    }

    /// Returns the per-token embedding width (the pooled vector's length).
    pub fn dim(&self) -> usize {
        self.dim
    }

    /// Returns the number of vocabulary rows.
    pub fn vocab_size(&self) -> usize {
        self.vocab_size
    }

    /// Mean-pools the rows for one token-id sequence into a single `dim`-length vector.
    ///
    /// The model2vec inference 鐵律, replicated exactly (verified against the reference + model2vec-rs):
    /// the `<unk>` token is DROPPED first (model2vec-rs `token_ids.retain(|id| id != unk_id)` /
    /// Python `model.py` `tokenize`), then for each remaining in-vocab token id accumulate its row
    /// scaled by its per-token weight (1.0 when the model ships no weights), then divide the
    /// accumulated sum by the count of pooled tokens — an UNWEIGHTED arithmetic mean of the (weighted)
    /// token rows. Out-of-range ids are skipped (the tokenizer can emit ids the distilled matrix does
    /// not cover); an empty-after-filter / all-out-of-range / empty sequence yields a zero vector (the
    /// only honest answer with no tokens to pool). Pooling all `<unk>` tokens (instead of dropping
    /// them) is exactly the bug that scrambled OOV embeddings before this fix. PURE → the math is
    /// unit-tested and mutation-hardened from a synthetic table.
    pub fn mean_pool(&self, ids: &[u32]) -> Vec<f32> {
        let mut sum = vec![0.0_f32; self.dim];
        let mut pooled: usize = 0;
        for &id in ids {
            if Some(id) == self.unk_token_id {
                continue; // The reference drops <unk> before pooling — exclude from sum AND count.
            }
            let index = id as usize;
            if index >= self.vocab_size {
                continue; // Tokenizer id outside the distilled matrix — skip, do not panic.
            }
            let scale = self.weights.as_ref().map_or(1.0, |weights| weights[index]);
            let row = &self.rows[index * self.dim..(index + 1) * self.dim];
            for (accumulator, &component) in sum.iter_mut().zip(row.iter()) {
                *accumulator += component * scale;
            }
            pooled += 1;
        }
        if pooled > 0 {
            let denominator = pooled as f32;
            for component in &mut sum {
                *component /= denominator;
            }
        }
        sum
    }
}

/// L2-normalizes a vector in place using model2vec's epsilon-floored norm (matches the reference).
///
/// model2vec divides by `‖v‖ + 1e-32` (its exact epsilon) so a zero vector stays zero rather than
/// producing NaNs, and a near-zero vector does not blow up. Kept as a dedicated helper (rather than
/// reusing the candle/provider `l2_normalize`, which uses a different epsilon policy) so the static
/// output matches the model2vec reference bit-for-bit at the parity boundary. PURE → unit-tested.
pub fn static_l2_normalize(vector: &mut [f32]) {
    let norm = vector.iter().map(|&component| component * component).sum::<f32>().sqrt() + 1e-32;
    for component in vector {
        *component /= norm;
    }
}

/// Computes one static embedding from a token-id sequence: mean-pool then optional L2-normalize.
///
/// The whole static inference for one text, PURE so it is fully unit-tested end-to-end from a
/// synthetic matrix without a tokenizer or weights file. `normalize` comes from the model's
/// `config.json` (model2vec applies L2 iff `config.normalize`); when off, the bare mean-pooled
/// vector is returned (still a valid embedding, just not unit-norm).
pub fn static_embed_ids(matrix: &StaticEmbeddingMatrix, ids: &[u32], normalize: bool) -> Vec<f32> {
    let mut vector = matrix.mean_pool(ids);
    if normalize {
        static_l2_normalize(&mut vector);
    }
    vector
}

// ---------------------------------------------------------------------------
// safetensors matrix load (real build only — the pure math above does not need it).
// ---------------------------------------------------------------------------

/// The safetensors tensor name model2vec stores the embedding matrix under, with fallbacks.
///
/// model2vec writes the matrix as `embeddings`; older / sentence-transformers exports use `0` or
/// `embedding.weight`. The loader tries them in order (matching model2vec-rs) so any model2vec
/// export loads without a per-model config knob.
#[cfg(not(any(test, coverage)))]
const STATIC_EMBEDDING_TENSOR_NAMES: &[&str] = &["embeddings", "0", "embedding.weight"];

/// safetensors tensor names model2vec uses for a vocab-quantized token→row remapping table.
///
/// A vocab-quantized model2vec export indexes rows through this `[vocab]` map (token id → row index)
/// instead of by raw token id. The default `potion-multilingual-128M` f32 export has NO such tensor,
/// so the loader HARD-ERRORS if one is present rather than silently scrambling every vector by
/// indexing the wrong rows — see [`load_static_matrix`]. (model2vec-rs honors the map; we reject it
/// because the supported default never carries one, keeping the hand-roll simple and correct.)
#[cfg(not(any(test, coverage)))]
const STATIC_TOKEN_MAPPING_TENSOR_NAMES: &[&str] = &["mapping", "token_mapping"];

/// Loads the static embedding matrix (+ optional weights) from a `model.safetensors` on disk.
///
/// Reads the tensor named one of [`STATIC_EMBEDDING_TENSOR_NAMES`] as `[vocab, dim]` f32, plus an
/// optional `weights` tensor (`[vocab]`). Offline (no network): reads only the file on disk. Gated
/// to the real build because it needs candle's safetensors loader; the PURE pooling above is what
/// the tests exercise. HARD-ERRORS on a `mapping`/`token_mapping` tensor (a vocab-quantized export):
/// the hand-roll indexes rows by raw token id, so honoring such a map would require row remapping the
/// supported f32 default never needs — failing loudly beats silently scrambling vectors.
#[cfg(not(any(test, coverage)))]
fn load_static_matrix(
    safetensors_path: &std::path::Path,
    unk_token_id: Option<u32>,
) -> Result<StaticEmbeddingMatrix> {
    use candle_core::Device;
    let tensors = candle_core::safetensors::load(safetensors_path, &Device::Cpu)
        .with_context(|| format!("loading static safetensors {}", safetensors_path.display()))?;
    if let Some(mapping_name) =
        STATIC_TOKEN_MAPPING_TENSOR_NAMES.iter().find(|name| tensors.contains_key(**name))
    {
        anyhow::bail!(
            "static safetensors {} carries a `{mapping_name}` token-remapping tensor (a vocab-quantized model2vec export), which is unsupported by the hand-rolled engine — use the f32 export (e.g. {DEFAULT_STATIC_MODEL_REPO})",
            safetensors_path.display()
        );
    }
    let matrix_tensor = STATIC_EMBEDDING_TENSOR_NAMES
        .iter()
        .find_map(|name| tensors.get(*name))
        .with_context(|| {
            format!(
                "static safetensors {} has no embedding tensor (looked for {STATIC_EMBEDDING_TENSOR_NAMES:?})",
                safetensors_path.display()
            )
        })?;
    let (vocab_size, dim) = matrix_tensor
        .dims2()
        .context("static embedding tensor is not a 2-D [vocab, dim] matrix")?;
    let rows = matrix_tensor
        .to_dtype(candle_core::DType::F32)
        .context("casting static embedding matrix to f32")?
        .flatten_all()
        .context("flattening static embedding matrix")?
        .to_vec1::<f32>()
        .context("reading static embedding matrix rows")?;
    let weights = match tensors.get("weights") {
        Some(weights_tensor) => Some(
            weights_tensor
                .to_dtype(candle_core::DType::F32)
                .context("casting static weights to f32")?
                .flatten_all()
                .context("flattening static weights")?
                .to_vec1::<f32>()
                .context("reading static weights")?,
        ),
        None => None,
    };
    let matrix = StaticEmbeddingMatrix::new(rows, dim, weights)?.with_unk_token(unk_token_id);
    debug_assert_eq!(matrix.vocab_size(), vocab_size);
    Ok(matrix)
}

/// Resolves the unk token id pooling should DROP, matching the model2vec reference exactly.
///
/// CRITICAL — mirror model2vec's `StaticModel.__init__`: it drops the unk token ONLY when the
/// tokenizer's model exposes a STRING `unk_token` (BPE / WordPiece), and otherwise pools everything.
/// The default `potion-multilingual-128M` ships a **Unigram** tokenizer that declares `unk_id` (an
/// index, content `[UNK]`) but NO string `unk_token`, so model2vec leaves `unk_token_id = None` and
/// pools `[UNK]` like any other token — and so must this engine, or it would DIVERGE from the
/// reference on OOV input (proven: dropping Unigram `[UNK]` collapses parity to ~0.80 cosine on the
/// OOV fixture rows). There is deliberately NO hardcoded string fallback — a hardcoded
/// `token_to_id("[UNK]")` would wrongly drop the Unigram unk and break parity. `None` → no filtering.
#[cfg(not(any(test, coverage)))]
fn resolve_unk_token_id(tokenizer: &tokenizers::Tokenizer, tokenizer_json: &[u8]) -> Option<u32> {
    resolve_unk_to_drop(tokenizer_json, tokenizer)
}

/// Resolves the unk token id from raw `tokenizer.json` bytes, EXACTLY mirroring model2vec's logic.
///
/// CRITICAL — match the reference's unk handling bit-for-bit (proven by the parity gate). model2vec's
/// `StaticModel.__init__` sets `unk_token_id` ONLY from `tokenizer.model.unk_token` (a STRING
/// attribute present on BPE / WordPiece models, resolved through the vocab); for a **Unigram** model
/// (which the default `potion-multilingual-128M` ships) that attribute is ABSENT, so model2vec leaves
/// `unk_token_id = None` and pools ALL tokens INCLUDING `[UNK]`. Mirroring that, this returns `None`
/// for a Unigram export (no dropping → matches the reference) and the resolved id for a BPE/WordPiece
/// export that declares an `unk_token` string (dropping → matches the reference, which DOES drop unk
/// for those models). See [`unk_spec_from_tokenizer_json`] for the PURE parse.
#[cfg(not(any(test, coverage)))]
fn resolve_unk_to_drop(
    tokenizer_json: &[u8],
    for_id_lookup: &tokenizers::Tokenizer,
) -> Option<u32> {
    match unk_spec_from_tokenizer_json(tokenizer_json)? {
        // BPE / WordPiece declare a string `unk_token`; model2vec drops it, so we resolve + drop it.
        UnkSpec::Token(token) => for_id_lookup.token_to_id(&token),
    }
}

/// The unk declaration extracted from a `tokenizer.json`'s `model` block (PURE, always compiled).
///
/// Only the BPE/WordPiece string form is a DROP signal: model2vec resolves its `unk_token_id` solely
/// from a string `unk_token` attribute. A Unigram model's `unk_id` (an index) is intentionally NOT a
/// drop signal — model2vec ignores it, so we must too, or we would diverge from the reference on OOV
/// input (proven: dropping Unigram's `[UNK]` drops parity to ~0.80 cosine on OOV rows).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum UnkSpec {
    /// A token-content string (BPE/WordPiece `unk_token`) to be resolved against the vocab + dropped.
    Token(String),
}

/// Parses the unk declaration from raw `tokenizer.json` bytes (PURE → unit-tested, always compiled).
///
/// Returns `Some(UnkSpec::Token(..))` ONLY for a model that declares a STRING `unk_token` (BPE /
/// WordPiece) — the exact and only condition under which model2vec drops the unk token. A Unigram
/// model declares `unk_id` (an index) but NO string `unk_token`, so this returns `None` for it,
/// matching model2vec's `unk_token_id = None` (pool everything, the parity-correct behavior). Also
/// `None` for malformed JSON or a missing `model` block.
pub(super) fn unk_spec_from_tokenizer_json(tokenizer_json: &[u8]) -> Option<UnkSpec> {
    let parsed: serde_json::Value = serde_json::from_slice(tokenizer_json).ok()?;
    let model = parsed.get("model")?;
    // ONLY a string `unk_token` (BPE / WordPiece) is a drop signal — mirror model2vec exactly. A
    // Unigram `unk_id` index is deliberately ignored (model2vec ignores it → so do we).
    let unk_token = model.get("unk_token").and_then(serde_json::Value::as_str)?;
    Some(UnkSpec::Token(unk_token.to_string()))
}

// ---------------------------------------------------------------------------
// The provider.
// ---------------------------------------------------------------------------

/// In-app static (model2vec) embedding provider implementing [`EmbeddingProvider`].
///
/// Holds the loaded matrix + tokenizer + normalize flag (real build) or the deterministic descriptor
/// (test/coverage build). No mutex is needed: pooling is read-only over an immutable matrix (unlike
/// the candle KV cache), so forwards are naturally concurrent. The real engine is gated behind
/// `cfg(not(any(test, coverage)))`; the stub returns deterministic, normalized vectors with the same
/// public call graph so the `Static` enum arm and the embed loop are exercised at 100% coverage
/// without a matrix.
pub struct StaticEmbeddingProvider {
    /// Provider identity recorded in the descriptor/fingerprint.
    provider_id: String,
    /// Model identifier (the repo id) recorded in the descriptor/fingerprint.
    model_id: String,
    /// The native matrix dim (the pooled vector length).
    dim: usize,
    /// Whether the engine L2-normalizes (from `config.normalize`), recorded in the descriptor.
    normalize: bool,
    /// The real engine state (matrix + tokenizer).
    ///
    /// F4: ALWAYS compiled (no longer `cfg`-gated). The test/coverage build no longer swaps the real
    /// tokenize→pool→normalize compute for a digest stub — it constructs a TINY in-memory engine via
    /// [`StaticEmbeddingProvider::new_stub`] so the REAL algorithm runs under the coverage gate. Only
    /// the disk model LOAD (safetensors/tokenizer file reads) stays behind a `cfg` seam.
    engine: Arc<StaticEngine>,
}

/// The loaded static engine: the immutable embedding matrix + tokenizer.
///
/// Both are immutable after load and shared behind an `Arc`, so embedding is lock-free and
/// concurrent across the worker pool (the static tier's throughput advantage over the candle path,
/// whose mutable KV cache forces a mutex). F4: always compiled — the REAL `embed` compute
/// (tokenize → matrix lookup → mean-pool → optional L2-norm) runs in every build, including coverage.
struct StaticEngine {
    matrix: StaticEmbeddingMatrix,
    tokenizer: tokenizers::Tokenizer,
    normalize: bool,
}

impl StaticEmbeddingProvider {
    /// Loads the default static engine from `models_dir`, requiring a verified model on disk.
    ///
    /// Consent-gated by CONSTRUCTION: the caller only reaches here after the model is present +
    /// verified (the selector checks the availability gate, itself gated on a consented download).
    /// Loading is offline (no network): it reads only the files on disk.
    #[cfg(not(any(test, coverage)))]
    pub fn load_default(paths: &ProjectPaths) -> Result<Self> {
        Self::load(paths, DEFAULT_STATIC_MODEL_REPO, DEFAULT_STATIC_MODEL_FILES)
    }

    /// Loads a static engine for one repo from `models_dir`, verifying readiness first (cheap S5
    /// presence + verified-marker check, NOT a full re-hash on every load).
    #[cfg(not(any(test, coverage)))]
    pub fn load(paths: &ProjectPaths, repo: &str, files: &[ModelFile]) -> Result<Self> {
        let model_dir = model_dir_for_repo(paths, repo);
        if !super::embedding_candle::model_is_loadable(&model_dir, files) {
            anyhow::bail!(
                "static model {repo} is not present or not verified in {}; download it (with consent) before selecting the static engine",
                model_dir.display()
            );
        }
        let config_bytes = std::fs::read(model_dir.join("config.json"))
            .with_context(|| format!("reading config.json for {repo}"))?;
        let config = parse_static_config(&config_bytes)?;
        // Load the tokenizer FIRST so its unk id can be threaded into the matrix: the reference drops
        // the unk token before pooling, so the matrix needs the id to exclude that row (S1 fix). The
        // unk id is read from the tokenizer.json bytes (Unigram `unk_id` / BPE `unk_token`), NOT a
        // hardcoded string — potion's Unigram unk is `[UNK]` (id 1), with no `<unk>` token at all.
        let tokenizer_path = model_dir.join("tokenizer.json");
        let tokenizer_json = std::fs::read(&tokenizer_path)
            .with_context(|| format!("reading tokenizer.json for {repo}"))?;
        let tokenizer = tokenizers::Tokenizer::from_bytes(&tokenizer_json)
            .map_err(|error| anyhow::anyhow!("loading tokenizer for {repo}: {error}"))?;
        let unk_token_id = resolve_unk_token_id(&tokenizer, &tokenizer_json);
        let matrix = load_static_matrix(&model_dir.join("model.safetensors"), unk_token_id)?;
        // Cross-check the declared dim against the loaded matrix so a mismatched config/matrix bundle
        // fails LOUDLY at load rather than producing wrong-dim vectors (D4: read, then verify).
        if config.hidden_dim != matrix.dim() {
            anyhow::bail!(
                "static model {repo} config hidden_dim {} disagrees with matrix dim {}; the bundle is inconsistent",
                config.hidden_dim,
                matrix.dim()
            );
        }
        Ok(Self::from_engine(
            &format!("static:{repo}"),
            repo,
            StaticEngine { matrix, tokenizer, normalize: config.normalize },
        ))
    }

    /// Assembles a provider from an already-built engine (matrix + tokenizer + normalize flag).
    ///
    /// F4 seam: the ONE always-compiled constructor both the disk [`load`](Self::load) path and the
    /// in-memory [`new_stub`](Self::new_stub) / gate-test paths funnel through, so the descriptor
    /// (dim/normalize) is derived from the SAME engine the real [`embed`] runs against. The dim is the
    /// matrix's ACTUAL column count (D4: read, never assumed); `normalize` mirrors the engine's flag.
    fn from_engine(provider_id: &str, model_id: &str, engine: StaticEngine) -> Self {
        Self {
            provider_id: provider_id.to_string(),
            model_id: model_id.to_string(),
            dim: engine.matrix.dim(),
            normalize: engine.normalize,
            engine: Arc::new(engine),
        }
    }

    /// Builds a deterministic stub provider backed by a TINY REAL in-memory engine (tests/coverage).
    ///
    /// F4 (test-methodology fix): this NO LONGER swaps the real compute for a digest stub. It builds a
    /// small programmatic [`tokenizers::Tokenizer`] (byte-level so any input tokenizes, no `[UNK]`
    /// surprises) plus an orthonormal [`StaticEmbeddingMatrix`], then funnels through
    /// [`from_engine`](Self::from_engine) so the embed loop, the `Static` enum dispatch, AND the REAL
    /// tokenize → matrix-lookup → mean-pool → L2-normalize algorithm all run at 100% coverage. The
    /// orthonormal "bag-of-bytes" matrix makes the embeddings content-reflecting (texts sharing a
    /// distinctive substring score higher), so a backfill+search behavioral test resolves a real hit
    /// through the real engine — not a hand-waved stub. `dim` is a small synthetic number
    /// ([`STUB_STATIC_DIM`]) with NO relationship to any real model (picking 256 would be the D4
    /// truth-assumption this design removes).
    #[cfg(any(test, coverage))]
    pub fn new_stub(provider_id: &str, repo: &str) -> Self {
        Self::from_engine(provider_id, repo, build_stub_engine(STUB_STATIC_DIM))
    }

    /// Builds this engine's runtime descriptor once the dim is known.
    ///
    /// Sets the per-adapter truth (A-S2): dtype=Float32 (components are f32 — static is a
    /// weight-storage property, not an output-component dtype), normalized=`config.normalize`,
    /// pooling=Mean (model2vec's real pooling), and NO instruction template (static models are
    /// symmetric — query and document share one encoding, so the role carries no instruction). The
    /// quant level is N/A for static, so the model id is the bare repo (a model swap still changes
    /// the fingerprint via `model_id`). `effective_dim` is the ACTUAL pooled length when observed,
    /// else the native matrix dim — never a hardcoded constant (D4).
    fn descriptor_with(&self, effective_dim: Option<usize>) -> EmbeddingDescriptor {
        EmbeddingDescriptor {
            provider_id: self.provider_id.clone(),
            model_id: self.model_id.clone(),
            effective_dim: effective_dim.or(Some(self.dim)),
            dtype: EmbeddingDtype::Float32,
            normalized: self.normalize,
            pooling: EmbeddingPooling::Mean,
            instruction_template: None,
        }
    }
}

impl EmbeddingProvider for StaticEmbeddingProvider {
    /// Embeds a batch: tokenize each text, mean-pool its rows, optionally L2-normalize.
    ///
    /// `role` is threaded per the trait contract but is a DOCUMENTED NO-OP: static models are
    /// symmetric (no query/document instruction asymmetry), so query and document share one
    /// encoding. The parameter stays wired so the call sites never change across engines.
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
        // `effective_dim` is the native matrix dim here; the indexing loop re-derives the descriptor
        // from the actual returned length when it stamps the fingerprint.
        self.descriptor_with(None)
    }
}

/// Real static embed: per-text tokenize → mean-pool → optional L2-normalize, one vector per input.
///
/// F4: ALWAYS compiled (no longer `cfg`-gated). Lock-free over the immutable matrix + tokenizer behind
/// the `Arc`, so the worker pool can embed concurrently (the static tier's throughput win). Reuses the
/// PURE [`static_embed_ids`] so every build shares the exact pooling/normalize math. In test/coverage
/// builds the engine is the tiny in-memory one [`build_stub_engine`] assembles; the algorithm is
/// identical — only the matrix/tokenizer are synthetic, so the REAL compute path is exercised + covered.
async fn embed_impl(
    provider: &StaticEmbeddingProvider,
    texts: &[String],
    _role: EmbeddingRole,
) -> Result<Vec<Vec<f32>>> {
    if texts.is_empty() {
        return Ok(Vec::new());
    }
    let engine = provider.engine.clone();
    let mut out = Vec::with_capacity(texts.len());
    for text in texts {
        let encoding = engine
            .tokenizer
            .encode(text.as_str(), false)
            .map_err(|error| anyhow::anyhow!("tokenizing input: {error}"))?;
        let mut ids: Vec<u32> = encoding.get_ids().to_vec();
        ids.truncate(STATIC_MAX_INPUT_TOKENS);
        out.push(static_embed_ids(&engine.matrix, &ids, engine.normalize));
    }
    Ok(out)
}

/// Synthetic dimension the stub engine emits — an arbitrary small number unrelated to any real model.
#[cfg(any(test, coverage))]
const STUB_STATIC_DIM: usize = 64;

/// The curated vocabulary the in-memory stub engine recognizes (lower-cased, Whitespace-split).
///
/// F4: a small fixed word list so the stub tokenizer maps the words PathKeep's test corpora use onto
/// DISTINCT ids (and everything else onto a shared `[UNK]` row). Index 0 is reserved for `[UNK]`; the
/// rest are content words. Keep this comfortably under [`STUB_STATIC_DIM`] so the one-hot matrix rows
/// stay orthonormal (each id maps to its own basis dimension), which is what lets a query word resolve
/// its document by cosine in the behavioral search test.
#[cfg(any(test, coverage))]
const STUB_VOCAB_WORDS: &[&str] = &[
    "[UNK]",
    "alpha",
    "beta",
    "gamma",
    "delta",
    "epsilon",
    "zeta",
    "rust",
    "history",
    "search",
    "gmail",
    "github",
    "news",
    "blog",
    "docs",
    "example",
    "page",
    "embedding",
    "vector",
    "quantum",
    "banana",
    "zebra",
    "kestrel",
    "falcon",
    "otter",
    "profile",
    "default",
    "domain",
    "title",
    "visited",
    "url",
    "http",
    "https",
    "com",
    "the",
    "and",
];

/// Builds a TINY REAL in-memory [`StaticEngine`] for tests/coverage (no disk, no network).
///
/// F4: this is the heart of the test-methodology fix — instead of a digest stub that bypasses the
/// algorithm, it constructs an ACTUAL [`tokenizers::Tokenizer`] (a Whitespace-split, lower-casing
/// WordLevel model over [`STUB_VOCAB_WORDS`], with every other word folding onto `[UNK]`) plus an
/// orthonormal [`StaticEmbeddingMatrix`] (row `id` is the unit basis vector `e_{id % dim}`). Mean-pool
/// over those rows yields a normalized bag-of-words vector — deterministic, never zero for non-empty
/// input, and CONTENT-REFLECTING: two texts sharing a distinctive vocab word land closer in cosine
/// than two that do not, so a real backfill→search behavioral test resolves the right page through the
/// REAL [`embed_impl`]. `dim` is [`STUB_STATIC_DIM`].
#[cfg(any(test, coverage))]
fn build_stub_engine(dim: usize) -> StaticEngine {
    use tokenizers::Tokenizer;
    use tokenizers::models::wordlevel::WordLevel;
    use tokenizers::normalizers::Lowercase;
    use tokenizers::pre_tokenizers::whitespace::Whitespace;

    // `vocab` collects into the ahash map `WordLevel::vocab` expects (inferred — no ahash import).
    let model = WordLevel::builder()
        .vocab(
            STUB_VOCAB_WORDS
                .iter()
                .enumerate()
                .map(|(index, word)| ((*word).to_string(), index as u32))
                .collect(),
        )
        .unk_token("[UNK]".to_string())
        .build()
        .expect("stub word-level model is well-formed");
    let mut tokenizer = Tokenizer::new(model);
    tokenizer.with_normalizer(Some(Lowercase));
    tokenizer.with_pre_tokenizer(Some(Whitespace {}));

    // Orthonormal rows: row `id` is the unit basis vector `e_{id % dim}`. With dim >= the vocab size the
    // mean-pool is a normalized word histogram, so distinct texts differ and a query word resolves its
    // document by cosine. The `[UNK]` row (id 0) is e0, so an all-unknown text still embeds non-zero.
    let vocab_size = STUB_VOCAB_WORDS.len();
    let mut rows = vec![0.0_f32; vocab_size * dim];
    for id in 0..vocab_size {
        rows[id * dim + (id % dim)] = 1.0;
    }
    let matrix =
        StaticEmbeddingMatrix::new(rows, dim, None).expect("stub static matrix is well-formed");
    StaticEngine { matrix, tokenizer, normalize: true }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn near_unit_norm(vector: &[f32]) -> f32 {
        vector.iter().map(|value| value * value).sum::<f32>().sqrt()
    }

    #[test]
    fn parse_static_config_reads_fields_and_defaults_normalize() {
        let config =
            parse_static_config(br#"{ "hidden_dim": 256, "normalize": false }"#).expect("parse");
        assert_eq!(config.hidden_dim, 256);
        assert!(!config.normalize);

        // Missing `normalize` defaults to model2vec's true.
        let defaulted = parse_static_config(br#"{ "hidden_dim": 256 }"#).expect("parse default");
        assert!(defaulted.normalize);
    }

    #[test]
    fn unk_spec_drops_only_bpe_string_unk_matching_model2vec() {
        // S1 correctness boundary: mirror model2vec EXACTLY. It drops unk ONLY when the tokenizer
        // model exposes a STRING `unk_token` (BPE / WordPiece). potion ships a **Unigram** tokenizer
        // that declares `unk_id` (an index) but NO string `unk_token`, so model2vec pools `[UNK]` —
        // and so must we. Dropping the Unigram unk would DIVERGE from the reference (~0.80 cosine on
        // OOV rows), which the parity gate proves.
        // Unigram with an `unk_id` INDEX but no string `unk_token` → None (do NOT drop — pool it).
        assert_eq!(
            unk_spec_from_tokenizer_json(br#"{ "model": { "type": "Unigram", "unk_id": 1 } }"#),
            None,
        );
        // BPE / WordPiece declare a string `unk_token` → drop it (model2vec does).
        assert_eq!(
            unk_spec_from_tokenizer_json(
                br#"{ "model": { "type": "BPE", "unk_token": "<unk>" } }"#
            ),
            Some(UnkSpec::Token("<unk>".to_string())),
        );
        // No unk declared → None.
        assert_eq!(unk_spec_from_tokenizer_json(br#"{ "model": { "type": "WordLevel" } }"#), None);
        // Malformed JSON → None, never a panic.
        assert_eq!(unk_spec_from_tokenizer_json(b"{not json"), None);
        // Missing `model` block → None.
        assert_eq!(unk_spec_from_tokenizer_json(br#"{ "version": "1.0" }"#), None);
    }

    #[test]
    fn parse_static_config_rejects_missing_dim_and_malformed_json() {
        assert!(parse_static_config(br#"{ "normalize": true }"#).is_err());
        assert!(parse_static_config(b"{not json").is_err());
    }

    #[test]
    fn matrix_new_validates_shape_and_weights() {
        // Well-formed: 3 rows × 2.
        let matrix = StaticEmbeddingMatrix::new(vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0], 2, None)
            .expect("matrix");
        assert_eq!(matrix.dim(), 2);
        assert_eq!(matrix.vocab_size(), 3);

        // dim 0 is rejected.
        assert!(StaticEmbeddingMatrix::new(vec![1.0], 0, None).is_err());
        // Ragged buffer (5 not a multiple of 2) is rejected.
        assert!(StaticEmbeddingMatrix::new(vec![1.0, 2.0, 3.0, 4.0, 5.0], 2, None).is_err());
        // Zero rows is rejected.
        assert!(StaticEmbeddingMatrix::new(Vec::new(), 2, None).is_err());
        // Weights length mismatch (2 weights for 3 rows) is rejected.
        assert!(
            StaticEmbeddingMatrix::new(vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0], 2, Some(vec![1.0, 2.0]))
                .is_err()
        );
        // Matching weights length is accepted.
        assert!(
            StaticEmbeddingMatrix::new(
                vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0],
                2,
                Some(vec![1.0, 1.0, 1.0])
            )
            .is_ok()
        );
    }

    #[test]
    fn mean_pool_averages_in_vocab_rows() {
        // 3 rows × 2: [ [2,4], [6,8], [10,12] ].
        let matrix =
            StaticEmbeddingMatrix::new(vec![2.0, 4.0, 6.0, 8.0, 10.0, 12.0], 2, None).expect("m");
        // Pool ids 0 and 2 → mean of [2,4] and [10,12] = [6,8].
        assert_eq!(matrix.mean_pool(&[0, 2]), vec![6.0, 8.0]);
        // Pool a single id → that row unchanged.
        assert_eq!(matrix.mean_pool(&[1]), vec![6.0, 8.0]);
    }

    #[test]
    fn mean_pool_skips_out_of_range_ids() {
        let matrix = StaticEmbeddingMatrix::new(vec![2.0, 4.0, 6.0, 8.0], 2, None).expect("m");
        // id 5 is out of range (vocab 2); it is skipped, so the mean is just row 0.
        assert_eq!(matrix.mean_pool(&[0, 5]), vec![2.0, 4.0]);
        // All ids out of range → zero vector (no token to pool), NOT a panic or NaN.
        assert_eq!(matrix.mean_pool(&[9, 9]), vec![0.0, 0.0]);
        // Empty sequence → zero vector.
        assert_eq!(matrix.mean_pool(&[]), vec![0.0, 0.0]);
    }

    #[test]
    fn mean_pool_drops_unk_token_from_sum_and_count() {
        // S1 fix: the reference DROPS <unk> before pooling (model2vec-rs `retain(|id| id != unk_id)`
        // / Python `tokenize`). 3 rows × 2: [ [2,4], [6,8], [10,12] ], with row 1 designated as <unk>.
        let matrix = StaticEmbeddingMatrix::new(vec![2.0, 4.0, 6.0, 8.0, 10.0, 12.0], 2, None)
            .expect("m")
            .with_unk_token(Some(1));
        // Pool ids 0, 1(=unk), 2: the unk id is excluded from BOTH the sum and the count, so the mean
        // is over rows 0 and 2 only → mean([2,4], [10,12]) = [6,8] (NOT mean of all three = [6,8]…
        // make the distinction sharp): without the drop, pooling all three averages to [6,8] too, so
        // use an asymmetric case below.
        assert_eq!(matrix.mean_pool(&[0, 1, 2]), vec![6.0, 8.0]);
        // Asymmetric proof the unk is truly excluded (not just numerically coincidental):
        // rows [ [0,0], [9,9](=unk), [3,3] ]; dropping unk → mean([0,0],[3,3]) = [1.5,1.5].
        // Pooling unk too → ([0,0]+[9,9]+[3,3])/3 = [4,4], which would FAIL this assertion.
        let asymmetric = StaticEmbeddingMatrix::new(vec![0.0, 0.0, 9.0, 9.0, 3.0, 3.0], 2, None)
            .expect("m")
            .with_unk_token(Some(1));
        assert_eq!(asymmetric.mean_pool(&[0, 1, 2]), vec![1.5, 1.5]);
        // A sequence that is ALL <unk> after filtering → zero vector (empty-after-filter honesty).
        assert_eq!(asymmetric.mean_pool(&[1, 1]), vec![0.0, 0.0]);
        // Without an unk id set (synthetic matrices), the unk row is pooled like any other.
        let no_unk =
            StaticEmbeddingMatrix::new(vec![0.0, 0.0, 9.0, 9.0, 3.0, 3.0], 2, None).expect("m");
        assert_eq!(no_unk.mean_pool(&[0, 1, 2]), vec![4.0, 4.0]);
    }

    #[test]
    fn mean_pool_applies_per_token_weights_before_averaging() {
        // 2 rows × 2: [ [1,1], [2,2] ], weights [3.0, 0.5].
        let matrix = StaticEmbeddingMatrix::new(vec![1.0, 1.0, 2.0, 2.0], 2, Some(vec![3.0, 0.5]))
            .expect("m");
        // Pool both: ( [1,1]*3 + [2,2]*0.5 ) / 2 = ( [3,3] + [1,1] ) / 2 = [2,2].
        assert_eq!(matrix.mean_pool(&[0, 1]), vec![2.0, 2.0]);
    }

    #[test]
    fn static_l2_normalize_makes_unit_norm_and_keeps_zero_zero() {
        let mut vector = vec![3.0, 4.0];
        static_l2_normalize(&mut vector);
        assert!((near_unit_norm(&vector) - 1.0).abs() < 1e-6);
        assert!((vector[0] - 0.6).abs() < 1e-6);
        assert!((vector[1] - 0.8).abs() < 1e-6);
        // A zero vector stays zero (epsilon floor avoids NaN).
        let mut zero = vec![0.0, 0.0];
        static_l2_normalize(&mut zero);
        assert_eq!(zero, vec![0.0, 0.0]);
    }

    #[test]
    fn static_embed_ids_normalizes_only_when_requested() {
        let matrix = StaticEmbeddingMatrix::new(vec![3.0, 4.0, 6.0, 8.0], 2, None).expect("m");
        // Normalize on: row 0 [3,4] → unit norm [0.6,0.8].
        let normalized = static_embed_ids(&matrix, &[0], true);
        assert!((near_unit_norm(&normalized) - 1.0).abs() < 1e-6);
        // Normalize off: bare mean-pooled row 0 [3,4].
        let raw = static_embed_ids(&matrix, &[0], false);
        assert_eq!(raw, vec![3.0, 4.0]);
    }

    #[tokio::test]
    async fn stub_embed_returns_normalized_symmetric_vectors() {
        let provider = StaticEmbeddingProvider::new_stub("static:test", "minishlab/potion");
        let docs = provider
            .embed(&["alpha".to_string(), "beta".to_string()], EmbeddingRole::Document)
            .await
            .expect("docs");
        assert_eq!(docs.len(), 2);
        assert_eq!(docs[0].len(), STUB_STATIC_DIM);
        assert!((near_unit_norm(&docs[0]) - 1.0).abs() < 1e-6);
        // Different texts differ.
        assert_ne!(docs[0], docs[1]);

        // SYMMETRY: the same text under Query vs Document MUST match (static models are symmetric).
        let query = provider.embed(&["alpha".to_string()], EmbeddingRole::Query).await.expect("q");
        assert_eq!(query[0], docs[0]);
    }

    #[tokio::test]
    async fn stub_embed_empty_input_short_circuits() {
        let provider = StaticEmbeddingProvider::new_stub("static:test", "repo");
        assert!(provider.embed(&[], EmbeddingRole::Document).await.expect("empty").is_empty());
    }

    #[test]
    fn descriptor_reports_static_truth() {
        let provider = StaticEmbeddingProvider::new_stub("static:test", "minishlab/potion");
        let descriptor = provider.descriptor();
        assert_eq!(descriptor.dtype, EmbeddingDtype::Float32);
        assert!(descriptor.normalized);
        assert_eq!(descriptor.pooling, EmbeddingPooling::Mean);
        assert_eq!(descriptor.instruction_template, None);
        assert_eq!(descriptor.effective_dim, Some(STUB_STATIC_DIM));
        assert_eq!(descriptor.provider_id, "static:test");
        assert_eq!(descriptor.model_id, "minishlab/potion");
        assert_eq!(provider.model_id(), "minishlab/potion");
    }

    use crate::models::{AiProviderConfig, AiProviderPurpose, AiRequestFormat};
    use secrecy::SecretString;

    fn static_runtime(default_model: &str) -> AiProviderRuntime {
        AiProviderRuntime {
            config: AiProviderConfig {
                id: "static-provider".to_string(),
                name: "In-app Static".to_string(),
                purpose: AiProviderPurpose::Embedding,
                request_format: AiRequestFormat::OpenAi,
                enabled: true,
                base_url: Some(STATIC_INAPP_BASE_URL.to_string()),
                default_model: default_model.to_string(),
                dimensions: None,
                ..AiProviderConfig::default()
            },
            api_key: None,
        }
    }

    fn external_runtime(base_url: Option<&str>) -> AiProviderRuntime {
        AiProviderRuntime {
            config: AiProviderConfig {
                id: "external-provider".to_string(),
                name: "External".to_string(),
                purpose: AiProviderPurpose::Embedding,
                request_format: AiRequestFormat::OpenAi,
                enabled: true,
                base_url: base_url.map(str::to_string),
                default_model: "text-embedding-test".to_string(),
                dimensions: None,
                ..AiProviderConfig::default()
            },
            api_key: Some(SecretString::from("key".to_string())),
        }
    }

    #[test]
    fn runtime_uses_static_detects_sentinel() {
        assert!(runtime_uses_static(&static_runtime("")));
        assert!(!runtime_uses_static(&external_runtime(Some("http://localhost:1234/v1"))));
        assert!(!runtime_uses_static(&external_runtime(None)));
    }

    #[test]
    fn static_repo_for_runtime_prefers_config_then_default() {
        assert_eq!(static_repo_for_runtime(&static_runtime("")), DEFAULT_STATIC_MODEL_REPO);
        assert_eq!(static_repo_for_runtime(&static_runtime("custom/static")), "custom/static");
    }

    #[test]
    fn degrade_static_to_external_falls_back_or_errors() {
        // A runtime carrying a usable external base URL degrades to the external adapter.
        // (`AnyEmbeddingProvider` is intentionally not `Debug`, so match rather than `.expect`.)
        match degrade_static_to_external(&external_runtime(Some("http://host/v1")), "repo") {
            Ok(AnyEmbeddingProvider::External(_)) => {}
            Ok(_) => panic!("expected the external degrade arm"),
            Err(error) => panic!("expected degrade, got error: {error}"),
        }

        // A pure static runtime (only the sentinel, no external base) returns an actionable error.
        let error = degrade_static_to_external(&static_runtime(""), "minishlab/potion")
            .err()
            .expect("expected a no-external-fallback error");
        assert!(error.to_string().contains("not downloaded"));
    }

    #[test]
    fn stub_selector_builds_static_arm_for_static_runtime() {
        let dir = tempfile::tempdir().expect("tempdir");
        let paths = crate::config::project_paths_with_root(dir.path());
        match select_static_embedding_provider(&paths, &static_runtime("custom/static")) {
            Ok(provider @ AnyEmbeddingProvider::Static(_)) => {
                assert_eq!(provider.model_id(), "custom/static");
            }
            Ok(_) => panic!("expected the static arm"),
            Err(error) => panic!("expected the static arm, got error: {error}"),
        }
    }

    #[test]
    fn manifest_lists_the_three_static_files_from_one_repo() {
        assert_eq!(DEFAULT_STATIC_MODEL_FILES.len(), 3);
        let names: Vec<&str> = DEFAULT_STATIC_MODEL_FILES.iter().map(|file| file.name).collect();
        assert!(names.contains(&"config.json"));
        assert!(names.contains(&"tokenizer.json"));
        assert!(names.contains(&"model.safetensors"));
        // model2vec ships everything in ONE repo (unlike the candle GGUF/tokenizer split).
        for file in DEFAULT_STATIC_MODEL_FILES {
            assert_eq!(file.repo, DEFAULT_STATIC_MODEL_REPO);
            // Every digest is the pinned 64-hex shape (sentinel zeros are still 64 hex).
            assert_eq!(file.sha256.len(), 64);
        }
    }

    /// Builds a TINY real [`StaticEngine`] from a handful of WordLevel tokens (the F4 gate fixture).
    ///
    /// A 3-token Whitespace WordLevel model + a 3-dim orthonormal matrix, so `embed` runs the REAL
    /// tokenize → matrix-lookup → mean-pool → (optional) L2-norm with hand-checkable arithmetic.
    fn tiny_word_engine(normalize: bool) -> StaticEngine {
        use tokenizers::Tokenizer;
        use tokenizers::models::wordlevel::WordLevel;
        use tokenizers::pre_tokenizers::whitespace::Whitespace;

        // `vocab` collects into the ahash map `WordLevel::vocab` expects (inferred — no ahash import).
        let model = WordLevel::builder()
            .vocab(
                [
                    ("alpha".to_string(), 0_u32),
                    ("beta".to_string(), 1),
                    ("gamma".to_string(), 2),
                    ("[UNK]".to_string(), 3),
                ]
                .into_iter()
                .collect(),
            )
            .unk_token("[UNK]".to_string())
            .build()
            .expect("tiny word-level model");
        let mut tokenizer = Tokenizer::new(model);
        tokenizer.with_pre_tokenizer(Some(Whitespace {}));
        // 4 rows × 3 dims: row 0/1/2 are the unit basis vectors e0/e1/e2; the unk row reuses e0.
        let rows = vec![
            1.0, 0.0, 0.0, // alpha
            0.0, 1.0, 0.0, // beta
            0.0, 0.0, 1.0, // gamma
            1.0, 0.0, 0.0, // [UNK]
        ];
        let matrix = StaticEmbeddingMatrix::new(rows, 3, None).expect("tiny matrix");
        StaticEngine { matrix, tokenizer, normalize }
    }

    #[tokio::test]
    async fn real_in_memory_engine_embeds_deterministic_nonzero_correct_dim_vectors() {
        // F4 GATE: construct a TINY in-memory `StaticEngine` and run the REAL `embed` (the same
        // tokenize → lookup → mean-pool → L2-norm path the production engine uses), proving the
        // algorithm is COMPILED + COVERED rather than replaced by a digest stub.
        let provider = StaticEmbeddingProvider::from_engine(
            "static:tiny",
            "tiny/model",
            tiny_word_engine(true),
        );

        // The descriptor reflects the engine truth: dim = the matrix's real column count, Mean pooling.
        assert_eq!(provider.descriptor().effective_dim, Some(3));
        assert_eq!(provider.descriptor().pooling, EmbeddingPooling::Mean);

        let docs = provider
            .embed(&["alpha beta".to_string(), "gamma".to_string()], EmbeddingRole::Document)
            .await
            .expect("embed");
        assert_eq!(docs.len(), 2);
        // CORRECT DIM: every vector is the matrix dim (3).
        assert!(docs.iter().all(|vector| vector.len() == 3));
        // NON-ZERO + L2-NORM: "alpha beta" mean-pools e0 and e1 → [0.5, 0.5, 0] → unit-norm
        // [0.707…, 0.707…, 0].
        assert!((near_unit_norm(&docs[0]) - 1.0).abs() < 1e-6, "pooled vector is L2-normalized");
        let inv_sqrt2 = 1.0 / 2.0_f32.sqrt();
        assert!((docs[0][0] - inv_sqrt2).abs() < 1e-6);
        assert!((docs[0][1] - inv_sqrt2).abs() < 1e-6);
        assert!(docs[0][2].abs() < 1e-6);
        // "gamma" → e2 (already unit), so it is the third basis vector.
        assert_eq!(docs[1], vec![0.0, 0.0, 1.0]);

        // DETERMINISTIC: the same text embeds to the exact same vector across calls.
        let again =
            provider.embed(&["alpha beta".to_string()], EmbeddingRole::Document).await.unwrap();
        assert_eq!(again[0], docs[0]);
        // SYMMETRIC: query and document encodings match (static has no instruction asymmetry).
        let as_query =
            provider.embed(&["alpha beta".to_string()], EmbeddingRole::Query).await.unwrap();
        assert_eq!(as_query[0], docs[0]);
    }

    #[tokio::test]
    async fn real_in_memory_engine_pooling_and_norm_off_behave() {
        // F4 GATE (pooling + normalize=false): with L2-norm OFF the bare mean-pool is returned, and an
        // asymmetric token mix proves the mean weights each token equally.
        let provider = StaticEmbeddingProvider::from_engine(
            "static:tiny",
            "tiny/model",
            tiny_word_engine(false),
        );
        let out = provider
            .embed(&["alpha alpha beta".to_string()], EmbeddingRole::Document)
            .await
            .expect("embed");
        // mean(e0, e0, e1) = [2/3, 1/3, 0] — NOT unit norm (normalize is off), proving the raw pool.
        assert!((out[0][0] - 2.0 / 3.0).abs() < 1e-6);
        assert!((out[0][1] - 1.0 / 3.0).abs() < 1e-6);
        assert!(out[0][2].abs() < 1e-6);
        assert!(near_unit_norm(&out[0]) < 0.9, "normalize=false leaves the bare mean-pool");
    }

    #[tokio::test]
    async fn new_stub_uses_the_real_engine_for_distinct_nonzero_vectors() {
        // F4: `new_stub` is now a REAL tiny engine (WordLevel), so the coverage backfill exercises the
        // real compute. Distinct texts → distinct vectors; every vector is non-zero, unit-norm, and the
        // synthetic dim — proving the real tokenize→pool→norm path, not a bypass.
        let provider = StaticEmbeddingProvider::new_stub("static:stub", "stub/model");
        let vectors = provider
            .embed(
                &["banana kestrel".to_string(), "quantum zebra".to_string()],
                EmbeddingRole::Document,
            )
            .await
            .expect("embed");
        assert_eq!(vectors[0].len(), STUB_STATIC_DIM);
        assert_ne!(vectors[0], vectors[1], "distinct texts embed to distinct vectors");
        for vector in &vectors {
            assert!((near_unit_norm(vector) - 1.0).abs() < 1e-6);
            assert!(vector.iter().any(|component| *component != 0.0), "no all-zero vector");
        }
    }
}
