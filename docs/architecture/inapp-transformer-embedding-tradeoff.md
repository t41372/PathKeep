# A built-in transformer embedding model: trade-off (2026-10-16)

`WORK-AI-EMBED-INAPP` item 3. The measurements behind every number here are in [2026-10-inapp-embedding-runtimes.md](research/2026-10-inapp-embedding-runtimes.md).

## Decision (2026-10-16): no built-in engine

The user chose C: PathKeep bundles no inference engine. The in-app model stays the static one, and Settings, onboarding and History say plainly that it is a static embedding model: very fast, but it finds pages by meaning less well than the usual contextual embedding models. Contextual models run through an embeddings service (LM Studio, Ollama or an online one).

- The 2026-10-09 removal stands, now by the user's decision ([candle-removal.md](../plan/test-audit-2026-10/candle-removal.md)).
- D2 and `05` §4 / §7 / §8 have no in-app transformer left to govern and stay as they are.
- Searching an unfinished index together with the complete one, and building newest first, help a switch to an external model as much as they would have helped this one. They change accepted docs; the user agreed on 2026-10-16, and they are `WORK-INDEX-SEARCH-WHILE-BUILDING` in `BACKLOG.md`. A static index kept for good as a fallback when an external service is away (shape 2 below) the user declined the same day.
- The dimension evaluation is not needed.
- Everything below is the analysis that led here, kept as the record.

## What the user asked for

On 2026-10-15 the user corrected the 2026-10-09 removal of the in-app Qwen3 tier (`847c6209`): they had asked for dead code to be checked, not for a working feature to go. A built-in model matters because an embedding model is not something to change lightly, and a built-in one is more complete and of a piece with the app. On 2026-10-16, after the evaluation on their own history, they approved bringing a transformer tier back.

## What it would have changed in accepted docs

1. **The removal reversed** ([candle-removal.md](../plan/test-audit-2026-10/candle-removal.md)). The user agreed on 2026-10-16, then asked whether a bundled inference engine is worth carrying at all, and chose not to (see "Decision").

These three needed the user's consent:

2. **D2 changes runtime** (if an engine is bundled). `02-architecture-decisions.md` D2 makes candle the only in-app inference engine. Measured on this Mac, candle is 2.4–3.5 times slower than llama.cpp with a footprint two to three times larger, so the transformer runs on llama.cpp (see "Runtime"). The static model keeps its own Rust engine.
3. **An unfinished index is searched while it builds** (whatever is bundled; it applies to external models too). `semantic-index-control.md` and `intelligence.md` §1 search an incomplete chosen index only when no complete index exists. Here it is searched together with the complete one (see "Searching while it builds").
4. **No bounded working set, no GPU opt-in** (if an engine is bundled). `05-embedding-retrieval-revision.md` §4 and §8 had the transformer embed only starred, recent, tagged and often revisited pages, and §7 made the GPU path opt-in. Both are retired: the transformer embeds every page, newest first, where it runs (see "Ship an engine at all?").

## Ship an engine at all?

On 2026-10-16 the user asked whether PathKeep should bundle an inference engine, since each hardware accelerator needs its own build and these engines change fast, or whether the static model plus an external one is enough.

- **External is llama.cpp too.** Ollama, LM Studio and llamafile are llama.cpp builds. The question is whose build runs the model: ours, pinned, with Metal and the CPU only, or theirs, with every GPU backend and their own updates.
- **Accelerators.** Metal is part of the one macOS build. CUDA, Vulkan and ROCm are separate builds with their own runtimes, CUDA's several hundred MB; PathKeep would ship none of them, so a PC's graphics card is reached only through an external server. The x86 instruction set is fixed at build time as well: either one baseline build (AVX2, which Celeron and Pentium N-series chips lack; the plain x86-64 baseline is several times slower), or `GGML_BACKEND_DL` with `GGML_CPU_ALL_VARIANTS`, which is 13–14 CPU libraries per operating system, one picked at run time.
- **Updates.** One pinned model needs nothing new from llama.cpp, so an upgrade happens only for a reason: a security fix in code we call, a new OS or compiler. Each upgrade is still a pass over our bindings. `llama.h` at `a30fb492` carries 37 deprecated declarations, among them the model-loading and context-creation calls an embedding wrapper uses.
- **Where it runs well.** On Apple Silicon the user's 131,322 pages take minutes (about 3 at the benchmark's 45-token texts; page text runs longer) in about 1.6 GB. On the 4-core x86 target it is the CPU only: several hours for the same archive, a day or more per million pages, weeks at 14.4M, with 1.7–2 GB of the 8 GB in use while it builds.

|                                                  | Who gets the better model                                                                        | Model under the index             | PathKeep carries                                                             |
| ------------------------------------------------ | ------------------------------------------------------------------------------------------------ | --------------------------------- | ---------------------------------------------------------------------------- |
| A. Built in everywhere                           | everyone; fast on Apple Silicon, slow on PCs                                                     | pinned                            | llama.cpp on three operating systems, x86 CPU variants, bindings, CI on each |
| B. Built in on Apple Silicon; external elsewhere | Macs with Apple Silicon without setup; elsewhere, whoever runs a local server or uses a provider | pinned on Macs; elsewhere, theirs | llama.cpp for macOS arm64 with Metal, bindings                               |
| C. Static plus external                          | whoever runs a local server or uses a provider                                                   | whatever their server serves      | no engine                                                                    |

Recommended: **B.** The user chose C. B keeps what the user asked for, a built-in model that never changes under the index, where it runs in minutes, and leaves the GPU matrix to the apps whose job it is. C is a sound outcome too: semantic search still works everywhere with the static model, at 0.589 hit@10 against 0.729 on the user's history.

A pure-Rust engine (candle) avoids vcpkg, bindings and CPU variant libraries, since its float matrix code picks instructions at run time, and D2 stands. Measured, it is 2.4–3.5 times slower and needs 2.3–4.1 GiB at the build's batch, which is the problem on 8 GB.

## Model: Qwen3-Embedding-0.6B

|                                       | Qwen3-Embedding-0.6B        | EmbeddingGemma 2                                                             |
| ------------------------------------- | --------------------------- | ---------------------------------------------------------------------------- |
| hit@10 / MRR@10 on the user's history | 0.729 / 0.549               | 0.712 / 0.520                                                                |
| llama.cpp, M5 Max, Metal              | 653–665 docs/s              | 784–810 docs/s                                                               |
| llama.cpp, M5 Max, 4 CPU threads      | 22 (Q8_0) – 37 (F16) docs/s | 48 (Q8_0) – 71 (BF16) docs/s                                                 |
| Dimensions                            | 1024, Matryoshka            | 768, Matryoshka to 512 / 256 / 128                                           |
| Licence                               | Apache-2.0                  | Apache-2.0; the card also asks users to follow Gemma's prohibited-use policy |
| In llama.cpp releases                 | yes                         | from v0.7.0 (2026-10-15); not in vcpkg's port or `llama-cpp-2` yet           |

The static model scores 0.589 / 0.410 on the same history.

Qwen3-Embedding-0.6B finds the user's pages better (about 2 points of hit@10 and 3 of MRR over 4,430 searches), has a plain licence and runs on every llama.cpp release. EmbeddingGemma 2 is about twice as fast on a CPU; that shortens the first build on a machine without a GPU, which "Searching while it builds" makes useful long before it ends, while the quality difference is felt on every search for as long as the index exists. The engine keeps D4 (dimension, pooling and prompts come from the model), so EmbeddingGemma 2 can be added later by its own decision.

**Weights: Q8_0 GGUF** from Qwen's own repository (`Qwen/Qwen3-Embedding-0.6B-GGUF`, 639 MB). On Metal it is as fast as F16; on this Mac's CPU F16 is faster (37 against 22 docs/s) because Apple Silicon multiplies half-precision in hardware, but Macs with Apple Silicon always use Metal. The CPU path matters on x86 machines, where llama.cpp's int8 kernels favour Q8_0. Q8_0 also halves the download and the memory, and its vectors match the reference at cosine 0.9993.

**Dimension:** decided after the 512 / 256 evaluation on the user's history. At 14.4M pages the resident part of the index is 2.3 GB at 1024 dimensions and 0.9 GB at 256 (see "Weights, memory and disk"), so this decides whether the largest archive fits the 8 GB target.

## Runtime: llama.cpp

| Qwen3-Embedding-0.6B, M5 Max | Metal | 4 CPU threads | Peak memory                      |
| ---------------------------- | ----- | ------------- | -------------------------------- |
| llama.cpp (4k-token batches) | 665   | 37            | 1.0–1.4 GiB + the mapped weights |
| candle main (batch 16)       | 190   | 15.5          | 2.3–4.1 GiB                      |

The rule set before measuring was: candle if its CPU speed is within 1.5 times of llama.cpp's best, Metal works with batching, and the footprint at the build's batch stays under 1.5 GB. candle reaches the right vectors at every precision but misses the rest. The footprints are not like for like: llama.cpp maps the GGUF, and clean file pages do not count in a process footprint, so its weights (639 MB at Q8_0) come on top of its figure; candle copies its weights into its own tensors, which do count. Even so llama.cpp needs about 1.7–2 GB against candle's 3.3 GB and more. It also does not build from main today: its workspace asks for `zip` 8.6.0, and every 8.x release is yanked.

No 4-core x86 machine was measured. The CPU figures come from the M5 Max limited to 4 threads, which is faster per core than the target's 3 GHz x86; expect the target to be several times slower. A one-off run on a 4-vCPU x86 CI runner would give a real figure for both runtimes if wanted.

How llama.cpp enters the build:

|                        | vcpkg overlay port + our own bindings (recommended)                                                                                                                                                                                                                                                              | `llama-cpp-2` / `llama-cpp-sys-2` 0.1.160                                                                                   |
| ---------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------- |
| Code we run            | llama.cpp and ggml (ggml-org) built by vcpkg (Microsoft), our bindings                                                                                                                                                                                                                                           | the same llama.cpp, vendored by a crate with a 1,572-line build script, published by one person (657 stars, 1.6M downloads) |
| Supply-chain rule      | passes                                                                                                                                                                                                                                                                                                           | fails the star gate: needs the user's approval                                                                              |
| Native dependency rule | the accepted manager ([native-dependency-management.md](native-dependency-management.md)); its first product library, so `bun run check` gains a fast probe and the proof workflow an Apple Silicon lane (it skips `arm64-osx` today because of the OpenCC port; the `llama-cpp` and `ggml` ports do support it) | a published crate's vendored build (allowed); needs cmake and libclang (bindgen) on every build machine                     |
| llama.cpp version      | ours to choose: an overlay port pins a release and its SHA-512                                                                                                                                                                                                                                                   | whatever the crate vendors (`0dd5814` in 0.1.160)                                                                           |
| Our code               | `extern "C"` bindings for the dozen `llama.h` calls embedding needs, a safe wrapper, updated when we move to a new release                                                                                                                                                                                       | a safe wrapper over the crate's API                                                                                         |

Recommended: the vcpkg overlay port. Every line of native code then comes from ggml-org and Microsoft, the release is ours to pin, and nothing in the build depends on one maintainer. What it costs is our own bindings and wrapper, kept in step with `llama.h` on each upgrade, and the vcpkg build in CI on three platforms. Two details for either path: Metal on macOS (ggml's `metal` feature), and on x86 a CPU backend that picks AVX2 / AVX-512 at run time rather than at build time, so one binary runs fast on every machine.

tract (pure Rust, Sonos) was not measured. It would only matter if native code were refused altogether.

## Searching while it builds

A first build of the transformer index takes about 25 minutes per million pages on this M5 Max's GPU and many hours on a 4-core CPU. Three shapes were considered:

1. **Another model choice.** As with switching to an external model today: search keeps using the complete static index until the transformer index is complete, then the static index is deleted. Nothing new to build, but on a machine without a GPU nothing improves for a day or more, and the order of the build makes no difference to anyone.
2. **The transformer layered over a permanent static base.** Both indexes kept and searched for good. Every page is always covered, but two indexes are built and stored forever and every search embeds the query twice.
3. **Reranking only** (05 §5): the transformer rescores the static search's top results. Pages the static model misses never reach the transformer.

Chosen: **1, with what the new index already has searched from the start.** While the chosen index is incomplete, a search reads both the complete serving index and the chosen one, and fuses the two semantic lists with the lexical list by reciprocal rank. RRF already fuses lexical and semantic results by rank, so scores from different models never meet. Pages the chosen index has not reached still come from the serving index; every page stays findable. When the chosen index completes it serves alone and the old one is deleted, as today. The same holds when switching between external models. While a build runs, each search embeds its query with both models; with an external model that is a second request per search, for the length of the build only.

## Build order

A build walks visits by ascending history ID today, oldest first. Every build will walk newest first: recent pages are the ones searched most, and with the fusion above they reach the better model first. The cursor stays a single watermark, walked downwards; pause, resume and the update after each backup work as they do now.

## Weights, memory and disk

- **Download:** consent-gated, SHA-256-pinned and swapped in atomically, through `model_files.rs` like the static model.
- **Loaded** while a build runs and for searches, released after a few idle minutes. The static model stays resident (73–76 MiB).
- **No re-hash on every load** (done 2026-10-25, `ai/model_files.rs` `model_files_verified`). A cache miss used to re-verify every model file, about 1 s per 512 MB on this Mac and more on the target, and so did every app snapshot's model status. The verified marker now records each file's digest, size and modification time when it passed a full hash; loads and the status trust it while those match, and a full re-hash runs only when one differs (or the marker is missing or from an older build). The hash guards against a broken or half-written download, not against someone rewriting a file and restoring its size and time.
- **Batches** are capped by tokens, not texts, so one long page cannot blow up a batch.
- **Vectors** per unique page (binary plane resident, int8 plane and f32 source on disk):

| Dimension | Binary (resident) | int8 (disk) | f32 source (disk) | 1M pages, resident / disk | 14.4M pages, resident / disk |
| --------: | ----------------: | ----------: | ----------------: | ------------------------- | ---------------------------- |
|      1024 |             160 B |      1024 B |            4096 B | 160 MB / 5.1 GB           | 2.3 GB / 74 GB               |
|       512 |              96 B |       512 B |            2048 B | 96 MB / 2.6 GB            | 1.4 GB / 37 GB               |
|       256 |              64 B |       256 B |            1024 B | 64 MB / 1.3 GB            | 0.9 GB / 18 GB               |

Content dedup leaves far fewer unique pages than visits (131,322 pages for the user's 264,388 Takeout visits), but the 14.4M column is the worst case the 8 GB target has to survive, next to the transformer's own 1.7–2 GB while it builds.

## Settings

Settings → AI, the semantic search model, gains a second built-in choice:

- **Built in · fast**: potion-multilingual-128M, a static model. Very fast on any computer; reads words without their context.
- **Built in · better**: Qwen3-Embedding-0.6B. Reads words in context and finds pages by meaning much better. A one-time download of 639 MB; it runs on this Mac's graphics processor (elsewhere: on the processor). Building the index takes about T for this archive's N pages, measured on this computer, and search uses what is built from the start.

T comes from timing the first batches on the user's machine, as the progress row already does, never from a guess.

## Steps

1. llama.cpp in the build by the chosen path, Metal on macOS, run-time CPU dispatch on x86; the probe in `bun run check`.
2. The engine: load, tokenize with the model's own tokenizer, batch by tokens, last-token pooling, query instruction; parity against the sentence-transformers reference vectors, env-gated like the static parity test.
3. Model files: manifest, consent-gated download, trusted marker with size and modification time.
4. Builds walk newest first.
5. Search fuses an incomplete chosen index with the complete serving index.
6. Settings in en / zh-CN / zh-TW. E2E with a tiny Qwen3-architecture GGUF with random weights committed as a fixture (a few MB), so the real engine, download, build, fusion and switch run in the app; quality stays with the parity test and the evaluation.
7. Docs: `intelligence.md` §1, `semantic-index-control.md`, `tech-stack.md`, D2 in `02`, `05` §4 / §7 / §8 marked retired, CHANGELOG.
