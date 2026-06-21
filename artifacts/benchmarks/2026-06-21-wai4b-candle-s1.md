# W-AI-4b — S1 candle embedding throughput + parity (2026-06-21, QUANTIZED rework)

In-app candle Qwen3-Embedding-0.6B engine, reworked from F32 safetensors to a **quantized GGUF**
checkpoint (W-AI-4b). Reproducible artifact note.

## Quantized approach (what shipped & why)

- **Path:** a vendored quantized Qwen3 decoder built on candle-transformers' PUBLIC quantized
  building blocks (`gguf_file::Content`, `QMatMul::from_weights`, quantized `RmsNorm::from_qtensor`,
  `ConcatKvCache`, RoPE, `repeat_kv`). candle-transformers DOES ship `quantized_qwen3::ModelWeights`,
  but its public `forward` returns **logits** (it applies `lm_head` after narrowing to the last
  token) and all its layer/embedding fields are **private** — so it cannot yield the last-token
  **hidden state** an embedding model needs. The vendored decoder runs the identical Qwen3 forward
  but STOPS at the final RMSNorm and returns hidden states; last-token pooling + L2-normalize follow.
- **Hidden states confirmed exposed:** yes — via the vendored forward (the shipped `quantized_qwen3`
  alone does NOT expose them; reported per the directive). bf16-in-RAM fallback was NOT needed.
- **GGUF repo:** weights from the OFFICIAL `Qwen/Qwen3-Embedding-0.6B-GGUF` (Q8_0, 639 MB);
  `tokenizer.json` + `config.json` from the base `Qwen/Qwen3-Embedding-0.6B` (the GGUF repo ships no
  tokenizer). All fetched via `hf-hub`, SHA-256-pinned. Model reused across texts (KV cache reset
  between texts via the now-reachable `clear_kv_cache`) — no per-text model rebuild.

## Environment

- Machine: **Apple M5 Max, 18 cores** (NOT the 4-core/3GHz target — this is an OPTIMISTIC ceiling).
- Build: `--release`, candle 0.10.2 CPU-only (no accelerate/mkl/cuda/metal).
- Model: `Qwen/Qwen3-Embedding-0.6B-GGUF`, Q8_0 quantized weights loaded into candle quantized tensors.
- Engine: last-token pooling + L2-normalize; one text per forward (no padding); KV cache reset per text.

## Commands

```sh
PATHKEEP_CANDLE_MODELS_DIR=$(pwd)/artifacts/candle-models PATHKEEP_CANDLE_S1=1 \
  cargo test --manifest-path src-tauri/Cargo.toml -p vault-core \
  --test candle_embedding_e2e --release candle_s1_throughput_benchmark -- --nocapture

PATHKEEP_CANDLE_MODELS_DIR=$(pwd)/artifacts/candle-models PATHKEEP_CANDLE_PARITY=1 \
  cargo test --manifest-path src-tauri/Cargo.toml -p vault-core \
  --test candle_embedding_e2e --release candle_vs_lmstudio_cosine_parity -- --nocapture
```

(Q4_K_M is benchmarked when sideloaded under `<models>/quant-Q4_K_M/`; the official Qwen GGUF repo
ships only Q8_0 + f16, so Q4_K_M came from the community `Mungert/Qwen3-Embedding-0.6B-GGUF` repo for
the comparison only — it is NOT a shipped default.)

## Quant comparison (M5 Max, 18 cores)

| quant               | on-disk           | dim  | single d/s | batched d/s | peak RSS (max) | doc parity   | query-role parity | shipped?   |
| ------------------- | ----------------- | ---- | ---------- | ----------- | -------------- | ------------ | ----------------- | ---------- |
| **Q8_0 (default)**  | 639 MB            | 1024 | ~1.27      | ~1.25       | **~1.59 GB**   | **0.999537** | **0.999420**      | **YES**    |
| Q4_K_M (compare)    | 395 MB            | 1024 | ~1.25      | ~1.26       | ~1.0 GB (est.) | 0.982594 ❌  | —                 | no         |
| F32 (prior W-AI-4b) | 2.4 GB f32 in RAM | 1024 | 6.29       | 6.29        | ~3.4 GB        | 0.999698     | —                 | superseded |

- **14.4M first-fill ETA (Q8_0, batched):** ~14.4M / 1.25 / 3600 ≈ **~3,200 hours (~133 days)** on
  the 18-core M5 Max. On the 4-core/3 GHz target this is materially WORSE (CPU matmul scales with
  cores/clock): conservatively **months**.
- **Default = Q8_0**: it is the fastest quant whose parity holds (> 0.99). Q4_K_M's parity (0.983)
  is BELOW the 0.99 gate, and Q4 is no faster than Q8 on candle's CPU path anyway, so there is no
  reason to ship it. Q8_0 from the official Qwen repo is also the best supply-chain choice.
- The quant is recorded in the engine `model_id` (`<repo>:Q8_0`), so it is part of the embedding
  fingerprint: switching quant levels invalidates the index and re-embeds (a Q4 and Q8 index never
  share a fingerprint at the same dim).

## Parity (candle Q8_0 vs LM Studio `text-embedding-qwen3-embedding-0.6b`)

| text                                             | doc cosine   | query-role cosine |
| ------------------------------------------------ | ------------ | ----------------- |
| "the quick brown fox jumps over the lazy dog"    | 0.999537     | 0.999420          |
| "a treatise on the macroeconomics of central..." | 0.999834     | 0.999612          |
| "rust programming language memory safety..."     | 0.999732     | 0.999520          |
| **min**                                          | **0.999537** | **0.999420**      |

- **Document role** (no instruction): candle Q8_0 vs LM Studio's embedding of the same text.
- **Query role** (S2 fix): candle's query embedding (`Instruct: {task}\nQuery:{text}`, NO space)
  vs LM Studio's DOCUMENT embedding of that SAME explicitly-formatted string. This validates the
  corrected instruction template against an external REFERENCE, not against itself. (Discovered
  during W-AI-4b: LM Studio's `/v1/embeddings` does NOT auto-apply the query instruction — raw query
  vs instruction-formatted differ by cosine ~0.73 — so the external adapter's role no-op is correct,
  and the right reference is LM Studio embedding the formatted string as a document.) The wrong
  template (a stray space after `Query:`) measurably shifts the embedding (~0.997 cosine between the
  two formats on LM Studio), so this arm genuinely guards S2.

cosine ≈ 1.0 proves the quantized candle inference (tokenization + forward + last-token pooling +
L2-normalize) is correct against the same model served by LM Studio, for BOTH roles.

## Throughput interpretation & model2vec (D3) recommendation

The headline rework cut RAM roughly in HALF (≈3.4 GB F32 → ≈1.6 GB Q8_0) — the decisive win on the
8 GB target, where F32's footprint risked OOM during a background first-fill. **But quantization on
candle 0.10.2's CPU path is ~5× SLOWER than F32** (1.25 vs 6.29 docs/sec): candle's quantized matmul
dequantizes weights to f32 per call, so it pays the dequant cost ON TOP of an f32 GEMM, with no
native int8 dot-product kernel (this is where llama.cpp/LM Studio pull ahead — they have hand-tuned
int8 kernels candle lacks). Q4 and Q8 are nearly identical in speed for the same reason.

**Net:** quantization is the right RAM tradeoff for the constrained target, but it makes a full
14.4M first-fill on candle alone EVEN LESS practical than F32 did (months on the target).

**model2vec fast tier (D3) recommendation — STRENGTHENED, not weakened:**

- **Keep candle Q8_0 Qwen3-Embedding-0.6B as the default in-app QUALITY engine.** Parity is proven
  for both roles, RAM now fits the 8 GB target, it is fully offline, and it is fine for incremental
  /typical archives and query-time embedding (single doc ≈ 0.8 s here; slower but still interactive
  on the target, and the query path embeds ONE text).
- **The model2vec fast tier is now MORE clearly needed for large first-fills, not less.** The RAM
  improvement does NOT fix throughput — it made it worse. A multi-million-row first-fill on candle is
  impractical (months on the target). model2vec (static distilled embeddings, ~100–1000× faster on
  CPU) remains the drop-in fast tier the D4 model-agnostic design already accommodates: fast-tier
  first-fill, optional candle re-embed of hot/recent rows for quality. This is the project owner's
  call per D3; the data says the fast tier is REQUIRED for the 14.4M tail.
- Independent of the tier choice, the first-fill MUST stay the cancelable/resumable/off-thread
  background job it already is (`build_ai_index_with_control` + resumable cursor), with FTS5 +
  deterministic intelligence serving throughout.

## RAM note

Peak ~1.6 GB max RSS during Q8_0 embedding (down from ~3.4 GB F32). On an 8 GB target this is an
acceptable transient background-job footprint (the design treats first-fill as a background job),
and the headroom vs F32 is the reason for the switch. A further lever, if needed: a smaller quant
(Q4) cuts RAM more but its parity (0.983) falls below the index-quality gate, so it is not shipped.

## SHA-256 pins (recomputed 2026-06-21, F2)

```
config.json                      b5bf1f51fc45be473a54718cef92448d90a1be001bf9b9a44b8c7f10a19feaa9  (base repo)
tokenizer.json                   def76fb086971c7867b829c23a26261e38d9d74e02139253b38aeb9df8b4b50a  (base repo)
Qwen3-Embedding-0.6B-Q8_0.gguf   06507c7b42688469c4e7298b0a1e16deff06caf291cf0a5b278c308249c3e439  (GGUF repo)
Qwen3-Embedding-0.6B-q4_k_m.gguf c608745221a03d45ee7328aab5ae180ef5db54c9a47eda43ef05f73156ba824b  (Mungert, comparison only)
```
