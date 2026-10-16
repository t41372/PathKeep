# In-app transformer embeddings: runtimes and models (2026-10-15)

Input for `WORK-AI-EMBED-INAPP` item 3: which runtime to benchmark for a built-in transformer embedding model next to the static one, and what to measure. Desk research only; nothing here was run on our machines. Speeds come from vendors or the community and are marked as such.

## Candidates

| Model                                           | License                                                                                                        | Size                                                                                   | Quality (MMTEB mean / retrieval)              | Prompting                                                                                                                                                                                                                        |
| ----------------------------------------------- | -------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------- | --------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Qwen3-Embedding-0.6B                            | Apache-2.0                                                                                                     | ~440M parameters outside the embedding table                                           | 64.33 / 64.64; C-MTEB 66.33 (retrieval 71.03) | Query `Instruct: {task}\nQuery:{query}` (default task "Given a web search query, retrieve relevant passages that answer the query"); documents plain. Last-token pooling, left padding. Leaving out the instruction costs 1–5 %. |
| EmbeddingGemma 2 (2026-10-06)                   | Apache-2.0 (card metadata and linked text; not gated; the card still asks for the Gemma Prohibited Use Policy) | 740M total, text encoder ~270M, transformer ~130M; 768 dims, Matryoshka to 512/256/128 | 61.36 (57.89 at 128 dims) / not published     | Query `task: search result \| query: {q}`; document `title: {title} \| text: {content}` (`title: none` without one). Mean pooling; re-normalize after truncating dimensions. bf16 or f32 only: fp16 gives NaN.                   |
| potion-multilingual-128M (today's static model) | MIT                                                                                                            | 128M-row lookup                                                                        | 47.31 / 37.86                                 | none                                                                                                                                                                                                                             |

Both transformer models score about 25–27 points above the static model on retrieval. EmbeddingGemma 2 has about 3.4× fewer FLOPs per token than Qwen3-Embedding-0.6B (arithmetic from the configs).

EmbeddingGemma 2 is **not** Gemma 3: it has Gemma-4-style per-layer inputs, bidirectional attention within a symmetric sliding window, and head_dim 512 on the global layers. llama.cpp's `gemma-embedding2.cpp` (234 lines, PR #30054, merged 2026-10-06) is the reference implementation.

## Runtimes

| Runtime                                 | License · stars · owner                         | Metal                             | CPU int8              | Qwen3-Emb                              | EG2                                                                     | Supply-chain rule                                                                                                |
| --------------------------------------- | ----------------------------------------------- | --------------------------------- | --------------------- | -------------------------------------- | ----------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------- |
| candle 0.11.0 (2026-06-26)              | MIT/Apache · 21.2k · Hugging Face               | yes                               | GGUF Q8_0 / k-quants  | yes (`qwen3.rs`, `quantized_qwen3.rs`) | no: would need a new model port                                         | pure Rust, passes                                                                                                |
| llama.cpp via llama-cpp-2 0.1.160       | MIT · wrapper 657, llama.cpp 131k (ggml-org)    | yes                               | Q8_0/Q4_K, i8mm/VNNI  | yes                                    | llama.cpp master yes; the crate vendors a 2026-10-05 commit, so not yet | cmake + bindgen; the wrapper is under 6k (our own thin FFI over vendored llama.cpp is the alternative)           |
| ort 2.0.0-rc.14                         | MIT/Apache · 2.6k; ONNX Runtime 22k (Microsoft) | no (CoreML / WebGPU only)         | MLAS int8/int4        | via ONNX                               | ONNX export exists (onnx-community), untested                           | default build downloads binaries in build.rs: breaks the rule; vcpkg port is 1.23.2, too old for the default API |
| tract 0.23.9                            | MIT/Apache · 3.1k · Sonos                       | yes (0.23, per-node CPU fallback) | i8, Q4_0, MatMulNBits | via ONNX, unverified                   | via ONNX, unverified                                                    | pure Rust, known org                                                                                             |
| mistral.rs 0.9.4 (git; crates.io 0.8.1) | MIT · 7.8k                                      | yes                               | ISQ via candle        | yes                                    | no (issue #2473)                                                        | pins a candle git revision; a whole LLM engine                                                                   |

Ruled out: fastembed-rs (1.0k stars, EmbeddingGemma v1 only, defaults to ort's download), burn 0.22 (no models), LiteRT-LM (Google's runtime with EG2, but Metal is a prebuilt dylib and there is no Rust API).

Our 2026-06 candle numbers (F32 6.29 docs/s, Q8 1.25 docs/s on CPU) are out of date: candle main has aarch64 repack kernels for quantized weights (#3697, 2026-09-21) and a fix for quantized matmul running a 128-token prompt no faster per row than one token (#4003, 2026-10-02), which explains the Q8 result. Neither is in a release yet.

## Published speeds (not measured by us)

| Model · runtime · precision                                      | Hardware                     | Result                                                    | Source                                                 |
| ---------------------------------------------------------------- | ---------------------------- | --------------------------------------------------------- | ------------------------------------------------------ |
| EG2 text · LiteRT int4 · 128 tokens · batch 1                    | MacBook Pro M5               | CPU 31.3 ms, GPU 9.5 ms                                   | vendor model card                                      |
| same                                                             | Core Ultra / Arm 2.3–2.8 GHz | CPU 71.7 / 105 ms                                         | vendor model card                                      |
| EG2 · llama.cpp Q8_0                                             | M5 Max, Metal                | p50 13 ms; 30 docs/s indexing                             | community benchmark on a loaded machine                |
| same, 4 CPU threads                                              | M5 Max                       | p50 36.6 ms                                               | same                                                   |
| Qwen3-Embedding-0.6B · LM Studio (llama.cpp, Metal), our own log | this Mac, 2026-10-15         | ~0.47 requests/s of 33–44 texts, ~30 visits/s of progress | `~/.lmstudio/server-logs`, the user's real index build |

At 10 docs/s, 14.4M documents take 400 hours; at 100 docs/s, 40 hours. What matters is the unique documents behind the visits after content dedup, which the benchmark has to count.

## Measured on this Mac (2026-10-16)

Apple M5 Max (6 performance + 12 efficiency cores, 64 GB), macOS 27.0.1. llama.cpp master `a30fb492` (2026-10-16), built with Metal, `llama-embedding` over 2,000 synthetic page texts shaped like what PathKeep now sends (title, site and address words, a quarter with a few hundred characters of body; mixed English and Chinese; about 45 tokens each) in its default batching (about 180 texts, 8k tokens per batch). Throughput is from the tool's batch log, leaving out the first batch (warm-up, shader compile) and the last. The CPU rows use 4 threads with no GPU layers on 600 texts. Peak memory is the process footprint, mostly the 8k-token batch buffers. Synthetic text says nothing about quality; that needs the evaluation on real history.

| Model · precision         | Metal docs/s | CPU 4 threads docs/s | Peak memory (Metal / CPU) |
| ------------------------- | -----------: | -------------------: | ------------------------- |
| Qwen3-Embedding-0.6B Q8_0 |          640 |                   20 | 1.7 / 2.2 GB              |
| Qwen3-Embedding-0.6B F16  |          649 |                   33 | 1.7 / 1.8 GB              |
| EmbeddingGemma 2 Q8_0     |          810 |                   48 | 1.1 / 1.7 GB              |
| EmbeddingGemma 2 BF16     |          784 |                   71 | 1.1 / 1.6 GB              |

- On the CPU, Q8_0 is slower than F16 / BF16 on this chip: llama.cpp dequantizes for prompt processing, and the M5's half-precision matrix units are fast. A 4-core x86 machine without them will be slower still, and Q8_0 may win there; it has to be measured on one.
- On Metal, 1 million unique pages take about 26 minutes with Qwen3 and 21 with EmbeddingGemma 2; on 4 CPU threads, from 4 hours (EmbeddingGemma 2 BF16) to 14 (Qwen3 Q8_0). The unique pages behind 14.4M visits after content dedup is still to be counted.
- LM Studio's ~30 visits/s during the user's index build (2026-10-15) ran on the old text, often with 2,048 characters of body, over HTTP; the shorter page text should speed that up too.
- For comparison, the in-app static model runs about 1,900 texts/s on one thread (test profile, `static_embedding_e2e`). Mapping its matrix instead of copying it (2026-10-16) took its footprint from 1,596 MiB to 618 MiB at the same speed; 578 MiB of the rest is the `tokenizers` Unigram trie (a hash map per node, 500k pieces; unchanged in tokenizers 0.23.2).

## What to benchmark, in order

1. llama.cpp master (with #30054) through its own CLI or server, to get the ceiling: both models, BF16 / Q8_0 / Q4_K, Metal and `-t 4 -ngl 0`.
2. candle at git main (not 0.11.0): Qwen3-Embedding on CPU (Q8_0 / Q4_K) and Metal (F32 / BF16). The only candidate that passes the supply-chain rule with the model already written.
3. tract on the onnx-community exports, CPU and Metal; if they load, the cheapest pure-Rust path to EG2.
4. Then decide how EG2 would run: a candle port, or a thin FFI in our repo over vendored llama.cpp. ort last.

Measure: docs/s at batch 1, 16 and 64 on real titles and URLs (64–128 tokens, mixed zh / en, after the embedding text is cleaned, `WORK-AI-EMBED-INAPP` item 2); 4 CPU threads on this Mac and on a 4-core x86 (AVX2 path); Metal; peak RSS; cold start including Metal shader compile; cosine against a sentence-transformers f32 reference (≥ 0.99) per precision, EG2 fp16 on Metal in particular; recall@10 against the static model on our own evaluation set; hours to embed the unique documents behind 14.4M visits.
